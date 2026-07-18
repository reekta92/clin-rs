use crate::app::HelpTab;
use crate::config::{ClinConfig, KeybindPreset};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TipRequirement {
    #[default]
    None,
    /// Needs a numeric count prefix (only parsed when counts_enabled -> Vim/Helix).
    Counts,
    /// Needs multi-key sequence support (enable_key_sequences, or a preset that uses them).
    Sequences,
    /// Exists only under a specific preset.
    Preset(KeybindPreset),
}

impl TipRequirement {
    /// Returns a human-readable caveat string iff the current config does NOT satisfy this.
    /// None when satisfied (or no requirement) -> nothing rendered.
    pub fn caveat_if_unsatisfied(&self, config: &ClinConfig) -> Option<String> {
        match self {
            TipRequirement::None => None,
            TipRequirement::Counts if config.counts_enabled() => None,
            TipRequirement::Counts => Some("needs Vim/Helix preset for the count prefix".into()),
            TipRequirement::Sequences if config.sequences_enabled() => None,
            TipRequirement::Sequences => Some("needs enable_key_sequences = true".into()),
            TipRequirement::Preset(want) if config.core.keybind_preset == *want => None,
            TipRequirement::Preset(want) => Some(format!("needs {want} preset")),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HelpSuggestion {
    pub title: &'static str,
    pub body: &'static str,
    pub requires: TipRequirement,
}

const fn tip(title: &'static str, body: &'static str) -> HelpSuggestion {
    HelpSuggestion {
        title,
        body,
        requires: TipRequirement::None,
    }
}

const fn tip_req(
    title: &'static str,
    body: &'static str,
    requires: TipRequirement,
) -> HelpSuggestion {
    HelpSuggestion {
        title,
        body,
        requires,
    }
}

/// 2–4 sentence static paragraph describing what the tab's view is and does.
pub fn tab_description(tab: HelpTab) -> &'static str {
    match tab {
        HelpTab::Notes => {
            "Browse, organize, and manage your notes and folders. This is the landing view — navigate the list, open notes for editing, create folders, pin important items, tag notes, and switch between list, grid, and tree layouts. Use the command palette or search to jump anywhere."
        }
        HelpTab::Editor => {
            "Write and edit the selected note's title and body. Switch focus between the title field and the markdown body, toggle a live markdown preview, undo/redo edits, and hand off to an external editor like vim or nano. Changes auto-save when you return to the notes list."
        }
        HelpTab::Graph => {
            "Visualize your notes as a force-directed graph of wikilinks and shared tags. Pan and zoom the canvas, search for nodes, open notes directly from the graph, and toggle a minimap, legend, grid, and preview to navigate large vaults visually."
        }
        HelpTab::Draw => {
            "Sketch freehand strokes, drop shapes, and place text labels on a fixed-size drawing canvas. Switch between pen, shape picker, text, and eraser tools. Drawings are stored as .draw files you can reopen and edit."
        }
        HelpTab::Canvas => {
            "An infinite pinstar canvas for connecting notes and drawings spatially. Move and zoom freely, open notes inline, connect nodes with edges, and manage everything through a context menu. Toggle the editor pane and grid to suit your workflow."
        }
        HelpTab::Backup => {
            "Git-backed vault backup dashboard. Stage and unstage file changes, review diffs, write commit messages, push to or pull from a remote, and toggle auto-backup settings inline — all without leaving the app."
        }
        HelpTab::ContentTree => {
            "View the selected note's header hierarchy as a collapsible tree. Jump to any section to scroll the editor there; expand and collapse subtrees to navigate long documents."
        }
        HelpTab::Setup => {
            "First-run and onboarding wizard. Cycle theme, background, hint bar style, icon mode, and keybind preset with a live markdown preview. Re-open anytime from the command palette."
        }
        HelpTab::Templates => {
            "Create reusable note templates with frontmatter and date variables. Pick a template from the notes view, search by name, and auto-fill date tokens when generating a new note. Drop .toml files in the templates directory to add your own."
        }
        HelpTab::About => {
            "Application metadata: version, configuration file paths, and the full command-line interface reference. Use this tab to find config locations and CLI subcommands for notes, storage, keybinds, templates, and config management."
        }
    }
}

/// Curated suggestion pool per tab (min 5 entries; most have 6–7 for variety).
const NOTES_SUGGESTIONS: &[HelpSuggestion] = &[
    tip(
        "Command palette",
        "Press {list:OpenCommandPalette} to open the **Command Palette** and execute any action by typing its name.",
    ),
    tip(
        "Select mode",
        "Press {list:ToggleSelectMode} to toggle **Select Mode** and start selecting multiple notes in the list.",
    ),
    tip(
        "Select item",
        "Use {list:ToggleSelectItem} in **Select Mode** to select or deselect the currently focused note or folder.",
    ),
    tip_req(
        "Expand to depth",
        "Press {list:ExpandToLevel} to expand folders one level, or type a number first (e.g. `` 3 `` then {list:ExpandToLevel}) to expand to that depth.",
        TipRequirement::Counts,
    ),
    tip(
        "Preview pane",
        "Toggle the right-side preview pane using {list:TogglePreview} to view notes without opening them.",
    ),
    tip(
        "Sort order",
        "Cycle the note sorting order using {list:CycleSort} to sort by title, modified time, or size.",
    ),
    tip(
        "Folders first",
        "Press {list:ToggleFoldersFirst} to choose whether **folders** should always stay pinned to the top of the list.",
    ),
    tip(
        "Manage subnotes",
        "Press {list:ManageSubnotes} to view, attach, or manage **subnotes** associated with the selected note.",
    ),
    tip(
        "Create new folder",
        "Use {list:CreateFolder} to create a new **folder** in the currently active path.",
    ),
    tip(
        "Create new note",
        "Press {list:CreateNote} to create a new **note** inside the highlighted folder.",
    ),
    tip(
        "Pin a note",
        "Press {list:TogglePin} to **pin** a note so it stays pinned at the top of the list.",
    ),
    tip(
        "Manage tags",
        "Press {list:ManageTags} to add, rename, or remove **tags** from the selected note.",
    ),
    tip(
        "Open trash",
        "Press {list:OpenTrash} to view and recover deleted notes from the **trash**.",
    ),
    tip(
        "Search notes",
        "Press {list:Search} to jump to the **search bar** and filter notes by title or content.",
    ),
];
const EDITOR_SUGGESTIONS: &[HelpSuggestion] = &[
    tip(
        "Markdown preview",
        "Press {edit:ToggleMarkdownPreview} to toggle a side-by-side **live preview** of your rendered markdown note.",
    ),
    tip(
        "External editor",
        "Press {list:ToggleExternalEditor} from the list to launch your external `` $EDITOR `` (like Vim or Nano) for heavy editing.",
    ),
    tip(
        "Delete word",
        "Use {edit:DeleteWord} to quickly delete the **word** directly preceding your cursor.",
    ),
    tip(
        "Delete next word",
        "Press {edit:DeleteNextWord} to erase the **word** directly following your cursor.",
    ),
    tip(
        "Undo history",
        "Press {edit:Undo} to revert your last text modification; full edit history is kept for your session.",
    ),
    tip(
        "Redo changes",
        "Use {edit:Redo} to re-apply any text modifications that you previously reverted.",
    ),
    tip(
        "Cycle focus",
        "Use {edit:CycleFocus} to quickly tab between editing the note's **title** and the markdown **body**.",
    ),
    tip(
        "Go back",
        "Press {edit:Back} to save your changes and return to the main notes list.",
    ),
    tip(
        "Fullscreen preview",
        "Press {edit:TogglePreviewFullscreen} to expand the markdown **preview** to fill the entire screen.",
    ),
    tip(
        "Manage subnotes",
        "Press {edit:ManageSubnotes} to attach or manage **subnotes** while editing.",
    ),
    tip(
        "Auto-save",
        "Your note is **automatically saved** when you press {edit:Back} to leave the editor — no manual save required.",
    ),
    tip(
        "External editor workflow",
        "For heavy editing tasks, mark a note to open in your external `` $EDITOR `` via the list's {list:ToggleExternalEditor} action.",
    ),
    tip(
        "Outline pane",
        "Press {edit:ToggleOutline} to dock a sidebar listing the note's markdown headers. Use {edit:CycleFocus} to move focus into the pane, select a header, and press Enter to jump the cursor to that line.",
    ),
    tip(
        "Links pane",
        "Press {edit:ToggleLinks} to dock a sidebar listing outgoing and incoming links for the current note. Select a link and press Enter to open that note in the editor.",
    ),
    tip(
        "Linked-note preview",
        "Place the cursor on a [[wikilink]] and press {edit:PreviewLink} to pop up a rendered preview of the target note. Press Esc to close without navigating.",
    ),
    tip(
        "READ and EDIT modes",
        "On opening a note the editor is in **READ** mode showing rendered markdown. Press `e`/`i` to enter **EDIT** and type. `Esc` steps back: EDIT→READ, READ→list. Scroll READ with j/k/PageUp/PageDown/G/gg.",
    ),
];
const GRAPH_SUGGESTIONS: &[HelpSuggestion] = &[
    tip(
        "Auto-fit graph",
        "Press {graph:AutoFit} to recenter and rescale the whole graph to perfectly fit the viewport.",
    ),
    tip(
        "Search nodes",
        "Press {graph:ToggleSearch} to toggle a **search box** for filtering and locating nodes on the graph.",
    ),
    tip(
        "Toggle minimap",
        "Use {graph:ToggleMinimap} to show or hide the small **minimap** in the bottom corner of the viewport.",
    ),
    tip(
        "Toggle legend",
        "Press {graph:ToggleLegend} to display or hide the color-coded **legend** for nodes and links.",
    ),
    tip(
        "Toggle grid background",
        "Toggle the background grid overlay on the canvas using {graph:ToggleGrid}.",
    ),
    tip(
        "Open from graph",
        "Press {graph:OpenNote} to open the currently selected node directly in the note editor.",
    ),
    tip(
        "Zoom controls",
        "Use {graph:ZoomIn} to zoom closer into nodes, or {graph:ZoomOut} to zoom out for a wider view.",
    ),
    tip(
        "Reload configuration",
        "Press {graph:ReloadConfig} to reload graph visualization configurations from the configuration file.",
    ),
    tip(
        "Toggle status bar",
        "Press {graph:ToggleStatus} to show or hide the **status bar** at the bottom of the graph view.",
    ),
    tip(
        "Toggle preview",
        "Use {graph:TogglePreview} to toggle a **preview pane** that displays node content inline.",
    ),
    tip(
        "Refresh physics",
        "Press {graph:Refresh} to reset and re-run the physics simulation to reorganize node layout.",
    ),
    tip(
        "Graph help",
        "Press {graph:Help} to jump to the help page from the graph view, or {graph:Quit} to close the graph.",
    ),
];
const DRAW_SUGGESTIONS: &[HelpSuggestion] = &[
    tip(
        "Shape selector",
        "Press {draw:ToggleShapeSelector} to open the **Shape Selector** menu and drop rectangles, circles, or lines.",
    ),
    tip(
        "Pen tool",
        "Press {draw:SelectDrawTool} to activate the **Pen tool** and draw smooth freehand lines by dragging.",
    ),
    tip(
        "Text labels",
        "Use {draw:SelectTextTool} to select the **Text tool** and type custom text annotations anywhere on the canvas.",
    ),
    tip(
        "Eraser tool",
        "Press {draw:SelectEraseTool} to select the **Eraser tool** and rub out existing lines or shapes.",
    ),
    tip(
        "Toggle grid alignment",
        "Press {draw:ToggleGrid} to toggle the alignment grid to help you position your shapes.",
    ),
    tip(
        "Shape menu up",
        "Use the `` Up `` arrow key inside the shape selector to move the highlighted option up.",
    ),
    tip(
        "Shape menu down",
        "Use the `` Down `` arrow key inside the shape selector to move the highlighted option down.",
    ),
    tip(
        "Shape selection confirm",
        "Press `` Enter `` to confirm and select the highlighted shape type from the menu.",
    ),
    tip(
        "Drawing workflow",
        "Select a **tool** first, then click and drag on the canvas to place shapes, lines, or freehand strokes.",
    ),
    tip(
        "Grid alignment",
        "Enable the alignment **grid** to snap elements into position as you draw or move them.",
    ),
    tip(
        "Undo drawings",
        "Use {edit:Undo} to revert the last drawing action if you make a mistake.",
    ),
    tip(
        "Tool shortcuts",
        "Quickly switch between **drawing**, **text**, and **eraser** tools using their dedicated keybinds without opening menus.",
    ),
];
const CANVAS_SUGGESTIONS: &[HelpSuggestion] = &[
    tip(
        "Context menu",
        "Press {canvas:OpenContextMenu} to toggle a popup **context menu** on the selected node.",
    ),
    tip(
        "Edit or connect",
        "Press {canvas:EditOrConnect} to edit the selected canvas node or start wiring connections.",
    ),
    tip(
        "Toggle editor",
        "Press {canvas:ToggleEditorPane} to toggle a side editor pane to edit note bodies inline.",
    ),
    tip(
        "Canvas grid",
        "Toggle the background alignment grid on the infinite canvas using {canvas:ToggleGrid}.",
    ),
    tip(
        "Zoom controls",
        "Press {canvas:ZoomIn} or {canvas:ZoomOut} to adjust the zoom level of the infinite workspace.",
    ),
    tip(
        "Fine zoom",
        "Press {canvas:ZoomFineIn} or {canvas:ZoomFineOut} to perform very fine, detailed zooming.",
    ),
    tip(
        "Quit canvas",
        "Press {canvas:Quit} to save all changes and exit the infinite canvas mode.",
    ),
    tip(
        "Cycle editor focus",
        "Use {canvas:CycleFocus} to shift focus between the canvas and the sidebar editor pane.",
    ),
    tip(
        "Save canvas",
        "Press {canvas:Save} to save the current canvas state at any time.",
    ),
    tip(
        "Close context menu",
        "Press {canvas:MenuClose} to dismiss the open context menu without selecting an action.",
    ),
    tip(
        "Editor unfocus",
        "Press {canvas:EditorUnfocus} to move focus back to the canvas from the editor pane.",
    ),
    tip(
        "Sync raw editor",
        "Use {canvas:EditorSyncRaw} to save and sync any pending editor changes to the canvas.",
    ),
];
const BACKUP_SUGGESTIONS: &[HelpSuggestion] = &[
    tip(
        "Stage changes",
        "Press {backup:StageFile} to stage the currently highlighted git file modification.",
    ),
    tip(
        "Unstage changes",
        "Press {backup:UnstageFile} to unstage the highlighted file from the next commit.",
    ),
    tip(
        "Stage all files",
        "Press {backup:StageAll} to stage all modified and untracked files in the repository.",
    ),
    tip(
        "Enter commit mode",
        "Press {backup:EnterCommit} to focus the commit message field and prepare to commit staged files.",
    ),
    tip(
        "Confirm commit",
        "Press {backup:ConfirmCommit} to confirm the typed commit message and record the changes.",
    ),
    tip(
        "Cancel commit",
        "Press {backup:CancelCommit} to discard the typed commit message and exit commit mode.",
    ),
    tip(
        "Push commits",
        "Use {backup:Push} to push all your local commits to the configured remote repository.",
    ),
    tip(
        "Pull changes",
        "Use {backup:Pull} to pull the latest changes from your remote repository.",
    ),
    tip(
        "Git status refresh",
        "Press {backup:Refresh} to run a fresh git status and update the modifications pane.",
    ),
    tip(
        "Cycle sections",
        "Press {backup:CycleSection} to quickly jump between **Staged**, **Unstaged**, and **Untracked** sections.",
    ),
    tip(
        "Open settings",
        "Press {backup:OpenSettings} to configure git author name, email, and remote URL from within the app.",
    ),
    tip(
        "Settings fields",
        "Use {backup:NextField} or {backup:PrevField} to navigate between git settings fields in the settings popup.",
    ),
    tip(
        "Close settings",
        "Press {backup:CloseSettings} to dismiss the settings popup and return to the backup view.",
    ),
];
const TEMPLATES_SUGGESTIONS: &[HelpSuggestion] = &[
    tip(
        "Note templates",
        "Press {list:NewFromTemplate} from the main list to open the template picker for generating new notes.",
    ),
    tip(
        "Date variables",
        "Insert `` {date} ``, `` {datetime} ``, or `` {time} `` in your template to auto-insert timestamps.",
    ),
    tip(
        "Date components",
        "Use `` {year} ``, `` {month} ``, `` {day} ``, or `` {weekday} `` to customize note filenames dynamically.",
    ),
    tip(
        "Default template",
        "Place a file named `` default.toml `` in your templates directory to act as the default new-note template.",
    ),
    tip(
        "Template path",
        "Drop any custom template `.toml` files in the directory `` ~/.config/clin/templates/ `` to load them.",
    ),
    tip(
        "Command line template helper",
        "Run the command `` clin templates init `` in your terminal to scaffold example templates.",
    ),
    tip(
        "Picker search",
        "Type search characters inside the template picker list to filter templates by name instantly.",
    ),
    tip(
        "Template fields",
        "Note templates can define frontmatter, custom titles, and template body placeholders.",
    ),
    tip(
        "Frontmatter generation",
        "Templates can include `` +++ `` frontmatter blocks with default values for **title**, **tags**, and custom fields.",
    ),
    tip(
        "Variables list",
        "Available template variables: `` {title} ``, `` {name} ``, `` {folder} ``, `` {id} ``, plus all date components.",
    ),
    tip(
        "Picker layout",
        "The template picker shows the template **name** and **description** — use the description to hint at what the template creates.",
    ),
    tip(
        "Template inheritance",
        "You can nest templates by referencing other template files in the `` template `` field of a `.toml` template.",
    ),
];
const ABOUT_SUGGESTIONS: &[HelpSuggestion] = &[
    tip(
        "Config edit shortcut",
        "Type `` clin config edit `` in your terminal to open config.toml in your default editor.",
    ),
    tip(
        "CLI quick capture",
        "Run `` clin notes quick <text> `` to append a brief thought directly to your default inbox note.",
    ),
    tip(
        "Dump keybinds configuration",
        "Run `` clin keybinds export `` to dump the current active keybinding config as editable TOML.",
    ),
    tip(
        "Run database migrations",
        "Run `` clin storage migrate `` in your terminal if you need to migrate your database or note directory.",
    ),
    tip(
        "Check version",
        "Run `` clin --version `` in your shell to display the current build information and metadata.",
    ),
    tip(
        "Help commands",
        "Use {help:Search} to query command actions on the help page, or {help:Close} to exit help view.",
    ),
    tip(
        "Reroll suggestion tips",
        "Press {help:Reroll} to roll a fresh randomized selection of tip suggestions for the active tab.",
    ),
    tip(
        "Switch tabs",
        "Press {help:NextTab} or {help:PrevTab} to cycle through the help tabs for different application views.",
    ),
    tip(
        "List notes via CLI",
        "Run `` clin notes ls `` to list all notes in your vault from the command line.",
    ),
    tip(
        "Config key overrides",
        "Run `` clin config set key value `` to change a single config option from the terminal.",
    ),
    tip(
        "Graph view CLI",
        "Use `` clin graph `` from your terminal to open the graph visualization directly.",
    ),
    tip(
        "Version upgrade info",
        "Run `` clin upgrade check `` to see if a new version of clin is available.",
    ),
];
const CONTENT_TREE_SUGGESTIONS: &[HelpSuggestion] = &[
    tip(
        "Navigate sections",
        "Press {content_tree:MoveUp} or {content_tree:MoveDown} to move between headers in the tree.",
    ),
    tip(
        "Expand and collapse",
        "Press {content_tree:ToggleCollapse} to expand or collapse the selected section's subtree.",
    ),
    tip(
        "Expand all",
        "Press {content_tree:ExpandAll} to expand every collapsed section in the tree at once.",
    ),
    tip(
        "Collapse all",
        "Press {content_tree:CollapseAll} to collapse every section back to the top-level headers.",
    ),
    tip(
        "Open a heading",
        "Press {content_tree:Open} to jump the editor cursor to the line of the selected heading.",
    ),
    tip(
        "Back to notes",
        "Press {content_tree:Back} to close the content tree and return to the notes list.",
    ),
    tip(
        "Get help",
        "Press {content_tree:Help} to open the help view for keybind references.",
    ),
];
const SETUP_SUGGESTIONS: &[HelpSuggestion] = &[
    tip(
        "Navigate options",
        "Press {setup:Up} or {setup:Down} to move between configuration options in the wizard.",
    ),
    tip(
        "Cycle forward",
        "Press {setup:CycleNext} to cycle to the next value for the selected option.",
    ),
    tip(
        "Cycle backward",
        "Press {setup:CyclePrev} to go back to the previous value for the selected option.",
    ),
    tip(
        "Activate selection",
        "Press {setup:Activate} to confirm and activate the currently highlighted choice.",
    ),
    tip(
        "Finish wizard",
        "Press {setup:Finish} to complete the setup wizard and apply all chosen settings.",
    ),
    tip(
        "Live preview",
        "As you cycle through themes and styles, a **live markdown preview** updates in real time to show your changes.",
    ),
];

pub fn tab_suggestions(tab: HelpTab) -> &'static [HelpSuggestion] {
    match tab {
        HelpTab::Notes => NOTES_SUGGESTIONS,
        HelpTab::Editor => EDITOR_SUGGESTIONS,
        HelpTab::Graph => GRAPH_SUGGESTIONS,
        HelpTab::Draw => DRAW_SUGGESTIONS,
        HelpTab::Canvas => CANVAS_SUGGESTIONS,
        HelpTab::Backup => BACKUP_SUGGESTIONS,
        HelpTab::ContentTree => CONTENT_TREE_SUGGESTIONS,
        HelpTab::Setup => SETUP_SUGGESTIONS,
        HelpTab::Templates => TEMPLATES_SUGGESTIONS,
        HelpTab::About => ABOUT_SUGGESTIONS,
    }
}
/// Pick `count` random suggestions for `tab`. Returns fewer (or empty) if the
/// pool is smaller than `count`. Uses rand 0.8 (already a dependency).
pub fn roll_suggestions(tab: HelpTab, count: usize) -> Vec<&'static HelpSuggestion> {
    use rand::seq::SliceRandom;
    let pool = tab_suggestions(tab);
    let mut rng = rand::thread_rng();
    pool.choose_multiple(&mut rng, count).collect()
}

