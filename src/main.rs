#![cfg_attr(test, allow(clippy::unwrap_used))]
pub(crate) mod actions;
pub(crate) mod app_theme;
pub(crate) mod backup;
pub(crate) mod calendar;
pub(crate) mod cli;
mod config;
pub mod console;
pub(crate) mod constants;
pub(crate) mod content_tree;
pub(crate) mod draw;
pub(crate) mod editor;
pub(crate) mod frontmatter;
pub mod fsutil;
pub(crate) mod graf;
mod keybinds;
pub(crate) mod list_view;
pub(crate) mod markdown;
pub(crate) mod migration;
pub(crate) mod overlay;
pub(crate) mod palette;
pub(crate) mod pinstar;
pub(crate) mod popups;
pub(crate) mod preview;
pub(crate) mod sanitize;
pub(crate) mod snapshot;
mod templates;
pub(crate) mod text_edit;

use crate::cli::{Cli, Command, ConfigCmd, KeybindsCmd, NotesCmd, StorageCmd, TemplatesCmd};
use crate::config::ClinConfig;
use crate::keybinds::{EditAction, HelpAction, ListAction};
use clap::{CommandFactory, FromArgMatches};

use std::borrow::Cow;
use std::fs;
use std::io::{self, Stdout, Write};
use std::path::PathBuf;
use std::process;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use uuid::Uuid;

static SHOULD_EXIT: LazyLock<Arc<AtomicBool>> = LazyLock::new(|| Arc::new(AtomicBool::new(false)));

use anyhow::{Context, Result};
use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::layout::Rect;

