mod fs;
mod generate;

pub use fs::{PostComment, PostMeta};

use serde::Serialize;
use crate::characters::list_characters_fs;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DmResult {
    pub character_id: String,
    pub text: String,
}

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
    // Delete the post
    fs::delete_post(&app, &id)?;

    // Also remove any queue entries for this post to keep things clean
    let mut queue = pg::load_all(&app);
    queue.retain(|e| e.post_id.as_ref() != Some(&id));
    let _ = pg::update_entries_batch(&app, &queue); // Non-critical, log silently

    Ok(())
}

#[tauri::command]
pub fn like_post(app: tauri::AppHandle, id: String) -> Result<u32, String> {
    fs::like_post(&app, &id)
}

#[tauri::command]
pub fn unlike_post(app: tauri::AppHandle, id: String) -> Result<u32, String> {
    fs::unlike_post(&app, &id)
}

#[tauri::command]
pub fn hide_post(app: tauri::AppHandle, post_id: String) -> Result<(), String> {
    fs::hide_post(&app, &post_id)
}

#[tauri::command]
pub fn record_post_preference(
    app: tauri::AppHandle,
    post_id: String,
    character_id: String,
    post_text: String,
    post_image: Option<String>,
    user_reason: String,
) -> Result<(), String> {
    fs::record_post_preference(&app, &post_id, &character_id, &post_text, post_image.as_deref(), &user_reason)
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
/// This command now integrates with the post generation queue for crash recovery.
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
        text: text.clone(),
        image: None,
        created_at: created_at.clone(),
        like_count: 0,
    };

    fs::save_post(&app, &post)?;

    // Log this to the post generation queue for crash recovery tracking
    let mut queue_entry = pg::PostGenEntry::new(character_id, text, created_at);
    queue_entry.mark_post_created(post.id.clone());
    let _ = pg::save_queue_entry(&app, queue_entry); // Non-critical, log silently

    Ok(post)
}

/// Return true if the character's persona suggests an extroverted personality.
fn is_extrovert(persona: &str) -> bool {
    let lower = persona.to_lowercase();
    lower.contains("extrovert") || lower.contains("outgoing") || lower.contains("energetic")
        || lower.contains("enthusiastic") || lower.contains("lively") || lower.contains("talkative")
        || lower.contains("confident") || lower.contains("vibrant") || lower.contains("social")
        || lower.contains("friendly") || lower.contains("cheerful")
}

/// Resolve a character's display name for comment context.
fn author_display_name<'a>(characters: &'a [crate::characters::CharacterMeta], author_id: &str) -> &'a str {
    if author_id == "user" { return "User"; }
    characters.iter().find(|c| c.id == author_id).map(|c| c.name.as_str()).unwrap_or("Unknown")
}

/// Build (author_name, text) pairs from saved comments for a post.
fn load_comment_context(app: &tauri::AppHandle, post_id: &str, characters: &[crate::characters::CharacterMeta]) -> Vec<(String, String)> {
    fs::list_comments(app, post_id)
        .into_iter()
        .map(|c| (author_display_name(characters, &c.author_id).to_string(), c.text))
        .collect()
}

