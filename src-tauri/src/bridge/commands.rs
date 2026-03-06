use tauri::AppHandle;

use crate::ollama::types::OllamaMessage;
use crate::session::store;
use super::exec::{route_command, ExecResponse};
use super::health::{check_all_peers, check_peer};
use super::types::{PeerStatus, PingResponse};

/// Check if a single peer address is online and shares our hash key.
/// `address` should be "ip:port", e.g. "192.168.1.5:9876".
#[tauri::command]
pub async fn check_peer_online(app: AppHandle, address: String) -> bool {
    let hash_key = store::bootstrap(&app).hash_key;
    check_peer(&address, &hash_key).await
}

/// Check all paired peers from the session config and return their status.
#[tauri::command]
pub async fn get_all_peer_status(app: AppHandle) -> Vec<PeerStatus> {
    check_all_peers(&app).await
}

/// Route an LLM command to a specific device by its device ID.
///
/// - If `target_device_id` matches this device → run locally.
/// - If it matches a paired peer and the peer is online → forward via HTTP.
/// - If the peer is offline → queue the command and return `queued: true`.
#[tauri::command]
pub async fn send_to_device(
    app: AppHandle,
    target_device_id: String,
    message: String,
    model: String,
    history: Option<Vec<OllamaMessage>>,
) -> ExecResponse {
    let history = history.unwrap_or_default();
    route_command(&app, &target_device_id, &message, &model, history).await
}

/// Return this device's LAN address as "ip:port".
#[tauri::command]
pub fn get_local_address(app: AppHandle) -> Result<String, String> {
    let ip = local_ip_address::local_ip().map_err(|e| format!("Cannot detect LAN IP: {e}"))?;
    let port = store::bootstrap(&app).bridge_port;
    Ok(format!("{ip}:{port}"))
}

/// Validate that `address` is a syntactically safe "host:port" string and
/// return an error early before any outbound HTTP request is made.
fn validate_address(address: &str) -> Result<(), String> {
    let parts: Vec<&str> = address.splitn(2, ':').collect();
    if parts.len() != 2 || parts[1].parse::<u16>().is_err() {
        return Err("Invalid address — expected format: ip:port (e.g. 192.168.1.5:9876)".to_string());
    }
    // Disallow whitespace, slashes, or query characters that could corrupt the URL.
    if address.chars().any(|c| c.is_whitespace() || c == '/' || c == '?' || c == '#' || c == '@') {
        return Err("Address contains invalid characters.".to_string());
    }
    Ok(())
}

/// Ping a remote peer, verify hash keys match, and save it as a paired device.
#[tauri::command]
pub async fn discover_and_pair(app: AppHandle, address: String) -> Result<(), String> {
    validate_address(&address)?;

    let cfg = store::bootstrap(&app);
    let url = format!("http://{address}/ping");

    let http_resp = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?
        .get(&url)
        .query(&[("key", &cfg.hash_key)])
        .send()
        .await
        .map_err(|_| format!("Could not reach {address}"))?;

    if http_resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err("Hash key mismatch — both devices must share the same key.".to_string());
    }

    if !http_resp.status().is_success() {
        return Err(format!("Peer returned error: {}", http_resp.status()));
    }

    let resp = http_resp
        .json::<PingResponse>()
        .await
        .map_err(|_| "Invalid response from peer".to_string())?;

    if resp.device_id == cfg.device.device_id {
        return Err("Cannot pair with yourself.".to_string());
    }

    store::upsert_peer(
        &app,
        crate::session::types::PairedDevice {
            device_id: resp.device_id,
            address,
            label: resp.label.chars().take(64).collect(),
        },
    )?;
    Ok(())
}

/// Atomic QR pairing: verify the peer using the QR-supplied hash key, then
/// set the local hash key and save the peer — all in one step.
/// If the peer ping fails, the local hash key is left unchanged.
#[tauri::command]
pub async fn pair_from_qr(app: AppHandle, address: String, hash_key: String) -> Result<(), String> {
    validate_address(&address)?;

    let hash_key = hash_key.trim().to_string();
    if hash_key.len() != 64 || !hash_key.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("Invalid hash key in QR code.".to_string());
    }

    let url = format!("http://{address}/ping");
    let http_resp = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?
        .get(&url)
        .query(&[("key", &hash_key)])
        .send()
        .await
        .map_err(|_| format!("Could not reach {address}"))?;

    if http_resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err("Hash key mismatch — QR code does not match the target device.".to_string());
    }

    if !http_resp.status().is_success() {
        return Err(format!("Peer returned error: {}", http_resp.status()));
    }

    let resp = http_resp
        .json::<PingResponse>()
        .await
        .map_err(|_| "Invalid response from peer".to_string())?;

    let cfg = store::bootstrap(&app);
    if resp.device_id == cfg.device.device_id {
        return Err("Cannot pair with yourself.".to_string());
    }

    if resp.device_id == cfg.device.device_id {
        return Err("Cannot pair with yourself.".to_string());
    }

    // Only set the hash key and save the peer after a successful ping.
    store::set_hash_key(&app, &hash_key)?;
    store::upsert_peer(
        &app,
        crate::session::types::PairedDevice {
            device_id: resp.device_id.clone(),
            address: address.clone(),
            label: resp.label.chars().take(64).collect(),
        },
    )?;

    // Best-effort: tell the peer about us so it can save our address too.
    // Re-read config so we get the updated hash key and our current address.
    let updated = store::bootstrap(&app);
    if let Ok(my_ip) = local_ip_address::local_ip() {
        let my_address = format!("{my_ip}:{}", updated.bridge_port);
        let register_url = format!("http://{address}/register");
        let body = serde_json::json!({
            "key": &hash_key,
            "device_id": &updated.device.device_id,
            "label": &updated.device.label,
            "address": my_address,
        });
        let _ = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .ok()
            .map(|c| tauri::async_runtime::spawn(async move {
                let _ = c.post(&register_url).json(&body).send().await;
            }));
    }

    Ok(())
}

/// Generate an SVG QR code encoding this device's address + hash key for pairing.
#[tauri::command]
pub fn get_qr_pair_svg(app: AppHandle) -> Result<String, String> {
    let cfg = store::bootstrap(&app);
    let ip = local_ip_address::local_ip().map_err(|e| format!("Cannot detect LAN IP: {e}"))?;
    let payload = serde_json::json!({
        "address": format!("{ip}:{}", cfg.bridge_port),
        "hash_key": cfg.hash_key,
    });
    let data = payload.to_string();

    use qrcode::QrCode;
    let code = QrCode::new(data.as_bytes()).map_err(|e| format!("QR generation failed: {e}"))?;
    let svg = code
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(256, 256)
        .quiet_zone(true)
        .build();
    Ok(svg)
}
