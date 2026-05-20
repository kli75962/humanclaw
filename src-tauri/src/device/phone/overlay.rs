use tauri::AppHandle;
#[cfg(target_os = "android")]
use serde::Deserialize;
#[cfg(target_os = "android")]
use std::sync::{Mutex, OnceLock};
#[cfg(target_os = "android")]
use tauri::async_runtime::JoinHandle;
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
        start_cancel_poller(app);
    }
    let _ = app;
}

pub fn hide_overlay_local(app: &AppHandle) {
    #[cfg(target_os = "android")]
    {
        use crate::device::phone::plugin::PhoneControlHandle;
        let handle = app.state::<PhoneControlHandle<tauri::Wry>>();
        let _ = handle.0.run_mobile_plugin::<serde_json::Value>("hideOverlay", NoArgs {});
        stop_cancel_poller();
    }
    let _ = app;
}

/// Background task that polls the Kotlin overlay's `isCancelled` flag every
/// 200 ms while the overlay is visible. When the user taps the red dot, we
/// broadcast a `Cancel` SSE event to every paired peer so a PC that is currently
/// driving the agent loop will stop. Also sets the local cancel flag so chat
/// loops running on this device exit on the next round check.
#[cfg(target_os = "android")]
fn poller_slot() -> &'static Mutex<Option<JoinHandle<()>>> {
    static SLOT: OnceLock<Mutex<Option<JoinHandle<()>>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

#[cfg(target_os = "android")]
fn start_cancel_poller(app: &AppHandle) {
    let mut slot = poller_slot().lock().unwrap();
    if let Some(existing) = slot.take() {
        existing.abort();
    }
    let app = app.clone();
    let handle = tauri::async_runtime::spawn(async move {
        loop {
            if is_cancelled(&app) {
                crate::ai::CHAT_CANCEL.store(true, std::sync::atomic::Ordering::Relaxed);
                // Belt-and-braces: SSE for subscribed peers, plus a direct POST
                // so even peers whose SSE connection is mid-reconnect still hear
                // the cancel without waiting for the stream to re-establish.
                crate::network::sse::broadcast(crate::network::sse::SyncEvent::Cancel);
                push_cancel_to_peers(&app);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    });
    *slot = Some(handle);
}

#[cfg(target_os = "android")]
fn stop_cancel_poller() {
    if let Some(existing) = poller_slot().lock().unwrap().take() {
        existing.abort();
    }
}

#[cfg(target_os = "android")]
fn push_cancel_to_peers(app: &AppHandle) {
    let cfg = crate::session::store::bootstrap(app);
    if cfg.paired_devices.is_empty() { return; }
    let key = cfg.hash_key;
    let peers = cfg.paired_devices;
    tauri::async_runtime::spawn(async move {
        let client = crate::network::bridge_client();
        for peer in &peers {
            let url = format!("http://{}/cancel", peer.address);
            let body = serde_json::json!({ "key": key });
            let _ = client.post(&url).json(&body).send().await;
        }
    });
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