#[derive(Debug, Clone, Copy)]
pub struct PopupHelp {
    pub name: &'static str,
    pub body: &'static str,
}

/// Per-tab popup/overlay guides rendered in the help info pane.
/// Each tab with notable popups returns its own slice; About has no popups and returns empty.
pub fn tab_popup_descriptions(tab: HelpTab) -> &'static [PopupHelp] {
    match tab {
        HelpTab::Notes => NOTES_POPUPS,
        HelpTab::Editor => EDITOR_POPUPS,
        HelpTab::Graph => GRAPH_POPUPS,
        HelpTab::Draw => DRAW_POPUPS,
        HelpTab::Canvas => CANVAS_POPUPS,
        HelpTab::Backup => BACKUP_POPUPS,
        HelpTab::ContentTree => CONTENT_TREE_POPUPS,
        HelpTab::Setup => SETUP_POPUPS,
        HelpTab::Templates => TEMPLATES_POPUPS,
        HelpTab::About => &[],
    }
}
const CONTENT_TREE_POPUPS: &[PopupHelp] = &[];
const SETUP_POPUPS: &[PopupHelp] = &[];


const NOTES_POPUPS: &[PopupHelp] = &[
    PopupHelp {
        name: "Command Palette",
        body: "Press {list:OpenCommandPalette} to open the **Command Palette** — a fuzzy launcher that runs any action by name without remembering its key. Type to filter the list; ``Tab`` / ``Shift+Tab`` cycle the category tabs (General, Notes, Import, Append, Views, Settings). Move with ``Up`` / ``Down`` (wraps around) and run the highlighted action with ``Enter``; ``Esc`` closes. The palette opens scoped to the selected note, so note-specific actions apply to it.",
    },
    PopupHelp {
        name: "Tags",
        body: "Press {list:ManageTags} to open the **Tag Manager** for the selected note. Add tags as a comma-separated list in the input; ``Tab`` accepts the current suggestion or, once the list is empty, moves focus to the all-tags list (``Shift+Tab`` reverses). In the all-tags list, ``k`` / ``j`` or arrows move and ``d`` / ``Delete`` removes a tag. ``Ctrl+s`` enters **tag mode**: pick multiple notes, then ``Enter`` to apply the tag to all of them. Removing a tag asks for confirmation first.",
    },
    PopupHelp {
        name: "Subnotes",
        body: "Press {list:ManageSubnotes} to open the **Subnotes** manager — encrypted child notes attached to the selected note. In the list pane, ``j`` / ``k`` or arrows move, ``n`` (or ``Alt+n``) adds a subnote, ``d`` / ``Delete`` removes one, and ``Enter`` edits it; ``Tab`` cycles focus list → title → content. ``Ctrl+e`` hands the current subnote to your external editor. Notes that carry subnotes show a ⧉ marker in the notes list. Closing the popup auto-saves whenever anything changed.",
    },
    PopupHelp {
        name: "Search",
        body: "Press {list:Search} for **full-vault search** — it matches note titles and also greps note bodies, grouping content hits per note. Type to search live; ``Tab`` toggles between the input and the results. In the results, ``j`` / ``k`` or arrows move, ``l`` / ``Space`` expands a note's matched lines, and ``Enter`` jumps to the selected note. ``Esc`` cancels and returns to the list.",
    },
    PopupHelp {
        name: "Trash",
        body: "Press {list:OpenTrash} to open the **Trash** view and recover or permanently destroy deleted notes. Move with ``k`` / ``Up`` and ``j`` / ``Down``; ``r`` or ``Enter`` restores the selected note, ``d`` / ``Delete`` deletes it permanently, and ``E`` empties the whole trash. Restore, permanent delete, and empty each ask for confirmation before acting. ``Esc`` closes the view; restoring into an otherwise-empty trash closes it automatically.",
    },
    PopupHelp {
        name: "Outline",
        body: "Open the note's **Outline** to browse its markdown headers as a collapsible tree. Move with {content_tree:MoveUp} / {content_tree:MoveDown}, fold or unfold a section with {content_tree:ToggleCollapse}, and expand or collapse every heading at once with {content_tree:ExpandAll} / {content_tree:CollapseAll}. Press {content_tree:Open} on a heading to jump straight to that line in the editor, {content_tree:Back} to return to the note, or {content_tree:Help} for an in-view key guide.",
    },
];

