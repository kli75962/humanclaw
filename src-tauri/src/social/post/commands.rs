pub use super::fs::{self, PostComment, PostMeta};
pub use super::generate;
pub use super::reactions::*;
pub use super::recovery as pg;
pub use super::schedule;
pub use crate::social::character::memory as character_memory;
use crate::social::config::load_config;

use crate::social::character::list_characters_fs;
use serde::Serialize;
use std::collections::HashSet;
use std::sync::Mutex;

lazy_static::lazy_static! {
    static ref IN_PROGRESS_DRAFTS: Mutex<HashSet<String>> = Default::default();
}

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
    // 1. Immediately enqueue as drafted status
    let entry = pg::enqueue_draft(&app, character_id.clone())?;
    let entry_id = entry.id.clone();
    println!(
        "[PhoneClaw/Queue] Added post draft for {} (Queue ID: {})",
        character_id, entry_id
    );

    // 2. Lock the entry before spawning so resume_post_gen_queue won't race us.
    {
        let mut lock = IN_PROGRESS_DRAFTS.lock().unwrap();
        lock.insert(entry_id.clone());
    }

    // 3. Spawn the entire generation + reaction pipeline in the background.
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        process_entry_in_background(app_clone, entry_id, Some(target_time)).await;
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
    fs::record_post_preference(
        &app,
        &post_id,
        &character_id,
        &post_text,
        post_image.as_deref(),
        &user_reason,
    )
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

// ── Core background pipeline ─────────────────────────────────────────────────

