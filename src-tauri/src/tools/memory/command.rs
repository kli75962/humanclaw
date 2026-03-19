use std::io::Write;
use std::path::PathBuf;

use serde_json::Value;
use tauri::AppHandle;

use crate::bridge::health::check_peer;
use crate::memory::{memory_dir, normalize_memory_path, ALLOWED_FILES};
use crate::session::{store as session_store, types::PairedDevice};
use crate::tools::dispatch::ToolExecutionContext;

pub fn execute_memory_tool(app: &AppHandle, tool_args: &Value, context: &ToolExecutionContext) -> String {
    let cmd = tool_args.get("command").and_then(Value::as_str).unwrap_or("");
    let path = tool_args.get("path").and_then(Value::as_str);
    let content = tool_args.get("content").and_then(Value::as_str);
    let mode = tool_args.get("mode").and_then(Value::as_str);
    let query = tool_args.get("query").and_then(Value::as_str);
    let is_sync_request = tool_args
        .get("__memory_sync")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    if cmd == "create" || cmd == "update" {
        // Fire-and-forget writes keep tool rounds responsive.
        let app_clone = app.clone();
        let cmd_s = cmd.to_string();
        let path_s = path.map(String::from);
        let content_s = content.map(String::from);
        let mode_s = mode.map(String::from);
        let context_s = context.clone();
        tokio::spawn(async move {
            apply_memory_write_with_sync(
                &app_clone,
                &context_s,
                &cmd_s,
                path_s.as_deref(),
                content_s.as_deref(),
                mode_s.as_deref(),
                is_sync_request,
            )
            .await;
        });
        "ok: memory saved".to_string()
    } else {
        run_memory_command(app, cmd, path, content, mode, query)
    }
}

async fn apply_memory_write_with_sync(
    app: &AppHandle,
    context: &ToolExecutionContext,
    cmd: &str,
    path: Option<&str>,
    content: Option<&str>,
    mode: Option<&str>,
    is_sync_request: bool,
) {
    // Sync-originated writes should apply locally only, preventing sync loops.
    if is_sync_request {
        let _ = execute_memory_write(memory_dir(app), cmd, path, content, mode);
        return;
    }

    let cfg = session_store::bootstrap(app);
    let local_dir = memory_dir(app);
    if cfg.paired_devices.is_empty() {
        let _ = execute_memory_write(local_dir, cmd, path, content, mode);
        return;
    }

    let local_id = cfg.device.device_id.clone();
    let source_id = context
        .source_device_id
        .clone()
        .unwrap_or_else(|| local_id.clone());

    if source_id == local_id {
        let _ = execute_memory_write(local_dir.clone(), cmd, path, content, mode);
    } else if let Some(source_peer) = cfg
        .paired_devices
        .iter()
        .find(|p| p.device_id == source_id)
        .cloned()
    {
        let synced = sync_write_to_peer(
            &cfg.hash_key,
            &source_peer,
            context,
            cmd,
            path,
            content,
            mode,
        )
        .await;

        if synced {
            // Keep local copy in sync after source write succeeds.
            let _ = execute_memory_write(local_dir.clone(), cmd, path, content, mode);
        } else {
            // Source currently offline: persist locally, then retry source sync later.
            let _ = execute_memory_write(local_dir.clone(), cmd, path, content, mode);
            spawn_retry_sync(
                cfg.hash_key.clone(),
                source_peer,
                context.clone(),
                cmd.to_string(),
                path.map(ToString::to_string),
                content.map(ToString::to_string),
                mode.map(ToString::to_string),
            );
        }
    } else {
        // Unknown source device id: fall back to local write.
        let _ = execute_memory_write(local_dir.clone(), cmd, path, content, mode);
    }

    // Sync to all other peers; retry when offline.
    for peer in cfg.paired_devices {
        if peer.device_id == source_id {
            continue;
        }

        let synced = sync_write_to_peer(
            &cfg.hash_key,
            &peer,
            context,
            cmd,
            path,
            content,
            mode,
        )
        .await;

        if !synced {
            spawn_retry_sync(
                cfg.hash_key.clone(),
                peer,
                context.clone(),
                cmd.to_string(),
                path.map(ToString::to_string),
                content.map(ToString::to_string),
                mode.map(ToString::to_string),
            );
        }
    }
}

