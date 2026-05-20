use tauri::{AppHandle, Emitter};

use crate::ai::types::{AgentStatusPayload, StreamPayload};
use crate::network::sse::{broadcast, SyncEvent};

/// Emit `ollama-stream` to the local frontend AND broadcast the chunk to all
/// paired peers subscribed via SSE. `chat_id` lets the peer route the chunk to
/// the matching chat in its UI.
pub fn emit_stream(
    app: &AppHandle,
    chat_id: &str,
    content: String,
    done: bool,
    brief: Option<String>,
) -> Result<(), String> {
    let payload = StreamPayload {
        chat_id: Some(chat_id.to_string()),
        remote: false,
        content: content.clone(),
        done,
        brief: brief.clone(),
    };
    app.emit("ollama-stream", payload).map_err(|e| e.to_string())?;
    broadcast(SyncEvent::StreamChunk {
        chat_id: chat_id.to_string(),
        content,
        done,
        brief,
    });
    Ok(())
}

/// Same as `emit_stream` but swallows the emit error — for cleanup paths where
/// the caller can't propagate `Result`.
pub fn emit_stream_quiet(
    app: &AppHandle,
    chat_id: &str,
    content: String,
    done: bool,
    brief: Option<String>,
) {
    let _ = emit_stream(app, chat_id, content, done, brief);
}

/// Emit `agent-status` locally and broadcast to peers.
pub fn emit_status(app: &AppHandle, chat_id: &str, message: String) {
    let payload = AgentStatusPayload {
        chat_id: Some(chat_id.to_string()),
        remote: false,
        message: message.clone(),
    };
    let _ = app.emit("agent-status", payload);
    broadcast(SyncEvent::AgentStatus {
        chat_id: chat_id.to_string(),
        message,
    });
}

