use enigo::{Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};
use serde_json::Value;

use crate::tools::types::ToolResult;

pub fn execute(tool_name: &str, args: &Value) -> ToolResult {
    match tool_name {
        "pc_mouse_move"  => mouse_move(args),
        "pc_mouse_click" => mouse_click(args),
        "pc_type_text"   => type_text(args),
        "pc_key_press"   => key_press(args),
        _ => ToolResult {
            tool_name: tool_name.to_string(),
            success: false,
            output: format!("Unknown input tool: {tool_name}"),
        },
    }
}

fn make_enigo() -> Result<Enigo, String> {
    Enigo::new(&Settings::default()).map_err(|e| e.to_string())
}

fn mouse_move(args: &Value) -> ToolResult {
    let x = args.get("x").and_then(Value::as_i64).unwrap_or(0) as i32;
    let y = args.get("y").and_then(Value::as_i64).unwrap_or(0) as i32;
    let mut enigo = match make_enigo() {
        Ok(e) => e,
        Err(e) => return ToolResult { tool_name: "pc_mouse_move".to_string(), success: false, output: e },
    };
    match enigo.move_mouse(x, y, Coordinate::Abs) {
        Ok(_) => ToolResult {
            tool_name: "pc_mouse_move".to_string(),
            success: true,
            output: format!("Mouse moved to ({x}, {y})"),
        },
        Err(e) => ToolResult { tool_name: "pc_mouse_move".to_string(), success: false, output: e.to_string() },
    }
}

fn mouse_click(args: &Value) -> ToolResult {
    let x      = args.get("x").and_then(Value::as_i64).map(|v| v as i32);
    let y      = args.get("y").and_then(Value::as_i64).map(|v| v as i32);
    let btn    = args.get("button").and_then(Value::as_str).unwrap_or("left");
    let double = args.get("double").and_then(Value::as_bool).unwrap_or(false);

    let button = match btn { "right" => Button::Right, "middle" => Button::Middle, _ => Button::Left };

    let mut enigo = match make_enigo() {
        Ok(e) => e,
        Err(e) => return ToolResult { tool_name: "pc_mouse_click".to_string(), success: false, output: e },
    };

    if let (Some(x), Some(y)) = (x, y) {
        if let Err(e) = enigo.move_mouse(x, y, Coordinate::Abs) {
            return ToolResult { tool_name: "pc_mouse_click".to_string(), success: false, output: e.to_string() };
        }
    }

    let clicks = if double { 2 } else { 1 };
    for _ in 0..clicks {
        if let Err(e) = enigo.button(button, Direction::Click) {
            return ToolResult { tool_name: "pc_mouse_click".to_string(), success: false, output: e.to_string() };
        }
    }

    ToolResult {
        tool_name: "pc_mouse_click".to_string(),
        success: true,
        output: format!("Clicked {btn}{}", if double { " (double)" } else { "" }),
    }
}

fn type_text(args: &Value) -> ToolResult {
    let text = args.get("text").and_then(Value::as_str).unwrap_or("");
    let mut enigo = match make_enigo() {
        Ok(e) => e,
        Err(e) => return ToolResult { tool_name: "pc_type_text".to_string(), success: false, output: e },
    };
    match enigo.text(text) {
        Ok(_) => ToolResult {
            tool_name: "pc_type_text".to_string(),
            success: true,
            output: format!("Typed {} characters", text.len()),
        },
        Err(e) => ToolResult { tool_name: "pc_type_text".to_string(), success: false, output: e.to_string() },
    }
}

fn parse_key(s: &str) -> Option<Key> {
    match s {
        "return" | "enter"           => Some(Key::Return),
        "escape" | "esc"             => Some(Key::Escape),
        "backspace"                  => Some(Key::Backspace),
        "tab"                        => Some(Key::Tab),
        "space"                      => Some(Key::Space),
        "delete" | "del"             => Some(Key::Delete),
        "home"                       => Some(Key::Home),
        "end"                        => Some(Key::End),
        "pageup"                     => Some(Key::PageUp),
        "pagedown"                   => Some(Key::PageDown),
        "up"                         => Some(Key::UpArrow),
        "down"                       => Some(Key::DownArrow),
        "left"                       => Some(Key::LeftArrow),
        "right"                      => Some(Key::RightArrow),
        "ctrl" | "control"           => Some(Key::Control),
        "alt"                        => Some(Key::Alt),
        "shift"                      => Some(Key::Shift),
        "meta" | "super" | "win" | "cmd" | "command" => Some(Key::Meta),
        "f1"  => Some(Key::F1),  "f2"  => Some(Key::F2),  "f3"  => Some(Key::F3),
        "f4"  => Some(Key::F4),  "f5"  => Some(Key::F5),  "f6"  => Some(Key::F6),
        "f7"  => Some(Key::F7),  "f8"  => Some(Key::F8),  "f9"  => Some(Key::F9),
        "f10" => Some(Key::F10), "f11" => Some(Key::F11), "f12" => Some(Key::F12),
        s if s.len() == 1 => s.chars().next().map(Key::Unicode),
        _ => None,
    }
}

fn key_press(args: &Value) -> ToolResult {
    let raw = args.get("key").and_then(Value::as_str).unwrap_or("").trim().to_lowercase();
    let parts: Vec<&str> = raw.split('+').collect();
    let (modifiers, rest) = parts.split_at(parts.len().saturating_sub(1));
    let main = rest.first().copied().unwrap_or("");

    let mut enigo = match make_enigo() {
        Ok(e) => e,
        Err(e) => return ToolResult { tool_name: "pc_key_press".to_string(), success: false, output: e },
    };

    for &m in modifiers {
        if let Some(k) = parse_key(m) {
            if let Err(e) = enigo.key(k, Direction::Press) {
                return ToolResult { tool_name: "pc_key_press".to_string(), success: false, output: e.to_string() };
            }
        }
    }

    if let Some(k) = parse_key(main) {
        let _ = enigo.key(k, Direction::Click);
    }

    for &m in modifiers.iter().rev() {
        if let Some(k) = parse_key(m) {
            let _ = enigo.key(k, Direction::Release);
        }
    }

    ToolResult { tool_name: "pc_key_press".to_string(), success: true, output: format!("Pressed: {raw}") }
}
