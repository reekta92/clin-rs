#![cfg_attr(test, allow(clippy::unwrap_used))]
pub mod actions;
pub mod app_theme;
pub mod backup {
    pub mod app;
    pub mod git_ops;
    pub mod input;
    pub mod render;
    pub mod state;
    pub mod worker;
}
pub mod calendar;
pub mod cli;
pub mod config;
pub mod console;
pub mod draw {
    pub mod app;
    pub mod geometry;
    pub mod input;
    pub mod render;
    pub mod state;
}
pub mod editor;
pub(crate) mod editor_document;
pub(crate) mod editor_session;
pub mod event_source;
pub mod frontmatter;
pub mod fsutil;
pub mod goals;
pub mod graf_adapter;
pub mod image_render {
    pub mod cache;
    pub mod worker;

    /// Settle duration (150 ms) after the last zoom/scroll event before the
    /// view is considered settled and real pixel images resume rendering.
    pub const TRANSFORM_SETTLE: std::time::Duration = std::time::Duration::from_millis(150);
}
pub mod keybinds;
pub mod list_view;
pub mod local_state;
pub mod markdown;
pub mod migration;
pub mod note_index;
pub mod outline {
    pub mod app;
    pub mod input;
    pub mod parse;
    pub mod render;
    pub mod state;
}
pub mod overlay;
pub mod palette;
pub mod paths;
#[cfg(test)]
pub mod perf_tests;
pub mod pinstar;
pub mod popups;
pub mod preview;
pub mod session;
pub mod setup;
pub mod snapshot;
pub mod statusline;
pub mod templates;
pub mod text_edit;
pub mod todo;

use crate::cli::{
    CacheCmd, Cli, Command, ConfigCmd, KeybindsCmd, NotesCmd, StorageCmd, TemplatesCmd,
};
use crate::config::ClinConfig;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::overlay::{OverlayState, OverlayView, ViewKind};
use clap::{CommandFactory, FromArgMatches};

use std::fs;
use std::io::{self, Write};
use std::process;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use uuid::Uuid;

pub(crate) static SHOULD_EXIT: LazyLock<Arc<AtomicBool>> =
    LazyLock::new(|| Arc::new(AtomicBool::new(false)));
pub(crate) static SIGNAL_COUNT: AtomicU32 = AtomicU32::new(0);
pub(crate) static FORCE_QUIT: AtomicBool = AtomicBool::new(false);

use anyhow::{Context, Result};
use crossterm::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture, Event,
    KeyCode, KeyEventKind, KeyModifiers,
};
#[cfg(not(windows))]
use crossterm::event::{
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::layout::Rect;

pub mod app;
pub mod events;
pub mod storage;
pub mod ui;
use app::*;
use events::*;
use storage::*;
pub fn run() -> Result<()> {
    // Panic hook: restore terminal before abort
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        disable_raw_mode().ok();
        #[cfg(not(windows))]
        let _ = execute!(io::stderr(), PopKeyboardEnhancementFlags);
        let _ = execute!(
            io::stderr(),
            LeaveAlternateScreen,
            DisableMouseCapture,
            DisableBracketedPaste,
            crossterm::cursor::Show,
        );
        prev(panic_info);
    }));

    let matches = Cli::command()
        .styles(crate::console::CLAP_STYLES)
        .get_matches();
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());

    if let Some(path) = &cli.config {
        crate::config::set_config_path_override(path.clone());
    }
    if let Some(v) = &cli.vault {
        let expanded = crate::config::expand_path(&v.to_string_lossy());
        crate::config::set_storage_path_override(expanded);
    }

    match cli.command {
        None => launch_tui(None, cli.setup),
        Some(Command::Notes { action }) => run_notes(action),
        Some(Command::Storage { action }) => run_storage(action),
        Some(Command::Keybinds { action }) => run_keybinds(action),
        Some(Command::Templates { action }) => run_templates(action),
        Some(Command::Config { action }) => run_config(action),
        Some(Command::Cache { action }) => run_cache(action),
    }
}
fn launch_tui(open_title: Option<String>, force_setup: bool) -> Result<()> {
    let mut app = crate::session::bootstrap_app(open_title, force_setup)?;
    run_tui_session(&mut app)
}

