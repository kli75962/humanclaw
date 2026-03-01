use reqwest::Client;
use serde::Deserialize;
use tauri::command;

#[derive(Deserialize, serde::Serialize)]
pub struct OllamaModel {
    pub name: String,
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

/// Fetch the list of locally available Ollama models.
#[command]
pub async fn list_models() -> Result<Vec<OllamaModel>, String> {
    let client = Client::new();
    let response = client
        .get("http://10.0.2.2:11434/api/tags")
        .send()
        .await
        .map_err(|e| format!("Cannot reach Ollama: {e}"))?;

    let data: OllamaTagsResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse model list: {e}"))?;

    Ok(data.models)
}
