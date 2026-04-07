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
#[derive(Clone, Serialize)]
pub struct StreamPayload {
    pub content: String,
    pub done: bool,
    /// LLM-generated brief of the response (included when `done=true`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brief: Option<String>,
}

/// Status update emitted while the agent is executing tools.
#[derive(Clone, Serialize)]
pub struct AgentStatusPayload {
    pub message: String,
}

/// Maximum iterations of the agent tool-calling loop per invocation.
pub const MAX_AGENT_LOOPS: usize = 200;