async fn sync_write_to_peer(
    hash_key: &str,
    peer: &PairedDevice,
    context: &ToolExecutionContext,
    cmd: &str,
    path: Option<&str>,
    content: Option<&str>,
    mode: Option<&str>,
) -> bool {
    if !check_peer(&peer.address, hash_key).await {
        return false;
    }

    let url = format!("http://{}/tool", peer.address);
    let body = serde_json::json!({
        "key": hash_key,
        "tool_name": "memory",
        "tool_args": {
            "command": cmd,
            "path": path,
            "content": content,
            "mode": mode,
            "__memory_sync": true
        },
        "source_device_id": context.source_device_id,
        "source_device_type": context.source_device_type,
    });

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    match client.post(url).json(&body).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

fn spawn_retry_sync(
    hash_key: String,
    peer: PairedDevice,
    context: ToolExecutionContext,
    cmd: String,
    path: Option<String>,
    content: Option<String>,
    mode: Option<String>,
) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            let synced = sync_write_to_peer(
                &hash_key,
                &peer,
                &context,
                &cmd,
                path.as_deref(),
                content.as_deref(),
                mode.as_deref(),
            )
            .await;
            if synced {
                break;
            }
        }
    });
}

/// Write a memory file given a pre-resolved `dir` path.
fn execute_memory_write(
    dir: PathBuf,
    _cmd: &str,
    path: Option<&str>,
    content: Option<&str>,
    mode: Option<&str>,
) -> Result<(), String> {
    let p = path.ok_or_else(|| "'path' required".to_string())?;
    let body = content.ok_or_else(|| "'content' required".to_string())?;
    let name = normalize_memory_path(p).ok_or_else(|| format!("unknown memory file '{p}'"))?;
    let file_path = dir.join(name);

    let result = if mode == Some("append") {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .and_then(|mut f| f.write_all(body.as_bytes()))
    } else {
        std::fs::write(&file_path, body).map(|_| ())
    };

    result.map_err(|e| e.to_string())?;
    Ok(())
}

fn run_memory_command(
    app: &AppHandle,
    command: &str,
    path: Option<&str>,
    content: Option<&str>,
    mode: Option<&str>,
    query: Option<&str>,
) -> String {
    let dir = memory_dir(app);

    match command {
        "view" => {
            let Some(p) = path else {
                return "error: 'path' is required for view".to_string();
            };
            let Some(name) = normalize_memory_path(p) else {
                return format!("error: unknown memory file '{p}'");
            };
            std::fs::read_to_string(dir.join(name)).unwrap_or_else(|e| format!("error reading {name}: {e}"))
        }

        "create" | "update" => match execute_memory_write(dir, command, path, content, mode) {
            Ok(()) => "ok: memory saved".to_string(),
            Err(e) => format!("error: {e}"),
        },

        "search" => {
            let Some(q) = query else {
                return "error: 'query' is required for search".to_string();
            };
            let q_lower = q.to_lowercase();
            let terms: Vec<&str> = q_lower.split_whitespace().collect();

            let files: Vec<PathBuf> = if let Some(p) = path {
                match normalize_memory_path(p) {
                    Some(name) => vec![dir.join(name)],
                    None => return format!("error: unknown memory file '{p}'"),
                }
            } else {
                ALLOWED_FILES.iter().map(|f| dir.join(f)).collect()
            };

            let mut matches = Vec::new();
            for file_path in &files {
                let Ok(text) = std::fs::read_to_string(file_path) else {
                    continue;
                };
                let fname = file_path.file_name().unwrap_or_default().to_string_lossy();
                for (i, line) in text.lines().enumerate() {
                    let lower = line.to_lowercase();
                    if terms.iter().any(|t| lower.contains(*t)) {
                        matches.push(format!("{fname}:{}:{line}", i + 1));
                    }
                }
            }

            if matches.is_empty() {
                "no matches found".to_string()
            } else {
                matches.join("\n")
            }
        }

        other => format!("error: unknown memory command '{other}'"),
    }
}
