/// Headless Ollama agent loop — no Tauri event streaming, returns the final
/// response text.  Used by the bridge server when executing commands that
/// arrive from Discord or a remote peer device.
use futures_util::StreamExt;
use serde_json::Value;
use tauri::AppHandle;

use crate::memory::{
    append_conversation, bootstrap_memory, build_core_prompt, execute_memory_write,
    memory_dir, read_core, read_recent_conversations, run_memory_command,
};
use crate::phone::{execute_tool, get_installed_apps};
use crate::loadskills::{build_skills_prompt, load_tool_schemas};
use crate::web_search::web_search;

use super::ollama_client;
use super::types::{
    OllamaChatRequest, OllamaChunk, OllamaMessage, OllamaToolCall, ollama_chat_url,
};

const MAX_TOOL_ROUNDS: usize = 200;

// ── System prompt (mirrors chat.rs, without overlay/cancel) ─────────────────

async fn build_base_prompt(app: &AppHandle) -> String {
    let apps = get_installed_apps(app).await;
    let apps_list = if apps.is_empty() {
        "  (no apps found)".to_string()
    } else {
        let mut buf = String::with_capacity(apps.len() * 60);
        for (i, a) in apps.iter().enumerate() {
            if i > 0 {
                buf.push('\n');
            }
            buf.push_str(&a.prompt_line());
        }
        buf
    };

    let cfg = crate::session::store::bootstrap(app);
    let device_section = if cfg.paired_devices.is_empty() {
        String::new()
    } else {
        let mut buf = String::from("\n\n[PAIRED DEVICES]\n");
        buf.push_str("Phone tools (tap, swipe, get_screen, etc.) are forwarded to the paired Android device automatically.\n");
        for p in &cfg.paired_devices {
            buf.push_str(&format!("- {} ({})\n", p.label, p.device_id));
        }
        buf
    };

    format!(
        "You are PhoneClaw, an AI agent that controls devices on behalf of the user.\n\
        Be helpful, concise, and proactive. Break tasks into tool calls and execute them step by step.\n\
        Plain text only. NEVER use raw markdown symbols (`#`, `##`, `**`, `*`, `---`).\n\n\
        {skills}\n\n[INSTALLED APPS]\n{apps}{devices}",
        skills = build_skills_prompt(),
        apps = apps_list,
        devices = device_section,
    )
}

fn prepare_system(base: &str, core: &str) -> String {
    let core_block = build_core_prompt(core);
    if core_block.is_empty() {
        base.to_string()
    } else {
        format!("{core_block}\n\n{base}")
    }
}

// ── Single LLM round (accumulate only, no emit) ───────────────────────────────

