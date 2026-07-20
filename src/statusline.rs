use crate::app::{App, ViewMode};
use crate::app_theme::AppThemeColors;
use crate::config::StatuslineConfig;
use crate::storage::NoteSummary;
use crate::ui::{PreviewHeaderInfo, format_date, format_relative_time, format_size};
use ratatui::prelude::*;
use std::borrow::Cow;
use std::path::Path;

#[derive(Debug, Clone)]
pub enum Segment<'a> {
    Text(String),
    Composite(Vec<Span<'a>>),
    CompositeSplittable(Vec<Span<'a>>),
}

enum FlatSegment<'a> {
    Cell(String),
    Composite(Vec<Span<'a>>),
    Splittable(Vec<Span<'a>>),
}

pub struct StatuslineContext<'a> {
    pub config: &'a crate::config::ClinConfig,
    pub view: ViewMode,
    pub area: Option<Rect>,
    pub app_status: Option<&'a str>,
    pub vault_path: Option<&'a Path>,
    pub date_format: Option<&'a str>,
    pub app: Option<&'a App>,

    // active overlay states:
    pub graph: Option<&'a crate::graf::graph::GraphState>,
    pub draw: Option<&'a crate::draw::app::DrawAppState>,
    pub backup: Option<&'a crate::backup::state::BackupState>,
    pub outline: Option<&'a crate::outline::state::OutlineState>,
    pub canvas: Option<&'a crate::pinstar::state::PinstarState>,
    pub setup: Option<&'a crate::setup::SetupState>,

    // selected/edited note summary:
    pub note: Option<&'a NoteSummary>,
    pub preview_info: Option<&'a PreviewHeaderInfo>,

    // pre-built composite spans:
    pub hints: Option<Vec<Span<'a>>>,
    pub badge: Option<Vec<Span<'a>>>,
    pub pending: Option<Vec<Span<'a>>>,
    pub preview: Option<Vec<Span<'a>>>,
    pub detail: Option<Vec<Span<'a>>>,
}

impl<'a> StatuslineContext<'a> {
    pub fn for_view(app: &'a App, view: ViewMode) -> Self {
        Self {
            config: &app.config,
            view,
            area: None,
            app_status: Some(app.status.as_ref()),
            vault_path: Some(&app.storage.data_dir),
            date_format: Some(&app.date_format),
            app: Some(app),
            graph: None,
            draw: None,
            backup: None,
            outline: None,
            canvas: None,
            setup: None,
            note: None,
            preview_info: None,
            hints: None,
            badge: None,
            pending: None,
            preview: None,
            detail: None,
        }
    }

    pub fn for_overlay(config: &'a crate::config::ClinConfig, view: ViewMode) -> Self {
        Self {
            config,
            view,
            area: None,
            app_status: None,
            vault_path: None,
            date_format: None,
            app: None,
            graph: None,
            draw: None,
            backup: None,
            outline: None,
            canvas: None,
            setup: None,
            note: None,
            preview_info: None,
            hints: None,
            badge: None,
            pending: None,
            preview: None,
            detail: None,
        }
    }
}

pub fn active_note(app: &App, view: ViewMode) -> Option<&NoteSummary> {
    match view {
        ViewMode::List => {
            if let Some(crate::list_view::VisualItem::Note { summary_idx, .. }) =
                app.list.visual_list.get(app.list.visual_index)
            {
                app.notes.get(*summary_idx)
            } else {
                None
            }
        }
        ViewMode::Edit => {
            if let Some(id) = &app.editor.editing_id {
                app.notes.iter().find(|n| &n.id == id)
            } else {
                None
            }
        }
        _ => None,
    }
}

impl StatuslineContext<'_> {
    pub fn resolve(&self, name: &str) -> Option<Cow<'static, str>> {
        match name {
            // Global / App
            "view" => {
                let s = match self.view {
                    ViewMode::List => "Notes",
                    ViewMode::Edit => "Editor",
                    ViewMode::Help => "Help",
                    ViewMode::Graph => "Graph",
                    ViewMode::Draw => "Draw",
                    ViewMode::Canvas => "Canvas",
                    ViewMode::Backup => "Backup",
                    ViewMode::Outline => "Outline",
                    ViewMode::Setup => "Setup",
                };
                Some(s.into())
            }
            "status" => {
                let st = if let Some(app_st) = self.app_status {
                    if app_st == "Ready" || app_st.is_empty() {
                        "".into()
                    } else {
                        app_st.to_string().into()
                    }
                } else if let Some(app) = self.app {
                    if app.status == "Ready" || app.status.is_empty() {
                        "".into()
                    } else {
                        app.status.clone()
                    }
                } else {
                    "".into()
                };
                Some(st)
            }
            "vault" => {
                let name = if let Some(p) = self.vault_path {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string()
                } else if let Some(app) = self.app {
                    app.storage
                        .data_dir
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string()
                } else {
                    "".to_string()
                };
                Some(name.into())
            }
            "vault_path" => {
                let path = if let Some(p) = self.vault_path {
                    p.to_string_lossy().into_owned()
                } else if let Some(app) = self.app {
                    app.storage.data_dir.to_string_lossy().into_owned()
                } else {
                    "".to_string()
                };
                Some(path.into())
            }
            "version" => Some(format!("clin v{}", env!("CARGO_PKG_VERSION")).into()),

            // Date / Time
            "time" | "date" | "datetime" | "weekday" | "year" | "month" | "day" | "hour"
            | "minute" | "second" => {
                let now = chrono::Local::now();
                let fmt = self
                    .date_format
                    .or_else(|| self.app.map(|a| a.date_format.as_str()))
                    .unwrap_or("%Y-%m-%d");
                let val = match name {
                    "time" => now.format("%H:%M").to_string(),
                    "date" => now.format(fmt).to_string(),
                    "datetime" => format!("{} {}", now.format(fmt), now.format("%H:%M")),
                    "weekday" => now.format("%A").to_string(),
                    "year" => now.format("%Y").to_string(),
                    "month" => now.format("%m").to_string(),
                    "day" => now.format("%d").to_string(),
                    "hour" => now.format("%H").to_string(),
                    "minute" => now.format("%M").to_string(),
                    "second" => now.format("%S").to_string(),
                    _ => unreachable!(),
                };
                Some(val.into())
            }

            // Config Echo
            "theme" => Some(self.config.ui.theme.clone().into()),
            "preset" => {
                let p = match self.config.core.keybind_preset {
                    crate::config::KeybindPreset::Default => "default",
                    crate::config::KeybindPreset::Helix => "helix",
                    crate::config::KeybindPreset::Vim => "vim",
                    crate::config::KeybindPreset::Emacs => "emacs",
                };
                Some(p.into())
            }
            "icon_mode" => {
                let im = match self.config.ui.icon_mode {
                    crate::config::IconMode::Nerd => "nerd",
                    crate::config::IconMode::Unicode => "unicode",
                    crate::config::IconMode::None => "none",
                };
                Some(im.into())
            }
            "hint_bar_style" => {
                let hbs = match self.config.ui.hint_bar_style {
                    crate::config::HintBarStyle::Classic => "classic",
                    crate::config::HintBarStyle::Sharp => "sharp",
                    crate::config::HintBarStyle::Rounded => "rounded",
                    crate::config::HintBarStyle::Slanted => "slanted",
                };
                Some(hbs.into())
            }
            "background" => {
                let bg = match self.config.ui.background {
                    crate::config::Background::Transparent => "transparent",
                    crate::config::Background::Solid => "solid",
                };
                Some(bg.into())
            }

            "help_page" => Some(
                self.app
                    .map_or_else(|| "?".into(), |a| format!("{}", a.help_page + 1).into()),
            ),
            "help_total_pages" => Some(self.app.map_or_else(
                || "?".into(),
                |a| {
                    let rows = a.list.help_text_cache.as_ref().map_or(0, |r| r.len());
                    let ps = a.help_page_size.max(1) as usize;
                    format!("{}", rows.div_ceil(ps)).into()
                },
            )),

            // Goals
            "goal_words" => Some(
                self.app
                    .map(|a| a.goals_progress.words_written.to_string())
                    .unwrap_or_else(|| "0".to_string())
                    .into(),
            ),
            "goal_target" => Some(self.config.goals.word_goal.to_string().into()),
            "goal_notes" => Some(
                self.app
                    .map(|a| a.goals_progress.notes_modified.len().to_string())
                    .unwrap_or_else(|| "0".to_string())
                    .into(),
            ),
            "goal_note_target" => Some(self.config.goals.note_goal.to_string().into()),
            "goal_date" => Some(
                self.app
                    .map(|a| a.goals_progress.date.clone())
                    .unwrap_or_default()
                    .into(),
            ),

            // List View
            "title" => {
                let title = match self.view {
                    ViewMode::List => {
                        if let Some(app) = self.app {
                            if app.layout_edit {
                                "Notes - Editing Layout".to_string()
                            } else {
                                "Notes".to_string()
                            }
                        } else {
                            "Notes".to_string()
                        }
                    }
                    ViewMode::Edit => match self.app.map(|a| a.editor.edit_mode) {
                        Some(crate::editor::EditMode::Edit) => "EDITOR - EDIT MODE".to_string(),
                        _ => "EDITOR - READ MODE".to_string(),
                    },
                    ViewMode::Help => "Help".to_string(),
                    ViewMode::Graph => "Graph".to_string(),
                    ViewMode::Draw => "Draw".to_string(),
                    ViewMode::Canvas => "Canvas".to_string(),
                    ViewMode::Backup => "Backup".to_string(),
                    ViewMode::Outline => {
                        if let Some(tree) = &self.outline {
                            format!("OUTLINE — {}", tree.note_title)
                        } else {
                            "Outline".to_string()
                        }
                    }
                    ViewMode::Setup => "Setup".to_string(),
                };
                Some(title.into())
            }
            "sort_field" => {
                let sf = self
                    .app
                    .map(|a| match a.list.sort_field {
                        crate::list_view::SortField::Title => "title",
                        crate::list_view::SortField::Modified => "modified",
                    })
                    .unwrap_or("");
                Some(sf.into())
            }
            "sort_order" => {
                let so = self
                    .app
                    .map(|a| match a.list.sort_order {
                        crate::list_view::SortOrder::Ascending => "ascending",
                        crate::list_view::SortOrder::Descending => "descending",
                    })
                    .unwrap_or("");
                Some(so.into())
            }
            "layout" => {
                let layout = if let Some(app) = self.app {
                    &app.list.notes_layout
                } else {
                    &self.config.list.default_view
                };
                let l = match layout {
                    crate::config::NotesLayout::Tree => "tree",
                    crate::config::NotesLayout::Grid => "grid",
                };
                Some(l.into())
            }
            "density" => {
                let density = if let Some(app) = self.app {
                    &app.list.list_density
                } else {
                    &self.config.list.density
                };
                let d = match density {
                    crate::config::ListDensity::Compact => "compact",
                    crate::config::ListDensity::Comfortable => "comfortable",
                };
                Some(d.into())
            }
            "section" => {
                let folder = self.app.map(|a| a.list.grid_folder.as_str()).unwrap_or("");
                let s = if folder == crate::app::VIRTUAL_PINNED_PATH {
                    "pinned"
                } else if folder.starts_with(crate::app::VIRTUAL_SMART_PATH) {
                    "smart"
                } else {
                    "vault"
                };
                Some(s.into())
            }
            "folder" => Some(
                self.app
                    .map(|a| a.list.grid_folder.clone())
                    .unwrap_or_default()
                    .into(),
            ),
            "folder_count" => {
                let count = self.app.map(|a| a.catalog_folders.len()).unwrap_or(0);
                Some(count.to_string().into())
            }
            "tag_count" => {
                let count = self.app.map(|a| a.collect_live_tags().len()).unwrap_or(0);
                Some(count.to_string().into())
            }
            "note_count" => Some(
                self.app
                    .map(|a| a.notes.len())
                    .unwrap_or(0)
                    .to_string()
                    .into(),
            ),
            "visual_index" => Some(
                self.app
                    .map(|a| a.list.visual_index + 1)
                    .unwrap_or(0)
                    .to_string()
                    .into(),
            ),
            "visual_total" => Some(
                self.app
                    .map(|a| a.list.visual_list.len())
                    .unwrap_or(0)
                    .to_string()
                    .into(),
            ),
            "selected_count" => Some(
                self.app
                    .map(|a| a.list.selected_indices.len())
                    .unwrap_or(0)
                    .to_string()
                    .into(),
            ),
            "select_mode" => {
                let on = self
                    .app
                    .map(|a| a.list.list_mode != crate::list_view::ListMode::Normal)
                    .unwrap_or(false);
                Some((if on { "on" } else { "off" }).into())
            }
            "tag_to_assign" => Some(
                self.app
                    .and_then(|a| a.list.tag_to_assign.as_deref())
                    .unwrap_or("")
                    .to_string()
                    .into(),
            ),
            "search" => {
                let q = self
                    .app
                    .map(|a| {
                        if let Some(crate::popups::ActivePopup::Search(popup)) = &a.popups.active {
                            popup.input.lines().join("")
                        } else {
                            "".to_string()
                        }
                    })
                    .unwrap_or_default();
                Some(q.into())
            }
            "grep" => {
                let q = self
                    .app
                    .map(|a| {
                        if let Some(crate::popups::ActivePopup::Search(popup)) = &a.popups.active {
                            let parsed =
                                crate::app::parse_search_query(&popup.input.lines().join(""));
                            if parsed.grep_mode { "on" } else { "off" }
                        } else {
                            "off"
                        }
                    })
                    .unwrap_or("off");
                Some(q.into())
            }
            "tag_filter" => {
                let t = self
                    .app
                    .map(|a| {
                        if let Some(crate::popups::ActivePopup::Search(popup)) = &a.popups.active {
                            let parsed =
                                crate::app::parse_search_query(&popup.input.lines().join(""));
                            parsed
                                .tag_filter
                                .as_ref()
                                .map(|tags| tags.join(", "))
                                .unwrap_or_default()
                        } else {
                            "".to_string()
                        }
                    })
                    .unwrap_or_default();
                Some(t.into())
            }
            "folder_filter" => {
                let f = self
                    .app
                    .map(|a| {
                        if let Some(crate::popups::ActivePopup::Search(popup)) = &a.popups.active {
                            let parsed =
                                crate::app::parse_search_query(&popup.input.lines().join(""));
                            parsed.folder_filter.clone().unwrap_or_default()
                        } else {
                            "".to_string()
                        }
                    })
                    .unwrap_or_default();
                Some(f.into())
            }
            "pinned_count" => {
                let count = self
                    .app
                    .map(|a| a.notes.iter().filter(|n| n.pinned).count())
                    .unwrap_or(0);
                Some(count.to_string().into())
            }
            "pinned_on_top" => Some(
                (if self.app.map(|a| a.pinned_on_top).unwrap_or(false) {
                    "on"
                } else {
                    "off"
                })
                .into(),
            ),
            "folders_first" => Some(
                (if self.app.map(|a| a.list.folders_first).unwrap_or(false) {
                    "on"
                } else {
                    "off"
                })
                .into(),
            ),
            "list_preview" => Some(
                (if self.app.map(|a| a.list.preview_enabled).unwrap_or(false) {
                    "on"
                } else {
                    "off"
                })
                .into(),
            ),
            "calendar" => Some(
                (if self.app.map(|a| a.list.calendar_enabled).unwrap_or(false) {
                    "on"
                } else {
                    "off"
                })
                .into(),
            ),
            "layout_edit" => Some(
                (if self.app.map(|a| a.layout_edit).unwrap_or(false) {
                    "on"
                } else {
                    "off"
                })
                .into(),
            ),

            // Note (List + Edit)
            "note_title" => Some(
                self.note
                    .map(|n| n.title.as_str())
                    .unwrap_or("")
                    .to_string()
                    .into(),
            ),
            "note_id" => Some(
                self.note
                    .map(|n| n.id.as_str())
                    .unwrap_or("")
                    .to_string()
                    .into(),
            ),
            "note_folder" => Some(
                self.note
                    .map(|n| n.folder.as_str())
                    .unwrap_or("")
                    .to_string()
                    .into(),
            ),
            "note_format" => {
                let ext = self
                    .note
                    .and_then(|n| {
                        Path::new(&n.id)
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_default();
                Some(ext.into())
            }
            "note_size" => {
                let sz = self
                    .note
                    .map(|n| format_size(n.size_bytes))
                    .unwrap_or_default();
                Some(sz.into())
            }
            "note_links" => {
                let count = self.note.map(|n| n.links.len()).unwrap_or(0);
                Some(count.to_string().into())
            }
            "tags" => Some(
                self.note
                    .map(|n| n.tags.join(", "))
                    .unwrap_or_default()
                    .into(),
            ),
            "has_tags" => {
                let has = self.note.map(|n| !n.tags.is_empty()).unwrap_or(false);
                Some((if has { "on" } else { "off" }).into())
            }
            "note_pinned" => {
                let pinned = self.note.map(|n| n.pinned).unwrap_or(false);
                Some((if pinned { "on" } else { "off" }).into())
            }
            "note_updated" => {
                let formatted = self
                    .note
                    .map(|n| {
                        let fmt = self
                            .date_format
                            .or_else(|| self.app.map(|a| a.date_format.as_str()))
                            .unwrap_or("%Y-%m-%d");
                        format_date(n.updated_at, fmt)
                    })
                    .unwrap_or_default();
                Some(formatted.into())
            }
            "note_updated_rel" => {
                let rel = self
                    .note
                    .map(|n| format_relative_time(n.updated_at).into_owned())
                    .unwrap_or_default();
                Some(rel.into())
            }
            "prev_note" => Some(
                self.preview_info
                    .and_then(|i| i.prev_name.clone())
                    .unwrap_or_default()
                    .into(),
            ),
            "next_note" => Some(
                self.preview_info
                    .and_then(|i| i.next_name.clone())
                    .unwrap_or_default()
                    .into(),
            ),

            // Edit view
            "word_count" | "line_count" | "char_count" | "cursor_line" | "cursor_col"
            | "modified" | "reading_time" | "header_count" | "task_count" | "has_tasks"
            | "has_frontmatter" | "words_added" | "editing_id" | "editing_template"
            | "line_numbers" | "editor_preview" | "ext_editor" | "ext_editor_enabled" => {
                let app = match self.app {
                    Some(a) => a,
                    None => return Some("".into()),
                };
                if self.view != ViewMode::Edit {
                    return Some("".into());
                }

                let title = self
                    .note
                    .map(|n| n.title.as_str())
                    .unwrap_or("Untitled note");
                let content = app.editor.editor.lines().join("\n");
                let word_count = crate::goals::count_words(&content);

                let val = match name {
                    "word_count" => word_count.to_string(),
                    "line_count" => app.editor.editor.lines().len().to_string(),
                    "char_count" => content.chars().count().to_string(),
                    "cursor_line" => (app.editor.editor.cursor().0 + 1).to_string(),
                    "cursor_col" => (app.editor.editor.cursor().1 + 1).to_string(),
                    "modified" => {
                        let is_mod = if let Some(id) = &app.editor.editing_id {
                            if let Ok(note) = app.storage.load_note(id) {
                                let current_title =
                                    crate::events::get_title_text(&app.editor.title_editor);
                                current_title != note.title || content != note.content
                            } else {
                                false
                            }
                        } else if let Some(path) = &app.editor.template_edit_path {
                            if let Ok(orig_content) = std::fs::read_to_string(path) {
                                content != orig_content
                            } else {
                                false
                            }
                        } else {
                            false
                        };
                        (if is_mod { "on" } else { "off" }).to_string()
                    }
                    "reading_time" => ((word_count as f64 / 200.0).ceil() as usize).to_string(),
                    "header_count" => crate::outline::parse::parse_outline(title, &content)
                        .len()
                        .saturating_sub(1)
                        .to_string(),
                    "task_count" => {
                        let count = content
                            .lines()
                            .filter(|l| {
                                let t = l.trim_start();
                                t.starts_with("- [ ] ")
                                    || t.starts_with("- [x] ")
                                    || t.starts_with("* [ ] ")
                                    || t.starts_with("* [x] ")
                            })
                            .count();
                        count.to_string()
                    }
                    "has_tasks" => {
                        let count = content
                            .lines()
                            .filter(|l| {
                                let t = l.trim_start();
                                t.starts_with("- [ ] ")
                                    || t.starts_with("- [x] ")
                                    || t.starts_with("* [ ] ")
                                    || t.starts_with("* [x] ")
                            })
                            .count();
                        (if count > 0 { "on" } else { "off" }).to_string()
                    }
                    "has_frontmatter" => {
                        let has = content.starts_with("---\n");
                        (if has { "on" } else { "off" }).to_string()
                    }
                    "words_added" => {
                        let added = word_count as isize - app.editor.initial_word_count as isize;
                        added.to_string()
                    }
                    "editing_id" => app.editor.editing_id.clone().unwrap_or_default(),
                    "editing_template" => (if app.editor.template_edit_path.is_some() {
                        "on"
                    } else {
                        "off"
                    })
                    .to_string(),
                    "line_numbers" => (if app.editor.show_line_numbers {
                        "on"
                    } else {
                        "off"
                    })
                    .to_string(),
                    "editor_preview" => (if app.editor.editor_preview_enabled {
                        "on"
                    } else {
                        "off"
                    })
                    .to_string(),
                    "ext_editor" => app.editor.external_editor.clone().unwrap_or_default(),
                    "ext_editor_enabled" => (if app.editor.external_editor_enabled {
                        "on"
                    } else {
                        "off"
                    })
                    .to_string(),
                    _ => unreachable!(),
                };
                Some(val.into())
            }

            "fps" => {
                if let Some(app) = self.app {
                    Some(format!("{:.0}", app.fps).into())
                } else {
                    Some("".into())
                }
            }

            // Graph view
            "node_count" | "edge_count" | "selected_node" | "viewport_size" | "viewport_ratio"
            | "graph_settled" | "label_mode" | "node_color_mode" | "edge_color_mode"
            | "node_size_mode" | "zoom" | "show_grid" | "show_legend" | "show_minimap" => {
                let graph = match self.graph {
                    Some(g) => g,
                    None => return Some("".into()),
                };

                let graph_inner = graph.simulation.get_graph();

                let val = match name {
                    "node_count" => graph_inner.node_count().to_string(),
                    "edge_count" => graph_inner.edge_count().to_string(),
                    "selected_node" => graph
                        .selected_node
                        .and_then(|idx| graph_inner.node_weight(idx))
                        .map(|n| n.data.title.clone())
                        .unwrap_or_else(|| "none".to_string()),
                    "viewport_size" => {
                        let aspect = self
                            .area
                            .map(|a| a.width as f64 / a.height as f64)
                            .unwrap_or(1.0);
                        let x_bounds = graph.viewport.x_bounds(aspect);
                        let y_bounds = graph.viewport.y_bounds(aspect);
                        let (gx_min, gx_max, gy_min, gy_max) = graph.graph_bounds;
                        let graph_w = gx_max - gx_min;
                        let graph_h = gy_max - gy_min;
                        let vp_w = x_bounds[1] - x_bounds[0];
                        let vp_h = y_bounds[1] - y_bounds[0];
                        let graph_area = graph_w * graph_h;
                        let vp_area = vp_w * vp_h;
                        let size_pct = if graph_area > 0.0 {
                            (vp_area / graph_area * 100.0).clamp(0.0, 100.0)
                        } else {
                            100.0
                        };
                        format!("{:.0}", size_pct)
                    }
                    "viewport_ratio" => {
                        let (gx_min, gx_max, gy_min, gy_max) = graph.graph_bounds;
                        let graph_w = gx_max - gx_min;
                        let graph_h = gy_max - gy_min;
                        let range = graph_w.max(graph_h).max(1.0) * 1.4;
                        let full_zoom = 200.0 / range;
                        let ratio = graph.viewport.zoom / full_zoom;
                        format!("{:.1}", ratio)
                    }
                    "graph_settled" => (if graph.is_settled { "on" } else { "off" }).to_string(),
                    "label_mode" => {
                        let lm = match self.config.graf.visual.label_mode {
                            crate::config::LabelMode::Selected => "selected",
                            crate::config::LabelMode::Neighbors => "neighbors",
                            crate::config::LabelMode::All => "all",
                            crate::config::LabelMode::None => "none",
                        };
                        lm.to_string()
                    }
                    "node_color_mode" => {
                        let ncm = match self.config.graf.visual.node_color_mode {
                            crate::config::NodeColorMode::Tag => "tag",
                            crate::config::NodeColorMode::Folder => "folder",
                            crate::config::NodeColorMode::LinkCount => "link_count",
                            crate::config::NodeColorMode::Uniform => "uniform",
                        };
                        ncm.to_string()
                    }
                    "edge_color_mode" => {
                        let ecm = match self.config.graf.visual.edge_color_mode {
                            crate::config::EdgeColorMode::Source => "source",
                            crate::config::EdgeColorMode::Target => "target",
                            crate::config::EdgeColorMode::Uniform => "uniform",
                        };
                        ecm.to_string()
                    }
                    "node_size_mode" => {
                        let nsm = match self.config.graf.visual.node_size_mode {
                            crate::config::NodeSizeMode::Fixed => "fixed",
                            crate::config::NodeSizeMode::LinkCount => "link_count",
                        };
                        nsm.to_string()
                    }
                    "zoom" => graph.viewport.zoom.to_string(),
                    "show_grid" => (if self.config.graf.visual.show_grid {
                        "on"
                    } else {
                        "off"
                    })
                    .to_string(),
                    "show_legend" => (if self.config.graf.visual.show_legend {
                        "on"
                    } else {
                        "off"
                    })
                    .to_string(),
                    "show_minimap" => (if self.config.graf.visual.show_minimap {
                        "on"
                    } else {
                        "off"
                    })
                    .to_string(),
                    _ => unreachable!(),
                };
                Some(val.into())
            }

            // Draw view
            "tool" | "shape" | "element_count" | "draw_width" | "draw_height" | "draw_grid"
            | "draw_zoom" | "text_editing" => {
                let draw = match self.draw {
                    Some(d) => d,
                    None => return Some("".into()),
                };

                let val = match name {
                    "tool" => {
                        let t = match draw.active_tool {
                            crate::draw::state::DrawTool::Draw => "draw",
                            crate::draw::state::DrawTool::Erase => "erase",
                            crate::draw::state::DrawTool::Text => "text",
                            crate::draw::state::DrawTool::Shape => "shape",
                        };
                        t.to_string()
                    }
                    "shape" => {
                        let s = match draw.active_shape_type {
                            crate::draw::state::DrawShapeType::Rect => "rect",
                            crate::draw::state::DrawShapeType::Ellipse => "ellipse",
                            crate::draw::state::DrawShapeType::Diamond => "diamond",
                            crate::draw::state::DrawShapeType::Line => "line",
                            crate::draw::state::DrawShapeType::Arrow => "arrow",
                        };
                        s.to_string()
                    }
                    "element_count" => draw.data.elements.len().to_string(),
                    "draw_width" => draw.data.width.to_string(),
                    "draw_height" => draw.data.height.to_string(),
                    "draw_grid" => (if draw.show_grid { "on" } else { "off" }).to_string(),
                    "draw_zoom" => format!("{:.1}", draw.viewport.zoom),
                    "text_editing" => (if draw.text_editor.is_some() {
                        "on"
                    } else {
                        "off"
                    })
                    .to_string(),
                    _ => unreachable!(),
                };
                Some(val.into())
            }

            // Canvas / Pinstar view
            "canvas_nodes" | "canvas_edges" | "canvas_zoom" | "canvas_pan_x" | "canvas_pan_y"
            | "canvas_selected" | "canvas_grid" | "canvas_editor" => {
                let canvas = match self.canvas {
                    Some(c) => c,
                    None => return Some("".into()),
                };

                let val = match name {
                    "canvas_nodes" => canvas.data.nodes.len().to_string(),
                    "canvas_edges" => canvas.data.edges.len().to_string(),
                    "canvas_zoom" => format!("{:.1}", canvas.zoom),
                    "canvas_pan_x" => canvas.viewport_x.to_string(),
                    "canvas_pan_y" => canvas.viewport_y.to_string(),
                    "canvas_selected" => canvas.selected_node_id.clone().unwrap_or_default(),
                    "canvas_grid" => (if canvas.show_grid { "on" } else { "off" }).to_string(),
                    "canvas_editor" => {
                        (if canvas.show_editor_pane { "on" } else { "off" }).to_string()
                    }
                    _ => unreachable!(),
                };
                Some(val.into())
            }

            // Outline view
            "outline_nodes" | "outline_headers" | "outline_visible" | "outline_cursor"
            | "outline_depth" | "outline_max_depth" | "outline_expanded" | "outline_heading"
            | "outline_note" | "outline_error" => {
                let ct = match self.outline {
                    Some(c) => c,
                    None => return Some("".into()),
                };

                let val = match name {
                    "outline_nodes" => ct.nodes.len().to_string(),
                    "outline_headers" => ct
                        .nodes
                        .iter()
                        .filter(|n| {
                            matches!(n.kind, crate::outline::parse::NodeKind::Header { .. })
                        })
                        .count()
                        .to_string(),
                    "outline_visible" => ct.visible_indices().len().to_string(),
                    "outline_cursor" => (ct.selected + 1).to_string(),
                    "outline_depth" => ct
                        .nodes
                        .get(ct.selected)
                        .map(|n| n.depth)
                        .unwrap_or(0)
                        .to_string(),
                    "outline_max_depth" => ct
                        .nodes
                        .iter()
                        .map(|n| n.depth)
                        .max()
                        .unwrap_or(0)
                        .to_string(),
                    "outline_expanded" => ct.expanded.len().to_string(),
                    "outline_heading" => ct
                        .nodes
                        .get(ct.selected)
                        .and_then(|n| {
                            if let crate::outline::parse::NodeKind::Header { title, .. } = &n.kind {
                                Some(title.clone())
                            } else {
                                None
                            }
                        })
                        .unwrap_or_default(),
                    "outline_note" => ct.note_title.clone(),
                    "outline_error" => if ct.load_error { "error" } else { "" }.to_string(),
                    _ => unreachable!(),
                };
                Some(val.into())
            }

            // Backup view
            "branch" | "ahead" | "behind" | "staged" | "unstaged" | "untracked"
            | "commit_count" | "last_commit" | "last_commit_msg" | "last_commit_author"
            | "last_commit_time" | "remote" | "remote_url" | "backup_section" | "input_mode"
            | "auto_push" | "repo_dirty" | "modified_text" => {
                let backup = match self.backup {
                    Some(b) => b,
                    None => return Some("".into()),
                };

                let val = match name {
                    "branch" => backup
                        .status
                        .as_ref()
                        .map(|s| s.branch.clone())
                        .unwrap_or_default(),
                    "ahead" => backup
                        .status
                        .as_ref()
                        .map(|s| s.ahead.to_string())
                        .unwrap_or_else(|| "0".to_string()),
                    "behind" => backup
                        .status
                        .as_ref()
                        .map(|s| s.behind.to_string())
                        .unwrap_or_else(|| "0".to_string()),
                    "staged" => backup
                        .status
                        .as_ref()
                        .map(|s| s.staged.len().to_string())
                        .unwrap_or_else(|| "0".to_string()),
                    "unstaged" => backup
                        .status
                        .as_ref()
                        .map(|s| s.unstaged.len().to_string())
                        .unwrap_or_else(|| "0".to_string()),
                    "untracked" => backup
                        .status
                        .as_ref()
                        .map(|s| s.untracked.len().to_string())
                        .unwrap_or_else(|| "0".to_string()),
                    "commit_count" => backup.commits.len().to_string(),
                    "last_commit" => backup
                        .commits
                        .first()
                        .map(|c| c.id[..std::cmp::min(7, c.id.len())].to_string())
                        .unwrap_or_default(),
                    "last_commit_msg" => backup
                        .commits
                        .first()
                        .map(|c| c.message.clone())
                        .unwrap_or_default(),
                    "last_commit_author" => backup
                        .commits
                        .first()
                        .map(|c| c.author.clone())
                        .unwrap_or_default(),
                    "last_commit_time" => backup
                        .commits
                        .first()
                        .map(|c| format_relative_time(c.time).into_owned())
                        .unwrap_or_default(),
                    "remote" => backup.settings.remote_name.lines().join(""),
                    "remote_url" => backup.settings.remote_url.lines().join(""),
                    "backup_section" => match backup.selected_section {
                        crate::backup::state::BackupSection::Status => "status",
                        crate::backup::state::BackupSection::History => "history",
                    }
                    .to_string(),
                    "input_mode" => match backup.input_mode {
                        crate::backup::state::BackupInputMode::Normal => "normal",
                        crate::backup::state::BackupInputMode::EditCommitMessage => "edit_commit",
                        crate::backup::state::BackupInputMode::EditSettings => "edit_settings",
                        crate::backup::state::BackupInputMode::EditSettingsField => {
                            "edit_settings_field"
                        }
                    }
                    .to_string(),
                    "auto_push" => (if backup.settings.auto_push {
                        "on"
                    } else {
                        "off"
                    })
                    .to_string(),
                    "repo_dirty" => {
                        let dirty = backup
                            .status
                            .as_ref()
                            .map(|s| {
                                !s.staged.is_empty()
                                    || !s.unstaged.is_empty()
                                    || !s.untracked.is_empty()
                            })
                            .unwrap_or(false);
                        (if dirty { "on" } else { "off" }).to_string()
                    }
                    "modified_text" => if let Some(status) = &backup.status {
                        if !status.staged.is_empty()
                            || !status.unstaged.is_empty()
                            || !status.untracked.is_empty()
                        {
                            "modified"
                        } else {
                            "clean"
                        }
                    } else {
                        ""
                    }
                    .to_string(),
                    _ => unreachable!(),
                };
                Some(val.into())
            }

            _ => None,
        }
    }
}

#[allow(unused_assignments)]
pub fn render_segments<'a>(
    template: &str,
    ctx: &StatuslineContext<'a>,
    _theme: &AppThemeColors,
) -> Vec<Segment<'a>> {
    let mut segments = Vec::new();

    // Cell assembler state.
    let mut cur = String::new();
    let mut cur_has_var = false;

    macro_rules! flush_cell {
        () => {{
            let trimmed = cur.trim();
            if !trimmed.is_empty() {
                segments.push(Segment::Text(trimmed.to_string()));
            }
            cur.clear();
            cur_has_var = false;
        }};
    }

    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            if chars.peek() == Some(&'{') {
                chars.next();
                cur.push('{');
            } else {
                let mut name = String::new();
                let mut found_close = false;
                while let Some(&nc) = chars.peek() {
                    if nc == '}' {
                        chars.next();
                        found_close = true;
                        break;
                    } else if nc == '{' {
                        break;
                    } else {
                        name.push(nc);
                        chars.next();
                    }
                }

                if found_close {
                    match name.as_str() {
                        "preview" | "detail" | "hints" | "badge" | "pending" => {
                            // Composites are their own segments: flush any pending
                            // text cell first, then push the composite directly.
                            flush_cell!();
                            let spans = match name.as_str() {
                                "preview" => ctx.preview.clone(),
                                "detail" => ctx.detail.clone(),
                                "hints" => ctx.hints.clone(),
                                "badge" => ctx.badge.clone(),
                                "pending" => ctx.pending.clone(),
                                _ => None,
                            };
                            if let Some(spans) = spans {
                                if name == "detail" {
                                    segments.push(Segment::CompositeSplittable(spans));
                                } else {
                                    segments.push(Segment::Composite(spans));
                                }
                            }
                        }
                        _ => {
                            // Boundary: a var cell is open and a whitespace run
                            // separates it from this new var → close the old cell.
                            if cur_has_var
                                && cur.chars().last().is_some_and(|ch| ch.is_whitespace())
                            {
                                flush_cell!();
                            }
                            if let Some(val) = ctx.resolve(&name) {
                                cur.push_str(&val);
                                cur_has_var = true;
                            } else {
                                // Unresolved name → literal text (not a var anchor).
                                cur.push('{');
                                cur.push_str(&name);
                                cur.push('}');
                            }
                        }
                    }
                } else {
                    // Unclosed '{' → literal.
                    cur.push('{');
                    cur.push_str(&name);
                }
            }
        } else if c == '}' {
            if chars.peek() == Some(&'}') {
                chars.next();
                cur.push('}');
            } else {
                cur.push('}');
            }
        } else {
            // Detect explicit " | " separator when a var cell is open: it forces a
            // cell boundary and is consumed (not rendered). Backtrack-safe via clone
            // (std::str::Chars: Clone; Peekable<I>: Clone when I: Clone).
            if c == ' ' && cur_has_var && chars.peek() == Some(&'|') {
                let mut probe = chars.clone();
                probe.next(); // consume '|'
                if probe.peek() == Some(&' ') {
                    chars.next(); // consume '|'
                    chars.next(); // consume ' '
                    flush_cell!();
                    continue;
                }
            }
            cur.push(c);
        }
    }

    flush_cell!();
    segments
}

