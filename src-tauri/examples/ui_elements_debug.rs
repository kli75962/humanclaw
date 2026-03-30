/// Debug tool: print all AT-SPI2 interactive UI elements to stdout.
/// Usage:
///   cargo run --example ui_elements_debug
///   cargo run --example ui_elements_debug -- youtube
///   cargo run --example ui_elements_debug -- "" 12   (custom max depth)
use atspi::{connection::AccessibilityConnection, proxy::accessible::AccessibleProxy, Role};

const MAX_DEPTH: u32    = 12;
const MAX_ELEMS: usize  = 500;
const MAX_CHILDREN: usize = 80;

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let window_filter = args.next().unwrap_or_default().to_lowercase();
    let max_depth: u32  = args.next().and_then(|s| s.parse().ok()).unwrap_or(MAX_DEPTH);

    eprintln!("window_filter={:?}  max_depth={max_depth}", window_filter);

    let conn = AccessibilityConnection::open().await
        .expect("Failed to connect to AT-SPI2. Is accessibility enabled?");
    let zconn = conn.connection();

    let root = AccessibleProxy::builder(zconn)
        .destination("org.a11y.atspi.Registry").unwrap()
        .path("/org/a11y/atspi/accessible/root").unwrap()
        .build().await
        .expect("Failed to build root proxy");

    let apps = root.get_children().await.expect("get_children failed");
    eprintln!("Top-level apps: {}", apps.len());

    // stack: (bus, path, window_title, depth)
    let mut stack: Vec<(String, String, String, u32)> = apps
        .into_iter()
        .map(|a| (a.name.clone(), a.path.to_string(), String::new(), 0u32))
        .collect();

    let mut total = 0usize;
    let mut found = 0usize;

    while let Some((bus, path, window_title, depth)) = stack.pop() {
        if depth > max_depth || found >= MAX_ELEMS { continue; }

        let proxy = match AccessibleProxy::builder(zconn)
            .destination(bus.as_str())
            .and_then(|b| b.path(path.as_str()))
        {
            Ok(b) => match b.build().await { Ok(p) => p, Err(_) => continue },
            Err(_) => continue,
        };

        let role = match proxy.get_role().await { Ok(r) => r, Err(_) => continue };
        let name = proxy.name().await.unwrap_or_default();
        total += 1;

        let cur_window = if matches!(role, Role::Frame | Role::Window | Role::Dialog) {
            name.clone()
        } else {
            window_title.clone()
        };

        // Filter by window
        if !window_filter.is_empty()
            && matches!(role, Role::Frame | Role::Window | Role::Dialog)
            && !cur_window.to_lowercase().contains(&window_filter)
        {
            continue;
        }

        if is_interactive(role) && !name.is_empty() {
            let in_window = window_filter.is_empty()
                || cur_window.to_lowercase().contains(&window_filter);
            if in_window {
                println!("[{:?}] {:?}  (window: {:?}, depth: {depth})", role, name, cur_window);
                found += 1;
            }
        }

        if let Ok(children) = proxy.get_children().await {
            for child in children.into_iter().take(MAX_CHILDREN) {
                stack.push((child.name, child.path.to_string(), cur_window.clone(), depth + 1));
            }
        }
    }

    eprintln!("\nScanned {total} nodes, found {found} interactive elements.");
}

fn is_interactive(role: Role) -> bool {
    matches!(
        role,
        Role::PushButton | Role::ToggleButton | Role::CheckBox | Role::RadioButton
            | Role::ComboBox | Role::Entry | Role::PasswordText | Role::Link
            | Role::MenuItem | Role::PageTab | Role::ListItem | Role::Slider
            | Role::SpinButton | Role::Heading
    )
}
