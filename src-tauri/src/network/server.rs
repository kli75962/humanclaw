use std::net::SocketAddr;
use std::sync::Arc;

use axum::{Json, Router, body::Body, extract::{ConnectInfo, Query, State}, http::{Method, StatusCode}, response::IntoResponse, routing::{any, get, post}};
use tauri::AppHandle;

use crate::session::store;
use super::exec::exec_handler;
use super::types::{CharacterImportRequest, ChatImportRequest, PersonaImportRequest, PingQuery, PingResponse, RegisterRequest, ToolRequest, ToolResponse, UnpairRequest};

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
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    Json(body): Json<RegisterRequest>,
) -> StatusCode {
    let cfg = store::bootstrap(&app);
    if body.key != cfg.hash_key {
        return StatusCode::UNAUTHORIZED;
    }
    if body.device_id == cfg.device.device_id {
        return StatusCode::OK; // self-registration is a no-op
    }
    // Use the actual connection IP (not the self-reported one) combined with
    // the peer's bridge port so Android mobile-data IPs don't cause misdirection.
    let actual_address = body.address
        .rsplit_once(':')
        .map(|(_, port)| format!("{}:{}", peer_addr.ip(), port))
        .unwrap_or(body.address.clone());
    let label: String = body.label.chars().take(64).collect();
    let _ = store::upsert_peer(
        &app,
        crate::session::types::PairedDevice {
            device_id: body.device_id.clone(),
            address: actual_address.clone(),
            label,
        },
    );

    let peer = crate::session::types::PairedDevice {
        device_id: body.device_id,
        address: actual_address,
        label: body.label,
    };
    let app_for_sync = (*app).clone();
    let peer_for_char = peer.clone();
    let peer_for_persona = peer.clone();
    tauri::async_runtime::spawn(async move {
        super::sync::chat::sync_after_pair(&app_for_sync, &peer).await;
        super::sync::character::sync_after_pair(&app_for_sync, &peer_for_char).await;
        super::sync::skills::sync_after_pair(&app_for_sync, &peer_for_persona).await;
    });

    // Notify frontend immediately: new device added + it's now online.
    use tauri::Emitter;
    app.emit("session-changed", serde_json::json!({})).ok();
    let app_clone = (*app).clone();
    tauri::async_runtime::spawn(async move {
        let statuses = crate::network::health::check_all_peers(&app_clone).await;
        app_clone.emit("peer-status-changed", statuses).ok();
    });
    StatusCode::OK
}

