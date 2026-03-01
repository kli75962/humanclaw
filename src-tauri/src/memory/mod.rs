mod extract;
mod extract_knowledge;
mod knowledge;
mod store;

pub use extract::extract_memories;
pub use extract_knowledge::extract_knowledge;
pub use knowledge::{add_knowledge, build_knowledge_prompt, clear_knowledge, delete_knowledge, load_knowledge, NavKnowledge};
pub use store::{add_memories, build_memory_prompt, clear_memories, delete_memory, load_memories, Memory};

// ----- Tauri commands exposed to the frontend -----

/// Return all stored personal preference memories.
#[tauri::command]
pub fn get_memories(app: tauri::AppHandle) -> Vec<Memory> {
    load_memories(&app)
}

/// Delete a single personal memory by its ID.
#[tauri::command]
pub fn delete_memory_cmd(app: tauri::AppHandle, id: String) {
    delete_memory(&app, &id);
}

/// Wipe all personal memories.
#[tauri::command]
pub fn clear_memories_cmd(app: tauri::AppHandle) {
    clear_memories(&app);
}

/// Return all stored navigation knowledge entries.
#[tauri::command]
pub fn get_knowledge(app: tauri::AppHandle) -> Vec<NavKnowledge> {
    load_knowledge(&app)
}

/// Delete a single navigation knowledge entry by its ID.
#[tauri::command]
pub fn delete_knowledge_cmd(app: tauri::AppHandle, id: String) {
    delete_knowledge(&app, &id);
}

/// Wipe all navigation knowledge.
#[tauri::command]
pub fn clear_knowledge_cmd(app: tauri::AppHandle) {
    clear_knowledge(&app);
}
