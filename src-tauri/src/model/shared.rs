use chrono::Local;
use serde::Serialize;
use tauri::AppHandle;

use crate::phone::get_installed_apps;
use crate::skills::{build_persona_prompt, build_skills_prompt};
use crate::tools::{build_core_prompt, read_core};

pub const MAX_TOOL_ROUNDS: usize = 200;

/// Payload emitted via the `ollama-stream` Tauri event for every token.
#[derive(Clone, Serialize)]
pub struct StreamPayload {
    pub content: String,
    pub done: bool,
}

/// Status update emitted while the agent is executing tools.
#[derive(Clone, Serialize)]
pub struct AgentStatusPayload {
    pub message: String,
}

/// Build the static part of the system prompt (persona + skills + installed apps + paired devices).
/// Core memory is injected separately each round via `prepare_system`.
pub async fn build_base_prompt(app: &AppHandle) -> String {
    let apps = get_installed_apps(app).await;
    let cfg = crate::session::store::bootstrap(app);
    let persona = build_persona_prompt(Some(cfg.persona.as_str()));
    let skills = build_skills_prompt();

    let mut buf = String::with_capacity(persona.len() + skills.len() + apps.len() * 60 + 256);
    buf.push_str(&persona);
    buf.push_str("\n\n");
    buf.push_str(&skills);
    buf.push_str("\n\n[INSTALLED APPS]\n");
    if apps.is_empty() {
        buf.push_str("  (no apps found)");
    } else {
        for (i, a) in apps.iter().enumerate() {
            if i > 0 { buf.push('\n'); }
            buf.push_str(&a.prompt_line());
        }
    }

    if !cfg.paired_devices.is_empty() {
        buf.push_str("\n\n[PAIRED DEVICES]\n");
        buf.push_str("Phone tools (tap, swipe, get_screen, etc.) are forwarded to the paired Android device automatically.\n");
        for p in &cfg.paired_devices {
            buf.push_str("- ");
            buf.push_str(&p.label);
            buf.push_str(" (");
            buf.push_str(&p.device_id);
            buf.push_str(")\n");
        }
    }

    buf
}

/// Assemble the full system prompt for one LLM round.
/// Re-reads core.md fresh each round so mid-session edits take effect immediately.
pub fn prepare_system(app: &AppHandle, base: &str) -> String {
    let core = read_core(app);
    let core_block = build_core_prompt(&core);
    let now = Local::now().format("%Y-%m-%d %H:%M:%S %Z").to_string();
    let datetime_block = format!("[CURRENT DATETIME]\n{now}");
    if core_block.is_empty() {
        format!("{datetime_block}\n\n{base}")
    } else {
        format!("{datetime_block}\n\n{core_block}\n\n{base}")
    }
}

/// Returns true if the user has signalled cancellation (frontend stop or Android overlay button).
pub fn should_cancel(app: &AppHandle) -> bool {
    crate::model::CHAT_CANCEL.load(std::sync::atomic::Ordering::Relaxed)
        || crate::phone::is_cancelled(app)
}
