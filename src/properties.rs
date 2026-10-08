use anyhow::{Context, Result, bail, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui_textarea::TextArea;
use serde_yaml_ng::Value;
use std::str::FromStr;
use yaml_edit::YamlFile;

use crate::app::{App, EditFocus, ViewMode};
use crate::app_theme::AppThemeColors;
use crate::frontmatter::{self, FrontmatterEdit};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PropertyType {
    String,
    Number,
    Boolean,
    Null,
    Yaml,
}
impl PropertyType {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::String => "String",
            Self::Number => "Number",
            Self::Boolean => "Boolean",
            Self::Null => "Null",
            Self::Yaml => "YAML",
        }
    }
    fn cycle(self, direction: isize) -> Self {
        let types = [
            Self::String,
            Self::Number,
            Self::Boolean,
            Self::Null,
            Self::Yaml,
        ];
        let index = types.iter().position(|value| *value == self).unwrap_or(0);
        types[(index as isize + direction).rem_euclid(5) as usize]
    }
    fn of(value: &Value) -> Self {
        match value {
            Value::String(_) => Self::String,
            Value::Number(_) => Self::Number,
            Value::Bool(_) => Self::Boolean,
            Value::Null => Self::Null,
            _ => Self::Yaml,
        }
    }
    fn encode(self, input: &str) -> Result<String> {
        match self {
            Self::String => Ok(serde_yaml_ng::to_string(&Value::String(input.into()))?),
            Self::Null => Ok("null".into()),
            Self::Yaml => Ok(input.into()), // Aliases are checked against the complete candidate.
            Self::Number | Self::Boolean => {
                let value: Value = serde_yaml_ng::from_str(input)?;
                ensure!(
                    matches!(
                        (self, &value),
                        (Self::Number, Value::Number(_)) | (Self::Boolean, Value::Bool(_))
                    ),
                    "Expected {}",
                    self.label()
                );
                Ok(serde_yaml_ng::to_string(&value)?)
            }
        }
    }
}

pub(crate) struct PropertyRow {
    pub(crate) key_yaml: String,
    pub(crate) key: Value,
    pub(crate) value_yaml: String,
    pub(crate) value: Value,
    pub(crate) kind: PropertyType,
    pub(crate) managed: bool,
}
impl PropertyRow {
    pub(crate) fn name(&self) -> &str {
        self.key.as_str().unwrap_or_else(|| self.key_yaml.trim())
    }
    pub(crate) fn summary(&self) -> &str {
        self.value
            .as_str()
            .unwrap_or_else(|| self.value_yaml.trim())
            .lines()
            .next()
            .unwrap_or("null")
    }
}

pub(crate) struct PropertyDialog {
    pub(crate) key_yaml: Option<String>,
    pub(crate) name: TextArea<'static>,
    pub(crate) value: TextArea<'static>,
    pub(crate) kind: PropertyType,
    pub(crate) control: usize, // Name / Type / Value / Apply
    pub(crate) error: Option<String>,
    pub(crate) rects: [Rect; 4],
}
impl PropertyDialog {
    fn advance(&mut self, direction: isize) {
        loop {
            self.control = (self.control as isize + direction).rem_euclid(4) as usize;
            if !(self.control == 0 && self.key_yaml.is_some()
                || self.control == 2 && self.kind == PropertyType::Null)
            {
                break;
            }
        }
    }
    fn edit(&self) -> Result<FrontmatterEdit> {
        let key_yaml = if let Some(key) = &self.key_yaml {
            key.clone()
        } else {
            let name = self.name.lines().join("\n");
            ensure!(
                !name.trim().is_empty() && !name.contains(['\r', '\n']),
                "Name must be nonempty and single-line"
            );
            serde_yaml_ng::to_string(&Value::String(name))?
        };
        Ok(FrontmatterEdit {
            key_yaml,
            value_yaml: Some(self.kind.encode(&self.value.lines().join("\n"))?),
        })
    }
}

pub(crate) enum PropertiesDialog {
    Edit(Box<PropertyDialog>),
    Delete {
        key_yaml: String,
        yes: bool,
        rects: [Rect; 2],
    },
}

