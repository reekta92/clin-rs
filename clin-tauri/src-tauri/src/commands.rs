//! commands.rs — All Tauri IPC commands for the clin desktop companion.
//!
//! Each `#[tauri::command]` function mirrors one endpoint from the clin
//! web-bridge HTTP API so that the JavaScript frontend (`tauri_adapter.js`)
//! can call `invoke(...)` and get back the same JSON shapes it would receive
//! from the HTTP server.

use crate::AppState;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use comrak::{markdown_to_html, Options};
use gray_matter::engine::YAML;
use gray_matter::Matter;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use walkdir::WalkDir;

// ---------------------------------------------------------------------------
// Helper: resolve a vault-relative path → absolute PathBuf
// ---------------------------------------------------------------------------
fn vault_path(state: &tauri::State<AppState>) -> PathBuf {
    state.vault_path.lock().unwrap().clone()
}

/// Resolve `rel` against the vault root. If `rel` is already absolute return
/// it unchanged.
fn resolve(vault: &Path, rel: &str) -> PathBuf {
    let p = PathBuf::from(rel);
    if p.is_absolute() {
        p
    } else {
        vault.join(rel)
    }
}

// ---------------------------------------------------------------------------
// Helper: config file path  (~/.config/clin/config.json)
// ---------------------------------------------------------------------------
fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("clin")
        .join("config.json")
}

fn read_config() -> Value {
    let p = config_path();
    if p.exists() {
        fs::read_to_string(&p)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(json!({}))
    } else {
        json!({})
    }
}

pub fn saved_vault_path() -> Option<PathBuf> {
    let cfg = read_config();
    cfg["vault"].as_str().map(PathBuf::from).filter(|p| p.is_dir())
}

fn write_config(val: &Value) -> Result<(), String> {
    let p = config_path();
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&p, serde_json::to_string_pretty(val).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Helper: wikilink extraction
// Finds [[Target]] and [[Target|Alias]] patterns in markdown text.
// ---------------------------------------------------------------------------
fn extract_wikilinks(text: &str) -> Vec<String> {
    // Lazily compiled regex — cheap after the first call per process.
    let re = Regex::new(r"\[\[([^\[\]|#]+)(?:[|#][^\[\]]*)?\]\]").unwrap();
    re.captures_iter(text)
        .map(|cap| cap[1].trim().to_string())
        .collect()
}

/// Replace every occurrence of `[[old_name]]` (and `[[old_name|...]]`) with
/// `[[new_name]]` (preserving any alias / heading anchor).
fn rewrite_wikilinks(text: &str, old_stem: &str, new_stem: &str) -> String {
    let re = Regex::new(&format!(
        r"\[\[{}((?:[|#][^\[\]]*)?)\]\]",
        regex::escape(old_stem)
    ))
    .unwrap();
    re.replace_all(text, format!("[[{}$1]]", new_stem))
        .into_owned()
}

// ---------------------------------------------------------------------------
// Helper: frontmatter parse
// ---------------------------------------------------------------------------
fn parse_frontmatter(source: &str) -> Value {
    let matter = Matter::<YAML>::new();
    match matter.parse(source).data {
        Some(d) => d.deserialize().unwrap_or(json!({})),
        None => json!({}),
    }
}

#[allow(dead_code)]
fn strip_frontmatter(source: &str) -> &str {
    if source.starts_with("---") {
        // Find the closing ---
        if let Some(end) = source[3..].find("\n---") {
            let after = end + 3 + 4; // skip \n---
            if after < source.len() {
                return source[after..].trim_start_matches('\n');
            }
        }
    }
    source
}

// ---------------------------------------------------------------------------
// Helper: collect all .md files under a directory
// ---------------------------------------------------------------------------
fn collect_md_files(vault: &Path) -> Vec<PathBuf> {
    WalkDir::new(vault)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path().extension().and_then(|x| x.to_str()) == Some("md")
        })
        .map(|e| e.into_path())
        .collect()
}

// ---------------------------------------------------------------------------
// 1. get_graph
// ---------------------------------------------------------------------------
#[derive(Serialize)]
struct GraphNode {
    id: String,
    label: String,
    path: String,
    title: String,
    tags: Vec<String>,
    folder: String,
    created: Option<String>,
    modified: Option<String>,
}

#[derive(Serialize)]
struct GraphEdge {
    source: String,
    target: String,
}

#[derive(Serialize)]
pub struct GraphResult {
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
    vault_name: String,
}

#[tauri::command]
pub async fn get_graph(state: tauri::State<'_, AppState>) -> Result<GraphResult, String> {
    let vault = vault_path(&state);
    eprintln!("[clin] get_graph vault={}", vault.display());
    let files = collect_md_files(&vault);
    eprintln!("[clin] get_graph found {} files", files.len());

    // Build a stem → relative-path index so we can resolve wikilinks.
    let mut stem_index: HashMap<String, String> = HashMap::new();
    for f in &files {
        let rel = f
            .strip_prefix(&vault)
            .unwrap_or(f)
            .to_string_lossy()
            .to_string();
        let stem = f
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        stem_index.insert(stem.clone(), rel.clone());
        stem_index.entry(rel.clone()).or_insert(rel);
    }

    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();
    let mut seen_edges: HashSet<(String, String)> = HashSet::new();

    for f in &files {
        let rel = f
            .strip_prefix(&vault)
            .unwrap_or(f)
            .to_string_lossy()
            .to_string();
        let label = f
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let folder = f
            .parent()
            .and_then(|p| p.strip_prefix(&vault).ok())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        let content = fs::read_to_string(f).unwrap_or_default();
        let fm = parse_frontmatter(&content);
        let title = fm["title"].as_str().unwrap_or(&label).to_string();
        let tags: Vec<String> = fm["tags"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        let modified = f
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .map(|t| {
                let dt: chrono::DateTime<chrono::Utc> = t.into();
                dt.to_rfc3339()
            });
        let created = fm["date"].as_str().or_else(|| fm["created"].as_str()).map(|s| s.to_string());

        nodes.push(GraphNode {
            id: rel.clone(),
            label: label.clone(),
            path: rel.clone(),
            title,
            tags,
            folder,
            created,
            modified,
        });

        let links = extract_wikilinks(&content);
        for link in links {
            if let Some(target_rel) = stem_index.get(&link) {
                let key = (rel.clone(), target_rel.clone());
                if !seen_edges.contains(&key) {
                    seen_edges.insert(key.clone());
                    edges.push(GraphEdge {
                        source: rel.clone(),
                        target: target_rel.clone(),
                    });
                }
            }
        }
    }

    let vault_name = vault
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    Ok(GraphResult {
        nodes,
        edges,
        vault_name,
    })
}

// ---------------------------------------------------------------------------
// 2. get_tree
// ---------------------------------------------------------------------------
#[derive(Serialize)]
pub struct TreeNode {
    name: String,
    path: String,
    is_dir: bool,
    children: Option<Vec<TreeNode>>,
}

fn build_tree(dir: &Path, vault: &Path) -> TreeNode {
    let name = dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let rel = dir
        .strip_prefix(vault)
        .unwrap_or(dir)
        .to_string_lossy()
        .to_string();

    if dir.is_file() {
        return TreeNode {
            name,
            path: rel,
            is_dir: false,
            children: None,
        };
    }

    let mut children: Vec<TreeNode> = fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| build_tree(&e.path(), vault))
        .collect();

    // Directories first, then files; both alphabetical.
    children.sort_by(|a, b| match (b.is_dir, a.is_dir) {
        (true, false) => std::cmp::Ordering::Greater,
        (false, true) => std::cmp::Ordering::Less,
        _ => a.name.cmp(&b.name),
    });

    TreeNode {
        name,
        path: rel,
        is_dir: true,
        children: Some(children),
    }
}

#[tauri::command]
pub async fn get_tree(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    let vault = vault_path(&state);
    if !vault.exists() {
        return Err(format!("Vault directory does not exist: {}", vault.display()));
    }
    Ok(json!({ "root": build_tree(&vault, &vault) }))
}

// ---------------------------------------------------------------------------
// 3. get_file
// ---------------------------------------------------------------------------
#[derive(Serialize)]
pub struct FileResult {
    source: String,
    html: String,
    path: String,
}

#[tauri::command]
pub async fn get_file(
    path: String,
    state: tauri::State<'_, AppState>,
) -> Result<FileResult, String> {
    let vault = vault_path(&state);
    let abs = resolve(&vault, &path);
    let source = fs::read_to_string(&abs)
        .map_err(|e| format!("Cannot read {}: {}", abs.display(), e))?;

    let mut opts = Options::default();
    opts.extension.strikethrough = true;
    opts.extension.table = true;
    opts.extension.autolink = true;
    opts.extension.tasklist = true;
    opts.extension.footnotes = true;
    opts.extension.wikilinks_title_after_pipe = true;
    opts.render.unsafe_ = true; // allow raw HTML passthrough

    let html = markdown_to_html(&source, &opts);

    Ok(FileResult {
        source,
        html,
        path,
    })
}

// ---------------------------------------------------------------------------
// 4. get_notes
// ---------------------------------------------------------------------------
#[derive(Serialize)]
pub struct NoteEntry {
    path: String,
    name: String,
    modified: String,
    created: String,
    tags: Vec<String>,
}

