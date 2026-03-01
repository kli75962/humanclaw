mod memory;
mod ollama;
mod phone;
mod skills;

use memory::{clear_knowledge_cmd, clear_memories_cmd, delete_knowledge_cmd, delete_memory_cmd, get_knowledge, get_memories};
use ollama::{chat_ollama, list_models};

/// App entry point — registers Tauri commands and starts the event loop.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(phone::plugin::init())
        .invoke_handler(tauri::generate_handler![
            chat_ollama,
            list_models,
            get_memories,
            delete_memory_cmd,
            clear_memories_cmd,
            get_knowledge,
            delete_knowledge_cmd,
            clear_knowledge_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

