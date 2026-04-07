use tauri::AppHandle;

use crate::ai::ollama::types::OllamaMessage;
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

/// Return this device's primary LAN address as "ip:port".
#[tauri::command]
pub fn get_local_address(app: AppHandle) -> Result<String, String> {
    let ip = local_ip_address::local_ip().map_err(|e| format!("Cannot detect LAN IP: {e}"))?;
    let port = store::bootstrap(&app).bridge_port;
    Ok(format!("{ip}:{port}"))
}

/// Return ALL non-loopback IPv4 addresses as "ip:port" candidates.
/// Used by QR pairing so the phone can try each one.
#[tauri::command]
pub fn get_all_local_addresses(app: AppHandle) -> Vec<String> {
    let port = store::bootstrap(&app).bridge_port;
    let mut addrs = Vec::new();
    if let Ok(ifaces) = local_ip_address::list_afinet_netifas() {
        for (_name, ip) in ifaces {
            if let std::net::IpAddr::V4(v4) = ip {
                if !v4.is_loopback() {
                    addrs.push(format!("{v4}:{port}"));
                }
            }
        }
    }
    // Fallback: if no addresses found, try the single-IP detection
    if addrs.is_empty() {
        if let Ok(ip) = local_ip_address::local_ip() {
            addrs.push(format!("{ip}:{port}"));
        }
    }
    addrs
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

/// Try pinging each address concurrently. Returns the first (address, PingResponse) that succeeds.
async fn probe_addresses(
    addresses: &[String],
    hash_key: &str,
) -> Result<(String, PingResponse), String> {
    use tokio::sync::mpsc;

    let (tx, mut rx) = mpsc::channel::<(String, PingResponse)>(1);

    for addr in addresses {
        let addr = addr.clone();
        let key = hash_key.to_string();
        let tx = tx.clone();
        tokio::spawn(async move {
            let url = format!("http://{addr}/ping");
            let Ok(http_resp) = crate::network::bridge_client().get(&url).query(&[("key", &key)]).send().await else {
                return;
            };
            if !http_resp.status().is_success() {
                return;
            }
            if let Ok(resp) = http_resp.json::<PingResponse>().await {
                let _ = tx.send((addr, resp)).await;
            }
        });
    }
    // Drop our sender so rx closes when all tasks finish.
    drop(tx);

    match rx.recv().await {
        Some(result) => Ok(result),
        None => Err(format!(
            "Could not reach any address: {}",
            addresses.join(", ")
        )),
    }
}

/// Ping a remote peer, verify hash keys match, and save it as a paired device.
#[tauri::command]
pub async fn discover_and_pair(app: AppHandle, address: String) -> Result<(), String> {
    validate_address(&address)?;

    let cfg = store::bootstrap(&app);
    let url = format!("http://{address}/ping");

    let http_resp = crate::network::bridge_client()
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
            device_id: resp.device_id.clone(),
            address: address.clone(),
            label: resp.label.chars().take(64).collect(),
        },
    )?;

    let peer = crate::session::types::PairedDevice {
        device_id: resp.device_id,
        address,
        label: resp.label,
    };
    let app_for_sync = app.clone();
    tauri::async_runtime::spawn(async move {
        super::sync::chat::sync_after_pair(&app_for_sync, &peer).await;
    });

    Ok(())
}

/// Atomic QR pairing: verify the peer using the QR-supplied hash key, then
/// set the local hash key and save the peer — all in one step.
/// Accepts multiple candidate addresses and tries each concurrently —
/// uses the first one that responds successfully.
#[tauri::command]
pub async fn pair_from_qr(
    app: AppHandle,
    addresses: Vec<String>,
    hash_key: String,
) -> Result<(), String> {
    if addresses.is_empty() {
        return Err("No addresses to try.".to_string());
    }
    for addr in &addresses {
        validate_address(addr)?;
    }

    let hash_key = hash_key.trim().to_string();
    if hash_key.len() != 64 || !hash_key.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("Invalid hash key in QR code.".to_string());
    }

    // Try all candidate addresses concurrently, use the first successful one.
    let (address, resp) = probe_addresses(&addresses, &hash_key).await?;

    let cfg = store::bootstrap(&app);
    if resp.device_id == cfg.device.device_id {
        return Err("Cannot pair with yourself.".to_string());
    }

    // The PC returns the permanent hash_key in the response when a pairing token was used.
    // Fall back to the QR value only for legacy QR codes that embed the hash_key directly.
    let effective_key = resp.hash_key.clone().unwrap_or_else(|| hash_key.clone());

    store::set_hash_key(&app, &effective_key)?;
    store::upsert_peer(
        &app,
        crate::session::types::PairedDevice {
            device_id: resp.device_id.clone(),
            address: address.clone(),
            label: resp.label.chars().take(64).collect(),
        },
    )?;

    let peer = crate::session::types::PairedDevice {
        device_id: resp.device_id,
        address: address.clone(),
        label: resp.label,
    };
    let app_for_sync = app.clone();
    tauri::async_runtime::spawn(async move {
        super::sync::chat::sync_after_pair(&app_for_sync, &peer).await;
    });

    // Best-effort: tell the peer about us so it can save our address too.
    // Use effective_key (the newly established shared key) for authentication.
    let updated = store::bootstrap(&app);
    if let Ok(my_ip) = local_ip_address::local_ip() {
        let my_address = format!("{my_ip}:{}", updated.bridge_port);
        let register_url = format!("http://{address}/register");
        let body = serde_json::json!({
            "key": &effective_key,
            "device_id": &updated.device.device_id,
            "label": &updated.device.label,
            "address": my_address,
        });
        let client = crate::network::bridge_client().clone();
        tauri::async_runtime::spawn(async move {
            let _ = client.post(&register_url).json(&body).send().await;
        });
    }

    Ok(())
}

/// Generate an SVG QR code for device pairing.
/// The QR embeds a one-time pairing token — the permanent hash_key is NEVER placed in the QR.
/// Each call regenerates the token (invalidating any previous one).
/// Pass `custom_address` to use ONLY that address (e.g. public IP for cross-network).
#[tauri::command]
pub fn get_qr_pair_svg(app: AppHandle, custom_address: Option<String>) -> Result<String, String> {
    let addresses: Vec<String> = match custom_address.filter(|a| !a.trim().is_empty()) {
        Some(addr) => {
            validate_address(&addr)?;
            vec![addr]
        }
        None => {
            let addrs = get_all_local_addresses(app);
            if addrs.is_empty() {
                return Err("Cannot detect any network address.".to_string());
            }
            addrs
        }
    };
    // Generate a fresh one-time token — the hash_key never leaves this device via QR.
    let pairing_token = super::pairing_token::generate();
    let payload = serde_json::json!({
        "addresses": addresses,
        "pairing_token": pairing_token,
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
