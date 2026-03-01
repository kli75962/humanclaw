use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// A single message in the Ollama chat conversation.
/// Roles accepted by Ollama: "user" | "assistant" | "system" | "tool"
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OllamaMessage {
    pub role: String,
    pub content: String,
    /// Tool calls requested by the assistant (Ollama function-calling format).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OllamaToolCall>>,
}

/// A tool call issued by the LLM (function-calling).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OllamaToolCall {
    pub function: OllamaToolCallFunction,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OllamaToolCallFunction {
    pub name: String,
    /// Arguments may arrive as a JSON object OR as a JSON-encoded string —
    /// `deserialize_arguments` handles both transparently.
    #[serde(deserialize_with = "deserialize_arguments")]
    pub arguments: Value,
}

/// Deserializes LLM tool call arguments that may arrive as either:
/// - A JSON object: `{"package_name": "com.foo.bar"}`
/// - A JSON-encoded string: `"{\"package_name\": \"com.foo.bar\"}"`
fn deserialize_arguments<'de, D>(deserializer: D) -> Result<Value, D::Error>
where
    D: Deserializer<'de>,
{
    let v = Value::deserialize(deserializer)?;
    if let Value::String(s) = &v {
        if let Ok(parsed) = serde_json::from_str::<Value>(s) {
            return Ok(parsed);
        }
    }
    Ok(v)
}

/// Request body sent to the Ollama `/api/chat` endpoint.
#[derive(Serialize)]
pub struct OllamaChatRequest {
    pub model: String,
    pub messages: Vec<OllamaMessage>,
    pub stream: bool,
    /// Ollama tool schemas (passed only when tools are available).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<Value>,
}

/// A single NDJSON chunk from the Ollama streaming response.
#[derive(Deserialize)]
pub struct OllamaChunk {
    pub message: Option<OllamaMessage>,
    pub done: bool,
}

/// Payload emitted via the `ollama-stream` Tauri event for every token.
#[derive(Clone, Serialize)]
pub struct StreamPayload {
    pub content: String,
    pub done: bool,
}

/// Status update emitted while the agent is executing tools.
#[derive(Clone, Serialize)]
pub struct AgentStatusPayload {
    pub message: String,
}
