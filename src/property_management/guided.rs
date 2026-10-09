use super::*;
use crossterm::event::{Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

pub(super) enum GuidedAction {
    Handled,
    Preview,
    Pass,
}

impl PropertyManager {
    pub(super) fn sync_guided(&mut self) -> Result<()> {
        let text = match &mut self.guided {
            Some(GuidedManager::Bulk(form)) => form.bulk_text()?,
            Some(GuidedManager::Definitions {
                definitions,
                keys,
                form,
                ..
            }) => {
                let mut proposed = definitions.clone();
                if let Some(form) = form {
                    crate::property_model::validate_key(&form.key())?;
                    proposed.properties.insert(form.key(), form.definition()?);
                }
                proposed.check()?;
                let mut document: toml_edit::DocumentMut = self.input.lines().join("\n").parse()?;
                let desired: toml_edit::DocumentMut =
                    toml_edit::ser::to_string_pretty(&proposed)?.parse()?;
                for (key, item) in desired.iter() {
                    if let Some(existing) = document.get_mut(key) {
                        crate::config::merge::merge_edit_item(existing, item.clone());
                    } else {
                        document.insert(key, item.clone());
                    }
                }
                *definitions = proposed;
                for key in definitions.properties.keys() {
                    if !keys.contains(key) {
                        keys.push(key.clone());
                    }
                }
                keys.sort();
                document.to_string()
            }
            None => return Ok(()),
        };
        self.input.select_all();
        self.input.insert_str(text);
        Ok(())
    }
}

impl App {
    pub(super) fn enable_guided_management(&self, manager: &mut PropertyManager) -> Result<()> {
        let text = manager.input.lines().join("\n");
        manager.guided = match manager.mode {
            PropertyManagerMode::Definitions => {
                let definitions: PropertyDefinitions = toml::from_str(&text)?;
                definitions.check()?;
                let keys = definitions
                    .properties
                    .keys()
                    .chain(self.notes.iter().flat_map(|note| note.properties.keys()))
                    .cloned()
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect();
                Some(GuidedManager::Definitions {
                    definitions,
                    keys,
                    selected: 0,
                    form: None,
                })
            }
            PropertyManagerMode::Bulk => {
                let input: BulkInput = toml::from_str(&text)?;
                let mut form = ManagementForm::new(
                    self,
                    &input.name,
                    self.property_definitions.properties.get(&input.name),
                )?;
                form.kind = input.kind;
                form.enabled = !input.unset;
                if let Some(value) = input.value {
                    let value = crate::property_model::toml_to_yaml(&value)?;
                    form.value.select_all();
                    form.value.insert_str(match value {
                        serde_yaml_ng::Value::String(value) => value,
                        _ => serde_yaml_ng::to_string(&value)?,
                    });
                }
                form.refresh_choices(self);
                Some(GuidedManager::Bulk(Box::new(form)))
            }
            _ => None,
        };
        Ok(())
    }

    pub(super) fn handle_guided_management(
        &self,
        manager: &mut PropertyManager,
        event: &Event,
    ) -> Result<GuidedAction> {
        if matches!(event, Event::Key(key) if key.code == KeyCode::F(2)) {
            manager.sync_guided()?;
            manager.guided = None;
            return Ok(GuidedAction::Handled);
        }
        if matches!(event, Event::Key(key) if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('s') | KeyCode::Enter))
        {
            return Ok(GuidedAction::Pass);
        }
        let Some(guided) = &mut manager.guided else {
            return Ok(GuidedAction::Pass);
        };
        let definition = matches!(guided, GuidedManager::Definitions { .. });
        let form = match guided {
            GuidedManager::Bulk(form) => Some(form),
            GuidedManager::Definitions {
                definitions,
                keys,
                selected,
                form,
            } => {
                if form.is_none() {
                    match event {
                        Event::Key(key) => match key.code {
                            KeyCode::Up => *selected = selected.saturating_sub(1),
                            KeyCode::Down => {
                                *selected = (*selected + 1).min(keys.len().saturating_sub(1))
                            }
                            KeyCode::Char('a') => {
                                *form = Some(Box::new(ManagementForm::new(self, "", None)?))
                            }
                            KeyCode::Enter => {
                                let name = keys.get(*selected).map_or("", String::as_str);
                                *form = Some(Box::new(ManagementForm::new(
                                    self,
                                    name,
                                    definitions.properties.get(name),
                                )?));
                            }
                            KeyCode::Delete => {
                                if let Some(key) = keys.get(*selected) {
                                    definitions.properties.remove(key);
                                    return Ok(GuidedAction::Preview);
                                }
                            }
                            KeyCode::Esc => return Ok(GuidedAction::Pass),
                            _ => {}
                        },
                        Event::Mouse(mouse) => match mouse.kind {
                            MouseEventKind::ScrollUp => *selected = selected.saturating_sub(1),
                            MouseEventKind::ScrollDown => {
                                *selected = (*selected + 1).min(keys.len().saturating_sub(1))
                            }
                            MouseEventKind::Down(MouseButton::Left)
                                if crate::events::contains_cell(
                                    manager.input_rect,
                                    mouse.column,
                                    mouse.row,
                                ) =>
                            {
                                let start = selected.saturating_sub(
                                    manager.input_rect.height.saturating_sub(1) as usize / 2,
                                );
                                *selected = (start
                                    + mouse.row.saturating_sub(manager.input_rect.y + 1) as usize)
                                    .min(keys.len().saturating_sub(1));
                                if let Some(name) = keys.get(*selected) {
                                    *form = Some(Box::new(ManagementForm::new(
                                        self,
                                        name,
                                        definitions.properties.get(name),
                                    )?));
                                }
                            }
                            MouseEventKind::Down(MouseButton::Left)
                                if crate::events::contains_cell(
                                    manager.apply_rect,
                                    mouse.column,
                                    mouse.row,
                                ) =>
                            {
                                return Ok(GuidedAction::Pass);
                            }
                            _ => {}
                        },
                        _ => {}
                    }
                    return Ok(GuidedAction::Handled);
                }
                if matches!(event, Event::Key(key) if key.code == KeyCode::Esc) {
                    *form = None;
                    return Ok(GuidedAction::Handled);
                }
                form.as_mut()
            }
        };
        let Some(form) = form else {
            return Ok(GuidedAction::Handled);
        };
        match event {
            Event::Paste(text) => {
                let name = form.control == 0;
                if let Some(input) = form.textarea() {
                    input.insert_str(if name {
                        text.replace(['\r', '\n'], " ")
                    } else {
                        text.clone()
                    });
                }
            }
            Event::Key(key) => match key.code {
                KeyCode::Esc => return Ok(GuidedAction::Pass),
                KeyCode::Tab => form.advance(1, definition),
                KeyCode::BackTab => form.advance(-1, definition),
                KeyCode::Enter if form.control == 6 => return Ok(GuidedAction::Preview),
                KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down
                    if form.control == 1 =>
                {
                    form.kind =
                        form.kind
                            .cycle(if matches!(key.code, KeyCode::Left | KeyCode::Up) {
                                -1
                            } else {
                                1
                            });
                    form.refresh_choices(self);
                }
                KeyCode::Char(' ') | KeyCode::Enter if form.control == 5 => {
                    form.enabled = !form.enabled
                }
                KeyCode::Char(' ') if form.control == 2 && form.kind == PropertyKind::Boolean => {
                    let next = if form.value.lines().join("").trim() == "true" {
                        "false"
                    } else {
                        "true"
                    };
                    form.value.select_all();
                    form.value.insert_str(next);
                }
                KeyCode::Up | KeyCode::Down
                    if form.control == 2
                        && !form.choices.is_empty()
                        && matches!(
                            form.kind,
                            PropertyKind::Select
                                | PropertyKind::MultiSelect
                                | PropertyKind::NoteReference
                                | PropertyKind::NoteReferences
                        ) =>
                {
                    form.choice = (form.choice as isize
                        + if key.code == KeyCode::Up { -1 } else { 1 })
                    .rem_euclid(form.choices.len() as isize)
                        as usize;
                    if matches!(
                        form.kind,
                        PropertyKind::Select | PropertyKind::NoteReference
                    ) {
                        form.choose();
                    }
                }
                KeyCode::Char(' ')
                    if form.control == 2
                        && !form.choices.is_empty()
                        && matches!(
                            form.kind,
                            PropertyKind::MultiSelect | PropertyKind::NoteReferences
                        ) =>
                {
                    form.choose()
                }
                _ => {
                    if let Some(input) = form.textarea() {
                        crate::text_edit::feed_key(&self.keybinds, input, *key);
                    }
                    if form.control == 0
                        && !definition
                        && let Some(declared) =
                            self.property_definitions.properties.get(&form.key())
                    {
                        form.kind = declared.kind;
                        form.options.select_all();
                        form.options.insert_str(declared.options.join("\n"));
                    }
                }
            },
            Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
                if crate::events::contains_cell(manager.apply_rect, mouse.column, mouse.row) {
                    return Ok(GuidedAction::Pass);
                }
                if let Some(control) = form
                    .rects
                    .iter()
                    .position(|rect| crate::events::contains_cell(*rect, mouse.column, mouse.row))
                {
                    form.control = control;
                    match control {
                        1 => form.kind = form.kind.cycle(1),
                        5 => form.enabled = !form.enabled,
                        6 => return Ok(GuidedAction::Preview),
                        _ => {
                            let rect = form.rects[control];
                            if let Some(input) = form.textarea() {
                                crate::events::move_textarea_cursor_to_mouse(
                                    input,
                                    rect,
                                    mouse.column,
                                    mouse.row,
                                    0,
                                    0,
                                );
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        form.refresh_choices(self);
        Ok(GuidedAction::Handled)
    }
}

pub(super) fn draw_guided_management(
    frame: &mut ratatui::Frame,
    manager: &mut PropertyManager,
    notes: &[crate::storage::NoteSummary],
    theme: &crate::app_theme::AppThemeColors,
    area: Rect,
) {
    let Some(guided) = &mut manager.guided else {
        return;
    };
    let definition = matches!(guided, GuidedManager::Definitions { .. });
    let form = match guided {
        GuidedManager::Bulk(form) => Some(form),
        GuidedManager::Definitions {
            definitions,
            keys,
            selected,
            form,
        } => {
            if form.is_none() {
                let height = area.height.saturating_sub(1) as usize;
                let start = selected.saturating_sub(height / 2);
                frame.render_widget(
                    Paragraph::new("↑/↓ Enter edit/adopt · a add · Delete remove · F2 TOML"),
                    Rect::new(area.x, area.y, area.width, 1),
                );
                for (row, key) in keys.iter().enumerate().skip(start).take(height) {
                    let declared = definitions.properties.get(key);
                    let mut values = std::collections::BTreeMap::<String, usize>::new();
                    let mut observed = std::collections::BTreeSet::new();
                    let mut conflicts = 0usize;
                    for value in notes.iter().filter_map(|note| note.properties.get(key)) {
                        observed.insert(match value {
                            PropertyValue::Null => "null",
                            PropertyValue::Bool(_) => "boolean",
                            PropertyValue::Integer(_)
                            | PropertyValue::Unsigned(_)
                            | PropertyValue::Number(_) => "number",
                            PropertyValue::String(_) => "string",
                            PropertyValue::List(_) => "list",
                            PropertyValue::Unsupported => "YAML",
                        });
                        *values.entry(value.display()).or_default() += 1;
                        conflicts += usize::from(
                            declared.is_some_and(|definition| !definition.validate_indexed(value)),
                        );
                    }
                    let text = format!(
                        "{} [{}] observed {:?}; {} values; {} conflicts{}",
                        key,
                        declared.map_or("undeclared", |definition| definition.kind.label()),
                        observed,
                        values.len(),
                        conflicts,
                        declared.map_or_else(String::new, |definition| format!(
                            " · {} · default {}",
                            definition.description,
                            definition
                                .default
                                .as_ref()
                                .map_or_else(|| "absent".into(), ToString::to_string)
                        ))
                    );
                    frame.render_widget(
                        Paragraph::new(crate::fsutil::sanitize_for_terminal(&text).to_string())
                            .style(if row == *selected {
                                ratatui::style::Style::default()
                                    .fg(theme.highlight_fg)
                                    .bg(theme.highlight_bg)
                            } else {
                                theme.bg_style()
                            }),
                        Rect::new(area.x, area.y + 1 + (row - start) as u16, area.width, 1),
                    );
                }
                return;
            }
            form.as_mut()
        }
    };
    let Some(form) = form else {
        return;
    };
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Min(3),
        Constraint::Length(if definition { 3 } else { 0 }),
        Constraint::Length(if definition { 3 } else { 0 }),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);
    form.rects.copy_from_slice(&chunks);
    let border = |title: &str, control| {
        Block::default()
            .borders(Borders::ALL)
            .title(title.to_owned())
            .border_style(
                ratatui::style::Style::default().fg(if control == form.control {
                    theme.accent
                } else {
                    theme.muted
                }),
            )
    };
    form.name.set_block(border("Exact key", 0));
    frame.render_widget(&form.name, chunks[0]);
    frame.render_widget(
        Paragraph::new(format!("{} ←/→ · F2 advanced TOML", form.kind.label()))
            .block(border("Type", 1)),
        chunks[1],
    );
    let selected = form.choices.get(form.choice).map_or("", String::as_str);
    let value_title = match form.kind {
        PropertyKind::Boolean => "Boolean: Space toggles".into(),
        PropertyKind::Date => "Date YYYY-MM-DD".into(),
        PropertyKind::DateTime => "Date/time RFC 3339 with offset".into(),
        PropertyKind::List => {
            "List: Enter adds row; Backspace/Delete edits; YAML array for mixed values".into()
        }
        PropertyKind::Select
        | PropertyKind::MultiSelect
        | PropertyKind::NoteReference
        | PropertyKind::NoteReferences => format!("Value ↑/↓ choice; Space toggles: {selected}"),
        _ => "Value".into(),
    };
    form.value.set_block(border(&value_title, 2));
    frame.render_widget(&form.value, chunks[2]);
    if definition {
        form.description.set_block(border("Description", 3));
        frame.render_widget(&form.description, chunks[3]);
        form.options
            .set_block(border("Select options: one per line", 4));
        frame.render_widget(&form.options, chunks[4]);
    }
    frame.render_widget(
        Paragraph::new(if definition {
            if form.enabled {
                "[x] Default enabled (Space toggles)"
            } else {
                "[ ] No default (Space toggles)"
            }
        } else if form.enabled {
            "[x] Set value (Space switches to unset)"
        } else {
            "[ ] Unset key (Space switches to set)"
        })
        .style(if form.control == 5 {
            ratatui::style::Style::default().fg(theme.accent)
        } else {
            theme.bg_style()
        }),
        chunks[5],
    );
    frame.render_widget(
        Paragraph::new("[ Preview; no writes yet ]")
            .wrap(Wrap { trim: false })
            .style(ratatui::style::Style::default().fg(if form.control == 6 {
                theme.accent
            } else {
                theme.muted
            })),
        chunks[6],
    );
}
