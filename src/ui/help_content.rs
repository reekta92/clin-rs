use crate::app::HelpTab;

#[derive(Debug, Clone, Copy)]
pub struct HelpSuggestion {
    pub title: &'static str,
    pub body: &'static str,
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
        HelpTab::Templates => {
            "Create reusable note templates with frontmatter and date variables. Pick a template from the notes view, search by name, and auto-fill date tokens when generating a new note. Drop .toml files in the templates directory to add your own."
        }
        HelpTab::ContentTree => {
            "Navigate the active note as an outline of its markdown headers. Collapse and expand sections, jump straight to a heading, and expand or collapse all to get a bird's-eye outline of long documents."
        }
        HelpTab::About => {
            "Application metadata: version, configuration file paths, and the full command-line interface reference. Use this tab to find config locations and CLI subcommands for notes, storage, keybinds, templates, and config management."
        }
    }
}

/// Curated suggestion pool per tab (min 5 entries; most have 6–7 for variety).
pub fn tab_suggestions(tab: HelpTab) -> &'static [HelpSuggestion] {
    match tab {
        HelpTab::Notes => &[
            HelpSuggestion {
                title: "Command palette",
                body: "Press {list:OpenCommandPalette} to open the **Command Palette** and execute any action by typing its name.",
            },
            HelpSuggestion {
                title: "Select mode",
                body: "Press {list:ToggleSelectMode} to toggle **Select Mode** and start selecting multiple notes in the list.",
            },
            HelpSuggestion {
                title: "Select item",
                body: "Use {list:ToggleSelectItem} in **Select Mode** to select or deselect the currently focused note or folder.",
            },
            HelpSuggestion {
                title: "Expand to depth",
                body: "Press {list:ExpandToLevel} followed by a number to expand all folders in the hierarchy to that specific depth.",
            },
            HelpSuggestion {
                title: "Preview pane",
                body: "Toggle the right-side preview pane using {list:TogglePreview} to view notes without opening them.",
            },
            HelpSuggestion {
                title: "Sort order",
                body: "Cycle the note sorting order using {list:CycleSort} to sort by title, modified time, or size.",
            },
            HelpSuggestion {
                title: "Folders first",
                body: "Press {list:ToggleFoldersFirst} to choose whether **folders** should always stay pinned to the top of the list.",
            },
            HelpSuggestion {
                title: "Manage subnotes",
                body: "Press {list:ManageSubnotes} to view, attach, or manage **subnotes** associated with the selected note.",
            },
            HelpSuggestion {
                title: "Create new folder",
                body: "Use {list:CreateFolder} to create a new **folder** in the currently active path.",
            },
            HelpSuggestion {
                title: "Create new note",
                body: "Press {list:CreateNote} to create a new **note** inside the highlighted folder.",
            },
        ],
        HelpTab::Editor => &[
            HelpSuggestion {
                title: "Markdown preview",
                body: "Press {edit:ToggleMarkdownPreview} to toggle a side-by-side **live preview** of your rendered markdown note.",
            },
            HelpSuggestion {
                title: "External editor",
                body: "Press {list:ToggleExternalEditor} from the list to launch your external `` $EDITOR `` (like Vim or Nano) for heavy editing.",
            },
            HelpSuggestion {
                title: "Delete word",
                body: "Use {edit:DeleteWord} to quickly delete the **word** directly preceding your cursor.",
            },
            HelpSuggestion {
                title: "Delete next word",
                body: "Press {edit:DeleteNextWord} to erase the **word** directly following your cursor.",
            },
            HelpSuggestion {
                title: "Undo history",
                body: "Press {edit:Undo} to revert your last text modification; full edit history is kept for your session.",
            },
            HelpSuggestion {
                title: "Redo changes",
                body: "Use {edit:Redo} to re-apply any text modifications that you previously reverted.",
            },
            HelpSuggestion {
                title: "Cycle focus",
                body: "Use {edit:CycleFocus} to quickly tab between editing the note's **title** and the markdown **body**.",
            },
            HelpSuggestion {
                title: "Go back",
                body: "Press {edit:Back} to save your changes and return to the main notes list.",
            },
        ],
        HelpTab::Graph => &[
            HelpSuggestion {
                title: "Auto-fit graph",
                body: "Press {graph:AutoFit} to recenter and rescale the whole graph to perfectly fit the viewport.",
            },
            HelpSuggestion {
                title: "Search nodes",
                body: "Press {graph:ToggleSearch} to toggle a **search box** for filtering and locating nodes on the graph.",
            },
            HelpSuggestion {
                title: "Toggle minimap",
                body: "Use {graph:ToggleMinimap} to show or hide the small **minimap** in the bottom corner of the viewport.",
            },
            HelpSuggestion {
                title: "Toggle legend",
                body: "Press {graph:ToggleLegend} to display or hide the color-coded **legend** for nodes and links.",
            },
            HelpSuggestion {
                title: "Toggle grid background",
                body: "Toggle the background grid overlay on the canvas using {graph:ToggleGrid}.",
            },
            HelpSuggestion {
                title: "Open from graph",
                body: "Press {graph:OpenNote} to open the currently selected node directly in the note editor.",
            },
            HelpSuggestion {
                title: "Zoom controls",
                body: "Use {graph:ZoomIn} to zoom closer into nodes, or {graph:ZoomOut} to zoom out for a wider view.",
            },
            HelpSuggestion {
                title: "Reload configuration",
                body: "Press {graph:ReloadConfig} to reload graph visualization configurations from the configuration file.",
            },
        ],
        HelpTab::Draw => &[
            HelpSuggestion {
                title: "Shape selector",
                body: "Press {draw:ToggleShapeSelector} to open the **Shape Selector** menu and drop rectangles, circles, or lines.",
            },
            HelpSuggestion {
                title: "Pen tool",
                body: "Press {draw:SelectDrawTool} to activate the **Pen tool** and draw smooth freehand lines by dragging.",
            },
            HelpSuggestion {
                title: "Text labels",
                body: "Use {draw:SelectTextTool} to select the **Text tool** and type custom text annotations anywhere on the canvas.",
            },
            HelpSuggestion {
                title: "Eraser tool",
                body: "Press {draw:SelectEraseTool} to select the **Eraser tool** and rub out existing lines or shapes.",
            },
            HelpSuggestion {
                title: "Toggle grid alignment",
                body: "Press {draw:ToggleGrid} to toggle the alignment grid to help you position your shapes.",
            },
            HelpSuggestion {
                title: "Shape menu up",
                body: "Use the `` Up `` arrow key inside the shape selector to move the highlighted option up.",
            },
            HelpSuggestion {
                title: "Shape menu down",
                body: "Use the `` Down `` arrow key inside the shape selector to move the highlighted option down.",
            },
            HelpSuggestion {
                title: "Shape selection confirm",
                body: "Press `` Enter `` to confirm and select the highlighted shape type from the menu.",
            },
        ],
        HelpTab::Canvas => &[
            HelpSuggestion {
                title: "Context menu",
                body: "Press {canvas:OpenContextMenu} to toggle a popup **context menu** on the selected node.",
            },
            HelpSuggestion {
                title: "Edit or connect",
                body: "Press {canvas:EditOrConnect} to edit the selected canvas node or start wiring connections.",
            },
            HelpSuggestion {
                title: "Toggle editor",
                body: "Press {canvas:ToggleEditorPane} to toggle a side editor pane to edit note bodies inline.",
            },
            HelpSuggestion {
                title: "Canvas grid",
                body: "Toggle the background alignment grid on the infinite canvas using {canvas:ToggleGrid}.",
            },
            HelpSuggestion {
                title: "Zoom controls",
                body: "Press {canvas:ZoomIn} or {canvas:ZoomOut} to adjust the zoom level of the infinite workspace.",
            },
            HelpSuggestion {
                title: "Fine zoom",
                body: "Press {canvas:ZoomFineIn} or {canvas:ZoomFineOut} to perform very fine, detailed zooming.",
            },
            HelpSuggestion {
                title: "Quit canvas",
                body: "Press {canvas:Quit} to save all changes and exit the infinite canvas mode.",
            },
            HelpSuggestion {
                title: "Cycle editor focus",
                body: "Use {canvas:CycleFocus} to shift focus between the canvas and the sidebar editor pane.",
            },
        ],
        HelpTab::Backup => &[
            HelpSuggestion {
                title: "Stage changes",
                body: "Press {backup:StageFile} to stage the currently highlighted git file modification.",
            },
            HelpSuggestion {
                title: "Unstage changes",
                body: "Press {backup:UnstageFile} to unstage the highlighted file from the next commit.",
            },
            HelpSuggestion {
                title: "Stage all files",
                body: "Press {backup:StageAll} to stage all modified and untracked files in the repository.",
            },
            HelpSuggestion {
                title: "Enter commit mode",
                body: "Press {backup:EnterCommit} to focus the commit message field and prepare to commit staged files.",
            },
            HelpSuggestion {
                title: "Confirm commit",
                body: "Press {backup:ConfirmCommit} to confirm the typed commit message and record the changes.",
            },
            HelpSuggestion {
                title: "Cancel commit",
                body: "Press {backup:CancelCommit} to discard the typed commit message and exit commit mode.",
            },
            HelpSuggestion {
                title: "Push commits",
                body: "Use {backup:Push} to push all your local commits to the configured remote repository.",
            },
            HelpSuggestion {
                title: "Pull changes",
                body: "Use {backup:Pull} to pull the latest changes from your remote repository.",
            },
            HelpSuggestion {
                title: "Git status refresh",
                body: "Press {backup:Refresh} to run a fresh git status and update the modifications pane.",
            },
        ],
        HelpTab::Templates => &[
            HelpSuggestion {
                title: "Note templates",
                body: "Press {list:NewFromTemplate} from the main list to open the template picker for generating new notes.",
            },
            HelpSuggestion {
                title: "Date variables",
                body: "Insert `` {date} ``, `` {datetime} ``, or `` {time} `` in your template to auto-insert timestamps.",
            },
            HelpSuggestion {
                title: "Date components",
                body: "Use `` {year} ``, `` {month} ``, `` {day} ``, or `` {weekday} `` to customize note filenames dynamically.",
            },
            HelpSuggestion {
                title: "Default template",
                body: "Place a file named `` default.toml `` in your templates directory to act as the default new-note template.",
            },
            HelpSuggestion {
                title: "Template path",
                body: "Drop any custom template `.toml` files in the directory `` ~/.config/clin/templates/ `` to load them.",
            },
            HelpSuggestion {
                title: "Command line template helper",
                body: "Run the command `` clin templates init `` in your terminal to scaffold example templates.",
            },
            HelpSuggestion {
                title: "Picker search",
                body: "Type search characters inside the template picker list to filter templates by name instantly.",
            },
            HelpSuggestion {
                title: "Template fields",
                body: "Note templates can define frontmatter, custom titles, and template body placeholders.",
            },
        ],
        HelpTab::ContentTree => &[
            HelpSuggestion {
                title: "Outline navigation",
                body: "Use `` Up `` and `` Down `` arrow keys to move focus between different markdown header nodes.",
            },
            HelpSuggestion {
                title: "Jump to section",
                body: "Press {content_tree:Open} on a header node to jump straight to that line in the editor.",
            },
            HelpSuggestion {
                title: "Toggle collapse",
                body: "Use {content_tree:ToggleCollapse} to expand or collapse the outline sub-tree of the selected heading.",
            },
            HelpSuggestion {
                title: "Expand all outline",
                body: "Press {content_tree:ExpandAll} to recursively expand all headings in the outline tree.",
            },
            HelpSuggestion {
                title: "Collapse all outline",
                body: "Press {content_tree:CollapseAll} to collapse every heading in the tree for a top-level view.",
            },
            HelpSuggestion {
                title: "Go back to editor",
                body: "Press {content_tree:Back} to exit the outline navigation view and return to the editor.",
            },
            HelpSuggestion {
                title: "Content tree help",
                body: "Press {content_tree:Help} to display the keyboard commands helper within the outline pane.",
            },
            HelpSuggestion {
                title: "Realtime sync",
                body: "The heading outline tree updates in real-time as you write and edit the active note's body.",
            },
        ],
        HelpTab::About => &[
            HelpSuggestion {
                title: "Config edit shortcut",
                body: "Type `` clin config edit `` in your terminal to open config.toml in your default editor.",
            },
            HelpSuggestion {
                title: "CLI quick capture",
                body: "Run `` clin notes quick <text> `` to append a brief thought directly to your default inbox note.",
            },
            HelpSuggestion {
                title: "Dump keybinds configuration",
                body: "Run `` clin keybinds export `` to dump the current active keybinding config as editable TOML.",
            },
            HelpSuggestion {
                title: "Run database migrations",
                body: "Run `` clin storage migrate `` in your terminal if you need to migrate your database or note directory.",
            },
            HelpSuggestion {
                title: "Check version",
                body: "Run `` clin --version `` in your shell to display the current build information and metadata.",
            },
            HelpSuggestion {
                title: "Help commands",
                body: "Use {help:Search} to query command actions on the help page, or {help:Close} to exit help view.",
            },
            HelpSuggestion {
                title: "Reroll suggestion tips",
                body: "Press {help:Reroll} to roll a fresh randomized selection of tip suggestions for the active tab.",
            },
            HelpSuggestion {
                title: "Switch tabs",
                body: "Press {help:NextTab} or {help:PrevTab} to cycle through the help tabs for different application views.",
            },
        ],
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
/// Only `Notes` returns entries; all other tabs return empty.
pub fn tab_popup_descriptions(tab: HelpTab) -> &'static [PopupHelp] {
    match tab {
        HelpTab::Notes => NOTES_POPUPS,
        _ => &[],
    }
}

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
];
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
            HelpTab::ContentTree,
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

            // 2. Verify we have at least 8 suggestions per tab
            let suggestions = tab_suggestions(tab);
            assert!(
                suggestions.len() >= 8,
                "Tab {:?} has only {} suggestions, need at least 8",
                tab,
                suggestions.len()
            );

            // 3. Verify rolling exactly 3 suggestions works
            let rolled = roll_suggestions(tab, 3);
            assert_eq!(
                rolled.len(),
                3,
                "Rolled suggestions for {:?} should be 3",
                tab
            );

            // 4. Verify all 3 suggestions are unique
            let title0 = rolled[0].title;
            let title1 = rolled[1].title;
            let title2 = rolled[2].title;
            assert!(
                title0 != title1 && title1 != title2 && title0 != title2,
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
            HelpTab::ContentTree,
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
    fn test_popup_descriptions() {
        let tabs = [
            HelpTab::Notes,
            HelpTab::Editor,
            HelpTab::Graph,
            HelpTab::Draw,
            HelpTab::Canvas,
            HelpTab::Backup,
            HelpTab::Templates,
            HelpTab::ContentTree,
            HelpTab::About,
        ];

        for &tab in &tabs {
            let popups = tab_popup_descriptions(tab);
            if tab == HelpTab::Notes {
                assert_eq!(
                    popups.len(),
                    5,
                    "Notes tab should have exactly 5 popup descriptions, got {}",
                    popups.len()
                );
            } else {
                assert!(
                    popups.is_empty(),
                    "{:?} tab should have no popup descriptions, got {}",
                    tab,
                    popups.len()
                );
            }
        }

        // Verify all popup names and bodies are non-empty
        for popup in tab_popup_descriptions(HelpTab::Notes) {
            assert!(!popup.name.is_empty(), "Popup name should not be empty");
            assert!(!popup.body.is_empty(), "Popup body for '{}' should not be empty", popup.name);
        }
    }
}
