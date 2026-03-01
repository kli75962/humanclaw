use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

/// A single piece of general phone navigation knowledge.
/// Shared for all users — stores navigation paths learned during agentic sessions.
/// e.g. "To turn off WiFi: Settings > Network & internet > Internet > WiFi toggle"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavKnowledge {
    /// Unique ID — milliseconds since epoch as string.
    pub id: String,
    /// Human-readable navigation fact, e.g. "To turn off Bluetooth: Settings > Connected devices > toggle Bluetooth".
    pub content: String,
    /// Unix timestamp (seconds) when this entry was created.
    pub created_at: u64,
}

fn knowledge_path(app: &AppHandle) -> std::path::PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_default()
        .join("knowledge.json")
}

pub fn load_knowledge(app: &AppHandle) -> Vec<NavKnowledge> {
    let path = knowledge_path(app);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

fn save_knowledge(app: &AppHandle, items: &[NavKnowledge]) {
    let path = knowledge_path(app);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(
        &path,
        serde_json::to_string_pretty(items).unwrap_or_default(),
    );
}

pub fn add_knowledge(app: &AppHandle, contents: Vec<String>) {
    if contents.is_empty() {
        return;
    }
    let mut items = load_knowledge(app);
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
        // Skip if an identical entry already exists
        if items.iter().any(|k| k.content == content) {
            continue;
        }
        items.push(NavKnowledge {
            id: format!("{}", now_ms + i as u128),
            content,
            created_at: now_s as u64,
        });
    }
    save_knowledge(app, &items);
}

pub fn delete_knowledge(app: &AppHandle, id: &str) {
    let mut items = load_knowledge(app);
    items.retain(|k| k.id != id);
    save_knowledge(app, &items);
}

pub fn clear_knowledge(app: &AppHandle) {
    save_knowledge(app, &[]);
}

/// Formats stored knowledge into a block injected into the system prompt.
/// Placed before the user-preference memory so the LLM can use known paths
/// to navigate faster.
pub fn build_knowledge_prompt(items: &[NavKnowledge]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let list = items
        .iter()
        .map(|k| format!("- {}", k.content))
        .collect::<Vec<_>>()
        .join("\n");
    format!("[NAVIGATION KNOWLEDGE — prefer these known paths when navigating]\n{list}")
}
