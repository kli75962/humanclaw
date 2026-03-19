use serde::{Deserialize, Serialize};

/// What kind of device this is.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeviceType {
    Android,
    Desktop,
}

pub const DEFAULT_PERSONA: &str = "persona_default";

pub fn default_persona() -> String {
    DEFAULT_PERSONA.to_string()
}

/// Static info about this device.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeviceInfo {
    /// Randomly generated UUID, unique per installation.
    pub device_id: String,
    pub device_type: DeviceType,
    /// Human-readable label the user assigns (e.g. "My Phone", "Home PC").
    pub label: String,
}

/// A remote device that shares the same hash key and is listed as a peer.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PairedDevice {
    pub device_id: String,
    /// HTTP address the peer exposes for cross-device routing, e.g. "192.168.1.5:9876".
    pub address: String,
    pub label: String,
}

/// The full session config persisted locally on disk.
/// All devices that share the same `hash_key` are considered paired.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SessionConfig {
    pub device: DeviceInfo,
    /// 64-character random hex key shared across paired devices.
    pub hash_key: String,
    /// Addresses of known peer devices (populated manually or via sharing).
    pub paired_devices: Vec<PairedDevice>,
    /// Port this device listens on for cross-device bridge requests (default 9876).
    #[serde(default = "default_port")]
    pub bridge_port: u16,
    /// Optional manual override for the Ollama host/IP.
    /// When unset, the app uses platform defaults (desktop localhost or paired desktop on Android).
    #[serde(default)]
    pub ollama_host_override: Option<String>,
    /// Ollama API port (default 11434).
    #[serde(default = "default_ollama_port")]
    pub ollama_port: u16,
    /// Selected LLM persona skill name.
    #[serde(default = "default_persona")]
    pub persona: String,
}

fn default_port() -> u16 {
    9876
}

pub fn default_ollama_port() -> u16 {
    11434
}
