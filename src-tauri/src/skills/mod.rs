// Auto-generated agent registry (skills + tools) embedded at compile time.
include!(concat!(env!("OUT_DIR"), "/agent_registry.rs"));

use serde_json::Value;
use std::sync::OnceLock;

pub const DEFAULT_PERSONA_SKILL: &str = "persona_default";

static SKILLS_PROMPT: OnceLock<String> = OnceLock::new();
static TOOL_SCHEMAS: OnceLock<Vec<Value>> = OnceLock::new();

fn is_persona_skill(name: &str) -> bool {
    name.starts_with("persona_")
}

/// All SKILL.md files joined as a single text block for the system prompt.
/// Computed once and cached for the lifetime of the process.
pub fn build_skills_prompt() -> &'static str {
    SKILLS_PROMPT.get_or_init(|| {
        let mut out = String::new();
        let mut first = true;
        for s in SKILLS.iter().filter(|s| !is_persona_skill(s.name)) {
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

/// Build the selected persona prompt, with fallback to default persona.
pub fn build_persona_prompt(selected: Option<&str>) -> String {
    let selected = selected.unwrap_or(DEFAULT_PERSONA_SKILL);

    if let Some(skill) = SKILLS.iter().find(|s| s.name == selected && is_persona_skill(s.name)) {
        return skill.content.to_string();
    }

    if let Some(skill) = SKILLS.iter().find(|s| s.name == DEFAULT_PERSONA_SKILL) {
        return skill.content.to_string();
    }

    String::new()
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
