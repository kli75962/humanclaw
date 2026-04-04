use serde_json::Value;

use crate::tools::types::ToolResult;

/// Maximum bytes returned to the LLM from a single command.
const MAX_OUTPUT: usize = 8_000;

pub async fn execute(tool_name: &str, args: &Value) -> ToolResult {
    let command = match args.get("command").and_then(Value::as_str) {
        Some(c) if !c.is_empty() => c.to_string(),
        _ => return ToolResult::err(tool_name, "INVALID_ARGS", "command is required"),
    };

    let cmd_args: Vec<String> = args
        .get("args")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();

    let timeout_secs = args.get("timeout_secs").and_then(Value::as_u64).unwrap_or(30);

    match run_command(&command, &cmd_args, timeout_secs).await {
        Ok(output) => ToolResult::ok(tool_name, output),
        Err(e)     => ToolResult::err(tool_name, "EXECUTION_FAILED", e),
    }
}

async fn run_command(command: &str, args: &[String], timeout_secs: u64) -> Result<String, String> {
    use tokio::process::Command;
    use tokio::time::{timeout, Duration};

    let fut = Command::new(command).args(args).output();

    let output = timeout(Duration::from_secs(timeout_secs), fut)
        .await
        .map_err(|_| format!("Command timed out after {timeout_secs}s"))?
        .map_err(|e| format!("Failed to spawn '{command}': {e}"))?;

    let mut result = String::new();

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.trim().is_empty() {
        result.push_str(stdout.trim());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.trim().is_empty() {
        if !result.is_empty() { result.push('\n'); }
        result.push_str("[stderr] ");
        result.push_str(stderr.trim());
    }

    if result.is_empty() {
        result = format!("exit code: {}", output.status.code().unwrap_or(-1));
    }

    if result.len() > MAX_OUTPUT {
        result.truncate(MAX_OUTPUT);
        result.push_str("\n...[truncated]");
    }

    Ok(result)
}
