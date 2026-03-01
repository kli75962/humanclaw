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
        // Desktop stub — useful for development without a device
        vec![
            InstalledApp {
                name: "YouTube".into(),
                package_name: "com.google.android.youtube".into(),
                is_system: false,
            },
            InstalledApp {
                name: "Chrome".into(),
                package_name: "com.android.chrome".into(),
                is_system: false,
            },
        ]
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
