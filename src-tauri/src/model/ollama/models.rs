use serde::Deserialize;
use tauri::command;

use super::ollama_client;

#[derive(Deserialize, serde::Serialize)]
pub struct OllamaModel {
    pub name: String,
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

async fn fetch_models_from_url(url: &str) -> Result<Vec<OllamaModel>, String> {
    let response = ollama_client()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Cannot reach Ollama at {url}: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Ollama at {url} returned {status}: {body}"));
    }

    let data: OllamaTagsResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse model list from {url}: {e}"))?;

    Ok(data.models)
}

/// Fetch Ollama models from a specific host:port without saving the endpoint.
#[command]
pub async fn list_models_at(host: String, port: u16) -> Result<Vec<String>, String> {
    let url = format!("http://{}:{}/api/tags", host, port);
    fetch_models_from_url(&url)
        .await
        .map(|models| models.into_iter().map(|m| m.name).collect())
}

/// Fetch the list of locally available Ollama models.
#[command]
pub async fn list_models(app: tauri::AppHandle) -> Result<Vec<OllamaModel>, String> {
    let tags_url = super::types::ollama_tags_url(&app);

    match fetch_models_from_url(&tags_url).await {
        Ok(models) => Ok(models),
        Err(primary_err) => {
            #[cfg(not(target_os = "android"))]
            {
                let fallback_url = "http://127.0.0.1:11434/api/tags";
                if tags_url != fallback_url {
                    return fetch_models_from_url(fallback_url)
                        .await
                        .map_err(|fallback_err| format!("{primary_err}; fallback failed: {fallback_err}"));
                }
            }
            Err(primary_err)
        }
    }
}