fn run_notes(action: NotesCmd) -> Result<()> {
    match action {
        NotesCmd::List => {
            let (storage, _) = Storage::init();
            let storage = storage?;
            let app = App::new(storage)?;
            for (index, note) in app.notes.iter().enumerate() {
                println!(
                    "{} {}",
                    console::dim(&format!("{}.", index + 1)),
                    note.title
                );
            }
            Ok(())
        }
        NotesCmd::New {
            template,
            body,
            no_tui,
            title,
        } => {
            let (storage, _) = Storage::init();
            let storage = storage?;
            let mut app = App::new(storage)?;

            let final_title = title.unwrap_or_else(|| "New Note".to_string());

            let (content, tags) = if let Some(tmpl_name) = template {
                let template_manager = app.storage.template_manager();
                if let Ok(templates) = template_manager.list() {
                    if let Some(template_summary) =
                        templates.into_iter().find(|t| t.name == tmpl_name)
                    {
                        if let Ok(template_data) = template_manager.load(&template_summary.filename)
                        {
                            (template_data.content.template.clone(), Vec::new())
                        } else {
                            eprintln!(
                                "{}",
                                console::error(&format!(
                                    "Failed to load template data: {tmpl_name}"
                                ))
                            );
                            process::exit(1);
                        }
                    } else {
                        eprintln!(
                            "{}",
                            console::error(&format!("Template not found: {tmpl_name}"))
                        );
                        process::exit(1);
                    }
                } else {
                    (String::new(), Vec::new())
                }
            } else {
                (String::new(), Vec::new())
            };
            // --body overrides template content when both are given.
            // Capture presence before `body` is moved by the `if let` below.
            let has_body = body.is_some();
            let content = if let Some(b) = body { b } else { content };

            let id = Uuid::new_v4().simple().to_string();
            let note = Note {
                title: final_title.clone(),
                content,
                updated_at: crate::ui::now_unix_secs(),
                tags,
            };

            let saved_id = app.storage.save_note(&id, &note)?;

            if no_tui || has_body {
                println!(
                    "{}",
                    console::success(&format!("Created note: {}", console::bold(&final_title)))
                );
                return Ok(());
            }

            app.editor.editing_id = Some(saved_id.clone());
            app.refresh_note_single(None, &saved_id);
            app.load_and_open_note(&saved_id, None);
            run_tui_session(&mut app)
        }
        NotesCmd::Open { title } => launch_tui(Some(title), false),
        NotesCmd::Cat { title } => {
            let (storage, _) = Storage::init();
            let storage = storage?;
            let app = App::new(storage)?;
            let id = app
                .notes
                .iter()
                .find(|n| n.title.eq_ignore_ascii_case(title.trim()))
                .map(|n| n.id.clone());
            match id {
                Some(id) => match app.storage.load_note(&id) {
                    Ok(note) => {
                        println!("{}", note.content);
                        Ok(())
                    }
                    Err(e) => {
                        eprintln!("{}", console::error(&format!("Failed to load note: {e}")));
                        process::exit(1);
                    }
                },
                None => {
                    eprintln!(
                        "{}",
                        console::error(&format!("No note found with title: {title}"))
                    );
                    process::exit(1);
                }
            }
        }
        NotesCmd::Quick { content, title } => {
            let (storage, _) = Storage::init();
            let mut storage = storage?;

            let id = Uuid::new_v4().simple().to_string();
            let final_title = title.unwrap_or_else(|| "Quick Note".to_string());
            let note = Note {
                title: final_title.clone(),
                content,
                updated_at: crate::ui::now_unix_secs(),
                tags: Vec::new(),
            };

            let _saved_id = storage.save_note(&id, &note)?;

            println!(
                "{}",
                console::success(&format!("Created note: {}", console::bold(&final_title)))
            );

            Ok(())
        }
        NotesCmd::Search { query } => {
            use fuzzy_matcher::FuzzyMatcher;
            use fuzzy_matcher::skim::SkimMatcherV2;

            let (storage, _) = Storage::init();
            let storage = storage?;
            let app = App::new(storage)?;
            let matcher = SkimMatcherV2::default();
            let mut hits: Vec<(i64, String, String)> = Vec::new(); // (score, title, folder)
            for note in &app.notes {
                let mut best: Option<i64> = matcher.fuzzy_match(&note.title, &query);
                // content match (substring) as a fallback when the title does not match
                if best.is_none()
                    && let Ok(full) = app.storage.load_note(&note.id)
                    && full.content.contains(&query)
                {
                    best = Some(0); // content hit, low rank
                }
                if let Some(score) = best {
                    hits.push((score, note.title.clone(), note.folder.clone()));
                }
            }
            hits.sort_by_key(|b| std::cmp::Reverse(b.0));
            if hits.is_empty() {
                println!(
                    "{}",
                    console::info(&format!("No notes matched \"{query}\"."))
                );
            } else {
                for (_, title, folder) in hits {
                    if folder.is_empty() {
                        println!("{}", console::bold(&title));
                    } else {
                        println!(
                            "{}  {}",
                            console::bold(&title),
                            console::dim(&format!("[{folder}]"))
                        );
                    }
                }
            }
            Ok(())
        }
    }
}

fn prompt_migrate_default(
    to: &std::path::Path,
    info_msg: &str,
) -> Result<Option<std::path::PathBuf>> {
    let default = ClinConfig::default_storage_path()?;
    if default.exists() && default.is_dir() && default.as_path() != to {
        println!("{}", console::info(info_msg));
        println!(
            "Found data at default location: {}",
            console::path(&default)
        );
        print!("{}", console::warning("Migrate from there? [y/N]: "));
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if input.trim().eq_ignore_ascii_case("y") {
            Ok(Some(default))
        } else {
            println!("{}", console::warning("Migration cancelled."));
            Ok(None)
        }
    } else {
        anyhow::bail!("No previous storage location found. Nothing to migrate.");
    }
}

