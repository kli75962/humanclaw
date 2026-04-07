pub mod sync;
pub mod commands;
pub mod delivery;
pub mod exec;
pub mod health;
pub mod server;
pub mod pairing_token;
pub mod types;

pub use health::*;
pub use server::*;
pub use commands::*;

use std::sync::OnceLock;
use reqwest::Client;

/// Shared bridge HTTP client.
pub(crate) fn bridge_client() -> &'static Client {
    static BRIDGE_CLIENT: OnceLock<Client> = OnceLock::new();
    BRIDGE_CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("failed to build bridge HTTP client")
    })
}
