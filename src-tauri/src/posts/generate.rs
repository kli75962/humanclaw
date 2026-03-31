/// Single-shot LLM generation for IG content (posts, comments, DMs).
/// Does NOT stream and does NOT use tools — just system + user → assistant.
use futures_util::StreamExt;
use serde::Serialize;
use tauri::AppHandle;
use chrono::{DateTime, Duration, Local, Utc};

use crate::characters::CharacterMeta;
use crate::model::ollama::types::{OllamaChunk, OllamaMessage};
use crate::skills::get_skill_content;

// ── Output parsing ────────────────────────────────────────────────────────────

/// Parse the TIME tag and post text from LLM output.
/// Expected format:  TIME:<ISO 8601>\n<post text>
/// Returns (timestamp_rfc3339, post_text).
/// If the tag is absent or unparseable the timestamp falls back to 2 hours ago.
pub fn parse_post_output(raw: &str) -> (String, String) {
    let raw = raw.trim();
    if let Some(rest) = raw.strip_prefix("TIME:") {
        let (time_str, body) = rest.split_once('\n').unwrap_or((rest, ""));
        let text = body.trim().to_string();

        // Validate: must be a real datetime in the past
        if let Ok(dt) = DateTime::parse_from_rfc3339(time_str.trim()) {
            let utc = dt.with_timezone(&Utc);
            let now = Utc::now();
            // Clamp to past in case LLM accidentally writes a future time
            let clamped = utc.min(now - Duration::minutes(5));
            return (clamped.to_rfc3339(), text);
        }
    }

    // Fallback: 2 hours ago, return full raw as text
    let fallback = (Utc::now() - Duration::hours(2)).to_rfc3339();
    (fallback, raw.to_string())
}

/// Pick a realistic comment timestamp: some time after the post was created.
/// Delay ranges from 5 minutes to ~4 hours based on a seed.
pub fn pick_comment_timestamp(post_created_at: &str, seed: u64) -> String {
    let post_utc = DateTime::parse_from_rfc3339(post_created_at)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now());

    let now_utc = Utc::now();
    let delay_mins = 5 + (seed % 235) as i64; // 5 min → ~4 hours
    let candidate = post_utc + Duration::minutes(delay_mins);

    // Never exceed current time
    candidate.min(now_utc).to_rfc3339()
}

#[derive(Serialize)]
struct SimpleRequest<'a> {
    model: &'a str,
    messages: Vec<OllamaMessage>,
    stream: bool,
}

/// Helper: Extract persona skill name from character persona field.
/// Tries to match persona names dynamically by looking for skill files.
fn get_persona_skill_name(character: &CharacterMeta) -> Option<String> {
    let persona_lower = character.persona.to_lowercase();

    // Try direct persona name: if it's "jk" or "persona_jk", construct skill name
    if persona_lower.starts_with("persona_") {
        // Already prefixed, use as-is
        return Some(character.persona.clone());
    }

    // Try to construct skill name from persona field
    let candidate = format!("persona_{}", persona_lower.replace(' ', "_"));
    if get_skill_content(&candidate).is_some() {
        return Some(candidate);
    }

    // If persona is a simple word, also try it without modification
    if get_skill_content(&format!("persona_{}", persona_lower)).is_some() {
        return Some(format!("persona_{}", persona_lower));
    }

    None
}

/// Extract user birthday from core memory.
fn extract_user_birthday(core: &str) -> Option<String> {
    let lines = core.lines();
    for line in lines {
        if line.contains("[USER_BIRTHDAY]") {
            if let Some(date) = line.split(':').nth(1) {
                return Some(date.trim().to_string());
            }
        }
    }
    None
}

