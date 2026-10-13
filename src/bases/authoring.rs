//! Native dialogs delegate YAML mutation to lossless BaseDocument and vault writes to guarded storage.

use super::{
    edits::{self, Transaction},
    format::{BaseDocument, Layout},
    state::BasesState,
    write::{Operation, Outcome, WriteTask},
};
use crate::{
    app::App,
    frontmatter::properties::{PropertyDocument, parse_mapping},
    storage::documents::DocumentSnapshot,
};
use anyhow::{Context, Result, bail};
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{List, ListItem, ListState},
};
use ratatui_textarea::TextArea;
use serde_yaml_ng::Value;

#[derive(Clone, Copy)]
pub enum Command {
    View(&'static str),
    GlobalFilter,
    Formula,
    Summary,
    Label,
    AddView,
    RemoveView,
    NewNote,
    Export,
}
pub const COMMANDS: &[(Command, &str, &str)] = &[
    (Command::View("name"), "Rename view", "View name"),
    (
        Command::View("type"),
        "Layout",
        "table, list, cards or kanban",
    ),
    (
        Command::GlobalFilter,
        "Global filter",
        "Expression; blank removes filter",
    ),
    (
        Command::View("filters"),
        "View filter",
        "Expression; blank removes filter",
    ),
    (
        Command::View("order"),
        "Displayed properties",
        "YAML list: [file.name, status, formula.Total]",
    ),
    (
        Command::View("sort"),
        "Row sorting",
        "YAML list: [{property: score, direction: DESC}]",
    ),
    (
        Command::View("groupBy"),
        "Grouping",
        "YAML mapping: {property: status, direction: ASC}",
    ),
    (
        Command::View("groupOrder"),
        "Manual group order",
        "YAML list: [Open, Done, null]; blank = automatic",
    ),
    (
        Command::View("limit"),
        "Row limit",
        "Non-negative integer; blank = unlimited",
    ),
    (
        Command::View("summaries"),
        "Column summaries",
        "YAML mapping: {score: Average}",
    ),
    (
        Command::Formula,
        "Formula",
        "name = expression; blank expression removes formula",
    ),
    (
        Command::Summary,
        "Custom summary",
        "name = expression; blank expression removes summary",
    ),
    (Command::Label, "Property label", "property = display name"),
    (
        Command::AddView,
        "Add view",
        "layout = name; layout: table, list, cards, kanban",
    ),
    (
        Command::RemoveView,
        "Remove current view",
        "Type DELETE to confirm; last view cannot be removed",
    ),
    (
        Command::NewNote,
        "New note",
        "Vault-relative Markdown path; existing files never overwritten",
    ),
    (
        Command::Export,
        "Export visible rows",
        "Vault-relative .csv or .md path; existing files never overwritten",
    ),
];
pub enum EditIntent {
    Edit,
    Clear,
    Paste(String),
    Move(String),
}
pub enum Target {
    Cell {
        snapshots: Vec<DocumentSnapshot>,
        key: String,
        clear: bool,
    },
    Config(Command),
    NewNote,
    Export,
    MoveGroup,
}
pub enum Dialog {
    Menu {
        selected: usize,
    },
    Prompt {
        target: Target,
        title: String,
        hint: String,
        input: Box<TextArea<'static>>,
    },
}
impl Dialog {
    pub fn prompt(target: Target, title: &str, hint: &str, value: &str) -> Self {
        Self::Prompt {
            target,
            title: title.into(),
            hint: hint.into(),
            input: Box::new(TextArea::new(
                value.lines().map(str::to_owned).collect::<Vec<_>>(),
            )),
        }
    }
    pub fn render(&mut self, frame: &mut Frame, area: Rect, app: &App) {
        match self {
            Self::Menu { selected } => {
                let inner = crate::ui::draw_popup_frame(
                    frame,
                    area,
                    "Bases configuration",
                    crate::ui::PopupSize::Large,
                    crate::ui::PopupHints::Text("Enter select · Esc cancel"),
                    &app.app_theme,
                );
                let list = List::new(COMMANDS.iter().map(|(_, label, _)| ListItem::new(*label)))
                    .highlight_style(
                        Style::default()
                            .fg(app.app_theme.highlight_fg)
                            .bg(app.app_theme.highlight_bg),
                    );
                frame.render_stateful_widget(
                    list,
                    inner,
                    &mut ListState::default().with_selected(Some(*selected)),
                );
            }
            Self::Prompt {
                title, hint, input, ..
            } => {
                let inner = crate::ui::draw_popup_frame(
                    frame,
                    area,
                    title,
                    crate::ui::PopupSize::Medium,
                    crate::ui::PopupHints::Text("Enter save · Esc cancel · Ctrl+v paste"),
                    &app.app_theme,
                );
                if inner.height > 0 {
                    frame.render_widget(
                        ratatui::widgets::Paragraph::new(hint.as_str())
                            .style(Style::default().fg(app.app_theme.muted)),
                        Rect::new(inner.x, inner.y, inner.width, 1),
                    );
                }
                input.set_style(app.app_theme.bg_style().fg(app.app_theme.text));
                input.set_cursor_style(
                    Style::default()
                        .fg(app.app_theme.highlight_fg)
                        .bg(app.app_theme.highlight_bg),
                );
                frame.render_widget(
                    &**input,
                    Rect::new(
                        inner.x,
                        inner.y.saturating_add(1),
                        inner.width,
                        inner.height.saturating_sub(1),
                    ),
                );
            }
        }
    }
}
fn yaml_value(source: &str) -> Result<Value> {
    let mapping = parse_mapping(&format!("value: {source}"))?;
    if mapping.len() != 1 {
        bail!("input must contain one YAML value");
    }
    mapping.get("value").cloned().context("missing YAML value")
}
fn pair(source: &str) -> Result<(&str, &str)> {
    let (name, value) = source.split_once('=').context("expected name = value")?;
    let name = name.trim();
    if name.is_empty() {
        bail!("name cannot be empty");
    }
    Ok((name, value.trim()))
}

pub fn configure(
    document: &BaseDocument,
    view: usize,
    command: Command,
    source: &str,
) -> Result<BaseDocument> {
    let document = match command {
        Command::View(key) => {
            let value = if source.trim().is_empty() {
                None
            } else if matches!(key, "name" | "type" | "filters") {
                Some(Value::String(source.into()))
            } else {
                Some(yaml_value(source)?)
            };
            document.set_view_field(view, key, value.as_ref())?
        }
        Command::GlobalFilter => document.set_global_field(
            "filters",
            (!source.trim().is_empty())
                .then(|| Value::String(source.into()))
                .as_ref(),
        )?,
        Command::Formula => {
            let (name, expression) = pair(source)?;
            document.set_formula(name, (!expression.is_empty()).then_some(expression))?
        }
        Command::Summary | Command::Label => {
            let (name, value) = pair(source)?;
            let field = if matches!(command, Command::Summary) {
                "summaries"
            } else {
                "properties"
            };
            let mut mapping = parse_mapping(document.source())?
                .get(field)
                .and_then(Value::as_mapping)
                .cloned()
                .unwrap_or_default();
            if matches!(command, Command::Summary) {
                if value.is_empty() {
                    mapping.remove(Value::String(name.into()));
                } else {
                    mapping.insert(Value::String(name.into()), Value::String(value.into()));
                }
            } else {
                let mut settings = mapping
                    .get(name)
                    .and_then(Value::as_mapping)
                    .cloned()
                    .unwrap_or_default();
                if value.is_empty() {
                    settings.remove(Value::String("displayName".into()));
                } else {
                    settings.insert(
                        Value::String("displayName".into()),
                        Value::String(value.into()),
                    );
                }
                mapping.insert(Value::String(name.into()), Value::Mapping(settings));
            }
            document.set_global_field(field, Some(&Value::Mapping(mapping)))?
        }
        Command::AddView => {
            let (layout, name) = pair(source)?;
            let layout = match layout {
                "table" => Layout::Table,
                "list" => Layout::List,
                "cards" => Layout::Cards,
                "kanban" => Layout::Kanban,
                _ => bail!("choose table, list, cards or kanban"),
            };
            document.add_view(&layout, name)?
        }
        Command::RemoveView => {
            if source != "DELETE" {
                bail!("type DELETE to remove view");
            }
            document.remove_view(view)?
        }
        Command::NewNote | Command::Export => bail!("not a configuration field"),
    };
    for (index, view) in document.config.views.iter().enumerate() {
        if view.layout.supported() {
            super::query::Query::compile(&document, index)?;
        }
    }
    Ok(document)
}

impl BasesState {
    pub fn open_row(&mut self, app: &mut App) -> Result<()> {
        self.ensure_idle()?;
        let loaded = self.loaded.as_ref().context("base has not loaded")?;
        let row = self.row().context("select a note row")?;
        let value = loaded
            .columns
            .get(self.column)
            .and_then(|column| row.values.get(column));
        let id = match value {
            Some(super::value::Value::Link(link)) => loaded
                .metadata
                .resolve_link(&link.source, &link.target.display())
                .context("link is unresolved in public dataset")?
                .id
                .clone(),
            Some(super::value::Value::File(id)) => id.clone(),
            _ => row.id.clone(),
        };
        self.write_task = Some(WriteTask::spawn(
            app.storage.clone(),
            &app.notes_worker_pool,
            Operation::Open {
                id,
                metadata: loaded.metadata.clone(),
            },
        ));
        Ok(())
    }
    pub fn start_edit(&mut self, app: &mut App, intent: EditIntent) -> Result<()> {
        self.ensure_idle()?;
        let loaded = self.loaded.as_ref().context("base has not loaded")?;
        let column = match &intent {
            EditIntent::Move(_) => loaded.document.config.views[loaded.view]
                .group_by
                .as_ref()
                .context("view has no groupBy property")?
                .property
                .clone(),
            _ => loaded
                .columns
                .get(self.column)
                .context("no property selected")?
                .clone(),
        };
        edits::property_key(&column)?;
        let ids = if self.selected_ids.is_empty() {
            vec![self.row().context("select a note row")?.id.clone()]
        } else {
            self.result()
                .context("query unavailable")?
                .rows
                .iter()
                .filter(|row| self.selected_ids.contains(&row.id))
                .map(|row| row.id.clone())
                .collect()
        };
        self.edit_intent = Some(intent);
        self.write_task = Some(WriteTask::spawn(
            app.storage.clone(),
            &app.notes_worker_pool,
            Operation::Prepare {
                metadata: loaded.metadata.clone(),
                ids,
                column,
            },
        ));
        Ok(())
    }
    fn ensure_idle(&self) -> Result<()> {
        if self.write_task.is_some() {
            bail!("Bases write operation in progress");
        }
        if self.loading {
            bail!("wait for Bases query to finish");
        }
        Ok(())
    }
    pub fn replay(&mut self, app: &mut App, forward: bool) -> Result<()> {
        self.ensure_idle()?;
        let transaction = if forward {
            self.redo.last()
        } else {
            self.undo.last()
        }
        .context("no history available")?
        .clone();
        self.write_task = Some(WriteTask::spawn(
            app.storage.clone(),
            &app.notes_worker_pool,
            Operation::Replay {
                transaction,
                forward,
            },
        ));
        Ok(())
    }
    fn remember(&mut self, transaction: Transaction) {
        if transaction.changes.is_empty() {
            return;
        }
        self.redo.clear();
        self.undo.push(transaction);
        while self.undo.iter().map(Transaction::bytes).sum::<usize>() > edits::MAX_HISTORY_BYTES
            || self.undo.len() > 64
        {
            self.undo.remove(0);
        }
    }
    pub fn poll_write(&mut self, app: &mut App) -> bool {
        let Some(result) = self.write_task.as_ref().and_then(WriteTask::poll) else {
            return false;
        };
        self.write_task = None;
        match result {
            Err(error) => {
                self.edit_intent = None;
                app.set_temporary_status(&format!("Bases: {error:#}"));
            }
            Ok(Outcome::Prepared { snapshots, key }) => {
                let intent = self.edit_intent.take().unwrap_or(EditIntent::Edit);
                let initial = snapshots
                    .first()
                    .and_then(|snapshot| PropertyDocument::parse(snapshot.source()).ok())
                    .and_then(|doc| doc.properties.get(&key).cloned())
                    .map_or_else(String::new, |value| {
                        value.as_str().map_or_else(
                            || serde_json::to_string(&value).unwrap_or_default(),
                            str::to_owned,
                        )
                    });
                let target = Target::Cell {
                    snapshots,
                    key: key.clone(),
                    clear: matches!(intent, EditIntent::Clear),
                };
                match intent {
                    EditIntent::Paste(source) | EditIntent::Move(source) => {
                        if let Err(error) = self.submit(app, &target, &source) {
                            app.set_temporary_status(&format!("Bases: {error:#}"));
                        }
                    }
                    EditIntent::Clear => {
                        self.dialog = Some(Dialog::prompt(
                            target,
                            &format!("Clear {key}"),
                            "Type CLEAR; removes property from selected notes",
                            "",
                        ))
                    }
                    EditIntent::Edit => {
                        self.dialog = Some(Dialog::prompt(
                            target,
                            &format!("Edit {key}"),
                            "Declared text/date: literal text; other types: YAML value",
                            &initial,
                        ))
                    }
                }
            }
            Ok(Outcome::Changed(transaction)) => {
                self.remember(transaction);
                self.refresh(app, true);
                app.request_notes_reconcile();
                app.set_temporary_status_static("Properties saved");
            }
            Ok(Outcome::Replayed {
                transaction,
                forward,
            }) => {
                if forward {
                    self.redo.pop();
                    self.undo.push(transaction);
                } else {
                    self.undo.pop();
                    self.redo.push(transaction);
                }
                self.named_view = None;
                self.refresh(app, true);
                app.request_notes_reconcile();
                app.set_temporary_status_static("History applied");
            }
            Ok(Outcome::Saved(change)) => {
                self.remember(Transaction {
                    changes: vec![change],
                });
                self.refresh(app, false);
                app.request_notes_reconcile();
                app.set_temporary_status_static("Base configuration saved");
            }
            Ok(Outcome::Created(snapshot)) => {
                self.refresh(app, true);
                app.request_notes_reconcile();
                app.set_temporary_status(&format!("Created {}", snapshot.id()));
            }
            Ok(Outcome::Opened(snapshot)) => app.open_public_document(snapshot),
        }
        true
    }
    pub fn submit(&mut self, app: &mut App, target: &Target, source: &str) -> Result<()> {
        self.ensure_idle()?;
        let loaded = self.loaded.as_ref().context("base has not loaded")?;
        let operation = match target {
            Target::Cell {
                snapshots,
                key,
                clear,
            } => {
                let value = if *clear {
                    if source != "CLEAR" {
                        bail!("type CLEAR to remove property");
                    }
                    None
                } else {
                    let current = snapshots
                        .first()
                        .map(|snapshot| PropertyDocument::parse(snapshot.source()))
                        .transpose()?
                        .and_then(|doc| doc.properties.get(key).cloned());
                    Some(edits::parse_input(
                        &loaded.metadata.property_types,
                        key,
                        source,
                        current.as_ref(),
                    )?)
                };
                Operation::Commit {
                    snapshots: snapshots.clone(),
                    key: key.clone(),
                    value,
                }
            }
            Target::Config(command) => {
                let document = configure(&loaded.document, loaded.view, *command, source)?;
                self.named_view = document
                    .config
                    .views
                    .get(loaded.view)
                    .map(|view| view.name.clone());
                Operation::Save {
                    baseline: loaded.snapshot.clone(),
                    source: document.source().into(),
                }
            }
            Target::MoveGroup => {
                self.start_edit(app, EditIntent::Move(source.into()))?;
                return Ok(());
            }
            Target::NewNote => {
                if !source.ends_with(".md") {
                    bail!("new note path must end in .md");
                }
                Operation::Create {
                    id: source.into(),
                    source: String::new(),
                }
            }
            Target::Export => {
                let result = loaded
                    .result
                    .as_ref()
                    .map_err(|error| anyhow::anyhow!("{error:#}"))?;
                if !source.ends_with(".md") && !source.ends_with(".csv") {
                    bail!("export path must end in .csv or .md");
                }
                Operation::Export {
                    id: source.into(),
                    columns: loaded.columns.clone(),
                    result: result.clone(),
                    markdown: source.ends_with(".md"),
                }
            }
        };
        self.write_task = Some(WriteTask::spawn(
            app.storage.clone(),
            &app.notes_worker_pool,
            operation,
        ));
        Ok(())
    }
    pub fn dialog_event(&mut self, event: &Event, app: &mut App) -> bool {
        let Some(mut dialog) = self.dialog.take() else {
            return false;
        };
        let mut keep = true;
        if let Event::Key(key) = event
            && key.kind != KeyEventKind::Release
        {
            match (&mut dialog, key.code) {
                (_, KeyCode::Esc) => keep = false,
                (Dialog::Menu { selected }, KeyCode::Up | KeyCode::Char('k')) => {
                    *selected = selected.saturating_sub(1)
                }
                (Dialog::Menu { selected }, KeyCode::Down | KeyCode::Char('j')) => {
                    *selected = (*selected + 1).min(COMMANDS.len() - 1)
                }
                (Dialog::Menu { selected }, KeyCode::Enter) => {
                    let (command, title, hint) = COMMANDS[*selected];
                    let target = match command {
                        Command::NewNote => Target::NewNote,
                        Command::Export => Target::Export,
                        _ => Target::Config(command),
                    };
                    self.dialog = Some(Dialog::prompt(target, title, hint, ""));
                    keep = false;
                }
                (Dialog::Prompt { input, .. }, KeyCode::Enter)
                    if key.modifiers.contains(KeyModifiers::SHIFT) =>
                {
                    input.insert_newline();
                }
                (Dialog::Prompt { target, input, .. }, KeyCode::Enter) => {
                    match self.submit(app, target, &input.lines().join("\n")) {
                        Ok(()) => keep = false,
                        Err(error) => app.set_temporary_status(&format!("Bases: {error:#}")),
                    }
                }
                (Dialog::Prompt { input, .. }, KeyCode::Char('v'))
                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    if let Some(text) = crate::text_edit::read_system_clipboard() {
                        input.insert_str(text);
                    }
                }
                (Dialog::Prompt { input, .. }, _) => {
                    input.input(*key);
                }
                _ => {}
            }
        } else if let Event::Paste(text) = event
            && let Dialog::Prompt { input, .. } = &mut dialog
        {
            input.insert_str(text);
        }
        if keep {
            self.dialog = Some(dialog);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authoring_preserves_plugin_fields_and_rejects_invalid_expressions() {
        let doc=BaseDocument::parse("# retain\nplugin: {opaque: true}\nviews: [{type: table, name: A, pluginOption: retained}]\n").unwrap();
        let doc = configure(&doc, 0, Command::Formula, "Total = 2 + 3").unwrap();
        let doc = configure(
            &doc,
            0,
            Command::View("groupBy"),
            "{property: status, direction: ASC}",
        )
        .unwrap();
        assert!(doc.source().contains("# retain"));
        assert!(doc.source().contains("pluginOption: retained"));
        assert!(
            configure(
                &doc,
                0,
                Command::Formula,
                "Bad = fetch('https://example.com')"
            )
            .is_err()
        );
        assert!(configure(&doc, 0, Command::RemoveView, "DELETE").is_err());
        assert_eq!(
            configure(&doc, 0, Command::AddView, "kanban = Board")
                .unwrap()
                .config
                .views
                .len(),
            2
        );
    }
}
