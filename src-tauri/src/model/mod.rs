pub mod claude;
pub mod ollama;
pub(crate) mod shared;

use std::sync::atomic::{AtomicBool, Ordering};

pub use claude::chat_claude;
pub use ollama::{chat_ollama, list_models, list_models_at};

/// Set to true by `cancel_chat`. Reset at the start of each new chat run.
pub(crate) static CHAT_CANCEL: AtomicBool = AtomicBool::new(false);

/// Tauri command — signal the running chat loop to stop.
#[tauri::command]
pub fn cancel_chat() {
    CHAT_CANCEL.store(true, Ordering::Relaxed);
}
