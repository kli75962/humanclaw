use serde_json::Value;

use crate::tools::types::ToolResult;

// ── Public entry points ───────────────────────────────────────────────────────

pub async fn execute_activate(tool_name: &str, args: &Value) -> ToolResult {
    let window = args.get("window_title").and_then(Value::as_str).unwrap_or("");
    let name   = args.get("name").and_then(Value::as_str).unwrap_or("");
    if name.is_empty() {
        return ToolResult { tool_name: tool_name.to_string(), success: false, output: "name is required".into() };
    }
    match do_activate(window, name).await {
        Ok(msg) => ToolResult { tool_name: tool_name.to_string(), success: true,  output: msg },
        Err(e)  => ToolResult { tool_name: tool_name.to_string(), success: false, output: e },
    }
}

pub async fn execute_set_text(tool_name: &str, args: &Value) -> ToolResult {
    let window = args.get("window_title").and_then(Value::as_str).unwrap_or("");
    let name   = args.get("name").and_then(Value::as_str).unwrap_or("");
    let text   = args.get("text").and_then(Value::as_str).unwrap_or("");
    if name.is_empty() {
        return ToolResult { tool_name: tool_name.to_string(), success: false, output: "name is required".into() };
    }
    match do_set_text(window, name, text).await {
        Ok(msg) => ToolResult { tool_name: tool_name.to_string(), success: true,  output: msg },
        Err(e)  => ToolResult { tool_name: tool_name.to_string(), success: false, output: e },
    }
}

// ── Platform dispatcher ───────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
async fn do_activate(window: &str, name: &str) -> Result<String, String> {
    linux_activate(window, name).await
}
#[cfg(target_os = "linux")]
async fn do_set_text(window: &str, name: &str, text: &str) -> Result<String, String> {
    linux_set_text(window, name, text).await
}

#[cfg(target_os = "windows")]
async fn do_activate(window: &str, name: &str) -> Result<String, String> {
    let (w, n) = (window.to_string(), name.to_string());
    tokio::task::spawn_blocking(move || windows_activate(&w, &n)).await.map_err(|e| e.to_string())?
}
#[cfg(target_os = "windows")]
async fn do_set_text(window: &str, name: &str, text: &str) -> Result<String, String> {
    let (w, n, t) = (window.to_string(), name.to_string(), text.to_string());
    tokio::task::spawn_blocking(move || windows_set_text(&w, &n, &t)).await.map_err(|e| e.to_string())?
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
async fn do_activate(_: &str, _: &str) -> Result<String, String> {
    Err("pc_activate is only supported on Linux and Windows.".into())
}
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
async fn do_set_text(_: &str, _: &str, _: &str) -> Result<String, String> {
    Err("pc_set_text is only supported on Linux and Windows.".into())
}

// ── Linux / AT-SPI2 ───────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn name_matches(el_name: &str, filter: &str) -> bool {
    el_name.to_lowercase().contains(&filter.to_lowercase())
}

#[cfg(target_os = "linux")]
fn window_matches(window_title: &str, filter: &str) -> bool {
    filter.is_empty() || window_title.to_lowercase().contains(&filter.to_lowercase())
}

/// Walk the AT-SPI2 tree and return `(bus_name, object_path, matched_name, window_title)`
/// for the first element whose name contains `name_filter` inside a window whose title
/// contains `window_filter`.
#[cfg(target_os = "linux")]
async fn find_linux(
    window_filter: &str,
    name_filter:   &str,
) -> Result<(String, String, String, String), String> {
    use atspi::{connection::AccessibilityConnection, proxy::accessible::AccessibleProxy, Role};

    const MAX_DEPTH: u32 = 8;
    const MAX_CHILDREN: usize = 60;

    let conn: AccessibilityConnection =
        AccessibilityConnection::open().await.map_err(|e| format!("{e}"))?;
    let zconn = conn.connection();

    let root = AccessibleProxy::builder(zconn)
        .destination("org.a11y.atspi.Registry")
        .and_then(|b| b.path("/org/a11y/atspi/accessible/root"))
        .map_err(|e| e.to_string())?
        .build()
        .await
        .map_err(|e| e.to_string())?;

    let apps = root.get_children().await.map_err(|e| e.to_string())?;

    // stack: (bus, path, window_title, depth)
    let mut stack: Vec<(String, String, String, u32)> = apps
        .into_iter()
        .map(|a| (a.name, a.path.to_string(), String::new(), 0u32))
        .collect();

    while let Some((bus, path, window_title, depth)) = stack.pop() {
        if depth > MAX_DEPTH { continue; }

        let proxy = match AccessibleProxy::builder(zconn)
            .destination(bus.as_str())
            .and_then(|b| b.path(path.as_str()))
        {
            Ok(b) => match b.build().await { Ok(p) => p, Err(_) => continue },
            Err(_) => continue,
        };

        let role = match proxy.get_role().await { Ok(r) => r, Err(_) => continue };
        let el_name = proxy.name().await.unwrap_or_default();

        let cur_window = if matches!(role, Role::Frame | Role::Window | Role::Dialog) {
            el_name.clone()
        } else {
            window_title.clone()
        };

        if name_matches(&el_name, name_filter) && window_matches(&cur_window, window_filter) {
            return Ok((bus, path, el_name, cur_window));
        }

        if let Ok(children) = proxy.get_children().await {
            for child in children.into_iter().take(MAX_CHILDREN) {
                stack.push((child.name, child.path.to_string(), cur_window.clone(), depth + 1));
            }
        }
    }

    Err(format!("Element '{name_filter}' not found in window '{window_filter}'"))
}