#[derive(Default)]
pub(crate) struct PropertiesState {
    pub(crate) baseline: Option<String>,
    pub(crate) current: Option<String>,
    pub(crate) rows: Vec<PropertyRow>,
    pub(crate) selected: usize,
    pub(crate) scroll: usize,
    pub(crate) expanded: bool,
    pub(crate) focused: bool,
    pub(crate) pending: Vec<FrontmatterEdit>,
    pub(crate) revision: u64,
    pub(crate) error: Option<String>,
    pub(crate) dialog: Option<PropertiesDialog>,
    pub(crate) focus_request: Option<EditFocus>,
    pub(crate) last_click: Option<(usize, std::time::Instant)>,
    pub(crate) visible_rows: usize,
}
impl PropertiesState {
    pub(crate) fn load(header: Result<Option<String>>) -> Self {
        let mut state = Self::default();
        match header {
            Ok(header) => {
                state.baseline = header.clone();
                state.current = header;
                if let Err(error) = state.rebuild() {
                    state.error = Some(error.to_string());
                }
            }
            Err(error) => state.error = Some(error.to_string()),
        }
        state
    }
    fn rebuild(&mut self) -> Result<()> {
        let header = self.current.as_deref().unwrap_or("---\n---\n");
        let semantic = frontmatter::validate_header(header)?;
        let (Some(header), _) = frontmatter::split_header(header.as_bytes())? else {
            bail!("Missing header")
        };
        let opening = if header.starts_with("---\r\n") { 5 } else { 4 };
        let end = header.trim_end_matches(['\r', '\n']).len() - 3;
        let file = YamlFile::from_str(&header[opening..end])?;
        let mut rows = Vec::with_capacity(semantic.len());
        if let Some(mapping) = file.document().and_then(|document| document.as_mapping()) {
            for entry in mapping.entries() {
                let key_yaml = entry
                    .key_node()
                    .context("Unsupported property key")?
                    .to_string();
                let key: Value =
                    serde_yaml_ng::from_str(&key_yaml).context("Unsupported property key")?;
                let value = semantic
                    .get(&key)
                    .context("Unsupported property syntax")?
                    .clone();
                let value_node = entry.value_node();
                let value_yaml = value_node
                    .as_ref()
                    .map_or_else(|| "null".into(), ToString::to_string);
                let mut kind = PropertyType::of(&value);
                if value_node.is_some_and(|node| {
                    matches!(
                        node,
                        yaml_edit::YamlNode::Alias(_) | yaml_edit::YamlNode::TaggedNode(_)
                    )
                }) {
                    kind = PropertyType::Yaml;
                }
                rows.push(PropertyRow {
                    managed: frontmatter::is_managed_key(&key),
                    key_yaml,
                    key,
                    value_yaml,
                    value,
                    kind,
                });
            }
        }
        rows.retain(|row| !row.managed);
        self.rows = rows;
        self.selected = self.selected.min(self.rows.len()); // Add row follows entries.
        self.scroll = self.scroll.min(self.rows.len());
        self.error = None;
        Ok(())
    }
    pub(crate) fn refresh(
        &mut self,
        header: Result<Option<String>>,
        committed: bool,
    ) -> Result<()> {
        if committed {
            self.pending.clear();
            // Keep committed snapshot even when the post-write read fails.
            self.baseline = self.current.clone();
        }
        let header = header?;
        let current = if self.pending.is_empty() {
            header.clone()
        } else {
            Some(frontmatter::apply_edits(
                header.as_deref().unwrap_or("---\n---\n"),
                &self.pending,
            )?)
        };
        self.baseline = header;
        self.current = current;
        self.rebuild()
    }
    pub(crate) fn commit(&mut self, edit: FrontmatterEdit, new: bool) -> Result<bool> {
        ensure!(
            self.error.is_none(),
            "Structured properties disabled: {}",
            self.error.as_deref().unwrap_or("")
        );
        let key: Value = serde_yaml_ng::from_str(&edit.key_yaml)?;
        ensure!(
            !frontmatter::is_managed_key(&key),
            "Managed by Clin; use existing note controls"
        );
        let before = frontmatter::validate_header(self.current.as_deref().unwrap_or("---\n---\n"))?;
        ensure!(
            !new || !before.contains_key(&key),
            "Property already exists"
        );
        let original =
            frontmatter::validate_header(self.baseline.as_deref().unwrap_or("---\n---\n"))?;
        let mut pending = self.pending.clone();
        pending.retain(|delta| {
            serde_yaml_ng::from_str::<Value>(&delta.key_yaml)
                .ok()
                .as_ref()
                != Some(&key)
        });
        pending.push(edit);
        let candidate =
            frontmatter::apply_edits(self.baseline.as_deref().unwrap_or("---\n---\n"), &pending)?;
        let after = frontmatter::validate_header(&candidate)?;
        if before == after {
            return Ok(false);
        }
        if original.get(&key) == after.get(&key) {
            pending.pop();
        }
        let candidate =
            frontmatter::apply_edits(self.baseline.as_deref().unwrap_or("---\n---\n"), &pending)?;
        // Build a fresh tree, never clone mutable CST for rollback.
        let mut view = Self::load(Ok(Some(candidate)));
        ensure!(
            view.error.is_none(),
            "{}",
            view.error.as_deref().unwrap_or("Invalid properties")
        );
        self.current = view.current;
        self.rows = std::mem::take(&mut view.rows);
        self.pending = pending;
        self.selected = self.selected.min(self.rows.len());
        self.scroll = self.scroll.min(self.rows.len());
        self.revision = self.revision.wrapping_add(1);
        Ok(true)
    }
    pub(crate) fn begin_add(&mut self, theme: &AppThemeColors) {
        if self.error.is_some() {
            return;
        }
        self.dialog = Some(PropertiesDialog::Edit(Box::new(PropertyDialog {
            key_yaml: None,
            name: crate::ui::make_popup_textarea(theme, "Name"),
            value: crate::ui::make_popup_textarea(theme, "Value"),
            kind: PropertyType::String,
            control: 0,
            error: None,
            rects: [Rect::default(); 4],
        })));
    }
    pub(crate) fn begin_edit(&mut self, theme: &AppThemeColors, delete: bool) -> Result<()> {
        ensure!(self.error.is_none(), "Structured properties disabled");
        let row = self
            .rows
            .get(self.selected)
            .context("Select a property first")?;
        ensure!(!row.managed, "Managed by Clin; use existing note controls");
        if delete {
            self.dialog = Some(PropertiesDialog::Delete {
                key_yaml: row.key_yaml.clone(),
                yes: false,
                rects: [Rect::default(); 2],
            });
        } else {
            let mut name = crate::ui::make_popup_textarea(theme, "");
            name.insert_str(row.name());
            let mut value = crate::ui::make_popup_textarea(theme, "Value");
            value.insert_str(if row.kind == PropertyType::String {
                row.value.as_str().unwrap_or("")
            } else {
                row.value_yaml.trim_end_matches(['\r', '\n'])
            });
            self.dialog = Some(PropertiesDialog::Edit(Box::new(PropertyDialog {
                key_yaml: Some(row.key_yaml.clone()),
                name,
                value,
                kind: row.kind,
                control: 2,
                error: None,
                rects: [Rect::default(); 4],
            })));
            if row.kind == PropertyType::Null
                && let Some(PropertiesDialog::Edit(dialog)) = &mut self.dialog
            {
                dialog.control = 1;
            }
        }
        Ok(())
    }
    pub(crate) fn paste(&mut self, data: &str) -> bool {
        if let Some(PropertiesDialog::Edit(dialog)) = &mut self.dialog {
            match dialog.control {
                0 if dialog.key_yaml.is_none() => {
                    dialog.name.insert_str(data.replace(['\r', '\n'], " "));
                }
                2 if dialog.kind != PropertyType::Null => {
                    dialog.value.insert_str(data);
                }
                _ => {}
            }
        }
        true // Lists and non-text dialog controls consume paste too.
    }
    fn navigate(&mut self, delta: isize) {
        self.selected = self
            .selected
            .saturating_add_signed(delta)
            .min(self.rows.len());
        self.scroll_to_selection();
    }
    pub(crate) fn scroll_to_selection(&mut self) {
        let height = self.visible_rows.max(1);
        if self.selected < self.scroll {
            self.scroll = self.selected;
        }
        if self.selected >= self.scroll + height {
            self.scroll = self.selected + 1 - height;
        }
    }
}

