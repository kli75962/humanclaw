pub mod chat;
pub mod types;

pub use chat::chat_claude;
pub use chat::load_api_key;

use std::sync::OnceLock;

static CLAUDE_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

pub fn claude_client() -> &'static reqwest::Client {
    CLAUDE_CLIENT.get_or_init(reqwest::Client::new)
}

/// Simple message type accepted from the frontend (role + string content).
/// Identical in shape to the user/assistant messages that the frontend sends.
#[derive(serde::Deserialize)]
pub struct InputMessage {
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub brief: Option<String>,
}