#[tauri::command]
pub async fn get_notes(state: tauri::State<'_, AppState>) -> Result<Vec<NoteEntry>, String> {
    let vault = vault_path(&state);
    let files = collect_md_files(&vault);
    let mut notes: Vec<NoteEntry> = Vec::new();

    for f in files {
        let rel = f
            .strip_prefix(&vault)
            .unwrap_or(&f)
            .to_string_lossy()
            .to_string();
        let name = f
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let meta = fs::metadata(&f).ok();
        let modified = meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .map(|t| {
                let dt: chrono::DateTime<chrono::Utc> = t.into();
                dt.to_rfc3339()
            })
            .unwrap_or_default();
        let created = meta
            .as_ref()
            .and_then(|m| m.created().ok())
            .map(|t| {
                let dt: chrono::DateTime<chrono::Utc> = t.into();
                dt.to_rfc3339()
            })
            .unwrap_or_default();

        let content = fs::read_to_string(&f).unwrap_or_default();
        let fm = parse_frontmatter(&content);
        let tags: Vec<String> = fm
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|t| t.as_str())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();

        notes.push(NoteEntry {
            path: rel,
            name,
            modified,
            created,
            tags,
        });
    }

    // Sort most-recently-modified first
    notes.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(notes)
}

// ---------------------------------------------------------------------------
// 5. get_pinned_notes
// ---------------------------------------------------------------------------
fn pinned_path(vault: &Path) -> PathBuf {
    vault.join(".clin").join("pinned.json")
}

#[tauri::command]
pub async fn get_pinned_notes(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let vault = vault_path(&state);
    let p = pinned_path(&vault);
    if !p.exists() {
        return Ok(vec![]);
    }
    let raw = fs::read_to_string(&p).map_err(|e| e.to_string())?;
    let list: Vec<String> = serde_json::from_str(&raw).unwrap_or_default();
    Ok(list)
}

// ---------------------------------------------------------------------------
// 6. get_calendar
// ---------------------------------------------------------------------------
#[derive(Serialize)]
pub struct CalEvent {
    title: String,
    date: String,
    path: String,
    #[serde(rename = "type")]
    kind: String,
}

#[tauri::command]
pub async fn get_calendar(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<CalEvent>, String> {
    let vault = vault_path(&state);
    let files = collect_md_files(&vault);
    let mut events: Vec<CalEvent> = Vec::new();

    for f in files {
        let content = fs::read_to_string(&f).unwrap_or_default();
        let fm = parse_frontmatter(&content);
        let rel = f
            .strip_prefix(&vault)
            .unwrap_or(&f)
            .to_string_lossy()
            .to_string();
        let title = f
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        // Look for common date-like frontmatter keys
        for key in &["date", "due", "created", "scheduled", "deadline"] {
            if let Some(v) = fm.get(key) {
                let date_str = match v {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    _ => continue,
                };
                events.push(CalEvent {
                    title: title.clone(),
                    date: date_str,
                    path: rel.clone(),
                    kind: key.to_string(),
                });
            }
        }
    }

    events.sort_by(|a, b| a.date.cmp(&b.date));
    Ok(events)
}

// ---------------------------------------------------------------------------
// 7. get_properties
// ---------------------------------------------------------------------------
#[tauri::command]
pub async fn get_properties(
    path: String,
    state: tauri::State<'_, AppState>,
) -> Result<Value, String> {
    let vault = vault_path(&state);
    let abs = resolve(&vault, &path);
    let content =
        fs::read_to_string(&abs).map_err(|e| format!("Cannot read {}: {}", abs.display(), e))?;
    Ok(parse_frontmatter(&content))
}

// ---------------------------------------------------------------------------
// 8. get_outline
// ---------------------------------------------------------------------------
#[derive(Serialize)]
pub struct OutlineItem {
    level: u8,
    text: String,
    anchor: String,
}

#[tauri::command]
pub async fn get_outline(
    path: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<OutlineItem>, String> {
    let vault = vault_path(&state);
    let abs = resolve(&vault, &path);
    let content =
        fs::read_to_string(&abs).map_err(|e| format!("Cannot read {}: {}", abs.display(), e))?;

    let heading_re = Regex::new(r"^(#{1,6})\s+(.+)$").unwrap();
    let mut items: Vec<OutlineItem> = Vec::new();

    for line in content.lines() {
        if let Some(cap) = heading_re.captures(line) {
            let level = cap[1].len() as u8;
            let text = cap[2].trim().to_string();
            // Generate a GitHub-style anchor: lowercase, spaces→hyphens, strip non-word chars
            let anchor = text
                .to_lowercase()
                .chars()
                .map(|c| if c == ' ' { '-' } else { c })
                .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .collect();
            items.push(OutlineItem { level, text, anchor });
        }
    }

    Ok(items)
}

// ---------------------------------------------------------------------------
// 9. get_subnotes
// ---------------------------------------------------------------------------
#[tauri::command]
pub async fn get_subnotes(
    path: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let vault = vault_path(&state);
    // Check for .clin/subnotes.json (or .bin treated as JSON) keyed by path
    let store = vault.join(".clin").join("subnotes.json");
    if store.exists() {
        let raw = fs::read_to_string(&store).map_err(|e| e.to_string())?;
        let map: HashMap<String, Vec<String>> = serde_json::from_str(&raw).unwrap_or_default();
        return Ok(map.get(&path).cloned().unwrap_or_default());
    }
    Ok(vec![])
}

// ---------------------------------------------------------------------------
// 10. get_bases
// ---------------------------------------------------------------------------
#[tauri::command]
pub async fn get_bases(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    let vault = vault_path(&state);
    let bases: Vec<String> = WalkDir::new(&vault)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path().extension().and_then(|x| x.to_str()) == Some("base")
        })
        .map(|e| {
            e.path()
                .strip_prefix(&vault)
                .unwrap_or(e.path())
                .to_string_lossy()
                .to_string()
        })
        .collect();
    Ok(bases)
}

// ---------------------------------------------------------------------------
// 11. get_base
// ---------------------------------------------------------------------------
#[tauri::command]
pub async fn get_base(
    path: String,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let vault = vault_path(&state);
    let abs = resolve(&vault, &path);
    fs::read_to_string(&abs).map_err(|e| format!("Cannot read base {}: {}", abs.display(), e))
}

// ---------------------------------------------------------------------------
// 12. get_canvas_files
// ---------------------------------------------------------------------------
#[tauri::command]
pub async fn get_canvas_files(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let vault = vault_path(&state);
    let files: Vec<String> = WalkDir::new(&vault)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path().extension().and_then(|x| x.to_str()) == Some("canvas")
        })
        .map(|e| {
            e.path()
                .strip_prefix(&vault)
                .unwrap_or(e.path())
                .to_string_lossy()
                .to_string()
        })
        .collect();
    Ok(files)
}

// ---------------------------------------------------------------------------
// 13. get_canvas
// ---------------------------------------------------------------------------
#[tauri::command]
pub async fn get_canvas(
    path: String,
    state: tauri::State<'_, AppState>,
) -> Result<Value, String> {
    let vault = vault_path(&state);
    let abs = resolve(&vault, &path);
    let raw =
        fs::read_to_string(&abs).map_err(|e| format!("Cannot read canvas {}: {}", abs.display(), e))?;
    serde_json::from_str(&raw)
        .map_err(|e| format!("Canvas file is not valid JSON ({}): {}", abs.display(), e))
}

// ---------------------------------------------------------------------------
// 14. get_themes
// ---------------------------------------------------------------------------
#[tauri::command]
pub async fn get_themes() -> Result<Vec<String>, String> {
    Ok(vec![
        "default".into(),
        "dark".into(),
        "light".into(),
        "nord".into(),
        "dracula".into(),
        "solarized-dark".into(),
        "solarized-light".into(),
        "monokai".into(),
        "tokyo-night".into(),
        "catppuccin-mocha".into(),
        "catppuccin-latte".into(),
        "gruvbox-dark".into(),
        "gruvbox-light".into(),
        "rose-pine".into(),
        "one-dark".into(),
        "ayu-dark".into(),
        "ayu-light".into(),
        "material".into(),
        "github".into(),
    ])
}

