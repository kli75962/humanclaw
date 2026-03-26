use std::path::PathBuf;
use std::sync::RwLock;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

use super::types::{default_ollama_port, default_persona, DeviceInfo, DeviceType, PairedDevice, PcPermissions, SessionConfig};

const SESSION_FILE: &str = "session.json";

/// In-memory cache for the session config.
/// Avoids re-reading session.json from disk on every call.
static CONFIG_CACHE: RwLock<Option<SessionConfig>> = RwLock::new(None);

// ── Path ─────────────────────────────────────────────────────────────────────

pub fn session_dir(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_default()
}

fn session_path(app: &AppHandle) -> PathBuf {
    session_dir(app).join(SESSION_FILE)
}

// ── Detect device type ───────────────────────────────────────────────────────

fn detect_device_type() -> DeviceType {
    #[cfg(target_os = "android")]
    return DeviceType::Android;

    #[cfg(not(target_os = "android"))]
    return DeviceType::Desktop;
}

// ── Load / Save ───────────────────────────────────────────────────────────────

/// Load session config from disk.  Returns `None` if file doesn't exist or
/// fails to parse (caller should call `bootstrap` to create a default).
pub fn load(app: &AppHandle) -> Option<SessionConfig> {
    // Check cache first
    if let Ok(guard) = CONFIG_CACHE.read() {
        if let Some(ref cfg) = *guard {
            return Some(cfg.clone());
        }
    }
    let bytes = std::fs::read(session_path(app)).ok()?;
    let cfg: SessionConfig = serde_json::from_slice(&bytes).ok()?;
    // Populate cache
    if let Ok(mut guard) = CONFIG_CACHE.write() {
        *guard = Some(cfg.clone());
    }
    Some(cfg)
}

/// Write session config to disk atomically (write-then-rename).
/// Also updates the in-memory cache.
pub fn save(app: &AppHandle, config: &SessionConfig) -> Result<(), String> {
    let path = session_path(app);
    let dir = session_dir(app);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;

    // Write to a temp file then rename so the file is never half-written.
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;

    // Update cache
    if let Ok(mut guard) = CONFIG_CACHE.write() {
        *guard = Some(config.clone());
    }
    Ok(())
}

/// Load or create a default session config.
/// Generates a cryptographically random hash key on first boot.
pub fn bootstrap(app: &AppHandle) -> SessionConfig {
    if let Some(cfg) = load(app) {
        return cfg;
    }
    // Two random UUID v4s concatenated in simple form = 64 lowercase hex chars.
    let hash_key = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let cfg = SessionConfig {
        device: DeviceInfo {
            device_id: Uuid::new_v4().to_string(),
            device_type: detect_device_type(),
            label: default_label(),
        },
        hash_key,
        paired_devices: Vec::new(),
        bridge_port: 9876,
        ollama_host_override: None,
        ollama_port: default_ollama_port(),
        persona: default_persona(),
        pc_permissions: PcPermissions::default(),
    };
    let _ = save(app, &cfg);
    cfg
}

fn default_label() -> String {
    #[cfg(target_os = "android")]
    return "My Phone".to_string();

    #[cfg(not(target_os = "android"))]
    // Use the HOSTNAME env var if available, otherwise fall back to "My PC".
    std::env::var("HOSTNAME").unwrap_or_else(|_| "My PC".to_string())
}

// ── Mutations ────────────────────────────────────────────────────────────────

/// Replace the session hash key directly.
/// Rejects anything that isn't a 64-character lowercase hex string.
pub fn set_hash_key(app: &AppHandle, hash_key: &str) -> Result<SessionConfig, String> {
    let hash_key = hash_key.trim();
    if hash_key.len() != 64 || !hash_key.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("Invalid hash key — must be the 64-character hex key shown in the app.".to_string());
    }
    let mut cfg = load(app).ok_or("Session not initialised")?;
    cfg.hash_key = hash_key.to_string();
    save(app, &cfg)?;
    Ok(cfg)
}

/// Update the device label.
pub fn set_label(app: &AppHandle, label: &str) -> Result<SessionConfig, String> {
    let mut cfg = load(app).ok_or("Session not initialised")?;
    cfg.device.label = label.to_string();
    save(app, &cfg)?;
    Ok(cfg)
}

/// Add or update a paired peer (matched by `device_id`).
pub fn upsert_peer(app: &AppHandle, peer: PairedDevice) -> Result<SessionConfig, String> {
    let mut cfg = load(app).ok_or("Session not initialised")?;
    if let Some(existing) = cfg.paired_devices.iter_mut().find(|p| p.device_id == peer.device_id) {
        *existing = peer;
    } else {
        cfg.paired_devices.push(peer);
    }
    save(app, &cfg)?;
    Ok(cfg)
}

/// Remove a paired peer by `device_id`.
pub fn remove_peer(app: &AppHandle, device_id: &str) -> Result<SessionConfig, String> {
    let mut cfg = load(app).ok_or("Session not initialised")?;
    cfg.paired_devices.retain(|p| p.device_id != device_id);
    save(app, &cfg)?;
    Ok(cfg)
}

/// Set the Ollama endpoint used for model listing and chat requests.
pub fn set_ollama_endpoint(app: &AppHandle, host: &str, port: u16) -> Result<SessionConfig, String> {
    let host = host.trim();
    if host.is_empty() {
        return Err("Host is required".to_string());
    }
    if host.contains(' ') || host.contains('/') {
        return Err("Host must be an IP or hostname only (no protocol/path)".to_string());
    }
    if port == 0 {
        return Err("Port must be between 1 and 65535".to_string());
    }

    let mut cfg = load(app).ok_or("Session not initialised")?;
    cfg.ollama_host_override = Some(host.to_string());
    cfg.ollama_port = port;
    save(app, &cfg)?;
    Ok(cfg)
}

/// Set the selected persona skill.
pub fn set_persona(app: &AppHandle, persona: &str) -> Result<SessionConfig, String> {
    let persona = persona.trim();
    if persona.is_empty() {
        return Err("Persona is required".to_string());
    }

    let compiled = crate::skills::persona_skill_names();
    let runtime = crate::skills::list_runtime_persona_names(app);
    let known_compiled = compiled.iter().any(|n| *n == persona);
    let known_runtime = runtime.iter().any(|n| n == persona);
    if !known_compiled && !known_runtime {
        return Err(format!("Unknown persona: {persona}"));
    }

    let mut cfg = load(app).ok_or("Session not initialised")?;
    cfg.persona = persona.to_string();
    save(app, &cfg)?;
    Ok(cfg)
}

/// Update PC control tool permissions.
pub fn set_pc_permissions(app: &AppHandle, permissions: PcPermissions) -> Result<SessionConfig, String> {
    let mut cfg = load(app).ok_or("Session not initialised")?;
    cfg.pc_permissions = permissions;
    save(app, &cfg)?;
    Ok(cfg)
}
