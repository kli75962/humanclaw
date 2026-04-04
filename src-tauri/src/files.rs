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
