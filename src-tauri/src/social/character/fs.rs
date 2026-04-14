use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::chat::memory_dir;

/// Full character definition — stored in the characters index.
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CharacterMeta {
    pub id: String,
    pub name: String,
    /// Optional icon stored as a base64 data URL (e.g. "data:image/jpeg;base64,...").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub model: String,
    pub persona: String,
    pub background: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub birthday: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live2d_model_id: Option<String>,
    /// Sociability score 0–100, computed from persona config. Not stored in JSON.
    #[serde(default, skip_deserializing)]
    pub sociability: u8,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CharacterSyncPayload {
    pub characters: Vec<CharacterMeta>,
}

fn characters_dir(app: &AppHandle) -> PathBuf {
    memory_dir(app).join("characters")
}

fn characters_index_path(app: &AppHandle) -> PathBuf {
    characters_dir(app).join("_index.json")
}

fn is_safe_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_alphanumeric() || c == '-')
}

/// List all characters in insertion order (newest first).
pub fn list_characters(app: &AppHandle) -> Vec<CharacterMeta> {
    let text = std::fs::read_to_string(characters_index_path(app)).unwrap_or_default();
    serde_json::from_str(&text).unwrap_or_default()
}

/// Create or update a character in the index.
pub fn save_character(app: &AppHandle, character: &CharacterMeta) -> Result<(), String> {
    if !is_safe_id(&character.id) {
        return Err("Invalid character id".to_string());
    }
    let dir = characters_dir(app);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let mut chars = list_characters(app);
    if let Some(existing) = chars.iter_mut().find(|c| c.id == character.id) {
        *existing = character.clone();
    } else {
        chars.insert(0, character.clone());
    }

    let json = serde_json::to_string(&chars).map_err(|e| e.to_string())?;
    std::fs::write(characters_index_path(app), json).map_err(|e| e.to_string())
}

/// Remove a character from the index.
pub fn delete_character(app: &AppHandle, id: &str) -> Result<(), String> {
    if !is_safe_id(id) {
        return Err("Invalid character id".to_string());
    }
    let mut chars = list_characters(app);
    chars.retain(|c| c.id != id);
    let json = serde_json::to_string(&chars).map_err(|e| e.to_string())?;
    let dir = characters_dir(app);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(characters_index_path(app), json).map_err(|e| e.to_string())
}

/// Build a full snapshot for cross-device synchronization.
pub fn export_character_sync_payload(app: &AppHandle) -> CharacterSyncPayload {
    CharacterSyncPayload {
        characters: list_characters(app),
    }
}

/// Apply an incoming snapshot.
/// - `replace=true`: mirror remote state exactly.
/// - `replace=false`: merge remote characters into local (last-write-wins by id).
pub fn import_character_sync_payload(
    app: &AppHandle,
    payload: CharacterSyncPayload,
    replace: bool,
) -> Result<(), String> {
    let dir = characters_dir(app);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    if replace {
        let json = serde_json::to_string(&payload.characters).map_err(|e| e.to_string())?;
        return std::fs::write(characters_index_path(app), json).map_err(|e| e.to_string());
    }

    let mut merged = list_characters(app);
    for incoming in payload.characters.into_iter().rev() {
        if !is_safe_id(&incoming.id) {
            continue;
        }
        if let Some(existing) = merged.iter_mut().find(|c| c.id == incoming.id) {
            *existing = incoming;
        } else {
            merged.insert(0, incoming);
        }
    }

    let json = serde_json::to_string(&merged).map_err(|e| e.to_string())?;
    std::fs::write(characters_index_path(app), json).map_err(|e| e.to_string())
}
