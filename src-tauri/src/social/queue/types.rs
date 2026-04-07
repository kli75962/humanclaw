use serde::{Deserialize, Serialize};

/// Delivery state of a queued cross-device command.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum QueueStatus {
    /// Not yet delivered — peer was offline when enqueued.
    Pending,
    /// Successfully delivered to the target device.
    Delivered,
    /// Max retry attempts exceeded — will not retry automatically.
    Failed,
}

/// A cross-device command that could not be delivered immediately.
/// Persisted to disk as JSONL so it survives app restarts.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QueueEntry {
    /// UUID — unique ID for this entry.
    pub id: String,
    /// Device ID of the peer that should execute this command.
    pub target_device_id: String,
    /// HTTP address of the peer at enqueue time, e.g. "192.168.1.5:9876".
    pub target_address: String,
    /// The raw JSON payload to POST to the peer's `/exec` endpoint.
    pub payload: serde_json::Value,
    /// Unix timestamp (seconds) when the entry was created.
    pub created_at: u64,
    pub status: QueueStatus,
    /// Number of delivery attempts so far.
    pub attempts: u32,
}

impl QueueEntry {
    pub const MAX_ATTEMPTS: u32 = 10;
}
