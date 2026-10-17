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
    widgets::{List, ListItem},
};
use ratatui_textarea::TextArea;
use serde_yaml_ng::Value;

#[derive(Clone, Copy)]
pub enum Command {
    View(&'static str),
    Source,
    GlobalFilter,
    Formula,
    Summary,
    Label,
    AddView,
    RemoveView,
    NewNote,
    Export,
    Group(super::groups::GroupAction),
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
        Command::View("rowHeight"),
        "Table row height",
        "medium, tall or extra; blank = short",
    ),
    (
        Command::View("columnSize"),
        "Table column widths",
        "YAML pixel-width mapping: {file.name: 240, score: 120}; blank = default",
    ),
    (
        Command::View("markers"),
        "List markers",
        "bullet, number or none; blank = bullet",
    ),
    (
        Command::View("indentProperties"),
        "Indent list properties",
        "true or false; blank = false",
    ),
    (
        Command::View("separator"),
        "List property separator",
        "YAML text; blank or empty text = comma and space",
    ),
    (
        Command::View("cardSize"),
        "Card width",
        "Width in pixels; blank = default",
    ),
    (
        Command::View("columnWidth"),
        "Kanban column width",
        "Width in pixels; blank = default",
    ),
    (
        Command::View("image"),
        "Cover property",
        "Property name, e.g. note.cover or formula.Cover; blank = none",
    ),
    (
        Command::View("imageFit"),
        "Image fit",
        "cover or contain; blank = default",
    ),
    (
        Command::View("imageAspectRatio"),
        "Image aspect ratio",
        "Width / height; positive number; blank = 1",
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
    (
        Command::Group(super::groups::GroupAction::Add),
        "Add empty group",
        "YAML value for new group; null = None",
    ),
    (
        Command::Group(super::groups::GroupAction::Hide),
        "Hide focused group",
        "",
    ),
    (
        Command::Group(super::groups::GroupAction::Left),
        "Move focused group left",
        "",
    ),
    (
        Command::Group(super::groups::GroupAction::Right),
        "Move focused group right",
        "",
    ),
    (
        Command::Group(super::groups::GroupAction::Automatic),
        "Automatic group order / show all",
        "",
    ),
    (
        Command::Group(super::groups::GroupAction::HideAll),
        "Hide all groups",
        "",
    ),
    (
        Command::Source,
        "Edit Bases source",
        "Open public source for repair",
    ),
];
pub enum EditIntent {
    Edit,
    Clear,
    Paste(String),
    Move(String),
    Group {
        ids: Vec<String>,
        value: Option<Value>,
        metadata: std::sync::Arc<crate::storage::metadata::VaultMetadata>,
        source: String,
    },
}
pub enum Target {
    Cell {
        snapshots: Vec<DocumentSnapshot>,
        key: String,
        clear: bool,
    },
    Config {
        command: Command,
        baseline: DocumentSnapshot,
        view: usize,
    },
    NewNote {
        group: Option<(String, Option<Value>)>,
    },
    Export,
    MoveGroup,
    GroupOrder {
        action: super::groups::GroupAction,
        baseline: DocumentSnapshot,
    },
}
pub enum Dialog {
    Image {
        id: String,
        protocol: Box<ratatui_image::protocol::StatefulProtocol>,
    },
    Menu {
        selected: usize,
        offset: usize,
        area: Rect,
        bounds: Rect,
    },
    Prompt {
        target: Target,
        title: String,
        hint: String,
        input: Box<TextArea<'static>>,
        area: Rect,
        bounds: Rect,
        selection: crate::text_edit::MouseTextSelection,
    },
}
impl Dialog {
    pub fn prompt(target: Target, title: &str, hint: &str, value: &str) -> Self {
        Self::Prompt {
            target,
            area: Rect::default(),
            bounds: Rect::default(),
            selection: crate::text_edit::MouseTextSelection::default(),
            title: title.into(),
            hint: hint.into(),
            input: Box::new(TextArea::new(
                value.lines().map(str::to_owned).collect::<Vec<_>>(),
            )),
        }
    }
    pub fn render(&mut self, frame: &mut Frame, area: Rect, app: &App) {
        match self {
            Self::Image { id, protocol } => {
                let title = format!(
                    "Public image · {}",
                    crate::fsutil::sanitize_for_terminal(id)
                );
                let hints = format!(
                    "{} close",
                    app.keybinds
                        .display_bases(crate::keybinds::BasesAction::Back)
                );
                let inner = crate::ui::draw_popup_frame(
                    frame,
                    area,
                    &title,
                    crate::ui::PopupSize::Large,
                    crate::ui::PopupHints::Text(&hints),
                    &app.app_theme,
                );
                frame.render_stateful_widget(
                    ratatui_image::StatefulImage::default()
                        .resize(ratatui_image::Resize::Fit(None)),
                    inner,
                    &mut **protocol,
                );
            }
            Self::Menu {
                selected,
                offset,
                area: menu_area,
                bounds,
            } => {
                let inner = crate::ui::draw_popup_frame(
                    frame,
                    area,
                    "Bases configuration",
                    crate::ui::PopupSize::Large,
                    crate::ui::PopupHints::Text("Enter select · Esc cancel"),
                    &app.app_theme,
                );
                *menu_area = inner;
                *bounds = crate::ui::centered_rect(crate::ui::PopupSize::Large, area);
                let list = List::new(COMMANDS.iter().map(|(_, label, _)| ListItem::new(*label)))
                    .highlight_style(
                        Style::default()
                            .fg(app.app_theme.highlight_fg)
                            .bg(app.app_theme.highlight_bg),
                    );
                let mut state = crate::ui::list_state_selected(Some(*selected), *offset);
                frame.render_stateful_widget(list, inner, &mut state);
                *offset = state.offset();
            }
            Self::Prompt {
                title,
                hint,
                input,
                area: input_area,
                bounds,
                ..
            } => {
                let inner = crate::ui::draw_popup_frame(
                    frame,
                    area,
                    title,
                    crate::ui::PopupSize::Medium,
                    crate::ui::PopupHints::Text("Enter save · Esc cancel · Ctrl+v paste"),
                    &app.app_theme,
                );
                *bounds = crate::ui::centered_rect(crate::ui::PopupSize::Medium, area);
                *input_area = Rect::new(
                    inner.x,
                    inner.y.saturating_add(1),
                    inner.width,
                    inner.height.saturating_sub(1),
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
                frame.render_widget(&**input, *input_area);
            }
        }
    }
}
pub(super) fn yaml_value(source: &str) -> Result<Value> {
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
    let original = document;
    let edited_view = view;
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
        Command::NewNote | Command::Export | Command::Group(_) | Command::Source => {
            bail!("not a configuration field")
        }
    };
    for (index, view) in document.config.views.iter().enumerate() {
        if view.layout.supported()
            && (index == edited_view
                || index >= original.config.views.len()
                || super::query::Query::compile(original, index).is_ok())
        {
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
        if let Some(value @ super::value::Value::Image(_)) = value
            && let super::media::CoverSource::Remote(url) =
                super::media::cover_source(value, &row.id, &loaded.metadata)?
        {
            if !app.config.features.images.is_enabled() || app.image_picker.is_none() {
                bail!("native image preview unavailable; enable images and restart");
            }
            self.write_task = Some(WriteTask::for_app(app, Operation::OpenRemoteImage { url }));
            return Ok(());
        }
        let id = match value {
            Some(super::value::Value::Link(link)) => loaded
                .metadata
                .resolve_link(&link.source, &link.target.display())
                .context("link is unresolved in public dataset")?
                .id
                .clone(),
            Some(super::value::Value::File(id)) => id.clone(),
            Some(super::value::Value::Image(target)) => loaded
                .metadata
                .resolve_link(&row.id, &target.display())
                .context("image is unresolved in public dataset")?
                .id
                .clone(),
            _ => row.id.clone(),
        };
        if loaded
            .metadata
            .file(&id)
            .is_some_and(|file| crate::storage::is_image_ext(&file.extension))
            && (!app.config.features.images.is_enabled() || app.image_picker.is_none())
        {
            bail!("native image preview unavailable; enable images and restart");
        }
        self.write_task = Some(WriteTask::for_app(
            app,
            Operation::Open {
                id,
                metadata: loaded.metadata.clone(),
            },
        ));
        Ok(())
    }
    pub(super) fn open_cover(&mut self, app: &mut App) -> Result<()> {
        self.ensure_idle()?;
        let loaded = self.loaded.as_ref().context("base has not loaded")?;
        let row = self.row().context("select a note row")?;
        let view = &loaded.document.config.views[loaded.view];
        if !matches!(view.layout, Layout::Cards | Layout::Kanban) {
            bail!("covers require Cards or Kanban layout");
        }
        let property = super::layout_options::CardOptions::parse(view, 8)?
            .image
            .context("view has no cover property")?;
        let value = row
            .values
            .get(&property)
            .context("cover property unavailable")?;
        let source = super::media::cover_source(value, &row.id, &loaded.metadata)?;
        if !app.config.features.images.is_enabled() || app.image_picker.is_none() {
            bail!("native image preview unavailable; enable images and restart");
        }
        let operation = match source {
            super::media::CoverSource::Public(id) => Operation::Open {
                id,
                metadata: loaded.metadata.clone(),
            },
            super::media::CoverSource::Remote(url) => Operation::OpenRemoteImage { url },
        };
        self.write_task = Some(WriteTask::for_app(app, operation));
        Ok(())
    }
    pub fn start_edit(&mut self, app: &mut App, intent: EditIntent) -> Result<()> {
        self.ensure_idle()?;
        let loaded = self.loaded.as_ref().context("base has not loaded")?;
        let column = match &intent {
            EditIntent::Move(_) | EditIntent::Group { .. } => loaded.document.config.views
                [loaded.view]
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
        if column != "file.folder" || !matches!(intent, EditIntent::Move(_)) {
            edits::property_key(&column)?;
        }
        let ids = if let EditIntent::Group { ids, .. } = &intent {
            ids.clone()
        } else if self.selected_ids.is_empty() {
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
        if column == "file.folder"
            && let EditIntent::Move(folder) = intent
        {
            return self.start_folder_move(app, ids, folder);
        }
        self.edit_intent = Some(intent);
        self.write_task = Some(WriteTask::for_app(
            app,
            Operation::Prepare {
                metadata: loaded.metadata.clone(),
                ids,
                column,
            },
        ));
        Ok(())
    }
    pub(super) fn ensure_idle(&self) -> Result<()> {
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
        for change in &transaction.changes {
            let baseline = if forward {
                &change.before
            } else {
                &change.after
            };
            if self.embedded_request.as_ref().is_some_and(|request| {
                request.host.as_deref() == Some(baseline.id())
                    && matches!(
                        request.definition,
                        super::embedding::Definition::Inline { .. }
                    )
            }) {
                app.ensure_inline_host_write(baseline)?;
            }
        }
        self.write_task = Some(WriteTask::for_app(
            app,
            Operation::Replay {
                transaction,
                forward,
            },
        ));
        Ok(())
    }
    fn remap_moved_focus(&mut self, transaction: &Transaction, forward: bool) {
        let focused = self.row().map(|row| row.id.clone());
        for plan in &transaction.moves {
            let (from, to) = if forward {
                (plan.source_id(), plan.destination_id())
            } else {
                (plan.destination_id(), plan.source_id())
            };
            if self.selected_ids.remove(from) {
                self.selected_ids.insert(to.to_owned());
            }
            if focused.as_deref() == Some(from) {
                self.focus_id = Some(to.to_owned());
            }
        }
    }
    fn remember(&mut self, transaction: Transaction) {
        if transaction.changes.is_empty() && transaction.moves.is_empty() {
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
        let complete = !matches!(&result, Ok(Outcome::Prepared { .. }));
        let palette_error = result.as_ref().err().map(|error| format!("{error:#}"));
        match result {
            Err(error) => {
                self.edit_intent = None;
                if self.creating {
                    self.loading = false;
                    self.error = Some(format!("Could not create Bases document: {error:#}"));
                } else {
                    self.refresh(app, true);
                }
                app.request_notes_reconcile();
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
                match intent {
                    EditIntent::Group {
                        value,
                        metadata,
                        source,
                        ..
                    } => {
                        self.write_task = Some(WriteTask::for_app(
                            app,
                            Operation::AssignGroup {
                                snapshots,
                                key,
                                value,
                                metadata,
                                source,
                            },
                        ));
                    }
                    EditIntent::Move(source) if source.trim().is_empty() => {
                        self.write_task = Some(WriteTask::for_app(
                            app,
                            Operation::Commit {
                                snapshots,
                                key,
                                value: None,
                            },
                        ));
                    }
                    EditIntent::Paste(source) | EditIntent::Move(source) => {
                        let target = Target::Cell {
                            snapshots,
                            key,
                            clear: false,
                        };
                        if let Err(error) = self.submit(app, &target, &source) {
                            app.finish_palette_bases_write(Some(format!("{error:#}")));
                            app.set_temporary_status(&format!("Bases: {error:#}"));
                        }
                    }
                    EditIntent::Clear => self.present_dialog(
                        app,
                        Dialog::prompt(
                            Target::Cell {
                                snapshots,
                                key: key.clone(),
                                clear: true,
                            },
                            &format!("Clear {key}"),
                            "Type CLEAR; removes property from selected notes",
                            "",
                        ),
                    ),
                    EditIntent::Edit => self.present_dialog(
                        app,
                        Dialog::prompt(
                            Target::Cell {
                                snapshots,
                                key: key.clone(),
                                clear: false,
                            },
                            &format!("Edit {key}"),
                            "Declared text/date: literal text; other types: YAML value",
                            &initial,
                        ),
                    ),
                }
            }
            Ok(Outcome::Changed(transaction)) => {
                self.remap_moved_focus(&transaction, true);
                let moved = !transaction.moves.is_empty();
                for change in &transaction.changes {
                    app.synchronize_public_change(&change.before, &change.after);
                }
                self.remember(transaction);
                self.refresh(app, true);
                app.request_notes_reconcile();
                app.set_temporary_status_static(if moved {
                    "Public files moved"
                } else {
                    "Properties saved"
                });
            }
            Ok(Outcome::Replayed {
                transaction,
                forward,
            }) => {
                for change in &transaction.changes {
                    let (before, after) = if forward {
                        (&change.before, &change.after)
                    } else {
                        (&change.after, &change.before)
                    };
                    app.synchronize_public_change(before, after);
                    if let Some(request) = &self.embedded_request
                        && matches!(
                            request.definition,
                            super::embedding::Definition::Inline { .. }
                        )
                        && request.host.as_deref() == Some(after.id())
                    {
                        match request.after_save(after.source()) {
                            Ok(updated) => self.embedded_request = Some(updated),
                            Err(error) => app
                                .set_temporary_status(&format!("Inline source refresh: {error:#}")),
                        }
                    }
                }
                self.remap_moved_focus(&transaction, forward);
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
            Ok(Outcome::Saved { change, embedded }) => {
                app.synchronize_public_change(&change.before, &change.after);
                self.embedded_request = embedded.map(|embedded| *embedded);
                self.remember(Transaction {
                    changes: vec![change],
                    moves: Vec::new(),
                });
                self.refresh(app, false);
                app.request_notes_reconcile();
                app.set_temporary_status_static("Base configuration saved");
            }
            Ok(Outcome::CreatedBase(snapshot)) => {
                self.creating = false;
                self.error = None;
                self.refresh(app, true);
                app.request_notes_reconcile();
                app.set_temporary_status(&format!("Created {}", snapshot.id()));
            }
            Ok(Outcome::Created(snapshot)) => {
                self.focus_id = Some(snapshot.id().to_owned());
                self.refresh(app, true);
                app.request_notes_reconcile();
                app.set_temporary_status(&format!("Created {}", snapshot.id()));
            }
            Ok(Outcome::Opened(snapshot)) => {
                if app.canvas_state.as_ref().is_some_and(|canvas| {
                    canvas.state.path == app.storage.notes_dir.join(snapshot.id())
                        && canvas.base_host_has_pending_changes()
                }) {
                    app.set_temporary_status_static(
                        "Finish and save the Canvas edit before opening its source",
                    );
                    return true;
                }
                if self.embedded_request.is_some()
                    && app.editor.editing_id.is_some()
                    && app.embedded_host_editor.is_none()
                {
                    app.embedded_host_editor = Some(Box::new(std::mem::take(&mut app.editor)));
                }
                app.open_public_document(snapshot);
            }
            Ok(Outcome::Image { id, image }) => {
                if app.config.features.images.is_enabled()
                    && let Some(picker) = &app.image_picker
                {
                    self.dialog = Some(Dialog::Image {
                        id,
                        protocol: Box::new(picker.new_resize_protocol(*image)),
                    });
                } else {
                    app.set_temporary_status("Native image preview unavailable");
                }
            }
        }
        if complete {
            app.finish_palette_bases_write(palette_error);
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
            Target::Config {
                command,
                baseline,
                view,
            } => {
                if loaded.snapshot.id() != baseline.id()
                    || loaded.snapshot.source() != baseline.source()
                {
                    bail!("Bases document changed; close dialog and reload before authoring");
                }
                let document = configure(&loaded.document, *view, *command, source)?;
                self.named_view = document
                    .config
                    .views
                    .get(*view)
                    .map(|view| view.name.clone());
                Operation::Save {
                    baseline: baseline.clone(),
                    source: loaded.configuration_source(app, &document)?,
                    embedded: self.embedded_request.clone(),
                }
            }
            Target::MoveGroup => {
                self.start_edit(app, EditIntent::Move(source.into()))?;
                return Ok(());
            }
            Target::NewNote { group } => Operation::CreateNote {
                id: source.into(),
                group: group.clone(),
                metadata: loaded.metadata.clone(),
                source: self.id.clone(),
            },
            Target::GroupOrder { action, baseline } => {
                if loaded.snapshot.id() != baseline.id()
                    || loaded.snapshot.source() != baseline.source()
                {
                    bail!("Bases document changed; close dialog and reload before authoring");
                }
                self.change_group_order(app, *action, source)?;
                return Ok(());
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
        self.write_task = Some(WriteTask::for_app(app, operation));
        Ok(())
    }
    pub fn dialog_event(&mut self, event: &Event, app: &mut App) -> bool {
        let Some(mut dialog) = self.dialog.take() else {
            return false;
        };
        let mut keep = true;
        if matches!(dialog, Dialog::Image { .. }) {
            if let Event::Key(key) = event
                && key.kind != KeyEventKind::Release
                && matches!(
                    app.keybinds.resolve_bases(
                        &mut self.matcher,
                        *key,
                        app.config.sequences_enabled(),
                        app.config.counts_enabled()
                    ),
                    crate::keybinds::MatchOutcome::Matched(crate::keybinds::BasesAction::Back, _)
                )
            {
                app.needs_full_redraw = true;
            } else {
                self.dialog = Some(dialog);
            }
            return true;
        }
        let mut activate = false;
        if let Event::Mouse(mouse) = event {
            use crossterm::event::{MouseButton, MouseEventKind};
            match &mut dialog {
                Dialog::Menu {
                    selected,
                    offset,
                    area,
                    bounds,
                } => match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left)
                        if !bounds.contains((mouse.column, mouse.row).into()) =>
                    {
                        return true;
                    }
                    MouseEventKind::Down(MouseButton::Left)
                        if area.contains((mouse.column, mouse.row).into()) =>
                    {
                        let index = *offset + usize::from(mouse.row - area.y);
                        if index < COMMANDS.len() {
                            activate = *selected == index;
                            *selected = index;
                        }
                    }
                    MouseEventKind::ScrollDown => {
                        *selected = (*selected + 3).min(COMMANDS.len() - 1)
                    }
                    MouseEventKind::ScrollUp => *selected = selected.saturating_sub(3),
                    _ => {}
                },
                Dialog::Prompt {
                    input,
                    area,
                    bounds,
                    selection,
                    ..
                } => match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left)
                        if !bounds.contains((mouse.column, mouse.row).into()) =>
                    {
                        return true;
                    }
                    MouseEventKind::Down(MouseButton::Left)
                        if area.contains((mouse.column, mouse.row).into()) =>
                    {
                        let (row, column) = crate::ui::get_textarea_scroll(input);
                        crate::events::move_textarea_cursor_to_mouse(
                            input,
                            *area,
                            mouse.column,
                            mouse.row,
                            row,
                            column,
                        );
                        selection.begin(&mut **input);
                    }
                    MouseEventKind::Drag(MouseButton::Left) if selection.active => {
                        selection.mark_drag();
                        let (row, column) = crate::ui::get_textarea_scroll(input);
                        crate::events::move_textarea_cursor_to_mouse(
                            input,
                            *area,
                            mouse.column,
                            mouse.row,
                            row,
                            column,
                        );
                    }
                    MouseEventKind::Up(MouseButton::Left) if selection.active => {
                        if let Some(notice) = selection.finish(&mut **input, true) {
                            app.set_temporary_status(notice);
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        let activate_event = Event::Key(crossterm::event::KeyEvent::new(
            KeyCode::Enter,
            KeyModifiers::NONE,
        ));
        let event = if activate { &activate_event } else { event };
        if let Event::Key(key) = event
            && key.kind != KeyEventKind::Release
        {
            match (&mut dialog, key.code) {
                (_, KeyCode::Esc) => keep = false,
                (Dialog::Menu { selected, .. }, KeyCode::Up | KeyCode::Char('k')) => {
                    *selected = selected.saturating_sub(1)
                }
                (Dialog::Menu { selected, .. }, KeyCode::Down | KeyCode::Char('j')) => {
                    *selected = (*selected + 1).min(COMMANDS.len() - 1)
                }
                (Dialog::Menu { selected, .. }, KeyCode::Enter) => {
                    if let Err(error) = self.run_configuration_command(app, *selected) {
                        app.set_temporary_status(&format!("Bases: {error:#}"));
                        self.dialog = Some(dialog);
                    }
                    return true;
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