/// Trigger other characters to comment on a post with naturally delayed timestamps.
/// Chance is personality-based: extrovert ~45%, introvert ~2%.
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

    let prior = load_comment_context(&app, &post_id, &all_characters);

    for character in all_characters.iter().filter(|c| c.id != post.character_id) {
        // Like: score from LLM → used as probability %
        let like_score = generate::generate_like_score(&app, character, &post.text).await;
        if pseudo_rand(&character.id, &format!("like{post_id}")) < like_score {
            let _ = fs::like_post(&app, &post_id);
        }

        // Comment: extrovert 45%, introvert 2%
        let chance: u8 = if is_extrovert(&character.persona) { 45 } else { 2 };
        if pseudo_rand(&character.id, &post_id) < chance {
            if let Ok(comment_text) = generate::generate_comment_text(
                &app, character, &author_name, &post.text, &prior,
            ).await {
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

/// When a user comments on their own post after a character has commented,
/// that character has a 70% chance to reply.
#[tauri::command]
pub async fn react_to_user_comment(
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
    let prior = load_comment_context(&app, &post_id, &all_characters);

    // Collect characters who have already commented (before the user's current comment).
    let existing_comments = fs::list_comments(&app, &post_id);
    let already_commented: std::collections::HashSet<&str> = existing_comments
        .iter()
        .filter(|c| c.author_id != "user")
        .map(|c| c.author_id.as_str())
        .collect();

    let author_name = all_characters
        .iter()
        .find(|c| c.id == post.character_id)
        .map(|c| c.name.as_str())
        .unwrap_or("them")
        .to_string();

    for character in &all_characters {
        if !already_commented.contains(character.id.as_str()) {
            continue;
        }
        if pseudo_rand(&character.id, &format!("ucreply{post_id}")) < 70 {
            if let Ok(comment_text) = generate::generate_comment_text(
                &app, character, &author_name, &post.text, &prior,
            ).await {
                let seed = str_hash(&format!("ucreact{}{}", character.id, post_id));
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


/// When a user creates a post:
///   extrovert → 50% chance to react; of those, 5% DM instead of comment.
///   introvert → 3% chance to comment only.
/// Returns DM entries so the frontend can inject them into the character's chat.
#[tauri::command]
pub async fn react_to_user_post(
    app: tauri::AppHandle,
    post_id: String,
) -> Result<Vec<DmResult>, String> {
    let posts = fs::list_posts(&app);
    let post = posts
        .iter()
        .find(|p| p.id == post_id)
        .ok_or_else(|| "Post not found".to_string())?
        .clone();

    let characters = list_characters_fs(&app);
    let mut dms = Vec::new();

    for character in &characters {
        // Like: score from LLM → used as probability %
        let like_score = generate::generate_like_score(&app, character, &post.text).await;
        if pseudo_rand(&character.id, &format!("like{post_id}")) < like_score {
            let _ = fs::like_post(&app, &post_id);
        }

        let extrovert = is_extrovert(&character.persona);
        let chance: u8 = if extrovert { 50 } else { 3 };
        if pseudo_rand(&character.id, &post_id) >= chance {
            continue;
        }

        // Extroverts: 5% chance to DM instead of comment.
        if extrovert && pseudo_rand(&character.id, &format!("dm{post_id}")) < 5 {
            let trigger = format!(
                "The user posted: \"{}\". React naturally and start a conversation.",
                post.text
            );
            if let Ok(text) = generate::generate_dm_text(&app, character, &trigger).await {
                dms.push(DmResult { character_id: character.id.clone(), text });
            }
        } else {
            let comment_seed = str_hash(&format!("comment{}{}", character.id, post_id));
            if let Ok(comment_text) = generate::generate_comment_text(
                &app, character, "you", &post.text, &[],
            ).await {
                let comment = PostComment {
                    id: uuid_v4(),
                    post_id: post_id.clone(),
                    author_id: character.id.clone(),
                    text: comment_text,
                    created_at: generate::pick_comment_timestamp(&post.created_at, comment_seed),
                };
                let _ = fs::add_comment(&app, &comment);
            }
        }
    }

    Ok(dms)
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

// ── Queue Integration ────────────────────────────────────────────────────────

use crate::queue::post_gen as pg;

/// Resume any pending post generation queue entries (called on app startup).
/// Processes posts that were interrupted due to app crash/network issues.
#[tauri::command]
pub async fn resume_post_gen_queue(app: tauri::AppHandle) -> Result<u32, String> {
    let mut pending = pg::load_pending(&app);
    let mut completed = 0;

    for entry in &mut pending {
        // Skip if it's already in a terminal failure state
        if entry.state == pg::PostGenState::Failed {
            continue;
        }

        // If post text was generated but not yet saved, save it now
        if entry.state == pg::PostGenState::PostGenerating {
            if entry.post_id.is_some() {
                // Post was already created, move to reactions
                entry.start_reactions();
            } else {
                // Create the post from generated text
                let post = PostMeta {
                    id: uuid_v4(),
                    character_id: entry.character_id.clone(),
                    text: entry.generated_text.clone(),
                    image: None,
                    created_at: entry.generated_timestamp.clone(),
                    like_count: 0,
                };

                if let Err(e) = fs::save_post(&app, &post) {
                    entry.mark_failed(format!("Failed to save post: {e}"));
                    continue;
                }

                entry.mark_post_created(post.id.clone());
            }
        }

        // If post exists but reactions haven't been processed
        let post_id_clone = entry.post_id.clone();
        if let Some(post_id) = post_id_clone {
            if entry.state == pg::PostGenState::PostCreated {
                entry.start_reactions();
            }

            // Generate reactions if still in progress
            if entry.state == pg::PostGenState::ReactionsInProgress {
                // Generate comments from other characters
                let posts = fs::list_posts(&app);
                if let Some(post) = posts.iter().find(|p| p.id == post_id) {
                    let all_characters = list_characters_fs(&app);
                    let author_name = all_characters
                        .iter()
                        .find(|c| c.id == post.character_id)
                        .map(|c| c.name.as_str())
                        .unwrap_or("them")
                        .to_string();

                    let prior = load_comment_context(&app, &post_id, &all_characters);

                    for character in all_characters.iter().filter(|c| c.id != post.character_id) {
                        // Generate like (if not already tracked)
                        if !entry.likes.iter().any(|l| l.character_id == character.id) {
                            let like_score = generate::generate_like_score(&app, character, &post.text).await;
                            let did_like = pseudo_rand(&character.id, &format!("like{}", &post_id)) < like_score;
                            entry.add_like(character.id.clone(), like_score, did_like);
                            if did_like {
                                let _ = fs::like_post(&app, &post_id);
                            }
                        }

                        // Skip if comment already generated for this character
                        if entry.comments.iter().any(|c| c.character_id == character.id) {
                            continue;
                        }

                        // Generate comment
                        let chance: u8 = if is_extrovert(&character.persona) { 45 } else { 2 };
                        if pseudo_rand(&character.id, &post_id) < chance {
                            if let Ok(comment_text) = generate::generate_comment_text(
                                &app, character, &author_name, &post.text, &prior,
                            ).await {
                                entry.add_comment(character.id.clone(), comment_text.clone());

                                let seed = str_hash(&format!("reaction{}{}", character.id, &post_id));
                                let comment = PostComment {
                                    id: uuid_v4(),
                                    post_id: post_id.clone(),
                                    author_id: character.id.clone(),
                                    text: comment_text,
                                    created_at: generate::pick_comment_timestamp(&post.created_at, seed),
                                };
                                if let Err(e) = fs::add_comment(&app, &comment) {
                                    entry.mark_comment_failed(&character.id, e);
                                } else {
                                    entry.mark_comment_created(&character.id, comment.id);
                                }
                            }
                        }
                    }
                }

                entry.mark_completed();
                completed += 1;
            }
        }
    }

    // Save updated entries
    if !pending.is_empty() {
        pg::update_entries_batch(&app, &pending)?;
    }

    Ok(completed)
}
