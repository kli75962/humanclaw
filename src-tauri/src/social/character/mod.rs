pub mod fs;
pub mod memory;
pub use fs::{CharacterMeta, CharacterSyncPayload, export_character_sync_payload, import_character_sync_payload, list_characters as list_characters_fs};

fn emit_and_sync(app: &tauri::AppHandle) {
    use tauri::Emitter;
    let _ = app.emit("character-sync-updated", serde_json::json!({}));
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        crate::network::sync::character::sync_to_all_peers(&app_clone).await;
    });
}

/// List all characters with sociability scores injected from persona configs.
#[tauri::command]
pub fn list_characters(app: tauri::AppHandle) -> Vec<CharacterMeta> {
    let mut chars = fs::list_characters(&app);
    for c in &mut chars {
        c.sociability = crate::skills::get_sociability_for_persona(&app, &c.persona);
    }
    chars
}

/// Create or update a character.
#[tauri::command]
pub fn save_character(app: tauri::AppHandle, character: CharacterMeta) -> Result<(), String> {
    let r = fs::save_character(&app, &character);
    if r.is_ok() {
        emit_and_sync(&app);
    }
    r
}

/// Delete a character by id.
#[tauri::command]
pub fn delete_character(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let r = fs::delete_character(&app, &id);
    if r.is_ok() {
        emit_and_sync(&app);
    }
    r
}