impl App {
    pub fn toggle_properties(&mut self) {
        if self.mode == ViewMode::List {
            let Some(item) = self.list.visual_list.get(self.list.visual_index) else {
                self.set_temporary_status_static("Select a note first");
                return;
            };
            let crate::list_view::VisualItem::Note { summary_idx, .. } = item else {
                self.set_temporary_status_static("Properties are available for notes");
                return;
            };
            let Some(id) = self.notes.get(*summary_idx).map(|note| note.id.clone()) else {
                self.set_temporary_status_static("Select a note first");
                return;
            };
            if id.ends_with(".clin") {
                self.open_note_at_line(&id, None);
                return;
            }
            if !id.ends_with(".md") && !id.ends_with(".txt") {
                self.set_temporary_status_static("Properties are available for notes");
                return;
            }
            self.load_and_open_note(&id, None);
        }
        if self.mode != ViewMode::Edit || !self.properties_available() {
            self.set_temporary_status_static("Properties are available for notes");
            return;
        }
        if self.preview_fullscreen {
            self.toggle_preview_fullscreen();
        }
        let state = &mut self.editor.properties;
        state.expanded = !state.expanded;
        state.focus_request = Some(if state.expanded {
            EditFocus::Properties
        } else {
            EditFocus::Body
        });
    }
    pub(crate) fn properties_available(&self) -> bool {
        self.editor.template_edit_path.is_none()
            && self.editor.editing_id.as_deref().is_some_and(|id| {
                let ext = std::path::Path::new(id)
                    .extension()
                    .and_then(|extension| extension.to_str());
                matches!(ext, None | Some("md" | "txt"))
            })
    }
    pub(crate) fn properties_layout_rows(&self) -> Option<usize> {
        (self.properties_available() && self.editor.properties.expanded)
            .then_some(self.editor.properties.rows.len() + 1)
    }
    fn property_committed(&mut self) {
        self.editor.autosave_status = crate::editor::AutosaveStatus::Unsaved;
        self.editor.autosave_timer =
            Some(std::time::Instant::now() + std::time::Duration::from_secs(2));
        *self.editor.modified_status_cache.borrow_mut() = None;
        self.write_draft();
    }
    pub(crate) fn handle_properties_dialog_key(&mut self, key: KeyEvent) -> bool {
        let Some(mut dialog) = self.editor.properties.dialog.take() else {
            return false;
        };
        self.seq_matcher.clear();
        if key.code == KeyCode::Esc {
            return true;
        }
        if self
            .keybinds
            .matches_edit(crate::keybinds::EditAction::InsertDate, &key)
        {
            if let PropertiesDialog::Edit(input) = &mut dialog
                && input.control == 2
                && matches!(input.kind, PropertyType::String | PropertyType::Yaml)
            {
                input.value.insert_str(
                    chrono::Local::now()
                        .format(&self.config.editor.date_format)
                        .to_string(),
                );
            }
            self.editor.properties.dialog = Some(dialog);
            return true;
        }
        let mut edit = None;
        let mut close = false;
        match &mut dialog {
            PropertiesDialog::Delete { key_yaml, yes, .. } => match key.code {
                KeyCode::Left
                | KeyCode::Right
                | KeyCode::Tab
                | KeyCode::BackTab
                | KeyCode::Up
                | KeyCode::Down => *yes = !*yes,
                KeyCode::Enter => {
                    if *yes {
                        edit = Some(Ok((
                            FrontmatterEdit {
                                key_yaml: key_yaml.clone(),
                                value_yaml: None,
                            },
                            false,
                        )));
                    } else {
                        close = true;
                    }
                }
                _ => {}
            },
            PropertiesDialog::Edit(input) => match key.code {
                KeyCode::Enter
                    if key.modifiers.contains(KeyModifiers::CONTROL) || input.control == 3 =>
                {
                    edit = Some(input.edit().map(|edit| (edit, input.key_yaml.is_none())));
                }
                KeyCode::Tab => input.advance(1),
                KeyCode::BackTab => input.advance(-1),
                KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down
                    if input.control == 1 =>
                {
                    input.kind =
                        input
                            .kind
                            .cycle(if matches!(key.code, KeyCode::Left | KeyCode::Up) {
                                -1
                            } else {
                                1
                            });
                }
                KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down
                    if input.control == 2 && input.kind == PropertyType::Boolean =>
                {
                    let value = if input.value.lines().join("\n").trim() == "true" {
                        "false"
                    } else {
                        "true"
                    };
                    input.value.select_all();
                    input.value.insert_str(value);
                }
                KeyCode::Enter
                    if input.control != 2
                        || !matches!(input.kind, PropertyType::String | PropertyType::Yaml) =>
                {
                    input.advance(1)
                }
                _ => {
                    if input.control == 0 {
                        crate::text_edit::feed_key(&self.keybinds, &mut input.name, key);
                    } else if input.control == 2 {
                        crate::text_edit::feed_key(&self.keybinds, &mut input.value, key);
                    }
                }
            },
        }
        if let Some(edit) = edit {
            match edit.and_then(|(edit, new)| self.editor.properties.commit(edit, new)) {
                Ok(changed) => {
                    if changed {
                        self.property_committed();
                    }
                    close = true;
                }
                Err(error) => match &mut dialog {
                    PropertiesDialog::Edit(input) => input.error = Some(error.to_string()),
                    PropertiesDialog::Delete { .. } => {
                        self.set_temporary_status(&error.to_string())
                    }
                },
            }
        }
        if !close {
            self.editor.properties.dialog = Some(dialog);
        }
        true
    }
    pub(crate) fn handle_properties_list_key(&mut self, key: KeyEvent, focus: &mut EditFocus) {
        self.seq_matcher.clear();
        let state = &mut self.editor.properties;
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => state.navigate(-1),
            KeyCode::Down | KeyCode::Char('j') => state.navigate(1),
            KeyCode::Home => {
                state.selected = 0;
                state.scroll_to_selection();
            }
            KeyCode::End => {
                state.selected = state.rows.len();
                state.scroll_to_selection();
            }
            KeyCode::PageUp => state.navigate(-(state.visible_rows.max(1) as isize)),
            KeyCode::PageDown => state.navigate(state.visible_rows.max(1) as isize),
            KeyCode::Char('a') => state.begin_add(&self.app_theme),
            KeyCode::Enter if state.selected == state.rows.len() => {
                state.begin_add(&self.app_theme)
            }
            KeyCode::Enter | KeyCode::Delete => {
                if let Err(error) = state.begin_edit(&self.app_theme, key.code == KeyCode::Delete) {
                    self.set_temporary_status(&error.to_string());
                }
            }
            KeyCode::Char(' ') => {
                state.expanded = false;
                *focus = EditFocus::Body;
            }
            KeyCode::Esc => *focus = EditFocus::Body,
            _ => {}
        }
    }
}

