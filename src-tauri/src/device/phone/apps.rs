use tauri::AppHandle;
#[cfg(target_os = "android")]
use tauri::Manager;



#[cfg(target_os = "android")]
async fn fetch_from_plugin(app: &AppHandle) -> Result<Vec<InstalledApp>, String> {
    use serde_json::json;
    use crate::device::phone::plugin::PhoneControlHandle;

    #[derive(serde::Deserialize)]
    struct AppsResp { apps: Vec<InstalledApp> }

    let handle = app.state::<PhoneControlHandle<tauri::Wry>>();
    handle
        .0
        .run_mobile_plugin_async::<AppsResp>("getInstalledApps", json!({}))
        .await
        .map(|r| r.apps)
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
    use crate::device::phone::plugin::PhoneControlHandle;
    #[derive(serde::Deserialize)]
    struct Resp {
        enabled: bool,
    }
    let handle = app.state::<PhoneControlHandle<tauri::Wry>>();
    handle
        .0
        .run_mobile_plugin_async::<Resp>("checkAccessibility", json!({}))
        .await
        .map(|r| r.enabled)
        .map_err(|e| e.to_string())
}

#[cfg(target_os = "android")]
async fn open_settings_from_plugin(app: &AppHandle) -> Result<(), String> {
    use serde_json::json;
    use crate::device::phone::plugin::PhoneControlHandle;
    #[derive(serde::Deserialize)]
    struct Resp {}
    let handle = app.state::<PhoneControlHandle<tauri::Wry>>();
    handle
        .0
        .run_mobile_plugin_async::<Resp>("openAccessibilitySettings", json!({}))
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Switch the WebView between software (transparent, for camera overlay) and
/// hardware (default) rendering modes.  Must be called before/after scan().
#[tauri::command]
pub async fn set_camera_scan_mode(app: AppHandle, enabled: bool) {
    #[cfg(target_os = "android")]
    {
        use serde_json::json;
        use crate::device::phone::plugin::PhoneControlHandle;
        #[derive(serde::Deserialize)]
        struct Resp {}
        let handle = app.state::<PhoneControlHandle<tauri::Wry>>();
        let _ = handle.0.run_mobile_plugin_async::<Resp>("setCameraScanMode", json!({ "enabled": enabled })).await;
    }
    #[cfg(not(target_os = "android"))]
    { let _ = (app, enabled); }
}