pub fn line_from_segments<'a>(
    segs: &[Segment<'a>],
    theme: &AppThemeColors,
    is_header: bool,
    is_right: bool,
) -> Line<'a> {
    let mut flat = Vec::new();
    for seg in segs {
        match seg {
            Segment::Text(text) => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    flat.push(FlatSegment::Cell(trimmed.to_string()));
                }
            }
            Segment::Composite(spans) => {
                if !spans.is_empty() {
                    flat.push(FlatSegment::Composite(spans.clone()));
                }
            }
            Segment::CompositeSplittable(spans) => {
                if !spans.is_empty() {
                    flat.push(FlatSegment::Splittable(spans.clone()));
                }
            }
        }
    }

    if flat.is_empty() {
        return Line::default();
    }

    let is_powerline = matches!(
        theme.hint_bar_style,
        crate::config::HintBarStyle::Sharp
            | crate::config::HintBarStyle::Rounded
            | crate::config::HintBarStyle::Slanted
    );

    let flat: Vec<FlatSegment> = if is_powerline {
        let mut out = Vec::new();
        for seg in flat {
            match seg {
                FlatSegment::Splittable(spans) => {
                    let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
                    for part in text.split(" | ") {
                        let t = part.trim();
                        if !t.is_empty() {
                            out.push(FlatSegment::Cell(t.to_string()));
                        }
                    }
                }
                other => out.push(other),
            }
        }
        out
    } else {
        flat.into_iter()
            .map(|s| match s {
                FlatSegment::Splittable(spans) => FlatSegment::Composite(spans),
                other => other,
            })
            .collect()
    };

    let mut spans = Vec::new();

    if is_powerline {
        let sep_char = match theme.hint_bar_style {
            crate::config::HintBarStyle::Sharp => {
                if is_right {
                    "\u{e0b2}"
                } else {
                    "\u{e0b0}"
                }
            }
            crate::config::HintBarStyle::Rounded => {
                if is_right {
                    "\u{e0b6}"
                } else {
                    "\u{e0b4}"
                }
            }
            crate::config::HintBarStyle::Slanted => {
                if is_right {
                    "\u{e0be}"
                } else {
                    "\u{e0bc}"
                }
            }
            _ => unreachable!(),
        };

        let bg_colors = [
            theme.accent,
            theme.folder,
            theme.tag,
            theme.warning,
            theme.success,
        ];

        let bar_bg = if is_header {
            theme.title_bar_bg()
        } else {
            theme.hint_line_bg()
        };

        let mut cell_idx = 0;

        if is_right {
            for (idx, seg) in flat.iter().enumerate() {
                match seg {
                    FlatSegment::Cell(text) => {
                        let bg = bg_colors[cell_idx % bg_colors.len()];
                        let prev_bg = if idx > 0 {
                            let prev_cell_idx = cell_idx.saturating_sub(1);
                            get_segment_bg(
                                &flat[idx - 1],
                                prev_cell_idx,
                                is_header,
                                true,
                                theme,
                                &bg_colors,
                            )
                        } else {
                            None
                        };
                        let prev_bg_val = prev_bg.or(bar_bg).unwrap_or(Color::Reset);

                        let mut sep_style = Style::default().fg(bg);
                        if prev_bg.or(bar_bg).is_some() {
                            sep_style = sep_style.bg(prev_bg_val);
                        }
                        spans.push(Span::styled(sep_char, sep_style));

                        spans.push(Span::styled(
                            format!(" {} ", text),
                            Style::default()
                                .bg(bg)
                                .fg(theme.highlight_fg)
                                .add_modifier(Modifier::BOLD),
                        ));

                        cell_idx += 1;
                    }
                    FlatSegment::Composite(comp_spans) => {
                        spans.extend(comp_spans.clone());
                    }
                    FlatSegment::Splittable(_) => unreachable!(),
                }
            }
        } else {
            for (idx, seg) in flat.iter().enumerate() {
                match seg {
                    FlatSegment::Cell(text) => {
                        let bg = if is_header && cell_idx == 0 {
                            theme.heading
                        } else {
                            bg_colors[cell_idx % bg_colors.len()]
                        };

                        spans.push(Span::styled(
                            format!(" {} ", text),
                            Style::default()
                                .bg(bg)
                                .fg(theme.highlight_fg)
                                .add_modifier(Modifier::BOLD),
                        ));

                        let next_bg = if idx + 1 < flat.len() {
                            let next_cell_idx = cell_idx + 1;
                            get_segment_bg(
                                &flat[idx + 1],
                                next_cell_idx,
                                is_header,
                                false,
                                theme,
                                &bg_colors,
                            )
                        } else {
                            None
                        };
                        let next_bg_val = next_bg.or(bar_bg).unwrap_or(Color::Reset);

                        let mut sep_style = Style::default().fg(bg);
                        if next_bg.or(bar_bg).is_some() {
                            sep_style = sep_style.bg(next_bg_val);
                        }
                        spans.push(Span::styled(sep_char, sep_style));

                        cell_idx += 1;
                    }
                    FlatSegment::Composite(comp_spans) => {
                        spans.extend(comp_spans.clone());
                    }
                    FlatSegment::Splittable(_) => unreachable!(),
                }
            }
        }
    } else {
        let palette = [
            theme.accent,
            theme.folder,
            theme.tag,
            theme.warning,
            theme.success,
        ];
        // header_left (the title) uses uniform pre-fix styling — no rotation, no separators.
        let is_header_left = is_header && !is_right;
        let mut cell_idx = 0;
        let mut prev_was_cell = false;
        for seg in flat {
            match seg {
                FlatSegment::Cell(text) => {
                    let style = if is_header_left && cell_idx == 0 {
                        // Title heading badge (unchanged).
                        Style::default()
                            .fg(theme.highlight_fg)
                            .bg(theme.heading)
                            .add_modifier(Modifier::BOLD)
                    } else if is_header_left {
                        // Header-left additional cells: pre-fix uniform color.
                        Style::default().fg(theme.fg)
                    } else {
                        // Header-right + footer: rotating palette.
                        Style::default()
                            .fg(palette[cell_idx % palette.len()])
                            .add_modifier(Modifier::BOLD)
                    };
                    // " · " separators appear only between rotating cells, never in header-left.
                    if !is_header_left && prev_was_cell {
                        spans.push(Span::styled(" · ", Style::default().fg(theme.muted)));
                    }
                    let text_to_render = if is_header_left && cell_idx == 0 {
                        format!(" {} ", text.trim())
                    } else {
                        text
                    };
                    spans.push(Span::styled(text_to_render, style));
                    cell_idx += 1;
                    prev_was_cell = true;
                }
                FlatSegment::Composite(comp_spans) => {
                    spans.extend(comp_spans);
                    prev_was_cell = false;
                }
                FlatSegment::Splittable(_) => unreachable!(),
            }
        }
    }

    Line::from(spans)
}

