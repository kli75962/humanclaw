#[cfg(not(target_os = "android"))]
mod open_url;
#[cfg(not(target_os = "android"))]
mod screenshot;
#[cfg(not(target_os = "android"))]
mod system_run;

use serde_json::Value;
use tauri::AppHandle;

use crate::session::{store, types::PermissionState};
use crate::tools::permissions::request_permission;
use crate::tools::types::ToolResult;

pub fn is_pc_control_tool(name: &str) -> bool {
    matches!(
        name,
        "system_run" | "pc_open_url" | "pc_screenshot" | "pc_get_platform"
    )
}

fn permission_denied(tool_name: &str) -> ToolResult {
    ToolResult::err(
        tool_name,
        "PERMISSION_DENIED",
        format!("Permission denied: '{tool_name}' is not allowed in Settings → PC Control."),
    )
}

fn not_available(tool_name: &str) -> ToolResult {
    ToolResult::err(tool_name, "NOT_AVAILABLE", "PC control tools are not available on this platform.")
}

async fn check_permission(app: &AppHandle, tool_name: &str, field: &str, args: &Value) -> bool {
    let cfg = store::bootstrap(app);
    let p = &cfg.pc_permissions;
    let state = match field {
        "take_screenshot" => &p.take_screenshot,
        "launch_app"      => &p.launch_app,
        "shell_execution" => &p.shell_execution,
        _                 => return false,
    };
    match state {
        PermissionState::AllowAll     => true,
        PermissionState::NotAllow     => false,
        PermissionState::AskBeforeUse => request_permission(app, tool_name, field, args).await,
    }
}

fn get_platform_info() -> ToolResult {
    use std::env::consts;
    let info = serde_json::json!({ "os": consts::OS, "arch": consts::ARCH });
    ToolResult::ok("pc_get_platform", info.to_string())
}

pub async fn execute_pc_tool(app: &AppHandle, name: &str, args: &Value) -> ToolResult {
    #[cfg(target_os = "android")]
    return not_available(name);

    #[cfg(not(target_os = "android"))]
    {
        if name == "pc_get_platform" {
            return get_platform_info();
        }

        let perm_field = match name {
            "pc_screenshot" => "take_screenshot",
            "pc_open_url"   => "launch_app",
            "system_run"    => "shell_execution",
            _               => return not_available(name),
        };

        if !check_permission(app, name, perm_field, args).await {
            return permission_denied(name);
        }

        match name {
            "pc_screenshot" => screenshot::execute(name, args),
            "pc_open_url"   => open_url::execute(name, args).await,
            "system_run"    => system_run::execute(name, args).await,
            _               => not_available(name),
        }
    }
}