const EDITOR_POPUPS: &[PopupHelp] = &[
    PopupHelp {
        name: "Subnotes",
        body: "Press {edit:ManageSubnotes} while editing to open the **Subnotes** manager without leaving the editor — attach encrypted child notes to the note you are editing. In the list pane ``j`` / ``k`` or arrows move, ``n`` (or ``Alt+n``) adds a subnote, ``d`` / ``Delete`` removes one, and ``Enter`` edits; ``Tab`` cycles focus list → title → content and ``Ctrl+e`` hands the current subnote to your external editor. Closing the popup auto-saves whenever anything changed; notes carrying subnotes show a ⧉ marker in the list.",
    },
    PopupHelp {
        name: "Context Menu",
        body: "``Right-click`` the title or body to open the **Context Menu** with clipboard actions — Copy, Cut, Paste, Select All — applied to whichever field is focused. Move with ``k`` / ``j`` or arrows, run an item with ``Enter``, and ``Esc`` closes. The markdown preview is an inline toggle pane, not a popup.",
    },
];

const GRAPH_POPUPS: &[PopupHelp] = &[PopupHelp {
    name: "Search",
    body: "Press {graph:ToggleSearch} to open the **Search** overlay and jump to any node by title or tag. Type to filter live (``Backspace`` / ``Delete`` edit, ``Ctrl+u`` clears, arrows move the cursor); ``Up`` / ``Down`` or ``Tab`` / ``Shift+Tab`` move between matches and ``Enter`` centers the canvas on the selected node and closes the overlay. ``Esc`` cancels. The minimap, legend, grid, status bar, and preview are separate inline toggles, each with its own key in the table.",
}];

