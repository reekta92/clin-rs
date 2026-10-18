//! Command capabilities are derived from the live owner, never a hidden Notes selection.
use crate::app::{App, EditFocus, ViewMode};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ActionContext {
    pub mode: ViewMode,
    vault: PathBuf,
    pub note_id: Option<String>,
    folder: Option<String>,
    creation_folder: Option<String>,
    note_stamp: Option<(u64, Option<std::time::SystemTime>)>,
    owner: Option<PathBuf>,
    pub focus: Option<EditFocus>,
}

impl ActionContext {
    pub fn capture(app: &App, focus: Option<EditFocus>, note_id: Option<&str>) -> Self {
        let list = app.mode == ViewMode::List;
        let note_id = list
            .then(|| {
                note_id
                    .map(str::to_owned)
                    .or_else(|| app.get_selected_note_id())
            })
            .flatten();
        let note_stamp = note_id.as_ref().and_then(|id| Self::stamp(app, id));
        Self {
            mode: app.mode,
            vault: app.storage.data_dir.clone(),
            note_id,
            note_stamp,
            creation_folder: list.then(|| {
                if app.list.notes_layout == crate::config::NotesLayout::Grid {
                    app.list.grid_folder.clone()
                } else {
                    app.get_current_folder_context()
                }
            }),
            folder: if list {
                match app.list.visual_list.get(app.list.visual_index) {
                    Some(crate::list_view::VisualItem::Folder { path, .. }) => Some(path.clone()),
                    _ => None,
                }
            } else {
                None
            },
            owner: Self::owner(app),
            focus,
        }
    }

    fn owner(app: &App) -> Option<PathBuf> {
        match app.mode {
            ViewMode::Edit => app.editor.template_edit_path.clone().or_else(|| {
                app.editor
                    .editing_id
                    .as_ref()
                    .map(|id| app.storage.note_path(id))
            }),
            ViewMode::Canvas => app.canvas_state.as_ref().map(|s| s.state.path.clone()),
            ViewMode::Draw => app
                .draw_state
                .as_ref()
                .and_then(|s| s.current_file.as_ref())
                .map(|id| app.storage.note_path(id)),
            ViewMode::Bases => app
                .bases_state
                .as_ref()
                .map(|s| app.storage.notes_dir.join(&s.id)),
            ViewMode::Outline => app
                .outline_state
                .as_ref()
                .map(|s| app.storage.note_path(&s.note_id)),
            _ => None,
        }
    }

    pub fn origin_current(&self, app: &App) -> bool {
        self.mode == app.mode && self.vault == app.storage.data_dir
    }

    fn owner_current(&self, app: &App) -> bool {
        self.origin_current(app)
            && self.owner == Self::owner(app)
            && self.owner.as_ref().is_some_and(|path| path.is_file())
    }

    fn stamp(app: &App, id: &str) -> Option<(u64, Option<std::time::SystemTime>)> {
        std::fs::metadata(app.storage.note_path(id))
            .ok()
            .filter(|metadata| metadata.is_file())
            .map(|metadata| (metadata.len(), metadata.modified().ok()))
    }

