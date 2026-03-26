#[cfg(not(target_os = "android"))]
mod input;
#[cfg(not(target_os = "android"))]
mod screenshot;
#[cfg(not(target_os = "android"))]
mod shell;

use serde_json::Value;
use tauri::AppHandle;

use crate::session::{store, types::PermissionState};
use crate::tools::permissions::request_permission;
use crate::tools::types::ToolResult;

pub fn is_pc_control_tool(name: &str) -> bool {
    matches!(
        name,
        "pc_mouse_move"
            | "pc_mouse_click"
            | "pc_type_text"
            | "pc_key_press"
            | "pc_screenshot"
            | "pc_run_command"
            | "pc_file_write"
            | "pc_file_read"
            | "pc_file_delete"
    )
}

fn permission_denied(tool_name: &str) -> ToolResult {
    ToolResult {
        tool_name: tool_name.to_string(),
        success: false,
        output: format!("Permission denied: '{tool_name}' is not allowed in Settings → PC Control."),
    }
}

fn not_available(tool_name: &str) -> ToolResult {
    ToolResult {
        tool_name: tool_name.to_string(),
        success: false,
        output: "PC control tools are not available on this platform.".to_string(),
    }
}

async fn check_permission(app: &AppHandle, tool_name: &str, field: &str, args: &Value) -> bool {
    let cfg = store::bootstrap(app);
    let p = &cfg.pc_permissions;
    let state = match field {
        "mouse_control"   => &p.mouse_control,
        "keyboard_input"  => &p.keyboard_input,
        "take_screenshot" => &p.take_screenshot,
        "file_create"     => &p.file_create,
        "file_read"       => &p.file_read,
        "file_delete"     => &p.file_delete,
        "shell_command"   => &p.shell_command,
        _                 => return false,
    };
    match state {
        PermissionState::AllowAll    => true,
        PermissionState::NotAllow    => false,
        PermissionState::AskBeforeUse => request_permission(app, tool_name, field, args).await,
    }
}

pub async fn execute_pc_tool(app: &AppHandle, name: &str, args: &Value) -> ToolResult {
    #[cfg(target_os = "android")]
    return not_available(name);

    #[cfg(not(target_os = "android"))]
    {
        let perm_field = match name {
            "pc_mouse_move" | "pc_mouse_click"   => "mouse_control",
            "pc_type_text"  | "pc_key_press"     => "keyboard_input",
            "pc_screenshot"                      => "take_screenshot",
            "pc_run_command"                     => "shell_command",
            "pc_file_write"                      => "file_create",
            "pc_file_read"                       => "file_read",
            "pc_file_delete"                     => "file_delete",
            _                                    => return not_available(name),
        };

        if !check_permission(app, name, perm_field, args).await {
            return permission_denied(name);
        }

        match name {
            "pc_mouse_move" | "pc_mouse_click" | "pc_type_text" | "pc_key_press" => {
                input::execute(name, args)
            }
            "pc_screenshot" => screenshot::execute(name, args),
            "pc_run_command" => shell::run_command(name, args).await,
            "pc_file_write"  => shell::file_write(name, args),
            "pc_file_read"   => shell::file_read(name, args),
            "pc_file_delete" => shell::file_delete(name, args),
            _                => not_available(name),
        }
    }
}
