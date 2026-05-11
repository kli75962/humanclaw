use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

use super::core_tool::execute_memory_tool;
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
            | "type_credential"
            | "commit_suggestion"
            | "show_login_method_picker"
            | "fill_credential_field"
    )
}

async fn execute_phone_control_tool(app: &AppHandle, name: &str, args: &Value) -> ToolResult {
    #[cfg(target_os = "android")]
    let mut result = call_kotlin_tool(app, name, args).await;

    #[cfg(not(target_os = "android"))]
    let mut result = forward_to_android(app, name, args).await;

    if matches!(name, "get_screen" | "get_screen_deep") {
        append_tool_log(app, name, &result.output);
        if is_screen_unreadable(&result.output) {
            result.output.push_str(
                "\n\n[GESTURE_MAP_HINT] Screen content is minimal — app may use FLAG_SECURE or \
                 React Native rendering. Use search_gesture_maps(app_package=\"<pkg>\") to find \
                 recorded flows, or start_gesture_recording() to create one."
            );
        }
    }

    result
}

fn is_screen_unreadable(output: &str) -> bool {
    output == "[screen not accessible]"
        || output.chars().filter(|c| !c.is_whitespace()).count() < 120
}

pub fn append_tool_log(app: &AppHandle, tool_name: &str, content: &str) {
    let Ok(data_dir) = app.path().app_data_dir() else { return };
    let log_dir = data_dir.join("logs");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_path = log_dir.join("screen_log.txt");

    use std::io::Write;
    let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&log_path) else { return };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let _ = writeln!(file, "\n[{now}] {tool_name}\n{content}\n---");
}

#[cfg(not(target_os = "android"))]
async fn forward_to_android(app: &AppHandle, name: &str, args: &Value) -> ToolResult {
    use crate::network::types::ToolResponse;
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
    use crate::device::phone::plugin::PhoneControlHandle;
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
            let config_json = args.get("config_json").and_then(Value::as_str);

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
                    if let Err(e) = std::fs::write(skill_dir.join("SKILL.md"), content) {
                        return ToolResult::err("create_skill", "EXECUTION_FAILED", format!("Failed to write SKILL.md: {e}"));
                    }
                    // Write persona_config.json if provided
                    if let Some(cfg) = config_json {
                        // Validate it's parseable JSON before writing
                        if serde_json::from_str::<serde_json::Value>(cfg).is_ok() {
                            let _ = std::fs::write(skill_dir.join("persona_config.json"), cfg);
                        }
                    }
                    let app_clone = app.clone();
                    tauri::async_runtime::spawn(async move {
                        crate::network::sync::skills::sync_to_all_peers(&app_clone).await;
                    });
                    ToolResult::ok(
                        "create_skill",
                        format!("Persona '{skill_name}' saved. The user can now select it from Settings → Persona."),
                    )
                }
                Err(e) => ToolResult::err("create_skill", "EXECUTION_FAILED", format!("Could not resolve app data directory: {e}")),
            }
        }
        "get_installed_apps" => {
            #[cfg(target_os = "android")]
            {
                let apps = crate::device::phone::get_installed_apps(app).await;
                let result = if apps.is_empty() {
                    ToolResult::ok(name, "No user-installed apps found.")
                } else {
                    let list = apps.iter()
                        .filter(|a| !a.is_system)
                        .map(|a| a.prompt_line())
                        .collect::<Vec<_>>()
                        .join("\n");
                    if list.is_empty() {
                        ToolResult::ok(name, "No user-installed apps found.")
                    } else {
                        ToolResult::ok(name, list)
                    }
                };
                append_tool_log(app, name, &result.output);
                result
            }
            #[cfg(not(target_os = "android"))]
            {
                let result = forward_to_android(app, name, args).await;
                append_tool_log(app, name, &result.output);
                result
            }
        }
        _ if is_phone_control_tool(name) => execute_phone_control_tool(app, name, args).await,
        _ if super::pc::is_pc_control_tool(name) => super::pc::execute_pc_tool(app, name, args).await,
        _ if super::gesture_map::is_gesture_map_tool(name) =>
            super::gesture_map::execute_gesture_map_tool(app, name, args).await,
        _ => ToolResult::err(name, "NOT_FOUND", format!("Unknown tool: {name}")),
    }
}
