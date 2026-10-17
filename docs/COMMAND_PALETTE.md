# Command Palette

The same command palette opens with unmodified **F6** in List, Edit, Help, Graph, Draw, Canvas, Bases, Outline, and Setup. Only key presses open it. Existing custom bindings containing F6 anywhere in the focused view's sequence take priority; free that binding to use the fallback. The F2 keybind overlay advertises F6 when available.

Notes keeps its preset-specific palette shortcuts and right-click targeting. Bases keeps its configured palette shortcut. In Edit, `Ctrl+p` still toggles Markdown preview; Emacs navigation bindings are unchanged. A focused live Bases region expands into standalone Bases before opening its palette, preserving the host's return route. Dialogs and active gestures retain input ownership.

---

## Overview

The command palette provides a searchable list of actions. Users can invoke applicable registered actions without navigating menus. The palette opens over the current view and owns input. Actions that need a choice or value continue inside its frame; immediate actions close it after execution.
**Source:** `src/actions/mod.rs` (registry and dispatch), `src/actions/context.rs` (eligibility), `src/app/command_palette.rs` (opening and activation), `src/palette.rs` (search and tabs).

---

## Action Trait

```rust
pub trait Action: Send + Sync {
    fn id(&self) -> Cow<'static, str>;
    fn name(&self) -> Cow<'static, str>;
    fn description(&self) -> Cow<'static, str>;
    fn category(&self) -> ActionCategory {
        ActionCategory::General
    }
    fn glyph(&self) -> (&'static str, &'static str) {
        ("", "")
    }
    fn execute(&self, app: &mut App, context_note_id: Option<&str>) -> Result<()>;

    fn name_dynamic(&self, _app: &App) -> String {
        self.name().to_string()
    }
    fn description_dynamic(&self, _app: &App) -> String {
        self.description().to_string()
    }
}
```

| Method | Description |
|---|---|
| `name()` | Human-readable name for the palette |
| `description()` | Short help text shown in palette |
| `category()` | Grouping (General, Notes, Import, Append, Views, Settings) |
| `glyph()` | `(&'static str, &'static str)` — Nerd Font + Unicode pair, selected by `IconMode` |
| `execute()` | Perform the action, mutating `App` state |

---

## Registration

Actions are registered in the static `ACTIONS` lazy vector in `src/actions/mod.rs`. The visible action list depends on features, active view, live owner, supported file format, and captured focus; it is not a fixed action count. Add an action there after implementing `Action` and classify its ID explicitly in `actions/context.rs`; the palette consumes the registry through `get_all_actions()` and `get_all_action_infos()`.

Unknown IDs are denied. A registration test checks unique IDs and complete classification. Each refresh builds eligible metadata once, then derives category tabs and fuzzy results from that snapshot. Presentation categories never grant execution permissions.

| View | Commands |
| --- | --- |
| Notes | Applicable selected-file operations, creation/import, list settings, feature management, shared note settings |
| Edit | Editor settings and applicable body insertion; title/sidebar focus excludes body insertion |
| Graph | Live legend, minimap, and orphan settings |
| Bases | Guarded public operations and public Bases creation in the active file's parent folder |
| Canvas | Image insertion through Canvas history, raw-editor synchronization, and fallible save |
| Draw, Outline, Help | Shared settings and valid navigation |
| Setup | Pending theme/preset selectors, Finish Setup, and Cancel Setup only |

Established views also offer shared appearance, keybind, and behavior settings, plus available navigation. Redundant current-view destinations are hidden. Raw Bases/Canvas/drawing/template source buffers do not receive Markdown-only operations.


## Available Actions

Actions are grouped by category:

| Category | Shipped actions |
|---|---|
| **General** | Show info, paste image, insert image from file (shown under All) |
| **Import** | File, CSV, JSON, URL, and clipboard imports to a new note |
| **Append** | File, CSV, JSON, URL, and clipboard imports appended to current note; OCR paste |
| **Views** | Notes, Help, Graph, Setup, Setup Finish/Cancel, document creation, public Bases operations, vault switcher |
| **Settings** | Setup wizard, theme, keybind preset, editor/list/preview controls, goals, icon and hint-bar styles, smart folders, and graph visual controls |

File-format conversion uses `markitdown` when available, otherwise `pandoc`; URL import requires `curl`. CSV and JSON conversions are handled in Rust. Tabs start with **All**, then available Notes/Import/Append/Views/Settings categories; General actions appear under All, not a separate General tab.

## Execution

`get_all_action_infos(app)` and `execute_action(id, app, note_id)` retain their existing signatures. Both derive current context and delegate to the same eligibility policy as the palette. A supplied note ID outside Notes cannot grant access to the Notes selection. A caller without editor focus cannot enable body insertion.