pub(crate) fn draw_section(
    frame: &mut ratatui::Frame,
    app: &mut App,
    focus: EditFocus,
    area: Rect,
) {
    use ratatui::{
        style::{Modifier, Style},
        widgets::{Block, Paragraph},
    };
    if area.height < 2 {
        return;
    }
    let theme = &app.app_theme;
    let state = &mut app.editor.properties;
    let background = theme.preview_bg_style();
    frame.render_widget(Block::default().style(background), area);
    let marker = "▾";
    let header = if let Some(error) = &state.error {
        format!("{marker} Properties — {error}")
    } else {
        format!(
            "{marker} Properties ({})  {}",
            state.rows.len(),
            if focus == EditFocus::Properties {
                "a add · Enter edit · Delete remove · Space collapse"
            } else {
                ""
            }
        )
    };
    let style = if state.error.is_some() {
        background.fg(ratatui::style::Color::Red)
    } else if focus == EditFocus::Properties {
        background.fg(theme.accent).add_modifier(Modifier::BOLD)
    } else {
        background.fg(theme.muted)
    };
    frame.render_widget(
        Paragraph::new(header).style(style),
        Rect::new(area.x, area.y + 1, area.width, 1),
    );
    state.visible_rows = area.height.saturating_sub(2) as usize;
    state.scroll = state
        .scroll
        .min((state.rows.len() + 1).saturating_sub(state.visible_rows.max(1)));
    for (visible, index) in (state.scroll..=state.rows.len())
        .take(state.visible_rows)
        .enumerate()
    {
        let text = if let Some(row) = state.rows.get(index) {
            format!(
                "  {}: {}  [{}]{}",
                row.name(),
                row.summary(),
                row.kind.label(),
                if row.managed { " (managed)" } else { "" }
            )
        } else {
            "  + Add property".into()
        };
        let style = if index == state.selected && focus == EditFocus::Properties {
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
        } else {
            background
        };
        frame.render_widget(
            Paragraph::new(text).style(style),
            Rect::new(area.x, area.y + 2 + visible as u16, area.width, 1),
        );
    }
}

