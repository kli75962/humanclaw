use super::fs::{self, PostComment};
use super::generate;
use crate::social::character::{list_characters_fs, memory as character_memory};
use super::commands::{DmResult, uuid_v4, str_hash, pseudo_rand};

// ── Sociability helpers ────────────────────────────────────────────────────────

/// Probability (0–100) that a character follows through on a comment decision.
/// s=0 → 15%, s=50 → 55%, s=100 → 95%
pub(crate) fn comment_follow_through(sociability: u8) -> u8 {
    15u8.saturating_add((sociability as u32 * 80 / 100) as u8)
}

/// Probability (0–100) that a character replies to a thread they've already joined.
/// s=0 → 35%, s=50 → 62%, s=100 → 90%
pub(crate) fn thread_reply_pct(sociability: u8) -> u8 {
    35u8.saturating_add((sociability as u32 * 55 / 100) as u8)
}

/// Probability (0–100) that a character sends a DM instead of a comment.
/// Only non-zero above sociability 55; s=100 → 5%
pub(crate) fn dm_pct(sociability: u8) -> u8 {
    sociability.saturating_sub(55) / 9
}

/// Resolve a character's display name for comment context.
pub(crate) fn author_display_name<'a>(characters: &'a [crate::social::character::fs::CharacterMeta], author_id: &str) -> &'a str {
    if author_id == "user" { return "User"; }
    characters.iter().find(|c| c.id == author_id).map(|c| c.name.as_str()).unwrap_or("Unknown")
}

/// Build (author_name, text) pairs from saved comments for a post.
pub(crate) fn load_comment_context(app: &tauri::AppHandle, post_id: &str, characters: &[crate::social::character::fs::CharacterMeta]) -> Vec<(String, String)> {
    fs::list_comments(app, post_id)
        .into_iter()
        .map(|c| (author_display_name(characters, &c.author_id).to_string(), c.text))
        .collect()
}

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
        let (will_like, will_comment) = generate::generate_reaction_decision(&app, character, &post.text).await;
        if will_like {
            let _ = fs::like_post(&app, &post_id);
        }
        // Follow-through probability scales with sociability
        let sociability = crate::skills::get_sociability_for_persona(&app, &character.persona);
        let actually_comment = will_comment
            && pseudo_rand(&character.id, &format!("fthr{post_id}")) < comment_follow_through(sociability);
        if actually_comment {
            if let Ok(result) = generate::generate_comment_text_with_memory(
                &app, character, &author_name, &post.text, &prior,
            ).await {
                let seed = str_hash(&format!("reaction{}{}", character.id, post_id));
                let comment = PostComment {
                    id: uuid_v4(),
                    post_id: post_id.clone(),
                    author_id: character.id.clone(),
                    text: result.text.clone(),
                    created_at: generate::pick_comment_timestamp(&post.created_at, seed),
                };
                let _ = fs::add_comment(&app, &comment);
                let entry = character_memory::MemoryEntry {
                    id: uuid_v4(),
                    character_id: character.id.clone(),
                    entry_type: character_memory::MemoryEntryType::Comment,
                    brief: result.brief,
                    importance: result.importance,
                    created_at: character_memory::current_ts(),
                };
                let _ = character_memory::add_entry(&app, entry);
            }
        }
    }

    Ok(())
}

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
        let sociability = crate::skills::get_sociability_for_persona(&app, &character.persona);
        if pseudo_rand(&character.id, &format!("ucreply{post_id}")) < thread_reply_pct(sociability) {
            if let Ok(result) = generate::generate_comment_text_with_memory(
                &app, character, &author_name, &post.text, &prior,
            ).await {
                let seed = str_hash(&format!("ucreact{}{}", character.id, post_id));
                let comment = PostComment {
                    id: uuid_v4(),
                    post_id: post_id.clone(),
                    author_id: character.id.clone(),
                    text: result.text.clone(),
                    created_at: generate::pick_comment_timestamp(&post.created_at, seed),
                };
                let _ = fs::add_comment(&app, &comment);
                let entry = character_memory::MemoryEntry {
                    id: uuid_v4(),
                    character_id: character.id.clone(),
                    entry_type: character_memory::MemoryEntryType::Comment,
                    brief: result.brief,
                    importance: result.importance,
                    created_at: character_memory::current_ts(),
                };
                let _ = character_memory::add_entry(&app, entry);
            }
        }
    }

    Ok(())
}

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
        let (will_like, will_comment) = generate::generate_reaction_decision(&app, character, &post.text).await;
        if will_like {
            let _ = fs::like_post(&app, &post_id);
        }
        let sociability = crate::skills::get_sociability_for_persona(&app, &character.persona);
        let actually_comment = will_comment
            && pseudo_rand(&character.id, &format!("fthr{post_id}")) < comment_follow_through(sociability);
        if !actually_comment { continue; }

        let dmp = dm_pct(sociability);
        if dmp > 0 && pseudo_rand(&character.id, &format!("dm{post_id}")) < dmp {
            let trigger = format!(
                "The user posted: \"{}\". React naturally and start a conversation.",
                post.text
            );
            if let Ok(text) = generate::generate_dm_text(&app, character, &trigger).await {
                dms.push(DmResult { character_id: character.id.clone(), text });
            }
        } else {
            let comment_seed = str_hash(&format!("comment{}{}", character.id, post_id));
            if let Ok(result) = generate::generate_comment_text_with_memory(
                &app, character, "you", &post.text, &[],
            ).await {
                let comment = PostComment {
                    id: uuid_v4(),
                    post_id: post_id.clone(),
                    author_id: character.id.clone(),
                    text: result.text.clone(),
                    created_at: generate::pick_comment_timestamp(&post.created_at, comment_seed),
                };
                let _ = fs::add_comment(&app, &comment);
                let entry = character_memory::MemoryEntry {
                    id: uuid_v4(),
                    character_id: character.id.clone(),
                    entry_type: character_memory::MemoryEntryType::Comment,
                    brief: result.brief,
                    importance: result.importance,
                    created_at: character_memory::current_ts(),
                };
                let _ = character_memory::add_entry(&app, entry);
            }
        }
    }

    Ok(dms)
}

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