mod app;
mod events;
mod storage;
mod ui;
use app::*;
use events::*;
use storage::*;
use ui::*;
fn main() -> Result<()> {
    // Panic hook: restore terminal before abort
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        disable_raw_mode().ok();
        let _ = execute!(
            io::stdout(),
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

    match cli.command {
        None => launch_tui(None),
        Some(Command::Notes { action }) => run_notes(action),
        Some(Command::Storage { action }) => run_storage(action),
        Some(Command::Keybinds { action }) => run_keybinds(action),
        Some(Command::Templates { action }) => run_templates(action),
        Some(Command::Config { action }) => run_config(action),
    }
}
fn launch_tui(open_title: Option<String>) -> Result<()> {
    let storage = Storage::init()?;
    let mut app = App::new_deferred(storage)?;
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
            let mut app = App::new(storage)?;
            app.refresh_notes()?;
            for (index, note) in app.notes.iter().enumerate() {
                println!(
                    "{} {}",
                    console::dim(&format!("{}.", index + 1)),
                    note.title
                );
            }
            Ok(())
        }
        NotesCmd::New { template, title } => {
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

            let id = Uuid::new_v4().simple().to_string();
            let note = Note {
                title: final_title,
                content,
                updated_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs(),
                tags,
            };

            let saved_id = app.storage.save_note(&id, &note)?;

            app.editor.editing_id = Some(saved_id.clone());
            app.refresh_notes()?;
            app.load_and_open_note(&saved_id, None);
            run_tui_session(&mut app)
        }
        NotesCmd::Open { title } => launch_tui(Some(title)),
        NotesCmd::Quick { content, title } => {
            let storage = Storage::init()?;

            let id = Uuid::new_v4().simple().to_string();
            let final_title = title.unwrap_or_else(|| "Quick Note".to_string());
            let note = Note {
                title: final_title.clone(),
                content,
                updated_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs(),
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
            let mut app = App::new(storage)?;
            app.refresh_notes()?;
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
            let old_path = bootstrap.effective_storage_path()?;

            if !path.is_absolute() {
                anyhow::bail!("Storage path must be absolute: {}", path.display());
            }

            fs::create_dir_all(&path)
                .with_context(|| format!("failed to create directory: {}", path.display()))?;

            if old_path.exists() && old_path != path {
                bootstrap.set_previous_storage_path(old_path);
            }

            bootstrap.set_storage_path(path.clone());
            bootstrap.save()?;

            println!(
                "{}",
                console::success(&format!("Storage path set to: {}", console::path(&path)))
            );

            if bootstrap.core.previous_storage_path.is_some() {
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
            bootstrap.reset_storage_path();
            bootstrap.save()?;
            let default = ClinConfig::default_storage_path()?;
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
            let mut bootstrap = ClinConfig::load()?;
            let to = bootstrap.effective_storage_path()?;

            let from = match bootstrap.core.previous_storage_path.clone() {
                Some(path) if path.exists() && path.is_dir() => path,
                _ => {
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
                    // Clin-native source: full directory copy
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

            bootstrap.clear_previous_storage_path();
            bootstrap.save()?;

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
            let keybinds = storage.load_keybinds_with_preset(config.core.keybind_preset);
            println!(
                "{} (preset: {})\n",
                console::bold("Current keybinds"),
                console::cyan(&config.core.keybind_preset.to_string())
            );
            println!("{}", console::section("List View"));
            println!(
                "  {:<18} {}",
                "Move up:",
                console::cyan(&keybinds.list_keys_display(ListAction::MoveUp))
            );
            println!(
                "  {:<18} {}",
                "Move down:",
                console::cyan(&keybinds.list_keys_display(ListAction::MoveDown))
            );
            println!(
                "  {:<18} {}",
                "Open:",
                console::cyan(&keybinds.list_keys_display(ListAction::Open))
            );
            println!(
                "  {:<18} {}",
                "Delete:",
                console::cyan(&keybinds.list_keys_display(ListAction::Delete))
            );
            println!(
                "  {:<18} {}",
                "Quit:",
                console::cyan(&keybinds.list_keys_display(ListAction::Quit))
            );
            println!(
                "  {:<18} {}",
                "Help:",
                console::cyan(&keybinds.list_keys_display(ListAction::Help))
            );
            println!(
                "  {:<18} {}",
                "Open location:",
                console::cyan(&keybinds.list_keys_display(ListAction::OpenLocation))
            );
            println!(
                "  {:<18} {}",
                "Cycle focus:",
                console::cyan(&keybinds.list_keys_display(ListAction::CycleFocus))
            );
            println!(
                "  {:<18} {}",
                "New from template:",
                console::cyan(&keybinds.list_keys_display(ListAction::NewFromTemplate))
            );
            println!("\n{}", console::section("Edit View"));
            println!(
                "  {:<18} {}",
                "Quit:",
                console::cyan(&keybinds.edit_keys_display(EditAction::Quit))
            );
            println!(
                "  {:<18} {}",
                "Back:",
                console::cyan(&keybinds.edit_keys_display(EditAction::Back))
            );
            println!(
                "  {:<18} {}",
                "Cycle focus:",
                console::cyan(&keybinds.edit_keys_display(EditAction::CycleFocus))
            );
            println!(
                "  {:<18} {}",
                "Select all:",
                console::cyan(&keybinds.edit_keys_display(EditAction::SelectAll))
            );
            println!(
                "  {:<18} {}",
                "Copy:",
                console::cyan(&keybinds.edit_keys_display(EditAction::Copy))
            );
            println!(
                "  {:<18} {}",
                "Cut:",
                console::cyan(&keybinds.edit_keys_display(EditAction::Cut))
            );
            println!(
                "  {:<18} {}",
                "Paste:",
                console::cyan(&keybinds.edit_keys_display(EditAction::Paste))
            );
            println!(
                "  {:<18} {}",
                "Undo:",
                console::cyan(&keybinds.edit_keys_display(EditAction::Undo))
            );
            println!(
                "  {:<18} {}",
                "Redo:",
                console::cyan(&keybinds.edit_keys_display(EditAction::Redo))
            );
            println!("\n{}", console::section("Help View"));
            println!(
                "  {:<18} {}",
                "Close:",
                console::cyan(&keybinds.help_keys_display(HelpAction::Close))
            );
            println!(
                "  {:<18} {}",
                "Scroll up:",
                console::cyan(&keybinds.help_keys_display(HelpAction::ScrollUp))
            );
            println!(
                "  {:<18} {}",
                "Scroll down:",
                console::cyan(&keybinds.help_keys_display(HelpAction::ScrollDown))
            );
            println!(
                "\n  {} {}",
                console::dim("Keybinds file:"),
                console::path(storage.keybinds_path())
            );
            Ok(())
        }
        KeybindsCmd::Export => {
            let storage = Storage::init()?;
            let keybinds = storage.load_keybinds();
            let toml = keybinds.to_toml();
            let content = toml::to_string_pretty(&toml)?;
            println!("{content}");
            Ok(())
        }
        KeybindsCmd::Reset => {
            let storage = Storage::init()?;
            let config = crate::config::ClinConfig::load().unwrap_or_default();
            let keybinds = config.core.keybind_preset.base_keybinds();
            storage.save_keybinds(&keybinds)?;
            println!("{}", console::success("Keybinds reset to defaults"));
            println!(
                "{} {}",
                console::dim("Keybinds file:"),
                console::path(storage.keybinds_path())
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
            let config = ClinConfig::load()?;
            println!("{}", toml::to_string_pretty(&config)?);
            Ok(())
        }
        ConfigCmd::Path => {
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
            let path = ClinConfig::config_path()?;
            if path.exists() {
                fs::remove_file(&path).context("failed to remove configuration file")?;
            }
            let _ = ClinConfig::load()?;
            println!(
                "{}",
                console::success("Configuration reset to default values.")
            );
            Ok(())
        }
    }
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

fn run_tui_session(app: &mut App) -> Result<()> {
    // Register signal handlers before entering raw mode. SIGHUP/SIGQUIT are
    // included on Unix so the terminal is restored deterministically (their
    // default disposition would kill the process before Drop runs).
    let _ = signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&SHOULD_EXIT));
    let _ = signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&SHOULD_EXIT));
    #[cfg(unix)]
    {
        let _ = signal_hook::flag::register(signal_hook::consts::SIGHUP, Arc::clone(&SHOULD_EXIT));
        let _ = signal_hook::flag::register(signal_hook::consts::SIGQUIT, Arc::clone(&SHOULD_EXIT));
    }

    // Spawn the background backup worker before entering the terminal.
    let (tx, done_rx) =
        crate::backup::worker::spawn(app.git_lock.clone(), app.backup_status.clone());
    app.backup_tx = Some(tx);

    // Run the TUI inside an inner block so `TerminalGuard` (raw mode + alt
    // screen) is dropped — restoring the terminal — BEFORE any blocking
    // quit-time backup. Any later signal/SIGKILL during the flush then leaves
    // the terminal clean.
    let result = {
        let _guard = TerminalGuard::enter(app.mouse_enabled)?;
        let backend = ratatui::backend::CrosstermBackend::new(io::stdout());
        let mut terminal = Terminal::new(backend).context("failed to create terminal")?;

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
        match done_rx.recv_timeout(crate::backup::worker::FLUSH_BOUND) {
            Ok(()) => println!("Done."),
            Err(_) => eprintln!("Backup still running in background; exiting."),
        }
        if let Some(msg) = app.backup_status.lock().take() {
            eprintln!("Backup warning: {msg}");
        }
    } else {
        drop(app.backup_tx.take());
        let _ = done_rx.recv_timeout(crate::backup::worker::FLUSH_BOUND);
    }

    match result {
        Ok(r) => r,
        Err(err) => std::panic::resume_unwind(err),
    }
}

fn run_app(
    terminal: &mut Terminal<ratatui::backend::CrosstermBackend<Stdout>>,
    app: &mut App,
) -> Result<()> {
    let mut focus = EditFocus::Body;
    let mut mouse_selecting = false;
    let mut mouse_dragged = false;

    // Start background note load for deferred startup
    let load_rx = if !app.initial_load_done && app.notes.is_empty() {
        Some(app.start_background_load())
    } else {
        None
    };

    while !app.should_quit {
        // Check for external SIGINT/SIGTERM
        if SHOULD_EXIT.load(Ordering::Acquire) {
            app.should_quit = true;
            break;
        }

        // Drain background load batches (non-blocking)
        if let Some(ref rx) = load_rx
            && !app.initial_load_done
        {
            let mut did_work = false;
            while let Ok(batch) = rx.try_recv() {
                did_work = true;
                app.merge_loaded(batch);
            }
            if did_work {
                app.needs_full_redraw = true;
            }
        }

        if app.mode == ViewMode::Graph {
            let mut config = match ClinConfig::load() {
                Ok(c) => c,
                Err(e) => {
                    app.set_temporary_status(&format!("Config error: {e}"));
                    ClinConfig::default()
                }
            };

            match crate::graf::app::run_graf_view(
                terminal,
                app.storage.clone(),
                &mut config,
                &app.keybinds,
                &mut app.seq_matcher,
            ) {
                Ok(crate::graf::app::GrafResult::NoteOpened(note_id)) => {
                    app.mode = ViewMode::List;
                    app.reload_theme();
                    app.open_note_from_graph(&note_id);
                }
                Ok(crate::graf::app::GrafResult::OpenHelp) => {
                    app.reload_theme();
                    app.return_mode = Some(ViewMode::Graph);
                    app.open_help_page_with_tab(crate::app::HelpTab::Graph);
                }
                _ => {
                    app.mode = app.return_mode.take().unwrap_or(ViewMode::List);
                    app.reload_theme();
                }
            }

            if let Err(e) = config.save() {
                app.set_temporary_status(&format!("Failed to save config: {e}"));
            }
            app.config = config;
            app.needs_full_redraw = true;
            terminal.clear()?;
            continue;
        }
        if app.mode == ViewMode::Backup {
            let config = app.config.clone();
            let vault_path = config
                .effective_storage_path()
                .unwrap_or_else(|_| PathBuf::from("."));

            let _ = crate::backup::app::run_backup_view(
                terminal,
                vault_path,
                &config,
                &app.keybinds,
                &app.app_theme,
                app.git_lock.clone(),
                &mut app.seq_matcher,
            );

            app.mode = app.return_mode.take().unwrap_or(ViewMode::List);
            app.reload_theme();
            app.needs_full_redraw = true;
            terminal.clear()?;
            app.reload_config();
            continue;
        }
        if app.mode == ViewMode::ContentTree {
            let note_id = app.get_selected_note_id();
            match crate::content_tree::app::run_content_tree_view(
                terminal,
                app.storage.clone(),
                note_id,
                &app.keybinds,
                app.app_theme.clone(),
                &mut app.seq_matcher,
            ) {
                Ok(crate::content_tree::app::ContentTreeResult::Back) => {
                    app.mode = app.return_mode.take().unwrap_or(ViewMode::List);
                    app.reload_theme();
                    app.needs_full_redraw = true;
                    terminal.clear()?;
                }
                Ok(crate::content_tree::app::ContentTreeResult::JumpToLine { note_id, line }) => {
                    app.reload_theme();
                    app.open_note_at_line(&note_id, Some(line)); // sets mode = Edit
                    app.needs_full_redraw = true;
                    terminal.clear()?;
                }
                Ok(crate::content_tree::app::ContentTreeResult::HelpRequested) => {
                    app.reload_theme();
                    app.return_mode = Some(ViewMode::ContentTree);
                    app.open_help_page_with_tab(crate::app::HelpTab::ContentTree);
                    app.needs_full_redraw = true;
                    terminal.clear()?;
                }
                Err(_) => {
                    app.mode = app.return_mode.take().unwrap_or(ViewMode::List);
                    app.reload_theme();
                    app.needs_full_redraw = true;
                    terminal.clear()?;
                }
            }
            continue;
        }

        if app.mode == ViewMode::Draw {
            let note_id = app.get_selected_note_id();

            let _ = crate::draw::app::run_draw_view(
                terminal,
                app.storage.clone(),
                &app.keybinds,
                note_id,
                app.app_theme.clone(),
                &mut app.seq_matcher,
            );
            app.close_draw_view();
            app.needs_full_redraw = true;
            terminal.clear()?;
            continue;
        }

        if app.mode == ViewMode::Canvas {
            let note_id = app.get_selected_note_id();
            match crate::pinstar::app::run_pinstar_view(
                terminal,
                app.storage.clone(),
                &app.keybinds,
                note_id,
                app.app_theme.clone(),
                &mut app.seq_matcher,
            ) {
                Ok(crate::pinstar::app::PinstarResult::HelpRequested) => {
                    app.reload_theme();
                    app.return_mode = Some(ViewMode::Canvas);
                    app.open_help_page_with_tab(crate::app::HelpTab::Canvas);
                }
                _ => {
                    app.close_canvas_view();
                }
            }
            app.needs_full_redraw = true;
            terminal.clear()?;
            continue;
        }

        app.tick_status();
        let failed = app.backup_status.lock().take();
        if let Some(msg) = failed {
            app.set_temporary_status(&format!("Backup failed: {msg}"));
        }

        if app.needs_full_redraw {
            terminal.clear()?;
            app.needs_full_redraw = false;
        }

        terminal.draw(|frame| draw_ui(frame, app, focus))?;

        let poll_timeout = if matches!(
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

        let need_redraw = app.poll_renderers();

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

        if need_redraw {
            terminal.draw(|frame| draw_ui(frame, app, focus))?;
        }

        if event::poll(poll_timeout).context("event poll failed")? {
            match event::read().context("failed to read event")? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match app.mode {
                    ViewMode::List => {
                        handle_list_keys(app, key);
                    }
                    ViewMode::Edit => {
                        handle_edit_keys(app, key, &mut focus);
                    }
                    ViewMode::Help => {
                        handle_help_keys(app, key);
                    }
                    ViewMode::Graph => {}
                    ViewMode::Draw => {}
                    ViewMode::Canvas => {}
                    ViewMode::Backup => {}
                    ViewMode::ContentTree => {}
                },
                Event::Mouse(mouse_event) if app.mode == ViewMode::List => {
                    let size = terminal.size().context("failed to get terminal size")?;
                    let area = Rect::new(0, 0, size.width, size.height);
                    handle_list_mouse(app, mouse_event, area);
                }
                Event::Mouse(mouse_event) if app.mode == ViewMode::Edit => {
                    let size = terminal.size().context("failed to get terminal size")?;
                    let area = Rect::new(0, 0, size.width, size.height);
                    handle_edit_mouse(
                        app,
                        mouse_event,
                        area,
                        &mut focus,
                        &mut mouse_selecting,
                        &mut mouse_dragged,
                    );
                }
                Event::Mouse(mouse_event) if app.mode == ViewMode::Help => {
                    let size = terminal.size().context("failed to get terminal size")?;
                    let area = Rect::new(0, 0, size.width, size.height);

                    let tab_bar_y = area.y;
                    if mouse_event.kind
                        == ratatui::crossterm::event::MouseEventKind::Down(
                            ratatui::crossterm::event::MouseButton::Left,
                        )
                        && mouse_event.row == tab_bar_y
                    {
                        let tabs: Vec<(&str, Option<&str>)> = crate::ui::HELP_TAB_NAMES
                            .iter()
                            .map(|&(l, g)| (l, Some(g)))
                            .collect();
                        let region = crate::ui::title_bar_tabs_region(area, "Help");
                        if let Some(i) = crate::ui::hit_test_tabs(
                            &tabs,
                            region.x,
                            region.width,
                            mouse_event.column,
                            app.config.ui.tab_icons_only,
                        ) {
                            app.switch_help_tab(crate::app::HelpTab::from_index(i));
                        }
                    } else if mouse_event.kind
                        == ratatui::crossterm::event::MouseEventKind::ScrollUp
                    {
                        app.help_scroll = app.help_scroll.saturating_sub(3);
                    } else if mouse_event.kind
                        == ratatui::crossterm::event::MouseEventKind::ScrollDown
                    {
                        let max_scroll = app
                            .list
                            .help_text_cache
                            .as_ref()
                            .map_or(0, |t| t.height().saturating_sub(5) as u16);
                        app.help_scroll = app.help_scroll.saturating_add(3).min(max_scroll);
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
                },
                Event::Resize(_, _) => {
                    terminal.autoresize()?;
                    app.needs_full_redraw = true;
                }
                _ => {}
            }
        }
    }
    Ok(())
}

pub use constants::*;