pub(crate) fn draw_dialog(frame: &mut ratatui::Frame, app: &mut App) {
    use crate::ui::{PopupHints, PopupSize, draw_popup_frame};
    use ratatui::{
        layout::{Constraint, Layout},
        style::Style,
        widgets::{Block, Borders, Paragraph, Wrap},
    };
    let Some(dialog) = &mut app.editor.properties.dialog else {
        return;
    };
    let theme = &app.app_theme;
    let hints = [
        ("Tab".into(), "control"),
        ("Ctrl+Enter".into(), "apply"),
        ("Esc".into(), "cancel"),
    ];
    let content = draw_popup_frame(
        frame,
        frame.area(),
        "PROPERTIES",
        if matches!(dialog, PropertiesDialog::Delete { .. }) {
            PopupSize::Small
        } else {
            PopupSize::Large
        },
        PopupHints::Keybinds(&hints),
        theme,
    );
    let border = |title: &str, selected: bool| {
        Block::default()
            .borders(Borders::ALL)
            .title(title.to_owned())
            .style(theme.bg_style())
            .border_style(Style::default().fg(if selected { theme.accent } else { theme.muted }))
    };
    match dialog {
        PropertiesDialog::Edit(input) => {
            let chunks = Layout::vertical([
                Constraint::Length(3),
                Constraint::Length(3),
                if input.kind == PropertyType::Null {
                    Constraint::Length(0)
                } else {
                    Constraint::Min(3)
                },
                Constraint::Length(2),
                Constraint::Length(1),
            ])
            .split(content);
            input.rects = [chunks[0], chunks[1], chunks[2], chunks[4]];
            input.name.set_block(border(
                if input.key_yaml.is_some() {
                    "Name (read-only)"
                } else {
                    "Name"
                },
                input.control == 0,
            ));
            input.name.set_cursor_style(if input.control == 0 {
                Style::default()
                    .fg(theme.highlight_fg)
                    .bg(theme.highlight_bg)
            } else {
                theme.bg_style()
            });
            frame.render_widget(&input.name, chunks[0]);
            frame.render_widget(
                Paragraph::new(format!("{}  ←/→", input.kind.label()))
                    .block(border("Type", input.control == 1)),
                chunks[1],
            );
            if input.kind != PropertyType::Null {
                input.value.set_block(border(
                    if input.kind == PropertyType::Boolean {
                        "Value ←/→ toggles"
                    } else {
                        "Value"
                    },
                    input.control == 2,
                ));
                input.value.set_cursor_style(if input.control == 2 {
                    Style::default()
                        .fg(theme.highlight_fg)
                        .bg(theme.highlight_bg)
                } else {
                    theme.bg_style()
                });
                frame.render_widget(&input.value, chunks[2]);
            }
            if let Some(error) = &input.error {
                frame.render_widget(
                    Paragraph::new(error.as_str())
                        .style(Style::default().fg(ratatui::style::Color::Red))
                        .wrap(Wrap { trim: false }),
                    chunks[3],
                );
            }
            frame.render_widget(
                Paragraph::new("[ Apply ]").style(if input.control == 3 {
                    Style::default()
                        .fg(theme.highlight_fg)
                        .bg(theme.highlight_bg)
                } else {
                    theme.bg_style()
                }),
                chunks[4],
            );
        }
        PropertiesDialog::Delete {
            key_yaml,
            yes,
            rects,
        } => {
            let chunks =
                Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(content);
            frame.render_widget(
                Paragraph::new(format!("Delete property {}?", key_yaml.trim()))
                    .wrap(Wrap { trim: false }),
                chunks[0],
            );
            let buttons =
                Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(chunks[1]);
            *rects = [buttons[0], buttons[1]];
            for (index, label) in ["[ Yes ]", "[ No ]"].iter().enumerate() {
                frame.render_widget(
                    Paragraph::new(*label).style(if (index == 0) == *yes {
                        Style::default()
                            .fg(theme.highlight_fg)
                            .bg(theme.highlight_bg)
                    } else {
                        theme.bg_style()
                    }),
                    buttons[index],
                );
            }
        }
    }
}

