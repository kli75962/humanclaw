use std::path::Path;
use std::time::Duration;
use serde_json::Value;

use crate::tools::types::ToolResult;

// ── Shell command ─────────────────────────────────────────────────────────────

pub async fn run_command(tool_name: &str, args: &Value) -> ToolResult {
    let cmd         = args.get("cmd").and_then(Value::as_str).unwrap_or("").to_string();
    let timeout_sec = args.get("timeout_secs").and_then(Value::as_u64).unwrap_or(30);

    if cmd.is_empty() {
        return ToolResult { tool_name: tool_name.to_string(), success: false, output: "cmd is required".to_string() };
    }

    let tool_name = tool_name.to_string();
    let result = tokio::time::timeout(
        Duration::from_secs(timeout_sec),
        tokio::task::spawn_blocking(move || run_blocking(&cmd)),
    )
    .await;

    match result {
        Ok(Ok(mut r)) => { r.tool_name = tool_name; r }
        Ok(Err(e))    => ToolResult { tool_name, success: false, output: format!("Task error: {e}") },
        Err(_)        => ToolResult { tool_name, success: false, output: format!("Command timed out after {timeout_sec}s") },
    }
}

fn run_blocking(cmd: &str) -> ToolResult {
    #[cfg(target_os = "windows")]
    let output = std::process::Command::new("cmd").args(["/C", cmd]).output();
    #[cfg(not(target_os = "windows"))]
    let output = std::process::Command::new("sh").args(["-c", cmd]).output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let mut result = stdout.trim_end().to_string();
            if !stderr.trim().is_empty() {
                if !result.is_empty() { result.push('\n'); }
                result.push_str("[stderr] ");
                result.push_str(stderr.trim_end());
            }
            if result.is_empty() { result = "(no output)".to_string(); }
            ToolResult { tool_name: "pc_run_command".to_string(), success: out.status.success(), output: result }
        }
        Err(e) => ToolResult { tool_name: "pc_run_command".to_string(), success: false, output: e.to_string() },
    }
}

// ── File operations ───────────────────────────────────────────────────────────

pub fn file_write(tool_name: &str, args: &Value) -> ToolResult {
    let path    = args.get("path").and_then(Value::as_str).unwrap_or("").trim().to_string();
    let content = args.get("content").and_then(Value::as_str).unwrap_or("");

    if path.is_empty() {
        return ToolResult { tool_name: tool_name.to_string(), success: false, output: "path is required".to_string() };
    }
    let p = Path::new(&path);
    if let Some(parent) = p.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return ToolResult { tool_name: tool_name.to_string(), success: false, output: e.to_string() };
        }
    }
    match std::fs::write(p, content) {
        Ok(_)  => ToolResult { tool_name: tool_name.to_string(), success: true, output: format!("Written: {path}") },
        Err(e) => ToolResult { tool_name: tool_name.to_string(), success: false, output: e.to_string() },
    }
}

pub fn file_read(tool_name: &str, args: &Value) -> ToolResult {
    let path = args.get("path").and_then(Value::as_str).unwrap_or("").trim().to_string();
    if path.is_empty() {
        return ToolResult { tool_name: tool_name.to_string(), success: false, output: "path is required".to_string() };
    }
    match std::fs::read_to_string(&path) {
        Ok(s)  => ToolResult { tool_name: tool_name.to_string(), success: true, output: s },
        Err(e) => ToolResult { tool_name: tool_name.to_string(), success: false, output: e.to_string() },
    }
}

pub fn file_delete(tool_name: &str, args: &Value) -> ToolResult {
    let path = args.get("path").and_then(Value::as_str).unwrap_or("").trim().to_string();
    if path.is_empty() {
        return ToolResult { tool_name: tool_name.to_string(), success: false, output: "path is required".to_string() };
    }
    let p = Path::new(&path);
    let result = if p.is_dir() { std::fs::remove_dir_all(p) } else { std::fs::remove_file(p) };
    match result {
        Ok(_)  => ToolResult { tool_name: tool_name.to_string(), success: true, output: format!("Deleted: {path}") },
        Err(e) => ToolResult { tool_name: tool_name.to_string(), success: false, output: e.to_string() },
    }
}
