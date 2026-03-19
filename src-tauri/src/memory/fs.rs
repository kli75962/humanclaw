use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

// ── Memory file names ────────────────────────────────────────────────────────

pub const CORE_FILE: &str = "core.md";
pub const ALLOWED_FILES: &[&str] = &[CORE_FILE];

const DEFAULT_CORE: &str = "\
# Core Memory
- Keep this file short.
- Write stable user facts here (name, recurring goals, preferences).
";

// ── Filesystem helpers ───────────────────────────────────────────────────────

pub fn memory_dir(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_default().join(".memory")
}

pub fn normalize_memory_path(path: &str) -> Option<&str> {
    // Accept "/memories/core.md", "/.memory/core.md", "core.md", etc.
    let base = path
        .trim()
        .trim_start_matches('/')
        .trim_start_matches("memories/")
        .trim_start_matches(".memory/");
    if ALLOWED_FILES.contains(&base) {
        Some(base)
    } else {
        None
    }
}

/// Create `.memory/` directory and seed each file if it does not yet exist.
pub fn bootstrap_memory(app: &AppHandle) {
    let dir = memory_dir(app);
    let _ = std::fs::create_dir_all(&dir);
    ensure_file(&dir.join(CORE_FILE), DEFAULT_CORE);
}

fn ensure_file(path: &PathBuf, default: &str) {
    if !path.exists() {
        let _ = std::fs::write(path, default);
    }
}

/// Read any allowed memory file by name.
pub fn read_memory_file(app: &AppHandle, filename: &str) -> Result<String, String> {
    if !ALLOWED_FILES.contains(&filename) {
        return Err(format!("Unknown memory file: {filename}"));
    }
    std::fs::read_to_string(memory_dir(app).join(filename)).map_err(|e| e.to_string())
}

/// Overwrite an allowed memory file.
pub fn write_memory_file(app: &AppHandle, filename: &str, content: &str) -> Result<(), String> {
    if !ALLOWED_FILES.contains(&filename) {
        return Err(format!("Unknown memory file: {filename}"));
    }
    std::fs::write(memory_dir(app).join(filename), content).map_err(|e| e.to_string())
}

// ── Multi-chat storage ───────────────────────────────────────────────────────

/// Metadata for a single chat session.
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatMeta {
    pub id: String,
    pub title: String,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatSyncChat {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub messages: Vec<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatSyncPayload {
    pub chats: Vec<ChatSyncChat>,
}

fn chats_dir(app: &AppHandle) -> PathBuf {
    memory_dir(app).join("chats")
}

fn chats_index_path(app: &AppHandle) -> PathBuf {
    chats_dir(app).join("_index.json")
}

/// Returns true if `id` is a safe UUID-ish string (alphanumeric + hyphens only).
fn is_safe_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_alphanumeric() || c == '-')
}

/// List all chats ordered newest-first (as stored in the index).
pub fn list_chats(app: &AppHandle) -> Vec<ChatMeta> {
    let text = std::fs::read_to_string(chats_index_path(app)).unwrap_or_default();
    serde_json::from_str(&text).unwrap_or_default()
}

/// Load the messages for a specific chat. Returns an empty vec on any error.
pub fn load_chat_messages(app: &AppHandle, id: &str) -> Vec<serde_json::Value> {
    if !is_safe_id(id) {
        return vec![];
    }
    let path = chats_dir(app).join(format!("{id}.json"));
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    serde_json::from_str(&text).unwrap_or_default()
}

/// Register a new chat in the index (messages file is created empty).
/// Call once when the chat is first created.
pub fn create_chat(app: &AppHandle, id: &str, title: &str, created_at: &str) -> Result<(), String> {
    if !is_safe_id(id) {
        return Err("Invalid chat id".to_string());
    }
    let dir = chats_dir(app);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    // Write empty messages file
    let msg_path = dir.join(format!("{id}.json"));
    std::fs::write(&msg_path, "[]").map_err(|e| e.to_string())?;

    // Prepend to index
    let mut metas = list_chats(app);
    if !metas.iter().any(|m| m.id == id) {
        metas.insert(0, ChatMeta { id: id.to_string(), title: title.to_string(), created_at: created_at.to_string() });
        let json = serde_json::to_string(&metas).map_err(|e| e.to_string())?;
        std::fs::write(chats_index_path(app), json).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Overwrite the messages file for an existing chat.
pub fn save_chat_messages(app: &AppHandle, id: &str, messages: Vec<serde_json::Value>) -> Result<(), String> {
    if !is_safe_id(id) {
        return Err("Invalid chat id".to_string());
    }
    let dir = chats_dir(app);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{id}.json"));
    let json = serde_json::to_string(&messages).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

/// Remove a chat from the index and delete its messages file.
pub fn delete_chat(app: &AppHandle, id: &str) -> Result<(), String> {
    if !is_safe_id(id) {
        return Err("Invalid chat id".to_string());
    }
    let dir = chats_dir(app);
    let msg_path = dir.join(format!("{id}.json"));
    let _ = std::fs::remove_file(&msg_path);
    let mut metas = list_chats(app);
    metas.retain(|m| m.id != id);
    let json = serde_json::to_string(&metas).map_err(|e| e.to_string())?;
    std::fs::write(chats_index_path(app), json).map_err(|e| e.to_string())
}

/// Build a full chat snapshot for cross-device synchronization.
pub fn export_chat_sync_payload(app: &AppHandle) -> ChatSyncPayload {
    let metas = list_chats(app);
    let chats = metas
        .into_iter()
        .map(|meta| ChatSyncChat {
            id: meta.id.clone(),
            title: meta.title,
            created_at: meta.created_at,
            messages: load_chat_messages(app, &meta.id),
        })
        .collect();
    ChatSyncPayload { chats }
}

/// Apply a chat snapshot.
/// - `replace=true`: mirror remote state exactly (including deletions).
/// - `replace=false`: merge remote chats into local without deleting locals.
pub fn import_chat_sync_payload(
    app: &AppHandle,
    payload: ChatSyncPayload,
    replace: bool,
) -> Result<(), String> {
    let dir = chats_dir(app);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let mut incoming_metas = Vec::<ChatMeta>::with_capacity(payload.chats.len());
    let mut incoming_ids = std::collections::HashSet::<String>::new();

    for chat in payload.chats {
        if !is_safe_id(&chat.id) {
            continue;
        }
        incoming_ids.insert(chat.id.clone());

        let path = dir.join(format!("{}.json", chat.id));
        let json = serde_json::to_string(&chat.messages).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())?;

        incoming_metas.push(ChatMeta {
            id: chat.id,
            title: chat.title,
            created_at: chat.created_at,
        });
    }

    if replace {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.file_name().and_then(|n| n.to_str()) == Some("_index.json") {
                    continue;
                }
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
                if !incoming_ids.contains(stem) {
                    let _ = std::fs::remove_file(path);
                }
            }
        }

        let json = serde_json::to_string(&incoming_metas).map_err(|e| e.to_string())?;
        std::fs::write(chats_index_path(app), json).map_err(|e| e.to_string())?;
        return Ok(());
    }

    let mut merged = list_chats(app);
    for meta in incoming_metas.into_iter().rev() {
        if let Some(existing) = merged.iter_mut().find(|m| m.id == meta.id) {
            *existing = meta;
        } else {
            merged.insert(0, meta);
        }
    }

    let json = serde_json::to_string(&merged).map_err(|e| e.to_string())?;
    std::fs::write(chats_index_path(app), json).map_err(|e| e.to_string())
}
