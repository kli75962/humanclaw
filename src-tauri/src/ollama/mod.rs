pub mod chat;
pub mod models;
pub mod types;

// Re-export the Tauri commands so lib.rs can register them directly
pub use chat::chat_ollama;
pub use models::list_models;
