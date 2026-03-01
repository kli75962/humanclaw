use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

/// A single remembered fact about the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    /// Unique ID — milliseconds since epoch as string.
    pub id: String,
    /// Human-readable preference or fact, e.g. "User prefers Traditional Chinese".
    pub content: String,
    /// Unix timestamp (seconds) when this memory was created.
    pub created_at: u64,
}

fn memories_path(app: &AppHandle) -> std::path::PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_default()
        .join("memories.json")
}

pub fn load_memories(app: &AppHandle) -> Vec<Memory> {
    let path = memories_path(app);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

fn save_memories(app: &AppHandle, memories: &[Memory]) {
    let path = memories_path(app);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(
        &path,
        serde_json::to_string_pretty(memories).unwrap_or_default(),
    );
}

pub fn add_memories(app: &AppHandle, contents: Vec<String>) {
    if contents.is_empty() {
        return;
    }
    let mut memories = load_memories(app);
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let now_s = now_ms / 1000;

    for (i, content) in contents.into_iter().enumerate() {
        let content = content.trim().to_string();
        if content.is_empty() {
            continue;
        }
        // Skip if an identical memory already exists
        if memories.iter().any(|m| m.content == content) {
            continue;
        }
        memories.push(Memory {
            id: format!("{}", now_ms + i as u128),
            content,
            created_at: now_s as u64,
        });
    }
    save_memories(app, &memories);
}

pub fn delete_memory(app: &AppHandle, id: &str) {
    let mut memories = load_memories(app);
    memories.retain(|m| m.id != id);
    save_memories(app, &memories);
}

pub fn clear_memories(app: &AppHandle) {
    save_memories(app, &[]);
}

/// Formats stored memories into a block injected into the system prompt.
pub fn build_memory_prompt(memories: &[Memory]) -> String {
    if memories.is_empty() {
        return String::new();
    }
    let list = memories
        .iter()
        .map(|m| format!("- {}", m.content))
        .collect::<Vec<_>>()
        .join("\n");
    format!("[USER MEMORY — apply these preferences in every response]\n{list}")
}
