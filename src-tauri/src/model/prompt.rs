use chrono::Local;
use tauri::AppHandle;

use crate::phone::get_installed_apps;
use crate::skills::{build_persona_prompt_with_runtime, build_skills_prompt};
use crate::tools::{build_core_prompt, read_core};

use super::types::CharacterOverride;

/// Build the static part of the system prompt (persona + skills + installed apps + paired devices).
/// Core memory is injected separately each round via `prepare_system`.
/// If `character` is provided, the character's persona + identity replaces the session persona.
pub async fn build_base_prompt(app: &AppHandle, character: Option<&CharacterOverride>) -> String {
    let apps = get_installed_apps(app).await;
    let cfg = crate::session::store::bootstrap(app);
    let persona = if let Some(char) = character {
        let persona_content = build_persona_prompt_with_runtime(app, Some(char.persona.as_str()));
        format!("You are {}.\n{}\nBackground: {}", char.name, persona_content, char.background)
    } else {
        build_persona_prompt_with_runtime(app, Some(cfg.persona.as_str()))
    };
    let skills = build_skills_prompt();

    let mut buf = String::with_capacity(persona.len() + skills.len() + apps.len() * 60 + 256);
    buf.push_str(&persona);
    buf.push_str("\n\n");
    buf.push_str(&skills);

    if !apps.is_empty() {
        buf.push_str("\n\n[INSTALLED APPS]\n");
        for (i, a) in apps.iter().enumerate() {
            if i > 0 { buf.push('\n'); }
            buf.push_str(&a.prompt_line());
        }
    }

    use crate::session::types::DeviceType;
    match cfg.device.device_type {
        DeviceType::Desktop => {
            buf.push_str("Running on: Desktop PC.\n");
            buf.push_str("Available PC tools: system_run (primary), pc_open_url, pc_screenshot (verify only), pc_get_platform.\n");
            buf.push_str("Phone tools (tap, swipe, get_screen, etc.) require a paired Android device.");
        }
        DeviceType::Android => {
            buf.push_str("Running on: Android phone.\n");
            buf.push_str("Default to phone tools (tap, swipe, type_text, press_key, get_screen, launch_app) for local tasks.\n");
            buf.push_str("PC tools (pc_*) require a paired desktop device.");
        }
    }

    if !cfg.paired_devices.is_empty() {
        buf.push_str("\n\n[PAIRED DEVICES]\n");
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
