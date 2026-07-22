#![cfg_attr(test, allow(clippy::unwrap_used))]
pub mod actions;
pub mod app_theme;
pub mod backup;
pub mod calendar;
pub mod cli;
pub mod config;
pub mod console;
pub mod constants;
pub mod draw;
pub mod editor;
pub mod frontmatter;
pub mod fsutil;
pub mod goals;
pub mod graf;
pub mod image_render;
pub mod keybinds;
pub mod list_view;
pub mod local_state;
pub mod markdown;
pub mod migration;
pub mod note_index;
pub mod outline;
pub mod overlay;
pub mod palette;
pub mod paths;
#[cfg(test)]
pub mod perf_tests;
pub mod pinstar;
pub mod popups;
pub mod preview;
pub mod sanitize;
pub mod setup;
pub mod snapshot;
pub mod statusline;
pub mod templates;
pub mod text_edit;

use crate::cli::{
    CacheCmd, Cli, Command, ConfigCmd, KeybindsCmd, NotesCmd, StorageCmd, TemplatesCmd,
};
use crate::config::ClinConfig;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::overlay::OverlayView;
use clap::{CommandFactory, FromArgMatches};

use std::borrow::Cow;
use std::fs;
use std::io::{self, Stdout, Write};
use std::process;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use uuid::Uuid;

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
static SHOULD_EXIT: LazyLock<Arc<AtomicBool>> = LazyLock::new(|| Arc::new(AtomicBool::new(false)));
static SIGNAL_COUNT: AtomicU32 = AtomicU32::new(0);
static FORCE_QUIT: AtomicBool = AtomicBool::new(false);

use anyhow::{Context, Result};
use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyCode, KeyEventKind, KeyModifiers,
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
    // MUST check before Storage::init — Storage::init -> ClinConfig::load creates config.toml.
    let first_run = crate::config::ClinConfig::config_path()
        .map(|p| !p.exists())
        .unwrap_or(false);
    let storage = Storage::init()?;
    let mut app = App::new_deferred(storage)?;
    if first_run || force_setup {
        app.open_setup_view();
    }
    if let Some(title) = open_title
        && !app.open_note_by_title(&title)
    {
        eprintln!(
            "{}",
            console::error(&format!("No note found with title: {title}"))
        );
        process::exit(1);
    }
    run_tui_session(&mut app)
}

