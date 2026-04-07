/// Daily post scheduling: LLM decides posting times once per day (stored per character).
/// On app open, due slots are returned to the frontend for generation.
///
/// Storage: {app_data}/.memory/post_schedule/{character_id}.json
use chrono::{Local, NaiveTime, TimeZone};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::AppHandle;

use crate::chat::memory_dir;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DaySchedule {
    pub character_id: String,
    /// YYYY-MM-DD in local time
    pub date: String,
    /// Sorted HH:MM times (24-hour) chosen by LLM
    pub times: Vec<String>,
    /// HH:MM times that have already been generated
    pub generated: Vec<String>,
}

// ── Paths ──────────────────────────────────────────────────────────────────────

fn schedule_dir(app: &AppHandle) -> PathBuf {
    memory_dir(app).join("post_schedule")
}

fn schedule_path(app: &AppHandle, character_id: &str) -> PathBuf {
    schedule_dir(app).join(format!("{character_id}.json"))
}

// ── I/O ───────────────────────────────────────────────────────────────────────

pub fn load(app: &AppHandle, character_id: &str) -> Option<DaySchedule> {
    let text = std::fs::read_to_string(schedule_path(app, character_id)).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn save(app: &AppHandle, schedule: &DaySchedule) -> Result<(), String> {
    let dir = schedule_dir(app);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string(schedule).map_err(|e| e.to_string())?;
    std::fs::write(schedule_path(app, &schedule.character_id), json).map_err(|e| e.to_string())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

pub fn today_str() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

/// Return HH:MM strings that are due (≤ current time) and not yet generated.
pub fn due_times(schedule: &DaySchedule) -> Vec<String> {
    let now_hhmm = Local::now().format("%H:%M").to_string();
    schedule
        .times
        .iter()
        .filter(|t| t.as_str() <= now_hhmm.as_str() && !schedule.generated.contains(t))
        .cloned()
        .collect()
}

/// Convert a "HH:MM" string for today into a full RFC 3339 datetime string.
pub fn hhmm_to_rfc3339_today(hhmm: &str) -> Option<String> {
    let naive_time = NaiveTime::parse_from_str(hhmm, "%H:%M").ok()?;
    let today = Local::now().date_naive();
    let naive_dt = today.and_time(naive_time);
    let local_dt = Local.from_local_datetime(&naive_dt).single()?;
    Some(local_dt.to_rfc3339())
}

/// Mark a time slot as generated. No-op if already marked.
pub fn mark_generated(app: &AppHandle, character_id: &str, time_str: &str) -> Result<(), String> {
    let mut sched = load(app, character_id).ok_or("No schedule found")?;
    if !sched.generated.contains(&time_str.to_string()) {
        sched.generated.push(time_str.to_string());
        save(app, &sched)?;
    }
    Ok(())
}

/// Fallback times when LLM is unavailable, based on sociability score.
pub fn fallback_times(sociability: u8) -> Vec<String> {
    match sociability {
        71..=100 => vec!["09:30".to_string(), "20:00".to_string()],
        41..=70  => vec!["15:30".to_string()],
        _        => vec!["21:00".to_string()],
    }
}
