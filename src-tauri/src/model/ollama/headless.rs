/// Headless Ollama agent loop — no Tauri event streaming, returns the final
/// response text.  Used by the bridge server for remote/Discord commands.
use futures_util::StreamExt;
use serde_json::Value;
use tauri::AppHandle;

use crate::memory::bootstrap_memory;
use crate::skills::load_tool_schemas;
use crate::tools::{execute_tool_with_context, ToolExecutionContext};
use crate::model::ollama::types::tool_message;
use crate::model::shared::{build_base_prompt, prepare_system, MAX_TOOL_ROUNDS};

use super::types::{OllamaChunk, OllamaMessage, OllamaRoundRequest, OllamaToolCall};

async fn stream_once_headless(
    app: &AppHandle,
    system_msg: &OllamaMessage,
    history: &[OllamaMessage],
    tool_schemas: &[Value],
    model: &str,
) -> Result<OllamaMessage, String> {
    let body = OllamaRoundRequest::new(model, system_msg, history, true, tool_schemas);

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
    let mut content = String::new();
    let mut tool_calls: Vec<OllamaToolCall> = Vec::new();
    let mut role = "assistant".to_string();

    while let Some(chunk_result) = byte_stream.next().await {
        let bytes = chunk_result.map_err(|e| format!("Stream error: {e}"))?;
        let text = String::from_utf8_lossy(&bytes);

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() { continue; }
            let Ok(parsed) = serde_json::from_str::<OllamaChunk>(line) else { continue };
            if let Some(ref msg) = parsed.message {
                role = msg.role.clone();
                content.push_str(&msg.content);
                if let Some(calls) = &msg.tool_calls {
                    tool_calls.extend(calls.iter().cloned());
                }
            }
            if parsed.done { break; }
        }
    }

    Ok(OllamaMessage {
        role,
        content,
        tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
        images: None,
    })
}

const MAX_TOOL_OUTPUT_CHARS: usize = 4000;
const MAX_BASE_HISTORY: usize = 12;
const MAX_TOOL_HISTORY_MSGS: usize = 8;

fn truncate_tool_output(tool_name: &str, output: String) -> String {
    // Never truncate data URLs — they are base64 images and must stay intact.
    if output.starts_with("data:") { return output; }
    let limit = if tool_name == "pc_ui_elements" { 10_000 } else { MAX_TOOL_OUTPUT_CHARS };
    if output.len() <= limit { return output; }
    format!("{}…[truncated, {} chars total]", &output[..limit], output.len())
}

/// Run a full agentic loop without emitting Tauri events.
/// Returns the final assistant response text.
pub async fn run_headless(
    app: &AppHandle,
    conversation: Vec<OllamaMessage>,
    model: &str,
    source_device_id: Option<String>,
    source_device_type: Option<String>,
) -> Result<String, String> {
    let tool_schemas = load_tool_schemas();
    bootstrap_memory(app);

    let base_prompt = build_base_prompt(app, None).await;
    let tool_context = ToolExecutionContext { source_device_id, source_device_type };

    let base_messages: Vec<OllamaMessage> = {
        let msgs: Vec<_> = conversation.into_iter().collect();
        if msgs.len() > MAX_BASE_HISTORY {
            msgs[msgs.len() - MAX_BASE_HISTORY..].to_vec()
        } else {
            msgs
        }
    };

    let mut tool_history: Vec<OllamaMessage> = Vec::new();
    let mut final_result: Result<String, String> =
        Err(format!("Agent exceeded maximum tool rounds ({MAX_TOOL_ROUNDS})"));

    'agent: for _ in 0..MAX_TOOL_ROUNDS {
        let system_content = prepare_system(app, &base_prompt);
        let system_msg = OllamaMessage {
            role: "system".to_string(),
            content: system_content,
            tool_calls: None,
            images: None,
        };

        let tool_slice = if tool_history.len() > MAX_TOOL_HISTORY_MSGS {
            &tool_history[tool_history.len() - MAX_TOOL_HISTORY_MSGS..]
        } else {
            &tool_history
        };

        let history: Vec<OllamaMessage> = base_messages.iter()
            .chain(tool_slice.iter())
            .cloned()
            .collect();

        let mut final_msg = match stream_once_headless(app, &system_msg, &history, tool_schemas, model).await {
            Ok(msg) => msg,
            Err(e) => {
                final_result = Err(e);
                break 'agent;
            }
        };

        let tool_calls = final_msg.tool_calls.take().unwrap_or_default();

        if tool_calls.is_empty() {
            final_result = Ok(final_msg.content);
            break 'agent;
        }

        final_msg.tool_calls = Some(tool_calls.clone());
        tool_history.push(final_msg);

        for call in &tool_calls {
            let tool_name = &call.function.name;
            let tool_args = &call.function.arguments;
            let output = execute_tool_with_context(app, tool_name, tool_args, &tool_context)
                .await
                .output;

            tool_history.push(tool_message(truncate_tool_output(tool_name, output)));
        }
    }

    final_result
}
