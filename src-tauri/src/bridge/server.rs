use std::net::SocketAddr;
use std::sync::Arc;

use axum::{Json, Router, extract::{Query, State}, http::StatusCode, routing::{get, post}};
use tauri::AppHandle;

use crate::session::store;
use super::exec::exec_handler;
use super::types::{PingQuery, PingResponse, RegisterRequest, ToolRequest, ToolResponse};

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
            device_id: body.device_id,
            address: body.address,
            label,
        },
    );
    // A device just came online — emit updated status to the frontend immediately.
    let app_clone = (*app).clone();
    tauri::async_runtime::spawn(async move {
        let statuses = crate::bridge::health::check_all_peers(&app_clone).await;
        use tauri::Emitter;
        app_clone.emit("peer-status-changed", statuses).ok();
    });
    StatusCode::OK
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
    let result = crate::phone::execute_tool(&app, &body.tool_name, &body.tool_args).await;
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
            .route("/tool", post(tool_handler))
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
