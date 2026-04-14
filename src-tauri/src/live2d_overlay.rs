/// Native GTK overlay window for Live2D character display on Linux.
///
/// Hover UI:
///   • 2 px border drawn BORDER_INSET px inside the window edge.
///   • L-shaped corner bracket handles at all four corners.
///   • × close button in the top-right corner (clear of the TR grab zone).
///   • Grab band: 0..GRAB_BAND px from each edge → aspect-ratio-locked resize.
///   • Anything beyond the grab band (window body) → free drag.

use std::sync::Mutex;
use std::cell::RefCell;
use std::rc::Rc;
use tauri::Emitter;

const BTN_SIZE:     i32 = 28;
/// Gap from window corner to close-button centre — large enough that the
/// button never overlaps the TR corner grab zone (GRAB_BAND = 16).
const BTN_PAD:      i32 = 14;
/// Width (px from each edge) of the resize grab band.
const GRAB_BAND:    i32 = 16;
/// Visual border is drawn this many px inside the window edge.
const BORDER_INSET: f64 = 5.0;
const BORDER_W:     f64 = 2.0;
const ARM:          f64 = 14.0; // corner bracket arm length

// ── helpers ──────────────────────────────────────────────────────────────────

fn close_cx(win_w: i32) -> f64 { (win_w - BTN_PAD - BTN_SIZE / 2) as f64 }
fn close_cy()              -> f64 { (BTN_PAD + BTN_SIZE / 2) as f64 }

fn hit_close(mx: f64, my: f64, win_w: i32) -> bool {
    let r = BTN_SIZE as f64 / 2.0;
    (mx - close_cx(win_w)).hypot(my - close_cy()) <= r
}

fn draw_circle_button(cr: &cairo::Context, cx: f64, cy: f64, r: f64,
                      label: &str, bg: (f64, f64, f64, f64)) {
    cr.set_operator(cairo::Operator::Over);
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.35);
    cr.arc(cx + 1.0, cy + 2.0, r, 0.0, 2.0 * std::f64::consts::PI);
    let _ = cr.fill();
    cr.set_source_rgba(bg.0, bg.1, bg.2, bg.3);
    cr.arc(cx, cy, r, 0.0, 2.0 * std::f64::consts::PI);
    let _ = cr.fill();
    cr.set_source_rgba(1.0, 1.0, 1.0, 1.0);
    cr.select_font_face("Sans", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
    cr.set_font_size(16.0);
    if let Ok(te) = cr.text_extents(label) {
        cr.move_to(cx - te.width() / 2.0 - te.x_bearing(),
                   cy + te.height() / 2.0 + te.y_bearing() + te.height());
        let _ = cr.show_text(label);
    }
}

// ── resize state ─────────────────────────────────────────────────────────────

/// Which side/corner is being dragged — encodes how to position the window from
/// the fixed anchor.
#[derive(Clone, Copy)]
enum PosMode {
    CornerTL, CornerTR, CornerBL, CornerBR,
    EdgeTop, EdgeBot, EdgeLeft, EdgeRight,
}

struct ResizeInfo {
    /// Fixed screen-coord anchor that does not move during the drag.
    anchor_sx: f64,
    anchor_sy: f64,
    pos_mode:  PosMode,
    /// Width / height aspect ratio to maintain.
    aspect:    f64,
}

/// Try to start a resize from the given pointer position inside the window.
/// Returns None if the click is outside the grab band OR on the close button.
/// `nat_aspect` is the model's natural width/height ratio (0 = fall back to window ratio).
fn make_resize_info(
    mx: f64, my: f64,
    win_x: i32, win_y: i32, win_w: i32, win_h: i32,
    nat_aspect: f64,
) -> Option<ResizeInfo> {
    // Never treat a close-button click as a resize.
    if hit_close(mx, my, win_w) { return None; }

    let gb  = GRAB_BAND as f64;
    let ww  = win_w as f64;
    let wh  = win_h as f64;
    let near_l = mx < gb;
    let near_r = mx > ww - gb;
    let near_t = my < gb;
    let near_b = my > wh - gb;
    if !near_l && !near_r && !near_t && !near_b { return None; }

    let fx     = win_x as f64;
    let fy     = win_y as f64;
    // Use the true model aspect ratio when available; fall back to window ratio.
    let aspect = if nat_aspect > 0.0 { nat_aspect } else { ww / wh };

    let (anchor_sx, anchor_sy, pos_mode) = match (near_t, near_b, near_l, near_r) {
        // corners — anchor is the diagonally opposite corner
        (true, _, true, _)  => (fx + ww,       fy + wh,       PosMode::CornerBR),
        (true, _, _, true)  => (fx,             fy + wh,       PosMode::CornerBL),
        (_, true, true, _)  => (fx + ww,        fy,            PosMode::CornerTR),
        (_, true, _, true)  => (fx,             fy,            PosMode::CornerTL),
        // edges — anchor is the centre of the opposite edge
        (true, _, _, _)     => (fx + ww / 2.0,  fy + wh,       PosMode::EdgeTop),
        (_, true, _, _)     => (fx + ww / 2.0,  fy,            PosMode::EdgeBot),
        (_, _, true, _)     => (fx + ww,        fy + wh / 2.0, PosMode::EdgeLeft),
        _                   => (fx,             fy + wh / 2.0, PosMode::EdgeRight),
    };
    Some(ResizeInfo { anchor_sx, anchor_sy, pos_mode, aspect })
}

