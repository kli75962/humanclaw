use serde::{Deserialize, Serialize};

/// Result returned to the LLM after executing a tool.
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_name: String,
    pub success: bool,
    pub output: String,
    /// Structured error code so the LLM can distinguish failure reasons.
    /// e.g. "PERMISSION_DENIED", "NOT_AVAILABLE", "NOT_FOUND", "INVALID_ARGS", "EXECUTION_FAILED"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

impl ToolResult {
    /// Construct a successful result.
    pub fn ok(tool_name: impl Into<String>, output: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
            success: true,
            output: output.into(),
            error_code: None,
        }
    }

    /// Construct a failure result with a structured error code.
    pub fn err(tool_name: impl Into<String>, code: &'static str, msg: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
            success: false,
            output: msg.into(),
            error_code: Some(code.to_string()),
        }
    }
}
