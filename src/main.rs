#![allow(dead_code)]

pub(crate) mod actions;
pub(crate) mod app_theme;
pub(crate) mod backup;
pub(crate) mod cli;
mod config;
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
pub(crate) mod palette;
pub(crate) mod pinstar;
pub(crate) mod popups;
pub(crate) mod preview;
pub(crate) mod sanitize;
pub(crate) mod snapshot;
mod templates;
pub(crate) mod text_edit;

use crate::config::ClinConfig;
use crate::keybinds::{EditAction, HelpAction, Keybinds, ListAction};

use std::borrow::Cow;
use std::fs;
use std::io::{self, Stdout, Write};
use std::path::PathBuf;
use std::time::Duration;
use std::{env, process};
use uuid::Uuid;

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
    let cli = parse_cli_command()?;

    match cli {
        CliCommand::Help => {
            print_cli_help();
            Ok(())
        }
        CliCommand::ListNoteTitles => {
            let storage = Storage::init()?;
            let mut app = App::new(storage)?;
            app.refresh_notes()?;
            for (index, note) in app.notes.iter().enumerate() {
                println!("{}. {}", index + 1, note.title);
            }
            Ok(())
        }
        CliCommand::QuickNote { content, title } => {
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

            println!("Created note: {}", final_title);

            Ok(())
        }
        CliCommand::NewAndOpen { title, template } => {
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
                            eprintln!("Failed to load template data: {tmpl_name}");
                            process::exit(1);
                        }
                    } else {
                        eprintln!("Template not found: {tmpl_name}");
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
        CliCommand::Run { edit_title } => {
            let storage = Storage::init()?;
            let mut app = App::new(storage)?;

            if let Some(title) = edit_title
                && !app.open_note_by_title(&title)
            {
                eprintln!("No note found with title: {title}");
                process::exit(1);
            }

            run_tui_session(&mut app)
        }

        CliCommand::ShowStoragePath => {
            let bootstrap = ClinConfig::load()?;
            let effective = bootstrap.effective_storage_path()?;
            println!("Storage path: {}", effective.display());
            if bootstrap.has_custom_storage_path() {
                println!("(custom path)");
            } else {
                println!("(default path)");
            }
            Ok(())
        }
        CliCommand::SetStoragePath { path } => {
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

            println!("Storage path set to: {}", path.display());

            if bootstrap.previous_storage_path.is_some() {
                println!("\nRun 'clin --migrate-storage' to migrate your existing data.");
            }

            Ok(())
        }
        CliCommand::ResetStoragePath => {
            let mut bootstrap = ClinConfig::load()?;
            bootstrap.reset_storage_path();
            bootstrap.save()?;
            let default = ClinConfig::default_storage_path()?;
            println!("Storage path reset to default: {}", default.display());
            Ok(())
        }
        CliCommand::MigrateStorage => {
            let mut bootstrap = ClinConfig::load()?;
            let to = bootstrap.effective_storage_path()?;

            let from = match bootstrap.previous_storage_path.clone() {
                Some(path) if path.exists() && path.is_dir() => path,
                _ => {
                    let default = ClinConfig::default_storage_path()?;
                    if default.exists() && default.is_dir() && default != to {
                        println!("No previous storage path recorded.");
                        println!("Found data at default location: {}", default.display());
                        print!("Migrate from there? [y/N]: ");
                        io::stdout().flush()?;

                        let mut input = String::new();
                        io::stdin().read_line(&mut input)?;
                        if !input.trim().eq_ignore_ascii_case("y") {
                            println!("Migration cancelled.");
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

            println!("Migrating data:");
            println!("  From: {}", from.display());
            println!("  To:   {}", to.display());
            println!();

            fs::create_dir_all(&to)
                .with_context(|| format!("failed to create destination: {}", to.display()))?;

            let mut migrated_count = 0;
            let mut skipped_count = 0;
            let mut conflict_action: Option<migration::ConflictAction> = None;

            let notes_src = from.join("notes");
            let notes_dst = to.join("notes");
            if notes_src.exists() && notes_src.is_dir() {
                fs::create_dir_all(&notes_dst)?;
                let (m, s, action) = migration::migrate_directory_with_conflict(
                    &notes_src,
                    &notes_dst,
                    conflict_action,
                )?;
                migrated_count += m;
                skipped_count += s;
                conflict_action = action;
            }

            let templates_src = from.join("templates");
            let templates_dst = to.join("templates");
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
            println!("Migration complete!");
            println!("  Migrated: {} items", migrated_count);
            if skipped_count > 0 {
                println!("  Skipped:  {} items", skipped_count);
            }
            println!();
            println!("Your old data remains at: {}", from.display());
            println!("You may delete it manually after verifying everything works.");

            Ok(())
        }

        CliCommand::ShowKeybinds => {
            let storage = Storage::init()?;
            let keybinds = storage.load_keybinds();
            println!("Current keybinds:\n");
            println!("[List View]");
            println!(
                "  Move up:        {}",
                keybinds.list_keys_display(ListAction::MoveUp)
            );
            println!(
                "  Move down:      {}",
                keybinds.list_keys_display(ListAction::MoveDown)
            );
            println!(
                "  Open:           {}",
                keybinds.list_keys_display(ListAction::Open)
            );
            println!(
                "  Delete:         {}",
                keybinds.list_keys_display(ListAction::Delete)
            );
            println!(
                "  Quit:           {}",
                keybinds.list_keys_display(ListAction::Quit)
            );
            println!(
                "  Help:           {}",
                keybinds.list_keys_display(ListAction::Help)
            );
            println!(
                "  Open location:  {}",
                keybinds.list_keys_display(ListAction::OpenLocation)
            );
            println!(
                "  Cycle focus:    {}",
                keybinds.list_keys_display(ListAction::CycleFocus)
            );
            println!(
                "  New from template: {}",
                keybinds.list_keys_display(ListAction::NewFromTemplate)
            );
            println!("\n[Edit View]");
            println!(
                "  Quit:           {}",
                keybinds.edit_keys_display(EditAction::Quit)
            );
            println!(
                "  Back:           {}",
                keybinds.edit_keys_display(EditAction::Back)
            );
            println!(
                "  Cycle focus:    {}",
                keybinds.edit_keys_display(EditAction::CycleFocus)
            );
            println!(
                "  Select all:     {}",
                keybinds.edit_keys_display(EditAction::SelectAll)
            );
            println!(
                "  Copy:           {}",
                keybinds.edit_keys_display(EditAction::Copy)
            );
            println!(
                "  Cut:            {}",
                keybinds.edit_keys_display(EditAction::Cut)
            );
            println!(
                "  Paste:          {}",
                keybinds.edit_keys_display(EditAction::Paste)
            );
            println!(
                "  Undo:           {}",
                keybinds.edit_keys_display(EditAction::Undo)
            );
            println!(
                "  Redo:           {}",
                keybinds.edit_keys_display(EditAction::Redo)
            );
            println!("\n[Help View]");
            println!(
                "  Close:          {}",
                keybinds.help_keys_display(HelpAction::Close)
            );
            println!(
                "  Scroll up:      {}",
                keybinds.help_keys_display(HelpAction::ScrollUp)
            );
            println!(
                "  Scroll down:    {}",
                keybinds.help_keys_display(HelpAction::ScrollDown)
            );
            println!("\nKeybinds file: {}", storage.keybinds_path().display());
            Ok(())
        }
        CliCommand::ExportKeybinds => {
            let storage = Storage::init()?;
            let keybinds = storage.load_keybinds();
            let toml = keybinds.to_toml();
            let content = toml::to_string_pretty(&toml)?;
            println!("{content}");
            Ok(())
        }
        CliCommand::ResetKeybinds => {
            let storage = Storage::init()?;
            let keybinds = Keybinds::default();
            storage.save_keybinds(&keybinds)?;
            println!("Keybinds reset to defaults");
            println!("Keybinds file: {}", storage.keybinds_path().display());
            Ok(())
        }

        CliCommand::ListTemplates => {
            let storage = Storage::init()?;
            let template_manager = storage.template_manager();
            let templates = template_manager.list()?;

            if templates.is_empty() {
                println!("No templates found.");
                println!("Templates directory: {}", storage.templates_dir.display());
                println!("\nRun 'clin --create-example-templates' to create example templates.");
            } else {
                println!("Available templates:\n");
                for (i, t) in templates.iter().enumerate() {
                    println!("  {}. {} ({})", i + 1, t.name, t.filename);
                }
                println!("\nTemplates directory: {}", storage.templates_dir.display());
            }
            Ok(())
        }
        CliCommand::CreateExampleTemplates => {
            let storage = Storage::init()?;
            let template_manager = storage.template_manager();
            template_manager.create_examples()?;
            println!(
                "Example templates created in: {}",
                storage.templates_dir.display()
            );

            let templates = template_manager.list()?;
            for t in templates {
                println!("  - {} ({})", t.name, t.filename);
            }
            Ok(())
        }
    }
}

fn parse_cli_command() -> Result<CliCommand> {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        return Ok(CliCommand::Run { edit_title: None });
    }

    match args[0].as_str() {
        "-h" | "--help" => Ok(CliCommand::Help),
        "-l" => Ok(CliCommand::ListNoteTitles),
        "-n" => {
            let mut title = None;
            let mut template = None;
            let mut i = 1;
            while i < args.len() {
                if args[i] == "--template" || args[i] == "-t" {
                    if i + 1 < args.len() {
                        template = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        anyhow::bail!("--template requires a template name");
                    }
                } else if title.is_none() {
                    title = Some(args[i..].join(" "));
                    break;
                } else {
                    i += 1;
                }
            }
            Ok(CliCommand::NewAndOpen { title, template })
        }
        "-q" => {
            if args.len() < 2 {
                anyhow::bail!("-q requires note content. Try: clin -q \"content\" [title]");
            }
            let content = args[1].clone();
            let title = if args.len() > 2 {
                Some(args[2..].join(" "))
            } else {
                None
            };
            Ok(CliCommand::QuickNote { content, title })
        }
        "-e" => {
            if args.len() < 2 {
                anyhow::bail!("-e requires a note title. Try: clin -e \"My Note\"");
            }
            Ok(CliCommand::Run {
                edit_title: Some(args[1..].join(" ")),
            })
        }

        "--storage-path" => Ok(CliCommand::ShowStoragePath),
        "--set-storage-path" => {
            if args.len() < 2 {
                anyhow::bail!("--set-storage-path requires a path");
            }
            Ok(CliCommand::SetStoragePath {
                path: PathBuf::from(&args[1]),
            })
        }
        "--reset-storage-path" => Ok(CliCommand::ResetStoragePath),
        "--migrate-storage" => Ok(CliCommand::MigrateStorage),

        "--keybinds" => Ok(CliCommand::ShowKeybinds),
        "--export-keybinds" => Ok(CliCommand::ExportKeybinds),
        "--reset-keybinds" => Ok(CliCommand::ResetKeybinds),

        "--list-templates" => Ok(CliCommand::ListTemplates),
        "--create-example-templates" => Ok(CliCommand::CreateExampleTemplates),
        unknown => anyhow::bail!("unknown argument: {unknown}. Use clin -h for help."),
    }
}

fn print_cli_help() {
    println!(
        "\x1b[1;32mclin\x1b[0m - Encrypted terminal note-taking app

\x1b[1;33mUSAGE:\x1b[0m
  clin [OPTIONS]

\x1b[1;33mNOTE OPERATIONS:\x1b[0m
  clin                        Launch interactive app
  \x1b[32m-n\x1b[0m \x1b[36m[TITLE]\x1b[0m                Create a new note and open it
  \x1b[32m-n\x1b[0m \x1b[32m-t, --template\x1b[0m \x1b[36m<NAME>\x1b[0m \x1b[36m[TITLE]\x1b[0m
                              Create a new note from a template
  \x1b[32m-q\x1b[0m \x1b[36m<CONTENT>\x1b[0m \x1b[36m[TITLE]\x1b[0m      Create a quick note and exit
  \x1b[32m-e\x1b[0m \x1b[36m<TITLE>\x1b[0m                Open a specific note by title
  \x1b[32m-l\x1b[0m                        List note titles
  \x1b[32m-h, --help\x1b[0m                Show this help message

\x1b[1;33mCONFIGURATION:\x1b[0m
  \x1b[32m--storage-path\x1b[0m            Show current storage path
  \x1b[32m--set-storage-path\x1b[0m \x1b[36m<PATH>\x1b[0m Set custom storage path
  \x1b[32m--reset-storage-path\x1b[0m      Reset to default storage path
  \x1b[32m--migrate-storage\x1b[0m         Migrate data from previous storage location

\x1b[1;33mKEYBINDS:\x1b[0m
  \x1b[32m--keybinds\x1b[0m                Show current keybindings
  \x1b[32m--export-keybinds\x1b[0m         Export keybinds as TOML
  \x1b[32m--reset-keybinds\x1b[0m          Reset keybinds to defaults

\x1b[1;33mTEMPLATES:\x1b[0m
  \x1b[32m--list-templates\x1b[0m          List available templates
  \x1b[32m--create-example-templates\x1b[0m Create example templates
"
    );
}

fn run_tui_session(app: &mut App) -> Result<()> {
    enable_raw_mode().context("failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableBracketedPaste
    )
    .context("failed to enter alternate screen")?;

    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("failed to create terminal")?;

    let run_result = {
        let mut terminal_safe = std::panic::AssertUnwindSafe(&mut terminal);
        let mut app_safe = std::panic::AssertUnwindSafe(&mut *app);
        let res = std::panic::catch_unwind(move || run_app(*terminal_safe, *app_safe));

        if app.mode == ViewMode::Edit {
            app.autosave();
        }

        match res {
            Ok(r) => r,
            Err(err) => std::panic::resume_unwind(err),
        }
    };

    disable_raw_mode().ok();
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        DisableBracketedPaste
    )
    .ok();
    terminal.show_cursor().ok();

    run_result
}

fn run_app(
    terminal: &mut Terminal<ratatui::backend::CrosstermBackend<Stdout>>,
    app: &mut App,
) -> Result<()> {
    let mut focus = EditFocus::Body;
    let mut mouse_selecting = false;
    let mut mouse_dragged = false;

    while !app.should_quit {
        if app.mode == ViewMode::Graph {
            let mut config = match ClinConfig::load() {
                Ok(c) => c,
                Err(e) => {
                    app.set_temporary_status(&format!("Config error: {}", e));
                    ClinConfig::default()
                }
            };

            match crate::graf::app::run_graf_view(
                terminal,
                app.storage.clone(),
                &mut config,
                &app.keybinds,
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
                app.set_temporary_status(&format!("Failed to save config: {}", e));
            }
            app.needs_full_redraw = true;
            terminal.clear()?;
            continue;
        }
        if app.mode == ViewMode::Backup {
            let config = ClinConfig::load().unwrap_or_default();
            let vault_path = config
                .effective_storage_path()
                .unwrap_or_else(|_| PathBuf::from("."));

            let _ = crate::backup::app::run_backup_view(
                terminal,
                vault_path,
                &config,
                &app.keybinds,
                &app.app_theme,
            );

            app.mode = app.return_mode.take().unwrap_or(ViewMode::List);
            app.reload_theme();
            app.needs_full_redraw = true;
            terminal.clear()?;
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
                        let tab_names = [
                            "Notes",
                            "Editor",
                            "Graph",
                            "Draw",
                            "Canvas",
                            "Backup",
                            "Templates",
                            "Content Tree",
                            "About",
                        ];
                        let mut tab_widths: [u16; 9] = [0; 9];
                        let mut total_width: u16 = 0;
                        for (i, name) in tab_names.iter().enumerate() {
                            tab_widths[i] = name.len() as u16 + 2;
                            total_width += tab_widths[i];
                            if i < tab_names.len() - 1 {
                                total_width += 3;
                            }
                        }
                        let start_x = area.x + (area.width.saturating_sub(total_width)) / 2;
                        let click_x = mouse_event.column;
                        if click_x >= start_x && click_x < start_x + total_width {
                            let mut offset = start_x;
                            for (i, tw) in tab_widths.iter().enumerate() {
                                if click_x < offset + tw {
                                    app.switch_help_tab(crate::app::HelpTab::from_index(i));
                                    break;
                                }
                                offset += tw + 3;
                            }
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
                _ => {}
            }
        }
    }

    if let Err(e) = app.try_auto_backup_on_quit() {
        eprintln!("clin: backup on quit failed: {e}");
    }
    Ok(())
}

pub use constants::*;
