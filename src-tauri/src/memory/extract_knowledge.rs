use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct ExtractResponse {
    message: ExtractMessage,
}

#[derive(Deserialize)]
struct ExtractMessage {
    content: String,
}

/// Ask Ollama to extract reusable navigation knowledge from a completed agentic session.
///
/// `user_goal` — the user's original request (e.g. "turn off WiFi")  
/// `tool_log`  — a compact text log of all tool calls executed, in order
///
/// Returns a list of concise navigation-path strings to store as general knowledge.
/// Returns an empty vec if nothing useful was learned or if the call fails.
pub async fn extract_knowledge(model: &str, user_goal: &str, tool_log: &str) -> Vec<String> {
    if tool_log.trim().is_empty() {
        return Vec::new();
    }

    let prompt = format!(
        r#"An AI agent just completed this task on an Android phone:
Goal: "{user_goal}"

Tools called in order:
{tool_log}

Extract ONLY reusable navigation facts that would help find the same settings or buttons faster next time.

Rules:
- Each fact must describe WHERE something is located, not what happened.
- Format: "To [action]: [App] > [Screen] > [element]"
- Examples:
  "To turn off WiFi: Settings > Network & internet > Internet > WiFi toggle"
  "To enable dark mode: Settings > Display & touch > Dark mode toggle"
  "To clear app cache: Settings > Apps > [App name] > Storage > Clear cache"
- Only include facts about SPECIFIC UI locations, not general instructions.
- Skip tool calls that were retries or errors.
- Return a JSON array of strings. Return [] if no useful location facts were learned.

Return ONLY the raw JSON array, no markdown, no explanation."#
    );

    let client = reqwest::Client::new();
    let body = json!({
        "model": model,
        "messages": [{ "role": "user", "content": prompt }],
        "stream": false,
    });

    let Ok(resp) = client
        .post("http://10.0.2.2:11434/api/chat")
        .json(&body)
        .send()
        .await
    else {
        return Vec::new();
    };

    let Ok(parsed) = resp.json::<ExtractResponse>().await else {
        return Vec::new();
    };

    let raw = parsed.message.content.trim().to_string();

    // Strip optional markdown code fences
    let json_str = raw
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    serde_json::from_str::<Vec<String>>(json_str).unwrap_or_default()
}
