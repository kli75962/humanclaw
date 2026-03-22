pub mod chat;
pub mod headless;
pub mod models;
pub mod types;

pub use chat::chat_ollama;
pub use models::{list_models, list_models_at};

use std::sync::OnceLock;

static OLLAMA_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

pub fn ollama_client() -> &'static reqwest::Client {
    OLLAMA_CLIENT.get_or_init(reqwest::Client::new)
}
