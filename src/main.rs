mod config;
pub mod constants;
pub mod frontmatter;
mod keybinds;
mod templates;

use crate::config::BootstrapConfig;
use crate::keybinds::{EditAction, HelpAction, Keybinds, ListAction};

use std::borrow::Cow;
use std::fs;
use std::io::{self, Stdout};
use std::path::PathBuf;
use std::time::Duration;
use std::{env, process};

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
            let final_title = title
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| String::from("Untitled note"));

            let note = Note {
                title: final_title.clone(),
                content,
                updated_at: now_unix_secs(),
                tags: Vec::new(),
            };

            let id = storage.new_note_id();
            let saved_id = storage.save_note(&id, &note, true)?;
            println!("Saved quick note \"{final_title}\" as {saved_id}.clin");
            Ok(())
        }
        CliCommand::NewAndOpen { title, template } => {
            let storage = Storage::init()?;
            let settings = storage.load_settings();

            // Get content from template if specified
            let (final_title, content) = if let Some(template_name) = template {
                let template_manager = storage.template_manager();
                if let Ok(tmpl) = template_manager.load(&template_name) {
                    let rendered = tmpl.render();
                    let t = title
                        .or(rendered.title)
                        .map(|t| t.trim().to_string())
                        .filter(|t| !t.is_empty())
                        .unwrap_or_else(|| String::from("Untitled note"));
                    (t, rendered.content)
                } else {
                    eprintln!("Template '{template_name}' not found");
                    process::exit(1);
                }
            } else {
                let t = title
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .unwrap_or_else(|| String::from("Untitled note"));
                (t, String::new())
            };

            let note = Note {
                title: final_title.clone(),
                content,
                updated_at: now_unix_secs(),
                tags: Vec::new(),
            };

            let ext = if settings.encryption_enabled {
                "clin"
            } else {
                "md"
            };
            let base_id = storage.new_note_id();
            let id = format!("{base_id}.{ext}");
            let saved_id = storage.save_note(&id, &note, settings.encryption_enabled)?;

            let mut app = App::new(storage)?;
            if let Some(v_idx) = app.visual_list.iter().position(|v| {
                if let crate::app::VisualItem::Note { id: v_id, .. } = v {
                    v_id == &saved_id
                } else {
                    false
                }
            }) {
                app.visual_index = v_idx;
                app.open_selected();
            } else {
                app.open_note_by_title(&final_title);
            }

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
        // Storage path commands
        CliCommand::ShowStoragePath => {
            let bootstrap = BootstrapConfig::load()?;
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
            let mut bootstrap = BootstrapConfig::load()?;
            let old_path = bootstrap.effective_storage_path()?;

            // Validate path
            if !path.is_absolute() {
                anyhow::bail!("Storage path must be absolute: {}", path.display());
            }

            // Create directory if it doesn't exist
            fs::create_dir_all(&path)
                .with_context(|| format!("failed to create directory: {}", path.display()))?;

            bootstrap.set_storage_path(path.clone());
            bootstrap.save()?;

            println!("Storage path set to: {}", path.display());

            // Offer to migrate
            if old_path.exists() && old_path != path {
                println!("\nMigrate existing data from {}?", old_path.display());
                println!("Run: clin --migrate-storage");
            }

            Ok(())
        }
        CliCommand::ResetStoragePath => {
            let mut bootstrap = BootstrapConfig::load()?;
            bootstrap.reset_storage_path();
            bootstrap.save()?;
            let default = BootstrapConfig::default_storage_path()?;
            println!("Storage path reset to default: {}", default.display());
            Ok(())
        }
        // Keybind commands
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
        // Template commands
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
            // Check for --template flag
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
        // Storage path commands
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
        // Keybind commands
        "--keybinds" => Ok(CliCommand::ShowKeybinds),
        "--export-keybinds" => Ok(CliCommand::ExportKeybinds),
        "--reset-keybinds" => Ok(CliCommand::ResetKeybinds),
        // Template commands
        "--list-templates" => Ok(CliCommand::ListTemplates),
        "--create-example-templates" => Ok(CliCommand::CreateExampleTemplates),
        unknown => anyhow::bail!("unknown argument: {unknown}. Use clin -h for help."),
    }
}

fn print_cli_help() {
    println!(
        "clin - Encrypted terminal note-taking app

USAGE:
  clin                        Launch interactive app
  clin -n [TITLE]             Create a new note and open it
  clin -n --template <NAME> [TITLE]
                              Create a note from template
  clin -q <CONTENT> [TITLE]   Create a quick note and exit
  clin -e <TITLE>             Open a specific note by title
  clin -l                     List note titles
  clin -h                     Show this help

CONFIGURATION:
  --storage-path              Show current storage path
  --set-storage-path <PATH>   Set custom storage path
  --reset-storage-path        Reset to default storage path

KEYBINDS:
  --keybinds                  Show current keybindings
  --export-keybinds           Export keybinds as TOML
  --reset-keybinds            Reset keybinds to defaults

TEMPLATES:
  --list-templates            List available templates
  --create-example-templates  Create example templates
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
    let mut should_quit = false;
    let mut focus = EditFocus::Body;
    let mut mouse_selecting = false;
    let mut mouse_dragged = false;

    while !should_quit {
        app.tick_status();
        terminal.draw(|frame| draw_ui(frame, app, focus))?;

        if event::poll(Duration::from_millis(200)).context("event poll failed")? {
            match event::read().context("failed to read event")? {
                Event::Key(key) if key.kind == KeyEventKind::Press => match app.mode {
                    ViewMode::List => {
                        if handle_list_keys(app, key) {
                            should_quit = true;
                        }
                    }
                    ViewMode::Edit => {
                        if handle_edit_keys(app, key, &mut focus) {
                            should_quit = true;
                        }
                    }
                    ViewMode::Help => {
                        handle_help_keys(app, key);
                    }
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
                    if mouse_event.kind == ratatui::crossterm::event::MouseEventKind::ScrollUp {
                        app.help_scroll = app.help_scroll.saturating_sub(3);
                    } else if mouse_event.kind
                        == ratatui::crossterm::event::MouseEventKind::ScrollDown
                    {
                        let max_scroll = app
                            .help_text_cache
                            .as_ref()
                            .map_or(0, |t| t.height().saturating_sub(5) as u16);
                        app.help_scroll = app.help_scroll.saturating_add(3).min(max_scroll);
                    }
                }
                Event::Paste(data) if app.mode == ViewMode::Edit => match focus {
                    EditFocus::Title => {
                        let normalized = data.replace(['\r', '\n'], " ");
                        app.title_editor.insert_str(normalized);
                        app.status = Cow::Borrowed("Pasted title text");
                    }
                    EditFocus::Body => {
                        app.editor.insert_str(data);
                        app.status = Cow::Borrowed("Pasted content");
                    }
                    EditFocus::EncryptionToggle => {}
                },
                _ => {}
            }
        }
    }

    Ok(())
}

pub use constants::*;