The palette captures origin, vault, applicable owner path, Notes file/folder identity, and actual editor focus. Targeted execution revalidates the live owner and file; changed/deleted targets, renamed editors, replaced origins, and vault changes are rejected. Shared settings remain usable after a document-only invalidation.

Keyboard and mouse activation share one operation: select the action ID, validate, execute once, and report failure. Integrated steps retain the captured action context and revalidate before submission. Info/Outline/subnotes commands reject changed Notes selections instead of retargeting through a visual index.

---

## Popup UI

The command palette is rendered by `CommandPalette` widget in `src/palette.rs`:

```
┌─────────────────────────────────────────────┐
│  > search_query                             │
├─────────────────────────────────────────────┤
│ All · Notes · Import · Append · Views · Set │
├─────────────────────────────────────────────┤
│   Outline               Headers..      │
│   Open Graph View            Switch..       │
│   Create Drawing             Create..       │
│   Create Canvas Map          Create..       │
└─────────────────────────────────────────────┘
```
### Search Behavior

- Fuzzy matching against dynamic action **names**, not descriptions
- Results ranked by `SkimMatcherV2` score
- Results update on every keystroke and relevant context/config refresh
- Pasted CR/LF sequences become spaces; paste refreshes before Enter can activate
- Copy/cut/select-all operate on the query without changing the background editor or quitting
- Resize preserves the query; blank result-space clicks retain the palette; outside clicks dismiss and are consumed
- If no results, shows "No matching actions" message

### Keyboard Navigation

| Key | Action |
|---|---|
| Type | Filter results |
| `Tab` | Next category |
| `Shift+Tab` | Previous category |
| `Up` / `Down` | Navigate list |
| `Enter` | Execute selected action |
| `Esc` | Return one step; close at the root |

---

## Integrated action steps

When launched from the palette, these workflows keep the same frame:

| Workflow | Body / input |
| --- | --- |
| Theme | Filterable built-in/custom themes, current-theme marker, general/Graph background controls |
| Navigation preset, icon mode, hint-bar style, sort order | Filterable existing options |
| Word/note goals | Current numeric value in the input field |
| Drawing, Canvas, Bases creation | Name input; collisions retain input and never overwrite a file |
| URL import/append | URL input, retaining the original append target |
| Vault switcher | Filterable vaults; F2 renames the selected vault, Delete requests the existing removal confirmation |
| Bases configuration | All 34 existing configuration commands and their follow-up inputs |
| Bases property edit, public note creation, export, group move | Existing typed value/path inputs with guarded asynchronous writes |

The header field filters a choice list or edits the requested value, depending on the step. Category tabs appear only at the command root. Esc returns to the parent, restoring its query, selection and scroll. Outside clicks close through the workflow's save/cleanup path. Failed validation or writes retain the input and display the error. Pending writes block departure and duplicate submission.

Settings choices include visual examples: themes show four palette colors, icon modes show note/folder/settings icons, hint styles render a single real hint, and navigation presets describe their keys. The workflow bar uses the root category background and shows the applied value where useful; steps without controls or a current value omit it. Theme background controls are clickable bar items: **Alt+B** and **Alt+G** toggle general and Graph backgrounds; their shortcuts appear in the popup footer. Action-category tabs use the accent color for inactive and selected categories. On means solid; Off means transparent. Setup omits the Graph control. Validation errors and pending-operation status appear separately, and shortcut instructions stay in the footer.

Theme changes apply immediately and the selector stays open; leaving it saves outside Setup. Hint-style and navigation-preset selectors also stay open. In Setup, theme/preset changes remain pending until Finish. Bases value inputs use Shift+Enter for a newline and Enter to submit. The body Apply row also submits inputs with the mouse.

Direct shortcut launches retain their original popup presentation, including F4 vault switching. The Subnotes action opens its original manager popup with its list, title and content panes. Confirmations, search dialogs (including Bases search), system file/directory pickers, read-only Info/image previews, and destination views/editors remain separate. Removing a Bases view still requires its separate typed DELETE confirmation; clearing a property still requires CLEAR. No other standalone Notes/view dialogs are migrated.

---

## View Lifecycle

All opening paths use `App::open_command_palette`. Overlay aliases return `OverlayResult::OpenCommandPalette` so the live owner is restored before command construction. Both real event dispatchers route open-palette input before background copy/force-quit handling. Background autosave and Bases completion polling continue.

