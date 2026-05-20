use std::convert::Infallible;
use std::sync::OnceLock;
use std::time::Duration;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use super::server::BridgeState;
use super::types::PingQuery;
use crate::session::store;

/// Cross-device sync event broadcast over SSE to every paired peer that is
/// currently subscribed to `/events`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SyncEvent {
    /// Token chunk from a streaming LLM response. `done=true` marks the end.
    StreamChunk {
        chat_id: String,
        content: String,
        done: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        brief: Option<String>,
    },
    /// Agent loop status (round, tool count, etc).
    AgentStatus {
        chat_id: String,
        message: String,
    },
    /// Chat list / messages updated — peer should reload the relevant chat.
    ChatUpdated { chat_id: String },
    /// Settings change: `field` is e.g. `"persona"`, `"pc_permissions"`,
    /// `"ollama_model"`. `value` carries the new value as JSON.
    SettingsChanged {
        field: String,
        value: serde_json::Value,
    },
}

fn channel() -> &'static broadcast::Sender<SyncEvent> {
    static CHAN: OnceLock<broadcast::Sender<SyncEvent>> = OnceLock::new();
    CHAN.get_or_init(|| broadcast::channel(256).0)
}

/// Broadcast an event to all currently-subscribed peers (no-op when nobody is
/// listening — `send` only errors if there are zero receivers, which is fine).
pub fn broadcast(event: SyncEvent) {
    let _ = channel().send(event);
}

/// Subscribe to the local broadcast channel.
pub fn subscribe() -> broadcast::Receiver<SyncEvent> {
    channel().subscribe()
}

/// GET /events?key=<hash> — long-lived Server-Sent Events stream of
/// `SyncEvent` JSON payloads, one per `data:` line, with periodic keep-alive
/// comments so intermediaries don't drop the connection.
pub async fn events_handler(
    State(app): State<BridgeState>,
    Query(query): Query<PingQuery>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    let cfg = store::bootstrap(&app);
    if query.key != cfg.hash_key {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let rx = subscribe();
    let stream = futures_util::stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(ev) => {
                    if let Ok(json) = serde_json::to_string(&ev) {
                        return Some((Ok(Event::default().data(json)), rx));
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    });
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15))))
}
