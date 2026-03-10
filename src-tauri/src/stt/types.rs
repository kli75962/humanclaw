/// Resolve the Google Cloud API key for Speech-to-Text.
/// Priority: provided UI override → GOOGLE_API_KEY env var from .secrets.
pub fn resolve_google_api_key(override_key: Option<&str>) -> Result<String, String> {
    match override_key {
        Some(k) if !k.trim().is_empty() => Ok(k.trim().to_string()),
        _ => std::env::var("GOOGLE_API_KEY")
            .map_err(|_| "GOOGLE_API_KEY not set — add it to src-tauri/.secrets or Settings".to_string()),
    }
}