const DRAW_POPUPS: &[PopupHelp] = &[
    PopupHelp {
        name: "Shape Selector",
        body: "Press {draw:ToggleShapeSelector} to open the **Shape Selector** and choose a shape. ``Up`` / ``Down`` cycle the shape types, ``Enter`` confirms and switches to the Shape tool (then click-drag on the canvas to draw it), and ``Esc`` cancels. The pen, text, and eraser tools each have their own key in the table.",
    },
    PopupHelp {
        name: "Text Editor",
        body: "``Right-click`` an existing text label to open the **Text Editor** and change its content. Type to edit, ``Enter`` saves the change and writes the ``.draw`` file, ``Esc`` discards. To create new text instead, pick the Text tool and left-click an empty spot on the canvas.",
    },
];

const CANVAS_POPUPS: &[PopupHelp] = &[
    PopupHelp {
        name: "Context Menu",
        body: "Press {canvas:OpenContextMenu} to open the **Context Menu** for the selected node — or for the canvas when nothing is selected. ``k`` / ``j`` or arrows move, ``Enter`` runs the item, ``Esc`` closes. Actions include editing, renaming, connecting, recoloring, and deleting the focused node.",
    },
    PopupHelp {
        name: "Node Editor",
        body: "Press {canvas:EditOrConnect} on a selected node to open its **floating editor** and edit the node's text in place — every keystroke autosaves. Close it with ``Esc`` (the CloseEditor key). If a connection is already in progress, this same key completes the edge to the target node instead.",
    },
    PopupHelp {
        name: "Editor Pane & Rename",
        body: "Press {canvas:ToggleEditorPane} to toggle a side **editor pane** showing the selected node's full raw text; ``CycleFocus`` moves the cursor into it. A **rename popup** (type a new node ID, ``Enter`` confirms, ``Esc`` cancels) is reached from the context menu.",
    },
];

