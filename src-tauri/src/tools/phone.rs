use serde_json::Value;
use tauri::AppHandle;
#[cfg(target_os = "android")]
use tauri::Manager;

use super::ToolResult;

#[cfg(not(target_os = "android"))]
static PHONE_CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();

pub fn is_phone_control_tool(name: &str) -> bool {
    matches!(
        name,
        "tap"
            | "swipe"
            | "type_text"
            | "press_key"
            | "launch_app"
            | "get_screen"
            | "get_screen_deep"
    )
}

pub async fn execute_phone_control_tool(app: &AppHandle, name: &str, args: &Value) -> ToolResult {
    #[cfg(target_os = "android")]
    {
        call_kotlin_tool(app, name, args).await
    }

    #[cfg(not(target_os = "android"))]
    {
        forward_to_android(app, name, args).await
    }
}

/// Desktop: forward a phone-control tool call to the first paired Android device via POST /tool.
#[cfg(not(target_os = "android"))]
async fn forward_to_android(app: &AppHandle, name: &str, args: &Value) -> ToolResult {
    use crate::bridge::types::ToolResponse;
    use crate::session::store;

    let cfg = store::bootstrap(app);
    let peer = cfg.paired_devices.first();
    let Some(peer) = peer else {
        return ToolResult {
            tool_name: name.to_string(),
            success: false,
            output: "No paired Android device. Pair a device first via Settings -> Pair with QR code.".to_string(),
        };
    };

    let url = format!("http://{}/tool", peer.address);
    let body = serde_json::json!({
        "key": cfg.hash_key,
        "tool_name": name,
        "tool_args": args,
    });

    let client = PHONE_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build phone forwarding client")
    });

    match client.post(&url).json(&body).send().await {
        Ok(resp) if resp.status().is_success() => match resp.json::<ToolResponse>().await {
            Ok(r) => ToolResult {
                tool_name: name.to_string(),
                success: r.success,
                output: r.output,
            },
            Err(e) => ToolResult {
                tool_name: name.to_string(),
                success: false,
                output: format!("Invalid response from phone: {e}"),
            },
        },
        Ok(resp) => ToolResult {
            tool_name: name.to_string(),
            success: false,
            output: format!("Phone returned {}", resp.status()),
        },
        Err(e) => ToolResult {
            tool_name: name.to_string(),
            success: false,
            output: format!("Could not reach phone at {}: {e}", peer.address),
        },
    }
}

#[cfg(target_os = "android")]
async fn call_kotlin_tool(app: &AppHandle, name: &str, args: &Value) -> ToolResult {
    use crate::phone::plugin::PhoneControlHandle;
    use serde_json::json;

    let handle = app.state::<PhoneControlHandle<tauri::Wry>>();
    let payload = json!({
        "tool": name,
        "args": args,
    });

    match handle
        .0
        .run_mobile_plugin::<ToolResult>("executeTool", payload)
    {
        Ok(result) => result,
        Err(e) => ToolResult {
            tool_name: name.to_string(),
            success: false,
            output: format!("Plugin error: {e}"),
        },
    }
}