/// Build the character identity + current datetime + user preferences block for the system prompt.
/// Now includes BOTH the general skill guide and the character's persona skill for authentic voice.
fn character_system(skill_content: &str, character: &CharacterMeta, core: &str) -> String {
    let now = Local::now();
    let datetime_str = now.format("%Y-%m-%dT%H:%M:%S%:z").to_string();
    let pref_block = if core.trim().is_empty() {
        String::new()
    } else {
        format!("\n\n[USER PREFERENCES]\n{}", core.trim())
    };

    // Extract user birthday for context
    let birthday_block = if let Some(birthday) = extract_user_birthday(core) {
        format!("\n\n[USER BIRTHDAY]\n{}", birthday)
    } else {
        String::new()
    };

    // Try to load the character's persona skill for voice guidance
    let persona_guide = if let Some(skill_name) = get_persona_skill_name(character) {
        if let Some(persona_skill_content) = get_skill_content(&skill_name) {
            format!("\n\n[YOUR VOICE GUIDE]\n{}", persona_skill_content)
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    format!(
        "You are {}.\n{}\nBackground: {}\n\n[CURRENT DATETIME]\n{}{}{}\n\n[GENERATION GUIDE]\n{}{}\n---\n[FOLLOW YOUR VOICE GUIDE ABOVE TO WRITE AUTHENTICALLY]",
        character.name, character.persona, character.background,
        datetime_str, pref_block, birthday_block, skill_content, persona_guide,
    )
}

/// Make a single non-streaming Ollama request and return the response text.
async fn complete_once(
    app: &AppHandle,
    model: &str,
    system: &str,
    user_prompt: &str,
) -> Result<String, String> {
    let messages = vec![
        OllamaMessage { role: "system".to_string(), content: system.to_string(), tool_calls: None, images: None },
        OllamaMessage { role: "user".to_string(), content: user_prompt.to_string(), tool_calls: None, images: None },
    ];
    let body = SimpleRequest { model, messages, stream: true };

    let response = crate::model::ollama::ollama_client()
        .post(crate::model::ollama::types::ollama_chat_url(app))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Ollama unreachable: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(format!("Ollama {status}: {text}"));
    }

    let mut byte_stream = response.bytes_stream();
    let mut content = String::new();

    while let Some(chunk) = byte_stream.next().await {
        let bytes = chunk.map_err(|e| format!("Stream error: {e}"))?;
        let text = String::from_utf8_lossy(&bytes);
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() { continue; }
            let Ok(parsed) = serde_json::from_str::<OllamaChunk>(line) else { continue };
            if let Some(ref msg) = parsed.message {
                content.push_str(&msg.content);
            }
            if parsed.done { break; }
        }
    }

    Ok(content.trim().to_string())
}

/// Generate a new post for a character.
/// Returns `(timestamp_rfc3339, post_text)` parsed from the LLM's TIME tag.
pub async fn generate_post_text(
    app: &AppHandle,
    character: &CharacterMeta,
    context: Option<&str>,
) -> Result<(String, String), String> {
    let skill = get_skill_content("generate-post").unwrap_or("");
    let core = crate::tools::read_core(app);
    let system = character_system(skill, character, &core); // Persona skill loaded automatically

    let prompt = match context {
        Some(ctx) if !ctx.is_empty() => {
            format!("Write a post. You may draw subtle inspiration from this context if it feels natural: {ctx}")
        }
        _ => "Write a post that reflects your personality and current state of mind.".to_string(),
    };

    let raw = complete_once(app, &character.model, &system, &prompt).await?;
    let (ts, text) = parse_post_output(&raw);
    if text.is_empty() {
        return Err("Generated post has no text content".to_string());
    }
    Ok((ts, text))
}

/// Generate a comment text for a character on a given post.
/// `prior_comments` is a list of (author_name, comment_text) already on the post — may be empty.
pub async fn generate_comment_text(
    app: &AppHandle,
    character: &CharacterMeta,
    post_author_name: &str,
    post_text: &str,
    prior_comments: &[(String, String)],
) -> Result<String, String> {
    let skill = get_skill_content("post-comment").unwrap_or("");
    let core = crate::tools::read_core(app);
    let system = character_system(skill, character, &core);

    let mut prompt = format!("{post_author_name} posted: \"{post_text}\"");
    if !prior_comments.is_empty() {
        prompt.push_str("\n\nComments so far:");
        for (author, text) in prior_comments {
            prompt.push_str(&format!("\n{author}: {text}"));
        }
    }
    prompt.push_str("\n\nWrite a comment on this post.");

    complete_once(app, &character.model, &system, &prompt).await
}

/// Ask the character how much they resonate with / like a post.
/// Returns a score 0–100, used directly as a like probability %.
pub async fn generate_like_score(
    app: &AppHandle,
    character: &CharacterMeta,
    post_text: &str,
) -> u8 {
    let core = crate::tools::read_core(app);
    let system = character_system("", character, &core);
    let prompt = format!(
        "Post: \"{post_text}\"\n\nHow much do you personally like or resonate with this post? Reply with a single integer 0-100. Nothing else."
    );
    match complete_once(app, &character.model, &system, &prompt).await {
        Ok(s) => s.trim().parse::<u8>().unwrap_or(0),
        Err(_) => 0,
    }
}

/// Generate a DM from a character to the user.
/// `trigger` describes why the character is reaching out.
pub async fn generate_dm_text(
    app: &AppHandle,
    character: &CharacterMeta,
    trigger: &str,
) -> Result<String, String> {
    let skill = get_skill_content("post-dm").unwrap_or("");
    let core = crate::tools::read_core(app);
    let system = character_system(skill, character, &core);

    let prompt = format!("Send a direct message to the user. Trigger: {trigger}");

    complete_once(app, &character.model, &system, &prompt).await
}
