pub mod capture;
pub mod commands;
pub mod transcribe;
pub mod types;

pub use commands::{stt_start, stt_stop};

use std::sync::OnceLock;

static STT_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

pub fn stt_client() -> &'static reqwest::Client {
    STT_CLIENT.get_or_init(reqwest::Client::new)
}
