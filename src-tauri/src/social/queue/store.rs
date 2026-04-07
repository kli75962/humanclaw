use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::AppHandle;
use uuid::Uuid;

use crate::chat::memory_dir;
use super::types::{QueueEntry, QueueStatus};

const QUEUE_FILE: &str = "queue.jsonl";

// ── Path ─────────────────────────────────────────────────────────────────────

pub fn queue_path(app: &AppHandle) -> PathBuf {
    memory_dir(app).join(QUEUE_FILE)
}

// ── Read ─────────────────────────────────────────────────────────────────────

/// Load all queue entries from disk.  Skips malformed lines silently.
pub fn load_all(app: &AppHandle) -> Vec<QueueEntry> {
    let path = queue_path(app);
    let Ok(file) = std::fs::File::open(&path) else {
        return Vec::new();
    };
    std::io::BufReader::new(file)
        .lines()
        .filter_map(|l| l.ok())
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<QueueEntry>(&l).ok())
        .collect()
}

/// Load only entries with `Pending` status.
pub fn load_pending(app: &AppHandle) -> Vec<QueueEntry> {
    load_all(app)
        .into_iter()
        .filter(|e| e.status == QueueStatus::Pending)
        .collect()
}

// ── Write ─────────────────────────────────────────────────────────────────────

/// Rewrite the queue file with the given entries.
/// Call this after mutating any entries (atomic write via tmp file).
fn save_all(app: &AppHandle, entries: &[QueueEntry]) -> Result<(), String> {
    let path = queue_path(app);
    // Ensure the parent directory exists.
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let tmp = path.with_extension("jsonl.tmp");
    let file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    let mut writer = std::io::BufWriter::new(file);
    for entry in entries {
        let line = serde_json::to_string(entry).map_err(|e| e.to_string())?;
        writeln!(writer, "{line}").map_err(|e| e.to_string())?;
    }
    drop(writer); // flush before rename
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

// ── Mutations ────────────────────────────────────────────────────────────────

/// Append a new pending entry to the queue.
pub fn enqueue(
    app: &AppHandle,
    target_device_id: String,
    target_address: String,
    payload: serde_json::Value,
) -> Result<QueueEntry, String> {
    let entry = QueueEntry {
        id: Uuid::new_v4().to_string(),
        target_device_id,
        target_address,
        payload,
        created_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        status: QueueStatus::Pending,
        attempts: 0,
    };

    // Append a single line — fast, no full rewrite needed.
    let path = queue_path(app);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let line = serde_json::to_string(&entry).map_err(|e| e.to_string())?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| e.to_string())?;
    writeln!(file, "{line}").map_err(|e| e.to_string())?;

    Ok(entry)
}

/// Replace multiple entries by ID in one read + write pass.
/// More efficient than calling `update_entry` in a loop.
pub fn update_entries_batch(app: &AppHandle, updated: &[QueueEntry]) -> Result<(), String> {
    let mut all = load_all(app);
    for entry in &mut all {
        if let Some(u) = updated.iter().find(|u| u.id == entry.id) {
            *entry = u.clone();
        }
    }
    save_all(app, &all)
}

/// Remove entries that are `Delivered` to keep the file small.
pub fn purge_delivered(app: &AppHandle) -> Result<(), String> {
    let kept: Vec<QueueEntry> = load_all(app)
        .into_iter()
        .filter(|e| e.status != QueueStatus::Delivered)
        .collect();
    save_all(app, &kept)
}
