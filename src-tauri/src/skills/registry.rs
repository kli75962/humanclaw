use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

// ── Shared Data Structures ──

#[derive(Clone, Debug)]
pub struct Skill {
    pub name: String,
    pub content: String,
    pub config: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Tool {
    pub content: String,
}

// Memory caches
static SKILLS: std::sync::RwLock<Option<Vec<Skill>>> = std::sync::RwLock::new(None);
static TOOLS: std::sync::RwLock<Option<Vec<Tool>>> = std::sync::RwLock::new(None);

// ── Dynamic Resolution ──

/// Returns the root path to `src` (containing `skills` and `tools/regtools`).
/// - in Dev Mode it tries to find the local absolute path `src` inside `src-tauri`.
/// - in Prod Mode (packaged) it uses `app.path().resource_dir() / "src"`.
fn get_src_root(app: &AppHandle) -> PathBuf {
    #[cfg(debug_assertions)]
    {
        // When running in dev, we can locate relative to `CARGO_MANIFEST_DIR` if set,
        // or just expect the current working directory is `src-tauri`.
        if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            let p = PathBuf::from(manifest_dir).join("src");
            if p.exists() {
                return p;
            }
        }
        // Fallback or if not running via cargo but still debug
        let p = std::env::current_dir().unwrap_or_default().join("src");
        if p.exists() {
            return p;
        }
    }

    // In production or if dev path resolution failed, we use Tauri's embedded resources
    let res = app.path().resource_dir().unwrap_or_default();
    res.join("src")
}

// ── Scanners ──

fn load_skills_from_disk(base: &Path) -> Vec<Skill> {
    let mut entries = Vec::new();
    let skills_dir = base.join("skills");

    if let Ok(read_dir) = fs::read_dir(skills_dir) {
        let mut dirs: Vec<PathBuf> = read_dir
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        dirs.sort(); // Deterministic order

        for dir in dirs {
            let skill_md = dir.join("SKILL.md");
            if skill_md.exists() {
                if let Ok(content) = fs::read_to_string(&skill_md) {
                    let name = dir
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned();
                    let config_path = dir.join("persona_config.json");
                    let config = if config_path.exists() {
                        fs::read_to_string(config_path).ok()
                    } else {
                        None
                    };

                    entries.push(Skill {
                        name,
                        content,
                        config,
                    });
                }
            }
        }
    }

    entries
}

fn load_tools_from_disk(base: &Path) -> Vec<Tool> {
    let mut entries = Vec::new();
    let tools_dir = base.join("tools").join("regtools");

    if let Ok(read_dir) = fs::read_dir(tools_dir) {
        let mut files: Vec<PathBuf> = read_dir
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
            .collect();
        files.sort();

        for file in files {
            if let Ok(content) = fs::read_to_string(&file) {
                entries.push(Tool { content });
            }
        }
    }

    entries
}

// ── Public Accessors ──

/// Retrieve all skills. Caches the result on the first call.
/// Set `force_reload = true` to re-scan the filesystem.
pub fn get_skills(app: &AppHandle, force_reload: bool) -> Vec<Skill> {
    {
        let lock = SKILLS.read().unwrap();
        if let Some(ref s) = *lock {
            if !force_reload {
                return s.clone();
            }
        }
    }

    let root = get_src_root(app);
    let loaded = load_skills_from_disk(&root);

    let mut lock = SKILLS.write().unwrap();
    *lock = Some(loaded.clone());
    loaded
}

/// Retrieve all tools. Caches the result on the first call.
/// Set `force_reload = true` to re-scan the filesystem.
pub fn get_tools(app: &AppHandle, force_reload: bool) -> Vec<Tool> {
    {
        let lock = TOOLS.read().unwrap();
        if let Some(ref t) = *lock {
            if !force_reload {
                return t.clone();
            }
        }
    }

    let root = get_src_root(app);
    let loaded = load_tools_from_disk(&root);

    let mut lock = TOOLS.write().unwrap();
    *lock = Some(loaded.clone());
    loaded
}

/// Fetch parsed tool JSON schemas. Clears OnceLock wrapper cache on reload.
pub fn load_tool_schemas(app: &AppHandle) -> Vec<Value> {
    let tools = get_tools(app, false);
    tools
        .into_iter()
        .filter_map(|t| serde_json::from_str(&t.content).ok())
        .collect()
}
