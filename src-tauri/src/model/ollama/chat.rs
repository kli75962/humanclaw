use futures_util::StreamExt;
use tokio::time::{timeout, Duration};
use serde_json::Value;
use tauri::{AppHandle, Emitter};

use crate::memory::bootstrap_memory;
use crate::phone::{hide_overlay, show_overlay};
use crate::skills::load_tool_schemas;
use crate::tools::{execute_tool_with_context, ToolExecutionContext};
use crate::model::ollama::types::tool_message;
use crate::model::shared::{
    build_base_prompt, prepare_system, should_cancel, AgentStatusPayload, StreamPayload,
    MAX_TOOL_ROUNDS,
};

use super::types::{OllamaChunk, OllamaMessage, OllamaRoundRequest, OllamaToolCall};

fn local_tool_context(app: &AppHandle) -> ToolExecutionContext {
    let cfg = crate::session::store::bootstrap(app);
    ToolExecutionContext {
        source_device_id: Some(cfg.device.device_id),
        source_device_type: Some(cfg.device.device_type.label().to_string()),
    }
}

/// Execute one streaming request to Ollama.
/// Emits content tokens to the frontend and returns the fully assembled message.
async fn stream_once(
    app: &AppHandle,
    system_msg: &OllamaMessage,
    conversation: &[OllamaMessage],
    tool_schemas: &[Value],
    model: &str,
) -> Result<OllamaMessage, String> {
    let body = OllamaRoundRequest::new(model, system_msg, conversation, true, tool_schemas);

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
    let mut accumulated_tool_calls: Vec<OllamaToolCall> = Vec::new();
    let mut final_role = "assistant".to_string();

    // 120 s idle timeout per chunk — prevents indefinite hang when a model stalls mid-stream.
    const CHUNK_IDLE_SECS: u64 = 120;

    loop {
        let next = timeout(Duration::from_secs(CHUNK_IDLE_SECS), byte_stream.next()).await;
        let chunk_result = match next {
            Ok(Some(r)) => r,
            Ok(None) => break,                       // stream ended normally
            Err(_) => return Err(format!("Ollama stream timed out after {CHUNK_IDLE_SECS}s — model may be too slow or stalled")),
        };

        if should_cancel(app) {
            return Err("CANCELLED".to_string());
        }
        let bytes = chunk_result.map_err(|e| format!("Stream error: {e}"))?;
        let text = String::from_utf8_lossy(&bytes);

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() { continue; }

            let Ok(parsed) = serde_json::from_str::<OllamaChunk>(line) else { continue };

            if let Some(ref msg) = parsed.message {
                final_role = msg.role.clone();
                if !msg.content.is_empty() {
                    accumulated_content.push_str(&msg.content);
                    app.emit("ollama-stream", StreamPayload {
                        content: msg.content.clone(),
                        done: false,
                    }).map_err(|e| e.to_string())?;
                }
                if let Some(calls) = &msg.tool_calls {
                    accumulated_tool_calls.extend(calls.iter().cloned());
                }
            }

            if parsed.done { break; }
        }
    } // end loop

    Ok(OllamaMessage {
        role: final_role,
        content: accumulated_content,
        tool_calls: if accumulated_tool_calls.is_empty() { None } else { Some(accumulated_tool_calls) },
        images: None,
    })
}

/// Maximum characters for a single tool result sent to the model.
/// Prevents huge outputs (e.g. long element lists) from overflowing context.
const MAX_TOOL_OUTPUT_CHARS: usize = 4000;

/// Keep only the last N messages from the base conversation history
/// so accumulated chat doesn't blow up the context window.
const MAX_BASE_HISTORY: usize = 12;

/// Keep at most this many tool-round messages (assistant tool_calls + tool results)
/// within one agent invocation.
const MAX_TOOL_HISTORY_MSGS: usize = 8; // 4 rounds × 2 messages

fn truncate_tool_output(tool_name: &str, output: String) -> String {
    // Never truncate data URLs — they are base64 images and must stay intact
    // so tool_message() can move them to the `images` field correctly.
    if output.starts_with("data:") { return output; }
    if output.len() <= MAX_TOOL_OUTPUT_CHARS { return output; }
    format!("{}…[truncated, {} chars total]", &output[..MAX_TOOL_OUTPUT_CHARS], output.len())
}

