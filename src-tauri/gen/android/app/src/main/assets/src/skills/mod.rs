use serde_json::Value;
use tauri::{AppHandle, Manager};

pub mod registry;
pub use registry::{get_skills, load_tool_schemas};

pub mod persona_build;
pub mod persona_sync;

pub use persona_build::{create_persona_background, get_persona_build_status, clear_persona_build_status};
pub use persona_sync::{export_persona_sync_payload, import_persona_sync_payload, PersonaSyncPayload};

/// Returns names of all compiled persona skills (those beginning with "persona-" or "persona_").
pub fn persona_skill_names(app: &AppHandle) -> Vec<String> {
    get_skills(app, false).iter()
        .filter(|s| is_persona_skill(&s.name))
        .map(|s| s.name.to_string())
        .collect()
}

pub const DEFAULT_PERSONA_SKILL: &str = "persona-default";

static SKILLS_PROMPT: std::sync::OnceLock<String> = std::sync::OnceLock::new();

fn is_persona_skill(name: &str) -> bool {
    name.starts_with("persona-") || name.starts_with("persona_")
}

// ── Runtime persona helpers ───────────────────────────────────────────────────

/// List names of all user-created (runtime) personas.
pub fn list_runtime_persona_names(app: &AppHandle) -> Vec<String> {
    let Ok(data_dir) = app.path().app_data_dir() else { return vec![]; };
    let custom_dir = data_dir.join("custom_personas");
    let Ok(entries) = std::fs::read_dir(&custom_dir) else { return vec![]; };
    entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if (name.starts_with("persona-") || name.starts_with("persona_"))
                && e.path().join("SKILL.md").exists()
            {
                Some(name)
            } else {
                None
            }
        })
        .collect()
}

/// Read a runtime persona's SKILL.md content.
fn get_runtime_persona_content(app: &AppHandle, name: &str) -> Option<String> {
    let data_dir = app.path().app_data_dir().ok()?;
    let path = data_dir.join("custom_personas").join(name).join("SKILL.md");
    std::fs::read_to_string(path).ok()
}

/// Public alias used by persona_sync submodule.
pub fn get_runtime_persona_content_pub(app: &AppHandle, name: &str) -> Option<String> {
    get_runtime_persona_content(app, name)
}

/// Build the non-persona skills list for the system prompt (static, no runtime entries).
pub fn build_skills_prompt(app: &AppHandle) -> String {
    SKILLS_PROMPT.get_or_init(|| {
        get_skills(app, false).iter()
            .filter(|s| !is_persona_skill(&s.name))
            .map(|s| format!("- **{}**: {}", s.name, s.content.lines().next().unwrap_or("")))
            .collect::<Vec<_>>()
            .join("\n")
    }).clone()
}

/// Resolve the active persona content, considering both compiled and runtime personas.
/// `selected` should be the raw persona field (e.g. `"persona-jk"` or `"persona_jk"`).
pub fn build_persona_prompt_with_runtime(app: &AppHandle, selected: Option<&str>) -> String {
    let name = selected.unwrap_or(DEFAULT_PERSONA_SKILL);
    get_persona_content(app, name)
}

/// Resolve the active persona's SKILL.md content.
/// Priority: compiled skills → runtime skills → default.
pub fn get_persona_content(app: &AppHandle, selected: &str) -> String {
    // Compiled personas first
    if let Some(skill) = get_skills(app, false).iter().find(|s| s.name == selected && is_persona_skill(&s.name)) {
        return skill.content.to_string();
    }

    // Runtime (user-created) personas
    if let Some(content) = get_runtime_persona_content(app, selected) {
        return content;
    }

    // Fallback to default
    get_skills(app, false).iter()
        .find(|s| s.name == DEFAULT_PERSONA_SKILL)
        .map(|s| s.content.to_string())
        .unwrap_or_default()
}

/// Return the sociability score (0–100) for a persona name.
pub fn get_sociability_for_persona(app: &AppHandle, persona: &str) -> u8 {
    let persona_lower = persona.to_lowercase();
    let slug = persona_lower
        .strip_prefix("persona_")
        .or_else(|| persona_lower.strip_prefix("persona-"))
        .unwrap_or(&persona_lower);
    let underscore = format!("persona_{slug}");
    let dash = format!("persona-{slug}");

    // 1. Embedded compiled skill configs
    let skills = get_skills(app, false);
    for skill in &skills {
        if skill.name == underscore || skill.name == dash {
            if let Some(ref json) = skill.config {
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

/// Look up a skill's content by exact name.
pub fn get_skill_content(app: &AppHandle, name: &str) -> Option<String> {
    get_skills(app, false).into_iter().find(|s| s.name == name).map(|s| s.content)
}
