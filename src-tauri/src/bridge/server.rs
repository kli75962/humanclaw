use std::net::SocketAddr;
use std::sync::Arc;

use axum::{Json, Router, extract::{Query, State}, http::StatusCode, routing::{get, post}};
use tauri::AppHandle;

use crate::session::store;
use super::exec::exec_handler;
use super::types::{CharacterImportRequest, ChatImportRequest, PingQuery, PingResponse, RegisterRequest, ToolRequest, ToolResponse, UnpairRequest};

// ── Server state ─────────────────────────────────────────────────────────────

/// Minimal state passed into axum handlers — only the AppHandle for disk reads.
pub type BridgeState = Arc<AppHandle>;

// ── Handlers ─────────────────────────────────────────────────────────────────

/// GET /ping?key=<token_or_hash>
///
/// Two modes:
/// 1. One-time pairing token — generate a fresh permanent hash_key, save it locally,
///    return it in the response body. Token is immediately destroyed after use.
/// 2. Permanent hash_key — standard peer-liveness check; hash_key is NOT echoed back.
///
/// Returns 401 if neither matches.
async fn ping_handler(
    State(app): State<BridgeState>,
    Query(query): Query<PingQuery>,
) -> Result<Json<PingResponse>, StatusCode> {
    // ── Mode 1: one-time pairing token ────────────────────────────────────────
    if super::pairing_token::validate_and_consume(&query.key) {
        // Generate a brand-new permanent hash_key and persist it.
        let new_key = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple(),
        );
        store::set_hash_key(&app, &new_key).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let cfg = store::bootstrap(&app);
        return Ok(Json(PingResponse {
            device_id: cfg.device.device_id,
            label: cfg.device.label,
            hash_key: Some(new_key), // sent once, over HTTP, to the phone
        }));
    }

    // ── Mode 2: permanent hash_key (ongoing peer-liveness checks) ────────────
    let cfg = store::bootstrap(&app);
    if query.key != cfg.hash_key {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(Json(PingResponse {
        device_id: cfg.device.device_id,
        label: cfg.device.label,
        hash_key: None,
    }))
}

/// POST /register — a paired peer registers itself so this device knows its address.
/// Body must include the shared hash key for authentication.
async fn register_handler(
    State(app): State<BridgeState>,
    Json(body): Json<RegisterRequest>,
) -> StatusCode {
    let cfg = store::bootstrap(&app);
    if body.key != cfg.hash_key {
        return StatusCode::UNAUTHORIZED;
    }
    if body.device_id == cfg.device.device_id {
        return StatusCode::OK; // self-registration is a no-op
    }
    let label: String = body.label.chars().take(64).collect();
    let _ = store::upsert_peer(
        &app,
        crate::session::types::PairedDevice {
            device_id: body.device_id.clone(),
            address: body.address.clone(),
            label,
        },
    );

    let peer = crate::session::types::PairedDevice {
        device_id: body.device_id,
        address: body.address,
        label: body.label,
    };
    let app_for_sync = (*app).clone();
    let peer_for_char = peer.clone();
    tauri::async_runtime::spawn(async move {
        super::chat_sync::sync_after_pair(&app_for_sync, &peer).await;
        super::character_sync::sync_after_pair(&app_for_sync, &peer_for_char).await;
    });

    // A device just came online — emit updated status to the frontend immediately.
    let app_clone = (*app).clone();
    tauri::async_runtime::spawn(async move {
        let statuses = crate::bridge::health::check_all_peers(&app_clone).await;
        use tauri::Emitter;
        app_clone.emit("peer-status-changed", statuses).ok();
    });
    StatusCode::OK
}