fn run_notes(action: NotesCmd) -> Result<()> {
    match action {
        NotesCmd::List => {
            let storage = Storage::init()?;
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
            let storage = Storage::init()?;
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
            let storage = Storage::init()?;
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
            let mut storage = Storage::init()?;

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

            let storage = Storage::init()?;
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

fn run_storage(action: StorageCmd) -> Result<()> {
    match action {
        StorageCmd::Show => {
            let bootstrap = ClinConfig::load()?;
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
            let mut bootstrap = ClinConfig::load()?;
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
                let state_path = paths.state_path();
                let _ = crate::local_state::LocalState::update(&state_path, |s| {
                    if s.storage_migration.is_none() {
                        s.storage_migration = Some(crate::local_state::StorageMigrationState {
                            previous_path: old_path.clone(),
                            target_path: path.clone(),
                        });
                    }
                    Ok(())
                });
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
            let mut bootstrap = ClinConfig::load()?;
            let old_path = bootstrap.effective_storage_path()?;
            let default = ClinConfig::default_storage_path()?;

            // Record migration before resetting
            if old_path != default
                && let Ok(paths) = crate::paths::AppPaths::discover(ClinConfig::config_path()?)
            {
                let state_path = paths.state_path();
                let _ = crate::local_state::LocalState::update(&state_path, |s| {
                    if s.storage_migration.is_none() {
                        s.storage_migration = Some(crate::local_state::StorageMigrationState {
                            previous_path: old_path.clone(),
                            target_path: default.clone(),
                        });
                    }
                    Ok(())
                });
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
            let bootstrap = ClinConfig::load()?;
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
                            let default = ClinConfig::default_storage_path()?;
                            if default.exists() && default.is_dir() && default != to {
                                println!(
                                    "{}",
                                    console::info("Recorded previous path does not exist.")
                                );
                                println!(
                                    "Found data at default location: {}",
                                    console::path(&default)
                                );
                                print!("{}", console::warning("Migrate from there? [y/N]: "));
                                io::stdout().flush()?;

                                let mut input = String::new();
                                io::stdin().read_line(&mut input)?;
                                if !input.trim().eq_ignore_ascii_case("y") {
                                    println!("{}", console::warning("Migration cancelled."));
                                    return Ok(());
                                }
                                default
                            } else {
                                anyhow::bail!(
                                    "No previous storage location found. Nothing to migrate."
                                );
                            }
                        }
                    } else {
                        let default = ClinConfig::default_storage_path()?;
                        if default.exists() && default.is_dir() && default != to {
                            println!("{}", console::info("No previous storage path recorded."));
                            println!(
                                "Found data at default location: {}",
                                console::path(&default)
                            );
                            print!("{}", console::warning("Migrate from there? [y/N]: "));
                            io::stdout().flush()?;

                            let mut input = String::new();
                            io::stdin().read_line(&mut input)?;
                            if !input.trim().eq_ignore_ascii_case("y") {
                                println!("{}", console::warning("Migration cancelled."));
                                return Ok(());
                            }
                            default
                        } else {
                            anyhow::bail!(
                                "No previous storage location found. Nothing to migrate."
                            );
                        }
                    }
                } else {
                    let default = ClinConfig::default_storage_path()?;
                    if default.exists() && default.is_dir() && default != to {
                        println!("{}", console::info("No previous storage path recorded."));
                        println!(
                            "Found data at default location: {}",
                            console::path(&default)
                        );
                        print!("{}", console::warning("Migrate from there? [y/N]: "));
                        io::stdout().flush()?;

                        let mut input = String::new();
                        io::stdin().read_line(&mut input)?;
                        if !input.trim().eq_ignore_ascii_case("y") {
                            println!("{}", console::warning("Migration cancelled."));
                            return Ok(());
                        }
                        default
                    } else {
                        anyhow::bail!("No previous storage location found. Nothing to migrate.");
                    }
                }
            } else {
                let default = ClinConfig::default_storage_path()?;
                if default.exists() && default.is_dir() && default != to {
                    println!("{}", console::info("No previous storage path recorded."));
                    println!(
                        "Found data at default location: {}",
                        console::path(&default)
                    );
                    print!("{}", console::warning("Migrate from there? [y/N]: "));
                    io::stdout().flush()?;

                    let mut input = String::new();
                    io::stdin().read_line(&mut input)?;
                    if !input.trim().eq_ignore_ascii_case("y") {
                        println!("{}", console::warning("Migration cancelled."));
                        return Ok(());
                    }
                    default
                } else {
                    anyhow::bail!("No previous storage location found. Nothing to migrate.");
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
            let target_is_vault = bootstrap.has_custom_storage_path();

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

            // Determine effective target dirs
            let notes_dst = if target_is_vault {
                to.clone()
            } else {
                to.join("notes")
            };
            let templates_dst = if target_is_vault {
                to.join(".clin").join("templates")
            } else {
                to.join("templates")
            };

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
            let storage = Storage::init()?;
            let config = crate::config::ClinConfig::load().unwrap_or_default();
            println!(
                "{}",
                storage
                    .keybinds_path_for_preset(config.core.keybind_preset)
                    .display()
            );
            Ok(())
        }
        KeybindsCmd::Export => {
            let storage = Storage::init()?;
            let config = crate::config::ClinConfig::load().unwrap_or_default();
            let preset = config.core.keybind_preset;
            let keybinds = storage.load_keybinds_with_preset(preset);
            let toml = keybinds.to_toml();
            let content = toml::to_string_pretty(&toml)?;
            println!("{content}");
            Ok(())
        }
        KeybindsCmd::Reset => {
            let storage = Storage::init()?;
            let config = crate::config::ClinConfig::load().unwrap_or_default();
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
            let storage = Storage::init()?;
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
            let storage = Storage::init()?;
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
            let storage = Storage::init()?;
            let app_paths = crate::paths::AppPaths::discover(ClinConfig::config_path()?)?;
            let vault_id = crate::local_state::vault_identity_path(&storage.data_dir)?;
            let digest = crate::paths::vault_cache_digest(&vault_id);
            let scoped_cache = app_paths.scoped_summary_cache_path(&digest);

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

    let window_start = app.watcher_window_start.unwrap();
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
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
                if ev.paths.len() == 2 {
                    let old_p = &ev.paths[0];
                    let new_p = &ev.paths[1];
                    if let (Ok(rel_old), Ok(rel_new)) = (
                        old_p.strip_prefix(&app.storage.notes_dir),
                        new_p.strip_prefix(&app.storage.notes_dir),
                    ) {
                        if let (Some(old_str), Some(new_str)) = (rel_old.to_str(), rel_new.to_str())
                        {
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
        {
            if ack_rx.recv_timeout(Duration::from_millis(100)).is_ok() {
                break;
            }
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
        if let Err(e) = entered {
            disable_raw_mode().ok(); // guard won't be built; clean up raw mode ourselves
            return Err(e).context("failed to enter alternate screen");
        }
        Ok(TerminalGuard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        disable_raw_mode().ok();
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
    // Clean up any orphaned plaintext temp files from a prior crashed session.
    crate::fsutil::cleanup_orphaned_temp_files();

    let register_signal = |sig: std::os::raw::c_int| {
        // SAFETY: signal_hook::low_level::register is async-signal-safe.
        // The closure only performs atomic stores and fetch-adds, which are
        // safe operations within a signal handler.
        let _ = unsafe {
            signal_hook::low_level::register(sig, || {
                SHOULD_EXIT.store(true, Ordering::Release);
                if SIGNAL_COUNT.fetch_add(1, Ordering::SeqCst) >= 1 {
                    FORCE_QUIT.store(true, Ordering::Release);
                }
            })
        };
    };
    register_signal(signal_hook::consts::SIGINT);
    register_signal(signal_hook::consts::SIGTERM);
    #[cfg(unix)]
    {
        register_signal(signal_hook::consts::SIGHUP);
        register_signal(signal_hook::consts::SIGQUIT);
    }

    // Spawn the background backup worker before entering the terminal.
    let (tx, done_rx) =
        crate::backup::worker::spawn(app.git_lock.clone(), app.backup_status.clone());

    app.backup_tx = Some(tx);

    // Spawn the background image decode worker.
    let (decode_tx, decode_rx) = crate::image_render::worker::spawn();
    app.image_decode_tx = Some(decode_tx);
    app.image_decode_rx = Some(decode_rx);

    // Initialize the optional file system watcher for auto-refreshing the
    // notes list when external editors or sync tools modify files.
    // Uses raw `notify` (not `notify-debouncer-mini`) so we can manually
    // filter out `Access` events, preventing an infinite refresh loop caused
    // by the app reading its own files during `refresh_notes()`.
    let _watcher = if app.config.core.auto_refresh {
        use notify::{EventKind, RecursiveMode, Watcher};
        let (tx, rx) = std::sync::mpsc::sync_channel::<crate::app::WatchedFsEvent>(1024);
        let overflow = Arc::new(AtomicBool::new(false));
        app.fs_event_rx = Some(rx);
        app.fs_overflow = overflow.clone();

        let notes_path = app.storage.notes_dir.clone();
        let overflow_cb = overflow.clone();
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let observed_at = Instant::now();
            let event = match res {
                Ok(e) => e,
                Err(_) => {
                    overflow_cb.store(true, Ordering::SeqCst);
                    return;
                }
            };
            if event.need_rescan() {
                overflow_cb.store(true, Ordering::SeqCst);
                return;
            }
            if matches!(event.kind, EventKind::Access(_)) {
                return;
            }
            if !event.paths.is_empty() {
                let all_ignored = event.paths.iter().all(|p| {
                    let path_str = p.to_string_lossy();
                    path_str.contains("/.git/")
                        || path_str.contains("\\.git\\")
                        || path_str.ends_with(".tmp")
                        || path_str.ends_with(".lock")
                        || path_str.ends_with("~")
                });
                if all_ignored {
                    return;
                }
            }

            if tx
                .try_send(crate::app::WatchedFsEvent { observed_at, event })
                .is_err()
            {
                overflow_cb.store(true, Ordering::SeqCst);
            }
        })
        .ok();

        if let Some(ref mut w) = watcher {
            let _ = w.watch(&notes_path, RecursiveMode::Recursive);
        }
        watcher
    } else {
        None
    };

    let result = {
        let _guard = TerminalGuard::enter(app.mouse_enabled)?;
        let backend = ratatui::backend::CrosstermBackend::new(io::stdout());
        let mut terminal = Terminal::new(backend).context("failed to create terminal")?;
        // Detect terminal graphics protocol while inside alt-screen+raw mode.
        // Skip detection entirely when image rendering is disabled in config.
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
        let res = std::panic::catch_unwind(move || run_app(*terminal_safe, *app_safe));
        if app.mode == ViewMode::Edit {
            app.autosave();
        }
        res
    }; // _guard dropped here: raw mode off, alt screen left

    let signal_exit = SHOULD_EXIT.load(Ordering::Acquire);

    if signal_exit {
        // Don't join: the worker may be mid-commit. libgit2 commits are atomic
        // (HEAD updated last), so killing the thread cannot corrupt the repo.
        drop(app.backup_tx.take());
    } else if app.config.backup.enabled && app.config.backup.backup_on_quit {
        println!("Backing up…");
        let _ = app.backup_tx.as_ref().map(|tx| {
            tx.send(crate::backup::worker::BackupJob::Flush(
                "auto: backup on quit".into(),
            ))
        });
        drop(app.backup_tx.take());
        let deadline = std::time::Instant::now() + crate::backup::worker::FLUSH_BOUND;
        let timed_out = loop {
            if FORCE_QUIT.load(Ordering::Acquire) {
                break true;
            }
            match done_rx.recv_timeout(std::time::Duration::from_millis(200)) {
                Ok(()) => break false,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break true,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if std::time::Instant::now() >= deadline {
                        break true;
                    }
                }
            }
        };
        if timed_out {
            eprintln!("Backup still running in background; exiting.");
        } else {
            println!("Done.");
        }
        if let Some(msg) = app.backup_status.lock().take() {
            eprintln!("Backup warning: {msg}");
        }
    } else {
        drop(app.backup_tx.take());
        let deadline = std::time::Instant::now() + crate::backup::worker::FLUSH_BOUND;
        loop {
            if FORCE_QUIT.load(Ordering::Acquire) {
                break;
            }
            match done_rx.recv_timeout(std::time::Duration::from_millis(200)) {
                Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if std::time::Instant::now() >= deadline {
                        break;
                    }
                }
            }
        }
    }

    match result {
        Ok(r) => r,
        Err(err) => std::panic::resume_unwind(err),
    }
}

/// Drain mouse events already sitting in the crossterm queue and feed each to the
/// overlay view, collapsing a burst of drag events into one later render.
/// Called only after the loop has already dispatched a `Drag` event, so during an
/// active pan the queue contains only further `Drag`/`Up` mouse events.
/// Non-mouse events break the loop (vanishingly rare during an active drag; the
/// single read event is dropped rather than re-queued because crossterm cannot
/// push back). Results from drained events are discarded — a mouse Drag/Up never
/// returns Exit/NoteOpened/OpenHelp.
fn drain_queued_mouse_events<V: OverlayView>(
    view: &mut V,
    app: &mut App,
    terminal: &Terminal<ratatui::backend::CrosstermBackend<Stdout>>,
) -> Result<()> {
    while event::poll(Duration::ZERO)? {
        match event::read()? {
            ev @ Event::Mouse(_) => {
                let _ = view.overlay_handle_event(ev, app, terminal)?;
            }
            _ => break,
        }
    }
    Ok(())
}

fn run_app(
    terminal: &mut Terminal<ratatui::backend::CrosstermBackend<Stdout>>,
    app: &mut App,
) -> Result<()> {
    if app.config.core.syntax_highlighting {
        let code_theme = std::sync::Arc::from(app.config.core.code_theme.as_str());
        crate::markdown::prewarm_syntax_assets(code_theme);
    }

    let mut focus = EditFocus::Body;
    let mut mouse_selecting = false;
    let mut mouse_dragged = false;
    let mut list_dirty = true;
    let mut prev_mode = app.mode;

    while !app.should_quit {
        if SHOULD_EXIT.load(Ordering::Acquire) {
            app.should_quit = true;
            break;
        }

        if app.mode != prev_mode {
            if app.mode == ViewMode::List {
                list_dirty = true;
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

        if app.tick_status() && app.mode == ViewMode::List {
            list_dirty = true;
        }
        let failed = app.backup_status.lock().take();
        if let Some(msg) = failed {
            app.set_temporary_status(&format!("Backup failed: {msg}"));
            if app.mode == ViewMode::List {
                list_dirty = true;
            }
        }

        if let Some(ref idx) = app.note_index {
            if let Some(expiry) = idx.min_membership_expiry {
                if crate::ui::now_unix_secs() >= expiry {
                    app.rebuild_note_index();
                    if app.mode == ViewMode::List {
                        list_dirty = true;
                    }
                }
            }
        }

        if app.needs_full_redraw {
            terminal.clear()?;
            app.needs_full_redraw = false;
            list_dirty = true;
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
            && let Some(graf) = &mut app.graph_state
        {
            graf.overlay_update(&mut app.config);
        }

        let should_draw = if app.mode == ViewMode::List {
            list_dirty
        } else {
            true
        };

        if should_draw {
            if let Err(e) = terminal.draw(|frame| crate::ui::draw_ui(frame, app, focus)) {
                return Err(e.into());
            }
            let now = std::time::Instant::now();
            if app.mode == ViewMode::Graph {
                if let Some(ref mut graph_state) = app.graph_state {
                    if graph_state.config_errors.is_empty() {
                        graph_state.record_frame(now);
                    }
                }
            }
            let elapsed = now.duration_since(app.last_frame_time).as_secs_f64();
            app.fps = app.fps * 0.9 + (1.0 / elapsed.max(0.001)) * 0.1;
            app.last_frame_time = now;
            if app.mode == ViewMode::List {
                list_dirty = false;
            }
        }

        let active_catalog = app.catalog_status.is_some();
        let active_search = app.search_status.is_some() || app.search_debounce_deadline.is_some();

        let poll_timeout = if app.mode == ViewMode::Graph || app.mode == ViewMode::Draw {
            Duration::from_millis(16)
        } else if app.mode == ViewMode::Canvas {
            Duration::from_millis(100)
        } else if active_catalog || active_search {
            Duration::from_millis(32)
        } else if app.is_first_cache_build
            || matches!(
                app.list.preview_content,
                Some(crate::list_view::PreviewContent::Markdown(ref r)) if r.is_pending()
            )
            || app
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
                    app.set_temporary_status(&format!("Image decode failed: {e}"));
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
            if let Err(e) = terminal.draw(|frame| crate::ui::draw_ui(frame, app, focus)) {
                return Err(e.into());
            }
            let now = std::time::Instant::now();
            if app.mode == ViewMode::Graph {
                if let Some(ref mut graph_state) = app.graph_state {
                    if graph_state.config_errors.is_empty() {
                        graph_state.record_frame(now);
                    }
                }
            }
        } else if app.mode == ViewMode::List && need_redraw {
            list_dirty = true;
        }

        if event::poll(poll_timeout).context("event poll failed")? {
            list_dirty = true;
            match event::read().context("failed to read event")? {
                // Global Ctrl+C — immediately kill process
                Event::Key(key)
                    if key.kind == KeyEventKind::Press
                        && key.code == KeyCode::Char('c')
                        && key.modifiers == KeyModifiers::CONTROL =>
                {
                    crate::force_quit();
                }
                mut ev @ (Event::Key(_) | Event::Mouse(_)) => {
                    // Phase 1: coalesce Moved events — drain all queued Moved
                    // events, keeping only the last position
                    let _coalesced = if let Event::Mouse(mouse_event) = &ev {
                        if mouse_event.kind == ratatui::crossterm::event::MouseEventKind::Moved {
                            let mut last = *mouse_event;
                            while event::poll(Duration::ZERO)? {
                                match event::read()? {
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
                    // Global popups & palette get first chance to consume
                    let size = terminal.size().context("failed to get terminal size")?;
                    let area = Rect::new(0, 0, size.width, size.height);
                    if crate::events::handle_global_popups_and_palette(app, ev.clone(), area) {
                        continue;
                    }

                    // Popup mouse handling (runs for all views)
                    if let Event::Mouse(ref mouse_event) = ev
                        && crate::events::handle_global_popup_mouse(app, mouse_event, area)
                    {
                        continue;
                    }

                    match ev {
                        Event::Key(key) if key.kind == KeyEventKind::Press => {
                            let handled = match app.mode {
                                ViewMode::List => handle_list_keys(app, key),
                                ViewMode::Help => {
                                    handle_help_keys(app, key);
                                    false
                                }
                                ViewMode::Edit => handle_edit_keys(app, key, &mut focus),
                                ViewMode::Setup => {
                                    crate::events::handle_setup_keys(app, key);
                                    false
                                }
                                ViewMode::Graph => {
                                    if let Some(mut graf) = app.graph_state.take() {
                                        let res = graf.overlay_handle_event(
                                            Event::Key(key),
                                            app,
                                            terminal,
                                        );
                                        app.graph_state = Some(graf);
                                        match res? {
                                            crate::overlay::OverlayResult::NoteOpened(note_id) => {
                                                if let Err(e) = app.config.save() {
                                                    app.set_temporary_status(&format!(
                                                        "Failed to save config: {e}"
                                                    ));
                                                }
                                                app.graph_state = None;
                                                app.mode = ViewMode::List;

                                                app.reload_theme();
                                                app.open_note_from_graph(&note_id);
                                            }
                                            crate::overlay::OverlayResult::OpenHelp(tab) => {
                                                app.reload_theme();
                                                app.open_help_page_with_tab(tab);
                                            }
                                            crate::overlay::OverlayResult::Exit => {
                                                if let Err(e) = app.config.save() {
                                                    app.set_temporary_status(&format!(
                                                        "Failed to save config: {e}"
                                                    ));
                                                }

                                                app.graph_state = None;
                                                app.mode = app
                                                    .return_mode
                                                    .take()
                                                    .unwrap_or(ViewMode::List);

                                                app.reload_theme();
                                            }
                                            _ => {}
                                        }
                                        true
                                    } else {
                                        false
                                    }
                                }
                                ViewMode::Draw => {
                                    if let Some(mut draw) = app.draw_state.take() {
                                        let res = draw.overlay_handle_event(
                                            Event::Key(key),
                                            app,
                                            terminal,
                                        );
                                        app.draw_state = Some(draw);
                                        match res? {
                                            crate::overlay::OverlayResult::Exit => {
                                                app.draw_state = None;
                                                app.close_draw_view();
                                            }
                                            crate::overlay::OverlayResult::OpenHelp(tab) => {
                                                app.reload_theme();
                                                app.open_help_page_with_tab(tab);
                                            }
                                            _ => {}
                                        }
                                        true
                                    } else {
                                        false
                                    }
                                }
                                ViewMode::Canvas => {
                                    if let Some(mut canvas) = app.canvas_state.take() {
                                        let res = canvas.overlay_handle_event(
                                            Event::Key(key),
                                            app,
                                            terminal,
                                        );
                                        app.canvas_state = Some(canvas);
                                        match res? {
                                            crate::overlay::OverlayResult::OpenHelp(tab) => {
                                                app.reload_theme();
                                                app.open_help_page_with_tab(tab);
                                            }
                                            crate::overlay::OverlayResult::Exit => {
                                                app.close_canvas_view();
                                            }
                                            _ => {}
                                        }
                                        true
                                    } else {
                                        false
                                    }
                                }
                                ViewMode::Backup => {
                                    if let Some(mut backup) = app.backup_state.take() {
                                        let res = backup.overlay_handle_event(
                                            Event::Key(key),
                                            app,
                                            terminal,
                                        );
                                        app.backup_state = Some(backup);
                                        match res? {
                                            crate::overlay::OverlayResult::Exit => {
                                                app.reload_config();
                                                app.backup_state = None;
                                                app.mode = app
                                                    .return_mode
                                                    .take()
                                                    .unwrap_or(ViewMode::List);

                                                app.reload_theme();
                                            }
                                            crate::overlay::OverlayResult::OpenHelp(tab) => {
                                                app.reload_theme();
                                                app.open_help_page_with_tab(tab);
                                            }
                                            _ => {}
                                        }
                                        true
                                    } else {
                                        false
                                    }
                                }
                                ViewMode::Outline => {
                                    if let Some(mut tree) = app.outline_state.take() {
                                        let res = tree.overlay_handle_event(
                                            Event::Key(key),
                                            app,
                                            terminal,
                                        );
                                        app.outline_state = Some(tree);
                                        match res? {
                                            crate::overlay::OverlayResult::Exit => {
                                                app.outline_state = None;
                                                app.mode = app
                                                    .return_mode
                                                    .take()
                                                    .unwrap_or(ViewMode::List);

                                                app.reload_theme();
                                            }
                                            crate::overlay::OverlayResult::JumpToLine {
                                                note_id: _,
                                                line: _,
                                            } => {
                                                app.outline_state = None;
                                                app.mode = app
                                                    .return_mode
                                                    .take()
                                                    .unwrap_or(ViewMode::List);

                                                app.reload_theme();
                                            }
                                            crate::overlay::OverlayResult::OpenHelp(tab) => {
                                                app.reload_theme();
                                                app.open_help_page_with_tab(tab);
                                            }
                                            _ => {}
                                        }
                                        true
                                    } else {
                                        false
                                    }
                                }
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
                                        while event::poll(Duration::ZERO)? {
                                            match event::read()? {
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
                                ViewMode::Edit => {
                                    handle_edit_mouse(
                                        app,
                                        mouse_event,
                                        area,
                                        &mut focus,
                                        &mut mouse_selecting,
                                        &mut mouse_dragged,
                                    );
                                    if matches!(
                                        mouse_event.kind,
                                        ratatui::crossterm::event::MouseEventKind::Drag(_)
                                    ) {
                                        while event::poll(Duration::ZERO)? {
                                            match event::read()? {
                                                Event::Mouse(next) => {
                                                    app.mouse_pos = Some((next.column, next.row));
                                                    handle_edit_mouse(
                                                        app,
                                                        next,
                                                        area,
                                                        &mut focus,
                                                        &mut mouse_selecting,
                                                        &mut mouse_dragged,
                                                    );
                                                }
                                                _ => break,
                                            }
                                        }
                                    }
                                }
                                ViewMode::Help => {
                                    handle_help_mouse(app, mouse_event, area);
                                }
                                ViewMode::Graph => {
                                    let mut is_drag = false;
                                    if let Some(mut graf) = app.graph_state.take() {
                                        is_drag = matches!(
                                            mouse_event.kind,
                                            ratatui::crossterm::event::MouseEventKind::Drag(_)
                                        );
                                        let result = graf.overlay_handle_event(
                                            Event::Mouse(mouse_event),
                                            app,
                                            terminal,
                                        );
                                        app.graph_state = Some(graf);
                                        match result? {
                                            crate::overlay::OverlayResult::NoteOpened(note_id) => {
                                                if let Err(e) = app.config.save() {
                                                    app.set_temporary_status(&format!(
                                                        "Failed to save config: {e}"
                                                    ));
                                                }
                                                app.graph_state = None;
                                                app.mode = ViewMode::List;

                                                app.reload_theme();
                                                app.open_note_from_graph(&note_id);
                                            }
                                            crate::overlay::OverlayResult::OpenHelp(tab) => {
                                                app.reload_theme();
                                                app.open_help_page_with_tab(tab);
                                            }
                                            crate::overlay::OverlayResult::Exit => {
                                                if let Err(e) = app.config.save() {
                                                    app.set_temporary_status(&format!(
                                                        "Failed to save config: {e}"
                                                    ));
                                                }

                                                app.graph_state = None;
                                                app.mode = app
                                                    .return_mode
                                                    .take()
                                                    .unwrap_or(ViewMode::List);

                                                app.reload_theme();
                                            }
                                            _ => {}
                                        }
                                    }
                                    if is_drag && let Some(mut graf) = app.graph_state.take() {
                                        drain_queued_mouse_events(&mut graf, app, terminal)?;
                                        app.graph_state = Some(graf);
                                    }
                                }
                                ViewMode::Draw => {
                                    let mut coalesce = false;
                                    if let Some(mut draw) = app.draw_state.take() {
                                        coalesce = matches!(
                                            mouse_event.kind,
                                            ratatui::crossterm::event::MouseEventKind::Drag(_)
                                                | ratatui::crossterm::event::MouseEventKind::ScrollUp
                                                | ratatui::crossterm::event::MouseEventKind::ScrollDown
                                        );
                                        let result = draw.overlay_handle_event(
                                            Event::Mouse(mouse_event),
                                            app,
                                            terminal,
                                        );
                                        app.draw_state = Some(draw);
                                        match result? {
                                            crate::overlay::OverlayResult::Exit => {
                                                app.draw_state = None;
                                                app.close_draw_view();
                                            }
                                            crate::overlay::OverlayResult::OpenHelp(tab) => {
                                                app.reload_theme();
                                                app.open_help_page_with_tab(tab);
                                            }
                                            _ => {}
                                        }
                                    }
                                    if coalesce && let Some(mut draw) = app.draw_state.take() {
                                        drain_queued_mouse_events(&mut draw, app, terminal)?;
                                        app.draw_state = Some(draw);
                                    }
                                }
                                ViewMode::Canvas => {
                                    let mut coalesce = false;
                                    if let Some(mut canvas) = app.canvas_state.take() {
                                        coalesce = matches!(
                                            mouse_event.kind,
                                            ratatui::crossterm::event::MouseEventKind::Drag(_)
                                                | ratatui::crossterm::event::MouseEventKind::ScrollUp
                                                | ratatui::crossterm::event::MouseEventKind::ScrollDown
                                        );
                                        let _ = canvas.overlay_handle_event(
                                            Event::Mouse(mouse_event),
                                            app,
                                            terminal,
                                        )?;
                                        app.canvas_state = Some(canvas);
                                    }
                                    if coalesce && let Some(mut canvas) = app.canvas_state.take() {
                                        drain_queued_mouse_events(&mut canvas, app, terminal)?;
                                        app.canvas_state = Some(canvas);
                                    }
                                }
                                ViewMode::Backup => {
                                    if let Some(mut backup) = app.backup_state.take() {
                                        let _ = backup.overlay_handle_event(
                                            Event::Mouse(mouse_event),
                                            app,
                                            terminal,
                                        )?;
                                        app.backup_state = Some(backup);
                                    }
                                }
                                ViewMode::Outline => {
                                    if let Some(mut tree) = app.outline_state.take() {
                                        let _ = tree.overlay_handle_event(
                                            Event::Mouse(mouse_event),
                                            app,
                                            terminal,
                                        )?;
                                        app.outline_state = Some(tree);
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
                Event::Paste(data) if app.mode == ViewMode::Edit => match focus {
                    EditFocus::Title => {
                        let normalized = data.replace(['\r', '\n'], " ");
                        app.editor.title_editor.insert_str(normalized);
                        app.status = Cow::Borrowed("Pasted title text");
                        app.request_editor_preview_update();
                    }
                    EditFocus::Body => {
                        app.editor.editor.insert_str(data);
                        app.status = Cow::Borrowed("Pasted body text");
                        app.request_editor_preview_update();
                    }
                    EditFocus::Sidebar => {}
                },
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }
    perform_orderly_catalog_shutdown(app);
    Ok(())
}

pub use constants::*;
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
            list_dirty = false;
        }

        assert_eq!(draw_count, 2);
    }
}
