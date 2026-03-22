use futures_util::StreamExt;
use serde_json::Value;
use tauri::{AppHandle, Emitter};

use crate::memory::bootstrap_memory;
use crate::phone::{hide_overlay, show_overlay};
use crate::skills::load_tool_schemas;
use crate::tools::{execute_tool_with_context, ToolExecutionContext};
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

    while let Some(chunk_result) = byte_stream.next().await {
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
    }

    Ok(OllamaMessage {
        role: final_role,
        content: accumulated_content,
        tool_calls: if accumulated_tool_calls.is_empty() { None } else { Some(accumulated_tool_calls) },
    })
}

/// Tauri command — streams a chat completion from the local Ollama server with
/// full agentic tool-calling support.
#[tauri::command]
pub async fn chat_ollama(
    app: AppHandle,
    messages: Vec<OllamaMessage>,
    model: String,
) -> Result<(), String> {
    crate::model::CHAT_CANCEL.store(false, std::sync::atomic::Ordering::Relaxed);

    let tool_schemas = load_tool_schemas();
    bootstrap_memory(&app);

    let base_prompt = build_base_prompt(&app).await;
    let mut conversation: Vec<OllamaMessage> = messages;
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
        };

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
        conversation.push(final_msg);

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

            conversation.push(OllamaMessage {
                role: "tool".to_string(),
                content: output,
                tool_calls: None,
            });
        }
    }

    hide_overlay(&app);
    Err(format!("Agent exceeded maximum tool rounds ({MAX_TOOL_ROUNDS})"))
}
