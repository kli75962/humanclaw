use tauri::{AppHandle, Manager};
use super::list_runtime_persona_names;

// ── Persona build status (persisted JSON) ────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PersonaBuildStatus {
    pub status: String,       // "creating" | "done" | "interrupted"
    pub display_name: String, // shown in the notice
    pub model: String,
    pub sex: String,
    pub age_range: String,
    pub vibe: String,
    pub world: String,
    pub connects_by: String,
    pub persona_name: String, // original wizard input
}

fn persona_build_status_path(app: &AppHandle) -> std::path::PathBuf {
    app.path().app_data_dir().unwrap_or_default().join("persona_build_status.json")
}

fn read_persona_build_status(app: &AppHandle) -> Option<PersonaBuildStatus> {
    let text = std::fs::read_to_string(persona_build_status_path(app)).ok()?;
    serde_json::from_str(&text).ok()
}

pub(super) fn write_persona_build_status(app: &AppHandle, status: &PersonaBuildStatus) {
    let path = persona_build_status_path(app);
    if let Ok(text) = serde_json::to_string(status) {
        let _ = std::fs::write(path, text);
    }
}

#[tauri::command]
pub fn get_persona_build_status(app: AppHandle) -> Option<PersonaBuildStatus> {
    read_persona_build_status(&app)
}

#[tauri::command]
pub fn clear_persona_build_status(app: AppHandle) -> Result<(), String> {
    let path = persona_build_status_path(&app);
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| e.to_string())
    } else {
        Ok(())
    }
}

/// Background persona creation — runs the LLM headless and calls create_skill without
/// opening a chat window. Returns the LLM's final reply on success.
#[tauri::command]
pub async fn create_persona_background(
    app: AppHandle,
    model: String,
    sex: String,
    age_range: String,
    vibe: String,
    world: String,
    connects_by: String,
    persona_name: String,
) -> Result<String, String> {
    use crate::ai::ollama::types::OllamaMessage;
    use crate::ai::ollama::headless::run_headless;

    // Snapshot existing runtime personas before creation
    let before_names: std::collections::HashSet<String> =
        list_runtime_persona_names(&app).into_iter().collect();

    // Write "creating" status
    let display_name = if persona_name == "random" {
        "new persona".to_string()
    } else {
        persona_name.clone()
    };
    let build_status = PersonaBuildStatus {
        status: "creating".to_string(),
        display_name: display_name.clone(),
        model: model.clone(),
        sex: sex.clone(),
        age_range: age_range.clone(),
        vibe: vibe.clone(),
        world: world.clone(),
        connects_by: connects_by.clone(),
        persona_name: persona_name.clone(),
    };
    write_persona_build_status(&app, &build_status);

    let message = format!(
        "Create a new persona using the create_skill tool based on these preferences:\n\
        - Gender: {sex}\n\
        - Age range: {age_range}\n\
        - Vibe: {vibe}\n\
        - World: {world}\n\
        - Connects by: {connects_by}\n\
        - Name: {persona_name}\n\n\
        Follow the persona skill creation guide. The skill name must start with \"persona_\"."
    );

    let conversation = vec![OllamaMessage {
        role: "user".to_string(),
        content: message,
        tool_calls: None,
        images: None,
        brief: None,
    }];

    let result = run_headless(&app, conversation, &model, None, None).await;

    match &result {
        Ok(_) => {
            let after_names: std::collections::HashSet<String> =
                list_runtime_persona_names(&app).into_iter().collect();
            let new_display = after_names
                .difference(&before_names)
                .next()
                .map(|dir_name| {
                    // Prefer display_name from persona_config.json if the LLM wrote one
                    if let Ok(data_dir) = app.path().app_data_dir() {
                        let cfg_path = data_dir
                            .join("custom_personas")
                            .join(dir_name)
                            .join("persona_config.json");
                        if let Ok(text) = std::fs::read_to_string(&cfg_path) {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                                if let Some(dn) = v.get("display_name").and_then(|x| x.as_str()) {
                                    if !dn.is_empty() {
                                        return dn.to_string();
                                    }
                                }
                            }
                        }
                    }
                    // Fallback: humanize the directory slug
                    let slug = dir_name
                        .strip_prefix("persona_")
                        .or_else(|| dir_name.strip_prefix("persona-"))
                        .unwrap_or(dir_name);
                    let mut name = slug.replace('_', " ");
                    name = name.split_whitespace()
                        .map(|w| {
                            let mut c = w.chars();
                            match c.next() {
                                None => String::new(),
                                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    name
                })
                .unwrap_or(display_name);
            write_persona_build_status(&app, &PersonaBuildStatus {
                status: "done".to_string(),
                display_name: new_display,
                ..build_status
            });
        }
        Err(_) => {
            write_persona_build_status(&app, &PersonaBuildStatus {
                status: "interrupted".to_string(),
                ..build_status
            });
        }
    }

    result
}
