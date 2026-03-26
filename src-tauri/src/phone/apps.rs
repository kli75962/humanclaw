use serde::{Deserialize, Serialize};
use tauri::AppHandle;
#[cfg(target_os = "android")]
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledApp {
    pub name: String,
    pub package_name: String,
    pub is_system: bool,
}

impl InstalledApp {
    /// Format entries into a compact list line for the system prompt.
    pub fn prompt_line(&self) -> String {
        format!("- {} ({})", self.name, self.package_name)
    }
}

/// Fetch all installed apps.
/// On Android this calls the Kotlin PhoneControlPlugin; on desktop returns an empty list.
pub async fn get_installed_apps(_app: &AppHandle) -> Vec<InstalledApp> {
    #[cfg(target_os = "android")]
    {
        fetch_from_plugin(_app).await.unwrap_or_default()
    }

    #[cfg(not(target_os = "android"))]
    {
        vec![]
    }
}

#[cfg(target_os = "android")]
async fn fetch_from_plugin(app: &AppHandle) -> Result<Vec<InstalledApp>, String> {
    use serde_json::json;
    use crate::phone::plugin::PhoneControlHandle;
    let handle = app.state::<PhoneControlHandle<tauri::Wry>>();
    handle
        .0
        .run_mobile_plugin::<Vec<InstalledApp>>("getInstalledApps", json!({}))
        .map_err(|e| e.to_string())
}

/// Check whether the PhoneClaw accessibility service is enabled.
/// Returns true on non-Android builds (desktop stub).
#[tauri::command]
pub async fn check_accessibility_enabled(app: AppHandle) -> bool {
    #[cfg(target_os = "android")]
    {
        check_accessibility_from_plugin(&app).await.unwrap_or(false)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        true
    }
}

/// Open Android Accessibility Settings.
/// No-op on non-Android builds.
#[tauri::command]
pub async fn open_accessibility_settings(app: AppHandle) {
    #[cfg(target_os = "android")]
    {
        let _ = open_settings_from_plugin(&app).await;
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
    }
}

#[cfg(target_os = "android")]
async fn check_accessibility_from_plugin(app: &AppHandle) -> Result<bool, String> {
    use serde_json::json;
    use crate::phone::plugin::PhoneControlHandle;
    #[derive(serde::Deserialize)]
    struct Resp {
        enabled: bool,
    }
    let handle = app.state::<PhoneControlHandle<tauri::Wry>>();
    handle
        .0
        .run_mobile_plugin::<Resp>("checkAccessibility", json!({}))
        .map(|r| r.enabled)
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "android")]
async fn open_settings_from_plugin(app: &AppHandle) -> Result<(), String> {
    use serde_json::json;
    use crate::phone::plugin::PhoneControlHandle;
    #[derive(serde::Deserialize)]
    struct Resp {}
    let handle = app.state::<PhoneControlHandle<tauri::Wry>>();
    handle
        .0
        .run_mobile_plugin::<Resp>("openAccessibilitySettings", json!({}))
        .map(|_| ())
        .map_err(|e| e.to_string())
}
