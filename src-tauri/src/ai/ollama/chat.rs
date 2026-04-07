use futures_util::StreamExt;
use tokio::time::{timeout, Duration};
use serde_json::Value;
use tauri::{AppHandle, Emitter};

use crate::chat::bootstrap_memory;
use crate::device::phone::{hide_overlay, show_overlay};
use crate::skills::load_tool_schemas;
use crate::tools::{execute_tool_with_context, ToolExecutionContext};
use crate::ai::ollama::types::tool_message;
use crate::ai::prompt::{build_base_prompt, prepare_system, should_cancel};
use crate::ai::types::{AgentStatusPayload, StreamPayload, MAX_AGENT_LOOPS};
use crate::ai::history::{
    compress_text_history, extract_brief, memory_instruction,
    trim_tool_start_index, CompressMsg,
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
                    if !accumulated_content.contains("---MEMORY---") {
                        app.emit("ollama-stream", StreamPayload {
                            content: msg.content.clone(),
                            done: false,
                            brief: None,
                        }).map_err(|e| e.to_string())?;
                    }
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
        brief: None,
    })
}

/// Maximum characters for a single tool result sent to the model.
const MAX_TOOL_OUTPUT_CHARS: usize = 4000;

fn truncate_tool_output(_tool_name: &str, output: String) -> String {
    // Never truncate data URLs — they are base64 images and must stay intact.
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
    character: Option<crate::ai::types::CharacterOverride>,
) -> Result<(), String> {
    crate::ai::CHAT_CANCEL.store(false, std::sync::atomic::Ordering::Relaxed);

    let tool_schemas = load_tool_schemas(&app);
    bootstrap_memory(&app);

    // Base prompt without memory (reused for rounds after the first).
    let base_prompt = build_base_prompt(&app, character.as_ref()).await;

    // ── Tiered text history compression ──────────────────────────────────────
    let msgs_no_sys: Vec<_> = messages.into_iter().filter(|m| m.role != "system").collect();
    let compress_entries: Vec<CompressMsg> = msgs_no_sys
        .iter()
        .map(|m| CompressMsg {
            role: m.role.clone(),
            content: m.content.clone(),
            brief: m.brief.clone(),
        })
        .collect();
    let compressed = compress_text_history(&compress_entries);
    let base_messages = msgs_no_sys[compressed.keep_from..].to_vec();

    // ── Round-0 prompt ───────────────────────────────────────────────────────
    let base_prompt_round0 = {
        let mut p = base_prompt.clone();

        // Older conversation summary
        if let Some(ref summary) = compressed.older_summary {
            p.push_str("\n\n");
            p.push_str(summary);
        }

        // Character memory context (post + comment + conversation)
        if let Some(ref char) = character {
            if let Some(ref char_id) = char.id {
                use crate::social::character::memory::build_memory_context;
                let mem = build_memory_context(&app, char_id);
                if !mem.is_empty() {
                    p.push_str("\n\n");
                    p.push_str(&mem);
                }
            }
        }

        // Memory instruction — always enabled (brief only for normal, brief+importance for character)
        let has_character_id = character.as_ref().and_then(|c| c.id.as_ref()).is_some();
        p.push_str("\n\n");
        p.push_str(&memory_instruction(has_character_id));

        p
    };

    // Tool-round messages accumulated during this agent invocation only.
    let mut tool_history: Vec<OllamaMessage> = Vec::new();
    let tool_context = local_tool_context(&app);

    show_overlay(&app);

    for round in 0..MAX_AGENT_LOOPS {
        if should_cancel(&app) {
            hide_overlay(&app);
            app.emit("ollama-stream", StreamPayload {
                content: "\n\n[Cancelled by user]".to_string(),
                done: true,
                brief: None,
            }).ok();
            return Ok(());
        }

        let effective_base = if round == 0 { &base_prompt_round0 } else { &base_prompt };
        let system_content = prepare_system(&app, effective_base);
        let system_msg = OllamaMessage {
            role: "system".to_string(),
            content: system_content,
            tool_calls: None,
            images: None,
            brief: None,
        };

        // Trim tool history to last MAX_TOOL_ROUNDS_KEPT rounds.
        let tool_start = trim_tool_start_index(&tool_history, |m| {
            m.role == "assistant" && m.tool_calls.is_some()
        });
        let tool_slice = &tool_history[tool_start..];

        // Combine base history + recent tool rounds into one slice for this request.
        let conversation: Vec<OllamaMessage> = base_messages.iter()
            .chain(tool_slice.iter())
            .cloned()
            .collect();

        let final_msg = match stream_once(&app, &system_msg, &conversation, &tool_schemas, &model).await {
            Ok(msg) => msg,
            Err(e) if e == "CANCELLED" => {
                hide_overlay(&app);
                app.emit("ollama-stream", StreamPayload {
                    content: "\n\n[Cancelled by user]".to_string(),
                    done: true,
                    brief: None,
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

            // Extract brief from the response
            let brief = extract_brief(&final_msg.content);

            // Save conversation memory if character mode with id is active
            if let Some(ref char_override) = character {
                if let Some(ref char_id) = char_override.id {
                    let (_, mem) = crate::social::post::generate::parse_comment_output_full(&final_msg.content);
                    if let Some(m) = mem {
                        use crate::social::character::memory::{add_entry, current_ts, MemoryEntry, MemoryEntryType};
                        let entry = MemoryEntry {
                            id: format!("{:x}", current_ts()),
                            character_id: char_id.clone(),
                            entry_type: MemoryEntryType::Conversation,
                            brief: m.brief,
                            importance: m.importance,
                            created_at: current_ts(),
                        };
                        let _ = add_entry(&app, entry);
                    }
                }
            }

            app.emit("ollama-stream", StreamPayload { content: String::new(), done: true, brief })
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
                    brief: None,
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
    Err(format!("Agent exceeded maximum tool rounds ({MAX_AGENT_LOOPS})"))
}