pub(crate) fn handle_dialog_mouse(app: &mut App, mouse: crossterm::event::MouseEvent) -> bool {
    use crossterm::event::{MouseButton, MouseEventKind};
    let Some(dialog) = &mut app.editor.properties.dialog else {
        return false;
    };
    let mut apply = false;
    if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
        match dialog {
            PropertiesDialog::Edit(input) => {
                if let Some(control) = input
                    .rects
                    .iter()
                    .position(|rect| crate::events::contains_cell(*rect, mouse.column, mouse.row))
                {
                    if control == 0 && input.key_yaml.is_some() {
                        return true;
                    }
                    input.control = control;
                    match control {
                        1 => input.kind = input.kind.cycle(1),
                        3 => apply = true,
                        0 | 2 => {
                            let textarea = if control == 0 {
                                &mut input.name
                            } else {
                                &mut input.value
                            };
                            let rect = input.rects[control];
                            let inner = Rect::new(
                                rect.x + 1,
                                rect.y + 1,
                                rect.width.saturating_sub(2),
                                rect.height.saturating_sub(2),
                            );
                            crate::events::move_textarea_cursor_to_mouse(
                                textarea,
                                inner,
                                mouse.column,
                                mouse.row,
                                0,
                                0,
                            );
                        }
                        _ => {}
                    }
                }
            }
            PropertiesDialog::Delete { yes, rects, .. } => {
                if let Some(index) = rects
                    .iter()
                    .position(|rect| crate::events::contains_cell(*rect, mouse.column, mouse.row))
                {
                    *yes = index == 0;
                    apply = true;
                }
            }
        }
    }
    if apply {
        app.handle_properties_dialog_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    }
    true
}

