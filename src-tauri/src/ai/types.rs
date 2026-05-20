use serde::{Deserialize, Serialize};

/// Character identity override — passed from the frontend when in Chat Mode.
/// Replaces the session persona in the system prompt.
#[derive(Deserialize, Clone)]
pub struct CharacterOverride {
    pub id: Option<String>,
    pub name: String,
    pub persona: String,     // persona skill name, e.g. "persona_jk"
    pub background: String,
}

/// Payload emitted via the `ollama-stream` Tauri event for every token.
///
/// `chat_id` identifies which chat the stream belongs to so that listeners on
/// the peer device can route the chunk to the correct conversation. `remote=true`
/// means the chunk originated on a paired device (received via SSE).
#[derive(Clone, Serialize)]
pub struct StreamPayload {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_id: Option<String>,
    #[serde(default)]
    pub remote: bool,
    pub content: String,
    pub done: bool,
    /// LLM-generated brief of the response (included when `done=true`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brief: Option<String>,
}

/// Status update emitted while the agent is executing tools.
#[derive(Clone, Serialize)]
pub struct AgentStatusPayload {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_id: Option<String>,
    #[serde(default)]
    pub remote: bool,
    pub message: String,
}

/// Maximum iterations of the agent tool-calling loop per invocation.
pub const MAX_AGENT_LOOPS: usize = 200;