fn run_storage(action: StorageCmd) -> Result<()> {
    match action {
        StorageCmd::Show => {
            let bootstrap = ClinConfig::load().0?;
            let effective = bootstrap.effective_storage_path()?;
            println!(
                "{} {}",
                console::bold("Storage path:"),
                console::path(&effective)
            );
            if bootstrap.has_custom_storage_path() {
                println!("{}", console::yellow("(custom path)"));
            } else {
                println!("{}", console::dim("(default path)"));
            }
            Ok(())
        }
        StorageCmd::Set { path } => {
            let mut bootstrap = ClinConfig::load().0?;
            let path = crate::config::expand_path(&path.to_string_lossy());
            let old_path = bootstrap.effective_storage_path()?;

            if !path.is_absolute() {
                anyhow::bail!("Storage path must be absolute: {}", path.display());
            }

            fs::create_dir_all(&path)
                .with_context(|| format!("failed to create directory: {}", path.display()))?;

            // Record storage migration in state.json (not in config.toml)
            if old_path.exists()
                && old_path != path
                && let Ok(paths) = crate::paths::AppPaths::discover(ClinConfig::config_path()?)
            {
                let _ = crate::local_state::record_storage_migration(
                    &paths.state_path(),
                    &old_path,
                    &path,
                );
            }

            bootstrap.set_storage_path(path.clone());
            bootstrap.save()?;

            println!(
                "{}",
                console::success(&format!("Storage path set to: {}", console::path(&path)))
            );

            // Check for migration hint
            if let Ok(paths) = crate::paths::AppPaths::discover(ClinConfig::config_path()?)
                && let Ok(state) = crate::local_state::LocalState::load(&paths.state_path())
                && state.storage_migration.is_some()
            {
                println!();
                println!(
                    "{}",
                    console::hint("Run 'clin storage migrate' to migrate your existing data.")
                );
            }

            Ok(())
        }
        StorageCmd::Reset => {
            let mut bootstrap = ClinConfig::load().0?;
            let old_path = bootstrap.effective_storage_path()?;
            let default = ClinConfig::default_storage_path()?;

            // Record migration before resetting
            if old_path != default
                && let Ok(paths) = crate::paths::AppPaths::discover(ClinConfig::config_path()?)
            {
                let _ = crate::local_state::record_storage_migration(
                    &paths.state_path(),
                    &old_path,
                    &default,
                );
            }

            bootstrap.reset_storage_path();
            bootstrap.save()?;
            println!(
                "{}",
                console::success(&format!(
                    "Storage path reset to default: {}",
                    console::path(&default)
                ))
            );
            Ok(())
        }
        StorageCmd::Migrate => {
            let bootstrap = ClinConfig::load().0?;
            let to = bootstrap.effective_storage_path()?;

            // Read storage migration from state.json
            let from = if let Ok(paths) =
                crate::paths::AppPaths::discover(ClinConfig::config_path()?)
            {
                if let Ok(state) = crate::local_state::LocalState::load(&paths.state_path()) {
                    if let Some(ref m) = state.storage_migration {
                        let prev = m.previous_path.clone();
                        if prev.exists() && prev.is_dir() {
                            prev
                        } else {
                            match prompt_migrate_default(
                                &to,
                                "Recorded previous path does not exist.",
                            )? {
                                Some(p) => p,
                                None => return Ok(()),
                            }
                        }
                    } else {
                        match prompt_migrate_default(&to, "No previous storage path recorded.")? {
                            Some(p) => p,
                            None => return Ok(()),
                        }
                    }
                } else {
                    match prompt_migrate_default(&to, "No previous storage path recorded.")? {
                        Some(p) => p,
                        None => return Ok(()),
                    }
                }
            } else {
                match prompt_migrate_default(&to, "No previous storage path recorded.")? {
                    Some(p) => p,
                    None => return Ok(()),
                }
            };

            if from == to {
                anyhow::bail!("Source and destination are the same. Nothing to migrate.");
            }

            println!("{}", console::bold("Migrating data:"));
            println!("  {} {}", console::dim("From:"), console::path(&from));
            println!("  {} {}", console::dim("To:"), console::path(&to));
            println!();

            fs::create_dir_all(&to)
                .with_context(|| format!("failed to create destination: {}", to.display()))?;

            let mut migrated_count = 0;
            let mut skipped_count = 0;
            let mut conflict_action: Option<migration::ConflictAction> = None;

            let source_is_vault = is_existing_vault(&from);

            // Determine effective source dirs (vault mode: notes at root, .clin/templates/)
            let notes_src = if source_is_vault {
                from.clone()
            } else {
                from.join("notes")
            };
            let templates_src = if source_is_vault {
                from.join(".clin").join("templates")
            } else {
                from.join("templates")
            };

            // Determine effective target dirs (always vault layout now)
            let notes_dst = to.clone();
            let templates_dst = to.join(".clin").join("templates");

            // Migrate notes
            if notes_src.exists() && notes_src.is_dir() {
                fs::create_dir_all(&notes_dst)?;
                let (m, s, action) = if source_is_vault {
                    // Vault-mode source: only copy note files, skip hidden/clin dirs
                    migration::migrate_note_files_with_conflict(
                        &notes_src,
                        &notes_dst,
                        conflict_action,
                    )?
                } else {
                    migration::migrate_directory_with_conflict(
                        &notes_src,
                        &notes_dst,
                        conflict_action,
                    )?
                };
                migrated_count += m;
                skipped_count += s;
                conflict_action = action;
            }

            // Migrate templates
            if templates_src.exists() && templates_src.is_dir() {
                fs::create_dir_all(&templates_dst)?;
                let (m, s, _) = migration::migrate_directory_with_conflict(
                    &templates_src,
                    &templates_dst,
                    conflict_action,
                )?;
                migrated_count += m;
                skipped_count += s;
            }

            // Clear storage migration record after successful migration
            if let Ok(paths) = crate::paths::AppPaths::discover(ClinConfig::config_path()?) {
                let state_path = paths.state_path();
                let _ = crate::local_state::LocalState::update(&state_path, |s| {
                    s.storage_migration = None;
                    Ok(())
                });
            }

            println!();
            println!("{}", console::success("Migration complete!"));
            println!(
                "  {} {}",
                console::dim("Migrated:"),
                console::bold(&format!("{migrated_count} items"))
            );
            if skipped_count > 0 {
                println!(
                    "  {} {}",
                    console::dim("Skipped:"),
                    console::yellow(&format!("{skipped_count} items"))
                );
            }
            println!();
            println!("Your old data remains at: {}", console::path(&from));
            println!(
                "{}",
                console::dim("You may delete it manually after verifying everything works.")
            );

            Ok(())
        }
    }
}

fn run_keybinds(action: KeybindsCmd) -> Result<()> {
    match action {
        KeybindsCmd::Show => {
            let (storage, _) = Storage::init();
            let storage = storage?;
            let config = crate::config::ClinConfig::load().0.unwrap_or_default();
            println!(
                "{}",
                storage
                    .keybinds_path_for_preset(config.core.keybind_preset)
                    .display()
            );
            Ok(())
        }
        KeybindsCmd::Export => {
            let (storage, _) = Storage::init();
            let storage = storage?;
            let config = crate::config::ClinConfig::load().0.unwrap_or_default();
            let preset = config.core.keybind_preset;
            let (keybinds, _warnings) = storage.load_keybinds_with_preset(preset);
            let toml = keybinds.to_toml();
            let content = toml::to_string_pretty(&toml)?;
            println!("{content}");
            Ok(())
        }
        KeybindsCmd::Reset => {
            let (storage, _) = Storage::init();
            let storage = storage?;
            let config = crate::config::ClinConfig::load().0.unwrap_or_default();
            let preset = config.core.keybind_preset;
            let keybinds = preset.base_keybinds();
            storage.save_keybinds_for_preset(&keybinds, preset)?;
            println!("{}", console::success("Keybinds reset to defaults"));
            println!(
                "{} {}",
                console::dim("Keybinds file:"),
                console::path(storage.keybinds_path_for_preset(preset))
            );
            Ok(())
        }
    }
}

fn run_templates(action: TemplatesCmd) -> Result<()> {
    match action {
        TemplatesCmd::List => {
            let (storage, _) = Storage::init();
            let storage = storage?;
            let template_manager = storage.template_manager();
            let templates = template_manager.list()?;

            if templates.is_empty() {
                println!("{}", console::info("No templates found."));
                println!(
                    "Templates directory: {}",
                    console::path(&storage.templates_dir)
                );
                println!();
                println!(
                    "{}",
                    console::hint("Run 'clin templates init' to create example templates.")
                );
            } else {
                println!("{}\n", console::bold("Available templates:"));
                for (i, t) in templates.iter().enumerate() {
                    println!(
                        "  {} {} {}",
                        console::dim(&format!("{}.", i + 1)),
                        console::bold(&t.name),
                        console::dim(&format!("({})", t.filename))
                    );
                }
                println!(
                    "\nTemplates directory: {}",
                    console::path(&storage.templates_dir)
                );
            }
            Ok(())
        }
        TemplatesCmd::Init => {
            let (storage, _) = Storage::init();
            let storage = storage?;
            let template_manager = storage.template_manager();
            template_manager.create_examples()?;
            println!(
                "{}",
                console::success(&format!(
                    "Example templates created in: {}",
                    console::path(&storage.templates_dir)
                ))
            );

            let templates = template_manager.list()?;
            for t in templates {
                println!(
                    "  {} {} {}",
                    console::dim("-"),
                    console::bold(&t.name),
                    console::dim(&format!("({})", t.filename))
                );
            }
            Ok(())
        }
    }
}

