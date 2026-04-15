use super::draw::{hit_close, GRAB_BAND};

/// Which side/corner is being dragged — encodes how to position the window from
/// the fixed anchor.
#[derive(Clone, Copy)]
pub enum PosMode {
    CornerTL, CornerTR, CornerBL, CornerBR,
    EdgeTop, EdgeBot, EdgeLeft, EdgeRight,
}

pub struct ResizeInfo {
    /// Fixed screen-coord anchor that does not move during the drag.
    pub anchor_sx: f64,
    pub anchor_sy: f64,
    pub pos_mode:  PosMode,
    /// Width / height aspect ratio to maintain.
    pub aspect:    f64,
}

/// Try to start a resize from the given pointer position inside the window.
/// Returns None if the click is outside the grab band OR on the close button.
/// `nat_aspect` is the model's natural width/height ratio (0 = fall back to window ratio).
pub fn make_resize_info(
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
pub fn apply_resize(ri: &ResizeInfo, rx: f64, ry: f64) -> (i32, i32, i32, i32) {
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
