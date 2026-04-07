use tauri::AppHandle;

use crate::chat::{read_memory_file, CORE_FILE};

/// Read `core.md`. Returns an empty string if the file is missing.
pub fn read_core(app: &AppHandle) -> String {
    read_memory_file(app, CORE_FILE).unwrap_or_default()
}

/// Wrap core.md content for injection into the system prompt.
pub fn build_core_prompt(core: &str) -> String {
    let trimmed = core.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    format!("[CORE MEMORY - always apply these facts in your responses]\n{trimmed}")
}
