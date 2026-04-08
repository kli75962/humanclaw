use std::collections::HashMap;
use futures_util::StreamExt;
use serde_json::Value;
use tauri::{AppHandle, Emitter};

use crate::chat::bootstrap_memory;
use crate::device::phone::{hide_overlay, show_overlay};
use crate::skills::load_tool_schemas;
use crate::tools::{execute_tool_with_context, ToolExecutionContext};
use crate::ai::prompt::{build_base_prompt, prepare_system, should_cancel};
use crate::ai::types::{AgentStatusPayload, StreamPayload, MAX_AGENT_LOOPS};
use crate::ai::history::{
    compress_text_history, extract_brief, memory_instruction,
    trim_tool_start_index, CompressMsg,
};

use super::types::{
    openai_tool_to_claude, ClaudeBlock, ClaudeContent, ClaudeMessage, ClaudeRequest,
    ClaudeRoundResult, ClaudeToolCall, ContentBlockDelta, ContentBlockStartData,
    InFlightBlock, SseEvent,
};
use super::InputMessage;

const CLAUDE_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const MAX_TOKENS: u32 = 8096;

fn load_api_key() -> Result<String, String> {
    #[cfg(not(target_os = "android"))]
    {
        let entry = keyring::Entry::new("phoneclaw", "claude_api_key")
            .map_err(|e| e.to_string())?;
        match entry.get_password() {
            Ok(key) if !key.is_empty() => Ok(key),
            Ok(_) => Err("Claude API key is empty. Set it in Settings → Model.".to_string()),
            Err(keyring::Error::NoEntry) => {
                Err("Claude API key not configured. Set it in Settings → Model.".to_string())
            }
            Err(e) => Err(e.to_string()),
        }
    }
    #[cfg(target_os = "android")]
    {
        Err("Claude API is not supported on Android.".to_string())
    }
}

fn local_tool_context(app: &AppHandle) -> ToolExecutionContext {
    let cfg = crate::session::store::bootstrap(app);
    ToolExecutionContext {
        source_device_id: Some(cfg.device.device_id),
        source_device_type: Some(cfg.device.device_type.label().to_string()),
    }
}

/// Execute one streaming round against the Claude Messages API.
/// Emits content tokens to the frontend and returns the assembled result.
async fn stream_once(
    app: &AppHandle,
    api_key: &str,
    system: &str,
    history: &[ClaudeMessage],
    tool_schemas: &[Value],
    model: &str,
) -> Result<ClaudeRoundResult, String> {
    let claude_tools: Vec<Value> = tool_schemas
        .iter()
        .filter_map(openai_tool_to_claude)
        .collect();

    let body = ClaudeRequest {
        model,
        max_tokens: MAX_TOKENS,
        system,
        messages: history,
        tools: &claude_tools,
        stream: true,
    };

    let response = super::claude_client()
        .post(CLAUDE_API_URL)
        .header("x-api-key", api_key)
        .header("anthropic-version", ANTHROPIC_VERSION)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Failed to reach Claude API: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Claude API returned {status}: {text}"));
    }

    let mut byte_stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut blocks: HashMap<usize, InFlightBlock> = HashMap::new();
    let mut stop_reason = String::new();

    while let Some(chunk_result) = byte_stream.next().await {
        if should_cancel(app) {
            return Err("CANCELLED".to_string());
        }

        let bytes = chunk_result.map_err(|e| format!("Stream error: {e}"))?;
        buffer.push_str(&String::from_utf8_lossy(&bytes));

        while let Some(end) = buffer.find("\n\n") {
            let message = buffer[..end].to_string();
            buffer = buffer[end + 2..].to_string();

            let mut event_type = String::new();
            let mut data_line = String::new();

            for line in message.lines() {
                if let Some(v) = line.strip_prefix("event: ") {
                    event_type = v.trim().to_string();
                } else if let Some(v) = line.strip_prefix("data: ") {
                    data_line = v.trim().to_string();
                }
            }

            if data_line.is_empty() || event_type == "ping" { continue; }

            let Ok(event) = serde_json::from_str::<SseEvent>(&data_line) else { continue };

            match event {
                SseEvent::ContentBlockStart { index, content_block } => {
                    let block = match content_block {
                        ContentBlockStartData::Text { .. } => {
                            InFlightBlock::Text { text: String::new(), emitted: 0 }
                        }
                        ContentBlockStartData::ToolUse { id, name } => {
                            InFlightBlock::ToolUse { id, name, input_json: String::new() }
                        }
                    };
                    blocks.insert(index, block);
                }

                SseEvent::ContentBlockDelta { index, delta } => {
                    match delta {
                        ContentBlockDelta::TextDelta { text } => {
                            if let Some(InFlightBlock::Text { text: acc, emitted }) = blocks.get_mut(&index) {
                                acc.push_str(&text);
                                const MEMORY_MARKER: &str = "---MEMORY";
                                let mut safe_end = acc.find(MEMORY_MARKER)
                                    .unwrap_or_else(|| acc.len().saturating_sub(MEMORY_MARKER.len()));
                                while safe_end > 0 && !acc.is_char_boundary(safe_end) {
                                    safe_end -= 1;
                                }
                                if safe_end > *emitted {
                                    let to_emit = acc[*emitted..safe_end].to_string();
                                    app.emit("ollama-stream", StreamPayload {
                                        content: to_emit,
                                        done: false,
                                        brief: None,
                                    }).map_err(|e| e.to_string())?;
                                    *emitted = safe_end;
                                }
                            }
                        }
                        ContentBlockDelta::InputJsonDelta { partial_json } => {
                            if let Some(InFlightBlock::ToolUse { input_json, .. }) = blocks.get_mut(&index) {
                                input_json.push_str(&partial_json);
                            }
                        }
                    }
                }

                SseEvent::MessageDelta { delta } => {
                    if let Some(reason) = delta.stop_reason {
                        stop_reason = reason;
                    }
                }

                SseEvent::Error { error } => {
                    return Err(format!("Claude API error: {}", error.message));
                }

                _ => {}
            }
        }
    }

    // Assemble final result from completed blocks
    let mut text = String::new();
    let mut tool_calls: Vec<ClaudeToolCall> = Vec::new();

    let mut ordered: Vec<(usize, InFlightBlock)> = blocks.into_iter().collect();
    ordered.sort_by_key(|(i, _)| *i);

    for (_, block) in ordered {
        match block {
            InFlightBlock::Text { text: t, .. } => {
                text = t;
            }
            InFlightBlock::ToolUse { id, name, input_json } => {
                let input = serde_json::from_str::<Value>(&input_json)
                    .unwrap_or(serde_json::json!({}));
                tool_calls.push(ClaudeToolCall { id, name, input });
            }
        }
    }

    Ok(ClaudeRoundResult { text, tool_calls, stop_reason })
}

