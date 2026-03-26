// Auto-generated agent registry (skills + tools) embedded at compile time.
include!(concat!(env!("OUT_DIR"), "/agent_registry.rs"));

use serde_json::Value;
use std::sync::OnceLock;
use tauri::{AppHandle, Manager};

pub const DEFAULT_PERSONA_SKILL: &str = "persona_default";

static SKILLS_PROMPT: OnceLock<String> = OnceLock::new();
static TOOL_SCHEMAS: OnceLock<Vec<Value>> = OnceLock::new();

fn is_persona_skill(name: &str) -> bool {
    name.starts_with("persona_")
}

fn is_ig_skill(name: &str) -> bool {
    matches!(name, "generate_post" | "post_comment" | "post_dm")
}

/// Look up a skill's content by exact name. Returns None if not found.
pub fn get_skill_content(name: &str) -> Option<&'static str> {
    SKILLS.iter().find(|s| s.name == name).map(|s| s.content)
}

/// All SKILL.md files joined as a single text block for the system prompt.
/// Computed once and cached for the lifetime of the process.
pub fn build_skills_prompt() -> &'static str {
    SKILLS_PROMPT.get_or_init(|| {
        let mut out = String::new();
        let mut first = true;
        for s in SKILLS.iter().filter(|s| !is_persona_skill(s.name) && !is_ig_skill(s.name)) {
            if !first {
                out.push_str("\n\n---\n\n");
            }
            first = false;
            out.push_str(s.content);
        }
        out
    })
}

/// Return all available persona skill names sorted by registry order.
pub fn persona_skill_names() -> Vec<&'static str> {
    SKILLS
        .iter()
        .filter(|s| is_persona_skill(s.name))
        .map(|s| s.name)
        .collect()
}

/// List runtime persona names stored in the app data directory.
pub fn list_runtime_persona_names(app: &AppHandle) -> Vec<String> {
    let Ok(data_dir) = app.path().app_data_dir() else { return vec![]; };
    let custom_dir = data_dir.join("custom_personas");
    let Ok(entries) = std::fs::read_dir(&custom_dir) else { return vec![]; };
    let mut names: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir() && e.path().join("SKILL.md").exists())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    names.sort();
    names
}

/// Load a runtime persona's SKILL.md content by name.
pub fn get_runtime_persona_content(app: &AppHandle, name: &str) -> Option<String> {
    let data_dir = app.path().app_data_dir().ok()?;
    std::fs::read_to_string(data_dir.join("custom_personas").join(name).join("SKILL.md")).ok()
}

/// Build persona prompt, checking runtime personas after compiled ones.
pub fn build_persona_prompt_with_runtime(app: &AppHandle, selected: Option<&str>) -> String {
    let selected = selected.unwrap_or(DEFAULT_PERSONA_SKILL);

    // Compiled personas first
    if let Some(skill) = SKILLS.iter().find(|s| s.name == selected && is_persona_skill(s.name)) {
        return skill.content.to_string();
    }

    // Runtime (user-created) personas
    if let Some(content) = get_runtime_persona_content(app, selected) {
        return content;
    }

    // Fallback to default
    SKILLS.iter()
        .find(|s| s.name == DEFAULT_PERSONA_SKILL)
        .map(|s| s.content.to_string())
        .unwrap_or_default()
}

// ── Runtime persona sync helpers ──────────────────────────────────────────────

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
            get_runtime_persona_content(app, &name)
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
        if !entry.name.starts_with("persona_") {
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

/// All tool JSON schemas parsed and cached for the lifetime of the process.
pub fn load_tool_schemas() -> &'static [Value] {
    TOOL_SCHEMAS.get_or_init(|| {
        TOOLS
            .iter()
            .filter_map(|t| serde_json::from_str(t.content).ok())
            .collect()
    })
}
