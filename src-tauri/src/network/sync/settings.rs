use tauri::{AppHandle, Emitter};

use crate::session::store;
use crate::session::types::PairedDevice;
use super::super::types::OllamaModelPayload;

async fn fetch_peer_ollama_model(
    app: &AppHandle,
    peer: &PairedDevice,
) -> Result<Option<String>, String> {
    let key = store::bootstrap(app).hash_key;
    let url = format!("http://{}/settings/ollama_model", peer.address);

    let resp = crate::network::bridge_client()
        .get(url)
        .query(&[("key", key)])
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("ollama_model export failed: {}", resp.status()));
    }

    resp.json::<OllamaModelPayload>()
        .await
        .map(|p| p.model)
        .map_err(|e| e.to_string())
}

async fn push_ollama_model_to_peer(
    app: &AppHandle,
    peer: &PairedDevice,
    model: &str,
) -> Result<(), String> {
    let key = store::bootstrap(app).hash_key;
    let url = format!("http://{}/settings/ollama_model", peer.address);
    let body = serde_json::json!({ "key": key, "model": model });

    let resp = crate::network::bridge_client()
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("ollama_model push failed: {}", resp.status()));
    }
    Ok(())
}

/// After pairing: pull peer's model; if peer has one and local differs (or local is empty),
/// adopt peer's value. If only local has one, push it to peer. Both sides run this
/// symmetrically — when both have a value, remote wins (consistent with other syncs).
pub async fn sync_after_pair(app: &AppHandle, peer: &PairedDevice) {
    let local = store::bootstrap(app).ollama_model;
    let remote = fetch_peer_ollama_model(app, peer).await.ok().flatten();

    match (local, remote) {
        (_, Some(m)) => {
            if store::set_ollama_model(app, &m).is_ok() {
                let _ = app.emit("session-changed", serde_json::json!({}));
            }
        }
        (Some(local_model), None) => {
            let _ = push_ollama_model_to_peer(app, peer, &local_model).await;
        }
        (None, None) => {}
    }
}

/// Push the local model selection to all paired peers (fire-and-forget).
pub async fn push_ollama_model_to_all_peers(app: &AppHandle, model: &str) {
    let cfg = store::bootstrap(app);
    for peer in &cfg.paired_devices {
        let _ = push_ollama_model_to_peer(app, peer, model).await;
    }
}
