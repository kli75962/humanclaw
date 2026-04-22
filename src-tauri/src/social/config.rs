use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::AppHandle;
use crate::chat::memory_dir;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SocialConfig {
    pub comment_follow_through_base_pct: u8,
    pub comment_follow_through_scale_pct: u8,
    pub thread_reply_base_pct: u8,
    pub thread_reply_scale_pct: u8,
    pub dm_sociability_threshold: u8,
    pub dm_scale_divisor: u8,
    pub max_posts_high_sociability: u8,
    pub max_posts_medium_sociability: u8,
    pub max_posts_low_sociability: u8,
    pub rag_max_results: u8,
}

impl Default for SocialConfig {
    fn default() -> Self {
        Self {
            comment_follow_through_base_pct: 15,
            comment_follow_through_scale_pct: 50,
            thread_reply_base_pct: 30,
            thread_reply_scale_pct: 50,
            dm_sociability_threshold: 50,
            dm_scale_divisor: 9,
            max_posts_high_sociability: 3,
            max_posts_medium_sociability: 2,
            max_posts_low_sociability: 1,
            rag_max_results: 3,
        }
    }
}

pub fn config_path(app: &AppHandle) -> PathBuf {
    memory_dir(app).join("social_config.json")
}

pub fn load_config(app: &AppHandle) -> SocialConfig {
    let path = config_path(app);
    if let Ok(text) = std::fs::read_to_string(&path) {
        if let Ok(config) = serde_json::from_str(&text) {
            return config;
        }
    }
    // If it doesn't exist or is invalid, create and return default
    let default_config = SocialConfig::default();
    let _ = save_config(app, &default_config);
    default_config
}

pub fn save_config(app: &AppHandle, config: &SocialConfig) -> Result<(), String> {
    let path = config_path(app);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_social_config(app: AppHandle) -> SocialConfig {
    load_config(&app)
}

#[tauri::command]
pub fn save_social_config(app: AppHandle, config: SocialConfig) -> Result<(), String> {
    save_config(&app, &config)
}
