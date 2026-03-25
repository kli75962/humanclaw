mod fs;
mod generate;

pub use fs::{PostComment, PostMeta};

use serde::Serialize;
use crate::characters::list_characters_fs;

// ── Basic CRUD ───────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_posts(app: tauri::AppHandle) -> Vec<PostMeta> {
    fs::list_posts(&app)
}

#[tauri::command]
pub fn save_post(app: tauri::AppHandle, post: PostMeta) -> Result<(), String> {
    fs::save_post(&app, &post)
}

#[tauri::command]
pub fn delete_post(app: tauri::AppHandle, id: String) -> Result<(), String> {
    fs::delete_post(&app, &id)
}

#[tauri::command]
pub fn like_post(app: tauri::AppHandle, id: String) -> Result<u32, String> {
    fs::like_post(&app, &id)
}

#[tauri::command]
pub fn unlike_post(app: tauri::AppHandle, id: String) -> Result<u32, String> {
    fs::unlike_post(&app, &id)
}

// ── Comments ─────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_comments(app: tauri::AppHandle, post_id: String) -> Vec<PostComment> {
    fs::list_comments(&app, &post_id)
}

#[tauri::command]
pub fn add_comment(app: tauri::AppHandle, comment: PostComment) -> Result<(), String> {
    fs::add_comment(&app, &comment)
}

// ── AI Generation ─────────────────────────────────────────────────────────────

/// Generate and save a new post for a character.
/// Timestamp is inferred from the post content (morning coffee → 8-10am local, etc.).
#[tauri::command]
pub async fn generate_character_post(
    app: tauri::AppHandle,
    character_id: String,
    context: Option<String>,
) -> Result<PostMeta, String> {
    let characters = list_characters_fs(&app);
    let character = characters
        .iter()
        .find(|c| c.id == character_id)
        .ok_or_else(|| format!("Character {character_id} not found"))?;

    let (created_at, text) = generate::generate_post_text(&app, character, context.as_deref()).await?;

    let post = PostMeta {
        id: uuid_v4(),
        character_id: character_id.clone(),
        text,
        image: None,
        created_at,
        like_count: 0,
    };

    fs::save_post(&app, &post)?;
    Ok(post)
}

/// Trigger other characters to comment on a post with naturally delayed timestamps.
#[tauri::command]
pub async fn trigger_character_reactions(
    app: tauri::AppHandle,
    post_id: String,
) -> Result<(), String> {
    let posts = fs::list_posts(&app);
    let post = posts
        .iter()
        .find(|p| p.id == post_id)
        .ok_or_else(|| "Post not found".to_string())?
        .clone();

    let all_characters = list_characters_fs(&app);

    let author_name = all_characters
        .iter()
        .find(|c| c.id == post.character_id)
        .map(|c| c.name.as_str())
        .unwrap_or("them")
        .to_string();

    for character in all_characters.iter().filter(|c| c.id != post.character_id) {
        if pseudo_rand(&character.id, &post_id) < 60 {
            if let Ok(comment_text) = generate::generate_comment_text(
                &app, character, &author_name, &post.text,
            ).await {
                // Delayed timestamp: each character reacts at a different time
                let seed = str_hash(&format!("reaction{}{}", character.id, post_id));
                let comment = PostComment {
                    id: uuid_v4(),
                    post_id: post_id.clone(),
                    author_id: character.id.clone(),
                    text: comment_text,
                    created_at: generate::pick_comment_timestamp(&post.created_at, seed),
                };
                let _ = fs::add_comment(&app, &comment);
            }
        }
    }

    Ok(())
}

/// Result of a character reacting to a user post.
/// action = "dm"  → text is a DM the frontend should inject into the character's chat
/// action = "comment" → already saved; text is the comment content
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReactResult {
    pub character_id: String,
    pub action: String,
    pub text: String,
    pub comment_id: Option<String>,
}

/// When a user creates a post, each character decides:
///   30% chance → DM (wants a deeper conversation, asks questions)
///   70% chance → comment (short reaction saved with delayed timestamp)
///
/// Frontend is responsible for injecting DM results into the character's chat.
#[tauri::command]
pub async fn react_to_user_post(
    app: tauri::AppHandle,
    post_id: String,
) -> Result<Vec<ReactResult>, String> {
    let posts = fs::list_posts(&app);
    let post = posts
        .iter()
        .find(|p| p.id == post_id)
        .ok_or_else(|| "Post not found".to_string())?
        .clone();

    let characters = list_characters_fs(&app);
    let mut results = Vec::new();

    for character in &characters {
        let seed = str_hash(&format!("react{}{}", character.id, post_id));

        if seed % 100 < 30 {
            // DM path: generate a conversational message that invites a reply
            let trigger = format!(
                "The user posted: \"{}\". You want to start a real conversation about it — ask something genuine.",
                post.text
            );
            if let Ok(text) = generate::generate_dm_text(&app, character, &trigger).await {
                results.push(ReactResult {
                    character_id: character.id.clone(),
                    action: "dm".to_string(),
                    text,
                    comment_id: None,
                });
            }
        } else {
            // Comment path: short reaction with delayed timestamp
            let comment_seed = str_hash(&format!("comment{}{}", character.id, post_id));
            if let Ok(comment_text) = generate::generate_comment_text(
                &app, character, "you", &post.text,
            ).await {
                let comment = PostComment {
                    id: uuid_v4(),
                    post_id: post_id.clone(),
                    author_id: character.id.clone(),
                    text: comment_text.clone(),
                    created_at: generate::pick_comment_timestamp(&post.created_at, comment_seed),
                };
                let comment_id = comment.id.clone();
                let _ = fs::add_comment(&app, &comment);
                results.push(ReactResult {
                    character_id: character.id.clone(),
                    action: "comment".to_string(),
                    text: comment_text,
                    comment_id: Some(comment_id),
                });
            }
        }
    }

    Ok(results)
}

/// Generate a DM from a character and return the text (frontend handles routing into chat).
#[tauri::command]
pub async fn generate_character_dm(
    app: tauri::AppHandle,
    character_id: String,
    trigger: String,
) -> Result<String, String> {
    let characters = list_characters_fs(&app);
    let character = characters
        .iter()
        .find(|c| c.id == character_id)
        .ok_or_else(|| format!("Character {character_id} not found"))?;

    generate::generate_dm_text(&app, character, &trigger).await
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:x}-{:x}", t, t.wrapping_mul(6364136223846793005))
}

/// FNV-1a hash of a string → u64, used as a deterministic seed.
fn str_hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

fn pseudo_rand(seed1: &str, seed2: &str) -> u8 {
    (str_hash(&format!("{seed1}{seed2}")) % 100) as u8
}