fn run_config(action: ConfigCmd) -> Result<()> {
    match action {
        ConfigCmd::Show => {
            let path = ClinConfig::config_path()?;
            println!("{}", console::path(&path));
            Ok(())
        }
        ConfigCmd::Edit => {
            let path = ClinConfig::config_path()?;
            let editor = std::env::var("VISUAL")
                .or_else(|_| std::env::var("EDITOR"))
                .map_err(|_| {
                    anyhow::anyhow!(
                        "no editor set; define $VISUAL or $EDITOR (e.g. export EDITOR=nvim)"
                    )
                })?;
            std::process::Command::new(&editor)
                .arg(&path)
                .status()
                .with_context(|| format!("failed to launch editor: {editor}"))?;
            Ok(())
        }
        ConfigCmd::Reset => {
            let _ = ClinConfig::reset()?;
            println!(
                "{}",
                console::success("Configuration reset to default values.")
            );
            Ok(())
        }
    }
}

fn run_cache(action: CacheCmd) -> Result<()> {
    match action {
        CacheCmd::Reset => {
            let (storage, _) = Storage::init();
            let storage = storage?;
            let app_paths = crate::paths::AppPaths::discover(ClinConfig::config_path()?)?;
            let vault_id = crate::local_state::vault_identity_path(&storage.data_dir)?;
            let digest = crate::paths::vault_cache_digest(&vault_id);
            let scoped_cache = app_paths.scoped_summary_cache_path(digest);

            let mut cache_locations = vec![scoped_cache, app_paths.summary_cache_path()];
            cache_locations.push(app_paths.config_root_cache_path());
            let default_root = app_paths.default_config_root_cache_path();
            if !cache_locations.contains(&default_root) {
                cache_locations.push(default_root);
            }

            let mut any_removed = false;
            for path in &cache_locations {
                if crate::fsutil::remove_file_if_exists(path).with_context(|| {
                    format!("failed to remove note-summary cache: {}", path.display())
                })? {
                    any_removed = true;
                    println!(
                        "{}",
                        console::success(&format!(
                            "Note-summary cache cleared: {}",
                            console::path(path)
                        ))
                    );
                }
            }

            if !any_removed {
                println!(
                    "{}",
                    console::info(&format!(
                        "Note-summary cache already empty: {}",
                        console::path(&cache_locations[0])
                    ))
                );
            }
            Ok(())
        }
    }
}
fn process_watcher_events(app: &mut App) {
    let Some(ref rx) = app.fs_event_rx else {
        return;
    };
    let overflow = app.fs_overflow.swap(false, Ordering::SeqCst);

    let mut events = Vec::new();
    while let Ok(evt) = rx.try_recv() {
        events.push(evt);
    }

    if overflow {
        app.request_notes_reconcile();
        app.watcher_window_start = None;
        return;
    }

    if events.is_empty() && app.watcher_window_start.is_none() {
        return;
    }

    if app.watcher_window_start.is_none() {
        if let Some(first) = events.first() {
            app.watcher_window_start = Some(first.observed_at);
        } else {
            app.watcher_window_start = Some(Instant::now());
        }
    }

    let Some(window_start) = app.watcher_window_start else {
        return;
    };
    if Instant::now() < window_start + Duration::from_millis(250) {
        return;
    }

    app.watcher_window_start = None;

    let mut changes_map: HashMap<String, crate::app::catalog::PathChange> = HashMap::new();
    let mut needs_full_reconcile = false;

    'events_loop: for watched in events {
        use notify::EventKind;
        use notify::event::{ModifyKind, RenameMode};

        let ev = watched.event;
        if ev.paths.is_empty() {
            needs_full_reconcile = true;
            break 'events_loop;
        }

        match ev.kind {
            EventKind::Access(_) => continue,
            EventKind::Create(_)
            | EventKind::Modify(ModifyKind::Data(_))
            | EventKind::Modify(ModifyKind::Metadata(_)) => {
                for path in &ev.paths {
                    if path.is_dir() {
                        needs_full_reconcile = true;
                        break 'events_loop;
                    }
                    if let Ok(rel) = path.strip_prefix(&app.storage.notes_dir) {
                        if let Some(rel_str) = rel.to_str() {
                            let norm_id = rel_str.replace('\\', "/");
                            changes_map.insert(
                                norm_id.clone(),
                                crate::app::catalog::PathChange::Upsert(norm_id),
                            );
                        } else {
                            needs_full_reconcile = true;
                            break 'events_loop;
                        }
                    } else {
                        needs_full_reconcile = true;
                        break 'events_loop;
                    }
                }
            }
            EventKind::Remove(_) => {
                for path in &ev.paths {
                    if let Ok(rel) = path.strip_prefix(&app.storage.notes_dir) {
                        if let Some(rel_str) = rel.to_str() {
                            let norm_id = rel_str.replace('\\', "/");
                            changes_map.insert(
                                norm_id.clone(),
                                crate::app::catalog::PathChange::Remove(norm_id),
                            );
                        } else {
                            needs_full_reconcile = true;
                            break 'events_loop;
                        }
                    } else {
                        needs_full_reconcile = true;
                        break 'events_loop;
                    }
                }
            }
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)) if ev.paths.len() == 2 => {
                let old_p = &ev.paths[0];
                let new_p = &ev.paths[1];
                if let (Ok(rel_old), Ok(rel_new)) = (
                    old_p.strip_prefix(&app.storage.notes_dir),
                    new_p.strip_prefix(&app.storage.notes_dir),
                ) {
                    if let (Some(old_str), Some(new_str)) = (rel_old.to_str(), rel_new.to_str()) {
                        let old_norm = old_str.replace('\\', "/");
                        let new_norm = new_str.replace('\\', "/");
                        changes_map.insert(
                            old_norm.clone(),
                            crate::app::catalog::PathChange::Remove(old_norm),
                        );
                        changes_map.insert(
                            new_norm.clone(),
                            crate::app::catalog::PathChange::Upsert(new_norm),
                        );
                    } else {
                        needs_full_reconcile = true;
                        break 'events_loop;
                    }
                } else {
                    needs_full_reconcile = true;
                    break 'events_loop;
                }
            }
            _ => {
                needs_full_reconcile = true;
                break 'events_loop;
            }
        }
        if needs_full_reconcile || changes_map.len() > 512 {
            needs_full_reconcile = true;
            break;
        }
    }

    if needs_full_reconcile || changes_map.len() > 512 {
        app.request_notes_reconcile();
    } else if !changes_map.is_empty() {
        let changes: Vec<_> = changes_map.into_values().collect();
        app.send_catalog_paths(changes);
    }
}

