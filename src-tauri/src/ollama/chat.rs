use futures_util::StreamExt;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter};

use crate::memory::bootstrap_memory;
use crate::phone::{get_installed_apps, hide_overlay, is_cancelled, show_overlay};
use crate::skills::{build_persona_prompt, build_skills_prompt, load_tool_schemas};
use crate::tools::{build_core_prompt, execute_tool_with_context, read_core, ToolExecutionContext};

use super::types::{
    AgentStatusPayload, OllamaChunk, OllamaMessage, OllamaToolCall,
    StreamPayload,
};

const MAX_TOOL_ROUNDS: usize = 200;

/// Set to true by `cancel_chat` command. Reset to false at the start of each chat_ollama run.
static CHAT_CANCEL: AtomicBool = AtomicBool::new(false);

/// Tauri command — signal the running chat_ollama loop to stop.
#[tauri::command]
pub fn cancel_chat() {
    CHAT_CANCEL.store(true, Ordering::Relaxed);
}

/// Returns true if either the overlay cancel button (Android) or the frontend stop button was pressed.
fn should_cancel(app: &AppHandle) -> bool {
    CHAT_CANCEL.load(Ordering::Relaxed) || is_cancelled(app)
}

fn local_tool_context(app: &AppHandle) -> ToolExecutionContext {
    let cfg = crate::session::store::bootstrap(app);
    let source_device_type = match cfg.device.device_type {
        crate::session::types::DeviceType::Android => "phone",
        crate::session::types::DeviceType::Desktop => "pc",
    }
    .to_string();

    ToolExecutionContext {
        source_device_id: Some(cfg.device.device_id),
        source_device_type: Some(source_device_type),
    }
}

/// Build the static part of the system prompt (skills + installed apps).
/// Called once per chat. Core memory is injected separately each round via prepareCall.
async fn build_base_prompt(app: &AppHandle) -> String {
    let apps = get_installed_apps(app).await;
    let apps_list = if apps.is_empty() {
        "  (no apps found)".to_string()
    } else {
        // Write directly into a pre-sized String to avoid an intermediate Vec allocation.
        let mut buf = String::with_capacity(apps.len() * 60);
        for (i, a) in apps.iter().enumerate() {
            if i > 0 { buf.push('\n'); }
            buf.push_str(&a.prompt_line());
        }
        buf
    };

    let cfg = crate::session::store::bootstrap(app);
    let persona = build_persona_prompt(Some(cfg.persona.as_str()));

    format!(
        "{persona}\n\n{skills}\n\n[INSTALLED APPS]\n{apps}",
        persona = persona,
        skills = build_skills_prompt(),
        apps = apps_list,
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
    system_msg: &OllamaMessage,
    conversation: &[OllamaMessage],
    tool_schemas: &[Value],
    model: &str,
) -> Result<OllamaMessage, String> {
    let body = super::types::OllamaRoundRequest::new(model, system_msg, conversation, true, tool_schemas);

    let response = super::ollama_client()
        .post(super::types::ollama_chat_url(app))
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
        // Check cancel on every chunk
        if should_cancel(app) {
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
    // Reset cancel flag for this run
    CHAT_CANCEL.store(false, Ordering::Relaxed);

    let tool_schemas = load_tool_schemas();

    // Bootstrap memory files on first run (no-op if they already exist)
    bootstrap_memory(&app);

    // Build the static part of the system prompt once (skills + installed apps).
    // Core memory is re-read fresh every round (“prepareCall” pattern).
    let base_prompt = build_base_prompt(&app).await;

    // Conversation history without system message — system is prepended fresh each round.
    let mut conversation: Vec<OllamaMessage> = messages.into_iter().collect();
    let tool_context = local_tool_context(&app);

    // Show overlay indicator above all apps while the agent loop is running
    show_overlay(&app);

    // Agentic loop: keep going until the LLM stops issuing tool calls
    for round in 0..MAX_TOOL_ROUNDS {
        // Check if the user tapped cancel (overlay button or frontend stop button)
        if should_cancel(&app) {
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
        let final_msg = match stream_once(&app, &system_msg, &conversation, tool_schemas, &model).await {
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

            let output = execute_tool_with_context(&app, tool_name, tool_args, &tool_context)
                .await
                .output;

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