// ---------------------------------------------------------------------------
// 15. get_vaults
// ---------------------------------------------------------------------------
#[tauri::command]
pub async fn get_vaults(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<Value>, String> {
    let current = vault_path(&state);
    let config = read_config();
    let mut vaults: Vec<Value> = config
        .get("vaults")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    // Ensure the currently-active vault is always in the list.
    let current_str = current.to_string_lossy().to_string();
    let already_present = vaults
        .iter()
        .any(|v| v.get("path").and_then(|p| p.as_str()) == Some(&current_str));
    if !already_present {
        vaults.insert(
            0,
            json!({
                "path": current_str,
                "name": current.file_name().unwrap_or_default().to_string_lossy()
            }),
        );
    }

    Ok(vaults)
}

// ---------------------------------------------------------------------------
// 16. get_templates
// ---------------------------------------------------------------------------
#[tauri::command]
pub async fn get_templates(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let vault = vault_path(&state);
    let mut results: Vec<String> = Vec::new();

    // 1. Look inside the vault's own templates/ folder.
    let vault_templates = vault.join("templates");
    if vault_templates.is_dir() {
        for entry in WalkDir::new(&vault_templates)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            results.push(
                entry
                    .path()
                    .strip_prefix(&vault)
                    .unwrap_or(entry.path())
                    .to_string_lossy()
                    .to_string(),
            );
        }
    }

    // 2. Look in ~/.config/clin/templates/
    let global_templates = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("clin")
        .join("templates");
    if global_templates.is_dir() {
        for entry in WalkDir::new(&global_templates)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            results.push(entry.path().to_string_lossy().to_string());
        }
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// 17. open_file — open in system default app via shell plugin
// ---------------------------------------------------------------------------
#[tauri::command]
pub async fn open_file(
    path: String,
    state: tauri::State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<Value, String> {
    use tauri_plugin_shell::ShellExt;
    let vault = vault_path(&state);
    let abs = resolve(&vault, &path);
    #[allow(deprecated)]
    app_handle
        .shell()
        .open(abs.to_string_lossy().as_ref(), None)
        .map_err(|e| format!("Failed to open file: {}", e))?;
    Ok(json!({ "ok": true }))
}

// ---------------------------------------------------------------------------
// 18. create_file
// ---------------------------------------------------------------------------
#[derive(Deserialize)]
pub struct CreateFileArgs {
    path: String,
    template: Option<String>,
    content: Option<String>,
}

#[tauri::command]
pub async fn create_file(
    args: CreateFileArgs,
    state: tauri::State<'_, AppState>,
) -> Result<Value, String> {
    let vault = vault_path(&state);
    let abs = resolve(&vault, &args.path);

    if let Some(parent) = abs.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let body = if let Some(content) = args.content {
        content
    } else if let Some(tmpl_path) = args.template {
        let tmpl_abs = resolve(&vault, &tmpl_path);
        fs::read_to_string(&tmpl_abs).unwrap_or_default()
    } else {
        // Generate a minimal frontmatter stub
        let title = abs
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let now = chrono::Utc::now().format("%Y-%m-%d").to_string();
        format!("---\ntitle: {}\ndate: {}\ntags: []\n---\n\n# {}\n", title, now, title)
    };

    fs::write(&abs, body).map_err(|e| format!("Cannot write {}: {}", abs.display(), e))?;
    Ok(json!({ "ok": true, "path": args.path }))
}

// ---------------------------------------------------------------------------
// 19. move_file
// ---------------------------------------------------------------------------
#[derive(Deserialize)]
pub struct MoveFileArgs {
    from: String,
    to: String,
    update_links: Option<bool>,
}

#[tauri::command]
pub async fn move_file(
    args: MoveFileArgs,
    state: tauri::State<'_, AppState>,
) -> Result<Value, String> {
    let vault = vault_path(&state);
    let src = resolve(&vault, &args.from);
    let dst = resolve(&vault, &args.to);

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    fs::rename(&src, &dst)
        .map_err(|e| format!("Cannot move {} → {}: {}", src.display(), dst.display(), e))?;

    // Optionally rewrite wikilinks across the vault.
    if args.update_links.unwrap_or(true) {
        let old_stem = src
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let new_stem = dst
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        if old_stem != new_stem {
            for f in collect_md_files(&vault) {
                if let Ok(content) = fs::read_to_string(&f) {
                    let updated = rewrite_wikilinks(&content, &old_stem, &new_stem);
                    if updated != content {
                        let _ = fs::write(&f, updated);
                    }
                }
            }
        }
    }

    Ok(json!({ "ok": true, "from": args.from, "to": args.to }))
}

// ---------------------------------------------------------------------------
// 20. create_folder
// ---------------------------------------------------------------------------
#[derive(Deserialize)]
pub struct CreateFolderArgs {
    path: String,
}

#[tauri::command]
pub async fn create_folder(
    args: CreateFolderArgs,
    state: tauri::State<'_, AppState>,
) -> Result<Value, String> {
    let vault = vault_path(&state);
    let abs = resolve(&vault, &args.path);
    fs::create_dir_all(&abs)
        .map_err(|e| format!("Cannot create directory {}: {}", abs.display(), e))?;
    Ok(json!({ "ok": true, "path": args.path }))
}

// ---------------------------------------------------------------------------
// 21. create_link
// ---------------------------------------------------------------------------
#[derive(Deserialize)]
pub struct CreateLinkArgs {
    source: String,
    target: String,
}

#[tauri::command]
pub async fn create_link(
    args: CreateLinkArgs,
    state: tauri::State<'_, AppState>,
) -> Result<Value, String> {
    let vault = vault_path(&state);
    let abs = resolve(&vault, &args.source);
    let mut content =
        fs::read_to_string(&abs).map_err(|e| format!("Cannot read {}: {}", abs.display(), e))?;

    // Derive the wikilink target stem (strip .md if present)
    let stem = Path::new(&args.target)
        .file_stem()
        .unwrap_or_else(|| std::ffi::OsStr::new(&args.target))
        .to_string_lossy()
        .to_string();
    let link = format!("[[{}]]", stem);

    // Avoid adding duplicate links
    if !content.contains(&link) {
        content.push('\n');
        content.push_str(&link);
        content.push('\n');
        fs::write(&abs, content)
            .map_err(|e| format!("Cannot write {}: {}", abs.display(), e))?;
    }

    Ok(json!({ "ok": true, "link": link }))
}

// ---------------------------------------------------------------------------
// 22. delete_link
// ---------------------------------------------------------------------------
#[derive(Deserialize)]
pub struct DeleteLinkArgs {
    source: String,
    target: String,
}

#[tauri::command]
pub async fn delete_link(
    args: DeleteLinkArgs,
    state: tauri::State<'_, AppState>,
) -> Result<Value, String> {
    let vault = vault_path(&state);
    let abs = resolve(&vault, &args.source);
    let content =
        fs::read_to_string(&abs).map_err(|e| format!("Cannot read {}: {}", abs.display(), e))?;

    let stem = Path::new(&args.target)
        .file_stem()
        .unwrap_or_else(|| std::ffi::OsStr::new(&args.target))
        .to_string_lossy()
        .to_string();

    // Remove lines that consist solely of the wikilink (and surrounding blank lines)
    let link_re =
        Regex::new(&format!(r"\[\[{}(?:[|#][^\[\]]*)?\]\]", regex::escape(&stem))).unwrap();
    let updated = link_re.replace_all(&content, "").to_string();

    if updated != content {
        fs::write(&abs, updated)
            .map_err(|e| format!("Cannot write {}: {}", abs.display(), e))?;
    }

    Ok(json!({ "ok": true }))
}

// ---------------------------------------------------------------------------
// 23. pin_note — toggle entry in .clin/pinned.json
// ---------------------------------------------------------------------------
#[derive(Deserialize)]
pub struct PinNoteArgs {
    path: String,
    pinned: Option<bool>,
}

#[tauri::command]
pub async fn pin_note(
    args: PinNoteArgs,
    state: tauri::State<'_, AppState>,
) -> Result<Value, String> {
    let vault = vault_path(&state);
    let p = pinned_path(&vault);
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let mut list: Vec<String> = if p.exists() {
        serde_json::from_str(&fs::read_to_string(&p).unwrap_or_default()).unwrap_or_default()
    } else {
        vec![]
    };

    let already_pinned = list.contains(&args.path);
    // `pinned` field: true = force pin, false = force unpin, None = toggle
    let should_pin = args.pinned.unwrap_or(!already_pinned);

    if should_pin && !already_pinned {
        list.push(args.path.clone());
    } else if !should_pin && already_pinned {
        list.retain(|x| x != &args.path);
    }

    fs::write(&p, serde_json::to_string_pretty(&list).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;

    Ok(json!({ "ok": true, "pinned": should_pin, "path": args.path }))
}

// ---------------------------------------------------------------------------
// 24. set_theme
// ---------------------------------------------------------------------------
#[derive(Deserialize)]
pub struct SetThemeArgs {
    theme: String,
}

#[tauri::command]
pub async fn set_theme(args: SetThemeArgs) -> Result<Value, String> {
    let mut config = read_config();
    config["theme"] = json!(args.theme);
    write_config(&config)?;
    Ok(json!({ "ok": true, "theme": args.theme }))
}

// ---------------------------------------------------------------------------
// 25. set_vault
// ---------------------------------------------------------------------------
#[derive(Deserialize)]
pub struct SetVaultArgs {
    path: String,
}

#[tauri::command]
pub async fn set_vault(
    args: SetVaultArgs,
    state: tauri::State<'_, AppState>,
) -> Result<Value, String> {
    let new_vault = PathBuf::from(&args.path);
    if !new_vault.exists() {
        return Err(format!("Vault path does not exist: {}", args.path));
    }

    // Update in-memory state
    {
        let mut guard = state.vault_path.lock().unwrap();
        *guard = new_vault.clone();
    }

    // Persist to config
    let mut config = read_config();
    config["active_vault"] = json!(args.path);

    // Keep a "recent_vaults" list (most-recent first, capped at 10)
    let path_str = args.path.clone();
    let vault_entry = json!({
        "path": path_str,
        "name": new_vault.file_name().unwrap_or_default().to_string_lossy()
    });
    let mut recents: Vec<serde_json::Value> = config
        .get("recent_vaults")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    recents.retain(|v| v.get("path").and_then(|p| p.as_str()) != Some(&path_str));
    recents.insert(0, vault_entry);
    recents.truncate(10);
    config["recent_vaults"] = json!(recents);

    write_config(&config)?;
    Ok(json!({ "ok": true, "path": args.path }))
}

// ---------------------------------------------------------------------------
// 26. sync_calendar — stub (CalDAV support is a future feature)
// ---------------------------------------------------------------------------
#[tauri::command]
pub async fn sync_calendar(_args: Option<Value>) -> Result<Value, String> {
    Ok(json!({
        "ok": true,
        "message": "CalDAV sync not yet configured"
    }))
}

// ---------------------------------------------------------------------------
// 27. save_file — write note content to disk
// ---------------------------------------------------------------------------
#[derive(Deserialize)]
pub struct SaveFileArgs {
    path: String,
    content: String,
}

#[tauri::command]
pub async fn save_file(
    args: SaveFileArgs,
    state: tauri::State<'_, AppState>,
) -> Result<Value, String> {
    let vault = vault_path(&state);
    let abs = resolve(&vault, &args.path);
    fs::write(&abs, &args.content)
        .map_err(|e| format!("Cannot write {}: {}", abs.display(), e))?;
    Ok(json!({ "ok": true }))
}

// ---------------------------------------------------------------------------
// 28. delete_file — remove a note from the vault
// ---------------------------------------------------------------------------
#[derive(Deserialize)]
pub struct DeleteFileArgs {
    path: String,
}

#[tauri::command]
pub async fn delete_file(
    args: DeleteFileArgs,
    state: tauri::State<'_, AppState>,
) -> Result<Value, String> {
    let vault = vault_path(&state);
    let abs = resolve(&vault, &args.path);
    fs::remove_file(&abs)
        .map_err(|e| format!("Cannot delete {}: {}", abs.display(), e))?;
    Ok(json!({ "ok": true }))
}

// ---------------------------------------------------------------------------
// 29. delete_folder — remove a directory (and all contents)
// ---------------------------------------------------------------------------
#[derive(Deserialize)]
pub struct DeleteFolderArgs {
    path: String,
}

#[tauri::command]
pub async fn delete_folder(
    args: DeleteFolderArgs,
    state: tauri::State<'_, AppState>,
) -> Result<Value, String> {
    let vault = vault_path(&state);
    let abs = resolve(&vault, &args.path);
    fs::remove_dir_all(&abs)
        .map_err(|e| format!("Cannot delete folder {}: {}", abs.display(), e))?;
    Ok(json!({ "ok": true }))
}

// ---------------------------------------------------------------------------
// 30. duplicate_file — copy a note to a new path
// ---------------------------------------------------------------------------
#[derive(Deserialize)]
pub struct DuplicateFileArgs {
    path: String,
    new_path: Option<String>,
}

#[tauri::command]
pub async fn duplicate_file(
    args: DuplicateFileArgs,
    state: tauri::State<'_, AppState>,
) -> Result<Value, String> {
    let vault = vault_path(&state);
    let src = resolve(&vault, &args.path);
    let dest_rel = args.new_path.unwrap_or_else(|| {
        let stem = src.file_stem().unwrap_or_default().to_string_lossy();
        let ext  = src.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
        let parent = src.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        if parent.is_empty() {
            format!("{}-copy{}", stem, ext)
        } else {
            format!("{}/{}-copy{}", parent, stem, ext)
        }
    });
    let dst = resolve(&vault, &dest_rel);
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::copy(&src, &dst)
        .map_err(|e| format!("Cannot duplicate {}: {}", src.display(), e))?;
    Ok(json!({ "ok": true, "path": dest_rel }))
}

// ---------------------------------------------------------------------------
// 31. rename_file — rename a note, optionally rewriting wikilinks
// ---------------------------------------------------------------------------
#[derive(Deserialize)]
pub struct RenameFileArgs {
    old_path: String,
    new_path: String,
    rename_wikilinks: Option<bool>,
}

#[tauri::command]
pub async fn rename_file(
    args: RenameFileArgs,
    state: tauri::State<'_, AppState>,
) -> Result<Value, String> {
    let vault = vault_path(&state);
    let src = resolve(&vault, &args.old_path);
    let dst = resolve(&vault, &args.new_path);

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::rename(&src, &dst)
        .map_err(|e| format!("Cannot rename: {}", e))?;

    if args.rename_wikilinks.unwrap_or(true) {
        let old_stem = src.file_stem().unwrap_or_default().to_string_lossy().to_string();
        let new_stem = dst.file_stem().unwrap_or_default().to_string_lossy().to_string();
        if old_stem != new_stem {
            for f in collect_md_files(&vault) {
                if let Ok(content) = fs::read_to_string(&f) {
                    let updated = rewrite_wikilinks(&content, &old_stem, &new_stem);
                    if updated != content {
                        let _ = fs::write(&f, updated);
                    }
                }
            }
        }
    }

    Ok(json!({ "ok": true, "renamed": true }))
}

// ---------------------------------------------------------------------------
// 32. save_properties — write frontmatter properties back to a note
// ---------------------------------------------------------------------------
#[derive(Deserialize)]
pub struct SavePropertiesArgs {
    path: String,
    properties: Value,
}

#[tauri::command]
pub async fn save_properties(
    args: SavePropertiesArgs,
    state: tauri::State<'_, AppState>,
) -> Result<Value, String> {
    let vault = vault_path(&state);
    let abs = resolve(&vault, &args.path);
    let content = fs::read_to_string(&abs)
        .map_err(|e| format!("Cannot read {}: {}", abs.display(), e))?;

    // Simple approach: replace or prepend frontmatter.
    let (_, body) = split_frontmatter(&content);
    let mut fm = String::from("---\n");
    if let Some(obj) = args.properties.as_object() {
        for (k, v) in obj {
            match v {
                Value::String(s) => fm.push_str(&format!("{}: {}\n", k, s)),
                Value::Array(arr) => {
                    let items: Vec<String> = arr.iter()
                        .map(|x| x.as_str().unwrap_or_default().to_string())
                        .collect();
                    fm.push_str(&format!("{}: [{}]\n", k, items.join(", ")));
                }
                _ => fm.push_str(&format!("{}: {}\n", k, v)),
            }
        }
    }
    fm.push_str("---\n");
    fm.push_str(body);
    fs::write(&abs, fm).map_err(|e| format!("Cannot write: {}", e))?;
    Ok(json!({ "ok": true }))
}

fn split_frontmatter(content: &str) -> (String, &str) {
    if content.starts_with("---\n") || content.starts_with("---\r\n") {
        let rest = &content[4..];
        if let Some(end) = rest.find("\n---\n").or_else(|| rest.find("\r\n---\r\n")) {
            let fm = rest[..end].to_string();
            let body_start = end + if rest[end..].starts_with("\r\n") { 6 } else { 5 };
            return (fm, &content[4 + body_start..]);
        }
    }
    (String::new(), content)
}

// ---------------------------------------------------------------------------
// 33. search_notes — full-text search across vault
// ---------------------------------------------------------------------------
#[derive(Deserialize)]
pub struct SearchArgs {
    q: String,
}

#[tauri::command]
pub async fn search_notes(
    args: SearchArgs,
    state: tauri::State<'_, AppState>,
) -> Result<Value, String> {
    let vault = vault_path(&state);
    let query = args.q.to_lowercase();
    let mut results = Vec::new();

    for entry in collect_md_files(&vault) {
        if let Ok(content) = fs::read_to_string(&entry) {
            let lower = content.to_lowercase();
            if lower.contains(&query) {
                let idx = lower.find(&query).unwrap_or(0);
                let start = idx.saturating_sub(60);
                let end = (idx + query.len() + 60).min(content.len());
                let snippet = content[start..end].replace('\n', " ");
                let rel = entry.strip_prefix(&vault).unwrap_or(&entry);
                let title = entry.file_stem().unwrap_or_default().to_string_lossy()
                    .to_string().replace('-', " ").replace('_', " ");
                results.push(json!({
                    "path": rel.to_string_lossy(),
                    "title": title,
                    "snippet": format!("…{}…", snippet),
                }));
            }
        }
    }

    Ok(json!({ "results": results }))
}

// ---------------------------------------------------------------------------
// 34. get_smart_folder — filter notes by virtual folder name
// ---------------------------------------------------------------------------
#[derive(Deserialize)]
pub struct SmartFolderArgs {
    name: String,
}

#[tauri::command]
pub async fn get_smart_folder(
    args: SmartFolderArgs,
    state: tauri::State<'_, AppState>,
) -> Result<Value, String> {
    let vault = vault_path(&state);
    let now = chrono::Utc::now();
    let today = now.format("%Y-%m-%d").to_string();
    let week_ago = (now - chrono::Duration::days(7)).format("%Y-%m-%d").to_string();

    let pinned_path = vault.join(".clin").join("pinned.json");
    let pinned: std::collections::HashSet<String> = fs::read_to_string(&pinned_path)
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .unwrap_or_default()
        .into_iter()
        .collect();

    let mut results = Vec::new();
    for entry in collect_md_files(&vault) {
        let rel = entry.strip_prefix(&vault).unwrap_or(&entry)
            .to_string_lossy().replace('\\', "/");
        let meta = entry.metadata().map_err(|e| e.to_string())?;
        let modified = meta.modified().ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| {
                let secs = d.as_secs() as i64;
                chrono::DateTime::from_timestamp(secs, 0)
                    .map(|dt| dt.format("%Y-%m-%d").to_string())
                    .unwrap_or_default()
            })
            .unwrap_or_default();

        let include = match args.name.as_str() {
            "today" => modified == today,
            "this-week" => modified >= week_ago,
            "pinned" => pinned.contains(&rel),
            "untagged" => {
                let content = fs::read_to_string(&entry).unwrap_or_default();
                let m = Matter::<YAML>::new();
                let parsed = m.parse(&content);
                let tags_count = parsed.data.as_ref()
                    .and_then(|d| d.deserialize::<Value>().ok())
                    .and_then(|v| v.get("tags").and_then(|t| t.as_array()).map(|a| a.len()))
                    .unwrap_or(0);
                tags_count == 0
            }
            _ => false,
        };

        if include {
            let title = entry.file_stem().unwrap_or_default()
                .to_string_lossy().to_string();
            results.push(json!({
                "id": rel,
                "title": title,
                "modified": modified,
                "pinned": pinned.contains(&rel),
            }));
        }
    }

    Ok(json!(results))
}

// ---------------------------------------------------------------------------
// 35. pick_vault_dir — open a native OS folder picker dialog (Tauri-only)
// ---------------------------------------------------------------------------
#[tauri::command]
pub async fn pick_vault_dir(app: tauri::AppHandle) -> Result<Value, String> {
    use tauri_plugin_dialog::DialogExt;
    let path = app.dialog()
        .file()
        .set_title("Select vault directory")
        .blocking_pick_folder();
    match path {
        Some(p) => Ok(json!({ "ok": true, "path": p.to_string() })),
        None    => Ok(json!({ "ok": false })),
    }
}

// ---------------------------------------------------------------------------
// 36. get_health — vault health summary (broken links, orphans, stats)
// ---------------------------------------------------------------------------
#[tauri::command]
pub async fn get_health(state: tauri::State<'_, AppState>) -> Result<Value, String> {
    let vault = vault_path(&state);
    let files = collect_md_files(&vault);
    let total = files.len();

    let mut all_links: Vec<(String, String)> = Vec::new(); // (source_rel, target_stem)
    let mut defined: std::collections::HashSet<String> = std::collections::HashSet::new();

    let re = Regex::new(r"\[\[([^\]|]+)").unwrap();
    for f in &files {
        let rel = f.strip_prefix(&vault).unwrap_or(f)
            .to_string_lossy().replace('\\', "/");
        let stem = f.file_stem().unwrap_or_default().to_string_lossy().to_string();
        defined.insert(stem);
        if let Ok(content) = fs::read_to_string(f) {
            for cap in re.captures_iter(&content) {
                let target = cap[1].trim().to_string();
                all_links.push((rel.clone(), target));
            }
        }
    }

    let broken: Vec<Value> = all_links.iter()
        .filter(|(_, target)| {
            let stem = target.split('/').last().unwrap_or(target);
            let stem_no_ext = stem.trim_end_matches(".md");
            !defined.contains(stem_no_ext)
        })
        .map(|(src, tgt)| json!({ "kind": "broken_link", "note": src, "target": tgt }))
        .collect();

    // Orphans: notes with no inbound links
    let mut has_inbound: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (_, target) in &all_links {
        has_inbound.insert(target.split('/').last()
            .unwrap_or(target).trim_end_matches(".md").to_string());
    }
    let orphans: Vec<Value> = files.iter()
        .filter(|f| {
            let stem = f.file_stem().unwrap_or_default().to_string_lossy().to_string();
            !has_inbound.contains(&stem)
        })
        .map(|f| {
            let rel = f.strip_prefix(&vault).unwrap_or(f)
                .to_string_lossy().replace('\\', "/");
            json!({ "kind": "orphan", "note": rel, "detail": "no inbound links" })
        })
        .collect();

    let mut issues = broken.clone();
    issues.extend(orphans.iter().cloned());

    Ok(json!({
        "total_notes": total,
        "broken_links": broken.len(),
        "orphaned_notes": orphans.len(),
        "total_edges": all_links.len(),
        "issues": issues,
    }))
}

// ---------------------------------------------------------------------------
// 37. get_settings — read persisted app settings
// ---------------------------------------------------------------------------
#[tauri::command]
pub async fn get_settings() -> Result<Value, String> {
    let cfg = read_config();
    Ok(json!({
        "theme":           cfg.get("theme").and_then(|v| v.as_str()).unwrap_or("default"),
        "vault":           cfg.get("vault").and_then(|v| v.as_str()).unwrap_or("~/vault"),
        "graph_physics":   cfg.get("graph_physics").and_then(|v| v.as_bool()).unwrap_or(true),
        "auto_save":       cfg.get("auto_save").and_then(|v| v.as_bool()).unwrap_or(true),
        "spell_check":     cfg.get("spell_check").and_then(|v| v.as_bool()).unwrap_or(false),
        "editor_font_size": cfg.get("editor_font_size").and_then(|v| v.as_i64()).unwrap_or(14),
    }))
}

// ---------------------------------------------------------------------------
// 38. set_settings — persist arbitrary settings key/value pairs
// ---------------------------------------------------------------------------
#[tauri::command]
pub async fn set_settings(settings: Value) -> Result<Value, String> {
    let mut cfg = read_config();
    if let Some(obj) = settings.as_object() {
        for (k, v) in obj {
            cfg[k] = v.clone();
        }
    }
    write_config(&cfg)?;
    Ok(json!({ "ok": true }))
}

// ---------------------------------------------------------------------------
// 39. get_recent_vaults — list recently opened vault folders
// ---------------------------------------------------------------------------
#[tauri::command]
pub async fn get_recent_vaults() -> Result<Value, String> {
    let cfg = read_config();
    Ok(cfg.get("recent_vaults").cloned().unwrap_or(json!([])))
}

// ===========================================================================
// Protocol handler support — clin:// URI scheme
//
// register_uri_scheme_protocol("clin", ...) in lib.rs routes all fetch()
// calls from the WebView here.  These are plain sync functions that mirror
// every #[tauri::command] above, taking vault: &Path directly instead of
// tauri::State so they work in the sync protocol-handler closure.
// ===========================================================================

// ── URL helpers ──────────────────────────────────────────────────────────────

fn qs_get(qs: &str, key: &str) -> String {
    for part in qs.split('&') {
        if let Some((k, v)) = part.split_once('=') {
            if k == key {
                return url_decode(v);
            }
        }
    }
    String::new()
}

fn url_decode(s: &str) -> String {
    let mut bytes: Vec<u8> = Vec::with_capacity(s.len());
    let src = s.as_bytes();
    let mut i = 0;
    while i < src.len() {
        if src[i] == b'%' && i + 2 < src.len() {
            let hi = (src[i + 1] as char).to_digit(16);
            let lo = (src[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                bytes.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        } else if src[i] == b'+' {
            bytes.push(b' ');
            i += 1;
            continue;
        }
        bytes.push(src[i]);
        i += 1;
    }
    String::from_utf8(bytes)
        .unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).to_string())
}

// ── Entry point ──────────────────────────────────────────────────────────────

/// Called from the register_uri_scheme_protocol closure in lib.rs.
/// Returns (HTTP status, JSON value).
pub fn handle_proto_request(
    vault: &Path,
    method: &str,
    path: &str,
    qs: &str,
    body: &Value,
) -> (u16, Value) {
    match proto_route(vault, method, path, qs, body) {
        Ok(v)  => (200, v),
        Err(e) => (500, json!({ "error": e })),
    }
}

fn proto_route(
    vault: &Path,
    method: &str,
    path: &str,
    qs: &str,
    body: &Value,
) -> Result<Value, String> {
    match (method, path) {
        // ── GET ─────────────────────────────────────────────────────────────
        ("GET", "/graph.json")    => p_get_graph(vault),
        ("GET", "/tree")          => p_get_tree(vault),
        ("GET", "/file")          => p_get_file(vault, &qs_get(qs, "path")),
        ("GET", "/notes")         => p_get_notes(vault),
        ("GET", "/notes/pinned")  => p_get_pinned_notes(vault),
        ("GET", "/calendar")      => p_get_calendar(vault),
        ("GET", "/properties")    => p_get_properties(vault, &qs_get(qs, "path")),
        ("GET", "/outline")       => p_get_outline(vault, &qs_get(qs, "path")),
        ("GET", "/subnotes")      => p_get_subnotes(vault, &qs_get(qs, "path")),
        ("GET", "/bases")         => p_get_bases(vault),
        ("GET", "/base")          => p_get_base(vault, &qs_get(qs, "path")),
        ("GET", "/canvas-files")  => p_get_canvas_files(vault),
        ("GET", "/canvas")        => p_get_canvas(vault, &qs_get(qs, "path")),
        ("GET", "/themes")        => p_get_themes(),
        ("GET", "/vaults")        => p_get_vaults(vault),
        ("GET", "/templates")     => p_get_templates(vault),
        ("GET", "/health")        => p_get_health(vault),
        ("GET", "/settings")      => p_get_settings(),
        ("GET", "/recent-vaults") => p_get_recent_vaults_p(),
        ("GET", "/search")        => p_search_notes(vault, &qs_get(qs, "q")),
        ("GET", "/smart-folder")  => p_get_smart_folder(vault, &qs_get(qs, "name")),
        // ── POST ────────────────────────────────────────────────────────────
        ("POST", "/file/open") =>
            p_open_file(vault, body.get("path").and_then(|v| v.as_str()).unwrap_or("")),
        ("POST", "/file/save")       => p_save_file(vault, body),
        ("POST", "/file/create")     => p_create_file(vault, body),
        ("POST", "/file/move")       => p_move_file(vault, body),
        ("POST", "/file/duplicate")  => p_duplicate_file(vault, body),
        ("POST",   "/file/delete")
        | ("DELETE", "/file/delete") => p_delete_file(vault, body),
        ("POST",   "/folder/delete")
        | ("DELETE", "/folder/delete") => p_delete_folder(vault, body),
        ("POST", "/folder/create")   => p_create_folder(vault, body),
        ("POST", "/rename")          => p_rename_file(vault, body),
        ("POST", "/link/create")     => p_create_link(vault, body),
        ("POST", "/link/delete")     => p_delete_link(vault, body),
        ("POST", "/note/pin")        => p_pin_note(vault, body),
        ("POST", "/properties/save") => p_save_properties(vault, body),
        ("POST", "/settings/theme")  => p_set_theme(body),
        ("POST", "/settings/vault")  => p_set_vault(body),
        ("POST", "/settings")        => p_set_settings(body),
        ("POST", "/calendar/sync")   =>
            Ok(json!({ "ok": true, "message": "CalDAV sync not yet configured" })),
        ("POST", "/vault/pick")      =>
            Ok(json!({ "ok": false, "message": "vault picker not available via protocol" })),
        ("OPTIONS", _)               => Ok(json!({})),
        _ => Err(format!("unknown endpoint: {} {}", method, path)),
    }
}

// ── GET implementations ──────────────────────────────────────────────────────

fn p_get_graph(vault: &Path) -> Result<Value, String> {
    let files = collect_md_files(vault);
    let mut stem_index: HashMap<String, String> = HashMap::new();
    for f in &files {
        let rel = f.strip_prefix(vault).unwrap_or(f).to_string_lossy().to_string();
        let stem = f.file_stem().unwrap_or_default().to_string_lossy().to_string();
        stem_index.insert(stem, rel.clone());
        stem_index.entry(rel.clone()).or_insert(rel);
    }
    let mut nodes: Vec<Value> = Vec::new();
    let mut edges: Vec<Value> = Vec::new();
    let mut seen: HashSet<(String, String)> = HashSet::new();
    for f in &files {
        let rel = f.strip_prefix(vault).unwrap_or(f).to_string_lossy().to_string();
        let label = f.file_stem().unwrap_or_default().to_string_lossy().to_string();
        let folder = f.parent()
            .and_then(|p| p.strip_prefix(vault).ok())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let content = fs::read_to_string(f).unwrap_or_default();
        let fm = parse_frontmatter(&content);
        let title = fm["title"].as_str().unwrap_or(&label).to_string();
        let tags: Vec<String> = fm["tags"].as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        let modified = f.metadata().ok().and_then(|m| m.modified().ok()).map(|t| {
            let dt: chrono::DateTime<chrono::Utc> = t.into();
            dt.to_rfc3339()
        });
        let created = fm["date"].as_str()
            .or_else(|| fm["created"].as_str())
            .map(|s| s.to_string());
        nodes.push(json!({
            "id": rel, "label": label, "path": rel, "title": title,
            "tags": tags, "folder": folder, "created": created, "modified": modified
        }));
        for link in extract_wikilinks(&content) {
            if let Some(target) = stem_index.get(&link) {
                let key = (rel.clone(), target.clone());
                if seen.insert(key) {
                    edges.push(json!({ "source": rel, "target": target }));
                }
            }
        }
    }
    let vault_name = vault.file_name().unwrap_or_default().to_string_lossy().to_string();
    Ok(json!({ "nodes": nodes, "edges": edges, "vault_name": vault_name }))
}

fn p_get_tree(vault: &Path) -> Result<Value, String> {
    if !vault.exists() {
        return Err(format!("vault does not exist: {}", vault.display()));
    }
    Ok(json!({ "root": build_tree(vault, vault) }))
}

fn p_get_file(vault: &Path, path: &str) -> Result<Value, String> {
    if path.is_empty() {
        return Err("path is required".to_string());
    }
    let abs = resolve(vault, path);
    let source = fs::read_to_string(&abs)
        .map_err(|e| format!("cannot read {}: {}", abs.display(), e))?;
    let mut opts = Options::default();
    opts.extension.strikethrough = true;
    opts.extension.table = true;
    opts.extension.autolink = true;
    opts.extension.tasklist = true;
    opts.extension.footnotes = true;
    opts.extension.wikilinks_title_after_pipe = true;
    opts.render.unsafe_ = true;
    let html = markdown_to_html(&source, &opts);
    Ok(json!({ "source": source, "html": html, "path": path }))
}

fn p_get_notes(vault: &Path) -> Result<Value, String> {
    let files = collect_md_files(vault);
    let mut notes: Vec<Value> = Vec::new();
    for f in files {
        let rel  = f.strip_prefix(vault).unwrap_or(&f).to_string_lossy().to_string();
        let name = f.file_stem().unwrap_or_default().to_string_lossy().to_string();
        let meta = fs::metadata(&f).ok();
        let modified = meta.as_ref().and_then(|m| m.modified().ok()).map(|t| {
            let dt: chrono::DateTime<chrono::Utc> = t.into(); dt.to_rfc3339()
        }).unwrap_or_default();
        let created = meta.as_ref().and_then(|m| m.created().ok()).map(|t| {
            let dt: chrono::DateTime<chrono::Utc> = t.into(); dt.to_rfc3339()
        }).unwrap_or_default();
        let content = fs::read_to_string(&f).unwrap_or_default();
        let fm = parse_frontmatter(&content);
        let tags: Vec<String> = fm.get("tags").and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|t| t.as_str()).map(String::from).collect())
            .unwrap_or_default();
        notes.push(json!({ "path": rel, "name": name, "modified": modified,
                           "created": created, "tags": tags }));
    }
    notes.sort_by(|a, b| {
        b["modified"].as_str().unwrap_or("").cmp(a["modified"].as_str().unwrap_or(""))
    });
    Ok(json!(notes))
}

