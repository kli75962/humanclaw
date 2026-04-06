use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::oneshot;
use uuid::Uuid;

/// Holds pending `ask_user` requests indexed by a unique request ID.
/// Each entry contains a sender that resolves once the frontend submits answers.
pub struct PendingAskUserRequests(pub Mutex<HashMap<String, oneshot::Sender<HashMap<usize, String>>>>);

/// Emit an `ask-user-request` event to the frontend and block until the user
/// submits answers for all questions.  Returns a map from question index → answer.
pub async fn request_ask_user(
    app: &AppHandle,
    questions: &serde_json::Value,
) -> HashMap<usize, String> {
    let state = app.state::<PendingAskUserRequests>();
    let (tx, rx) = oneshot::channel::<HashMap<usize, String>>();
    let id = Uuid::new_v4().to_string();

    {
        let mut map = state.0.lock().unwrap();
        map.insert(id.clone(), tx);
    }

    app.emit(
        "ask-user-request",
        serde_json::json!({
            "id": id,
            "questions": questions,
        }),
    )
    .ok();

    rx.await.unwrap_or_default()
}

/// Tauri command — called by the frontend when the user has answered all questions.
/// Resolves the pending oneshot channel so the tool call can return to the LLM.
#[tauri::command]
pub fn respond_ask_user(
    state: tauri::State<'_, PendingAskUserRequests>,
    id: String,
    answers: HashMap<usize, String>,
) {
    let mut map = state.0.lock().unwrap();
    if let Some(tx) = map.remove(&id) {
        let _ = tx.send(answers);
    }
}