/// GET /chat/export?key=<hash_key>
/// Returns full chat snapshot for synchronization.
async fn chat_export_handler(
    State(app): State<BridgeState>,
    Query(query): Query<PingQuery>,
) -> Result<Json<crate::chat::ChatSyncPayload>, StatusCode> {
    let cfg = store::bootstrap(&app);
    if query.key != cfg.hash_key {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(Json(crate::chat::export_chat_sync_payload(&app)))
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

    match crate::chat::import_chat_sync_payload(&app, body.payload, body.replace) {
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
) -> Result<Json<crate::social::character::CharacterSyncPayload>, StatusCode> {
    let cfg = store::bootstrap(&app);
    if query.key != cfg.hash_key {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(Json(crate::social::character::export_character_sync_payload(&app)))
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
    match crate::social::character::import_character_sync_payload(&app, body.payload, body.replace) {
        Ok(()) => {
            use tauri::Emitter;
            let _ = app.emit("character-sync-updated", serde_json::json!({}));
            StatusCode::OK
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// GET /personas/export?key=<hash_key>
async fn persona_export_handler(
    State(app): State<BridgeState>,
    Query(query): Query<PingQuery>,
) -> Result<Json<crate::skills::PersonaSyncPayload>, StatusCode> {
    let cfg = store::bootstrap(&app);
    if query.key != cfg.hash_key {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(Json(crate::skills::export_persona_sync_payload(&app)))
}

/// POST /personas/import
async fn persona_import_handler(
    State(app): State<BridgeState>,
    Json(body): Json<PersonaImportRequest>,
) -> StatusCode {
    let cfg = store::bootstrap(&app);
    if body.key != cfg.hash_key {
        return StatusCode::UNAUTHORIZED;
    }
    match crate::skills::import_persona_sync_payload(&app, body.payload, body.replace) {
        Ok(()) => StatusCode::OK,
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

// ── Ollama proxy ─────────────────────────────────────────────────────────────

/// Proxy any request to the local Ollama instance, forwarding method/headers/body.
/// Allows paired Android phones to reach PC's Ollama through the already-open bridge port.
async fn ollama_proxy_handler(
    State(app): State<BridgeState>,
    axum::extract::Path(path): axum::extract::Path<String>,
    method: Method,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    let cfg = store::bootstrap(&app);
    let port = if cfg.ollama_port == 0 { 11434 } else { cfg.ollama_port };
    let url = format!("http://127.0.0.1:{port}/{path}");

    let client = crate::network::bridge_client();
    let mut req = client.request(
        reqwest::Method::from_bytes(method.as_str().as_bytes()).unwrap_or(reqwest::Method::GET),
        &url,
    );
    for (name, value) in &headers {
        let n = name.as_str();
        if n == "host" || n == "content-length" { continue; }
        if let Ok(v) = value.to_str() {
            req = req.header(n, v);
        }
    }
    req = req.body(body.to_vec());

    match req.send().await {
        Ok(resp) => {
            let status = axum::http::StatusCode::from_u16(resp.status().as_u16())
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            let mut builder = axum::http::Response::builder().status(status);
            for (k, v) in resp.headers() {
                builder = builder.header(k, v);
            }
            let bytes = resp.bytes().await.unwrap_or_default();
            builder.body(Body::from(bytes)).unwrap_or_else(|_| {
                axum::http::Response::builder()
                    .status(500)
                    .body(Body::empty())
                    .unwrap()
            })
        }
        Err(e) => axum::http::Response::builder()
            .status(502)
            .body(Body::from(format!("Ollama proxy error: {e}")))
            .unwrap(),
    }
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
            .route("/personas/export", get(persona_export_handler))
            .route("/personas/import", post(persona_import_handler))
            .route("/exec", post(exec_handler))
            .route("/proxy/ollama/*path", any(ollama_proxy_handler))
            .with_state(state);

        // Try to bind to all interfaces. On systems with virtual interfaces (Tailscale),
        // we need to ensure proper binding. Try IPv4 first (0.0.0.0), then IPv6 (::).
        let listener = {
            let mut found = None;

            // Try IPv4 0.0.0.0 first
            for try_port in port..=port + 10 {
                let try_addr = SocketAddr::from(([0, 0, 0, 0], try_port));
                match tokio::net::TcpListener::bind(try_addr).await {
                    Ok(l) => {
                        if try_port != port {
                            eprintln!("[bridge] port {port} busy, using {try_port} instead");
                        }
                        eprintln!("[bridge] listening on 0.0.0.0:{try_port}");
                        found = Some(l);
                        break;
                    }
                    Err(_) if try_port < port + 10 => continue,
                    Err(_) => break,
                }
            }

            // If IPv4 failed, try IPv6 ::
            if found.is_none() {
                for try_port in port..=port + 10 {
                    let try_addr = SocketAddr::from((std::net::Ipv6Addr::UNSPECIFIED, try_port));
                    match tokio::net::TcpListener::bind(try_addr).await {
                        Ok(l) => {
                            if try_port != port {
                                eprintln!("[bridge] port {port} busy, using {try_port} instead");
                            }
                            eprintln!("[bridge] listening on [::]:{try_port}");
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
            }

            found.unwrap()
        };

        eprintln!("[bridge] listening on {}", listener.local_addr().unwrap());

        if let Err(e) = axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        ).await {
            eprintln!("[bridge] server error: {e}");
        }
    });
}