fn p_get_pinned_notes(vault: &Path) -> Result<Value, String> {
    let p = pinned_path(vault);
    if !p.exists() {
        return Ok(json!([]));
    }
    let raw  = fs::read_to_string(&p).map_err(|e| e.to_string())?;
    let list: Vec<String> = serde_json::from_str(&raw).unwrap_or_default();
    Ok(json!(list))
}

fn p_get_calendar(vault: &Path) -> Result<Value, String> {
    let files = collect_md_files(vault);
    let mut events: Vec<Value> = Vec::new();
    for f in files {
        let content  = fs::read_to_string(&f).unwrap_or_default();
        let fm       = parse_frontmatter(&content);
        let rel      = f.strip_prefix(vault).unwrap_or(&f).to_string_lossy().to_string();
        let title    = f.file_stem().unwrap_or_default().to_string_lossy().to_string();
        for key in &["date", "due", "created", "scheduled", "deadline"] {
            if let Some(v) = fm.get(*key) {
                let date_str = match v {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    _ => continue,
                };
                events.push(json!({ "title": title, "date": date_str,
                                    "path": rel, "type": key }));
            }
        }
    }
    events.sort_by(|a, b| a["date"].as_str().unwrap_or("").cmp(b["date"].as_str().unwrap_or("")));
    Ok(json!(events))
}

