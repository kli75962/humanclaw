#![allow(dead_code)]
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ── Outbound message types ────────────────────────────────────────────────────

/// A message in the Claude conversation history.
/// Content may be a plain string (user/simple assistant) or a content block array.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ClaudeMessage {
    pub role: String,
    pub content: ClaudeContent,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum ClaudeContent {
    Text(String),
    Blocks(Vec<ClaudeBlock>),
}

impl From<String> for ClaudeContent {
    fn from(s: String) -> Self {
        ClaudeContent::Text(s)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClaudeBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

/// Claude API request.
#[derive(Serialize)]
pub struct ClaudeRequest<'a> {
    pub model: &'a str,
    pub max_tokens: u32,
    pub system: &'a str,
    pub messages: &'a [ClaudeMessage],
    #[serde(skip_serializing_if = "<[Value]>::is_empty")]
    pub tools: &'a [Value],
    pub stream: bool,
}

// ── Tool schema conversion ────────────────────────────────────────────────────

/// Convert an OpenAI-format tool schema to Claude format.
///
/// OpenAI: `{ "type": "function", "function": { "name", "description", "parameters" } }`
/// Claude: `{ "name", "description", "input_schema" }`
pub fn openai_tool_to_claude(tool: &Value) -> Option<Value> {
    let func = tool.get("function")?;
    let name = func.get("name")?.as_str()?;
    let description = func.get("description").and_then(Value::as_str).unwrap_or("");
    let input_schema = func.get("parameters").cloned()
        .unwrap_or_else(|| serde_json::json!({"type": "object", "properties": {}}));

    Some(serde_json::json!({
        "name": name,
        "description": description,
        "input_schema": input_schema,
    }))
}

// ── SSE response types ────────────────────────────────────────────────────────

/// An in-flight content block being assembled during streaming.
#[derive(Debug)]
pub enum InFlightBlock {
    Text { text: String, emitted: usize },
    ToolUse { id: String, name: String, input_json: String },
}

/// Fully assembled result from one Claude streaming round.
pub struct ClaudeRoundResult {
    pub text: String,
    pub tool_calls: Vec<ClaudeToolCall>,
    pub stop_reason: String,
}

pub struct ClaudeToolCall {
    pub id: String,
    pub name: String,
    pub input: Value,
}

// ── SSE event types ───────────────────────────────────────────────────────────

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SseEvent {
    MessageStart {
        message: MessageStartData,
    },
    ContentBlockStart {
        index: usize,
        content_block: ContentBlockStartData,
    },
    ContentBlockDelta {
        index: usize,
        delta: ContentBlockDelta,
    },
    ContentBlockStop {
        index: usize,
    },
    MessageDelta {
        delta: MessageDeltaData,
    },
    MessageStop,
    Ping,
    Error {
        error: ErrorData,
    },
}

#[derive(Deserialize, Debug)]
pub struct MessageStartData {
    pub id: String,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockStartData {
    Text { text: String },
    ToolUse { id: String, name: String },
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockDelta {
    TextDelta { text: String },
    InputJsonDelta { partial_json: String },
}

#[derive(Deserialize, Debug)]
pub struct MessageDeltaData {
    pub stop_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct ErrorData {
    pub message: String,
}
