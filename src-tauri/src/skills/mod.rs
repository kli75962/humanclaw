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
        brief: None,
    }];

    let result = run_headless(&app, conversation, &model, None, None).await;

    match &result {
        Ok(_) => {
            let after_names: std::collections::HashSet<String> =
                list_runtime_persona_names(&app).into_iter().collect();
            let new_display = after_names
                .difference(&before_names)
                .next()
                .map(|dir_name| {
                    // Prefer display_name from persona_config.json if the LLM wrote one
                    if let Ok(data_dir) = app.path().app_data_dir() {
                        let cfg_path = data_dir
                            .join("custom_personas")
                            .join(dir_name)
                            .join("persona_config.json");
                        if let Ok(text) = std::fs::read_to_string(&cfg_path) {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                                if let Some(dn) = v.get("display_name").and_then(|x| x.as_str()) {
                                    if !dn.is_empty() {
                                        return dn.to_string();
                                    }
                                }
                            }
                        }
                    }
                    // Fallback: humanize the directory slug
                    let slug = dir_name
                        .strip_prefix("persona_")
                        .or_else(|| dir_name.strip_prefix("persona-"))
                        .unwrap_or(dir_name);
                    let mut name = slug.replace('_', " ");
                    // Capitalize first letter of each word
                    name = name.split_whitespace()
                        .map(|w| {
                            let mut c = w.chars();
                            match c.next() {
                                None => String::new(),
                                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    name
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

/// Return the sociability score (0–100) for a persona name.
/// Priority: embedded persona_config.json → runtime persona_config.json → keyword scan.
/// Supports both new `sociability` field and legacy `personality_type` field.
pub fn get_sociability_for_persona(app: &AppHandle, persona: &str) -> u8 {
    let persona_lower = persona.to_lowercase();
    let slug = persona_lower
        .strip_prefix("persona_")
        .or_else(|| persona_lower.strip_prefix("persona-"))
        .unwrap_or(&persona_lower);
    let underscore = format!("persona_{slug}");
    let dash = format!("persona-{slug}");

    // 1. Embedded compiled skill configs
    for skill in SKILLS.iter() {
        if skill.name == underscore || skill.name == dash {
            if let Some(json) = skill.config {
                if let Ok(v) = serde_json::from_str::<Value>(json) {
                    if let Some(s) = v.get("sociability").and_then(|x| x.as_u64()) {
                        return s.min(100) as u8;
                    }
                    if let Some(pt) = v.get("personality_type").and_then(|x| x.as_str()) {
                        return match pt { "extrovert" => 75, _ => 25 };
                    }
                }
            }
        }
    }

    // 2. Runtime persona_config.json (user-created)
    if let Ok(data_dir) = app.path().app_data_dir() {
        for name in [&underscore, &dash] {
            let path = data_dir.join("custom_personas").join(name).join("persona_config.json");
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Ok(v) = serde_json::from_str::<Value>(&text) {
                    if let Some(s) = v.get("sociability").and_then(|x| x.as_u64()) {
                        return s.min(100) as u8;
                    }
                    if let Some(pt) = v.get("personality_type").and_then(|x| x.as_str()) {
                        return match pt { "extrovert" => 75, _ => 25 };
                    }
                }
            }
        }
    }

    // 3. Keyword scan fallback
    let extrovert_kw = ["extrovert", "outgoing", "energetic", "enthusiastic", "lively",
        "talkative", "confident", "vibrant", "social", "friendly", "cheerful", "playful",
        "upbeat", "jk"];
    let introvert_kw = ["introvert", "quiet", "reserved", "shy", "concise", "minimal",
        "direct", "taciturn", "withdrawn"];
    for kw in &extrovert_kw { if persona_lower.contains(kw) { return 75; } }
    for kw in &introvert_kw { if persona_lower.contains(kw) { return 25; } }
    60
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
