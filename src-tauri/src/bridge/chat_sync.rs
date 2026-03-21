use std::collections::HashMap;
use tauri::AppHandle;

use crate::memory::{
    export_chat_sync_payload, import_chat_sync_payload, ChatSyncChat, ChatSyncPayload,
};
use crate::session::store;
use crate::session::types::PairedDevice;

fn merge_chat_payloads(local: ChatSyncPayload, remote: ChatSyncPayload) -> ChatSyncPayload {
    let mut by_id: HashMap<String, ChatSyncChat> = HashMap::new();

    for chat in local.chats.into_iter().chain(remote.chats.into_iter()) {
        match by_id.get(&chat.id) {
            None => {
                by_id.insert(chat.id.clone(), chat);
            }
            Some(existing) => {
                // Prefer the version with more messages; if equal, keep newer title.
                let pick_new = chat.messages.len() > existing.messages.len();
                if pick_new {
                    by_id.insert(chat.id.clone(), chat);
                }
            }
        }
    }

    let mut chats: Vec<ChatSyncChat> = by_id.into_values().collect();
    chats.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    ChatSyncPayload { chats }
}

pub async fn push_chat_sync_to_peer(
    app: &AppHandle,
    peer: &PairedDevice,
    payload: &ChatSyncPayload,
    replace: bool,
) -> Result<(), String> {
    let key = store::bootstrap(app).hash_key;
    let url = format!("http://{}/chat/import", peer.address);
    let body = serde_json::json!({
        "key": key,
        "payload": payload,
        "replace": replace,
    });

    let resp = super::bridge_client()
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("peer sync failed: {}", resp.status()));
    }
    Ok(())
}

async fn fetch_peer_chat_sync(app: &AppHandle, peer: &PairedDevice) -> Result<ChatSyncPayload, String> {
    let key = store::bootstrap(app).hash_key;
    let url = format!("http://{}/chat/export", peer.address);

    let resp = super::bridge_client()
        .get(url)
        .query(&[("key", key)])
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("peer export failed: {}", resp.status()));
    }

    resp.json::<ChatSyncPayload>()
        .await
        .map_err(|e| e.to_string())
}

pub async fn sync_after_pair(app: &AppHandle, peer: &PairedDevice) {
    let local = export_chat_sync_payload(app);
    let remote = fetch_peer_chat_sync(app, peer).await.unwrap_or(ChatSyncPayload { chats: vec![] });

    let merged = merge_chat_payloads(local, remote);

    let _ = push_chat_sync_to_peer(app, peer, &merged, true).await;
    let _ = import_chat_sync_payload(app, merged, true);
}

pub async fn sync_to_all_peers(app: &AppHandle) {
    let cfg = store::bootstrap(app);
    if cfg.paired_devices.is_empty() {
        return;
    }

    let payload = export_chat_sync_payload(app);
    for peer in &cfg.paired_devices {
        let _ = push_chat_sync_to_peer(app, peer, &payload, true).await;
    }
}
