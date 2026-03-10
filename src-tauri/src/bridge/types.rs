use serde::{Deserialize, Serialize};

/// Response body for `GET /ping` — returned only when the caller's key matches.
/// `hash_key` is present ONLY when a one-time pairing token was used;
/// it carries the freshly-generated permanent key back to the phone.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PingResponse {
    pub device_id: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash_key: Option<String>,
}

/// Query parameters for `GET /ping`.
#[derive(Deserialize)]
pub struct PingQuery {
    /// The caller's hash key — must match the local key or the request is rejected.
    pub key: String,
}

/// Request body for `POST /register` — the caller registers itself as a peer.
#[derive(Deserialize)]
pub struct RegisterRequest {
    pub key: String,
    pub device_id: String,
    pub label: String,
    pub address: String,
}

/// Request body for `POST /tool` — execute a single tool on this device.
#[derive(Deserialize)]
pub struct ToolRequest {
    pub key: String,
    pub tool_name: String,
    pub tool_args: serde_json::Value,
}

/// Response for `POST /tool`.
#[derive(Serialize, Deserialize)]
pub struct ToolResponse {
    pub success: bool,
    pub output: String,
}

/// Online status of a single peer, returned to the frontend.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerStatus {
    pub device_id: String,
    pub label: String,
    pub address: String,
    pub online: bool,
}
