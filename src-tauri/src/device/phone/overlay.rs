use tauri::AppHandle;
#[cfg(target_os = "android")]
use serde::Deserialize;
#[cfg(target_os = "android")]
use tauri::Manager;

/// Used to deserialize the `{ "value": bool }` response from isCancelled.
#[cfg(target_os = "android")]
#[derive(Debug, Deserialize)]
struct BoolResult {
    value: bool,
}

/// A zero-sized type that serializes to `{}` with no heap allocation.
/// Replaces `json!({})` which allocates a new serde_json::Map on every call.
/// is_cancelled() is called on every streaming token, so this matters.
#[cfg(target_os = "android")]
#[derive(serde::Serialize)]
struct NoArgs {}

/// Show the floating recording-dot overlay above all apps.
/// Silently does nothing on desktop or if SYSTEM_ALERT_WINDOW is not granted.
pub fn show_overlay(app: &AppHandle) {
    #[cfg(target_os = "android")]
    {
        use crate::device::phone::plugin::PhoneControlHandle;
        let handle = app.state::<PhoneControlHandle<tauri::Wry>>();
        let _ = handle.0.run_mobile_plugin::<serde_json::Value>("showOverlay", NoArgs {});
    }
    let _ = app;
}

/// Hide the floating overlay.
pub fn hide_overlay(app: &AppHandle) {
    #[cfg(target_os = "android")]
    {
        use crate::device::phone::plugin::PhoneControlHandle;
        let handle = app.state::<PhoneControlHandle<tauri::Wry>>();
        let _ = handle.0.run_mobile_plugin::<serde_json::Value>("hideOverlay", NoArgs {});
    }
    let _ = app;
}

/// Returns true if the user tapped the overlay cancel button since the last call.
/// The flag is atomically reset on read — safe to poll every round.
pub fn is_cancelled(app: &AppHandle) -> bool {
    #[cfg(target_os = "android")]
    {
        use crate::device::phone::plugin::PhoneControlHandle;
        let handle = app.state::<PhoneControlHandle<tauri::Wry>>();
        if let Ok(result) = handle.0.run_mobile_plugin::<BoolResult>("isCancelled", NoArgs {}) {
            return result.value;
        }
    }
    let _ = app;
    false
}