/// Tauri command — streams a chat completion from the Claude API with
/// full agentic tool-calling support.
#[tauri::command]
pub async fn chat_claude(
    app: AppHandle,
    messages: Vec<InputMessage>,
    model: String,
    character: Option<crate::ai::types::CharacterOverride>,
) -> Result<(), String> {
    crate::ai::CHAT_CANCEL.store(false, std::sync::atomic::Ordering::Relaxed);

    let api_key = load_api_key()?;
    let tool_schemas = load_tool_schemas(&app);
    bootstrap_memory(&app);

    let base_prompt = build_base_prompt(&app, character.as_ref()).await;
    let tool_context = local_tool_context(&app);

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

    // Convert kept messages to Claude format
    let base_messages: Vec<ClaudeMessage> = msgs_no_sys[compressed.keep_from..]
        .iter()
        .map(|m| ClaudeMessage {
            role: m.role.clone(),
            content: ClaudeContent::Text(m.content.clone()),
        })
        .collect();

    // ── Round-0 prompt ───────────────────────────────────────────────────────
    let base_prompt_round0 = {
        let mut p = base_prompt.clone();

        if let Some(ref summary) = compressed.older_summary {
            p.push_str("\n\n");
            p.push_str(summary);
        }

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

        let has_character_id = character.as_ref().and_then(|c| c.id.as_ref()).is_some();
        p.push_str("\n\n");
        p.push_str(&memory_instruction(has_character_id));

        p
    };

    // Tool-round messages accumulated during this invocation only.
    let mut tool_history: Vec<ClaudeMessage> = Vec::new();

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
        let system = prepare_system(&app, effective_base);

        // Trim tool history to last MAX_TOOL_ROUNDS_KEPT rounds
        let tool_start = trim_tool_start_index(&tool_history, |m| m.role == "assistant");
        let tool_slice = &tool_history[tool_start..];

        let conversation: Vec<ClaudeMessage> = base_messages.iter()
            .chain(tool_slice.iter())
            .cloned()
            .collect();

        let result = match stream_once(&app, &api_key, &system, &conversation, &tool_schemas, &model).await {
            Ok(r) => r,
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

        app.emit("agent-status", AgentStatusPayload {
            message: format!(
                "[round {round}] tool_calls={} content_len={}",
                result.tool_calls.len(),
                result.text.len()
            ),
        }).ok();

        if result.tool_calls.is_empty() {
            hide_overlay(&app);

            let brief = extract_brief(&result.text);

            app.emit("ollama-stream", StreamPayload { content: String::new(), done: true, brief })
                .map_err(|e| e.to_string())?;
            return Ok(());
        }

        // Build assistant message with text + tool_use blocks for tool_history
        let mut assistant_blocks: Vec<ClaudeBlock> = Vec::new();
        if !result.text.is_empty() {
            assistant_blocks.push(ClaudeBlock::Text { text: result.text.clone() });
        }
        for call in &result.tool_calls {
            assistant_blocks.push(ClaudeBlock::ToolUse {
                id: call.id.clone(),
                name: call.name.clone(),
                input: call.input.clone(),
            });
        }
        tool_history.push(ClaudeMessage {
            role: "assistant".to_string(),
            content: ClaudeContent::Blocks(assistant_blocks),
        });

        // Execute tools and collect results as a user message with tool_result blocks
        let mut result_blocks: Vec<ClaudeBlock> = Vec::new();
        for call in &result.tool_calls {
            if should_cancel(&app) {
                hide_overlay(&app);
                app.emit("ollama-stream", StreamPayload {
                    content: "\n\n[Cancelled by user]".to_string(),
                    done: true,
                    brief: None,
                }).ok();
                return Ok(());
            }

            app.emit("agent-status", AgentStatusPayload {
                message: format!("Running tool: {}", call.name),
            }).ok();

            let output = execute_tool_with_context(&app, &call.name, &call.input, &tool_context)
                .await
                .output;

            result_blocks.push(ClaudeBlock::ToolResult {
                tool_use_id: call.id.clone(),
                content: output,
            });
        }

        tool_history.push(ClaudeMessage {
            role: "user".to_string(),
            content: ClaudeContent::Blocks(result_blocks),
        });
    }

    hide_overlay(&app);
    Err(format!("Agent exceeded maximum tool rounds ({MAX_AGENT_LOOPS})"))
}