fn perform_orderly_catalog_shutdown(app: &mut App) {
    app.catalog_generation.fetch_add(1, Ordering::SeqCst);
    while app.catalog_event_rx.try_recv().is_ok() {}

    let (ack_tx, ack_rx) = std::sync::mpsc::sync_channel(0);
    let deadline = Instant::now() + Duration::from_millis(500);

    while Instant::now() < deadline {
        if app
            .catalog_cmd_tx
            .try_send(crate::app::catalog::CatalogCommand::Flush {
                ack: ack_tx.clone(),
            })
            .is_ok()
            && ack_rx.recv_timeout(Duration::from_millis(100)).is_ok()
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let _ = app
        .catalog_cmd_tx
        .try_send(crate::app::catalog::CatalogCommand::Shutdown);
}
struct TerminalGuard;

impl TerminalGuard {
    fn enter(mouse_enabled: bool) -> Result<Self> {
        enable_raw_mode().context("failed to enable raw mode")?;
        let mut stdout = io::stdout();
        #[cfg(windows)]
        let entered = if mouse_enabled {
            execute!(
                stdout,
                EnterAlternateScreen,
                EnableMouseCapture,
                EnableBracketedPaste
            )
        } else {
            execute!(stdout, EnterAlternateScreen, EnableBracketedPaste)
        };
        #[cfg(not(windows))]
        let entered = if mouse_enabled {
            execute!(
                stdout,
                EnterAlternateScreen,
                EnableMouseCapture,
                EnableBracketedPaste,
                PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
            )
        } else {
            execute!(
                stdout,
                EnterAlternateScreen,
                EnableBracketedPaste,
                PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
            )
        };
        if let Err(e) = entered {
            #[cfg(not(windows))]
            let _ = execute!(stdout, PopKeyboardEnhancementFlags);
            let _ = execute!(
                stdout,
                LeaveAlternateScreen,
                DisableMouseCapture,
                DisableBracketedPaste
            );
            disable_raw_mode().ok(); // guard won't be built; clean up raw mode ourselves
            return Err(e).context("failed to enter alternate screen");
        }
        Ok(TerminalGuard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        disable_raw_mode().ok();
        #[cfg(not(windows))]
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
        let _ = execute!(
            io::stdout(),
            LeaveAlternateScreen,
            DisableMouseCapture,
            DisableBracketedPaste,
            crossterm::cursor::Show,
        );
    }
}

/// Immediately restore terminal and exit with code 130 (128 + SIGINT).
/// Bypasses graceful shutdown, saves, and confirm dialogs.
pub fn force_quit() -> ! {
    crossterm::terminal::disable_raw_mode().ok();
    #[cfg(not(windows))]
    let _ = crossterm::execute!(
        std::io::stdout(),
        crossterm::event::PopKeyboardEnhancementFlags
    );
    let _ = crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture,
        crossterm::event::DisableBracketedPaste,
        crossterm::cursor::Show,
    );
    std::process::exit(130);
}

fn run_tui_session(app: &mut App) -> Result<()> {
    loop {
        let guard = crate::session::start_session(app);
        let result = {
            let _terminal_guard = TerminalGuard::enter(app.mouse_enabled)?;
            let backend = ratatui::backend::CrosstermBackend::new(io::stdout());
            let mut terminal = Terminal::new(backend).context("failed to create terminal")?;
            app.image_picker = if app.config.image.enabled {
                Some(
                    ratatui_image::picker::Picker::from_query_stdio()
                        .unwrap_or_else(|_| ratatui_image::picker::Picker::halfblocks()),
                )
            } else {
                None
            };
            let mut terminal_safe = std::panic::AssertUnwindSafe(&mut terminal);
            let mut app_safe = std::panic::AssertUnwindSafe(&mut *app);
            let result = std::panic::catch_unwind(move || {
                run_app(
                    *terminal_safe,
                    *app_safe,
                    &mut crate::event_source::EventSource::Crossterm,
                )
            });
            if app.mode == ViewMode::Edit {
                let _ = app.autosave();
            }
            result
        };

        let rebootstrap = app.setup_rebootstrap.take();
        if let Some(request) = rebootstrap {
            crate::session::finish_session_for_rebootstrap(app, guard)?;
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => return Err(error),
                Err(error) => std::panic::resume_unwind(error),
            }
            let rebuilt = (|| -> Result<App> {
                let fresh = App::new_deferred(request.storage.clone())?;
                if request.previous_path.exists() && request.previous_path != request.selected_path
                {
                    let paths = crate::paths::AppPaths::discover(
                        crate::config::ClinConfig::config_path()?,
                    )?;
                    crate::local_state::record_storage_migration(
                        &paths.state_path(),
                        &request.previous_path,
                        &request.selected_path,
                    )?;
                }
                Ok(fresh)
            })();
            let mut fresh = match rebuilt {
                Ok(fresh) => fresh,
                Err(error) => {
                    request.previous_config.save().with_context(|| {
                        format!(
                            "failed to restore previous config after vault switch error: {error}"
                        )
                    })?;
                    let (storage, _) =
                        crate::storage::Storage::init_with_config(&request.previous_config);
                    let storage = storage.with_context(|| {
                        format!(
                            "failed to restore previous vault after vault switch error: {error}"
                        )
                    })?;
                    let mut restored = App::new_deferred(storage).with_context(|| {
                        format!("failed to reopen previous vault after vault switch error: {error}")
                    })?;
                    restored.open_setup_view();
                    if let Some(state) = restored.setup_state.as_mut() {
                        state.vault_path = request.selected_path.clone();
                    }
                    restored.set_temporary_status(&format!(
                        "Failed to switch vault; restored previous vault: {error}"
                    ));
                    *app = restored;
                    continue;
                }
            };
            for warning in request.warnings {
                fresh
                    .messages
                    .push(warning, crate::app::messages::MessageSeverity::Warning);
            }
            let _ = fresh.storage.template_manager().create_examples();
            fresh.set_temporary_status_static("Setup complete");
            *app = fresh;
            continue;
        }
        crate::session::finish_session(app, guard)?;
        match result {
            Ok(result) => return result,
            Err(error) => std::panic::resume_unwind(error),
        }
    }
}

