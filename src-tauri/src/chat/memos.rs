use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// Summary returned by list_memos (no messages for efficiency).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoMeta {
    pub id: String,
    pub title: String,
    pub created_at: String,
}

/// Full memo record stored on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemoFull {
    id: String,
    title: String,
    messages: Vec<serde_json::Value>,
    created_at: String,
}

fn memos_path(app: &AppHandle) -> std::path::PathBuf {
    app.path().app_data_dir().unwrap_or_default().join("memos.json")
}

fn load_all(app: &AppHandle) -> Vec<MemoFull> {
    std::fs::read_to_string(memos_path(app))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn persist(app: &AppHandle, memos: &[MemoFull]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(memos).map_err(|e| e.to_string())?;
    std::fs::write(memos_path(app), json).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_memos(app: AppHandle) -> Vec<MemoMeta> {
    load_all(&app)
        .into_iter()
        .map(|m| MemoMeta { id: m.id, title: m.title, created_at: m.created_at })
        .collect()
}

#[tauri::command]
pub fn load_memo_messages(app: AppHandle, id: String) -> Vec<serde_json::Value> {
    load_all(&app)
        .into_iter()
        .find(|m| m.id == id)
        .map(|m| m.messages)
        .unwrap_or_default()
}

#[tauri::command]
pub fn create_memo(
    app: AppHandle,
    title: String,
    messages: Vec<serde_json::Value>,
) -> Result<MemoMeta, String> {
    let mut memos = load_all(&app);
    let id = uuid::Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().to_rfc3339();
    memos.insert(0, MemoFull {
        id: id.clone(),
        title: title.clone(),
        messages,
        created_at: created_at.clone(),
    });
    persist(&app, &memos)?;
    Ok(MemoMeta { id, title, created_at })
}

#[tauri::command]
pub fn save_memo_messages(
    app: AppHandle,
    id: String,
    messages: Vec<serde_json::Value>,
) -> Result<(), String> {
    let mut memos = load_all(&app);
    if let Some(m) = memos.iter_mut().find(|m| m.id == id) {
        m.messages = messages;
    }
    persist(&app, &memos)
}

#[tauri::command]
pub fn delete_memo(app: AppHandle, id: String) -> Result<(), String> {
    let mut memos = load_all(&app);
    memos.retain(|m| m.id != id);
    persist(&app, &memos)
}