fn get_segment_bg(
    seg: &FlatSegment<'_>,
    cell_idx: usize,
    is_header: bool,
    is_right: bool,
    theme: &AppThemeColors,
    bg_colors: &[Color],
) -> Option<Color> {
    match seg {
        FlatSegment::Cell(_) => {
            if is_header && !is_right && cell_idx == 0 {
                Some(theme.heading)
            } else {
                Some(bg_colors[cell_idx % bg_colors.len()])
            }
        }
        FlatSegment::Composite(spans) => spans.first().and_then(|s| s.style.bg),
        FlatSegment::Splittable(_) => unreachable!(),
    }
}

pub struct StatuslineTemplates {
    pub header_left: Cow<'static, str>,
    pub header_right: Cow<'static, str>,
    pub footer_left: Cow<'static, str>,
    pub footer_right: Cow<'static, str>,
}

pub fn effective_templates(cfg: &StatuslineConfig, view: ViewMode) -> StatuslineTemplates {
    let view_override = match view {
        ViewMode::List => cfg.list.as_ref(),
        ViewMode::Edit => cfg.edit.as_ref(),
        ViewMode::Help => cfg.help.as_ref(),
        ViewMode::Graph => cfg.graph.as_ref(),
        ViewMode::Draw => cfg.draw.as_ref(),
        ViewMode::Canvas => cfg.canvas.as_ref(),
        ViewMode::Backup => cfg.backup.as_ref(),
        ViewMode::Outline => cfg.outline.as_ref(),
        ViewMode::Setup => None,
    };

    let header_left = view_override
        .and_then(|o| o.header_left.clone())
        .or_else(|| cfg.header_left.clone())
        .map(Cow::Owned)
        .unwrap_or_else(|| default_template(view, "header_left"));

    let header_right = view_override
        .and_then(|o| o.header_right.clone())
        .or_else(|| cfg.header_right.clone())
        .map(Cow::Owned)
        .unwrap_or_else(|| default_template(view, "header_right"));

    let footer_left = view_override
        .and_then(|o| o.footer_left.clone())
        .or_else(|| cfg.footer_left.clone())
        .map(Cow::Owned)
        .unwrap_or_else(|| default_template(view, "footer_left"));

    let footer_right = view_override
        .and_then(|o| o.footer_right.clone())
        .or_else(|| cfg.footer_right.clone())
        .map(Cow::Owned)
        .unwrap_or_else(|| default_template(view, "footer_right"));

    StatuslineTemplates {
        header_left,
        header_right,
        footer_left,
        footer_right,
    }
}

