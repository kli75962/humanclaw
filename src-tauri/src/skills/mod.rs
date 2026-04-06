// Auto-generated agent registry (skills + tools) embedded at compile time.
include!(concat!(env!("OUT_DIR"), "/agent_registry.rs"));

use serde_json::Value;
use std::sync::OnceLock;
use tauri::{AppHandle, Manager};

// ── Persona build status (persisted JSON) ────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PersonaBuildStatus {
    pub status: String,       // "creating" | "done" | "interrupted"
    pub display_name: String, // shown in the notice
    pub model: String,
    pub sex: String,
    pub age_range: String,
    pub vibe: String,
    pub world: String,
    pub connects_by: String,
    pub persona_name: String, // original wizard input
}

fn persona_build_status_path(app: &AppHandle) -> std::path::PathBuf {
    app.path().app_data_dir().unwrap_or_default().join("persona_build_status.json")
}

fn read_persona_build_status(app: &AppHandle) -> Option<PersonaBuildStatus> {
    let text = std::fs::read_to_string(persona_build_status_path(app)).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_persona_build_status(app: &AppHandle, status: &PersonaBuildStatus) {
    let path = persona_build_status_path(app);
    if let Ok(text) = serde_json::to_string(status) {
        let _ = std::fs::write(path, text);
    }
}

#[tauri::command]
pub fn get_persona_build_status(app: AppHandle) -> Option<PersonaBuildStatus> {
    read_persona_build_status(&app)
}

#[tauri::command]
pub fn clear_persona_build_status(app: AppHandle) -> Result<(), String> {
    let path = persona_build_status_path(&app);
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| e.to_string())
    } else {
        Ok(())
    }
}

/// Background persona creation — runs the LLM headless and calls create_skill without
/// opening a chat window. Returns the LLM's final reply on success.
#[tauri::command]
pub async fn create_persona_background(
    app: AppHandle,
    model: String,
    sex: String,
    age_range: String,
    vibe: String,
    world: String,
    connects_by: String,
    persona_name: String,
) -> Result<String, String> {
    use crate::model::ollama::types::OllamaMessage;
    use crate::model::ollama::headless::run_headless;

    // Snapshot existing runtime personas before creation
    let before_names: std::collections::HashSet<String> =
        list_runtime_persona_names(&app).into_iter().collect();

    // Write "creating" status
    let display_name = if persona_name == "random" {
        "new persona".to_string()
    } else {
        persona_name.clone()
    };
    let build_status = PersonaBuildStatus {
        status: "creating".to_string(),
        display_name: display_name.clone(),
        model: model.clone(),
        sex: sex.clone(),
        age_range: age_range.clone(),
        vibe: vibe.clone(),
        world: world.clone(),
        connects_by: connects_by.clone(),
        persona_name: persona_name.clone(),
    };
    write_persona_build_status(&app, &build_status);

    let message = format!(
        "Create a new persona using the create_skill tool based on these preferences:\n\
        - Gender: {sex}\n\
        - Age range: {age_range}\n\
        - Vibe: {vibe}\n\
        - World: {world}\n\
        - Connects by: {connects_by}\n\
        - Name: {persona_name}\n\n\
        Follow the persona skill creation guide. The skill name must start with \"persona_\"."
    );

    let conversation = vec![OllamaMessage {
        role: "user".to_string(),
        content: message,
        tool_calls: None,
        images: None,
    }];

    let result = run_headless(&app, conversation, &model, None, None).await;

    match &result {
        Ok(_) => {
            let after_names: std::collections::HashSet<String> =
                list_runtime_persona_names(&app).into_iter().collect();
            let new_display = after_names
                .difference(&before_names)
                .next()
                .map(|n| {
                    n.strip_prefix("persona_")
                        .or_else(|| n.strip_prefix("persona-"))
                        .unwrap_or(n)
                        .to_string()
                })
                .unwrap_or(display_name);
            write_persona_build_status(&app, &PersonaBuildStatus {
                status: "done".to_string(),
                display_name: new_display,
                ..build_status
            });
        }
        Err(_) => {
            write_persona_build_status(&app, &PersonaBuildStatus {
                status: "interrupted".to_string(),
                ..build_status
            });
        }
    }

    result
}

pub const DEFAULT_PERSONA_SKILL: &str = "persona-default";

static SKILLS_PROMPT: OnceLock<String> = OnceLock::new();
static TOOL_SCHEMAS: OnceLock<Vec<Value>> = OnceLock::new();

fn is_persona_skill(name: &str) -> bool {
    name.starts_with("persona-") || name.starts_with("persona_")
}

fn is_ig_skill(name: &str) -> bool {
    matches!(name, "generate-post" | "post-comment" | "post-dm"
                 | "generate_post" | "post_comment" | "post_dm")
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

/// All tool JSON schemas parsed and cached for the lifetime of the process.
pub fn load_tool_schemas() -> &'static [Value] {
    TOOL_SCHEMAS.get_or_init(|| {
        TOOLS
            .iter()
            .filter_map(|t| serde_json::from_str(t.content).ok())
            .collect()
    })
}
