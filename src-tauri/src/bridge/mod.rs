pub mod commands;
pub mod chat_sync;
pub mod character_sync;
pub mod exec;
pub mod health;
pub mod pairing_token;
pub mod server;
pub mod types;

pub use commands::{check_peer_online, discover_and_pair, get_all_local_addresses, get_all_peer_status, get_local_address, get_qr_pair_svg, pair_from_qr, send_to_device};
pub use server::start_bridge_server;
pub use health::start_peer_monitor;

// ── Shared HTTP client for bridge operations (pairing, sync, memory sync) ────
use std::sync::OnceLock;

static BRIDGE_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// Short-timeout client for bridge probes, pairing, and sync requests.
pub fn bridge_client() -> &'static reqwest::Client {
    BRIDGE_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(8))
            .build()
            .expect("failed to build bridge HTTP client")
    })
}
