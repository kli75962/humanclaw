use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use futures_util::StreamExt;
use tauri::{AppHandle, Emitter};
use tauri::async_runtime::JoinHandle;

use crate::ai::types::{AgentStatusPayload, StreamPayload};
use crate::session::store;
use crate::session::types::{PairedDevice, PcPermissions};
use super::sse::SyncEvent;

/// Map of peer device_id → background subscriber task.
fn handles() -> &'static Mutex<HashMap<String, JoinHandle<()>>> {
    static HANDLES: OnceLock<Mutex<HashMap<String, JoinHandle<()>>>> = OnceLock::new();
    HANDLES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Start subscriber tasks for every currently-paired peer (idempotent — re-running
/// will skip peers that already have a live task and start ones that don't).
pub fn start_subscribers(app: &AppHandle) {
    let cfg = store::bootstrap(app);
    for peer in cfg.paired_devices {
        ensure_subscriber(app, peer);
    }
}

/// Idempotently start a subscriber for one peer. Safe to call from the pairing
/// flow; will replace any dead task for the same device_id.
pub fn ensure_subscriber(app: &AppHandle, peer: PairedDevice) {
    let mut map = handles().lock().unwrap();
    // If a task already exists for this peer, abort and replace it — the
    // reconnect loop handles transient failures inside the task itself.
    if let Some(existing) = map.remove(&peer.device_id) {
        existing.abort();
    }
    let app_clone = app.clone();
    let device_id = peer.device_id.clone();
    let handle = tauri::async_runtime::spawn(async move {
        subscribe_forever(app_clone, peer).await;
    });
    map.insert(device_id, handle);
}

/// Stop a subscriber when a peer is unpaired.
pub fn stop_subscriber(device_id: &str) {
    let mut map = handles().lock().unwrap();
    if let Some(handle) = map.remove(device_id) {
        handle.abort();
    }
}

/// Open a long-lived SSE connection to one peer and reconnect on failure.
/// Each event received is applied locally (file writes + Tauri event emits).
async fn subscribe_forever(app: AppHandle, peer: PairedDevice) {
    loop {
        if let Err(e) = subscribe_once(&app, &peer).await {
            eprintln!("[sse] subscribe to {} failed: {e}", peer.address);
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

async fn subscribe_once(app: &AppHandle, peer: &PairedDevice) -> Result<(), String> {
    let key = store::bootstrap(app).hash_key;
    let url = format!("http://{}/events", peer.address);

    // Use a no-timeout client so the long-lived SSE stream isn't killed at 5s.
    let client = crate::network::ollama_proxy_client();
    let resp = client
        .get(&url)
        .query(&[("key", key.as_str())])
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("status {}", resp.status()));
    }

    let mut stream = resp.bytes_stream();
    let mut buf = String::new();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| e.to_string())?;
        buf.push_str(&String::from_utf8_lossy(&bytes));

        // SSE events are separated by blank lines (\n\n). Each event may have
        // multiple `data:` lines that concatenate.
        while let Some(idx) = buf.find("\n\n") {
            let raw_event = buf[..idx].to_string();
            buf.drain(..idx + 2);

            let data = raw_event
                .lines()
                .filter_map(|l| l.strip_prefix("data:").map(str::trim))
                .collect::<Vec<_>>()
                .join("\n");
            if data.is_empty() { continue; }

            match serde_json::from_str::<SyncEvent>(&data) {
                Ok(event) => apply_event(app, event),
                Err(e) => eprintln!("[sse] bad event: {e} raw={data}"),
            }
        }
    }
    Ok(())
}

fn apply_event(app: &AppHandle, event: SyncEvent) {
    match event {
        SyncEvent::StreamChunk { chat_id, content, done, brief } => {
            let _ = app.emit("ollama-stream", StreamPayload {
                chat_id: Some(chat_id),
                remote: true,
                content,
                done,
                brief,
            });
        }
        SyncEvent::AgentStatus { chat_id, message } => {
            let _ = app.emit("agent-status", AgentStatusPayload {
                chat_id: Some(chat_id),
                remote: true,
                message,
            });
        }
        SyncEvent::ChatUpdated { chat_id: _ } => {
            let _ = app.emit("chat-sync-updated", serde_json::json!({}));
        }
        SyncEvent::SettingsChanged { field, value } => {
            apply_setting_locally(app, &field, &value);
            let _ = app.emit("session-changed", serde_json::json!({}));
        }
        SyncEvent::Cancel => {
            crate::ai::CHAT_CANCEL.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

fn apply_setting_locally(app: &AppHandle, field: &str, value: &serde_json::Value) {
    match field {
        "persona" => {
            if let Some(s) = value.as_str() {
                let _ = store::set_persona_quiet(app, s);
            }
        }
        "ollama_model" => {
            if let Some(s) = value.as_str() {
                let _ = store::set_ollama_model(app, s);
            }
        }
        "pc_permissions" => {
            if let Ok(perms) = serde_json::from_value::<PcPermissions>(value.clone()) {
                let _ = store::set_pc_permissions_quiet(app, perms);
            }
        }
        _ => {}
    }
}
