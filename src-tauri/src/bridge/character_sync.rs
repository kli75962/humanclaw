use tauri::AppHandle;

use crate::characters::{
    export_character_sync_payload, import_character_sync_payload, CharacterSyncPayload,
};
use crate::session::store;
use crate::session::types::PairedDevice;

pub async fn push_character_sync_to_peer(
    app: &AppHandle,
    peer: &PairedDevice,
    payload: &CharacterSyncPayload,
    replace: bool,
) -> Result<(), String> {
    let key = store::bootstrap(app).hash_key;
    let url = format!("http://{}/characters/import", peer.address);
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
        return Err(format!("character sync failed: {}", resp.status()));
    }
    Ok(())
}

async fn fetch_peer_character_sync(
    app: &AppHandle,
    peer: &PairedDevice,
) -> Result<CharacterSyncPayload, String> {
    let key = store::bootstrap(app).hash_key;
    let url = format!("http://{}/characters/export", peer.address);

    let resp = super::bridge_client()
        .get(url)
        .query(&[("key", key)])
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("character export failed: {}", resp.status()));
    }

    resp.json::<CharacterSyncPayload>()
        .await
        .map_err(|e| e.to_string())
}

/// Bidirectional sync after a new pairing — merge local and remote, push merged to both.
pub async fn sync_after_pair(app: &AppHandle, peer: &PairedDevice) {
    let local = export_character_sync_payload(app);
    let remote = fetch_peer_character_sync(app, peer)
        .await
        .unwrap_or(CharacterSyncPayload { characters: vec![] });

    // Merge: union by id, remote wins on conflict (remote is newer post-pair).
    let mut merged = local.characters;
    for incoming in remote.characters {
        if !merged.iter().any(|c| c.id == incoming.id) {
            merged.push(incoming);
        }
    }
    let merged_payload = CharacterSyncPayload { characters: merged };

    let _ = push_character_sync_to_peer(app, peer, &merged_payload, true).await;
    let _ = import_character_sync_payload(app, merged_payload, true);
}

/// Push local characters to all paired peers (fire-and-forget).
pub async fn sync_to_all_peers(app: &AppHandle) {
    let cfg = store::bootstrap(app);
    if cfg.paired_devices.is_empty() {
        return;
    }

    let payload = export_character_sync_payload(app);
    for peer in &cfg.paired_devices {
        let _ = push_character_sync_to_peer(app, peer, &payload, true).await;
    }
}