fn p_get_properties(vault: &Path, path: &str) -> Result<Value, String> {
    let abs = resolve(vault, path);
    let content = fs::read_to_string(&abs)
        .map_err(|e| format!("cannot read {}: {}", abs.display(), e))?;
    Ok(parse_frontmatter(&content))
}

fn p_get_outline(vault: &Path, path: &str) -> Result<Value, String> {
    let abs = resolve(vault, path);
    let content = fs::read_to_string(&abs)
        .map_err(|e| format!("cannot read {}: {}", abs.display(), e))?;
    let heading_re = Regex::new(r"^(#{1,6})\s+(.+)$").unwrap();
    let items: Vec<Value> = content.lines().filter_map(|line| {
        heading_re.captures(line).map(|cap| {
            let level = cap[1].len() as u8;
            let text  = cap[2].trim().to_string();
            let anchor: String = text.to_lowercase().chars()
                .map(|c| if c == ' ' { '-' } else { c })
                .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .collect();
            json!({ "level": level, "text": text, "anchor": anchor })
        })
    }).collect();
    Ok(json!(items))
}

fn p_get_subnotes(vault: &Path, path: &str) -> Result<Value, String> {
    let store = vault.join(".clin").join("subnotes.json");
    if store.exists() {
        let raw = fs::read_to_string(&store).map_err(|e| e.to_string())?;
        let map: HashMap<String, Vec<String>> = serde_json::from_str(&raw).unwrap_or_default();
        return Ok(json!(map.get(path).cloned().unwrap_or_default()));
    }
    Ok(json!([]))
}

