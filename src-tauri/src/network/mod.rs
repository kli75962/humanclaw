pub mod sync;
pub mod commands;
pub mod delivery;
pub mod exec;
pub mod health;
pub mod server;
pub mod pairing_token;
pub mod sse;
pub mod sse_subscriber;
pub mod types;

pub use health::*;
pub use server::*;
pub use commands::*;

use std::sync::OnceLock;
use reqwest::Client;

/// Shared bridge HTTP client — short timeout, used for health checks and small requests.
pub(crate) fn bridge_client() -> &'static Client {
    static BRIDGE_CLIENT: OnceLock<Client> = OnceLock::new();
    BRIDGE_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("failed to build bridge HTTP client")
    })
}

/// Long-lived streaming client for the Ollama proxy — no overall timeout so LLM
/// responses can stream freely; only a connect timeout to catch dead hosts quickly.
pub(crate) fn ollama_proxy_client() -> &'static Client {
    static PROXY_CLIENT: OnceLock<Client> = OnceLock::new();
    PROXY_CLIENT.get_or_init(|| {
        Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("failed to build ollama proxy HTTP client")
    })
}
