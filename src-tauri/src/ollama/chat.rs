use futures_util::StreamExt;
use serde_json::Value;
use tauri::{AppHandle, Emitter};

use crate::memory::{
    add_knowledge, add_memories, build_knowledge_prompt, build_memory_prompt,
    extract_knowledge, extract_memories, load_knowledge, load_memories,
};
use crate::phone::{execute_tool, get_installed_apps, hide_overlay, is_cancelled, show_overlay};
use crate::skills::{build_skills_prompt, load_tool_schemas};

use super::types::{
    AgentStatusPayload, OllamaChatRequest, OllamaChunk, OllamaMessage, OllamaToolCall,
    StreamPayload,
};

const MAX_TOOL_ROUNDS: usize = 200;

/// Build the system prompt by combining general guidelines, skill definitions,
/// navigation knowledge, user memories, and the list of installed apps.
async fn build_system_prompt(
    app: &AppHandle,
    knowledge_block: &str,
    memory_block: &str,
) -> String {
    let apps = get_installed_apps(app).await;
    let apps_list = if apps.is_empty() {
        "  (no apps found)".to_string()
    } else {
        apps.iter()
            .map(|a| a.prompt_line())
            .collect::<Vec<_>>()
            .join("\n")
    };

    let skills_text = build_skills_prompt();

    // Build sections, only including non-empty blocks
    let mut sections = vec![skills_text];
    if !knowledge_block.is_empty() {
        sections.push(knowledge_block.to_string());
    }
    if !memory_block.is_empty() {
        sections.push(memory_block.to_string());
    }
    sections.push(format!("[INSTALLED APPS]\n{apps_list}"));
    sections.join("\n\n")
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
    let client = reqwest::Client::new();

    let body = OllamaChatRequest {
        model: model.to_string(),
        messages: messages.to_vec(),
        stream: true,
        tools: tool_schemas.to_vec(),
    };

    let response = client
        .post("http://10.0.2.2:11434/api/chat")
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

    // Load personal preferences and general navigation knowledge
    let memories  = load_memories(&app);
    let knowledge = load_knowledge(&app);
    let memory_block    = build_memory_prompt(&memories);
    let knowledge_block = build_knowledge_prompt(&knowledge);
    let system_prompt = build_system_prompt(&app, &knowledge_block, &memory_block).await;

    // Extract the user's latest message text for extraction later
    let user_last_msg = messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.clone())
        .unwrap_or_default();

    // Compact tool call log used to extract navigation knowledge after the session
    let mut tool_log_lines: Vec<String> = Vec::new();

    // Prepend system message with skills + apps context
    let system_msg = OllamaMessage {
        role: "system".to_string(),
        content: system_prompt,
        tool_calls: None,
    };
    let mut conversation: Vec<OllamaMessage> = std::iter::once(system_msg)
        .chain(messages.into_iter())
        .collect();

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
        let final_msg = match stream_once(&app, &conversation, &tool_schemas, &model).await {
            Ok(msg) => msg,
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

        // Check if the LLM requested tool calls
        let tool_calls = match &final_msg.tool_calls {
            Some(calls) if !calls.is_empty() => calls.clone(),
            _ => {
                // No tool calls — conversation is done
                hide_overlay(&app);

                // Extract new user preferences and navigation knowledge in background
                let extract_model = model.clone();
                let extract_user  = user_last_msg.clone();
                let extract_reply = final_msg.content.clone();
                let extract_app   = app.clone();
                let tool_log_str  = tool_log_lines.join("\n");
                tauri::async_runtime::spawn(async move {
                    // Personal preference memory
                    let new_memories =
                        extract_memories(&extract_model, &extract_user, &extract_reply).await;
                    add_memories(&extract_app, new_memories);

                    // General navigation knowledge (only if tools were actually used)
                    if !tool_log_str.is_empty() {
                        let new_knowledge =
                            extract_knowledge(&extract_model, &extract_user, &tool_log_str).await;
                        add_knowledge(&extract_app, new_knowledge);
                    }
                });

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
        };

        // Append assistant message with tool_calls to history
        conversation.push(final_msg);

        // Execute each tool and collect results
        for call in &tool_calls {
            let tool_name = &call.function.name;
            let tool_args = &call.function.arguments;

            // Notify frontend that a tool is running
            app.emit("agent-status", AgentStatusPayload {
                message: format!("Running tool: {tool_name}"),
            })
            .ok();

            let result = execute_tool(&app, tool_name, tool_args).await;

            // Only record successful tool calls for navigation knowledge extraction
            if result.success {
                tool_log_lines.push(format!(
                    "{}({}) → {}",
                    tool_name,
                    tool_args,
                    result.output.trim(),
                ));
            }

            // Append tool result as a "tool" role message
            conversation.push(OllamaMessage {
                role: "tool".to_string(),
                content: result.output,
                tool_calls: None,
            });
        }
        // Loop back to let the LLM process tool results
    }

    hide_overlay(&app);
    Err(format!("Agent exceeded maximum tool rounds ({MAX_TOOL_ROUNDS})"))
}
