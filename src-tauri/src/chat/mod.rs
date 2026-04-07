mod fs;
mod memos;
pub use fs::{bootstrap_memory, memory_dir};
pub use fs::{ALLOWED_FILES, CORE_FILE, normalize_memory_path};
pub use fs::read_memory_file;
pub use fs::{export_chat_sync_payload, import_chat_sync_payload, ChatSyncChat, ChatSyncPayload};

fn emit_and_sync(app: &tauri::AppHandle) {
    use tauri::Emitter;
    let _ = app.emit("chat-sync-updated", serde_json::json!({}));
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        crate::network::sync::chat::sync_to_all_peers(&app_clone).await;
    });
}

// ----- Tauri commands exposed to the frontend -----

/// Read one of the memory files: "core.md"
#[tauri::command]
pub fn get_memory_file(app: tauri::AppHandle, filename: String) -> Result<String, String> {
    fs::read_memory_file(&app, &filename)
}

/// Overwrite one of the memory files.
#[tauri::command]
pub fn set_memory_file(app: tauri::AppHandle, filename: String, content: String) -> Result<(), String> {
    fs::write_memory_file(&app, &filename, &content)
}

/// List all saved chats (newest first).
#[tauri::command]
pub fn list_chats(app: tauri::AppHandle) -> Vec<fs::ChatMeta> {
    fs::list_chats(&app)
}

/// Load the messages array for a specific chat id.
#[tauri::command]
pub fn load_chat_messages(app: tauri::AppHandle, id: String) -> Vec<serde_json::Value> {
    fs::load_chat_messages(&app, &id)
}

/// Register a new chat entry.
#[tauri::command]
pub fn create_chat(app: tauri::AppHandle, id: String, title: String, created_at: String) -> Result<(), String> {
    let r = fs::create_chat(&app, &id, &title, &created_at);
    if r.is_ok() {
        emit_and_sync(&app);
    }
    r
}

/// Persist the messages for an existing chat.
#[tauri::command]
pub fn save_chat_messages(app: tauri::AppHandle, id: String, messages: Vec<serde_json::Value>) -> Result<(), String> {
    let r = fs::save_chat_messages(&app, &id, messages);
    if r.is_ok() {
        emit_and_sync(&app);
    }
    r
}

/// Delete a chat and its messages.
#[tauri::command]
pub fn delete_chat(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let r = fs::delete_chat(&app, &id);
    if r.is_ok() {
        emit_and_sync(&app);
    }
    r
}

pub use memos::{list_memos, load_memo_messages, create_memo, save_memo_messages, delete_memo};
