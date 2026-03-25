use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::memory::memory_dir;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PostComment {
    pub id: String,
    pub post_id: String,
    /// Character id or "user".
    pub author_id: String,
    pub text: String,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PostMeta {
    pub id: String,
    pub character_id: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,  // base64 data URL
    pub created_at: String,
    pub like_count: u32,
}

fn posts_dir(app: &AppHandle) -> PathBuf {
    memory_dir(app).join("posts")
}

fn posts_index_path(app: &AppHandle) -> PathBuf {
    posts_dir(app).join("_index.json")
}

fn is_safe_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 80
        && id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

/// List all posts, newest first.
pub fn list_posts(app: &AppHandle) -> Vec<PostMeta> {
    let text = std::fs::read_to_string(posts_index_path(app)).unwrap_or_default();
    serde_json::from_str(&text).unwrap_or_default()
}

/// Create or update a post.
pub fn save_post(app: &AppHandle, post: &PostMeta) -> Result<(), String> {
    if !is_safe_id(&post.id) {
        return Err("Invalid post id".to_string());
    }
    let dir = posts_dir(app);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let mut posts = list_posts(app);
    if let Some(existing) = posts.iter_mut().find(|p| p.id == post.id) {
        *existing = post.clone();
    } else {
        posts.insert(0, post.clone());
    }

    let json = serde_json::to_string(&posts).map_err(|e| e.to_string())?;
    std::fs::write(posts_index_path(app), json).map_err(|e| e.to_string())
}

/// Delete a post by id.
pub fn delete_post(app: &AppHandle, id: &str) -> Result<(), String> {
    if !is_safe_id(id) {
        return Err("Invalid post id".to_string());
    }
    let mut posts = list_posts(app);
    posts.retain(|p| p.id != id);
    let dir = posts_dir(app);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string(&posts).map_err(|e| e.to_string())?;
    std::fs::write(posts_index_path(app), json).map_err(|e| e.to_string())
}

// ── Comments ────────────────────────────────────────────────────────────────

fn comments_index_path(app: &AppHandle) -> PathBuf {
    posts_dir(app).join("_comments.json")
}

fn all_comments(app: &AppHandle) -> Vec<PostComment> {
    let text = std::fs::read_to_string(comments_index_path(app)).unwrap_or_default();
    serde_json::from_str(&text).unwrap_or_default()
}

fn write_comments(app: &AppHandle, comments: &[PostComment]) -> Result<(), String> {
    let dir = posts_dir(app);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string(comments).map_err(|e| e.to_string())?;
    std::fs::write(comments_index_path(app), json).map_err(|e| e.to_string())
}

/// List all comments for a given post, oldest first.
pub fn list_comments(app: &AppHandle, post_id: &str) -> Vec<PostComment> {
    all_comments(app).into_iter().filter(|c| c.post_id == post_id).collect()
}

/// Append a new comment.
pub fn add_comment(app: &AppHandle, comment: &PostComment) -> Result<(), String> {
    if !is_safe_id(&comment.id) || !is_safe_id(&comment.post_id) {
        return Err("Invalid id".to_string());
    }
    let mut comments = all_comments(app);
    comments.push(comment.clone());
    write_comments(app, &comments)
}

// ── Likes ────────────────────────────────────────────────────────────────────

/// Increment the like count of a post. Returns the new count.
pub fn like_post(app: &AppHandle, id: &str) -> Result<u32, String> {
    update_like_count(app, id, true)
}

/// Decrement the like count of a post (min 0). Returns the new count.
pub fn unlike_post(app: &AppHandle, id: &str) -> Result<u32, String> {
    update_like_count(app, id, false)
}

fn update_like_count(app: &AppHandle, id: &str, increment: bool) -> Result<u32, String> {
    if !is_safe_id(id) {
        return Err("Invalid post id".to_string());
    }
    let mut posts = list_posts(app);
    let post = posts.iter_mut().find(|p| p.id == id)
        .ok_or_else(|| "Post not found".to_string())?;
    if increment {
        post.like_count += 1;
    } else {
        post.like_count = post.like_count.saturating_sub(1);
    }
    let new_count = post.like_count;
    let dir = posts_dir(app);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string(&posts).map_err(|e| e.to_string())?;
    std::fs::write(posts_index_path(app), json).map_err(|e| e.to_string())?;
    Ok(new_count)
}
