// Auto-generated agent registry (skills + tools) embedded at compile time.
include!(concat!(env!("OUT_DIR"), "/agent_registry.rs"));

use serde_json::Value;
use std::sync::OnceLock;

static SKILLS_PROMPT: OnceLock<String> = OnceLock::new();
static TOOL_SCHEMAS: OnceLock<Vec<Value>> = OnceLock::new();

/// All SKILL.md files joined as a single text block for the system prompt.
/// Computed once and cached for the lifetime of the process.
pub fn build_skills_prompt() -> &'static str {
    SKILLS_PROMPT.get_or_init(|| {
        let mut out = String::new();
        for (i, s) in SKILLS.iter().enumerate() {
            if i > 0 { out.push_str("\n\n---\n\n"); }
            out.push_str(s.content);
        }
        out
    })
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