/// Drain mouse events already sitting in the crossterm queue and feed each to the
/// overlay view, collapsing a burst of drag events into one later render.
/// Called only after the loop has already dispatched a `Drag` event, so during an
/// active pan the queue contains only further `Drag`/`Up` mouse events.
/// Non-mouse events break the loop (vanishingly rare during an active drag; the
/// single read event is dropped rather than re-queued because crossterm cannot
/// push back). Results from drained events are discarded — a mouse Drag/Up never
fn drain_queued_mouse_events<V: OverlayView>(
    view: &mut V,
    app: &mut App,
    term_area: Rect,
    events: &mut crate::event_source::EventSource,
) -> Result<()> {
    while events.poll(Duration::ZERO)? {
        match events.read()? {
            ev @ Event::Mouse(_) => {
                let _ = view.overlay_handle_event(ev, app, term_area)?;
            }
            _ => break,
        }
    }
    Ok(())
}

/// Apply an overlay's event outcome to the App, per view kind. Shared by the
/// key and mouse dispatch paths (single result-handling site).
fn finish_overlay_event(app: &mut App, kind: ViewKind, res: crate::overlay::OverlayResult) {
    use crate::overlay::OverlayResult;
    match res {
        OverlayResult::OpenHelp(tab) => {
            app.reload_theme();
            app.open_help_page_with_tab(tab);
        }
        other => match kind {
            ViewKind::Graph => match other {
                OverlayResult::NoteOpened(note_id) => {
                    if let Err(e) = app.config.save() {
                        app.set_temporary_status(&format!("Failed to save config: {e}"));
                    }
                    app.graph_plugin = None;
                    app.mode = ViewMode::List;

                    app.reload_theme();
                    app.open_note_from_graph(&note_id);
                }
                OverlayResult::NoteModified(note_id) => {
                    app.refresh_note_single(None, &note_id);
                }
                OverlayResult::Exit => {
                    if let Err(e) = app.config.save() {
                        app.set_temporary_status(&format!("Failed to save config: {e}"));
                    }

                    app.graph_plugin = None;
                    app.mode = app.return_mode.take().unwrap_or(ViewMode::List);

                    app.reload_theme();
                }
                _ => {}
            },
            ViewKind::Draw => {
                if matches!(other, OverlayResult::Exit) {
                    app.draw_state = None;
                    app.close_draw_view();
                }
            }
            ViewKind::Canvas => {
                if let OverlayResult::Exit = other {
                    app.close_canvas_view();
                }
            }
            ViewKind::Backup => {
                if matches!(other, OverlayResult::Exit) {
                    app.reload_config();
                    app.backup_state = None;
                    app.mode = app.return_mode.take().unwrap_or(ViewMode::List);

                    app.reload_theme();
                }
            }
            ViewKind::Outline => match other {
                OverlayResult::Exit | OverlayResult::JumpToLine { .. } => {
                    app.outline_state = None;
                    app.mode = app.return_mode.take().unwrap_or(ViewMode::List);

                    app.reload_theme();
                }
                _ => {}
            },
        },
    }
}

pub fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut ratatui::Terminal<B>,
    app: &mut crate::app::App,
    events: &mut crate::event_source::EventSource,
) -> Result<()>
where
    <B as ratatui::backend::Backend>::Error: std::error::Error + Send + Sync + 'static,
{
    run_app_with_hook(terminal, app, events, &mut |_| false)
}

