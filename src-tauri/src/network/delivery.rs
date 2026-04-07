use std::sync::OnceLock;
use reqwest::Client;
use tauri::AppHandle;

use crate::social::queue::store::{load_pending, purge_delivered, update_entries_batch};
use crate::social::queue::types::{QueueEntry, QueueStatus};

static DELIVERY_CLIENT: OnceLock<Client> = OnceLock::new();

fn client() -> &'static Client {
    DELIVERY_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("failed to build delivery HTTP client")
    })
}

/// Try to POST a single queue entry to the peer's `/exec` endpoint.
/// Returns `true` on HTTP 2xx, `false` otherwise.
async fn try_deliver(entry: &QueueEntry) -> bool {
    let url = format!("http://{}/exec", entry.target_address);
    client()
        .post(&url)
        .json(&entry.payload)
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Attempt to deliver all pending queue entries for a specific peer device.
/// Call this after detecting a peer is back online.
#[allow(dead_code)]
pub async fn flush_pending_for_peer(app: &AppHandle, target_device_id: &str) {
    let pending: Vec<QueueEntry> = load_pending(app)
        .into_iter()
        .filter(|e| e.target_device_id == target_device_id)
        .collect();

    let mut updated = Vec::with_capacity(pending.len());
    for mut entry in pending {
        entry.attempts += 1;
        let delivered = try_deliver(&entry).await;
        entry.status = if delivered {
            QueueStatus::Delivered
        } else if entry.attempts >= QueueEntry::MAX_ATTEMPTS {
            QueueStatus::Failed
        } else {
            entry.status
        };
        updated.push(entry);
    }

    let _ = update_entries_batch(app, &updated);
    let _ = purge_delivered(app);
}

/// Attempt to deliver ALL pending entries (called on startup or periodic check).
pub async fn flush_all_pending(app: &AppHandle) {
    let pending = load_pending(app);

    let mut updated = Vec::with_capacity(pending.len());
    for mut entry in pending {
        entry.attempts += 1;
        let delivered = try_deliver(&entry).await;
        entry.status = if delivered {
            QueueStatus::Delivered
        } else if entry.attempts >= QueueEntry::MAX_ATTEMPTS {
            QueueStatus::Failed
        } else {
            entry.status
        };
        updated.push(entry);
    }

    let _ = update_entries_batch(app, &updated);
    let _ = purge_delivered(app);
}
