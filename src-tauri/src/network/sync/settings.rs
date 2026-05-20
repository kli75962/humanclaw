use tauri::{AppHandle, Emitter};

use crate::session::store;
use crate::session::types::{PairedDevice, PcPermissions};
use super::super::types::{OllamaModelPayload, PcPermissionsPayload, PersonaPayload};

// ── ollama_model ────────────────────────────────────────────────────────────

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

// ── persona ─────────────────────────────────────────────────────────────────

async fn fetch_peer_persona(
    app: &AppHandle,
    peer: &PairedDevice,
) -> Result<Option<String>, String> {
    let key = store::bootstrap(app).hash_key;
    let url = format!("http://{}/settings/persona", peer.address);

    let resp = crate::network::bridge_client()
        .get(url)
        .query(&[("key", key)])
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("persona export failed: {}", resp.status()));
    }

    resp.json::<PersonaPayload>()
        .await
        .map(|p| if p.persona.is_empty() { None } else { Some(p.persona) })
        .map_err(|e| e.to_string())
}

async fn push_persona_to_peer(
    app: &AppHandle,
    peer: &PairedDevice,
    persona: &str,
) -> Result<(), String> {
    let key = store::bootstrap(app).hash_key;
    let url = format!("http://{}/settings/persona", peer.address);
    let body = serde_json::json!({ "key": key, "persona": persona });

    let resp = crate::network::bridge_client()
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("persona push failed: {}", resp.status()));
    }
    Ok(())
}

// ── pc_permissions ──────────────────────────────────────────────────────────

async fn fetch_peer_pc_permissions(
    app: &AppHandle,
    peer: &PairedDevice,
) -> Result<Option<PcPermissions>, String> {
    let key = store::bootstrap(app).hash_key;
    let url = format!("http://{}/settings/pc_permissions", peer.address);

    let resp = crate::network::bridge_client()
        .get(url)
        .query(&[("key", key)])
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("pc_permissions export failed: {}", resp.status()));
    }

    resp.json::<PcPermissionsPayload>()
        .await
        .map(|p| Some(p.permissions))
        .map_err(|e| e.to_string())
}

async fn push_pc_permissions_to_peer(
    app: &AppHandle,
    peer: &PairedDevice,
    permissions: &PcPermissions,
) -> Result<(), String> {
    let key = store::bootstrap(app).hash_key;
    let url = format!("http://{}/settings/pc_permissions", peer.address);
    let body = serde_json::json!({ "key": key, "permissions": permissions });

    let resp = crate::network::bridge_client()
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Err(format!("pc_permissions push failed: {}", resp.status()));
    }
    Ok(())
}

// ── public API ──────────────────────────────────────────────────────────────

/// After pairing: pull every synced setting from the peer; remote wins when both
/// sides have a value, otherwise local pushes its value. Both sides run this
/// symmetrically.
pub async fn sync_after_pair(app: &AppHandle, peer: &PairedDevice) {
    // ollama_model
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

    // persona — never None locally (always has a default), so just adopt remote if present.
    if let Some(remote_persona) = fetch_peer_persona(app, peer).await.ok().flatten() {
        if store::set_persona_quiet(app, &remote_persona).is_ok() {
            let _ = app.emit("session-changed", serde_json::json!({}));
        }
    } else {
        let local_persona = store::bootstrap(app).persona;
        let _ = push_persona_to_peer(app, peer, &local_persona).await;
    }

    // pc_permissions
    if let Some(remote_perms) = fetch_peer_pc_permissions(app, peer).await.ok().flatten() {
        if store::set_pc_permissions_quiet(app, remote_perms).is_ok() {
            let _ = app.emit("session-changed", serde_json::json!({}));
        }
    } else {
        let local_perms = store::bootstrap(app).pc_permissions;
        let _ = push_pc_permissions_to_peer(app, peer, &local_perms).await;
    }
}

/// Push the local model selection to all paired peers (fire-and-forget).
pub async fn push_ollama_model_to_all_peers(app: &AppHandle, model: &str) {
    let cfg = store::bootstrap(app);
    for peer in &cfg.paired_devices {
        let _ = push_ollama_model_to_peer(app, peer, model).await;
    }
}

/// Push the local persona selection to all paired peers (fire-and-forget).
pub async fn push_persona_to_all_peers(app: &AppHandle, persona: &str) {
    let cfg = store::bootstrap(app);
    for peer in &cfg.paired_devices {
        let _ = push_persona_to_peer(app, peer, persona).await;
    }
}

/// Push the local PC permissions to all paired peers (fire-and-forget).
pub async fn push_pc_permissions_to_all_peers(app: &AppHandle, permissions: &PcPermissions) {
    let cfg = store::bootstrap(app);
    for peer in &cfg.paired_devices {
        let _ = push_pc_permissions_to_peer(app, peer, permissions).await;
    }
}