/// `pre_draw_hook` runs every loop iteration before the draw phase.
/// record_frame/fps/dirty-flag bookkeeping still runs.
pub fn run_app_with_hook<B: ratatui::backend::Backend>(
    terminal: &mut ratatui::Terminal<B>,
    app: &mut crate::app::App,
    events: &mut crate::event_source::EventSource,
    pre_draw_hook: &mut dyn FnMut(&mut crate::app::App) -> bool,
) -> Result<()>
where
    <B as ratatui::backend::Backend>::Error: std::error::Error + Send + Sync + 'static,
{
    if app.config.core.syntax_highlighting {
        let code_theme = std::sync::Arc::from(app.config.core.code_theme.as_str());
        crate::markdown::prewarm_syntax_assets(code_theme);
    }

    let mut focus = EditFocus::Body;
    let mut list_dirty = true;
    let mut graph_dirty = true;
    let mut prev_mode = app.mode;

    while !app.should_quit {
        if SHOULD_EXIT.load(Ordering::Acquire) {
            app.should_quit = true;
            break;
        }
        if app.mode == ViewMode::Edit {
            crate::editor_session::run_editor_session(terminal, app, events, pre_draw_hook)?;
            prev_mode = app.mode;
            continue;
        }
        let msgs_before = app.messages.messages.len();
        while let Ok(msg) = app.message_rx.try_recv() {
            app.messages.push(msg.text, msg.severity);
        }
        if app.messages.messages.len() != msgs_before {
            list_dirty = true;
            graph_dirty = true;
        }
        if app.messages.tick_expirations() {
            list_dirty = true;
            graph_dirty = true;
        }

        if app.mode != prev_mode {
            match app.mode {
                ViewMode::List => list_dirty = true,
                ViewMode::Graph => graph_dirty = true,
                _ => {}
            }
            prev_mode = app.mode;
        }

        while let Ok(evt) = app.catalog_event_rx.try_recv() {
            app.handle_catalog_event(evt);
            if app.mode == ViewMode::List {
                list_dirty = true;
            }
        }
        app.handle_search_events();
        process_watcher_events(app);

        if app.tick_status() {
            if app.mode == ViewMode::List {
                list_dirty = true;
            } else if app.mode == ViewMode::Graph {
                graph_dirty = true;
            }
        }
        let failed = app.backup_status.lock().take();
        if let Some(msg) = failed {
            app.set_temporary_status(&format!("Backup failed: {msg}"));
            if app.mode == ViewMode::List {
                list_dirty = true;
            }
        }

        if let Some(ref idx) = app.note_index
            && let Some(expiry) = idx.min_membership_expiry
            && crate::ui::now_unix_secs() >= expiry
        {
            app.rebuild_note_index();
            if app.mode == ViewMode::List {
                list_dirty = true;
            }
        }

        if app.needs_full_redraw {
            terminal.clear()?;
            app.needs_full_redraw = false;
            list_dirty = true;
            graph_dirty = true;
        }

        if app.mode == ViewMode::List
            && app
                .list
                .sections
                .contains(&crate::config::NotesSection::Graf)
            && app.graph_preview.is_some()
            && app.graph_preview_steps < 100
        {
            list_dirty = true;
        }

        // Apply update ticks for continuous views before rendering
        if app.mode == ViewMode::Graph
            && let Some(graf) = &mut app.graph_plugin
        {
            graf.overlay_update(&mut app.config);
        }

        let graph_active = app
            .graph_plugin
            .as_ref()
            .and_then(|g| g.graph_state.as_ref())
            .is_some_and(|s| {
                let st = s.read();
                !st.is_settled || st.physics_worker_active
            });

        let should_draw = if app.mode == ViewMode::List {
            list_dirty
        } else if app.mode == ViewMode::Graph {
            graph_dirty || graph_active
        } else {
            true
        };

        let skip_draw = pre_draw_hook(app);

        if should_draw {
            if !skip_draw
                && let Err(e) = terminal.draw(|frame| crate::ui::draw_ui(frame, app, focus))
            {
                return Err(e.into());
            }
            let now = std::time::Instant::now();
            if app.mode == ViewMode::Graph
                && let Some(graph_state) = &mut app.graph_plugin
                && graph_state.config_errors.is_empty()
            {
                graph_state.record_frame(now);
            }
            let elapsed = now.duration_since(app.last_frame_time).as_secs_f64();
            app.fps = app.fps * 0.9 + (1.0 / elapsed.max(0.001)) * 0.1;
            app.last_frame_time = now;
            if app.mode == ViewMode::List {
                list_dirty = false;
            }
            if app.mode == ViewMode::Graph {
                graph_dirty = false;
            }
        }

        let active_catalog = app.catalog_status.is_some();
        let active_search = app.search_status.is_some() || app.search_debounce_deadline.is_some();

        let poll_timeout = if app.mode == ViewMode::Graph {
            let graph_idle = !graph_dirty && !graph_active;
            if graph_idle {
                Duration::from_millis(50)
            } else {
                Duration::from_millis(16)
            }
        } else if app.mode == ViewMode::Setup {
            Duration::from_millis(250)
        } else if app.mode == ViewMode::Draw {
            Duration::from_millis(16)
        } else if app.mode == ViewMode::Canvas {
            Duration::from_millis(100)
        } else if active_catalog || active_search {
            Duration::from_millis(32)
        } else if matches!(
            app.list.preview_content,
            Some(crate::list_view::PreviewContent::Markdown(ref r)) if r.is_pending()
        ) || app
            .editor
            .md_preview_renderer
            .as_ref()
            .is_some_and(|r| r.is_pending())
        {
            Duration::from_millis(50)
        } else {
            Duration::from_millis(200)
        };

        let mut need_redraw = app.poll_renderers();

        // Drain completed image decode jobs into local Vec to avoid borrow conflict
        let decoded_results: Vec<anyhow::Result<crate::image_render::worker::DecodedImage>> =
            match &app.image_decode_rx {
                Some(rx) => std::iter::from_fn(|| rx.try_recv().ok()).collect(),
                None => Vec::new(),
            };
        for res in decoded_results {
            match res {
                Ok(img) => {
                    app.install_image(img);
                }
                Err(e) => {
                    let text = format!("Image decode failed: {e}");
                    app.set_temporary_status(&text);
                    app.messages
                        .push(text, crate::app::messages::MessageSeverity::Warning);
                }
            }
            need_redraw = true;
        }

        // Auto-backup check
        if let Some(interval_mins) = app.config.backup.auto_backup_interval {
            let now = std::time::Instant::now();
            let should_backup = match app.last_auto_backup {
                Some(last) => now.duration_since(last).as_secs() >= interval_mins * 60,
                None => true,
            };
            if should_backup {
                app.enqueue_backup("auto: scheduled backup");
                app.last_auto_backup = Some(now);
            }
        }

        if app.mode != ViewMode::List && need_redraw {
            if !skip_draw
                && let Err(e) = terminal.draw(|frame| crate::ui::draw_ui(frame, app, focus))
            {
                return Err(e.into());
            }
            let now = std::time::Instant::now();
            if app.mode == ViewMode::Graph
                && let Some(graph_state) = &mut app.graph_plugin
                && graph_state.config_errors.is_empty()
            {
                graph_state.record_frame(now);
            }
        } else if app.mode == ViewMode::List && need_redraw {
            list_dirty = true;
        }

        if events.poll(poll_timeout).context("event poll failed")? {
            list_dirty = true;
            graph_dirty = true;
            let size = terminal.size().context("failed to get terminal size")?;
            let _area = Rect::new(0, 0, size.width, size.height);
            let mut pending: Vec<crossterm::event::Event> = Vec::with_capacity(8);
            pending.push(events.read().context("failed to read event")?);
            while pending.len() < 64 && events.poll(Duration::ZERO)? {
                pending.push(events.read()?);
            }
            let mut batch: Vec<crossterm::event::Event> = Vec::with_capacity(pending.len());
            for ev in pending {
                let is_moved = matches!(&ev, Event::Mouse(m)
                    if m.kind == ratatui::crossterm::event::MouseEventKind::Moved);
                if is_moved
                    && batch.last().is_some_and(|e| {
                        matches!(e, Event::Mouse(m)
                            if m.kind == ratatui::crossterm::event::MouseEventKind::Moved)
                    })
                {
                    if let Some(last) = batch.last_mut() {
                        *last = ev;
                    }
                } else {
                    batch.push(ev);
                }
            }
            for ev in batch {
                dispatch_event(events, app, ev, &mut focus, terminal)?;
            }
        }
    }
    perform_orderly_catalog_shutdown(app);
    Ok(())
}