fn p_get_bases(vault: &Path) -> Result<Value, String> {
    let bases: Vec<String> = WalkDir::new(vault).follow_links(true).into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file()
            && e.path().extension().and_then(|x| x.to_str()) == Some("base"))
        .map(|e| e.path().strip_prefix(vault).unwrap_or(e.path())
            .to_string_lossy().to_string())
        .collect();
    Ok(json!(bases))
}

fn p_get_base(vault: &Path, path: &str) -> Result<Value, String> {
    let abs = resolve(vault, path);
    let content = fs::read_to_string(&abs)
        .map_err(|e| format!("cannot read base {}: {}", abs.display(), e))?;
    Ok(json!(content))
}

fn p_get_canvas_files(vault: &Path) -> Result<Value, String> {
    let files: Vec<String> = WalkDir::new(vault).follow_links(true).into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file()
            && e.path().extension().and_then(|x| x.to_str()) == Some("canvas"))
        .map(|e| e.path().strip_prefix(vault).unwrap_or(e.path())
            .to_string_lossy().to_string())
        .collect();
    Ok(json!(files))
}

fn p_get_canvas(vault: &Path, path: &str) -> Result<Value, String> {
    let abs = resolve(vault, path);
    let raw = fs::read_to_string(&abs)
        .map_err(|e| format!("cannot read canvas {}: {}", abs.display(), e))?;
    serde_json::from_str(&raw).map_err(|e| format!("invalid canvas JSON: {}", e))
}

