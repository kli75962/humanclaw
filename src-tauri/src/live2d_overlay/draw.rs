pub const BTN_SIZE:     i32 = 28;
/// Gap from window corner to close-button centre — large enough that the
/// button never overlaps the TR corner grab zone (GRAB_BAND = 16).
pub const BTN_PAD:      i32 = 14;
/// Width (px from each edge) of the resize grab band.
pub const GRAB_BAND:    i32 = 16;
/// Visual border is drawn this many px inside the window edge.
pub const BORDER_INSET: f64 = 5.0;
pub const BORDER_W:     f64 = 2.0;
pub const ARM:          f64 = 14.0; // corner bracket arm length

pub fn close_cx(win_w: i32) -> f64 { (win_w - BTN_PAD - BTN_SIZE / 2) as f64 }
pub fn close_cy()              -> f64 { (BTN_PAD + BTN_SIZE / 2) as f64 }

pub fn hit_close(mx: f64, my: f64, win_w: i32) -> bool {
    let r = BTN_SIZE as f64 / 2.0;
    (mx - close_cx(win_w)).hypot(my - close_cy()) <= r
}

pub fn draw_circle_button(cr: &cairo::Context, cx: f64, cy: f64, r: f64,
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