fn default_template(view: ViewMode, field: &str) -> Cow<'static, str> {
    match field {
        "header_left" => "{title} {preview}".into(),
        "footer_left" => "{pending}{badge}{hints}".into(),
        "header_right" => {
            match view {
                ViewMode::Graph => "Nodes: {node_count} | Edges: {edge_count} | Selected: {selected_node} | Ratio: {viewport_ratio}x | FPS: {fps}   ".into(),
                ViewMode::Backup => "{branch} | ↑{ahead} ↓{behind} | {modified_text}".into(),
                ViewMode::List => "{detail}".into(),
                ViewMode::Setup => "{pinned_count} pinned".into(),
                ViewMode::Help => "Page {help_page}/{help_total_pages}".into(),
                ViewMode::Edit => "{word_count}w {char_count}c {cursor_line}:{cursor_col}".into(),
                _ => "".into(),
            }
        }
        "footer_right" => match view {
            ViewMode::List => "{note_count} notes ({selected_count} selected) | {version}".into(),
            _ => "{version}".into(),
        },
        _ => "".into(),
    }
}

pub fn render_header<'a>(
    ctx: &StatuslineContext<'a>,
    cfg: &StatuslineConfig,
    view: ViewMode,
    theme: &AppThemeColors,
) -> (Line<'a>, Option<Line<'a>>) {
    let tmpl = effective_templates(cfg, view);
    let left_segs = render_segments(&tmpl.header_left, ctx, theme);
    let right_segs = render_segments(&tmpl.header_right, ctx, theme);

    let left_line = line_from_segments(&left_segs, theme, true, false);
    let right_line = line_from_segments(&right_segs, theme, true, true);

    let right_opt = if right_line.width() > 0 && tmpl.header_right.trim() != "" {
        Some(right_line)
    } else {
        None
    };

    (left_line, right_opt)
}