fn p_get_themes() -> Result<Value, String> {
    Ok(json!(["default","dark","light","nord","dracula","solarized-dark","solarized-light",
        "monokai","tokyo-night","catppuccin-mocha","catppuccin-latte","gruvbox-dark",
        "gruvbox-light","rose-pine","one-dark","ayu-dark","ayu-light","material","github"]))
}

fn p_get_vaults(vault: &Path) -> Result<Value, String> {
    let config = read_config();
    let mut vaults: Vec<Value> = config.get("vaults")
        .and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let current_str = vault.to_string_lossy().to_string();
    if !vaults.iter().any(|v| v.get("path").and_then(|p| p.as_str()) == Some(&current_str)) {
        vaults.insert(0, json!({
            "path": current_str,
            "name": vault.file_name().unwrap_or_default().to_string_lossy()
        }));
    }
    Ok(json!(vaults))
}

fn p_get_templates(vault: &Path) -> Result<Value, String> {
    let mut results: Vec<String> = Vec::new();
    let vault_templates = vault.join("templates");
    if vault_templates.is_dir() {
        for entry in WalkDir::new(&vault_templates).max_depth(2).into_iter()
            .filter_map(|e| e.ok()).filter(|e| e.file_type().is_file())
        {
            results.push(entry.path().strip_prefix(vault).unwrap_or(entry.path())
                .to_string_lossy().to_string());
        }
    }
    let global_templates = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("clin").join("templates");
    if global_templates.is_dir() {
        for entry in WalkDir::new(&global_templates).max_depth(2).into_iter()
            .filter_map(|e| e.ok()).filter(|e| e.file_type().is_file())
        {
            results.push(entry.path().to_string_lossy().to_string());
        }
    }
    Ok(json!(results))
}

fn p_get_health(vault: &Path) -> Result<Value, String> {
    let files = collect_md_files(vault);
    let total = files.len();
    let mut all_links: Vec<(String, String)> = Vec::new();
    let mut defined: HashSet<String> = HashSet::new();
    let re = Regex::new(r"\[\[([^\]|]+)").unwrap();
    for f in &files {
        let rel  = f.strip_prefix(vault).unwrap_or(f).to_string_lossy().replace('\\', "/");
        let stem = f.file_stem().unwrap_or_default().to_string_lossy().to_string();
        defined.insert(stem);
        if let Ok(content) = fs::read_to_string(f) {
            for cap in re.captures_iter(&content) {
                all_links.push((rel.clone(), cap[1].trim().to_string()));
            }
        }
    }
    let broken: Vec<Value> = all_links.iter()
        .filter(|(_, tgt)| {
            let stem = tgt.split('/').last().unwrap_or(tgt).trim_end_matches(".md");
            !defined.contains(stem)
        })
        .map(|(src, tgt)| json!({ "kind": "broken_link", "note": src, "target": tgt }))
        .collect();
    let mut has_inbound: HashSet<String> = HashSet::new();
    for (_, tgt) in &all_links {
        has_inbound.insert(
            tgt.split('/').last().unwrap_or(tgt).trim_end_matches(".md").to_string()
        );
    }
    let orphans: Vec<Value> = files.iter()
        .filter(|f| {
            !has_inbound.contains(&f.file_stem().unwrap_or_default().to_string_lossy().to_string())
        })
        .map(|f| {
            let rel = f.strip_prefix(vault).unwrap_or(f).to_string_lossy().replace('\\', "/");
            json!({ "kind": "orphan", "note": rel, "detail": "no inbound links" })
        })
        .collect();
    let mut issues = broken.clone();
    issues.extend(orphans.iter().cloned());
    Ok(json!({
        "total_notes": total,
        "broken_links": broken.len(),
        "orphaned_notes": orphans.len(),
        "total_edges": all_links.len(),
        "issues": issues,
    }))
}

fn p_get_settings() -> Result<Value, String> {
    let cfg = read_config();
    Ok(json!({
        "theme":            cfg.get("theme").and_then(|v| v.as_str()).unwrap_or("default"),
        "vault":            cfg.get("vault").and_then(|v| v.as_str()).unwrap_or("~/vault"),
        "graph_physics":    cfg.get("graph_physics").and_then(|v| v.as_bool()).unwrap_or(true),
        "auto_save":        cfg.get("auto_save").and_then(|v| v.as_bool()).unwrap_or(true),
        "spell_check":      cfg.get("spell_check").and_then(|v| v.as_bool()).unwrap_or(false),
        "editor_font_size": cfg.get("editor_font_size").and_then(|v| v.as_i64()).unwrap_or(14),
    }))
}

fn p_get_recent_vaults_p() -> Result<Value, String> {
    let cfg = read_config();
    Ok(cfg.get("recent_vaults").cloned().unwrap_or(json!([])))
}

fn p_search_notes(vault: &Path, q: &str) -> Result<Value, String> {
    let query = q.to_lowercase();
    let mut results: Vec<Value> = Vec::new();
    for entry in collect_md_files(vault) {
        if let Ok(content) = fs::read_to_string(&entry) {
            let lower = content.to_lowercase();
            if lower.contains(&query) {
                let idx   = lower.find(&query).unwrap_or(0);
                let start = idx.saturating_sub(60);
                let end   = (idx + query.len() + 60).min(content.len());
                let snippet = content[start..end].replace('\n', " ");
                let rel   = entry.strip_prefix(vault).unwrap_or(&entry);
                let title = entry.file_stem().unwrap_or_default()
                    .to_string_lossy().to_string().replace('-', " ").replace('_', " ");
                results.push(json!({
                    "path": rel.to_string_lossy(),
                    "title": title,
                    "snippet": format!("…{}…", snippet),
                }));
            }
        }
    }
    Ok(json!({ "results": results }))
}

fn p_get_smart_folder(vault: &Path, name: &str) -> Result<Value, String> {
    let now      = chrono::Utc::now();
    let today    = now.format("%Y-%m-%d").to_string();
    let week_ago = (now - chrono::Duration::days(7)).format("%Y-%m-%d").to_string();
    let pinned_p = vault.join(".clin").join("pinned.json");
    let pinned: HashSet<String> = fs::read_to_string(&pinned_p).ok()
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .unwrap_or_default().into_iter().collect();
    let mut results: Vec<Value> = Vec::new();
    for entry in collect_md_files(vault) {
        let rel = entry.strip_prefix(vault).unwrap_or(&entry)
            .to_string_lossy().replace('\\', "/");
        let meta     = entry.metadata().map_err(|e| e.to_string())?;
        let modified = meta.modified().ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| {
                chrono::DateTime::from_timestamp(d.as_secs() as i64, 0)
                    .map(|dt| dt.format("%Y-%m-%d").to_string())
                    .unwrap_or_default()
            }).unwrap_or_default();
        let include = match name {
            "today"     => modified == today,
            "this-week" => modified >= week_ago,
            "pinned"    => pinned.contains(&rel),
            "untagged"  => {
                let content = fs::read_to_string(&entry).unwrap_or_default();
                let m       = Matter::<YAML>::new();
                let parsed  = m.parse(&content);
                let tags_count = parsed.data.as_ref()
                    .and_then(|d| d.deserialize::<Value>().ok())
                    .and_then(|v| v.get("tags").and_then(|t| t.as_array()).map(|a| a.len()))
                    .unwrap_or(0);
                tags_count == 0
            }
            _ => false,
        };
        if include {
            let title = entry.file_stem().unwrap_or_default().to_string_lossy().to_string();
            results.push(json!({
                "id":       rel,
                "title":    title,
                "modified": modified,
                "pinned":   pinned.contains(&rel),
            }));
        }
    }
    Ok(json!(results))
}

// ── POST / DELETE implementations ────────────────────────────────────────────

fn p_open_file(vault: &Path, path: &str) -> Result<Value, String> {
    let abs = resolve(vault, path);
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(&abs).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(&abs).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/c", "start", "", &abs.to_string_lossy().to_string()]).spawn();
    Ok(json!({ "ok": true }))
}

fn p_save_file(vault: &Path, body: &Value) -> Result<Value, String> {
    let path    = body["path"].as_str().ok_or("missing path")?;
    let content = body["content"].as_str().ok_or("missing content")?;
    let abs     = resolve(vault, path);
    fs::write(&abs, content).map_err(|e| format!("cannot write {}: {}", abs.display(), e))?;
    Ok(json!({ "ok": true }))
}

fn p_create_file(vault: &Path, body: &Value) -> Result<Value, String> {
    let path = body["path"].as_str().ok_or("missing path")?;
    let abs  = resolve(vault, path);
    if let Some(parent) = abs.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = if let Some(c) = body["content"].as_str() {
        c.to_string()
    } else if let Some(tmpl) = body["template"].as_str() {
        fs::read_to_string(resolve(vault, tmpl)).unwrap_or_default()
    } else {
        let title = abs.file_stem().unwrap_or_default().to_string_lossy().to_string();
        let now   = chrono::Utc::now().format("%Y-%m-%d").to_string();
        format!("---\ntitle: {title}\ndate: {now}\ntags: []\n---\n\n# {title}\n")
    };
    fs::write(&abs, content).map_err(|e| format!("cannot write {}: {}", abs.display(), e))?;
    Ok(json!({ "ok": true, "path": path }))
}

