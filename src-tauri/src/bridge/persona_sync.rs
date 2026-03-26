use tauri::AppHandle;

use crate::skills::{
    export_persona_sync_payload, import_persona_sync_payload, PersonaSyncPayload,
};
use crate::session::store;
use crate::session::types::PairedDevice;

pub async fn push_persona_sync_to_peer(
    app: &AppHandle,
    peer: &PairedDevice,
    payload: &PersonaSyncPayload,
    replace: bool,
) -> Result<(), String> {
    let key = store::bootstrap(app).hash_key;
    let url = format!("http://{}/personas/import", peer.address);
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
        return Err(format!("persona sync failed: {}", resp.status()));
    }
    Ok(())
}

async fn fetch_peer_persona_sync(
    app: &AppHandle,
    peer: &PairedDevice,
) -> Result<PersonaSyncPayload, String> {
    let key = store::bootstrap(app).hash_key;
    let url = format!("http://{}/personas/export", peer.address);

    let resp = super::bridge_client()
        .get(url)
        .query(&[("key", key)])
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("persona export failed: {}", resp.status()));
    }

    resp.json::<PersonaSyncPayload>()
        .await
        .map_err(|e| e.to_string())
}

/// Bidirectional sync after a new pairing — merge local and remote, push merged to both.
pub async fn sync_after_pair(app: &AppHandle, peer: &PairedDevice) {
    let local = export_persona_sync_payload(app);
    let remote = fetch_peer_persona_sync(app, peer)
        .await
        .unwrap_or(PersonaSyncPayload { personas: vec![] });

    // Merge: union by name, remote wins on conflict (remote is newer post-pair).
    let mut merged = local.personas;
    for incoming in remote.personas {
        if !merged.iter().any(|p| p.name == incoming.name) {
            merged.push(incoming);
        }
    }
    let merged_payload = PersonaSyncPayload { personas: merged };

    let _ = push_persona_sync_to_peer(app, peer, &merged_payload, true).await;
    let _ = import_persona_sync_payload(app, merged_payload, true);
}

/// Push local personas to all paired peers (fire-and-forget).
pub async fn sync_to_all_peers(app: &AppHandle) {
    let cfg = store::bootstrap(app);
    if cfg.paired_devices.is_empty() {
        return;
    }

    let payload = export_persona_sync_payload(app);
    for peer in &cfg.paired_devices {
        let _ = push_persona_sync_to_peer(app, peer, &payload, true).await;
    }
}