async fn stream_once_headless(
    messages: &[OllamaMessage],
    tool_schemas: &[Value],
    model: &str,
) -> Result<OllamaMessage, String> {
    let body = OllamaChatRequest {
        model: model.to_string(),
        messages: messages.to_vec(),
        stream: true,
        tools: tool_schemas.to_vec(),
    };

    let response = ollama_client()
        .post(ollama_chat_url())
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Failed to reach Ollama: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Ollama returned {status}: {text}"));
    }

    let mut byte_stream = response.bytes_stream();
    let mut content = String::new();
    let mut tool_calls: Vec<OllamaToolCall> = Vec::new();
    let mut role = "assistant".to_string();

    while let Some(chunk_result) = byte_stream.next().await {
        let bytes = chunk_result.map_err(|e| format!("Stream error: {e}"))?;
        let text = String::from_utf8_lossy(&bytes);

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(parsed) = serde_json::from_str::<OllamaChunk>(line) else {
                continue;
            };
            if let Some(ref msg) = parsed.message {
                role = msg.role.clone();
                content.push_str(&msg.content);
                if let Some(calls) = &msg.tool_calls {
                    tool_calls.extend(calls.iter().cloned());
                }
            }
            if parsed.done {
                break;
            }
        }
    }

    Ok(OllamaMessage {
        role,
        content,
        tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
    })
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Run a full agentic loop (including tool calls) without emitting Tauri events.
/// Returns the final assistant response text.
///
/// `conversation` should contain prior messages in oldest-first order,
/// ending with the latest user message.
pub async fn run_headless(
    app: &AppHandle,
    conversation: Vec<OllamaMessage>,
    model: &str,
) -> Result<String, String> {
    let tool_schemas = load_tool_schemas();
    bootstrap_memory(app);

    let base_prompt = {
        let prompt = build_base_prompt(app).await;
        let recent = read_recent_conversations(app, 5);
        if recent.is_empty() { prompt } else { format!("{prompt}\n\n{recent}") }
    };

    let mut history = conversation;

    for _ in 0..MAX_TOOL_ROUNDS {
        let core = read_core(app);
        let system_content = prepare_system(&base_prompt, &core);
        let system_msg = OllamaMessage {
            role: "system".to_string(),
            content: system_content,
            tool_calls: None,
        };

        let mut round_messages = Vec::with_capacity(history.len() + 1);
        round_messages.push(system_msg);
        round_messages.extend(history.iter().cloned());

        let mut final_msg =
            stream_once_headless(&round_messages, &tool_schemas, model).await?;

        let tool_calls = final_msg.tool_calls.take().unwrap_or_default();

        if tool_calls.is_empty() {
            // Done — save conversation in background and return
            let user_msg = history
                .iter()
                .find(|m| m.role == "user")
                .map(|m| m.content.chars().take(300).collect::<String>())
                .unwrap_or_default();
            let reply = final_msg.content.chars().take(500).collect::<String>();
            let conv_dir = memory_dir(app);
            tokio::spawn(async move {
                append_conversation(conv_dir, user_msg, reply);
            });

            return Ok(final_msg.content);
        }

        // Restore tool_calls for history
        final_msg.tool_calls = Some(tool_calls.clone());
        history.push(final_msg);

        // Execute tools
        for call in &tool_calls {
            let tool_name = &call.function.name;
            let tool_args = &call.function.arguments;

            let output = if tool_name == "memory" {
                let cmd     = tool_args.get("command").and_then(Value::as_str).unwrap_or("");
                let path    = tool_args.get("path").and_then(Value::as_str);
                let content = tool_args.get("content").and_then(Value::as_str);
                let mode    = tool_args.get("mode").and_then(Value::as_str);
                let query   = tool_args.get("query").and_then(Value::as_str);

                if cmd == "create" || cmd == "update" {
                    let dir       = memory_dir(app);
                    let cmd_s     = cmd.to_string();
                    let path_s    = path.map(String::from);
                    let content_s = content.map(String::from);
                    let mode_s    = mode.map(String::from);
                    tokio::spawn(async move {
                        let _ = execute_memory_write(dir, &cmd_s, path_s.as_deref(), content_s.as_deref(), mode_s.as_deref());
                    });
                    "ok: memory saved".to_string()
                } else {
                    run_memory_command(app, cmd, path, content, mode, query)
                }
            } else if tool_name == "web_search" {
                let query = tool_args.get("query").and_then(Value::as_str).unwrap_or("");
                if query.is_empty() {
                    "error: missing 'query' argument".to_string()
                } else {
                    let max = tool_args.get("max_results").and_then(Value::as_u64).unwrap_or(5) as usize;
                    web_search(query, max.clamp(1, 10)).await
                }
            } else if tool_name == "device_status" {
                let device_id = tool_args.get("device_id").and_then(Value::as_str).unwrap_or("");
                let cfg = crate::session::store::bootstrap(app);
                let online = if device_id.is_empty() || device_id == cfg.device.device_id {
                    "online: this device"
                } else {
                    let peer = cfg.paired_devices.iter().find(|p| p.device_id == device_id);
                    match peer {
                        Some(p) => {
                            // Try to ping the peer
                            if crate::bridge::health::check_peer(&p.address, &cfg.hash_key).await {
                                "online"
                            } else {
                                "offline"
                            }
                        },
                        None => "error: device not found",
                    }
                };
                format!("device_status: {online}")
            } else {
                execute_tool(app, tool_name, tool_args).await.output
            };

            history.push(OllamaMessage {
                role: "tool".to_string(),
                content: output,
                tool_calls: None,
            });
        }
    }

    Err(format!("Agent exceeded maximum tool rounds ({MAX_TOOL_ROUNDS})"))
}
