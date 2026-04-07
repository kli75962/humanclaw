use std::io::{Cursor, Read};

const MAX_CONTENT_CHARS: usize = 120_000;

/// Read a file from disk as plain text.
/// Handles text files directly and Office Open XML formats (docx/pptx/xlsx/odt/odp/ods)
/// by unpacking the ZIP and extracting text nodes from the embedded XML.
#[tauri::command]
pub async fn read_file_text(path: String) -> Result<String, String> {
    let p = std::path::Path::new(&path);
    let ext = p.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "docx" | "pptx" | "xlsx" | "odt" | "odp" | "ods" => {
            let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
            extract_office_text(Cursor::new(bytes))
        }
        _ => {
            std::fs::read_to_string(&path)
                .map_err(|_| format!("Cannot read binary file: .{ext}"))
                .map(|t| truncate(t))
        }
    }
}

/// Read the current clipboard image as a data URL (Linux: wl-paste for Wayland, xclip for X11).
/// Returns None if clipboard contains no image or tools are unavailable.
#[tauri::command]
pub async fn get_clipboard_image() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        use base64::Engine;

        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            // List available MIME types first, then read the first image type found.
            // --no-newline is required for binary data — without it wl-paste appends
            // a newline that corrupts the image bytes.
            if let Ok(types_out) = std::process::Command::new("wl-paste")
                .arg("--list-types")
                .output()
            {
                if types_out.status.success() {
                    for line in String::from_utf8_lossy(&types_out.stdout).lines() {
                        let mime = line.trim();
                        if mime.starts_with("image/") {
                            if let Ok(out) = std::process::Command::new("wl-paste")
                                .args(["--no-newline", "--type", mime])
                                .output()
                            {
                                if out.status.success() && !out.stdout.is_empty() {
                                    return Some(format!(
                                        "data:{};base64,{}",
                                        mime,
                                        base64::engine::general_purpose::STANDARD
                                            .encode(&out.stdout)
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            return None;
        }

        // X11 fallback via xclip
        for mime in &["image/png", "image/jpeg", "image/gif", "image/webp"] {
            if let Ok(out) = std::process::Command::new("xclip")
                .args(["-selection", "clipboard", "-t", mime, "-o"])
                .output()
            {
                if out.status.success() && !out.stdout.is_empty() {
                    return Some(format!(
                        "data:{};base64,{}",
                        mime,
                        base64::engine::general_purpose::STANDARD.encode(&out.stdout)
                    ));
                }
            }
        }
        None
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// Read file URIs from the clipboard (text/uri-list) and return decoded OS paths.
/// Used for Ctrl+V file paste on Linux Wayland.
#[tauri::command]
pub async fn get_clipboard_uri_list() -> Vec<String> {
    #[cfg(target_os = "linux")]
    {
        fn pct_decode(s: &str) -> String {
            let b = s.as_bytes();
            let mut out = String::with_capacity(b.len());
            let mut i = 0;
            while i < b.len() {
                if b[i] == b'%' && i + 2 < b.len() {
                    if let Ok(hex) = std::str::from_utf8(&b[i + 1..i + 3]) {
                        if let Ok(byte) = u8::from_str_radix(hex, 16) {
                            out.push(byte as char);
                            i += 3;
                            continue;
                        }
                    }
                }
                out.push(b[i] as char);
                i += 1;
            }
            out
        }

        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            let has_uri = std::process::Command::new("wl-paste")
                .arg("--list-types")
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).lines().any(|l| l.trim() == "text/uri-list"))
                .unwrap_or(false);

            if has_uri {
                if let Ok(out) = std::process::Command::new("wl-paste")
                    .args(["--no-newline", "--type", "text/uri-list"])
                    .output()
                {
                    if out.status.success() {
                        return String::from_utf8_lossy(&out.stdout)
                            .lines()
                            .filter(|l| l.starts_with("file://"))
                            .map(|l| pct_decode(l.trim_start_matches("file://")))
                            .collect();
                    }
                }
            }
        }
        vec![]
    }
    #[cfg(not(target_os = "linux"))]
    { vec![] }
}

/// Read any file from disk as a base64-encoded string (for images / binary files).
#[tauri::command]
pub async fn read_file_as_base64(path: String) -> Result<String, String> {
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    use base64::Engine;
    Ok(base64::engine::general_purpose::STANDARD.encode(&bytes))
}

/// Same as read_file_text but accepts raw bytes from the frontend (file picker).
#[tauri::command]
pub async fn extract_file_text_from_bytes(bytes: Vec<u8>, filename: String) -> Result<String, String> {
    let ext = std::path::Path::new(&filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "docx" | "pptx" | "xlsx" | "odt" | "odp" | "ods" => {
            extract_office_text(Cursor::new(bytes))
        }
        _ => Err(format!("Use text() for non-Office binary format: .{ext}")),
    }
}

// ── internals ──────────────────────────────────────────────────────────────

fn extract_office_text<R: Read + std::io::Seek>(reader: R) -> Result<String, String> {
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| e.to_string())?;

    // Collect relevant entry names first (can't borrow archive twice).
    let mut names: Vec<String> = (0..archive.len())
        .filter_map(|i| archive.by_index(i).ok().map(|f| f.name().to_string()))
        .filter(|n| is_relevant_xml(n))
        .collect();
    names.sort(); // slides / sheets in order

    let mut parts: Vec<String> = Vec::new();
    for name in &names {
        if let Ok(mut entry) = archive.by_name(name) {
            let mut raw = String::new();
            let _ = entry.read_to_string(&mut raw);
            let text = strip_xml_tags(&raw);
            if !text.trim().is_empty() {
                parts.push(text);
            }
        }
    }

    if parts.is_empty() {
        Err("No readable text found in file".to_string())
    } else {
        Ok(truncate(parts.join("\n\n")))
    }
}

/// Which XML entries inside the ZIP carry human-readable text.
fn is_relevant_xml(name: &str) -> bool {
    if !name.ends_with(".xml") { return false; }
    // DOCX
    if name.contains("word/document") { return true; }
    // PPTX slides
    if name.starts_with("ppt/slides/slide") { return true; }
    // ODF (odt/odp/ods)
    if name == "content.xml" { return true; }
    // XLSX shared strings + worksheets
    if name.contains("xl/sharedStrings") || name.starts_with("xl/worksheets/sheet") { return true; }
    false
}

/// Remove XML tags, collapse whitespace.
fn strip_xml_tags(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len() / 2);
    let mut in_tag = false;
    for ch in xml.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => { in_tag = false; out.push(' '); }
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate(s: String) -> String {
    if s.chars().count() <= MAX_CONTENT_CHARS {
        return s;
    }
    let cut: String = s.chars().take(MAX_CONTENT_CHARS).collect();
    format!("{cut}\n\n[…content truncated at {MAX_CONTENT_CHARS} characters]")
}