/// Tauri command — streams a chat completion from the local Ollama server with
/// full agentic tool-calling support.
#[tauri::command]
pub async fn chat_ollama(
    app: AppHandle,
    messages: Vec<OllamaMessage>,
    model: String,
    character: Option<crate::model::shared::CharacterOverride>,
) -> Result<(), String> {
    crate::model::CHAT_CANCEL.store(false, std::sync::atomic::Ordering::Relaxed);

    let tool_schemas = load_tool_schemas();
    bootstrap_memory(&app);

    let base_prompt = build_base_prompt(&app, character.as_ref()).await;

    // Trim base history to avoid carrying too much prior conversation.
    let base_messages: Vec<OllamaMessage> = {
        let msgs: Vec<_> = messages.into_iter().filter(|m| m.role != "system").collect();
        if msgs.len() > MAX_BASE_HISTORY {
            msgs[msgs.len() - MAX_BASE_HISTORY..].to_vec()
        } else {
            msgs
        }
    };

    // Tool-round messages accumulated during this agent invocation only.
    let mut tool_history: Vec<OllamaMessage> = Vec::new();
    let tool_context = local_tool_context(&app);

    show_overlay(&app);

    for round in 0..MAX_TOOL_ROUNDS {
        if should_cancel(&app) {
            hide_overlay(&app);
            app.emit("ollama-stream", StreamPayload {
                content: "\n\n[Cancelled by user]".to_string(),
                done: true,
            }).ok();
            return Ok(());
        }

        let system_content = prepare_system(&app, &base_prompt);
        let system_msg = OllamaMessage {
            role: "system".to_string(),
            content: system_content,
            tool_calls: None,
            images: None,
        };

        // Trim tool history to last MAX_TOOL_HISTORY_MSGS messages.
        let tool_slice = if tool_history.len() > MAX_TOOL_HISTORY_MSGS {
            &tool_history[tool_history.len() - MAX_TOOL_HISTORY_MSGS..]
        } else {
            &tool_history
        };

        // Combine base history + recent tool rounds into one slice for this request.
        let conversation: Vec<OllamaMessage> = base_messages.iter()
            .chain(tool_slice.iter())
            .cloned()
            .collect();

        let final_msg = match stream_once(&app, &system_msg, &conversation, tool_schemas, &model).await {
            Ok(msg) => msg,
            Err(e) if e == "CANCELLED" => {
                hide_overlay(&app);
                app.emit("ollama-stream", StreamPayload {
                    content: "\n\n[Cancelled by user]".to_string(),
                    done: true,
                }).ok();
                return Ok(());
            }
            Err(e) => {
                hide_overlay(&app);
                return Err(e);
            }
        };

        let tool_count = final_msg.tool_calls.as_ref().map_or(0, |v| v.len());
        app.emit("agent-status", AgentStatusPayload {
            message: format!("[round {round}] tool_calls={tool_count} content_len={}", final_msg.content.len()),
        }).ok();

        let mut final_msg = final_msg;
        let tool_calls = final_msg.tool_calls.take().unwrap_or_default();

        if tool_calls.is_empty() {
            hide_overlay(&app);
            app.emit("ollama-stream", StreamPayload { content: String::new(), done: true })
                .map_err(|e| e.to_string())?;
            return Ok(());
        }

        final_msg.tool_calls = Some(tool_calls.clone());
        tool_history.push(final_msg);

        for call in &tool_calls {
            if should_cancel(&app) {
                hide_overlay(&app);
                app.emit("ollama-stream", StreamPayload {
                    content: "\n\n[Cancelled by user]".to_string(),
                    done: true,
                }).ok();
                return Ok(());
            }

            let tool_name = &call.function.name;
            let tool_args = &call.function.arguments;

            app.emit("agent-status", AgentStatusPayload {
                message: format!("Running tool: {tool_name}"),
            }).ok();

            let output = execute_tool_with_context(&app, tool_name, tool_args, &tool_context)
                .await
                .output;

            tool_history.push(tool_message(truncate_tool_output(tool_name, output)));
        }
    }

    hide_overlay(&app);
    Err(format!("Agent exceeded maximum tool rounds ({MAX_TOOL_ROUNDS})"))
}
