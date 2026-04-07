/// Single-shot LLM generation for IG content (posts, comments, DMs).
/// Does NOT stream and does NOT use tools — just system + user → assistant.
use futures_util::StreamExt;
use serde::Serialize;
use tauri::AppHandle;
use chrono::{DateTime, Duration, Local, Utc};

use crate::social::character::CharacterMeta;
use crate::ai::ollama::types::{OllamaChunk, OllamaMessage};
use crate::skills::get_skill_content;

// ── Output parsing ────────────────────────────────────────────────────────────

/// Memory suffix appended by LLM after the main content.
/// Format (appended after content):
///   ---MEMORY---
///   BRIEF:<1-2 sentence summary>
///   IMPORTANCE:<0-100>
pub struct MemorySuffix {
    pub brief: String,
    pub importance: u8,
}

/// Split raw LLM output into (main_content, Option<MemorySuffix>).
/// Strips the `---MEMORY---` block if present.
fn split_memory_suffix(raw: &str) -> (&str, Option<MemorySuffix>) {
    if let Some(idx) = raw.find("---MEMORY---") {
        let content = raw[..idx].trim_end();
        let tail = &raw[idx + 12..]; // skip "---MEMORY---"
        let mut brief = String::new();
        let mut importance: u8 = 20;
        for line in tail.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("BRIEF:") {
                brief = rest.trim().to_string();
            } else if let Some(rest) = line.strip_prefix("IMPORTANCE:") {
                importance = rest.trim().parse::<u8>().unwrap_or(20).clamp(0, 100);
            }
        }
        let suffix = if brief.is_empty() {
            None
        } else {
            Some(MemorySuffix { brief, importance })
        };
        (content, suffix)
    } else {
        (raw.trim(), None)
    }
}

/// Parse the TIME tag and post text from LLM output.
/// Extended format:
///   TIME:<ISO 8601>
///   <post text>
///   ---MEMORY---
///   BRIEF:<summary>
///   IMPORTANCE:<score>
/// Returns (timestamp_rfc3339, post_text, Option<MemorySuffix>).
pub fn parse_post_output_full(raw: &str) -> (String, String, Option<MemorySuffix>) {
    let raw = raw.trim();
    let (content, mem) = split_memory_suffix(raw);
    if let Some(rest) = content.strip_prefix("TIME:") {
        let (time_str, body) = rest.split_once('\n').unwrap_or((rest, ""));
        let text = body.trim().to_string();
        if let Ok(dt) = DateTime::parse_from_rfc3339(time_str.trim()) {
            let utc = dt.with_timezone(&Utc);
            let now = Utc::now();
            let clamped = utc.min(now - Duration::minutes(5));
            return (clamped.to_rfc3339(), text, mem);
        }
    }
    let fallback = (Utc::now() - Duration::hours(2)).to_rfc3339();
    (fallback, content.to_string(), mem)
}

