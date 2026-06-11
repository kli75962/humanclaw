use std::path::PathBuf;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use tauri::{AppHandle, Manager};

use super::types::ToolResult;

// ── Community repo config ─────────────────────────────────────────────────────
// Replace these with your actual GitHub repo and Cloudflare Worker URL after setup.
const GITHUB_RAW_BASE: &str = "https://raw.githubusercontent.com/kli75962/phoneclaw-gesture_maps/main";
const WORKER_BASE: &str = "https://phoneclaw-gesture-maps.kli75962.workers.dev";

// ── HTTP client ───────────────────────────────────────────────────────────────

fn cloud_client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .user_agent("phoneclaw-gesture-map/1.0")
            .build()
            .expect("failed to build gesture map HTTP client")
    })
}

// ── Data structures ───────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GestureEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fx: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fy: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub delay_ms: u64,
    #[serde(default)]
    pub wait_for_screen_change: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_wait_ms: Option<u64>,
    // fill_credential fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_package: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GestureMap {
    pub schema_version: u32,
    pub name: String,
    pub app_package: String,
    #[serde(default)]
    pub recorded_app_version: String,
    pub created_at: String,
    #[serde(default)]
    pub last_verified_at: String,
    #[serde(default)]
    pub verified_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub screen_width_px: u32,
    pub screen_height_px: u32,
    pub events: Vec<GestureEvent>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GestureMapSummary {
    pub name: String,
    #[serde(default)]
    pub recorded_app_version: String,
    #[serde(default)]
    pub verified_count: u32,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub description: Option<String>,
    pub source: String, // "local" | "cloud"
}

#[derive(Serialize, Deserialize, Default)]
pub struct GestureMapSettings {
    pub share_enabled: bool,
}

// ── Cloud index format ────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct CloudIndex {
    maps: Vec<CloudIndexEntry>,
}

#[derive(Serialize, Deserialize)]
struct CloudIndexEntry {
    name: String,
    #[serde(default)]
    recorded_app_version: String,
    #[serde(default)]
    verified_count: u32,
    #[serde(default)]
    created_at: String,
    #[serde(default)]
    description: Option<String>,
}

// ── Path helpers ──────────────────────────────────────────────────────────────

pub fn gesture_map_dir(app: &AppHandle, app_package: &str) -> PathBuf {
    app.path().app_data_dir().unwrap_or_default()
        .join("gesture_maps")
        .join(sanitize_pkg(app_package))
}

fn sanitize_pkg(pkg: &str) -> String {
    pkg.chars()
        .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '_')
        .collect()
}

fn sanitize_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect()
}

fn map_path(app: &AppHandle, pkg: &str, name: &str) -> PathBuf {
    gesture_map_dir(app, pkg).join(format!("{}.json", sanitize_name(name)))
}

fn settings_path(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_default()
        .join("gesture_map_settings.json")
}

// ── Settings ──────────────────────────────────────────────────────────────────

