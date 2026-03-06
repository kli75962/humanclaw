use futures_util::StreamExt;
use serde_json::Value;
use tauri::{AppHandle, Emitter};

use crate::memory::{append_conversation, bootstrap_memory, build_core_prompt, execute_memory_write, memory_dir, read_core, read_recent_conversations, run_memory_command};
use crate::phone::{execute_tool, get_installed_apps, hide_overlay, is_cancelled, show_overlay};
use crate::loadskills::{build_skills_prompt, load_tool_schemas};
use crate::web_search::web_search;

use super::ollama_client;
use super::types::{
    AgentStatusPayload, OllamaChatRequest, OllamaChunk, OllamaMessage, OllamaToolCall,
    StreamPayload, ollama_chat_url,
};

const MAX_TOOL_ROUNDS: usize = 200;

/// Build the static part of the system prompt (skills + installed apps).
/// Called once per chat. Core memory is injected separately each round via prepareCall.
async fn build_base_prompt(app: &AppHandle) -> String {
    let apps = get_installed_apps(app).await;
    let apps_list = if apps.is_empty() {
        "  (no apps found)".to_string()
    } else {
        let mut buf = String::with_capacity(apps.len() * 60);
        for (i, a) in apps.iter().enumerate() {
            if i > 0 { buf.push('\n'); }
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
        "
        You are PhoneClaw, an AI agent that controls an Android phone on behalf of the user. \n\
        Be helpful, concise, and proactive. Break tasks into tool calls and execute them step by step. \n\
        Plain text only. NEVER use raw markdown symbols (`#`, `##`, `**`, `*`, `---`).
        
        \n\n{skills}\n\n[INSTALLED APPS]\n{apps}{devices}",
        skills = build_skills_prompt(),
        apps = apps_list,
        devices = device_section,
    )
}

/// Assemble the full system prompt for one LLM round.
/// Re-reads core.md fresh every round so updates made mid-session take effect
/// immediately on the next call (equivalent to the guide's `prepareCall` hook).
fn prepare_system(base: &str, core: &str) -> String {
    let core_block = build_core_prompt(core);
    if core_block.is_empty() {
        base.to_string()
    } else {
        format!("{core_block}\n\n{base}")
    }
}

/// Execute one streaming request to Ollama.
/// Returns the final assembled OllamaMessage (with tool_calls if any).
/// Tool_calls are accumulated across ALL chunks to support models that stream them.
async fn stream_once(
    app: &AppHandle,
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
    let mut accumulated_content = String::new();
    // Accumulate tool_calls from ALL chunks (some models stream them piecemeal)
    let mut accumulated_tool_calls: Vec<OllamaToolCall> = Vec::new();
    let mut final_role = "assistant".to_string();

    while let Some(chunk_result) = byte_stream.next().await {
        // Check cancel button on every chunk so the overlay responds immediately
        if is_cancelled(app) {
            return Err("CANCELLED".to_string());
        }
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
                final_role = msg.role.clone();

                // Accumulate text tokens and stream them to the frontend
                if !msg.content.is_empty() {
                    accumulated_content.push_str(&msg.content);
                    app.emit(
                        "ollama-stream",
                        StreamPayload {
                            content: msg.content.clone(),
                            done: false,
                        },
                    )
                    .map_err(|e| e.to_string())?;
                }

                // Accumulate tool_calls (may arrive on any chunk, typically the done one)
                if let Some(calls) = &msg.tool_calls {
                    accumulated_tool_calls.extend(calls.iter().cloned());
                }
            }

            if parsed.done {
                break;
            }
        }
    }

    Ok(OllamaMessage {
        role: final_role,
        content: accumulated_content,
        tool_calls: if accumulated_tool_calls.is_empty() {
            None
        } else {
            Some(accumulated_tool_calls)
        },
    })
}