/// The single authoritative function that drives a post generation entry from
/// `PostGenerating` all the way through to `Completed`.
///
/// **Lock contract**: the caller MUST insert `entry_id` into `IN_PROGRESS_DRAFTS`
/// before spawning this function, and this function WILL remove it when it exits
/// (success or failure).
///
/// **Incremental persistence**: every mutation is immediately flushed to disk
/// via `pg::update_entry` so that concurrent calls to `resume_post_gen_queue`
/// always read the latest state and never duplicate work.
async fn process_entry_in_background(
    app: tauri::AppHandle,
    entry_id: String,
    target_time: Option<String>,
) {
    // Helper macro to release the lock and return early.
    macro_rules! release_and_return {
        () => {{
            let mut lock = IN_PROGRESS_DRAFTS.lock().unwrap();
            lock.remove(&entry_id);
            return;
        }};
    }

    // ── Step 1: Load the queue entry ────────────────────────────────────────
    let mut all_entries = pg::load_all(&app);
    let Some(pos) = all_entries.iter().position(|e| e.id == entry_id) else {
        println!("[PhoneClaw/Queue] Entry {entry_id} not found in queue, aborting.");
        release_and_return!();
    };

    // Reset failed entries back to their resumable states.
    {
        let entry = &mut all_entries[pos];
        if entry.state == pg::PostGenState::Failed {
            if entry.post_id.is_none() {
                entry.state = pg::PostGenState::PostGenerating;
            } else {
                entry.state = pg::PostGenState::ReactionsInProgress;
            }
            entry.error = None;
            let _ = pg::update_entry(&app, entry.clone());
        }
    }

    let character_id = all_entries[pos].character_id.clone();

    // ── Step 2: Generate post text (if not already done) ────────────────────
    if all_entries[pos].state == pg::PostGenState::PostGenerating
        && all_entries[pos].generated_text.is_empty()
    {
        println!("[PhoneClaw/Ollama] Generating post text for {}…", character_id);

        let characters = list_characters_fs(&app);
        let Some(character) = characters.into_iter().find(|c| c.id == character_id) else {
            let entry = &mut all_entries[pos];
            entry.mark_failed("Character deleted".to_string());
            let _ = pg::update_entry(&app, entry.clone());
            release_and_return!();
        };

        match generate::generate_post_text_with_memory(
            &app,
            &character,
            None,
            target_time.as_deref(),
        )
        .await
        {
            Ok(post_result) => {
                println!("[PhoneClaw/Ollama] Post text ready for {}.", character_id);
                let entry = &mut all_entries[pos];
                entry.set_generated_text(post_result.text.clone(), post_result.timestamp.clone());
                let _ = pg::update_entry(&app, entry.clone()); // Persist immediately

                let mem_entry = character_memory::MemoryEntry {
                    id: uuid_v4(),
                    character_id: character_id.clone(),
                    entry_type: character_memory::MemoryEntryType::Post,
                    brief: post_result.brief,
                    importance: post_result.importance,
                    created_at: character_memory::current_ts(),
                };
                let _ = character_memory::add_entry(&app, mem_entry);
            }
            Err(err) => {
                println!("[PhoneClaw/Ollama] Failed generating post for {}: {}", character_id, err);
                let entry = &mut all_entries[pos];
                entry.mark_failed(err);
                let _ = pg::update_entry(&app, entry.clone());
                release_and_return!();
            }
        }
    }

    // ── Step 3: Save the post to disk ────────────────────────────────────────
    if all_entries[pos].state == pg::PostGenState::PostGenerating
        && all_entries[pos].post_id.is_none()
    {
        let entry = &all_entries[pos];
        let post = PostMeta {
            id: uuid_v4(),
            character_id: character_id.clone(),
            text: entry.generated_text.clone(),
            image: None,
            created_at: entry.generated_timestamp.clone(),
            like_count: 0,
        };
        if let Err(e) = fs::save_post(&app, &post) {
            let entry = &mut all_entries[pos];
            entry.mark_failed(format!("Failed to save post: {e}"));
            let _ = pg::update_entry(&app, entry.clone());
            release_and_return!();
        }
        let entry = &mut all_entries[pos];
        entry.mark_post_created(post.id.clone());
        let _ = pg::update_entry(&app, entry.clone()); // Persist post_id + PostCreated state
    }

    // ── Step 4: Transition to reactions phase ────────────────────────────────
    if all_entries[pos].state == pg::PostGenState::PostCreated {
        let entry = &mut all_entries[pos];
        entry.start_reactions();
        let _ = pg::update_entry(&app, entry.clone());
    }

    // ── Step 5: Generate per-character reactions ─────────────────────────────
    if all_entries[pos].state != pg::PostGenState::ReactionsInProgress {
        // Nothing more to do (already completed or unexpectedly failed).
        release_and_return!();
    }

    let post_id = match all_entries[pos].post_id.clone() {
        Some(id) => id,
        None => {
            let entry = &mut all_entries[pos];
            entry.mark_failed("ReactionsInProgress but post_id is None".to_string());
            let _ = pg::update_entry(&app, entry.clone());
            release_and_return!();
        }
    };

    let posts = fs::list_posts(&app);
    if let Some(post) = posts.iter().find(|p| p.id == post_id).cloned() {
        let all_characters = list_characters_fs(&app);
        let author_name = all_characters
            .iter()
            .find(|c| c.id == post.character_id)
            .map(|c| c.name.as_str())
            .unwrap_or("them")
            .to_string();
        let prior = load_comment_context(&app, &post_id, &all_characters);
        let cfg = load_config(&app);

        for character in all_characters.iter().filter(|c| c.id != post.character_id) {
            // ── Like decision ──────────────────────────────────────────────
            let entry = &all_entries[pos];
            let (did_like, will_comment_r) =
                if let Some(like_entry) = entry.likes.iter().find(|l| l.character_id == character.id) {
                    (
                        like_entry.did_like,
                        generate::generate_reaction_decision(&app, character, &post.text)
                            .await
                            .1,
                    )
                } else {
                    let (wl, wc) =
                        generate::generate_reaction_decision(&app, character, &post.text).await;
                    let entry = &mut all_entries[pos];
                    entry.add_like(character.id.clone(), if wl { 80 } else { 20 }, wl);
                    let _ = pg::update_entry(&app, entry.clone()); // Persist like immediately
                    if wl {
                        let _ = fs::like_post(&app, &post_id);
                    }
                    (wl, wc)
                };

            // Skip if this character has already generated a comment (idempotency).
            if all_entries[pos].comments.iter().any(|c| c.character_id == character.id) {
                continue;
            }

            // ── Comment decision / generation ──────────────────────────────
            let sociability = crate::skills::get_sociability_for_persona(&app, &character.persona);
            let actually_comment = will_comment_r
                && pseudo_rand(&character.id, &format!("fthr{}", &post_id))
                    < comment_follow_through(
                        sociability,
                        cfg.comment_follow_through_base_pct,
                        cfg.comment_follow_through_scale_pct,
                    );

            if did_like && actually_comment {
                if let Ok(comment_result) = generate::generate_comment_text_with_memory(
                    &app,
                    character,
                    &author_name,
                    &post.text,
                    &prior,
                )
                .await
                {
                    let seed = str_hash(&format!("reaction{}{}", character.id, &post_id));
                    let comment = PostComment {
                        id: uuid_v4(),
                        post_id: post_id.clone(),
                        author_id: character.id.clone(),
                        text: comment_result.text.clone(),
                        created_at: generate::pick_comment_timestamp(&post.created_at, seed),
                    };

                    let entry = &mut all_entries[pos];
                    entry.add_comment(character.id.clone(), comment_result.text.clone());

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
                    // Persist immediately after each character — no data loss on early exit.
                    let _ = pg::update_entry(&app, entry.clone());
                }
            }
        }
    }

    // ── Step 6: Mark completed ────────────────────────────────────────────────
    {
        let entry = &mut all_entries[pos];
        entry.mark_completed();
        let _ = pg::update_entry(&app, entry.clone());
    }
    println!("[PhoneClaw/Queue] Entry {} completed successfully.", entry_id);

    // Release the lock on the happy path.
    let mut lock = IN_PROGRESS_DRAFTS.lock().unwrap();
    lock.remove(&entry_id);
}