pub fn render_footer<'a>(
    ctx: &StatuslineContext<'a>,
    cfg: &StatuslineConfig,
    view: ViewMode,
    theme: &AppThemeColors,
) -> (Line<'a>, Option<Line<'a>>) {
    let tmpl = effective_templates(cfg, view);
    let left_segs = render_segments(&tmpl.footer_left, ctx, theme);
    let right_segs = render_segments(&tmpl.footer_right, ctx, theme);

    let left_line = line_from_segments(&left_segs, theme, false, false);
    let right_line = line_from_segments(&right_segs, theme, false, true);

    let right_opt = if right_line.width() > 0 && tmpl.footer_right.trim() != "" {
        Some(right_line)
    } else {
        None
    };

    (left_line, right_opt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ClinConfig;

    fn text_cells(segs: Vec<Segment>) -> Vec<String> {
        segs.into_iter()
            .filter_map(|s| match s {
                Segment::Text(t) => Some(t),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn test_render_segments_escapes() {
        let config = ClinConfig::default();
        let ctx = StatuslineContext::for_overlay(&config, ViewMode::List);
        let theme = AppThemeColors::default();

        let segs = render_segments("hello {{world}} {view} {invalid_var}", &ctx, &theme);
        assert_eq!(
            text_cells(segs),
            vec![
                "hello {world} Notes".to_string(),
                "{invalid_var}".to_string()
            ]
        );
    }

    #[test]
    fn test_statusline_per_variable_cells() {
        let config = ClinConfig::default();
        let ctx = StatuslineContext::for_overlay(&config, ViewMode::List);
        let theme = AppThemeColors::default();

        // Each whitespace-separated variable is its own cell.
        let segs = render_segments("{view} {preset}", &ctx, &theme);
        assert_eq!(
            text_cells(segs),
            vec!["Notes".to_string(), "default".to_string()]
        );
    }

    #[test]
    fn test_statusline_label_glues_to_var() {
        let config = ClinConfig::default();
        let ctx = StatuslineContext::for_overlay(&config, ViewMode::List);
        let theme = AppThemeColors::default();

        // A literal label before a var stays in the same cell.
        let segs = render_segments("Notes: {view}", &ctx, &theme);
        assert_eq!(text_cells(segs), vec!["Notes: Notes".to_string()]);
    }

    #[test]
    fn test_statusline_pipe_separator() {
        let config = ClinConfig::default();
        let ctx = StatuslineContext::for_overlay(&config, ViewMode::List);
        let theme = AppThemeColors::default();

        // Explicit " | " forces a boundary; the pipe is consumed, not rendered.
        let segs = render_segments("{view} | {preset}", &ctx, &theme);
        assert_eq!(
            text_cells(segs),
            vec!["Notes".to_string(), "default".to_string()]
        );
    }

    #[test]
    fn test_default_statusline_rendering() {
        let config = ClinConfig::default();
        let ctx = StatuslineContext::for_overlay(&config, ViewMode::List);
        let theme = AppThemeColors::default();

        let (left, right) = render_header(&ctx, &config.statusline, ViewMode::List, &theme);
        assert_eq!(left.width(), 7); // " Notes " (7 chars)
        assert!(right.is_none());
    }
}