pub fn load_settings(app: &AppHandle) -> GestureMapSettings {
    let path = settings_path(app);
    std::fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub fn save_settings(app: &AppHandle, settings: &GestureMapSettings) -> Result<(), String> {
    let path = settings_path(app);
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

// ── Local I/O ─────────────────────────────────────────────────────────────────

pub fn save_gesture_map(app: &AppHandle, map: &GestureMap) -> Result<(), String> {
    let dir = gesture_map_dir(app, &map.app_package);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = map_path(app, &map.app_package, &map.name);
    let json = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

pub fn load_gesture_map(app: &AppHandle, pkg: &str, name: &str) -> Option<GestureMap> {
    let path = map_path(app, pkg, name);
    let bytes = std::fs::read(&path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn list_gesture_maps_local(app: &AppHandle, pkg: &str) -> Vec<GestureMapSummary> {
    let dir = gesture_map_dir(app, pkg);
    let Ok(entries) = std::fs::read_dir(&dir) else { return vec![] };
    entries
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if path.extension()?.to_str()? != "json" { return None; }
            let bytes = std::fs::read(&path).ok()?;
            let map: GestureMap = serde_json::from_slice(&bytes).ok()?;
            Some(GestureMapSummary {
                name: map.name,
                recorded_app_version: map.recorded_app_version,
                verified_count: map.verified_count,
                created_at: map.created_at,
                description: map.description,
                source: "local".to_string(),
            })
        })
        .collect()
}

pub fn delete_gesture_map(app: &AppHandle, pkg: &str, name: &str) -> bool {
    let path = map_path(app, pkg, name);
    std::fs::remove_file(&path).is_ok()
}

pub fn increment_verified_count_local(app: &AppHandle, pkg: &str, name: &str) {
    let path = map_path(app, pkg, name);
    let Ok(bytes) = std::fs::read(&path) else { return };
    let Ok(mut map): Result<GestureMap, _> = serde_json::from_slice(&bytes) else { return };
    map.verified_count += 1;
    map.last_verified_at = chrono_now();
    let Ok(json) = serde_json::to_string_pretty(&map) else { return };
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, json).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Simple ISO 8601 without external dep
    let s = secs;
    let mins = s / 60; let secs_r = s % 60;
    let hrs = mins / 60; let mins_r = mins % 60;
    let days = hrs / 24; let hrs_r = hrs % 24;
    // days since epoch to date (approximate, good enough for a timestamp label)
    let year = 1970 + days / 365;
    let month_day = days % 365;
    let month = month_day / 30 + 1;
    let day = month_day % 30 + 1;
    format!("{year:04}-{month:02}-{day:02}T{hrs_r:02}:{mins_r:02}:{secs_r:02}Z")
}

// ── Cloud API ─────────────────────────────────────────────────────────────────

pub async fn fetch_index_from_raw(pkg: &str) -> Vec<GestureMapSummary> {
    let url = format!(
        "{}/{}/index.json",
        GITHUB_RAW_BASE,
        sanitize_pkg(pkg)
    );
    let Ok(resp) = cloud_client().get(&url).send().await else { return vec![] };
    if !resp.status().is_success() { return vec![]; }
    let Ok(index): Result<CloudIndex, _> = resp.json().await else { return vec![] };
    index.maps.into_iter().map(|e| GestureMapSummary {
        name: e.name,
        recorded_app_version: e.recorded_app_version,
        verified_count: e.verified_count,
        created_at: e.created_at,
        description: e.description,
        source: "cloud".to_string(),
    }).collect()
}

pub async fn download_map_from_raw(app: &AppHandle, pkg: &str, name: &str) -> Option<GestureMap> {
    let url = format!(
        "{}/{}/{}.json",
        GITHUB_RAW_BASE,
        sanitize_pkg(pkg),
        sanitize_name(name)
    );
    let Ok(resp) = cloud_client().get(&url).send().await else { return None };
    if !resp.status().is_success() { return None; }
    let Ok(map): Result<GestureMap, _> = resp.json().await else { return None };
    // Cache locally
    let _ = save_gesture_map(app, &map);
    Some(map)
}

pub async fn upload_map_to_worker(pkg: &str, name: &str, map: &GestureMap) {
    let url = format!("{}/maps/{}/{}", WORKER_BASE, sanitize_pkg(pkg), sanitize_name(name));
    let Ok(body) = serde_json::to_value(map) else { return };
    let _ = cloud_client()
        .post(&url)
        .json(&serde_json::json!({ "data": body }))
        .send()
        .await;
}

pub async fn verify_map_on_worker(pkg: &str, name: &str) {
    let url = format!("{}/maps/{}/{}/verify", WORKER_BASE, sanitize_pkg(pkg), sanitize_name(name));
    let _ = cloud_client().post(&url).send().await;
}

pub async fn increment_verified_count(app: &AppHandle, pkg: &str, name: &str) {
    increment_verified_count_local(app, pkg, name);
    verify_map_on_worker(pkg, name).await;
}

// ── Local packages with maps (for system prompt) ──────────────────────────────


// ── Tool routing ──────────────────────────────────────────────────────────────

pub fn is_gesture_map_tool(name: &str) -> bool {
    matches!(
        name,
        "search_gesture_maps"
            | "replay_gesture_map"
            | "start_gesture_recording"
            | "stop_gesture_recording"
    )
}

pub async fn execute_gesture_map_tool(
    app: &AppHandle,
    name: &str,
    args: &serde_json::Value,
) -> ToolResult {
    match name {
        "search_gesture_maps" => search_gesture_maps(app, args).await,
        "replay_gesture_map"  => replay_gesture_map(app, args).await,
        "start_gesture_recording" => start_gesture_recording(app).await,
        "stop_gesture_recording"  => stop_gesture_recording(app, args).await,
        _ => ToolResult::err(name, "NOT_FOUND", format!("Unknown gesture map tool: {name}")),
    }
}

// ── Tool implementations ──────────────────────────────────────────────────────

async fn search_gesture_maps(app: &AppHandle, args: &serde_json::Value) -> ToolResult {
    let pkg = args.get("app_package").and_then(|v| v.as_str()).unwrap_or("");
    if pkg.is_empty() {
        return ToolResult::err("search_gesture_maps", "INVALID_ARGS", "Missing app_package");
    }

    // Step 1: local
    let local = list_gesture_maps_local(app, pkg);
    if !local.is_empty() {
        return format_summary("search_gesture_maps", pkg, local);
    }

    // Step 2: cloud (only if local is empty)
    let cloud = fetch_index_from_raw(pkg).await;
    if cloud.is_empty() {
        return ToolResult::ok(
            "search_gesture_maps",
            format!(
                "No gesture maps found for {pkg}.\n\
                 Use start_gesture_recording() to record the screen actions, \
                 then stop_gesture_recording(app_package, name, description) to save."
            ),
        );
    }
    format_summary("search_gesture_maps", pkg, cloud)
}

fn format_summary(tool: &str, pkg: &str, maps: Vec<GestureMapSummary>) -> ToolResult {
    let mut out = format!("Gesture maps for {pkg}:\n");
    for m in &maps {
        out.push_str(&format!(
            "- {} [v{}, verified: {}, source: {}]",
            m.name, m.recorded_app_version, m.verified_count, m.source
        ));
        if let Some(desc) = &m.description {
            out.push_str(&format!(" — {desc}"));
        }
        out.push('\n');
    }
    out.push_str("\nUse replay_gesture_map(app_package, name) to execute one.");
    ToolResult::ok(tool, out)
}

async fn replay_gesture_map(app: &AppHandle, args: &serde_json::Value) -> ToolResult {
    let pkg  = args.get("app_package").and_then(|v| v.as_str()).unwrap_or("");
    let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
    if pkg.is_empty() || name.is_empty() {
        return ToolResult::err("replay_gesture_map", "INVALID_ARGS", "Missing app_package or name");
    }

    // Load local, or download from cloud
    let map = if let Some(m) = load_gesture_map(app, pkg, name) {
        m
    } else {
        match download_map_from_raw(app, pkg, name).await {
            Some(m) => m,
            None => return ToolResult::err(
                "replay_gesture_map",
                "NOT_FOUND",
                format!(
                    "Gesture map '{name}' not found for {pkg}. \
                     Use search_gesture_maps to list available maps."
                ),
            ),
        }
    };

    let events_json = match serde_json::to_string(&map.events) {
        Ok(j) => j,
        Err(e) => return ToolResult::err("replay_gesture_map", "EXECUTION_FAILED", e.to_string()),
    };

    let replay_args = serde_json::json!({
        "tool": "replay_gesture_map",
        "args": {
            "events_json":    events_json,
            "screen_width":   map.screen_width_px,
            "screen_height":  map.screen_height_px,
        }
    });

    let result = call_phone_tool(app, "replay_gesture_map", &replay_args).await;
    if result.success {
        // Fire-and-forget: increment verified count
        let app2  = app.clone();
        let pkg2  = pkg.to_string();
        let name2 = name.to_string();
        tauri::async_runtime::spawn(async move {
            increment_verified_count(&app2, &pkg2, &name2).await;
        });
    }
    result
}

async fn start_gesture_recording(app: &AppHandle) -> ToolResult {
    // Ask user for share preference before starting — blocks until user responds.
    let answers = crate::tools::ask_user::request_ask_user(
        app,
        &serde_json::json!([{
            "id": 0,
            "question": "Share this gesture recording with the community? Community recordings help all users automate apps that block screen reading.",
            "options": ["Yes, share with community", "No, keep private"]
        }]),
    ).await;
    let share = answers.get(&0).map(|s| s.as_str()) != Some("No, keep private");

    // Save the chosen preference for future recordings.
    let mut settings = load_settings(app);
    settings.share_enabled = share;
    let _ = save_settings(app, &settings);

    let phone_args = serde_json::json!({
        "tool": "start_gesture_recording",
        "args": { "is_sharing_mode": share }
    });
    let result = call_phone_tool(app, "start_gesture_recording", &phone_args).await;

    // Notify frontend to show the recording UI with a stop button.
    if result.success {
        use tauri::Emitter;
        app.emit("gesture-recording-started", serde_json::json!({})).ok();
    }

    result
}

async fn stop_gesture_recording(app: &AppHandle, args: &serde_json::Value) -> ToolResult {
    let pkg         = args.get("app_package").and_then(|v| v.as_str()).unwrap_or("");
    let name        = args.get("name").and_then(|v| v.as_str()).unwrap_or("unnamed");
    let description = args.get("description").and_then(|v| v.as_str()).map(|s| s.to_string());

    if pkg.is_empty() {
        return ToolResult::err("stop_gesture_recording", "INVALID_ARGS", "Missing app_package");
    }

    let phone_args = serde_json::json!({ "tool": "stop_gesture_recording", "args": {} });
    let raw = call_phone_tool(app, "stop_gesture_recording", &phone_args).await;
    if !raw.success {
        return raw;
    }

    // Kotlin returns a JSON object in output: { events, screen_width, screen_height, recorded_app_version, has_credential_events }
    let parsed: serde_json::Value = match serde_json::from_str(&raw.output) {
        Ok(v) => v,
        Err(e) => return ToolResult::err("stop_gesture_recording", "INVALID_ARGS",
            format!("Failed to parse recording output: {e}")),
    };

    let events: Vec<GestureEvent> = match parsed.get("events")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
    {
        Some(e) => e,
        None => return ToolResult::err("stop_gesture_recording", "INVALID_ARGS", "No events in recording output"),
    };

    let screen_width  = parsed.get("screen_width").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let screen_height = parsed.get("screen_height").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let recorded_app_version = parsed.get("recorded_app_version")
        .and_then(|v| v.as_str()).unwrap_or("").to_string();
    let has_credential_events = parsed.get("has_credential_events")
        .and_then(|v| v.as_bool()).unwrap_or(false);

    let event_count = events.len();
    let now = chrono_now();
    let map = GestureMap {
        schema_version: 1,
        name: name.to_string(),
        app_package: pkg.to_string(),
        recorded_app_version,
        created_at: now.clone(),
        last_verified_at: now,
        verified_count: 0,
        description,
        screen_width_px: screen_width,
        screen_height_px: screen_height,
        events,
    };

    if let Err(e) = save_gesture_map(app, &map) {
        return ToolResult::err("stop_gesture_recording", "EXECUTION_FAILED", e);
    }

    // Upload to cloud if sharing is enabled
    let settings = load_settings(app);
    if settings.share_enabled {
        let map2 = map.clone();
        let pkg2 = pkg.to_string();
        let name2 = name.to_string();
        tauri::async_runtime::spawn(async move {
            upload_map_to_worker(&pkg2, &name2, &map2).await;
        });
    }

    let credential_note = if has_credential_events {
        " (credential fields auto-converted to fill_credential events — no passwords stored)"
    } else {
        ""
    };

    ToolResult::ok(
        "stop_gesture_recording",
        format!(
            "Saved gesture map '{name}' for {pkg} with {event_count} events{credential_note}. \
             Use replay_gesture_map(app_package=\"{pkg}\", name=\"{name}\") next time this screen appears."
        ),
    )
}

// ── Phone tool bridge ─────────────────────────────────────────────────────────

async fn call_phone_tool(app: &AppHandle, tool_name: &str, payload: &serde_json::Value) -> ToolResult {
    #[cfg(target_os = "android")]
    {
        use crate::device::phone::plugin::PhoneControlHandle;
        let handle = app.state::<PhoneControlHandle<tauri::Wry>>();
        match handle.0.run_mobile_plugin::<ToolResult>("executeTool", payload.clone()) {
            Ok(r) => r,
            Err(e) => ToolResult::err(tool_name, "PLUGIN_ERROR", format!("Plugin error: {e}")),
        }
    }

    #[cfg(not(target_os = "android"))]
    {
        use crate::network::types::ToolResponse;
        use crate::session::store;

        let cfg = store::bootstrap(app);
        let Some(peer) = cfg.paired_devices.first() else {
            return ToolResult::err(tool_name, "DEVICE_NOT_FOUND", "No paired Android device.");
        };

        let url = format!("http://{}/tool", peer.address);
        let body = serde_json::json!({
            "key":       cfg.hash_key,
            "tool_name": payload.get("tool").and_then(|v| v.as_str()).unwrap_or(tool_name),
            "tool_args": payload.get("args").cloned().unwrap_or(serde_json::json!({})),
        });

        let client = crate::network::bridge_client();
        match client.post(&url).json(&body).send().await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<ToolResponse>().await {
                    Ok(r) => ToolResult { tool_name: tool_name.to_string(), success: r.success, output: r.output, error_code: None },
                    Err(e) => ToolResult::err(tool_name, "INVALID_RESPONSE", format!("Invalid response: {e}")),
                }
            }
            Ok(resp) => ToolResult::err(tool_name, "DEVICE_ERROR", format!("Phone returned {}", resp.status())),
            Err(e)   => ToolResult::err(tool_name, "DEVICE_UNREACHABLE", format!("Could not reach phone: {e}")),
        }
    }
}

// ── Tauri UI commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_gesture_maps_cmd(app: tauri::AppHandle, app_package: String) -> Vec<GestureMapSummary> {
    list_gesture_maps_local(&app, &app_package)
}

#[tauri::command]
pub fn delete_gesture_map_cmd(app: tauri::AppHandle, app_package: String, name: String) -> bool {
    delete_gesture_map(&app, &app_package, &name)
}

#[tauri::command]
pub fn get_gesture_share_setting(app: tauri::AppHandle) -> bool {
    load_settings(&app).share_enabled
}

#[tauri::command]
pub fn save_gesture_share_setting(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let mut settings = load_settings(&app);
    settings.share_enabled = enabled;
    save_settings(&app, &settings)
}