/// Compute new (w, h) from mouse root position, then (x, y) from anchor.
fn apply_resize(ri: &ResizeInfo, rx: f64, ry: f64) -> (i32, i32, i32, i32) {
    let dx = (rx - ri.anchor_sx).abs().max(80.0);
    let dy = (ry - ri.anchor_sy).abs().max(80.0);

    let (new_w, new_h): (i32, i32) = match ri.pos_mode {
        // Top/bottom edge: height is the primary axis.
        PosMode::EdgeTop | PosMode::EdgeBot => {
            ((dy * ri.aspect) as i32, dy as i32)
        }
        // Left/right edge: width is the primary axis.
        PosMode::EdgeLeft | PosMode::EdgeRight => {
            (dx as i32, (dx / ri.aspect) as i32)
        }
        // Corners: whichever dimension is proportionally larger wins.
        _ => if dx / ri.aspect >= dy {
            (dx as i32, (dx / ri.aspect) as i32)
        } else {
            ((dy * ri.aspect) as i32, dy as i32)
        },
    };
    let new_w = new_w.max(80);
    let new_h = new_h.max(80);

    let ax = ri.anchor_sx as i32;
    let ay = ri.anchor_sy as i32;
    let (new_x, new_y) = match ri.pos_mode {
        PosMode::CornerTL  => (ax,               ay),
        PosMode::CornerTR  => (ax - new_w,        ay),
        PosMode::CornerBL  => (ax,                ay - new_h),
        PosMode::CornerBR  => (ax - new_w,        ay - new_h),
        PosMode::EdgeTop   => (ax - new_w / 2,    ay - new_h),
        PosMode::EdgeBot   => (ax - new_w / 2,    ay),
        PosMode::EdgeLeft  => (ax - new_w,        ay - new_h / 2),
        PosMode::EdgeRight => (ax,                ay - new_h / 2),
    };
    (new_x, new_y, new_w, new_h)
}

// ── create_overlay ────────────────────────────────────────────────────────────

pub enum OverlayCmd {
    /// A new frame was stored in LatestFrame — read it and paint.
    DrawLatest,
    /// `nat_aspect` = nat_width / nat_height from JS (0.0 = unknown, use window ratio).
    Show  { x: i32, y: i32, width: i32, height: i32, nat_aspect: f64 },
    Hide,
}

pub struct OverlaySender(pub Mutex<glib::Sender<OverlayCmd>>);

/// Shared slot holding the most recent RGBA frame from JS.
/// IPC handler overwrites it on every incoming frame; the glib main loop
/// reads-and-clears it when it processes a DrawLatest command.
/// Intermediate frames that arrive before glib runs are silently replaced,
/// so Y-flip + BGRA conversion only runs for frames that actually get drawn.
pub type LatestFrameData = (Vec<u8>, u32, u32); // (pixels, width, height)
pub struct LatestFrame(pub std::sync::Arc<Mutex<Option<LatestFrameData>>>);

pub fn create_overlay(app: tauri::AppHandle, latest_frame: std::sync::Arc<Mutex<Option<LatestFrameData>>>) -> glib::Sender<OverlayCmd> {
    use gtk::prelude::*;
    use glib::clone::Downgrade;

    let (tx, rx) = glib::MainContext::channel::<OverlayCmd>(glib::Priority::DEFAULT);

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
    rx.attach(None, move |cmd| {
        let Some(w) = glib::clone::Upgrade::upgrade(&weak2) else {
            return glib::ControlFlow::Break;
        };
        match cmd {
            OverlayCmd::DrawLatest => {
                // Take the latest frame; if multiple DrawLatest commands queued up
                // before glib ran, subsequent finds find None and become no-ops.
                let Some((pixels, width, height)) = latest_frame.lock().unwrap().take() else {
                    return glib::ControlFlow::Continue;
                };
                let wi = width  as i32;
                let hi = height as i32;
                let wu = width  as usize;
                let hu = height as usize;

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
        glib::ControlFlow::Continue
    });

    tx
}
