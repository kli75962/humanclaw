use std::sync::Mutex;
use std::cell::RefCell;
use std::rc::Rc;
use tauri::Emitter;

use super::types::{OverlayCmd, LatestFrameData};
use super::draw::{BORDER_INSET, BORDER_W, ARM, BTN_SIZE, close_cx, close_cy, draw_circle_button, hit_close};
use super::resize::{ResizeInfo, make_resize_info, apply_resize};

pub fn create_overlay(app: tauri::AppHandle, latest_frame: std::sync::Arc<Mutex<Option<LatestFrameData>>>) -> async_channel::Sender<OverlayCmd> {
    use gtk::prelude::*;
    use glib::clone::Downgrade;

    let (tx, rx) = async_channel::unbounded::<OverlayCmd>();

    // ── Window ───────────────────────────────────────────────────────────
    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_decorated(false);
    window.set_app_paintable(true);
    window.set_skip_taskbar_hint(true);
    window.set_skip_pager_hint(true);
    window.set_keep_above(true);
    window.set_accept_focus(false);
    window.set_type_hint(gdk::WindowTypeHint::Utility);

    if let Some(screen) = gtk::prelude::WidgetExt::screen(&window) as Option<gdk::Screen> {
        if let Some(visual) = screen.rgba_visual() {
            window.set_visual(Some(&visual));
        }
    }
    window.set_default_size(400, 600);

    // override-redirect: WM ignores this window entirely — no edge clamping,
    // no focus stealing, window can be placed at any (x, y) including negative.
    window.realize();
    if let Some(gdk_win) = gtk::prelude::WidgetExt::window(&window) {
        gdk_win.set_override_redirect(true);
    }

    // ── Shared state ─────────────────────────────────────────────────────
    let frame_state:    Rc<RefCell<Option<cairo::ImageSurface>>> = Rc::new(RefCell::new(None));
    let hovered:        Rc<RefCell<bool>>               = Rc::new(RefCell::new(false));
    let dragging:       Rc<RefCell<bool>>               = Rc::new(RefCell::new(false));
    let drag_offset:    Rc<RefCell<(i32, i32)>>         = Rc::new(RefCell::new((0, 0)));
    let resize_info:    Rc<RefCell<Option<ResizeInfo>>> = Rc::new(RefCell::new(None));
    // Natural aspect ratio (width/height) passed from JS via show_live2d_overlay.
    let nat_aspect_st:  Rc<RefCell<f64>>                = Rc::new(RefCell::new(0.0));

    // ── Draw ─────────────────────────────────────────────────────────────
    let frame_draw   = frame_state.clone();
    let hovered_draw = hovered.clone();
    window.connect_draw(move |w, cr| {
        let alloc = w.allocation();
        let win_w = alloc.width();
        let win_h = alloc.height();
        let ww    = win_w as f64;
        let wh    = win_h as f64;

        // 1. Clear to fully transparent (Operator::Source = REPLACE, no ghost trails).
        cr.set_operator(cairo::Operator::Source);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        let _ = cr.paint();

        // 2. Blit character frame.
        if let Some(ref surface) = *frame_draw.borrow() {
            let sw = surface.width()  as f64;
            let sh = surface.height() as f64;
            if sw > 0.0 && sh > 0.0 { cr.scale(ww / sw, wh / sh); }
            cr.set_operator(cairo::Operator::Source);
            cr.set_source_surface(surface, 0.0, 0.0).ok();
            let _ = cr.paint();
            cr.identity_matrix();
        }

        // 3. Hover UI — border inset from edge, corner brackets, × button.
        if *hovered_draw.borrow() {
            cr.set_operator(cairo::Operator::Over);

            // Border drawn BORDER_INSET px inside the window edge
            let i = BORDER_INSET;
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.55);
            cr.set_line_width(BORDER_W);
            cr.rectangle(i, i, ww - i * 2.0, wh - i * 2.0);
            let _ = cr.stroke();

            // Corner bracket "L" shapes — also inset
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.90);
            cr.set_line_width(BORDER_W + 1.0);
            for &(cx, cy, sx, sy) in &[
                (i,        i,        1.0_f64,  1.0_f64),
                (ww - i,   i,       -1.0,       1.0),
                (i,        wh - i,   1.0,      -1.0),
                (ww - i,   wh - i,  -1.0,      -1.0),
            ] {
                cr.move_to(cx + sx * ARM, cy);
                cr.line_to(cx, cy);
                cr.line_to(cx, cy + sy * ARM);
                let _ = cr.stroke();
            }

            // × close button — top-right, clear of the TR resize grab zone
            let r = BTN_SIZE as f64 / 2.0;
            draw_circle_button(cr, close_cx(win_w), close_cy(), r,
                               "×", (0.71, 0.18, 0.18, 0.88));
        }

        glib::Propagation::Stop
    });

    // ── Input events ─────────────────────────────────────────────────────
    window.add_events(
        gdk::EventMask::BUTTON_PRESS_MASK
        | gdk::EventMask::BUTTON_RELEASE_MASK
        | gdk::EventMask::POINTER_MOTION_MASK
        | gdk::EventMask::ENTER_NOTIFY_MASK
        | gdk::EventMask::LEAVE_NOTIFY_MASK,
    );

    let hovered_enter = hovered.clone();
    window.connect_enter_notify_event(move |w, _| {
        *hovered_enter.borrow_mut() = true;
        w.queue_draw();
        glib::Propagation::Proceed
    });

    let hovered_leave = hovered.clone();
    window.connect_leave_notify_event(move |w, _| {
        *hovered_leave.borrow_mut() = false;
        w.queue_draw();
        glib::Propagation::Proceed
    });

    let app_press       = app.clone();
    let drag_press      = dragging.clone();
    let offset_press    = drag_offset.clone();
    let resize_press    = resize_info.clone();
    let nat_aspect_press = nat_aspect_st.clone();
    window.connect_button_press_event(move |w, e| {
        if e.button() != 1 { return glib::Propagation::Proceed; }
        let alloc    = w.allocation();
        let (ww, wh) = (alloc.width(), alloc.height());
        let (mx, my) = e.position();

        // × close button — checked first to guarantee it wins over resize zones.
        if hit_close(mx, my, ww) {
            w.hide();
            use tauri::Manager;
            if let Some(lw) = app_press.get_webview_window("live2d") {
                let _ = lw.close();
            }
            return glib::Propagation::Stop;
        }

        // Resize grab band (all 4 edges + 4 corners).
        let (wx, wy) = w.position();
        if let Some(ri) = make_resize_info(mx, my, wx, wy, ww, wh, *nat_aspect_press.borrow()) {
            *resize_press.borrow_mut() = Some(ri);
            return glib::Propagation::Stop;
        }

        // Body drag — moves the window freely, bypassing WM edge clamping.
        let (root_x, root_y) = e.root();
        let (win_x, win_y)   = w.position();
        *offset_press.borrow_mut() = (root_x as i32 - win_x, root_y as i32 - win_y);
        *drag_press.borrow_mut()   = true;
        glib::Propagation::Stop
    });

    let drag_motion   = dragging.clone();
    let offset_motion = drag_offset.clone();
    let resize_motion = resize_info.clone();
    window.connect_motion_notify_event(move |w, e| {
        if let Some(ref ri) = *resize_motion.borrow() {
            let (rx, ry) = e.root();
            let (nx, ny, nw, nh) = apply_resize(ri, rx, ry);
            w.resize(nw, nh);
            w.move_(nx, ny);
            return glib::Propagation::Proceed;
        }
        if *drag_motion.borrow() {
            let (root_x, root_y) = e.root();
            let (off_x, off_y)   = *offset_motion.borrow();
            w.move_(root_x as i32 - off_x, root_y as i32 - off_y);
        }
        glib::Propagation::Proceed
    });

    // On button release: end drag; if resize was active emit final size to JS.
    let drag_release   = dragging.clone();
    let resize_release = resize_info.clone();
    let app_release    = app.clone();
    window.connect_button_release_event(move |w, _| {
        *drag_release.borrow_mut() = false;
        if resize_release.borrow().is_some() {
            *resize_release.borrow_mut() = None;
            let alloc    = w.allocation();
            let (wx, wy) = w.position();
            app_release.emit("live2d-resized", serde_json::json!({
                "x": wx, "y": wy,
                "w": alloc.width(), "h": alloc.height()
            })).ok();
        }
        glib::Propagation::Proceed
    });

    // Emit position/size on configure so JS overlayPosRef stays current.
    let app_cfg = app.clone();
    window.connect_configure_event(move |_w, e| {
        let (x, y)   = e.position();
        let (cw, ch) = e.size();
        app_cfg.emit("live2d-moved", serde_json::json!({
            "x": x, "y": y, "w": cw as i32, "h": ch as i32
        })).ok();
        false
    });

    // ── Command receiver ─────────────────────────────────────────────────
    let frame_rx      = frame_state.clone();
    let nat_aspect_rx = nat_aspect_st.clone();
    let weak2: glib::WeakRef<gtk::Window> = Downgrade::downgrade(&window);

    glib::MainContext::default().spawn_local(async move {
        while let Ok(cmd) = rx.recv().await {
            let Some(w) = glib::clone::Upgrade::upgrade(&weak2) else {
                break;
            };

            match cmd {
                OverlayCmd::DrawLatest => {
                    // Take the latest frame; if multiple DrawLatest commands queued up
                    // before glib ran, subsequent finds find None and become no-ops.
                    let Some((pixels, width, height)) = latest_frame.lock().unwrap().take() else {
                        continue;
                    };
                    let wi = width  as i32;
                    let hi = height as i32;
                    let wu = width  as usize;

                    // ── Reuse cairo surface when dimensions haven't changed ─────
                    // Avoids a heap allocation + cairo surface creation per frame.
                    // On the first call, or after a window resize, we create a new
                    // surface; all subsequent same-size calls write in-place.
                    {
                        let mut state = frame_rx.borrow_mut();
                        let reuse = state.as_ref()
                            .map_or(false, |s| s.width() == wi && s.height() == hi);
                        if !reuse {
                            *state = cairo::ImageSurface::create(
                                cairo::Format::ARgb32, wi, hi,
                            ).ok();
                        }

                        if let Some(ref mut surface) = *state {
                            // Obtain stride before the mutable data() borrow.
                            let stride = surface.stride() as usize;
                            if let Ok(mut data) = surface.data() {
                                // Pixels arrive top-down (PIXI stage Y-flipped in JS),
                                // so src and dst are both scanned sequentially — optimal
                                // for the CPU prefetcher and store buffer.
                                // chunks_exact(4) tells LLVM the body is exactly 4 bytes
                                // wide, enabling SSE2/AVX2 auto-vectorization.
                                for (y, src_row) in pixels.chunks_exact(wu * 4).enumerate() {
                                    let dst_off = y * stride;
                                    for (x, s) in src_row.chunks_exact(4).enumerate() {
                                        let di = dst_off + x * 4;
                                        let r = s[0] as u32;
                                        let g = s[1] as u32;
                                        let b = s[2] as u32;
                                        let a = s[3] as u32;
                                        // Fast integer premultiply (no division):
                                        // equivalent to (c * a + 127) / 255.
                                        let pm = |c: u32| -> u8 {
                                            let p = c * a + 128;
                                            ((p + (p >> 8)) >> 8) as u8
                                        };
                                        data[di]   = pm(b);
                                        data[di+1] = pm(g);
                                        data[di+2] = pm(r);
                                        data[di+3] = a as u8;
                                    }
                                }
                                // ImageSurfaceData drops here → marks surface dirty.
                            }
                        }
                    } // RefMut dropped before queue_draw to avoid borrow conflicts.
                    w.queue_draw();
                }
                OverlayCmd::Show { x, y, width, height, nat_aspect } => {
                    if nat_aspect > 0.0 { *nat_aspect_rx.borrow_mut() = nat_aspect; }
                    if width > 0 && height > 0 {
                        w.resize(width, height);
                        w.move_(x, y);
                        if !gtk::prelude::WidgetExt::is_visible(&w) {
                            w.show_all();
                            w.present();
                        }
                    }
                }
                OverlayCmd::Hide => { w.hide(); }
            }
        }
    });

    tx
}
