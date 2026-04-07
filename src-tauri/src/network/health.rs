use std::collections::HashMap;
use std::sync::OnceLock;
use reqwest::Client;
use tauri::{AppHandle, Emitter};

use crate::session::store;
use super::types::PeerStatus;

/// Reuse the same HTTP client as the rest of the app (connection pool).
static HEALTH_CLIENT: OnceLock<Client> = OnceLock::new();

fn client() -> &'static Client {
    HEALTH_CLIENT.get_or_init(|| {
        Client::builder()
            // Short timeout — if a peer doesn't respond in 3 s it's considered offline.
            .timeout(std::time::Duration::from_secs(3))
            .build()
            .expect("failed to build health-check HTTP client")
    })
}

/// Ping a single peer address (e.g. `"192.168.1.5:9876"`) and check if it is
/// online AND shares our hash key.
/// Sends the key as a query param; the server returns 401 if it doesn't match.
pub async fn check_peer(address: &str, hash_key: &str) -> bool {
    let url = format!("http://{address}/ping");
    let Ok(resp) = client()
        .get(&url)
        .query(&[("key", hash_key)])
        .send()
        .await
    else {
        return false;
    };
    resp.status().is_success()
}

/// Check all paired peers in the session and return their online status.
pub async fn check_all_peers(app: &AppHandle) -> Vec<PeerStatus> {
    let cfg = store::bootstrap(app);
    let hash_key = cfg.hash_key.clone();

    // Spawn all peer checks concurrently to minimise latency.
    let mut tasks = Vec::with_capacity(cfg.paired_devices.len());
    for peer in cfg.paired_devices {
        let hk = hash_key.clone();
        tasks.push(tokio::spawn(async move {
            let online = check_peer(&peer.address, &hk).await;
            PeerStatus {
                device_id: peer.device_id,
                label: peer.label,
                address: peer.address,
                online,
            }
        }));
    }

    let mut results = Vec::with_capacity(tasks.len());
    for task in tasks {
        if let Ok(status) = task.await {
            results.push(status);
        }
    }
    results
}

/// Background task: polls all peers every 3 seconds and emits a
/// `peer-status-changed` Tauri event only when any device's status changes.
pub fn start_peer_monitor(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut prev: HashMap<String, bool> = HashMap::new();
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            let statuses = check_all_peers(&app).await;
            let changed = statuses.iter().any(|s| {
                prev.get(&s.device_id).copied() != Some(s.online)
            });
            if changed {
                for s in &statuses {
                    prev.insert(s.device_id.clone(), s.online);
                }
                app.emit("peer-status-changed", &statuses).ok();
            }
        }
    });
}
