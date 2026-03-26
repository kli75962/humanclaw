use serde::{Deserialize, Serialize};

/// Result returned to the LLM after executing a tool.
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_name: String,
    pub success: bool,
    pub output: String,
}