fn dispatch_event<B: ratatui::backend::Backend>(
    events: &mut crate::event_source::EventSource,
    app: &mut crate::app::App,
    ev: crossterm::event::Event,
    focus: &mut EditFocus,
    terminal: &mut ratatui::Terminal<B>,
) -> anyhow::Result<()>
where
    <B as ratatui::backend::Backend>::Error: std::error::Error + Send + Sync + 'static,
{
    match ev {
        // Global Ctrl+C — immediately signal exit
        Event::Key(key)
            if key.kind == KeyEventKind::Press
                && key.code == KeyCode::Char('c')
                && key.modifiers == KeyModifiers::CONTROL =>
        {
            if app.mode == ViewMode::Edit {
                let _ = app.autosave();
            } else if app
                .popups
                .active
                .as_ref()
                .is_some_and(|p| matches!(p, crate::popups::ActivePopup::Subnotes(_)))
            {
                let _ = app.close_subnotes_popup();
            }
            crate::force_quit();
        }
        mut ev @ (Event::Key(_) | Event::Mouse(_)) => {
            // Phase 1: coalesce Moved events — drain all queued Moved
            let _coalesced = if let Event::Mouse(mouse_event) = &ev {
                if mouse_event.kind == ratatui::crossterm::event::MouseEventKind::Moved {
                    let mut last = *mouse_event;
                    while events.poll(Duration::ZERO)? {
                        match events.read()? {
                            Event::Mouse(next)
                                if next.kind
                                    == ratatui::crossterm::event::MouseEventKind::Moved =>
                            {
                                last = next;
                            }
                            _ => break,
                        }
                    }
                    app.mouse_pos = Some((last.column, last.row));
                    Some(Event::Mouse(last))
                } else {
                    app.mouse_pos = Some((mouse_event.column, mouse_event.row));
                    None
                }
            } else {
                None
            };
            if let Some(ev2) = _coalesced {
                ev = ev2;
            }
            let size = terminal.size().context("failed to get terminal size")?;
            let area = Rect::new(0, 0, size.width, size.height);
            if crate::events::handle_global_popups_and_palette(app, ev.clone(), area) {
                return Ok(());
            }
            if let Event::Mouse(ref mouse_event) = ev
                && crate::events::handle_global_popup_mouse(app, mouse_event, area)
            {
                return Ok(());
            }
            match ev {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    let handled = match app.mode {
                        ViewMode::List => handle_list_keys(app, key),
                        ViewMode::Help => {
                            handle_help_keys(app, key);
                            false
                        }
                        // Edit events are consumed by `editor_session` before
                        // generic dispatch resumes.
                        ViewMode::Edit => false,
                        ViewMode::Setup => {
                            crate::events::handle_setup_keys(app, key);
                            false
                        }
                        ViewMode::Graph
                        | ViewMode::Canvas
                        | ViewMode::Draw
                        | ViewMode::Backup
                        | ViewMode::Outline => match OverlayState::take(app) {
                            Some(mut view) => {
                                let kind = view.kind();
                                let res = view.overlay_handle_event(Event::Key(key), app, area);
                                view.put_back(app);
                                finish_overlay_event(app, kind, res?);
                                true
                            }
                            None => false,
                        },
                    };
                    let _ = handled;
                }
                Event::Mouse(mouse_event) => {
                    let size = terminal.size().context("failed to get terminal size")?;
                    let area = Rect::new(0, 0, size.width, size.height);
                    match app.mode {
                        ViewMode::List => {
                            handle_list_mouse(app, mouse_event, area);
                            let is_drag = matches!(
                                mouse_event.kind,
                                ratatui::crossterm::event::MouseEventKind::Drag(_)
                            );
                            if is_drag {
                                while events.poll(Duration::ZERO)? {
                                    match events.read()? {
                                        Event::Mouse(next_mouse) => {
                                            app.mouse_pos =
                                                Some((next_mouse.column, next_mouse.row));
                                            handle_list_mouse(app, next_mouse, area);
                                        }
                                        _ => break,
                                    }
                                }
                            }
                        }
                        // Edit events are consumed by `editor_session` before
                        // generic dispatch resumes.
                        ViewMode::Edit => {}
                        ViewMode::Help => {
                            handle_help_mouse(app, mouse_event, area);
                        }
                        ViewMode::Graph
                        | ViewMode::Canvas
                        | ViewMode::Draw
                        | ViewMode::Backup
                        | ViewMode::Outline => {
                            let kind = ViewKind::from_mode(app.mode);
                            let scrolls = matches!(kind, Some(ViewKind::Draw | ViewKind::Canvas));
                            let mut coalesce = false;
                            if let Some(mut view) = OverlayState::take(app) {
                                coalesce = matches!(
                                    mouse_event.kind,
                                    ratatui::crossterm::event::MouseEventKind::Drag(_)
                                ) || (scrolls
                                    && matches!(
                                        mouse_event.kind,
                                        ratatui::crossterm::event::MouseEventKind::ScrollUp
                                            | ratatui::crossterm::event::MouseEventKind::ScrollDown
                                    ));
                                let result =
                                    view.overlay_handle_event(Event::Mouse(mouse_event), app, area);
                                view.put_back(app);
                                finish_overlay_event(app, kind.unwrap_or(ViewKind::Graph), result?);
                            }
                            if coalesce && let Some(mut view) = OverlayState::take(app) {
                                drain_queued_mouse_events(&mut view, app, area, events)?;
                                view.put_back(app);
                            }
                        }
                        ViewMode::Setup => {
                            crate::events::handle_setup_mouse(app, mouse_event, area);
                        }
                    }
                }
                _ => {}
            }
        }
        Event::Paste(data) => {
            if crate::events::handle_bracketed_paste(app, data, focus) {
                app.set_temporary_status("Pasted from clipboard");
            }
        }
        Event::Resize(_, _) => {}
        _ => {}
    }
    if let Some(msg) = crate::text_edit::take_clipboard_notice() {
        app.set_temporary_status(msg);
    }
    Ok(())
}

pub use event_source::EventSource;
pub use session::{SessionGuard, bootstrap_app, finish_session, start_session};
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn list_dirty_draws_only_on_change() {
        let storage = Storage {
            data_dir: PathBuf::from("/tmp"),
            config_dir: PathBuf::from("/tmp"),
            notes_dir: PathBuf::from("/tmp"),
            templates_dir: PathBuf::from("/tmp"),
            key: [1u8; 32],
            skip_dir_patterns: vec![],
        };
        let mut app = App::new(storage).unwrap();
        app.mode = ViewMode::List;

        let mut list_dirty = true;
        let mut draw_count = 0;

        for _tick in 0..5 {
            let should_draw = if app.mode == ViewMode::List {
                list_dirty
            } else {
                true
            };

            if should_draw {
                draw_count += 1;
                if app.mode == ViewMode::List {
                    list_dirty = false;
                }
            }
        }

        assert_eq!(draw_count, 1);

        app.set_temporary_status("New Status");
        list_dirty = true;

        if list_dirty {
            draw_count += 1;
        }

        assert_eq!(draw_count, 2);
    }
}
