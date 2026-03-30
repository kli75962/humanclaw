use serde::Serialize;
use serde_json::Value;

use crate::tools::types::ToolResult;

/// Compact element — no coordinates (LLM uses name, not x/y, to interact).
#[derive(Serialize)]
struct UiElement {
    role:         String,
    name:         String,
    window_title: String,
}

// ── Public entry point ────────────────────────────────────────────────────────

pub async fn execute(tool_name: &str, args: &Value) -> ToolResult {
    // Optional: only return elements from windows whose title contains this string.
    let window_filter = args
        .get("window_title")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_lowercase();

    match collect_elements(window_filter).await {
        Ok(els) => {
            let output = serde_json::to_string(&els).unwrap_or_else(|_| "[]".to_string());
            eprintln!("[pc_ui_elements] {} elements:\n{output}", els.len());
            ToolResult { tool_name: tool_name.to_string(), success: true, output }
        },
        Err(e) => {
            eprintln!("[pc_ui_elements] error: {e}");
            ToolResult { tool_name: tool_name.to_string(), success: false, output: e }
        },
    }
}

// ── Platform selector ─────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
async fn collect_elements(window_filter: String) -> Result<Vec<UiElement>, String> {
    collect_linux(window_filter).await
}

#[cfg(target_os = "windows")]
async fn collect_elements(window_filter: String) -> Result<Vec<UiElement>, String> {
    tokio::task::spawn_blocking(move || collect_windows(window_filter))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
async fn collect_elements(_: String) -> Result<Vec<UiElement>, String> {
    Err("pc_ui_elements is only supported on Linux and Windows.".to_string())
}

// ── Linux / AT-SPI2 ───────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
async fn collect_linux(window_filter: String) -> Result<Vec<UiElement>, String> {
    use atspi::{
        connection::AccessibilityConnection,
        proxy::accessible::AccessibleProxy,
        Role,
    };

    const MAX_DEPTH: u32 = 10;
    const MAX_ELEMS: usize = 200;
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

    // Stack: (bus_name, object_path, window_title, depth)
    let mut stack: Vec<(String, String, String, u32)> = apps
        .into_iter()
        .map(|a| (a.name.clone(), a.path.to_string(), String::new(), 0u32))
        .collect();

    let mut elements: Vec<UiElement> = Vec::new();

    while let Some((bus, path, window_title, depth)) = stack.pop() {
        if depth > MAX_DEPTH || elements.len() >= MAX_ELEMS {
            continue;
        }

        let proxy = match AccessibleProxy::builder(zconn)
            .destination(bus.as_str())
            .and_then(|b| b.path(path.as_str()))
        {
            Ok(b) => match b.build().await {
                Ok(p) => p,
                Err(_) => continue,
            },
            Err(_) => continue,
        };

        let role = match proxy.get_role().await {
            Ok(r) => r,
            Err(_) => continue,
        };

        let current_window = if matches!(role, Role::Frame | Role::Window | Role::Dialog) {
            proxy.name().await.unwrap_or_else(|_| window_title.clone())
        } else {
            window_title.clone()
        };

        // Skip entire subtree if window doesn't match filter
        if !window_filter.is_empty()
            && !current_window.to_lowercase().contains(&window_filter)
            && matches!(role, Role::Frame | Role::Window | Role::Dialog)
        {
            continue;
        }

        if is_interactive_role(role) {
            let name = proxy.name().await.unwrap_or_default();
            if !name.is_empty() {
                // Only include if window matches filter (or no filter)
                if window_filter.is_empty()
                    || current_window.to_lowercase().contains(&window_filter)
                {
                    elements.push(UiElement {
                        role: format!("{role:?}"),
                        name,
                        window_title: current_window.clone(),
                    });
                }
            }
        }

        if let Ok(children) = proxy.get_children().await {
            for child in children.into_iter().take(MAX_CHILDREN) {
                stack.push((
                    child.name.clone(),
                    child.path.to_string(),
                    current_window.clone(),
                    depth + 1,
                ));
            }
        }
    }

    Ok(elements)
}

#[cfg(target_os = "linux")]
fn is_interactive_role(role: atspi::Role) -> bool {
    use atspi::Role;
    matches!(
        role,
        Role::PushButton
            | Role::ToggleButton
            | Role::CheckBox
            | Role::RadioButton
            | Role::ComboBox
            | Role::Entry
            | Role::PasswordText
            | Role::Link
            | Role::MenuItem
            | Role::PageTab
            | Role::ListItem
            | Role::Slider
            | Role::SpinButton
    )
}

// ── Windows / UI Automation ───────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn collect_windows(window_filter: String) -> Result<Vec<UiElement>, String> {
    use uiautomation::{types::ControlType, UIAutomation};

    const MAX_ELEMS: usize = 150;

    let automation = UIAutomation::new().map_err(|e| e.to_string())?;
    let root = automation.get_root_element().map_err(|e| e.to_string())?;
    let walker = automation.get_control_view_walker().map_err(|e| e.to_string())?;

    let mut elements: Vec<UiElement> = Vec::new();
    walk_windows(&walker, &root, "", &window_filter, &mut elements, 0, MAX_ELEMS);
    Ok(elements)
}

#[cfg(target_os = "windows")]
fn walk_windows(
    walker:        &uiautomation::core::UITreeWalker,
    element:       &uiautomation::core::UIElement,
    window_title:  &str,
    window_filter: &str,
    elements:      &mut Vec<UiElement>,
    depth:         u32,
    max_elems:     usize,
) {
    use uiautomation::types::ControlType;

    if depth > 10 || elements.len() >= max_elems {
        return;
    }

    let name = element.get_name().unwrap_or_default();
    let ctrl = element.get_control_type().unwrap_or(ControlType::Custom);

    let current_window: String = if ctrl == ControlType::Window {
        name.clone()
    } else {
        window_title.to_string()
    };

    // Skip window subtree if it doesn't match filter
    if ctrl == ControlType::Window
        && !window_filter.is_empty()
        && !current_window.to_lowercase().contains(&window_filter.to_lowercase())
    {
        return;
    }

    if is_interactive_type(ctrl) && !name.is_empty() {
        if window_filter.is_empty()
            || current_window.to_lowercase().contains(&window_filter.to_lowercase())
        {
            elements.push(UiElement {
                role:         format!("{ctrl:?}"),
                name,
                window_title: current_window.clone(),
            });
        }
    }

    if let Ok(child) = walker.get_first_child(element) {
        walk_windows(walker, &child, &current_window, window_filter, elements, depth + 1, max_elems);
        let mut cur = child;
        while let Ok(sib) = walker.get_next_sibling(&cur) {
            walk_windows(walker, &sib, &current_window, window_filter, elements, depth + 1, max_elems);
            cur = sib;
        }
    }
}

#[cfg(target_os = "windows")]
fn is_interactive_type(ctrl: uiautomation::types::ControlType) -> bool {
    use uiautomation::types::ControlType;
    matches!(
        ctrl,
        ControlType::Button
            | ControlType::CheckBox
            | ControlType::RadioButton
            | ControlType::ComboBox
            | ControlType::Edit
            | ControlType::Hyperlink
            | ControlType::ListItem
            | ControlType::MenuItem
            | ControlType::Slider
            | ControlType::Spinner
            | ControlType::TabItem
            | ControlType::TreeItem
            | ControlType::SplitButton
    )
}
