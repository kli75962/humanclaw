use tauri::AppHandle;
#[cfg(target_os = "android")]
use serde::Deserialize;
#[cfg(target_os = "android")]
use serde_json::json;
#[cfg(target_os = "android")]
use tauri::Manager;

/// Used to deserialize the `{ "value": bool }` response from isCancelled.
#[cfg(target_os = "android")]
#[derive(Debug, Deserialize)]
struct BoolResult {
    value: bool,
}

/// Show the floating recording-dot overlay above all apps.
/// Silently does nothing on desktop or if SYSTEM_ALERT_WINDOW is not granted.
pub fn show_overlay(app: &AppHandle) {
    #[cfg(target_os = "android")]
    {
        use crate::phone::plugin::PhoneControlHandle;
        let handle = app.state::<PhoneControlHandle<tauri::Wry>>();
        let _ = handle.0.run_mobile_plugin::<serde_json::Value>("showOverlay", json!({}));
    }
    let _ = app;
}

/// Hide the floating overlay.
pub fn hide_overlay(app: &AppHandle) {
    #[cfg(target_os = "android")]
    {
        use crate::phone::plugin::PhoneControlHandle;
        let handle = app.state::<PhoneControlHandle<tauri::Wry>>();
        let _ = handle.0.run_mobile_plugin::<serde_json::Value>("hideOverlay", json!({}));
    }
    let _ = app;
}

/// Returns true if the user tapped the overlay cancel button since the last call.
/// The flag is atomically reset on read — safe to poll every round.
pub fn is_cancelled(app: &AppHandle) -> bool {
    #[cfg(target_os = "android")]
    {
        use crate::phone::plugin::PhoneControlHandle;
        let handle = app.state::<PhoneControlHandle<tauri::Wry>>();
        if let Ok(result) = handle.0.run_mobile_plugin::<BoolResult>("isCancelled", json!({})) {
            return result.value;
        }
    }
    let _ = app;
    false
}