`notes.open` and `graph.open` validate the destination, guard active operations, save owners, and prepare the destination before cleanup. A failed save or preparation retains source buffers, recovery drafts, workers, and return metadata. A successful save may remain on disk after failed destination preparation. Explicit Notes navigation ends at List; Graph destinations return to Notes on normal exit.

Help and Setup temporarily retain their caller and its return route. Opening Setup from Help first resolves Help to its caller. Expanded Bases restores its host through the existing lifecycle. Vault commitment and Setup vault rebootstrap use the same save/write guard. A writing Bases task must complete before departure.

Theme/preset/UI changes update retained owners, including those suspended under Help/Setup. Setup selectors update pending wizard state and live preview without persisting; Finish saves through the existing validation/confirmation workflow, and Cancel requests the existing discard confirmation.

---

## Adding a New Action

1. Create a new file in `src/actions/` (e.g., `src/actions/my_action.rs`)
2. Implement the `Action` trait:
   ```rust
   use super::{Action, ActionCategory};
   use crate::app::App;
   use anyhow::Result;
   use std::borrow::Cow;

   pub struct MyAction;

   impl Action for MyAction {
       fn id(&self) -> Cow<'static, str> {
           Cow::Borrowed("my.action")
       }
       fn name(&self) -> Cow<'static, str> {
           Cow::Borrowed("My Action")
       }
       fn description(&self) -> Cow<'static, str> {
           Cow::Borrowed("Description of what it does")
       }
       fn category(&self) -> ActionCategory {
           ActionCategory::General
       }
        fn glyph(&self) -> (&'static str, &'static str) {
            ("\u{f059}", "\u{2753}") // question-circle
        }
       fn execute(&self, app: &mut App, context_note_id: Option<&str>) -> Result<()> {
           // your logic here
           Ok(())
       }
   }
   ```

**Note:** Glyphs use Nerd Font icons. The terminal must use a Nerd Font for these to render correctly.
3. Register in `src/actions/mod.rs`:
   - Add `pub mod my_action;`
   - Add `Box::new(my_action::MyAction),` to the `ACTIONS` vec
4. Classify the ID in `src/actions/context.rs` and test its permitted and forbidden contexts.
5. Build and test; unclassified commands remain unavailable.

---

## Connections

- [ARCHITECTURE.md](ARCHITECTURE.md) — event loop, App state
- [THEME_SYSTEM.md](THEME_SYSTEM.md) — `SwitchThemeAction`
- [CANVAS.md](CANVAS.md) — `CreateCanvasAction`
- [DRAW.md](DRAW.md) — `CreateDrawAction`
- [GRAPH_VIEW.md](GRAPH_VIEW.md) — `NavigationAction`

## Bases scope

Standalone and expanded live Bases views use the same palette widget. Their
context adds native Bases actions for property editing/clear/copy/paste, guarded
history, saved views, configuration, source/image opening, grouping, creation and
export. Encrypted-note catalog actions and context are excluded. The default
Bases binding is `Ctrl+p`; configurable sequences and preset counts remain owned
by the shared keybinding resolver. Closing the full view restores its live region
and original editor/overlay return route. Shared settings/navigation do not inherit a private Notes target.

Notes has up to six root categories: All, Notes, Import, Append, Views, and Settings. Empty categories are hidden. Notes and Append need a captured Markdown/text-note target; selecting a folder or another file type can leave All, Import, Views, and Settings. Import and Append also require the Import feature. Eligibility is refreshed after actions and before execution so a stale target cannot be edited.

## Verification

The palette suites cover real main/editor dispatch, every view and preset, custom F6 bindings, captured targets, retained/suspended owners, integrated step navigation, live settings and Setup persistence. Workflow tests cover rendered previews, background controls by mouse/shortcut, category eligibility across settings changes, the standalone subnotes handoff and multiline saving, separate confirmations, stale Bases snapshots, blocked writes and a concurrent Bases filename collision.

Run `cargo test --locked palette_`, `cargo test --locked actions::`, and the affected `bases::` and editor suites. Release checks include `cargo fmt --all --check`, locked all-target checking, Clippy with `-D warnings`, the full test suite with temporary XDG directories, dependency auditing, and Rust 1.90.0 checking. Image HTTP tests need local sockets; opt-in performance checks remain separate.

## Frontmatter in the editor

**Toggle Frontmatter** (`editor.frontmatter`) shares the F7 visibility action for ordinary Markdown/text notes and unsaved new notes. It is unavailable in specialized source/template editors. The region starts hidden and uses `editor.frontmatter_mode`. Captured frontmatter focus excludes body insertion/image commands. Navigation saves pending header input as well as Title/Body and retains the owner and recovery draft on validation or publication failure.