const BACKUP_POPUPS: &[PopupHelp] = &[
    PopupHelp {
        name: "Commit Message",
        body: "Press {backup:EnterCommit} to open the **commit-message editor** and describe the staged changes. Type your message, then ``Enter`` (ConfirmCommit) commits — empty messages are rejected. ``Esc`` (CancelCommit) returns to the dashboard without committing. The diff pane beside it scrolls with its own keys.",
    },
    PopupHelp {
        name: "Settings",
        body: "Press {backup:OpenSettings} to open the **backup settings** editor and toggle auto-backup inline. ``Tab`` / ``Shift+Tab`` (NextField / PrevField) move between fields; ``Enter`` (ActivateField) flips a toggle or opens a text field (remote URL / name) for typing. Choose **Save** to persist and close, or ``Esc`` (CloseSettings) to discard.",
    },
];

const TEMPLATES_POPUPS: &[PopupHelp] = &[PopupHelp {
    name: "Template Picker",
    body: "Press {list:NewFromTemplate} to open the **template picker** and start a new note from a template. Type to search (``Tab`` swaps focus between the search box and the results); in the results ``Up`` / ``Down`` move and ``Enter`` generates a new note with date tokens filled. From the results, ``Space`` edits a template, ``d`` deletes one (asks confirmation), ``n`` creates a new template, and ``?`` jumps to template help.",
}];
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_help_tab_contents() {
        let tabs = [
            HelpTab::Notes,
            HelpTab::Editor,
            HelpTab::Graph,
            HelpTab::Draw,
            HelpTab::Canvas,
            HelpTab::Backup,
            HelpTab::Templates,
            HelpTab::About,
        ];

        for &tab in &tabs {
            // 1. Verify description is present and not empty
            let desc = tab_description(tab);
            assert!(
                !desc.is_empty(),
                "Description for {:?} should not be empty",
                tab
            );

            // 2. Verify we have at least 12 suggestions per tab
            let suggestions = tab_suggestions(tab);
            assert!(
                suggestions.len() >= 12,
                "Tab {:?} has only {} suggestions, need at least 12",
                tab,
                suggestions.len()
            );

            // 3. Verify rolling exactly 4 suggestions works
            let rolled = roll_suggestions(tab, 4);
            assert_eq!(
                rolled.len(),
                4,
                "Rolled suggestions for {:?} should be 4",
                tab
            );

            // 4. Verify all 4 suggestions are unique
            let title0 = rolled[0].title;
            let title1 = rolled[1].title;
            let title2 = rolled[2].title;
            let title3 = rolled[3].title;
            assert!(
                title0 != title1
                    && title1 != title2
                    && title0 != title2
                    && title0 != title3
                    && title1 != title3
                    && title2 != title3,
                "Rolled suggestions for {:?} contain duplicates: {:?}",
                tab,
                rolled
            );

            // 5. Verify overflow roll limit does not panic and returns up to size
            let rolled_excess = roll_suggestions(tab, 100);
            assert_eq!(rolled_excess.len(), suggestions.len());
        }
    }

    #[test]
    fn test_tip_key_resolution() {
        let kb = crate::keybinds::Keybinds::default();
        let tabs = [
            HelpTab::Notes,
            HelpTab::Editor,
            HelpTab::Graph,
            HelpTab::Draw,
            HelpTab::Canvas,
            HelpTab::Backup,
            HelpTab::Templates,
            HelpTab::About,
        ];

        for &tab in &tabs {
            let suggestions = tab_suggestions(tab);
            for suggestion in suggestions {
                let mut remaining = suggestion.body;
                while let Some(start) = remaining.find('{') {
                    remaining = &remaining[start + 1..];
                    if let Some(end) = remaining.find('}') {
                        let token = &remaining[..end];
                        // Only validate tokens with a colon (keybind references like {scope:Action}).
                        // Bare {date}, {time} etc. are template variable placeholders.
                        if token.contains(':') {
                            let resolved = crate::ui::help::resolve_tip_key(token, &kb);
                            assert!(
                                !resolved.starts_with("[ERR:"),
                                "Failed to resolve key token '{}' in tip '{}' (body: '{}')",
                                token,
                                suggestion.title,
                                suggestion.body
                            );
                        }
                        remaining = &remaining[end + 1..];
                    } else {
                        panic!(
                            "Unmatched brace in tip '{}' body: '{}'",
                            suggestion.title, suggestion.body
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_tip_requirement_caveats() {
        // None variant always returns None
        assert_eq!(
            TipRequirement::None.caveat_if_unsatisfied(&ClinConfig::default()),
            None
        );

        // Counts with default config (preset=Default) -> Some
        let default_config = ClinConfig::default();
        assert_eq!(
            TipRequirement::Counts.caveat_if_unsatisfied(&default_config),
            Some("needs Vim/Helix preset for the count prefix".to_string())
        );

        // Counts with Vim preset -> None
        let mut vim_config = ClinConfig::default();
        vim_config.core.keybind_preset = KeybindPreset::Vim;
        assert_eq!(
            TipRequirement::Counts.caveat_if_unsatisfied(&vim_config),
            None
        );

        // Preset(Vim) with default config -> Some
        assert_eq!(
            TipRequirement::Preset(KeybindPreset::Vim).caveat_if_unsatisfied(&default_config),
            Some("needs vim preset".to_string())
        );

        // Preset(Vim) with Vim config -> None
        assert_eq!(
            TipRequirement::Preset(KeybindPreset::Vim).caveat_if_unsatisfied(&vim_config),
            None
        );

        // Verify the ExpandToLevel tip carries Counts requirement
        let notes = tab_suggestions(HelpTab::Notes);
        let expand = notes
            .iter()
            .find(|s| s.title == "Expand to depth")
            .expect("Expand to depth tip missing");
        assert_eq!(expand.requires, TipRequirement::Counts);
        assert_eq!(
            expand
                .requires
                .caveat_if_unsatisfied(&ClinConfig::default()),
            Some("needs Vim/Helix preset for the count prefix".to_string())
        );
    }

    #[test]
    fn test_popup_descriptions() {
        // Expected per-tab popup counts — About has no popups.
        fn expected_count(tab: HelpTab) -> usize {
            match tab {
                HelpTab::Notes => 6,
                HelpTab::Editor => 2,
                HelpTab::Graph => 1,
                HelpTab::Draw => 2,
                HelpTab::Canvas => 3,
                HelpTab::Backup => 2,
                HelpTab::ContentTree => 0,
                HelpTab::Setup => 0,
                HelpTab::Templates => 1,
                HelpTab::About => 0,
            }
        }

        let tabs = [
            HelpTab::Notes,
            HelpTab::Editor,
            HelpTab::Graph,
            HelpTab::Draw,
            HelpTab::Canvas,
            HelpTab::Backup,
            HelpTab::ContentTree,
            HelpTab::Setup,
            HelpTab::Templates,
            HelpTab::About,
        ];
        for &tab in &tabs {
            let popups = tab_popup_descriptions(tab);
            let want = expected_count(tab);
            assert_eq!(
                popups.len(),
                want,
                "{:?} tab should have exactly {} popup descriptions, got {}",
                tab,
                want,
                popups.len()
            );
        }

        // Verify all popup names and bodies are non-empty across all tabs
        for &tab in &tabs {
            for popup in tab_popup_descriptions(tab) {
                assert!(
                    !popup.name.is_empty(),
                    "Popup name should not be empty for {:?}",
                    tab
                );
                assert!(
                    !popup.body.is_empty(),
                    "Popup body for '{:?}:{}' should not be empty",
                    tab,
                    popup.name
                );
            }
        }
    }
}