    fn note<'a>(&'a self, app: &App) -> Option<&'a str> {
        self.note_id.as_deref().filter(|id| {
            self.origin_current(app)
                && self.note_stamp.is_some()
                && self.note_stamp == Self::stamp(app, id)
        })
    }

    pub fn markdown_editor(&self, app: &App) -> bool {
        self.mode == ViewMode::Edit
            && self.owner_current(app)
            && app.editor.template_edit_path.is_none()
            && app.editor.editing_id.as_ref().is_some_and(|id| {
                id.ends_with(".md")
                    && (app.editor.public_document.is_none()
                        || app.editor.public_document_body_only)
            })
    }

    fn creation_folder_current(&self, app: &App) -> bool {
        self.creation_folder.as_ref().is_some_and(|folder| {
            *folder
                == if app.list.notes_layout == crate::config::NotesLayout::Grid {
                    app.list.grid_folder.clone()
                } else {
                    app.get_current_folder_context()
                }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Scope {
    Shared,
    Navigation,
    List,
    Note,
    Info,
    Presentation,
    Editor,
    Insert,
    Markdown,
    Image,
    Graph,
    Bases,
    CreateBase,
    Setup,
}

/// Exhaustive inventory: a newly registered action must choose its owner explicitly.
pub(super) fn scope(id: &str) -> Option<Scope> {
    use Scope::{
        Bases, CreateBase, Editor, Graph, Image, Info, Insert, List, Markdown, Navigation, Note,
        Presentation, Setup, Shared,
    };
    Some(match id {
        "switch_theme"
        | "keybind.preset"
        | "settings.icon_mode"
        | "settings.hint_bar_style"
        | "settings.tab_icons_only"
        | "settings.scrollbars"
        | "settings.scrollbar_pan_mode"
        | "settings.confirm_quit" => Shared,
        "notes.open" | "help.open" | "graph.open" | "setup_wizard" | "vault.switch"
        | "navigation.back" | "navigation.forward" | "document.save" | "app.quit" => Navigation,
        "draw.create"
        | "create_canvas"
        | "external_editor.toggle"
        | "toggle_notes_layout"
        | "templates.edit_folder_variables"
        | "settings.preview_pane"
        | "settings.calendar"
        | "settings.confirm_delete"
        | "settings.pinned_on_top"
        | "settings.inline_info"
        | "settings.cycle_sort"
        | "settings.show_hidden_files"
        | "settings.show_all_files"
        | "settings.layout_edit"
        | "settings.folders_first"
        | "settings.smart_folders"
        | "settings.configure_smart_folders"
        | "settings.show_file_size"
        | "settings.cycle_list_density"
        | "settings.cycle_week_start"
        | "settings.folder_graph_preview"
        | "settings.toggle_graph_view"
        | "settings.toggle_canvas_view"
        | "settings.toggle_draw_view"
        | "settings.toggle_outline_view"
        | "settings.toggle_help_view"
        | "settings.toggle_tags"
        | "settings.toggle_trash"
        | "settings.toggle_subnotes"
        | "settings.toggle_templates"
        | "settings.toggle_import"
        | "insert.file_new"
        | "insert.csv_new"
        | "insert.json_new"
        | "insert.url_new"
        | "insert.clipboard_new" => List,
        "note.rasterize"
        | "manage_subnotes_list"
        | "outline.open"
        | "ocr.paste"
        | "insert.file_append"
        | "insert.csv_append"
        | "insert.json_append"
        | "insert.url_append"
        | "insert.clipboard_append" => Note,
        "info.show" => Info,
        "bases.create" => CreateBase,
        "settings.wrap"
        | "settings.word_goal"
        | "settings.note_goal"
        | "settings.goals"
        | "settings.syntax_highlighting"
        | "settings.code_line_numbers" => Presentation,
        "editor.frontmatter" => Editor,
        "settings.zen"
        | "settings.line_numbers"
        | "settings.edit_mode_highlight"
        | "settings.ghost_syntax"
        | "settings.extended_markdown" => Editor,
        "settings.text_align" => Markdown,
        "editor.insert_date" => Insert,
        "paste_image" | "insert_image_from_file" => Image,
        "settings.graph_show_legend"
        | "settings.graph_show_minimap"
        | "settings.graph_show_orphan" => Graph,
        "bases.refresh"
        | "bases.next_view"
        | "bases.previous_view"
        | "bases.edit"
        | "bases.clear"
        | "bases.copy"
        | "bases.paste"
        | "bases.select"
        | "bases.select_all"
        | "bases.undo"
        | "bases.redo"
        | "bases.new_note"
        | "bases.export"
        | "bases.move_to_group"
        | "bases.search"
        | "bases.toggle_group"
        | "bases.open_cover"
        | "bases.previous_property"
        | "bases.next_property" => Bases,
        "setup.finish" | "setup.cancel" => Setup,
        _ if crate::bases::palette::configuration_command(id).is_some() => Bases,
        _ => return None,
    })
}

pub(super) fn eligible(id: &str, app: &App, context: &ActionContext) -> bool {
    use Scope::{
        Bases, CreateBase, Editor, Graph, Image, Info, Insert, List, Markdown, Navigation, Note,
        Presentation, Setup, Shared,
    };
    if !context.origin_current(app) {
        return false;
    }
    let Some(scope) = scope(id) else {
        return false;
    };
    if context.mode == ViewMode::Setup {
        return app.setup_state.is_some()
            && matches!(
                id,
                "switch_theme" | "keybind.preset" | "setup.finish" | "setup.cancel"
            );
    }
    let list = context.mode == ViewMode::List;
    let note = context.note(app);
    let text = note.is_some_and(|id| id.ends_with(".md") || id.ends_with(".txt"));
    match scope {
        Shared => true,
        Navigation => match id {
            "navigation.back" => app.can_navigate_history(false),
            "navigation.forward" => app.can_navigate_history(true),
            "document.save" => {
                context.owner == ActionContext::owner(app)
                    && matches!(
                        context.mode,
                        ViewMode::Edit | ViewMode::Draw | ViewMode::Canvas
                    )
            }
            "notes.open" => !list,
            "help.open" => context.mode != ViewMode::Help,
            "graph.open" => context.mode != ViewMode::Graph,
            _ => true,
        },
        List => {
            list && (!matches!(
                id,
                "draw.create"
                    | "create_canvas"
                    | "templates.edit_folder_variables"
                    | "insert.file_new"
                    | "insert.csv_new"
                    | "insert.json_new"
                    | "insert.url_new"
                    | "insert.clipboard_new"
            ) || context.creation_folder_current(app))
        }
        Note => {
            list && text
                && (!matches!(id, "outline.open" | "manage_subnotes_list")
                    || context.note_id == app.get_selected_note_id())
        }
        Info => {
            list && context.note_id == app.get_selected_note_id()
                && (text
                    || note.is_some_and(|id| id.ends_with(".clin"))
                    || context.folder.as_ref().is_some_and(|folder| {
                        !App::is_virtual_path(folder)
                            && app.storage.notes_dir.join(folder).is_dir()
                            && *folder == app.get_current_folder_context()
                    }))
        }
        Presentation => list || context.mode == ViewMode::Edit,
        Editor => {
            context.mode == ViewMode::Edit
                && (id != "editor.frontmatter" || app.editor.frontmatter_supported())
        }
        Markdown => context.markdown_editor(app),
        Insert => {
            context.mode == ViewMode::Edit
                && context.owner_current(app)
                && context.focus == Some(EditFocus::Body)
                && app.editor.template_edit_path.is_none()
                && (app.editor.public_document.is_none() || app.editor.public_document_body_only)
                && app
                    .editor
                    .editing_id
                    .as_ref()
                    .is_some_and(|id| id.ends_with(".md") || id.ends_with(".txt"))
        }
        Image => {
            (context.focus == Some(EditFocus::Body) && context.markdown_editor(app))
                || (context.mode == ViewMode::Canvas
                    && context.owner_current(app)
                    && app
                        .canvas_state
                        .as_ref()
                        .is_some_and(|s| !s.state.locked && s.palette_input_available()))
        }
        Graph => context.mode == ViewMode::Graph && app.graph_plugin.is_some(),
        Bases => {
            context.mode == ViewMode::Bases
                && context.owner_current(app)
                && app
                    .bases_state
                    .as_ref()
                    .is_some_and(|s| s.palette_command_available(id))
        }
        CreateBase => {
            (list && context.creation_folder_current(app))
                || (context.mode == ViewMode::Bases
                    && context.owner_current(app)
                    && app
                        .bases_state
                        .as_ref()
                        .is_some_and(|s| s.palette_input_available() && !s.is_writing()))
        }
        Setup => false,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn palette_registry_classifies_every_unique_action_and_denies_unknown_ids() {
        let mut ids = std::collections::HashSet::new();
        for action in super::super::get_all_actions() {
            let id = action.id();
            assert!(ids.insert(id.to_string()), "duplicate registration: {id}");
            assert!(super::scope(&id).is_some(), "unclassified action: {id}");
        }
        assert!(super::scope("settings.future").is_none());
        assert!(super::scope("bases.future").is_none());
    }
}