fn p_move_file(vault: &Path, body: &Value) -> Result<Value, String> {
    let from = body["from"].as_str().ok_or("missing from")?;
    let to   = body["to"].as_str().ok_or("missing to")?;
    let src  = resolve(vault, from);
    let dst  = resolve(vault, to);
    if let Some(p) = dst.parent() {
        fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    fs::rename(&src, &dst)
        .map_err(|e| format!("cannot move {} → {}: {}", src.display(), dst.display(), e))?;
    if body["update_links"].as_bool().unwrap_or(true) {
        let old_stem = src.file_stem().unwrap_or_default().to_string_lossy().to_string();
        let new_stem = dst.file_stem().unwrap_or_default().to_string_lossy().to_string();
        if old_stem != new_stem {
            for f in collect_md_files(vault) {
                if let Ok(c) = fs::read_to_string(&f) {
                    let u = rewrite_wikilinks(&c, &old_stem, &new_stem);
                    if u != c { let _ = fs::write(&f, u); }
                }
            }
        }
    }
    Ok(json!({ "ok": true, "from": from, "to": to }))
}

fn p_duplicate_file(vault: &Path, body: &Value) -> Result<Value, String> {
    let path = body["path"].as_str().ok_or("missing path")?;
    let src  = resolve(vault, path);
    let dest_rel = body["new_path"].as_str().map(|s| s.to_string()).unwrap_or_else(|| {
        let stem   = src.file_stem().unwrap_or_default().to_string_lossy();
        let ext    = src.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
        let parent = src.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        if parent.is_empty() { format!("{stem}-copy{ext}") }
        else { format!("{parent}/{stem}-copy{ext}") }
    });
    let dst = resolve(vault, &dest_rel);
    if let Some(p) = dst.parent() {
        fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    fs::copy(&src, &dst)
        .map_err(|e| format!("cannot duplicate {}: {}", src.display(), e))?;
    Ok(json!({ "ok": true, "path": dest_rel }))
}

fn p_delete_file(vault: &Path, body: &Value) -> Result<Value, String> {
    let path = body["path"].as_str().ok_or("missing path")?;
    let abs  = resolve(vault, path);
    fs::remove_file(&abs).map_err(|e| format!("cannot delete {}: {}", abs.display(), e))?;
    Ok(json!({ "ok": true }))
}

fn p_create_folder(vault: &Path, body: &Value) -> Result<Value, String> {
    let path = body["path"].as_str().ok_or("missing path")?;
    let abs  = resolve(vault, path);
    fs::create_dir_all(&abs)
        .map_err(|e| format!("cannot mkdir {}: {}", abs.display(), e))?;
    Ok(json!({ "ok": true, "path": path }))
}

fn p_delete_folder(vault: &Path, body: &Value) -> Result<Value, String> {
    let path = body["path"].as_str().ok_or("missing path")?;
    let abs  = resolve(vault, path);
    fs::remove_dir_all(&abs)
        .map_err(|e| format!("cannot rmdir {}: {}", abs.display(), e))?;
    Ok(json!({ "ok": true }))
}

fn p_rename_file(vault: &Path, body: &Value) -> Result<Value, String> {
    let old_path = body["old_path"].as_str().ok_or("missing old_path")?;
    let new_path = body["new_path"].as_str().ok_or("missing new_path")?;
    let src = resolve(vault, old_path);
    let dst = resolve(vault, new_path);
    if let Some(p) = dst.parent() {
        fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    fs::rename(&src, &dst).map_err(|e| format!("cannot rename: {}", e))?;
    if body["rename_wikilinks"].as_bool().unwrap_or(true) {
        let old_stem = src.file_stem().unwrap_or_default().to_string_lossy().to_string();
        let new_stem = dst.file_stem().unwrap_or_default().to_string_lossy().to_string();
        if old_stem != new_stem {
            for f in collect_md_files(vault) {
                if let Ok(c) = fs::read_to_string(&f) {
                    let u = rewrite_wikilinks(&c, &old_stem, &new_stem);
                    if u != c { let _ = fs::write(&f, u); }
                }
            }
        }
    }
    Ok(json!({ "ok": true, "renamed": true }))
}

fn p_create_link(vault: &Path, body: &Value) -> Result<Value, String> {
    let source = body["source"].as_str()
        .or_else(|| body["from"].as_str())
        .ok_or("missing source/from")?;
    let target = body["target"].as_str()
        .or_else(|| body["to_title"].as_str())
        .ok_or("missing target/to_title")?;
    let abs  = resolve(vault, source);
    let mut content = fs::read_to_string(&abs)
        .map_err(|e| format!("cannot read {}: {}", abs.display(), e))?;
    let stem = Path::new(target).file_stem()
        .unwrap_or_else(|| std::ffi::OsStr::new(target))
        .to_string_lossy().to_string();
    let link = format!("[[{stem}]]");
    if !content.contains(&link) {
        content.push('\n');
        content.push_str(&link);
        content.push('\n');
        fs::write(&abs, content)
            .map_err(|e| format!("cannot write {}: {}", abs.display(), e))?;
    }
    Ok(json!({ "ok": true, "link": link }))
}

fn p_delete_link(vault: &Path, body: &Value) -> Result<Value, String> {
    let source = body["source"].as_str()
        .or_else(|| body["from"].as_str())
        .ok_or("missing source/from")?;
    let target = body["target"].as_str()
        .or_else(|| body["to_title"].as_str())
        .ok_or("missing target/to_title")?;
    let abs = resolve(vault, source);
    let content = fs::read_to_string(&abs)
        .map_err(|e| format!("cannot read {}: {}", abs.display(), e))?;
    let stem = Path::new(target).file_stem()
        .unwrap_or_else(|| std::ffi::OsStr::new(target))
        .to_string_lossy().to_string();
    let link_re = Regex::new(&format!(
        r"\[\[{}(?:[|#][^\[\]]*)?\]\]", regex::escape(&stem)
    )).unwrap();
    let updated = link_re.replace_all(&content, "").to_string();
    if updated != content {
        fs::write(&abs, updated)
            .map_err(|e| format!("cannot write {}: {}", abs.display(), e))?;
    }
    Ok(json!({ "ok": true }))
}

fn p_pin_note(vault: &Path, body: &Value) -> Result<Value, String> {
    let path = body["path"].as_str().ok_or("missing path")?;
    let p    = pinned_path(vault);
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut list: Vec<String> = if p.exists() {
        serde_json::from_str(&fs::read_to_string(&p).unwrap_or_default()).unwrap_or_default()
    } else {
        vec![]
    };
    let already  = list.contains(&path.to_string());
    let want_pin = body["pinned"].as_bool().unwrap_or(!already);
    if want_pin && !already  { list.push(path.to_string()); }
    if !want_pin && already  { list.retain(|x| x != path); }
    fs::write(&p, serde_json::to_string_pretty(&list).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    Ok(json!({ "ok": true, "pinned": want_pin, "path": path }))
}

fn p_save_properties(vault: &Path, body: &Value) -> Result<Value, String> {
    let path       = body["path"].as_str().ok_or("missing path")?;
    let properties = &body["properties"];
    let abs        = resolve(vault, path);
    let content    = fs::read_to_string(&abs)
        .map_err(|e| format!("cannot read {}: {}", abs.display(), e))?;
    let (_, body_text) = split_frontmatter(&content);
    let mut fm = String::from("---\n");
    if let Some(obj) = properties.as_object() {
        for (k, v) in obj {
            match v {
                Value::String(s) => fm.push_str(&format!("{k}: {s}\n")),
                Value::Array(arr) => {
                    let items: Vec<String> = arr.iter()
                        .map(|x| x.as_str().unwrap_or_default().to_string()).collect();
                    fm.push_str(&format!("{k}: [{}]\n", items.join(", ")));
                }
                _ => fm.push_str(&format!("{k}: {v}\n")),
            }
        }
    }
    fm.push_str("---\n");
    fm.push_str(body_text);
    fs::write(&abs, fm).map_err(|e| format!("cannot write: {}", e))?;
    Ok(json!({ "ok": true }))
}

fn p_set_theme(body: &Value) -> Result<Value, String> {
    let theme  = body["theme"].as_str().ok_or("missing theme")?;
    let mut cfg = read_config();
    cfg["theme"] = json!(theme);
    write_config(&cfg)?;
    Ok(json!({ "ok": true, "theme": theme }))
}

fn p_set_vault(body: &Value) -> Result<Value, String> {
    let path      = body["path"].as_str().ok_or("missing path")?;
    let new_vault = PathBuf::from(path);
    if !new_vault.exists() {
        return Err(format!("vault path does not exist: {}", path));
    }
    let mut cfg = read_config();
    cfg["active_vault"] = json!(path);
    let vault_entry = json!({
        "path": path,
        "name": new_vault.file_name().unwrap_or_default().to_string_lossy()
    });
    let mut recents: Vec<Value> = cfg.get("recent_vaults")
        .and_then(|v| v.as_array()).cloned().unwrap_or_default();
    recents.retain(|v| v.get("path").and_then(|p| p.as_str()) != Some(path));
    recents.insert(0, vault_entry);
    recents.truncate(10);
    cfg["recent_vaults"] = json!(recents);
    write_config(&cfg)?;
    Ok(json!({ "ok": true, "path": path }))
}

fn p_set_settings(body: &Value) -> Result<Value, String> {
    let mut cfg = read_config();
    if let Some(obj) = body.as_object() {
        for (k, v) in obj { cfg[k] = v.clone(); }
    }
    write_config(&cfg)?;
    Ok(json!({ "ok": true }))
}