pub(crate) fn handle_list_mouse(
    app: &mut App,
    mouse: crossterm::event::MouseEvent,
    area: Rect,
    focus: &mut EditFocus,
) -> bool {
    use crossterm::event::{MouseButton, MouseEventKind};
    if !crate::events::contains_cell(area, mouse.column, mouse.row) {
        return false;
    }
    let state = &mut app.editor.properties;
    match mouse.kind {
        MouseEventKind::ScrollUp => state.scroll = state.scroll.saturating_sub(1),
        MouseEventKind::ScrollDown => {
            state.scroll = (state.scroll + 1)
                .min((state.rows.len() + 1).saturating_sub(state.visible_rows.max(1)))
        }
        MouseEventKind::Down(MouseButton::Left) if mouse.row == area.y + 1 => {
            app.toggle_properties();
            if let Some(requested) = app.editor.properties.focus_request.take() {
                *focus = requested;
            }
        }
        MouseEventKind::Down(MouseButton::Left) if mouse.row >= area.y + 2 => {
            *focus = EditFocus::Properties;
            let index = state.scroll + (mouse.row - area.y - 2) as usize;
            if index <= state.rows.len() {
                let double = state.last_click.is_some_and(|(previous, time)| {
                    previous == index && time.elapsed().as_millis() < 500
                });
                state.selected = index;
                state.last_click = Some((index, std::time::Instant::now()));
                if index == state.rows.len() {
                    state.begin_add(&app.app_theme);
                } else if double
                    && !state.rows[index].managed
                    && let Err(error) = state.begin_edit(&app.app_theme, false)
                {
                    app.set_temporary_status(&error.to_string());
                }
            }
        }
        _ => {}
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn properties_transaction_revert_types_and_errors() {
        let baseline = "---\n# retain\ncode: '001' # quote\nstatus: open\n---\n";
        let mut state = PropertiesState::load(Ok(Some(baseline.into())));
        let edit = |key: &str, value: &str| FrontmatterEdit {
            key_yaml: key.into(),
            value_yaml: Some(value.into()),
        };
        assert!(state.commit(edit("status", "answered"), false).unwrap());
        assert_eq!(state.pending.len(), 1);
        assert!(state.commit(edit("status", "open"), false).unwrap());
        assert!(state.pending.is_empty());
        assert_eq!(state.current.as_deref(), Some(baseline));
        let revision = state.revision;
        assert!(!state.commit(edit("code", "\"001\""), false).unwrap());
        assert_eq!(state.revision, revision);
        for (key, value, new) in [
            ("title", "x", true),
            ("status", "x", true),
            ("bad", "[", true),
        ] {
            assert!(state.commit(edit(key, value), new).is_err());
        }
        assert_eq!(state.current.as_deref(), Some(baseline));
        assert!(state.pending.is_empty());
        let encoded = PropertyType::String.encode("true").unwrap();
        assert_eq!(
            serde_yaml_ng::from_str::<Value>(&encoded).unwrap(),
            Value::String("true".into())
        );
        assert!(PropertyType::Number.encode("001 text").is_err());
        assert!(PropertyType::Boolean.encode("yes").is_err());
        assert!(
            state
                .commit(
                    edit("flag", &PropertyType::Boolean.encode("true").unwrap()),
                    true
                )
                .unwrap()
        );
        assert_eq!(state.rows.last().unwrap().kind, PropertyType::Boolean);
        assert!(
            state
                .commit(
                    FrontmatterEdit {
                        key_yaml: "flag".into(),
                        value_yaml: None
                    },
                    false
                )
                .unwrap()
        );
        assert_eq!(state.current.as_deref(), Some(baseline));
    }

    #[test]
    fn properties_focus_geometry_background_and_mouse() {
        use crate::app::EditSidebar;
        use crate::config::PreviewPosition;
        use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
        let _guard = crate::config::ConfigTestGuard::lock();
        let dir = tempfile::tempdir().unwrap();
        crate::config::set_config_path_override(dir.path().join("config.toml"));
        let storage = crate::storage::Storage {
            data_dir: dir.path().into(),
            config_dir: dir.path().into(),
            notes_dir: dir.path().into(),
            templates_dir: dir.path().join("templates"),
            key: [0; 32],
            skip_dir_patterns: vec![],
            rename_on_title_change: false,
        };
        std::fs::write(
            dir.path().join("note.md"),
            "---\nstatus: open\n---\nbody text",
        )
        .unwrap();
        let mut app = App::new(storage).unwrap();
        app.load_and_open_note("note.md", None);
        let area = Rect::new(0, 0, 100, 30);
        let mut focus = EditFocus::Body;
        let mut selection = crate::text_edit::MouseTextSelection::default();
        assert!(app.properties_layout_rows().is_none());
        for expected in [EditFocus::Properties, EditFocus::Title, EditFocus::Body] {
            crate::events::handle_edit_keys(
                &mut app,
                KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL),
                &mut focus,
            );
            assert_eq!(focus, expected);
        }
        assert!(!app.editor.properties.expanded);
        crate::events::handle_edit_keys(
            &mut app,
            KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL),
            &mut focus,
        );
        assert_eq!(focus, EditFocus::Properties);
        assert_eq!(app.editor.body.lines(), &["body text"]);
        app.app_theme.bg = Some(ratatui::style::Color::Rgb(40, 50, 60));
        for preview in [false, true] {
            for position in [PreviewPosition::Right, PreviewPosition::Left] {
                for sidebar in [EditSidebar::None, EditSidebar::Outline, EditSidebar::Links] {
                    app.editor.editor_preview_enabled = preview;
                    app.preview_position = position;
                    app.editor.sidebar = sidebar;
                    let body_area = crate::events::edit_view_outer_areas(area)[1];
                    let plain = crate::events::compute_edit_layout(
                        body_area, false, preview, sidebar, position, 0, None,
                    );
                    let expanded = crate::events::compute_edit_layout(
                        body_area,
                        false,
                        preview,
                        sidebar,
                        position,
                        0,
                        app.properties_layout_rows(),
                    );
                    assert_eq!(plain.preview, expanded.preview);
                    assert_eq!(plain.sidebar, expanded.sidebar);
                    let properties = expanded.properties.unwrap();
                    assert_eq!(properties.x, expanded.body.x);
                    assert_eq!(properties.width, expanded.body.width);
                    assert_eq!(properties.bottom(), expanded.body.y);
                    let mut terminal =
                        ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 30))
                            .unwrap();
                    terminal
                        .draw(|frame| crate::ui::draw_ui(frame, &mut app, focus))
                        .unwrap();
                    let buffer = terminal.backend().buffer();
                    for x in properties.x..properties.right() {
                        let cell = &buffer[(x, properties.y)];
                        assert_eq!(cell.symbol(), " ");
                        assert_eq!(cell.bg, app.app_theme.preview_bg().unwrap());
                    }
                    assert_eq!(
                        buffer[(properties.x, properties.y + 1)].bg,
                        app.app_theme.preview_bg().unwrap()
                    );
                    let click = MouseEvent {
                        kind: MouseEventKind::Down(MouseButton::Left),
                        column: properties.x + 2,
                        row: properties.y + 2,
                        modifiers: KeyModifiers::NONE,
                    };
                    crate::events::handle_edit_mouse(
                        &mut app,
                        click,
                        area,
                        &mut focus,
                        &mut selection,
                    );
                    assert_eq!(focus, EditFocus::Properties);
                    assert_eq!(app.editor.properties.selected, 0);
                    // Cancel any double-click dialog before clicking body.
                    app.editor.properties.dialog = None;
                    app.editor.text_align = crate::config::TextAlignment::Left;
                    let (_, body, _, _) = crate::events::edit_view_input_areas(
                        area,
                        false,
                        preview,
                        app.editor.body.lines().len(),
                        app.editor_show_line_numbers(),
                        sidebar,
                        position,
                        app.editor.header_title_rect,
                        0,
                        app.properties_layout_rows(),
                    );
                    crate::events::handle_edit_mouse(
                        &mut app,
                        MouseEvent {
                            column: body.x + 5,
                            row: body.y,
                            ..click
                        },
                        area,
                        &mut focus,
                        &mut selection,
                    );
                    assert_eq!(focus, EditFocus::Body);
                    assert_eq!(
                        app.editor.body.cursor(),
                        crate::editor_document::TextPosition { row: 0, col: 5 }
                    );
                }
            }
        }
        for height in 0..12 {
            let layout = crate::events::compute_edit_layout(
                Rect::new(0, 0, 8, height),
                false,
                false,
                EditSidebar::None,
                PreviewPosition::Right,
                0,
                Some(50),
            );
            assert!(layout.properties.unwrap().height <= 8);
            assert!(layout.body.height >= height.min(3));
        }
        let fullscreen = crate::events::compute_edit_layout(
            area,
            true,
            true,
            EditSidebar::None,
            PreviewPosition::Right,
            0,
            Some(50),
        );
        assert!(fullscreen.properties.is_none());
        app.editor.sidebar = EditSidebar::None;
        focus = EditFocus::Properties;
        crate::events::handle_edit_keys(
            &mut app,
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
            &mut focus,
        );
        assert_eq!(focus, EditFocus::Body);
        assert!(app.properties_layout_rows().is_none());
        app.config.editor.date_format = "DATE".into();
        app.editor.properties.focused = true;
        crate::actions::execute_action("editor.insert_date", &mut app, None).unwrap();
        assert_eq!(app.editor.body.lines(), &["body text"]);
        app.editor.properties.selected = 0;
        app.editor
            .properties
            .begin_edit(&app.app_theme, false)
            .unwrap();
        app.handle_properties_dialog_key(KeyEvent::new(KeyCode::Char(';'), KeyModifiers::CONTROL));
        if let Some(PropertiesDialog::Edit(dialog)) = &mut app.editor.properties.dialog {
            assert_eq!(dialog.value.lines(), &["openDATE"]);
            dialog.value.select_all();
        } else {
            panic!("Property dialog closed during date insertion");
        }
        crate::actions::execute_action("editor.insert_date", &mut app, None).unwrap();
        if let Some(PropertiesDialog::Edit(dialog)) = &app.editor.properties.dialog {
            assert_eq!(dialog.value.lines(), &["DATE"]);
        }
        app.handle_properties_dialog_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(app.editor.properties.pending.is_empty());
        assert_eq!(app.editor.body.lines(), &["body text"]);
    }
}