#[cfg(target_os = "linux")]
async fn linux_activate(window_filter: &str, name_filter: &str) -> Result<String, String> {
    use atspi::{connection::AccessibilityConnection, proxy::action::ActionProxy};

    let (bus, path, el_name, win) = find_linux(window_filter, name_filter).await?;

    let conn: AccessibilityConnection =
        AccessibilityConnection::open().await.map_err(|e| format!("{e}"))?;
    let zconn = conn.connection();

    let action = ActionProxy::builder(zconn)
        .destination(bus.as_str())
        .and_then(|b| b.path(path.as_str()))
        .map_err(|e| e.to_string())?
        .build()
        .await
        .map_err(|e| e.to_string())?;

    action.do_action(0).await.map_err(|e| e.to_string())?;
    Ok(format!("Activated '{el_name}' in '{win}'"))
}

#[cfg(target_os = "linux")]
async fn linux_set_text(window_filter: &str, name_filter: &str, text: &str) -> Result<String, String> {
    use atspi::{connection::AccessibilityConnection, proxy::editable_text::EditableTextProxy};

    let (bus, path, el_name, win) = find_linux(window_filter, name_filter).await?;

    let conn: AccessibilityConnection =
        AccessibilityConnection::open().await.map_err(|e| format!("{e}"))?;
    let zconn = conn.connection();

    let editable = EditableTextProxy::builder(zconn)
        .destination(bus.as_str())
        .and_then(|b| b.path(path.as_str()))
        .map_err(|e| e.to_string())?
        .build()
        .await
        .map_err(|e| e.to_string())?;

    editable.set_text_contents(text).await.map_err(|e| e.to_string())?;
    Ok(format!("Set text on '{el_name}' in '{win}'"))
}

// ── Windows / UI Automation ───────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn windows_activate(window_filter: &str, name_filter: &str) -> Result<String, String> {
    use uiautomation::UIAutomation;

    let automation = UIAutomation::new().map_err(|e| e.to_string())?;
    let root       = automation.get_root_element().map_err(|e| e.to_string())?;
    let walker     = automation.get_control_view_walker().map_err(|e| e.to_string())?;

    search_windows(&walker, &root, "", window_filter, name_filter, 0, &|el, name, win| {
        el.click().map_err(|e| e.to_string())?;
        Ok(format!("Activated '{name}' in '{win}'"))
    })
    .ok_or_else(|| format!("Element '{name_filter}' not found in window '{window_filter}'"))?
}

#[cfg(target_os = "windows")]
fn windows_set_text(window_filter: &str, name_filter: &str, text: &str) -> Result<String, String> {
    use uiautomation::UIAutomation;

    let automation = UIAutomation::new().map_err(|e| e.to_string())?;
    let root       = automation.get_root_element().map_err(|e| e.to_string())?;
    let walker     = automation.get_control_view_walker().map_err(|e| e.to_string())?;

    search_windows(&walker, &root, "", window_filter, name_filter, 0, &|el, name, win| {
        el.set_focus().map_err(|e| e.to_string())?;
        el.send_text(text, 0).map_err(|e| e.to_string())?;
        Ok(format!("Set text on '{name}' in '{win}'"))
    })
    .ok_or_else(|| format!("Element '{name_filter}' not found in window '{window_filter}'"))?
}

/// Recursive tree walker that calls `action` on the first matching element.
#[cfg(target_os = "windows")]
fn search_windows(
    walker:        &uiautomation::core::UITreeWalker,
    element:       &uiautomation::core::UIElement,
    window_title:  &str,
    window_filter: &str,
    name_filter:   &str,
    depth:         u32,
    action:        &dyn Fn(&uiautomation::core::UIElement, &str, &str) -> Result<String, String>,
) -> Option<Result<String, String>> {
    use uiautomation::types::ControlType;

    if depth > 12 { return None; }

    let name = element.get_name().unwrap_or_default();
    let ctrl = element.get_control_type().unwrap_or(ControlType::Custom);

    let cur_window: String = if ctrl == ControlType::Window {
        name.clone()
    } else {
        window_title.to_string()
    };

    let name_match   = name.to_lowercase().contains(&name_filter.to_lowercase());
    let window_match = window_filter.is_empty()
        || cur_window.to_lowercase().contains(&window_filter.to_lowercase());

    if name_match && window_match && !name.is_empty() {
        return Some(action(element, &name, &cur_window));
    }

    if let Ok(child) = walker.get_first_child(element) {
        if let Some(r) = search_windows(walker, &child, &cur_window, window_filter, name_filter, depth + 1, action) {
            return Some(r);
        }
        let mut cur = child;
        while let Ok(sib) = walker.get_next_sibling(&cur) {
            if let Some(r) = search_windows(walker, &sib, &cur_window, window_filter, name_filter, depth + 1, action) {
                return Some(r);
            }
            cur = sib;
        }
    }
    None
}
