use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

use super::memory::execute_memory_tool;
use super::types::ToolResult;

#[cfg(not(target_os = "android"))]
static PHONE_CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();

#[derive(Debug, Clone, Default)]
pub struct ToolExecutionContext {
    pub source_device_id: Option<String>,
    pub source_device_type: Option<String>,
}

fn is_phone_control_tool(name: &str) -> bool {
    matches!(
        name,
        "tap"
            | "swipe"
            | "type_text"
            | "press_key"
            | "launch_app"
            | "get_screen"
            | "get_screen_deep"
    )
}

async fn execute_phone_control_tool(app: &AppHandle, name: &str, args: &Value) -> ToolResult {
    #[cfg(target_os = "android")]
    {
        call_kotlin_tool(app, name, args).await
    }

    #[cfg(not(target_os = "android"))]
    {
        forward_to_android(app, name, args).await
    }
}

#[cfg(not(target_os = "android"))]
async fn forward_to_android(app: &AppHandle, name: &str, args: &Value) -> ToolResult {
    use crate::bridge::types::ToolResponse;
    use crate::session::store;

    let cfg = store::bootstrap(app);
    let peer = cfg.paired_devices.first();
    let Some(peer) = peer else {
        return ToolResult::err(
            name,
            "DEVICE_NOT_FOUND",
            "No paired Android device. Pair a device first via Settings -> Pair with QR code.",
        );
    };

    let url = format!("http://{}/tool", peer.address);
    let body = serde_json::json!({
        "key": cfg.hash_key,
        "tool_name": name,
        "tool_args": args,
    });

    let client = PHONE_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build phone forwarding client")
    });

    match client.post(&url).json(&body).send().await {
        Ok(resp) if resp.status().is_success() => match resp.json::<ToolResponse>().await {
            Ok(r) => ToolResult {
                tool_name: name.to_string(),
                success: r.success,
                output: r.output,
                error_code: None,
            },
            Err(e) => ToolResult::err(name, "INVALID_RESPONSE", format!("Invalid response from phone: {e}")),
        },
        Ok(resp) => ToolResult::err(name, "DEVICE_ERROR", format!("Phone returned {}", resp.status())),
        Err(e)   => ToolResult::err(name, "DEVICE_UNREACHABLE", format!("Could not reach phone at {}: {e}", peer.address)),
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
        Err(e) => ToolResult::err(name, "PLUGIN_ERROR", format!("Plugin error: {e}")),
    }
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
        "memory" => ToolResult::ok(name, execute_memory_tool(app, args, context)),
        "ask_user" => {
            let questions = args.get("questions").cloned().unwrap_or(serde_json::Value::Array(vec![]));
            let answers = super::ask_user::request_ask_user(app, &questions).await;

            // Format answers as readable Q/A pairs for the LLM
            let mut output = String::new();
            if let Some(arr) = questions.as_array() {
                for (i, q) in arr.iter().enumerate() {
                    let question_text = q.get("question").and_then(|v| v.as_str()).unwrap_or("");
                    let answer = answers.get(&i).map(|s| s.as_str()).unwrap_or("(no answer)");
                    if !output.is_empty() { output.push('\n'); }
                    output.push_str(&format!("Q: {question_text}\nA: {answer}"));
                }
            }
            ToolResult::ok(name, output)
        }
        "send_message" => {
            let content = args.get("content").and_then(Value::as_str).unwrap_or("");
            if !content.is_empty() {
                app.emit("ollama-injected-message", serde_json::json!({ "content": content })).ok();
            }
            ToolResult::ok(name, "ok: message delivered to user")
        }
        "create_skill" => {
            let skill_name = args.get("name").and_then(Value::as_str).unwrap_or("").trim().to_string();
            let content = args.get("content").and_then(Value::as_str).unwrap_or("");

            if !skill_name.starts_with("persona_") || skill_name.len() < 9 {
                return ToolResult::err(
                    "create_skill",
                    "INVALID_ARGS",
                    "Skill name must start with 'persona_' followed by at least one character.",
                );
            }
            if !skill_name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
                return ToolResult::err(
                    "create_skill",
                    "INVALID_ARGS",
                    "Skill name must contain only lowercase letters, digits, and underscores.",
                );
            }

            match app.path().app_data_dir() {
                Ok(data_dir) => {
                    let skill_dir = data_dir.join("custom_personas").join(&skill_name);
                    if let Err(e) = std::fs::create_dir_all(&skill_dir) {
                        return ToolResult::err("create_skill", "EXECUTION_FAILED", format!("Failed to create directory: {e}"));
                    }
                    match std::fs::write(skill_dir.join("SKILL.md"), content) {
                        Ok(_) => {
                            let app_clone = app.clone();
                            tauri::async_runtime::spawn(async move {
                                crate::bridge::persona_sync::sync_to_all_peers(&app_clone).await;
                            });
                            ToolResult::ok(
                                "create_skill",
                                format!("Persona '{skill_name}' saved. The user can now select it from Settings → Persona."),
                            )
                        }
                        Err(e) => ToolResult::err("create_skill", "EXECUTION_FAILED", format!("Failed to write SKILL.md: {e}")),
                    }
                }
                Err(e) => ToolResult::err("create_skill", "EXECUTION_FAILED", format!("Could not resolve app data directory: {e}")),
            }
        }
        _ if is_phone_control_tool(name) => execute_phone_control_tool(app, name, args).await,
        _ if super::pc::is_pc_control_tool(name) => super::pc::execute_pc_tool(app, name, args).await,
        _ => ToolResult::err(name, "NOT_FOUND", format!("Unknown tool: {name}")),
    }
}
