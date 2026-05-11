use tauri::{AppHandle, Manager};
use super::list_runtime_persona_names;
use super::get_runtime_persona_content_pub;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct PersonaEntry {
    pub name: String,
    pub content: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct PersonaSyncPayload {
    pub personas: Vec<PersonaEntry>,
}

/// Export all runtime personas as a sync payload.
pub fn export_persona_sync_payload(app: &AppHandle) -> PersonaSyncPayload {
    let names = list_runtime_persona_names(app);
    let personas = names
        .into_iter()
        .filter_map(|name| {
            get_runtime_persona_content_pub(app, &name)
                .map(|content| PersonaEntry { name, content })
        })
        .collect();
    PersonaSyncPayload { personas }
}

/// Write incoming personas to the app data dir.
/// If `replace` is false, existing personas are not overwritten.
pub fn import_persona_sync_payload(
    app: &AppHandle,
    payload: PersonaSyncPayload,
    replace: bool,
) -> Result<(), String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let custom_dir = data_dir.join("custom_personas");
    for entry in payload.personas {
        if !entry.name.starts_with("persona-") && !entry.name.starts_with("persona_") {
            continue;
        }
        let skill_dir = custom_dir.join(&entry.name);
        let skill_file = skill_dir.join("SKILL.md");
        if !replace && skill_file.exists() {
            continue;
        }
        std::fs::create_dir_all(&skill_dir).map_err(|e| e.to_string())?;
        std::fs::write(skill_file, &entry.content).map_err(|e| e.to_string())?;
    }
    Ok(())
}
