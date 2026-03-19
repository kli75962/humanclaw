use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::AppHandle;

use super::memory::execute_memory_tool;
use super::phone::{execute_phone_control_tool, is_phone_control_tool};

#[derive(Debug, Clone, Default)]
pub struct ToolExecutionContext {
    pub source_device_id: Option<String>,
    pub source_device_type: Option<String>,
}

/// Result returned to the LLM after executing a tool.
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_name: String,
    pub success: bool,
    pub output: String,
}

/// Unified tool dispatch entry point.
///
/// - Phone-control tools (tap/swipe/...) are always executed on Android.
///   On desktop, these are forwarded to the paired Android device.
pub async fn execute_tool_with_context(
    app: &AppHandle,
    name: &str,
    args: &Value,
    context: &ToolExecutionContext,
) -> ToolResult {
    match name {
        "memory" => ToolResult {
            tool_name: name.to_string(),
            success: true,
            output: execute_memory_tool(app, args, context),
        },
        _ if is_phone_control_tool(name) => execute_phone_control_tool(app, name, args).await,
        _ => ToolResult {
            tool_name: name.to_string(),
            success: false,
            output: format!("Unknown tool: {name}"),
        },
    }
}
