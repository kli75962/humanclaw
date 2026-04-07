use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::{Arc, OnceLock};
use tauri::AppHandle;
use axum::{Json, extract::State};
use reqwest::Client;

use crate::ai::ollama::{headless::run_headless, types::OllamaMessage};
use crate::social::queue::store::enqueue;
use crate::session::store as session_store;
use crate::network::health::check_peer;

// ── Shared HTTP client ────────────────────────────────────────────────────────

/// Reuse one client across all exec forwards — 120 s timeout for LLM responses.
static EXEC_CLIENT: OnceLock<Client> = OnceLock::new();

fn exec_client() -> &'static Client {
    EXEC_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .expect("failed to build exec HTTP client")
    })
}

// ── Wire types ────────────────────────────────────────────────────────────────

/// Inbound payload for `POST /exec` — sent by peers.
#[derive(Serialize, Deserialize, Clone)]
pub struct ExecRequest {
    /// Must match this device's hash key, otherwise the request is rejected.
    pub hash_key: String,
    /// Brief label of who sent this (peer device ID).
    pub source: String,
    /// Optional source device type string ("pc" | "phone").
    #[serde(default)]
    pub source_device_type: Option<String>,
    /// User message to run through the LLM.
    pub message: String,
    /// Ollama model to use.
    pub model: String,
    /// Prior conversation messages to inject as context (optional).
    #[serde(default)]
    pub history: Vec<OllamaMessage>,
}

/// Result returned from `POST /exec`.
#[derive(Serialize, Deserialize)]
pub struct ExecResponse {
    pub success: bool,
    pub response: String,
    /// `true` when the command was queued because the target peer was offline.
    pub queued: bool,
}

// ── Axum handler ─────────────────────────────────────────────────────────────

/// `POST /exec` — execute an LLM + tool-call task on this device.
///
/// Rejects requests whose `hash_key` doesn't match the local session.
pub async fn exec_handler(
    State(app): State<Arc<AppHandle>>,
    Json(req): Json<ExecRequest>,
) -> Json<ExecResponse> {
    let cfg = session_store::bootstrap(&app);

    // Security: only accept requests from devices sharing our hash key.
    if req.hash_key != cfg.hash_key {
        return Json(ExecResponse {
            success: false,
            response: "Rejected: hash key mismatch.".to_string(),
            queued: false,
        });
    }

    // Build conversation and run headless.
    let mut conversation = req.history;
    conversation.push(OllamaMessage {
        role: "user".to_string(),
        content: req.message,
        tool_calls: None,
        images: None,
        brief: None,
    });

    match run_headless(
        &app,
        conversation,
        &req.model,
        Some(req.source.clone()),
        req.source_device_type.clone(),
    )
    .await
    {
        Ok(text) => Json(ExecResponse {
            success: true,
            response: text,
            queued: false,
        }),
        Err(e) => Json(ExecResponse {
            success: false,
            response: format!("Agent error: {e}"),
            queued: false,
        }),
    }
}

// ── Routing helper (used by Tauri command or peer) ─────────────────────

/// Route an LLM command to the correct device.
///
/// - If `target_device_id` is this device → run headless locally.
/// - If `target_device_id` matches a paired peer AND peer is online → POST to their /exec.
/// - If peer is offline → queue the command.
pub async fn route_command(
    app: &AppHandle,
    target_device_id: &str,
    message: &str,
    model: &str,
    history: Vec<OllamaMessage>,
) -> ExecResponse {
    let cfg = session_store::bootstrap(app);

    // ── Target is this device ───────────────────────────────────────────────
    if target_device_id == cfg.device.device_id {
        let mut conversation = history;
        conversation.push(OllamaMessage {
            role: "user".to_string(),
            content: message.to_string(),
            tool_calls: None,
            images: None,
            brief: None,
        });
        let source_device_type = Some(cfg.device.device_type.label().to_string());
        return match run_headless(
            app,
            conversation,
            model,
            Some(cfg.device.device_id.clone()),
            source_device_type,
        )
        .await
        {
            Ok(text) => ExecResponse { success: true, response: text, queued: false },
            Err(e) => ExecResponse { success: false, response: e, queued: false },
        };
    }

    // ── Target is a paired peer ──────────────────────────────────────────────
    let Some(peer) = cfg.paired_devices.iter().find(|p| p.device_id == target_device_id) else {
        return ExecResponse {
            success: false,
            response: format!("Unknown device: {target_device_id}"),
            queued: false,
        };
    };

    let peer_address = peer.address.clone();
    let peer_device_id = peer.device_id.clone();

    // Check if peer is online.
    if check_peer(&peer_address, &cfg.hash_key).await {
        // Forward request directly.
        let payload = ExecRequest {
            hash_key: cfg.hash_key.clone(),
            source: cfg.device.device_id.clone(),
            source_device_type: Some(cfg.device.device_type.label().to_string()),
            message: message.to_string(),
            model: model.to_string(),
            history,
        };
        let url = format!("http://{peer_address}/exec");
        match exec_client().post(&url).json(&payload).send().await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<ExecResponse>().await {
                    Ok(r) => r,
                    Err(_) => ExecResponse {
                        success: false,
                        response: "Invalid response from peer".to_string(),
                        queued: false,
                    },
                }
            }
            _ => {
                // Forward failed — queue for later.
                queue_for_peer(app, &peer_device_id, &peer_address, message, model, &cfg.hash_key)
            }
        }
    } else {
        // Peer offline — queue the command.
        queue_for_peer(app, &peer_device_id, &peer_address, message, model, &cfg.hash_key)
    }
}

fn queue_for_peer(
    app: &AppHandle,
    device_id: &str,
    address: &str,
    message: &str,
    model: &str,
    hash_key: &str,
) -> ExecResponse {
    let payload = json!({
        "hash_key": hash_key,
        "source": "queued",
        "source_device_type": serde_json::Value::Null,
        "message": message,
        "model": model,
        "history": [],
    });
    let queued = enqueue(app, device_id.to_string(), address.to_string(), payload).is_ok();
    ExecResponse {
        success: queued,
        response: if queued {
            "Device offline — command queued and will be delivered when it reconnects.".to_string()
        } else {
            "Device offline and failed to queue command.".to_string()
        },
        queued,
    }
}
