use tauri::AppHandle;

use super::delivery::flush_all_pending;
use super::store::{enqueue, load_all, load_pending};
use super::types::QueueEntry;
use super::post_gen::{self as pg};

/// Return all queue entries (pending + delivered + failed) for the frontend.
#[tauri::command]
pub fn get_queue(app: AppHandle) -> Vec<QueueEntry> {
    load_all(&app)
}

/// Return only pending queue entries.
#[tauri::command]
pub fn get_pending_queue(app: AppHandle) -> Vec<QueueEntry> {
    load_pending(&app)
}

/// Manually enqueue a command for a peer.
/// Called by the bridge router when a peer is detected as offline.
#[tauri::command]
pub fn queue_command(
    app: AppHandle,
    target_device_id: String,
    target_address: String,
    payload: serde_json::Value,
) -> Result<QueueEntry, String> {
    enqueue(&app, target_device_id, target_address, payload)
}

/// Flush all pending queue entries (try to deliver now).
/// Useful when the user knows a peer just came back online.
#[tauri::command]
pub async fn flush_queue(app: AppHandle) {
    flush_all_pending(&app).await;
}

// ── Post Generation Queue ────────────────────────────────────────────────────

/// Get all post generation queue entries.
#[tauri::command]
pub fn get_post_gen_queue(app: AppHandle) -> Vec<pg::PostGenEntry> {
    pg::load_all(&app)
}

/// Get only pending post generation entries.
#[tauri::command]
pub fn get_post_gen_pending(app: AppHandle) -> Vec<pg::PostGenEntry> {
    pg::load_pending(&app)
}

/// Cleanup stale post generation entries (older than 7 days).
#[tauri::command]
pub fn cleanup_post_gen_stale(app: AppHandle) -> Result<u32, String> {
    pg::cleanup_stale(&app)
}