/// Tauri command — streams a chat completion from the local Ollama server with
/// full agentic tool-calling support.
///
/// Flow:
/// 1. Build system prompt (skills + installed apps)
/// 2. Send messages to Ollama with tool schemas
/// 3. If LLM responds with tool_calls → execute via Kotlin → append results → repeat
/// 4. When LLM responds with plain text → emit done event
#[tauri::command]
pub async fn chat_ollama(
    app: AppHandle,
    messages: Vec<OllamaMessage>,
    model: String,
) -> Result<(), String> {
    let tool_schemas = load_tool_schemas();

    // Bootstrap memory files on first run (no-op if they already exist)
    bootstrap_memory(&app);

    // Build the static part of the system prompt once (skills + installed apps).
    // Core memory is re-read fresh every round (“prepareCall” pattern).
    let base_prompt = {
        let prompt = build_base_prompt(&app).await;
        let recent = read_recent_conversations(&app, 5);
        if recent.is_empty() { prompt } else { format!("{prompt}\n\n{recent}") }
    };

    // Conversation history without system message — system is prepended fresh each round.
    let mut conversation: Vec<OllamaMessage> = messages.into_iter().collect();

    // Show overlay indicator above all apps while the agent loop is running
    show_overlay(&app);

    // Agentic loop: keep going until the LLM stops issuing tool calls
    for round in 0..MAX_TOOL_ROUNDS {
        // Check if the user tapped the cancel button on the overlay
        if is_cancelled(&app) {
            hide_overlay(&app);
            app.emit(
                "ollama-stream",
                StreamPayload {
                    content: "\n\n[Cancelled by user]".to_string(),
                    done: true,
                },
            )
            .ok();
            return Ok(());
        }

        // ── prepareCall: inject fresh core memory into system prompt every round ──
        let core = read_core(&app);
        let system_content = prepare_system(&base_prompt, &core);
        let system_msg = OllamaMessage {
            role: "system".to_string(),
            content: system_content,
            tool_calls: None,
        };
        // Prepend system message just for this round’s request
        let mut round_messages = Vec::with_capacity(conversation.len() + 1);
        round_messages.push(system_msg);
        round_messages.extend(conversation.iter().cloned());

        let final_msg = match stream_once(&app, &round_messages, &tool_schemas, &model).await {
            Ok(msg) => msg,
            Err(e) if e == "CANCELLED" => {
                hide_overlay(&app);
                app.emit(
                    "ollama-stream",
                    StreamPayload {
                        content: "\n\n[Cancelled by user]".to_string(),
                        done: true,
                    },
                )
                .ok();
                return Ok(());
            }
            Err(e) => {
                hide_overlay(&app);
                return Err(e);
            }
        };

        // Debug: surface what the model returned so the user can see
        let tool_count = final_msg.tool_calls.as_ref().map_or(0, |v| v.len());
        app.emit(
            "agent-status",
            AgentStatusPayload {
                message: format!(
                    "[round {round}] tool_calls={tool_count} content_len={}",
                    final_msg.content.len()
                ),
            },
        )
        .ok();

        // Extract tool_calls with take() so we get ownership without cloning,
        // then push final_msg (now tool_calls=None) into history.
        let mut final_msg = final_msg;
        let tool_calls = final_msg.tool_calls.take().unwrap_or_default();

        if tool_calls.is_empty() {
            // No tool calls — conversation is done
            hide_overlay(&app);
            app.emit(
                "ollama-stream",
                StreamPayload {
                    content: String::new(),
                    done: true,
                },
            )
            .map_err(|e| e.to_string())?;

            // Append conversation summary in background (fire-and-forget)
            let user_msg = conversation.iter()
                .find(|m| m.role == "user")
                .map(|m| m.content.chars().take(300).collect::<String>())
                .unwrap_or_default();
            let reply_text = final_msg.content.chars().take(500).collect::<String>();
            let conv_dir = memory_dir(&app);
            tokio::spawn(async move {
                append_conversation(conv_dir, user_msg, reply_text);
            });

            return Ok(());
        }

        // Restore tool_calls so history correctly records what was requested,
        // then push assistant message.
        final_msg.tool_calls = Some(tool_calls.clone());
        conversation.push(final_msg);

        // Execute each tool and collect results
        for call in &tool_calls {
            let tool_name = &call.function.name;
            let tool_args = &call.function.arguments;

            app.emit("agent-status", AgentStatusPayload {
                message: format!("Running tool: {tool_name}"),
            })
            .ok();

            // ── Intercept `device_status` — check peer reachability in Rust ──
            let output = if tool_name == "device_status" {
                let device_id = tool_args.get("device_id").and_then(Value::as_str).unwrap_or("");
                let cfg = crate::session::store::bootstrap(&app);
                if device_id.is_empty() || device_id == cfg.device.device_id {
                    "online".to_string()
                } else if let Some(peer) = cfg.paired_devices.iter().find(|p| p.device_id == device_id) {
                    let reachable = crate::bridge::health::check_peer(&peer.address, &cfg.hash_key).await;
                    if reachable { "online".to_string() } else { "offline".to_string() }
                } else {
                    "unknown device_id".to_string()
                }
            // ── Intercept `web_search` — run in Rust, not forwarded to phone ──
            } else if tool_name == "web_search" {
                let query = tool_args.get("query").and_then(Value::as_str).unwrap_or("");
                if query.is_empty() {
                    "error: missing 'query' argument".to_string()
                } else {
                    let max = tool_args.get("max_results").and_then(Value::as_u64).unwrap_or(5) as usize;
                    web_search(query, max.clamp(1, 10)).await
                }
            // ── Intercept `memory` tool calls in Rust — never forward to Kotlin ──
            } else if tool_name == "memory" {
                let cmd     = tool_args.get("command").and_then(Value::as_str).unwrap_or("");
                let path    = tool_args.get("path").and_then(Value::as_str);
                let content = tool_args.get("content").and_then(Value::as_str);
                let mode    = tool_args.get("mode").and_then(Value::as_str);
                let query   = tool_args.get("query").and_then(Value::as_str);

                if cmd == "create" || cmd == "update" {
                    // Fire-and-forget: write in background so the LLM loop never waits on disk I/O
                    let dir      = memory_dir(&app);
                    let cmd_s    = cmd.to_string();
                    let path_s   = path.map(String::from);
                    let content_s = content.map(String::from);
                    let mode_s   = mode.map(String::from);
                    tokio::spawn(async move {
                        let _ = execute_memory_write(
                            dir,
                            &cmd_s,
                            path_s.as_deref(),
                            content_s.as_deref(),
                            mode_s.as_deref(),
                        );
                    });
                    "ok: memory saved".to_string()
                } else {
                    // view / search need the result synchronously
                    run_memory_command(&app, cmd, path, content, mode, query)
                }
            } else {
                let result = execute_tool(&app, tool_name, tool_args).await;
                result.output
            };

            conversation.push(OllamaMessage {
                role: "tool".to_string(),
                content: output,
                tool_calls: None,
            });
        }
        // Loop back to let the LLM process tool results
    }

    hide_overlay(&app);
    Err(format!("Agent exceeded maximum tool rounds ({MAX_TOOL_ROUNDS})"))
}
