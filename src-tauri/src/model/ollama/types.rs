use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// A single message in the Ollama chat conversation.
/// Roles accepted by Ollama: "user" | "assistant" | "system" | "tool"
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OllamaMessage {
    pub role: String,
    /// Ollama may omit or set `content: null` for tool-call-only rounds.
    #[serde(default, deserialize_with = "deserialize_content")]
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OllamaToolCall>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OllamaToolCall {
    pub function: OllamaToolCallFunction,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OllamaToolCallFunction {
    pub name: String,
    #[serde(deserialize_with = "deserialize_arguments")]
    pub arguments: Value,
}

fn deserialize_content<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

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

// ── Zero-copy request builder ─────────────────────────────────────────────────

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

// ── URL helpers ───────────────────────────────────────────────────────────────

#[cfg(target_os = "android")]
fn default_host(app: &tauri::AppHandle) -> String {
    if let Some(cfg) = crate::session::store::load(app) {
        if let Some(peer) = cfg.paired_devices.first() {
            return peer.address.split(':').next().unwrap_or(&peer.address).to_string();
        }
    }
    "127.0.0.1".to_string()
}

fn ollama_host_port(app: &tauri::AppHandle) -> (String, u16) {
    if let Some(cfg) = crate::session::store::load(app) {
        let port = if cfg.ollama_port == 0 { 11434 } else { cfg.ollama_port };
        if let Some(host) = cfg
            .ollama_host_override
            .as_deref()
            .map(str::trim)
            .filter(|h| !h.is_empty())
        {
            return (host.to_string(), port);
        }
        #[cfg(target_os = "android")]
        return (default_host(app), port);
        #[cfg(not(target_os = "android"))]
        return ("127.0.0.1".to_string(), port);
    }
    #[cfg(target_os = "android")]
    { (default_host(app), 11434) }
    #[cfg(not(target_os = "android"))]
    { ("127.0.0.1".to_string(), 11434) }
}

pub fn ollama_chat_url(app: &tauri::AppHandle) -> String {
    let (host, port) = ollama_host_port(app);
    format!("http://{host}:{port}/api/chat")
}

pub fn ollama_tags_url(app: &tauri::AppHandle) -> String {
    let (host, port) = ollama_host_port(app);
    format!("http://{host}:{port}/api/tags")
}
