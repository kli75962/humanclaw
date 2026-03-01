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

/// Ask Ollama to extract new user preferences from a finished conversation exchange.
///
/// `user_message`      — the last thing the user typed
/// `assistant_response` — the final plain-text reply the LLM gave
///
/// Returns a list of concise preference strings ready to store as memories.
/// Returns an empty vec if nothing new was learned or if the call fails.
pub async fn extract_memories(
    model: &str,
    user_message: &str,
    assistant_response: &str,
) -> Vec<String> {
    let prompt = format!(
        r#"Analyze this conversation and extract ONLY explicit personal preferences, habits, or facts stated by the user that are worth remembering for future conversations.

User: "{user_message}"
Assistant: "{assistant_response}"

Rules:
- Only extract information the user explicitly stated — do NOT infer or guess.
- Each item must be a concise, self-contained sentence starting with "User".
- Examples: "User prefers Traditional Chinese", "User likes dark mode", "User's name is Alex".
- Return a JSON array of strings. Return [] if nothing new was learned.
- Do NOT extract things the assistant did, tool calls, or task results.

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
