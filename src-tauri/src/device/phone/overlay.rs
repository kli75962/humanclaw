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
///
/// On Android: calls the local Kotlin plugin (silently no-ops if SYSTEM_ALERT_WINDOW
/// is not granted).
///
/// On desktop: spawns a background task that POSTs `/overlay { action: "show" }` to
/// every paired peer so any paired phone sees a visual indicator that an LLM
/// running on this PC is operating it.
pub fn show_overlay(app: &AppHandle) {
    show_overlay_local(app);
    #[cfg(not(target_os = "android"))]
    broadcast_overlay(app, "show");
}

/// Hide the floating overlay.
///
/// On desktop: notifies every paired peer to hide its overlay.
pub fn hide_overlay(app: &AppHandle) {
    hide_overlay_local(app);
    #[cfg(not(target_os = "android"))]
    broadcast_overlay(app, "hide");
}

/// Apply the overlay change *only* to this device — never broadcasts. Called by
/// the `/overlay` HTTP handler so a peer's request never re-broadcasts and
/// creates a loop between paired devices.
pub fn show_overlay_local(app: &AppHandle) {
    #[cfg(target_os = "android")]
    {
        use crate::device::phone::plugin::PhoneControlHandle;
        let handle = app.state::<PhoneControlHandle<tauri::Wry>>();
        let _ = handle.0.run_mobile_plugin::<serde_json::Value>("showOverlay", NoArgs {});
    }
    let _ = app;
}

pub fn hide_overlay_local(app: &AppHandle) {
    #[cfg(target_os = "android")]
    {
        use crate::device::phone::plugin::PhoneControlHandle;
        let handle = app.state::<PhoneControlHandle<tauri::Wry>>();
        let _ = handle.0.run_mobile_plugin::<serde_json::Value>("hideOverlay", NoArgs {});
    }
    let _ = app;
}

#[cfg(not(target_os = "android"))]
fn broadcast_overlay(app: &AppHandle, action: &'static str) {
    let cfg = crate::session::store::bootstrap(app);
    if cfg.paired_devices.is_empty() {
        return;
    }
    let key = cfg.hash_key;
    let peers = cfg.paired_devices;
    tauri::async_runtime::spawn(async move {
        let client = crate::network::bridge_client();
        for peer in &peers {
            let url = format!("http://{}/overlay", peer.address);
            let body = serde_json::json!({ "key": key, "action": action });
            let _ = client.post(&url).json(&body).send().await;
        }
    });
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
