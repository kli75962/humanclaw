use serde_json::Value;

use crate::tools::types::ToolResult;

pub fn execute(tool_name: &str, args: &Value) -> ToolResult {
    let url = match args.get("url").and_then(Value::as_str) {
        Some(u) if !u.is_empty() => u,
        _ => return ToolResult::err(tool_name, "INVALID_ARGS", "url is required"),
    };

    match open_url(url) {
        Ok(msg) => ToolResult::ok(tool_name, msg),
        Err(e)  => ToolResult::err(tool_name, "EXECUTION_FAILED", e),
    }
}

#[cfg(target_os = "linux")]
fn open_url(url: &str) -> Result<String, String> {
    std::process::Command::new("xdg-open")
        .arg(url)
        .spawn()
        .map_err(|e| format!("xdg-open failed: {e}"))?;
    Ok(format!("Opened: {url}"))
}

#[cfg(target_os = "windows")]
fn open_url(url: &str) -> Result<String, String> {
    std::process::Command::new("cmd")
        .args(["/c", "start", "", url])
        .spawn()
        .map_err(|e| format!("start failed: {e}"))?;
    Ok(format!("Opened: {url}"))
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn open_url(_url: &str) -> Result<String, String> {
    Err("pc_open_url is only supported on Linux and Windows.".into())
}
