




pub use super::fs::{self, PostComment, PostMeta};
pub use super::generate;
pub use crate::social::character::memory as character_memory;
pub use super::schedule;
pub use super::recovery as pg;
pub use super::reactions::*;

use serde::Serialize;
use crate::social::character::list_characters_fs;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DmResult {
    pub character_id: String,
    pub text: String,
}

// ── Basic CRUD ───────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn generate_character_post(
    app: tauri::AppHandle,
    character_id: String,
    target_time: String,
) -> Result<String, String> {
    let characters = list_characters_fs(&app);
    let character = characters
        .iter()
        .find(|c| c.id == character_id)
        .ok_or_else(|| format!("Character {character_id} not found"))?;

    let post_result = generate::generate_post_text_with_memory(&app, character, None, Some(&target_time)).await?;

    let entry = pg::enqueue(
        &app,
        character_id,
        post_result.text.clone(),
        target_time,
    )?;

    // Add memory entry for the initial post text
    let mem_entry = character_memory::MemoryEntry {
        id: uuid_v4(),
        character_id: character.id.clone(),
        entry_type: character_memory::MemoryEntryType::Post,
        brief: post_result.brief,
        importance: post_result.importance,
        created_at: character_memory::current_ts(),
    };
    let _ = character_memory::add_entry(&app, mem_entry);

    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = resume_post_gen_queue(app_clone).await;
    });

    Ok(entry.id)
}
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

// ── Sociability helpers ────────────────────────────────────────────────────────










// ── Helpers ──────────────────────────────────────────────────────────────────

pub(crate) fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:x}-{:x}", t, t.wrapping_mul(6364136223846793005))
}

/// FNV-1a hash of a string → u64, used as a deterministic seed.
pub(crate) fn str_hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

pub(crate) fn pseudo_rand(seed1: &str, seed2: &str) -> u8 {
    (str_hash(&format!("{seed1}{seed2}")) % 100) as u8
}

// ── Queue Integration ────────────────────────────────────────────────────────


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
                        // Generate reaction decision (if not already tracked)
                        let (did_like, will_comment_r) = if let Some(like_entry) = entry.likes.iter().find(|l| l.character_id == character.id) {
                            // Already decided like; re-ask for comment decision since it wasn't stored
                            (like_entry.did_like, generate::generate_reaction_decision(&app, character, &post.text).await.1)
                        } else {
                            let (wl, wc) = generate::generate_reaction_decision(&app, character, &post.text).await;
                            entry.add_like(character.id.clone(), if wl { 80 } else { 20 }, wl);
                            if wl { let _ = fs::like_post(&app, &post_id); }
                            (wl, wc)
                        };

                        // Skip if comment already generated for this character
                        if entry.comments.iter().any(|c| c.character_id == character.id) {
                            continue;
                        }

                        let sociability = crate::skills::get_sociability_for_persona(&app, &character.persona);
                        let actually_comment = will_comment_r
                            && pseudo_rand(&character.id, &format!("fthr{}", &post_id)) < comment_follow_through(sociability);
                        if did_like && actually_comment {
                            if let Ok(comment_result) = generate::generate_comment_text_with_memory(
                                &app, character, &author_name, &post.text, &prior,
                            ).await {
                                entry.add_comment(character.id.clone(), comment_result.text.clone());
                                let seed = str_hash(&format!("reaction{}{}", character.id, &post_id));
                                let comment = PostComment {
                                    id: uuid_v4(),
                                    post_id: post_id.clone(),
                                    author_id: character.id.clone(),
                                    text: comment_result.text.clone(),
                                    created_at: generate::pick_comment_timestamp(&post.created_at, seed),
                                };
                                if let Err(e) = fs::add_comment(&app, &comment) {
                                    entry.mark_comment_failed(&character.id, e);
                                } else {
                                    entry.mark_comment_created(&character.id, comment.id.clone());
                                    let mem_entry = character_memory::MemoryEntry {
                                        id: uuid_v4(),
                                        character_id: character.id.clone(),
                                        entry_type: character_memory::MemoryEntryType::Comment,
                                        brief: comment_result.brief,
                                        importance: comment_result.importance,
                                        created_at: character_memory::current_ts(),
                                    };
                                    let _ = character_memory::add_entry(&app, mem_entry);
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

// ── Daily schedule ────────────────────────────────────────────────────────────

/// A post slot that is due for generation.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuePost {
    pub character_id: String,
    /// RFC 3339 target datetime — passed as `target_time` to `generate_character_post`.
    pub target_time: String,
    /// HH:MM string used to mark the slot as generated.
    pub time_str: String,
}

/// For each character, ensure today's schedule exists (asking LLM if needed),
/// then return all slots that are due and not yet generated.
#[tauri::command]
pub async fn get_due_posts(app: tauri::AppHandle) -> Vec<DuePost> {
    let characters = list_characters_fs(&app);
    let today = schedule::today_str();
    let mut due = Vec::new();

    for character in &characters {
        let sched = match schedule::load(&app, &character.id) {
            Some(s) if s.date == today => s,
            _ => {
                let sociability = crate::skills::get_sociability_for_persona(&app, &character.persona);
                let max_posts: u8 = match sociability {
                    71..=100 => 3,
                    41..=70  => 2,
                    _        => 1,
                };
                let mut times = generate::decide_posting_times(&app, character, max_posts).await;
                if times.is_empty() {
                    times = schedule::fallback_times(sociability);
                }
                let new_sched = schedule::DaySchedule {
                    character_id: character.id.clone(),
                    date: today.clone(),
                    times,
                    generated: vec![],
                };
                let _ = schedule::save(&app, &new_sched);
                new_sched
            }
        };

        for time_str in schedule::due_times(&sched) {
            if let Some(target_time) = schedule::hhmm_to_rfc3339_today(&time_str) {
                due.push(DuePost {
                    character_id: character.id.clone(),
                    target_time,
                    time_str,
                });
            }
        }
    }

    due
}

/// Mark a scheduled time slot as generated so it won't be re-generated on next open.
#[tauri::command]
pub fn mark_post_generated(
    app: tauri::AppHandle,
    character_id: String,
    time_str: String,
) -> Result<(), String> {
    schedule::mark_generated(&app, &character_id, &time_str)
}
