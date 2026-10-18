use super::{App, EditFocus, ViewMode};
use crate::actions::ActionContext;
use crate::keybinds::{GlobalAction, KeyCombo, MatchOutcome};
use crossterm::event::KeyEvent;

impl App {
    pub(crate) fn record_command_usage(&mut self, id: &str) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(u128::from(u64::MAX)) as u64;
        let now = now.max(
            self.command_usage
                .values()
                .map(|usage| usage.last_used)
                .max()
                .unwrap_or(0)
                .saturating_add(1),
        );
        self.command_usage
            .entry(id.to_owned())
            .or_default()
            .record(now);
        let Some(path) = &self.command_usage_path else {
            return;
        };
        match crate::local_state::LocalState::update(path, |state| {
            state
                .command_usage
                .entry(id.to_owned())
                .or_default()
                .record(now);
            Ok(())
        }) {
            Ok(state) => self.command_usage = state.command_usage,
            Err(error) => self.messages.push(
                format!("Command usage could not be saved: {error:#}"),
                super::messages::MessageSeverity::Warning,
            ),
        }
    }

    pub(crate) fn palette_shortcuts(&self, mode: ViewMode) -> Vec<KeyCombo> {
        fn claimed<A>(
            bindings: &std::collections::HashMap<A, Vec<crate::keybinds::KeyCombo>>,
            combo: &KeyCombo,
        ) -> bool {
            combo.keys.iter().any(|key| {
                let event = KeyEvent::new(key.code, key.modifiers);
                bindings.values().flatten().any(|binding| {
                    binding
                        .keys
                        .iter()
                        .any(|stroke| stroke.matches_event(&event))
                })
            })
        }
        let keybinds = match mode {
            ViewMode::Graph => self.graph_plugin.as_ref().map(|s| &s.keybinds),
            ViewMode::Draw => self.draw_state.as_ref().map(|s| &s.keybinds),
            ViewMode::Canvas => self.canvas_state.as_ref().map(|s| &s.keybinds),
            ViewMode::Outline => self.outline_state.as_ref().map(|s| &s.keybinds),
            _ => None,
        }
        .unwrap_or(&self.keybinds);
        self.keybinds
            .global
            .get(&GlobalAction::CommandPalette)
            .into_iter()
            .flatten()
            .filter(|combo| !match mode {
                ViewMode::List => claimed(&keybinds.list, combo),
                ViewMode::Edit => claimed(&keybinds.edit, combo),
                ViewMode::Help => claimed(&keybinds.help, combo),
                ViewMode::Graph => claimed(&keybinds.graph, combo),
                ViewMode::Draw => claimed(&keybinds.draw, combo),
                ViewMode::Canvas => claimed(&keybinds.canvas, combo),
                ViewMode::Bases => claimed(&keybinds.bases, combo),
                ViewMode::Outline => claimed(&keybinds.outline, combo),
                ViewMode::Setup => claimed(&keybinds.setup, combo),
            })
            .cloned()
            .collect()
    }

    pub(crate) fn palette_keys_display(&self, mode: ViewMode) -> String {
        self.palette_shortcuts(mode)
            .iter()
            .map(KeyCombo::to_display_string)
            .collect::<Vec<_>>()
            .join("/")
    }

    pub(crate) fn resolve_palette_shortcut(
        &mut self,
        key: KeyEvent,
        mode: ViewMode,
    ) -> MatchOutcome<GlobalAction> {
        let bindings = std::collections::HashMap::from([(
            GlobalAction::CommandPalette,
            self.palette_shortcuts(mode),
        )]);
        self.palette_matcher.resolve(key, &bindings, true, false)
    }

    pub(crate) fn palette_input_available(&self) -> bool {
        if self.popups.active.is_some()
            || self.popups.confirm.is_some()
            || self.vault_switcher.is_some()
            || self.messages.has_fatal()
            || self.messages.force_open
            || self.should_quit
            || self.setup_rebootstrap.is_some()
            || self.base_embeds.has_popup()
        {
            return false;
        }
        match self.mode {
            ViewMode::List => {
                self.list.note_drag.is_none()
                    && self.list.drag_hover.is_none()
                    && self.list.preview_drag_last_pos.is_none()
                    && self.list.scroll_drag.is_none()
                    && self.layout_drag.is_none()
            }
            ViewMode::Edit => {
                self.editor.find_popup.is_none()
                    && self.editor.go_to_line_input.is_none()
                    && self.editor.preview_drag_last_pos.is_none()
            }
            ViewMode::Help => self.help_search.popup.is_none(),
            ViewMode::Graph => self
                .graph_plugin
                .as_ref()
                .is_some_and(|s| s.palette_input_available()),
            ViewMode::Draw => self
                .draw_state
                .as_ref()
                .is_some_and(|s| s.palette_input_available()),
            ViewMode::Canvas => self
                .canvas_state
                .as_ref()
                .is_some_and(|s| s.palette_input_available()),
            ViewMode::Bases => self
                .bases_state
                .as_ref()
                .is_some_and(|s| s.palette_input_available()),
            ViewMode::Outline => self.outline_state.is_some(),
            ViewMode::Setup => self
                .setup_state
                .as_ref()
                .is_some_and(|s| s.vault_modal.is_none() && !s.confirm_exit),
        }
    }

    pub(crate) fn clear_palette_matchers(&mut self) {
        self.palette_matcher.clear();
        self.seq_matcher.clear();
        self.base_embeds.clear_palette_matchers();
        if let Some(s) = &mut self.graph_plugin {
            s.seq_matcher.clear();
        }
        if let Some(s) = &mut self.draw_state {
            s.seq_matcher.clear();
        }
        if let Some(s) = &mut self.canvas_state {
            s.seq_matcher.clear();
        }
        if let Some(s) = &mut self.outline_state {
            s.seq_matcher.clear();
        }
        if let Some(s) = &mut self.bases_state {
            s.matcher.clear();
        }
    }

    pub(crate) fn open_command_palette(&mut self, focus: Option<EditFocus>) -> bool {
        if self.command_palette.is_some() || !self.palette_input_available() {
            return false;
        }
        self.observe_navigation();
        let context = ActionContext::capture(self, focus, None);
        self.clear_palette_matchers();
        self.quick_keybinds_open = false;
        self.command_palette = Some(crate::palette::CommandPalette::with_context(context, self));
        true
    }

    pub(crate) fn dismiss_command_palette(&mut self) {
        self.command_palette = None;
        self.clear_palette_matchers();
    }

    pub(crate) fn activate_palette(
        &mut self,
        palette: crate::palette::CommandPalette,
        selected: usize,
    ) {
        if palette.workflow.is_some() {
            self.activate_palette_workflow(palette, selected);
            return;
        }
        let Some(item) = palette.items.get(selected) else {
            return;
        };
        let id = item.id.clone();
        let context = palette.context.clone();
        let mut palette = palette;
        palette.launching_action = Some(id.clone());
        self.command_palette = Some(palette);
        let result = crate::actions::execute_action_in_context(&id, self, &context);
        if let Some(mut palette) = self.command_palette.take() {
            palette.launching_action = None;
            if palette.workflow.is_some() {
                if let Err(error) = &result {
                    palette.error = Some(format!("{error:#}"));
                }
                self.command_palette = Some(palette);
            } else {
                self.clear_palette_matchers();
            }
        }
        if let Err(error) = result {
            let message = format!("Action failed: {error:#}");
            self.set_temporary_status(&message);
            self.messages
                .push(message, super::messages::MessageSeverity::Warning);
        }
    }

    pub(crate) fn refresh_command_palette(&mut self) {
        if let Some(mut palette) = self.command_palette.take() {
            if palette.context.origin_current(self) {
                palette.refresh_items(self);
                self.command_palette = Some(palette);
            } else {
                self.clear_palette_matchers();
            }
        }
    }
}
