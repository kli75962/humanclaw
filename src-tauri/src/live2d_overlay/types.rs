use std::sync::Mutex;

pub enum OverlayCmd {
    /// A new frame was stored in LatestFrame — read it and paint.
    DrawLatest,
    /// `nat_aspect` = nat_width / nat_height from JS (0.0 = unknown, use window ratio).
    Show  { x: i32, y: i32, width: i32, height: i32, nat_aspect: f64 },
    Hide,
}

pub struct OverlaySender(pub std::sync::Mutex<async_channel::Sender<OverlayCmd>>);

/// Shared slot holding the most recent RGBA frame from JS.
/// IPC handler overwrites it on every incoming frame; the glib main loop
/// reads-and-clears it when it processes a DrawLatest command.
/// Intermediate frames that arrive before glib runs are silently replaced,
/// so Y-flip + BGRA conversion only runs for frames that actually get drawn.
pub type LatestFrameData = (Vec<u8>, u32, u32); // (pixels, width, height)
pub struct LatestFrame(pub std::sync::Arc<Mutex<Option<LatestFrameData>>>);