/// GET /chat/export?key=<hash_key>
/// Returns full chat snapshot for synchronization.
async fn chat_export_handler(
    State(app): State<BridgeState>,
    Query(query): Query<PingQuery>,
) -> Result<Json<crate::memory::ChatSyncPayload>, StatusCode> {
    let cfg = store::bootstrap(&app);
    if query.key != cfg.hash_key {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(Json(crate::memory::export_chat_sync_payload(&app)))
}

/// POST /chat/import
/// Applies incoming chat snapshot with merge/replace mode.
async fn chat_import_handler(
    State(app): State<BridgeState>,
    Json(body): Json<ChatImportRequest>,
) -> StatusCode {
    let cfg = store::bootstrap(&app);
    if body.key != cfg.hash_key {
        return StatusCode::UNAUTHORIZED;
    }

    match crate::memory::import_chat_sync_payload(&app, body.payload, body.replace) {
        Ok(()) => {
            use tauri::Emitter;
            let _ = app.emit("chat-sync-updated", serde_json::json!({}));
            StatusCode::OK
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// POST /unpair — a peer requests us to remove it from our paired devices list.
async fn unpair_handler(
    State(app): State<BridgeState>,
    Json(body): Json<UnpairRequest>,
) -> StatusCode {
    let cfg = store::bootstrap(&app);
    if body.key != cfg.hash_key {
        return StatusCode::UNAUTHORIZED;
    }
    let _ = store::remove_peer(&app, &body.device_id);
    use tauri::Emitter;
    app.emit("session-changed", serde_json::json!({})).ok();
    StatusCode::OK
}

/// GET /characters/export?key=<hash_key>
async fn character_export_handler(
    State(app): State<BridgeState>,
    Query(query): Query<PingQuery>,
) -> Result<Json<crate::characters::CharacterSyncPayload>, StatusCode> {
    let cfg = store::bootstrap(&app);
    if query.key != cfg.hash_key {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(Json(crate::characters::export_character_sync_payload(&app)))
}

/// POST /characters/import
async fn character_import_handler(
    State(app): State<BridgeState>,
    Json(body): Json<CharacterImportRequest>,
) -> StatusCode {
    let cfg = store::bootstrap(&app);
    if body.key != cfg.hash_key {
        return StatusCode::UNAUTHORIZED;
    }
    match crate::characters::import_character_sync_payload(&app, body.payload, body.replace) {
        Ok(()) => {
            use tauri::Emitter;
            let _ = app.emit("character-sync-updated", serde_json::json!({}));
            StatusCode::OK
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// POST /tool — execute a single tool call on this device and return the result.
/// Authenticated via the shared hash key.
async fn tool_handler(
    State(app): State<BridgeState>,
    Json(body): Json<ToolRequest>,
) -> Result<Json<ToolResponse>, StatusCode> {
    let cfg = store::bootstrap(&app);
    if body.key != cfg.hash_key {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let context = crate::tools::ToolExecutionContext {
        source_device_id: body.source_device_id.clone(),
        source_device_type: body.source_device_type.clone(),
    };
    let result = crate::tools::execute_tool_with_context(&app, &body.tool_name, &body.tool_args, &context).await;
    Ok(Json(ToolResponse {
        success: result.success,
        output: result.output,
    }))
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Start the bridge HTTP server in the background.
/// Listens on `0.0.0.0:{port}` from the saved session config.
/// Non-blocking — spawns onto the Tauri async runtime.
pub fn start_bridge_server(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let port = {
            let cfg = store::bootstrap(&app);
            cfg.bridge_port
        };

        let state: BridgeState = Arc::new(app);

        let router = Router::new()
            .route("/ping", get(ping_handler))
            .route("/register", post(register_handler))
            .route("/unpair", post(unpair_handler))
            .route("/tool", post(tool_handler))
            .route("/chat/export", get(chat_export_handler))
            .route("/chat/import", post(chat_import_handler))
            .route("/characters/export", get(character_export_handler))
            .route("/characters/import", post(character_import_handler))
            .route("/exec", post(exec_handler))
            .with_state(state);

        // Try the configured port first, then fall back to the next few ports.
        let listener = {
            let mut found = None;
            for try_port in port..=port + 10 {
                let try_addr = SocketAddr::from(([0, 0, 0, 0], try_port));
                match tokio::net::TcpListener::bind(try_addr).await {
                    Ok(l) => {
                        if try_port != port {
                            eprintln!("[bridge] port {port} busy, using {try_port} instead");
                        }
                        found = Some(l);
                        break;
                    }
                    Err(_) if try_port < port + 10 => continue,
                    Err(e) => {
                        eprintln!("[bridge] failed to bind any port {port}–{}: {e}", port + 10);
                        return;
                    }
                }
            }
            found.unwrap()
        };

        eprintln!("[bridge] listening on {}", listener.local_addr().unwrap());

        if let Err(e) = axum::serve(listener, router).await {
            eprintln!("[bridge] server error: {e}");
        }
    });
}
