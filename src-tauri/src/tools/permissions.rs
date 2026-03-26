use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::oneshot;
use uuid::Uuid;

/// Holds pending "ask before use" permission requests.
/// When a tool requires confirmation, a sender is stored here and an event is
/// emitted to the frontend; the frontend calls `respond_pc_permission` to
/// resolve it.
pub struct PendingPermissions(pub Mutex<HashMap<String, oneshot::Sender<bool>>>);

/// Emit a `pc-permission-request` event and wait for the user's response.
/// Returns `true` if the user allowed the action, `false` otherwise.
pub async fn request_permission(
    app: &AppHandle,
    tool_name: &str,
    permission: &str,
    args: &serde_json::Value,
) -> bool {
    let state = app.state::<PendingPermissions>();
    let (tx, rx) = oneshot::channel::<bool>();
    let id = Uuid::new_v4().to_string();

    {
        let mut map = state.0.lock().unwrap();
        map.insert(id.clone(), tx);
    }

    app.emit(
        "pc-permission-request",
        serde_json::json!({
            "id": id,
            "tool": tool_name,
            "permission": permission,
            "args": args,
        }),
    )
    .ok();

    rx.await.unwrap_or(false)
}

/// Called by the frontend to resolve a pending permission request.
#[tauri::command]
pub fn respond_pc_permission(
    state: tauri::State<'_, PendingPermissions>,
    id: String,
    allowed: bool,
) {
    let mut map = state.0.lock().unwrap();
    if let Some(tx) = map.remove(&id) {
        let _ = tx.send(allowed);
    }
}