/// Parse a comment response that may contain a memory suffix.
/// Returns (comment_text, Option<MemorySuffix>).
pub fn parse_comment_output_full(raw: &str) -> (String, Option<MemorySuffix>) {
    let raw = raw.trim();
    let (content, mem) = split_memory_suffix(raw);
    (content.to_string(), mem)
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
/// Tries both dash and underscore variants since built-in skills use dashes (persona-jk)
/// while user-created skills use underscores (persona_jk).
fn get_persona_skill_name(app: &AppHandle, character: &CharacterMeta) -> Option<String> {
    let persona_lower = character.persona.to_lowercase();

    // Strip existing "persona_" or "persona-" prefix to get the bare slug
    let slug = if let Some(s) = persona_lower.strip_prefix("persona_") {
        s.to_string()
    } else if let Some(s) = persona_lower.strip_prefix("persona-") {
        s.to_string()
    } else {
        persona_lower.replace(' ', "_")
    };

    // Try underscore variant first (user-created), then dash variant (built-in)
    let underscore = format!("persona_{slug}");
    let dash = format!("persona-{slug}");

    if get_skill_content(app, &underscore).is_some() {
        return Some(underscore);
    }
    if get_skill_content(app, &dash).is_some() {
        return Some(dash);
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
/// Now includes BOTH the general skill guide and the character's persona skill for authentic voice,
/// plus a brief of past posts/comments from character memory.
fn character_system_with_memory(app: &AppHandle, skill_content: &str, character: &CharacterMeta, core: &str, memory_context: &str, target_time: Option<&str>) -> String {
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
    let persona_guide = if let Some(skill_name) = get_persona_skill_name(app, character) {
        if let Some(persona_skill_content) = get_skill_content(app, &skill_name) {
            format!("\n\n[YOUR VOICE GUIDE]\n{}", persona_skill_content)
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let memory_block = if memory_context.trim().is_empty() {
        String::new()
    } else {
        format!("\n\n{}", memory_context.trim())
    };

    let target_time_block = if let Some(tt) = target_time {
        format!("\n\n[TARGET POST TIME]\n{}\nWrite a post that would naturally occur at this time of day. Pick your timestamp within ±20 minutes of this target.", tt)
    } else {
        String::new()
    };

    format!(
        "You are {}.\n{}\nBackground: {}\n\n[CURRENT DATETIME]\n{}{}{}{}{}\n\n[GENERATION GUIDE]\n{}{}\n---\n[FOLLOW YOUR VOICE GUIDE ABOVE TO WRITE AUTHENTICALLY]",
        character.name, character.persona, character.background,
        datetime_str, pref_block, birthday_block, memory_block, target_time_block, skill_content, persona_guide,
    )
}

fn character_system(app: &AppHandle, skill_content: &str, character: &CharacterMeta, core: &str) -> String {
    character_system_with_memory(app, skill_content, character, core, "", None)
}

/// Make a single non-streaming Ollama request and return the response text.
async fn complete_once(
    app: &AppHandle,
    model: &str,
    system: &str,
    user_prompt: &str,
) -> Result<String, String> {
    let messages = vec![
        OllamaMessage { role: "system".to_string(), content: system.to_string(), tool_calls: None, images: None, brief: None },
        OllamaMessage { role: "user".to_string(), content: user_prompt.to_string(), tool_calls: None, images: None, brief: None },
    ];
    let body = SimpleRequest { model, messages, stream: true };

    let response = crate::ai::ollama::ollama_client()
        .post(crate::ai::ollama::types::ollama_chat_url(app))
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

/// Ask the character whether they would like and/or comment on a post.
/// Returns `(will_like, will_comment)` — a single LLM call, no probability gymnastics.
/// The LLM reasons from its persona about whether it would genuinely react.
pub async fn generate_reaction_decision(
    app: &AppHandle,
    character: &CharacterMeta,
    post_text: &str,
) -> (bool, bool) {
    let core = crate::tools::read_core(app);
    let system = character_system(app, "", character, &core);
    let prompt = format!(
        "Post: \"{post_text}\"\n\n\
        Based on who you are, decide:\n\
        - Would you like (react positively to) this post?\n\
        - Would you comment on it?\n\n\
        Reply with JSON only: {{\"like\": true/false, \"comment\": true/false}}\n\
        Be authentic to your persona. Not every post needs a reaction."
    );
    match complete_once(app, &character.model, &system, &prompt).await {
        Ok(raw) => {
            // Try to find JSON in the response
            let json_str = if let Some(start) = raw.find('{') {
                if let Some(end) = raw.rfind('}') {
                    &raw[start..=end]
                } else { &raw }
            } else { &raw };
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) {
                let like    = v.get("like").and_then(|x| x.as_bool()).unwrap_or(false);
                let comment = v.get("comment").and_then(|x| x.as_bool()).unwrap_or(false);
                (like, comment)
            } else {
                (false, false)
            }
        }
        Err(_) => (false, false),
    }
}

/// Ask the LLM to decide what times this character will post today.
/// Returns a sorted Vec of "HH:MM" strings (24-hour), up to `max_posts`.
pub async fn decide_posting_times(
    app: &AppHandle,
    character: &CharacterMeta,
    max_posts: u8,
) -> Vec<String> {
    let core = crate::tools::read_core(app);
    let system = character_system(app, "", character, &core);
    let now = chrono::Local::now().format("%H:%M").to_string();
    let prompt = format!(
        "It's currently {now}. Decide what times you will post on social media today.\n\
        Choose 0 to {max_posts} times that feel natural for your lifestyle and current mood.\n\
        Reply with JSON only: {{\"times\":[\"HH:MM\",...]}} — use 24-hour format.\n\
        Times can be in the past (you posted earlier) or future (you plan to post later).\n\
        Example: {{\"times\":[\"09:15\",\"14:30\",\"22:00\"]}}"
    );

    let raw = match complete_once(app, &character.model, &system, &prompt).await {
        Ok(r) => r,
        Err(_) => return vec![],
    };

    let json_str = if let (Some(s), Some(e)) = (raw.find('{'), raw.rfind('}')) {
        raw[s..=e].to_string()
    } else {
        return vec![];
    };

    let Ok(v) = serde_json::from_str::<serde_json::Value>(&json_str) else {
        return vec![];
    };

    let Some(arr) = v.get("times").and_then(|x| x.as_array()) else {
        return vec![];
    };

    let mut times: Vec<String> = arr
        .iter()
        .filter_map(|t| t.as_str())
        .filter(|t| t.len() == 5 && t.as_bytes().get(2) == Some(&b':'))
        .map(|t| t.to_string())
        .collect();

    times.sort();
    times.dedup();
    times.truncate(max_posts as usize);
    times
}

/// Generate a DM from a character to the user.
/// `trigger` describes why the character is reaching out.
pub async fn generate_dm_text(
    app: &AppHandle,
    character: &CharacterMeta,
    trigger: &str,
) -> Result<String, String> {
    let skill = get_skill_content(app, "post-dm").unwrap_or_default();
    let core = crate::tools::read_core(app);
    let system = character_system(app, &skill, character, &core);

    let prompt = format!("Send a direct message to the user. Trigger: {trigger}");

    complete_once(app, &character.model, &system, &prompt).await
}

/// Result of a combined post generation: content + memory brief embedded in one LLM call.
#[allow(dead_code)]
pub struct PostWithMemory {
    pub timestamp: String,
    pub text: String,
    pub brief: String,
    pub importance: u8,
}

/// Result of a combined comment generation.
pub struct CommentWithMemory {
    pub text: String,
    pub brief: String,
    pub importance: u8,
}

/// Generate a post with character memory context injected.
/// The LLM writes the post AND the memory brief in one response (see generate-post SKILL.md).
pub async fn generate_post_text_with_memory(
    app: &AppHandle,
    character: &CharacterMeta,
    context: Option<&str>,
    target_time: Option<&str>,
) -> Result<PostWithMemory, String> {
    let skill = get_skill_content(app, "generate-post").unwrap_or_default();
    let core = crate::tools::read_core(app);
    let memory_context = super::character_memory::build_memory_context(app, &character.id);
    let system = character_system_with_memory(app, &skill, character, &core, &memory_context, target_time);

    let prompt = match context {
        Some(ctx) if !ctx.is_empty() => {
            format!("Write a post. You may draw subtle inspiration from this context if it feels natural: {ctx}")
        }
        _ => "Write a post that reflects your personality and current state of mind.".to_string(),
    };

    let raw = complete_once(app, &character.model, &system, &prompt).await?;
    let (ts, text, mem) = parse_post_output_full(&raw);
    if text.is_empty() {
        return Err("Generated post has no text content".to_string());
    }
    let (brief, importance) = mem
        .map(|m| (m.brief, m.importance))
        .unwrap_or_else(|| (text.chars().take(120).collect(), 20));
    Ok(PostWithMemory { timestamp: ts, text, brief, importance })
}

/// Generate a comment with character memory context injected.
/// The LLM writes the comment AND the memory brief in one response (see post-comment SKILL.md).
pub async fn generate_comment_text_with_memory(
    app: &AppHandle,
    character: &CharacterMeta,
    post_author_name: &str,
    post_text: &str,
    prior_comments: &[(String, String)],
) -> Result<CommentWithMemory, String> {
    let skill = get_skill_content(app, "post-comment").unwrap_or_default();
    let core = crate::tools::read_core(app);
    let memory_context = super::character_memory::build_memory_context(app, &character.id);
    let system = character_system_with_memory(app, &skill, character, &core, &memory_context, None);

    let mut prompt = format!("{post_author_name} posted: \"{post_text}\"");
    if !prior_comments.is_empty() {
        prompt.push_str("\n\nComments so far:");
        for (author, text) in prior_comments {
            prompt.push_str(&format!("\n{author}: {text}"));
        }
    }
    prompt.push_str("\n\nWrite a comment on this post.");

    let raw = complete_once(app, &character.model, &system, &prompt).await?;
    let (text, mem) = parse_comment_output_full(&raw);
    let (brief, importance) = mem
        .map(|m| (m.brief, m.importance))
        .unwrap_or_else(|| (text.chars().take(120).collect(), 20));
    Ok(CommentWithMemory { text, brief, importance })
}
