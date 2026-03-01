use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::AppHandle;
#[cfg(target_os = "android")]
use tauri::Manager;

/// Result returned to the LLM after executing a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_name: String,
    pub success: bool,
    pub output: String,
}

/// Dispatch a tool call from the LLM to the Kotlin accessibility layer.
/// `name` is the function name, `args` is the parsed JSON arguments object.
pub async fn execute_tool(app: &AppHandle, name: &str, args: &Value) -> ToolResult {
    #[cfg(target_os = "android")]
    {
        call_kotlin_tool(app, name, args).await
    }

    #[cfg(not(target_os = "android"))]
    {
        // Desktop stub — echoes the call so development works without a device
        let _ = app;
        ToolResult {
            tool_name: name.to_string(),
            success: true,
            output: format!("[desktop stub] tool `{name}` called with args: {args}"),
        }
    }
}

#[cfg(target_os = "android")]
async fn call_kotlin_tool(app: &AppHandle, name: &str, args: &Value) -> ToolResult {
    use crate::phone::plugin::PhoneControlHandle;
    use serde_json::json;

    let handle = app.state::<PhoneControlHandle<tauri::Wry>>();
    let payload = json!({
        "tool": name,
        "args": args,
    });

    match handle
        .0
        .run_mobile_plugin::<ToolResult>("executeTool", payload)
    {
        Ok(result) => result,
        Err(e) => ToolResult {
            tool_name: name.to_string(),
            success: false,
            output: format!("Plugin error: {e}"),
        },
    }
}