// ── Queue patrol ─────────────────────────────────────────────────────────────

/// Resume any pending post generation queue entries (called on app startup and
/// on each 5-minute frontend poll).
///
/// This function is intentionally **non-blocking** — it dispatches each pending
/// entry into a background task and returns immediately.  The actual generation
/// work happens inside `process_entry_in_background`, which holds the
/// `IN_PROGRESS_DRAFTS` lock for its entire lifetime, preventing duplicate runs.
#[tauri::command]
pub async fn resume_post_gen_queue(app: tauri::AppHandle) -> Result<u32, String> {
    let pending = pg::load_pending(&app);
    let mut dispatched: u32 = 0;

    for entry in pending {
        // Skip if this entry is already being processed by a background task.
        {
            let lock = IN_PROGRESS_DRAFTS.lock().unwrap();
            if lock.contains(&entry.id) {
                continue;
            }
        }

        // Lock the entry NOW, before spawning, to prevent any other concurrent
        // call from also picking it up.
        {
            let mut lock = IN_PROGRESS_DRAFTS.lock().unwrap();
            lock.insert(entry.id.clone());
        }

        let app_clone = app.clone();
        let entry_id = entry.id.clone();
        println!(
            "[PhoneClaw/Queue] Dispatching background task for entry {}",
            entry_id
        );
        tauri::async_runtime::spawn(async move {
            // target_time is None for resumes — the generated text already
            // has its timestamp (or we'll fall back to the default 2-hr-ago offset).
            process_entry_in_background(app_clone, entry_id, None).await;
        });

        dispatched += 1;
    }

    if dispatched > 0 {
        println!(
            "[PhoneClaw/Queue] Patrol: dispatched {} background task(s).",
            dispatched
        );
    }

    Ok(dispatched)
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
                let sociability =
                    crate::skills::get_sociability_for_persona(&app, &character.persona);
                let cfg = load_config(&app);
                let max_posts: u8 = match sociability {
                    71..=100 => cfg.max_posts_high_sociability,
                    41..=70 => cfg.max_posts_medium_sociability,
                    _ => cfg.max_posts_low_sociability,
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
