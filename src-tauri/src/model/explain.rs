use futures_util::StreamExt;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::model::claude::InputMessage;
use crate::model::ollama::types::{OllamaChunk, OllamaMessage, OllamaRoundRequest};
use crate::model::prompt::{build_base_prompt, prepare_system};

#[derive(Serialize, Clone)]
pub struct ExplainPayload {
    pub content: String,
    pub done: bool,
}

/// Simple chat without tool-calling, for explain and memo follow-up.
/// Uses the persona system prompt. Emits `explain-stream` events.
#[tauri::command]
pub async fn explain_text(
    app: AppHandle,
    messages: Vec<InputMessage>,
    model: String,
) -> Result<(), String> {
    if model.starts_with("claude-") {
        explain_claude(&app, &messages, &model).await
    } else {
        explain_ollama(&app, &messages, &model).await
    }
}

async fn explain_ollama(app: &AppHandle, messages: &[InputMessage], model: &str) -> Result<(), String> {
    let base = build_base_prompt(app, None).await;
    let system_content = prepare_system(app, &base);

    let system_msg = OllamaMessage {
        role: "system".to_string(),
        content: system_content,
        tool_calls: None,
        images: None,
        brief: None,
    };

    let ollama_msgs: Vec<OllamaMessage> = messages.iter()
        .filter(|m| m.role != "system")
        .map(|m| OllamaMessage { role: m.role.clone(), content: m.content.clone(), tool_calls: None, images: None, brief: None })
        .collect();

    let body = OllamaRoundRequest::new(model, &system_msg, &ollama_msgs, true, &[]);

    let response = crate::model::ollama::ollama_client()
        .post(crate::model::ollama::types::ollama_chat_url(app))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Failed to reach Ollama: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Ollama returned {status}: {text}"));
    }

    let mut stream = response.bytes_stream();
    while let Some(result) = stream.next().await {
        let bytes = result.map_err(|e| format!("Stream error: {e}"))?;
        let text = String::from_utf8_lossy(&bytes);
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() { continue; }
            let Ok(parsed) = serde_json::from_str::<OllamaChunk>(line) else { continue };
            if let Some(ref msg) = parsed.message {
                if !msg.content.is_empty() {
                    app.emit("explain-stream", ExplainPayload { content: msg.content.clone(), done: false })
                        .map_err(|e| e.to_string())?;
                }
            }
            if parsed.done { break; }
        }
    }

    app.emit("explain-stream", ExplainPayload { content: String::new(), done: true })
        .map_err(|e| e.to_string())
}

#[allow(unused_variables)]
async fn explain_claude(app: &AppHandle, messages: &[InputMessage], model: &str) -> Result<(), String> {
    #[cfg(target_os = "android")]
    return Err("Claude API is not supported on Android.".to_string());

    #[cfg(not(target_os = "android"))]
    {
        let api_key = {
            let entry = keyring::Entry::new("phoneclaw", "claude_api_key")
                .map_err(|e| e.to_string())?;
            match entry.get_password() {
                Ok(key) if !key.is_empty() => Ok(key),
                Ok(_) => Err("Claude API key is empty.".to_string()),
                Err(keyring::Error::NoEntry) => Err("Claude API key not configured.".to_string()),
                Err(e) => Err(e.to_string()),
            }
        }?;

        let base = build_base_prompt(app, None).await;
        let system_content = prepare_system(app, &base);

        let history: Vec<_> = messages.iter()
            .filter(|m| m.role != "system")
            .map(|m| serde_json::json!({ "role": m.role, "content": m.content }))
            .collect();

        let body = serde_json::json!({
            "model": model,
            "max_tokens": 2048,
            "system": system_content,
            "messages": history,
            "stream": true
        });

        let response = crate::model::claude::claude_client()
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
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

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();

        while let Some(result) = stream.next().await {
            let bytes = result.map_err(|e| format!("Stream error: {e}"))?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(end) = buffer.find("\n\n") {
                let msg = buffer[..end].to_string();
                buffer = buffer[end + 2..].to_string();

                let mut event_type = String::new();
                let mut data = String::new();
                for line in msg.lines() {
                    if let Some(v) = line.strip_prefix("event: ") { event_type = v.trim().to_string(); }
                    else if let Some(v) = line.strip_prefix("data: ") { data = v.trim().to_string(); }
                }

                if data.is_empty() || event_type == "ping" { continue; }

                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&data) {
                    if val.get("type").and_then(|t| t.as_str()) == Some("content_block_delta") {
                        if let Some(text) = val.get("delta").and_then(|d| d.get("text")).and_then(|t| t.as_str()) {
                            app.emit("explain-stream", ExplainPayload { content: text.to_string(), done: false })
                                .map_err(|e| e.to_string())?;
                        }
                    }
                }
            }
        }

        app.emit("explain-stream", ExplainPayload { content: String::new(), done: true })
            .map_err(|e| e.to_string())
    }
}
