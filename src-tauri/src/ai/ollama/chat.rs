use futures_util::StreamExt;
use tokio::time::{timeout, Duration};
use serde_json::Value;
use tauri::AppHandle;

use crate::ai::emit::{emit_status, emit_stream, emit_stream_quiet};
use crate::chat::bootstrap_memory;
use crate::device::phone::{hide_overlay, show_overlay};
use crate::skills::load_tool_schemas;
use crate::tools::{execute_tool_with_context, ToolExecutionContext};
use crate::ai::ollama::types::tool_message;
use crate::ai::prompt::{build_base_prompt, prepare_system, should_cancel, wait_until_cancelled};
use crate::ai::types::MAX_AGENT_LOOPS;
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
    chat_id: &str,
    system_msg: &OllamaMessage,
    conversation: &[OllamaMessage],
    tool_schemas: &[Value],
    model: &str,
) -> Result<OllamaMessage, String> {
    let body = OllamaRoundRequest::new(model, system_msg, conversation, true, tool_schemas);

    let url = super::types::ollama_chat_url(app);
    eprintln!("[chat] POST {url} model={model} msgs={} tools={}", conversation.len(), tool_schemas.len());

    let response = super::ollama_client()
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Failed to reach Ollama: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        eprintln!("[chat] HTTP {status}: {text}");
        return Err(format!("Ollama returned {status}: {text}"));
    }

    let mut byte_stream = response.bytes_stream();
    let mut accumulated_content = String::new();
    let mut accumulated_tool_calls: Vec<OllamaToolCall> = Vec::new();
    let mut final_role = "assistant".to_string();
    let mut emitted_up_to: usize = 0;
    // Cross-chunk line buffer — TCP chunks can split an NDJSON line at any byte.
    // Without this, partial lines fail JSON parse and the chunk is silently dropped.
    let mut line_buf = String::new();
    const MEMORY_MARKER: &str = "---MEMORY";

    // 120 s idle timeout per chunk — prevents indefinite hang when a model stalls mid-stream.
    const CHUNK_IDLE_SECS: u64 = 120;

    loop {
        // Race the chunk wait against the cancel future so a tap on the phone
        // overlay aborts the stream immediately rather than after the next
        // Ollama chunk lands.
        let chunk_result = tokio::select! {
            next = timeout(Duration::from_secs(CHUNK_IDLE_SECS), byte_stream.next()) => {
                match next {
                    Ok(Some(r)) => r,
                    Ok(None) => break,
                    Err(_) => return Err(format!("Ollama stream timed out after {CHUNK_IDLE_SECS}s — model may be too slow or stalled")),
                }
            }
            _ = wait_until_cancelled(app.clone()) => {
                return Err("CANCELLED".to_string());
            }
        };

        if should_cancel(app) {
            return Err("CANCELLED".to_string());
        }
        let bytes = chunk_result.map_err(|e| format!("Stream error: {e}"))?;
        line_buf.push_str(&String::from_utf8_lossy(&bytes));

        // Drain only complete lines; keep any trailing partial line in the buffer.
        let mut start = 0;
        let mut complete_lines: Vec<String> = Vec::new();
        while let Some(nl) = line_buf[start..].find('\n') {
            let end = start + nl;
            complete_lines.push(line_buf[start..end].to_string());
            start = end + 1;
        }
        if start > 0 { line_buf.drain(..start); }

        for line in &complete_lines {
            let line = line.trim();
            if line.is_empty() { continue; }

            let Ok(parsed) = serde_json::from_str::<OllamaChunk>(line) else { continue };

            if let Some(ref msg) = parsed.message {
                final_role = msg.role.clone();
                if !msg.content.is_empty() {
                    accumulated_content.push_str(&msg.content);
                    let mut safe_end = accumulated_content.find(MEMORY_MARKER)
                        .unwrap_or_else(|| accumulated_content.len().saturating_sub(MEMORY_MARKER.len()));
                    while safe_end > 0 && !accumulated_content.is_char_boundary(safe_end) {
                        safe_end -= 1;
                    }
                    if safe_end > emitted_up_to {
                        let to_emit = accumulated_content[emitted_up_to..safe_end].to_string();
                        emit_stream(app, chat_id, to_emit, false, None)?;
                        emitted_up_to = safe_end;
                    }
                }
                if let Some(calls) = &msg.tool_calls {
                    accumulated_tool_calls.extend(calls.iter().cloned());
                }
            }

            if parsed.done { break; }
        }
    } // end loop

    eprintln!("[chat] stream end content_len={} tool_calls={} buf_tail={}",
        accumulated_content.len(), accumulated_tool_calls.len(), line_buf.len());

    // Flush any trailing partial line that wasn't newline-terminated.
    // Some servers omit the final '\n'; without this, the last chunk is lost.
    {
        let tail = line_buf.trim();
        if !tail.is_empty() {
            if let Ok(parsed) = serde_json::from_str::<OllamaChunk>(tail) {
                if let Some(ref msg) = parsed.message {
                    final_role = msg.role.clone();
                    if !msg.content.is_empty() {
                        accumulated_content.push_str(&msg.content);
                    }
                    if let Some(calls) = &msg.tool_calls {
                        accumulated_tool_calls.extend(calls.iter().cloned());
                    }
                }
            }
        }
    }

    // Flush any tail withheld during streaming (last MEMORY_MARKER.len() chars held back).
    let visible_end = accumulated_content.find(MEMORY_MARKER).unwrap_or(accumulated_content.len());
    if visible_end > emitted_up_to {
        emit_stream(
            app,
            chat_id,
            accumulated_content[emitted_up_to..visible_end].to_string(),
            false,
            None,
        )?;
    }

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
///
/// `chat_id` is the active chat's ID — broadcast with every stream chunk so peer
/// devices can route the chunk into the matching conversation in their UI.
#[tauri::command]
pub async fn chat_ollama(
    app: AppHandle,
    chat_id: String,
    messages: Vec<OllamaMessage>,
    model: String,
    character: Option<crate::ai::types::CharacterOverride>,
) -> Result<(), String> {
    crate::ai::CHAT_CANCEL.store(false, std::sync::atomic::Ordering::Relaxed);

    let tool_schemas = load_tool_schemas(&app);
    bootstrap_memory(&app);

    // ── Tiered text history compression ──────────────────────────────────────
    let msgs_no_sys: Vec<_> = messages.into_iter().filter(|m| m.role != "system").collect();

    let base_prompt = build_base_prompt(&app, character.as_ref()).await;
    let compress_entries: Vec<CompressMsg> = msgs_no_sys
        .iter()
        .map(|m| CompressMsg {
            role: m.role.clone(),
            content: m.content.clone(),
            brief: m.brief.clone(),
        })
        .collect();
    let compressed = compress_text_history(&compress_entries);
    let mut base_messages = msgs_no_sys[compressed.keep_from..].to_vec();

    // ── RAG injection (character chats only) ─────────────────────────────────
    if let Some(ref char) = character {
        if let Some(ref char_id) = char.id {
            if let Some(last_user) = base_messages.iter_mut().rev().find(|m| m.role == "user") {
                let cfg = crate::social::config::load_config(&app);
                let keywords = crate::social::post::generate::extract_rag_keywords(
                    &app, &model, &last_user.content,
                ).await;
                if !keywords.is_empty() {
                    let rag_block = crate::social::post::rag::search(
                        &app, &keywords, cfg.rag_max_results as usize,
                    );
                    if !rag_block.is_empty() {
                        last_user.content = format!("{rag_block}\n\n{}", last_user.content);
                    }
                }
                let _ = char_id; // suppress unused warning if rag skipped
            }
        }
    }

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
            emit_stream_quiet(&app, &chat_id, "\n\n[Cancelled by user]".to_string(), true, None);
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

        let final_msg = match stream_once(&app, &chat_id, &system_msg, &conversation, &tool_schemas, &model).await {
            Ok(msg) => msg,
            Err(e) if e == "CANCELLED" => {
                hide_overlay(&app);
                emit_stream_quiet(&app, &chat_id, "\n\n[Cancelled by user]".to_string(), true, None);
                return Ok(());
            }
            Err(e) => {
                hide_overlay(&app);
                emit_stream_quiet(&app, &chat_id, format!("\n\n[Error: {e}]"), true, None);
                return Err(e);
            }
        };

        let tool_count = final_msg.tool_calls.as_ref().map_or(0, |v| v.len());
        emit_status(
            &app,
            &chat_id,
            format!("[round {round}] tool_calls={tool_count} content_len={}", final_msg.content.len()),
        );

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

            emit_stream(&app, &chat_id, String::new(), true, brief)?;
            return Ok(());
        }

        final_msg.tool_calls = Some(tool_calls.clone());
        tool_history.push(final_msg);

        for call in &tool_calls {
            if should_cancel(&app) {
                hide_overlay(&app);
                emit_stream_quiet(&app, &chat_id, "\n\n[Cancelled by user]".to_string(), true, None);
                return Ok(());
            }

            let tool_name = &call.function.name;
            let tool_args = &call.function.arguments;

            emit_status(&app, &chat_id, format!("Running tool: {tool_name}"));

            eprintln!("[chat] tool start: {tool_name}");
            let tool_fut = execute_tool_with_context(&app, tool_name, tool_args, &tool_context);
            let output = tokio::select! {
                r = tool_fut => r.output,
                _ = wait_until_cancelled(app.clone()) => {
                    hide_overlay(&app);
                    emit_stream_quiet(&app, &chat_id, "\n\n[Cancelled by user]".to_string(), true, None);
                    return Ok(());
                }
            };
            eprintln!("[chat] tool end:   {tool_name} output_len={}", output.len());

            tool_history.push(tool_message(truncate_tool_output(tool_name, output)));
        }
    }

    hide_overlay(&app);
    let msg = format!("Agent exceeded maximum tool rounds ({MAX_AGENT_LOOPS})");
    emit_stream_quiet(&app, &chat_id, format!("\n\n[Error: {msg}]"), true, None);
    Err(msg)
}
