use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::AppHandle;

use crate::chat::memory_dir;
use crate::social::character::list_characters_fs;
use super::fs::{PostComment, PostMeta};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RagEntry {
    pub id: String,
    pub entry_type: String,       // "post" or "comment"
    pub author_id: String,        // character_id or "user"
    pub text: String,
    pub post_id: Option<String>,  // for comments: which post they belong to
}

fn index_path(app: &AppHandle) -> PathBuf {
    memory_dir(app).join("social_rag_index.json")
}

pub fn load_index(app: &AppHandle) -> Vec<RagEntry> {
    let text = std::fs::read_to_string(index_path(app)).unwrap_or_default();
    serde_json::from_str(&text).unwrap_or_default()
}

fn save_index(app: &AppHandle, entries: &[RagEntry]) {
    let Ok(json) = serde_json::to_string(entries) else { return };
    let path = index_path(app);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, json);
}

pub fn upsert_post(app: &AppHandle, post: &PostMeta) {
    let mut entries = load_index(app);
    if let Some(existing) = entries.iter_mut().find(|e| e.id == post.id) {
        existing.author_id = post.character_id.clone();
        existing.text = post.text.clone();
    } else {
        entries.push(RagEntry {
            id: post.id.clone(),
            entry_type: "post".to_string(),
            author_id: post.character_id.clone(),
            text: post.text.clone(),
            post_id: None,
        });
    }
    save_index(app, &entries);
}

pub fn upsert_comment(app: &AppHandle, comment: &PostComment) {
    let mut entries = load_index(app);
    if let Some(existing) = entries.iter_mut().find(|e| e.id == comment.id) {
        existing.author_id = comment.author_id.clone();
        existing.text = comment.text.clone();
    } else {
        entries.push(RagEntry {
            id: comment.id.clone(),
            entry_type: "comment".to_string(),
            author_id: comment.author_id.clone(),
            text: comment.text.clone(),
            post_id: Some(comment.post_id.clone()),
        });
    }
    save_index(app, &entries);
}

/// Remove a post and all its associated comments from the index.
pub fn remove_by_post_id(app: &AppHandle, post_id: &str) {
    let mut entries = load_index(app);
    entries.retain(|e| {
        // Remove the post itself
        if e.entry_type == "post" && e.id == post_id { return false; }
        // Remove comments belonging to this post
        if e.entry_type == "comment" {
            if let Some(ref pid) = e.post_id {
                if pid == post_id { return false; }
            }
        }
        true
    });
    save_index(app, &entries);
}

/// Search the index for entries matching any of the given keywords (case-insensitive substring).
/// Returns a formatted `[RELEVANT POSTS & COMMENTS]` block, or empty string if no matches.
pub fn search(app: &AppHandle, keywords: &[String], max: usize) -> String {
    if keywords.is_empty() || max == 0 {
        return String::new();
    }

    let entries = load_index(app);
    let characters = list_characters_fs(app);

    let matches: Vec<&RagEntry> = entries.iter()
        .filter(|e| {
            let text_lower = e.text.to_lowercase();
            keywords.iter().any(|k| text_lower.contains(&k.to_lowercase()))
        })
        .take(max)
        .collect();

    if matches.is_empty() {
        return String::new();
    }

    let resolve_name = |author_id: &str| -> String {
        if author_id == "user" {
            "User".to_string()
        } else {
            characters.iter()
                .find(|c| c.id == author_id)
                .map(|c| c.name.clone())
                .unwrap_or_else(|| author_id.to_string())
        }
    };

    let mut block = String::from("[RELEVANT POSTS & COMMENTS]\n");
    for entry in &matches {
        let author = resolve_name(&entry.author_id);
        let verb = if entry.entry_type == "post" { "posted" } else { "commented" };
        block.push_str(&format!("- [{author}] {verb}: \"{}\"\n", entry.text));
    }
    block
}
