use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// A single message in the Ollama chat conversation.
/// Roles accepted by Ollama: "user" | "assistant" | "system" | "tool"
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OllamaMessage {
    pub role: String,
    /// Ollama may omit or set `content: null` for tool-call-only rounds.
    /// Using a custom deserializer coerces null/absent → empty string so the
    /// whole chunk is never silently dropped by the streaming parser.
    #[serde(default, deserialize_with = "deserialize_content")]
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

/// Coerces `null` or a missing `content` field to an empty string.
/// Without this, any Ollama chunk where `content` is null fails to deserialize,
/// causing the streaming parser to silently skip the chunk — dropping tool_calls
/// contained in that same chunk and making it look like the model made no tool call.
fn deserialize_content<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
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


// ── Zero-copy request builder for per-round LLM calls ───────────────────────

/// Serializes [system_msg, history...] as a contiguous JSON array without
/// cloning any message. Used instead of building a temporary Vec each round.
struct RoundMessages<'a> {
    system: &'a OllamaMessage,
    history: &'a [OllamaMessage],
}

impl serde::Serialize for RoundMessages<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(1 + self.history.len()))?;
        seq.serialize_element(self.system)?;
        for msg in self.history {
            seq.serialize_element(msg)?;
        }
        seq.end()
    }
}

fn tools_is_empty<T>(s: &&[T]) -> bool {
    s.is_empty()
}

/// Borrowed request type — avoids cloning messages and tool schemas on every
/// LLM round in the agentic loop. Use instead of `OllamaChatRequest` when the
/// system message and conversation are already in separate owned structures.
#[derive(Serialize)]
pub struct OllamaRoundRequest<'a> {
    pub model: &'a str,
    messages: RoundMessages<'a>,
    pub stream: bool,
    #[serde(skip_serializing_if = "tools_is_empty")]
    pub tools: &'a [Value],
}

impl<'a> OllamaRoundRequest<'a> {
    pub fn new(
        model: &'a str,
        system: &'a OllamaMessage,
        history: &'a [OllamaMessage],
        stream: bool,
        tools: &'a [Value],
    ) -> Self {
        Self {
            model,
            messages: RoundMessages { system, history },
            stream,
            tools,
        }
    }
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

#[cfg(target_os = "android")]
fn default_android_fallback_host(app: &tauri::AppHandle) -> String {
    if let Some(cfg) = crate::session::store::load(app) {
        if let Some(peer) = cfg.paired_devices.first() {
            // peer.address is "ip:bridge_port", extract the ip part
            return peer.address.split(':').next().unwrap_or(&peer.address).to_string();
        }
    }
    // No paired device — Ollama unreachable until the phone is paired with the desktop.
    "127.0.0.1".to_string()
}

#[cfg(not(target_os = "android"))]
fn default_desktop_fallback_host() -> String {
    "127.0.0.1".to_string()
}

/// Return the Ollama host for the current platform.
/// - Android: uses the first paired device's address (the desktop running Ollama).
/// - Desktop: uses localhost.
pub fn ollama_host(app: &tauri::AppHandle) -> String {
    let (host, port) = if let Some(cfg) = crate::session::store::load(app) {
        let port = if cfg.ollama_port == 0 { 11434 } else { cfg.ollama_port };
        if let Some(host) = cfg
            .ollama_host_override
            .as_deref()
            .map(str::trim)
            .filter(|h| !h.is_empty())
        {
            (host.to_string(), port)
        } else {
            #[cfg(target_os = "android")]
            {
                (default_android_fallback_host(app), port)
            }
            #[cfg(not(target_os = "android"))]
            {
                (default_desktop_fallback_host(), port)
            }
        }
    } else {
        #[cfg(target_os = "android")]
        {
            (default_android_fallback_host(app), 11434)
        }
        #[cfg(not(target_os = "android"))]
        {
            (default_desktop_fallback_host(), 11434)
        }
    };

    format!("http://{host}:{port}")
}

pub fn ollama_chat_url(app: &tauri::AppHandle) -> String {
    format!("{}/api/chat", ollama_host(app))
}

pub fn ollama_tags_url(app: &tauri::AppHandle) -> String {
    format!("{}/api/tags", ollama_host(app))
}
