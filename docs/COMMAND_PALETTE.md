 # Command Palette

Technical docs for the command palette and Action trait — an extensible action system accessible via Ctrl+P or Shift+Enter.

---

## Overview

The command palette provides a searchable list of actions. Users can invoke any registered action without navigating menus. The palette is modeless — it opens over view and closes after executing or canceling.
**Source:** `src/actions/mod.rs` (Action trait + registry), `src/palette.rs` (popup widget)

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
    fn glyph(&self) -> &'static str {
        ""
    }
    fn execute(&self, app: &mut App, context_note_id: Option<&str>) -> Result<()>;
}
```

| Method | Description |
|---|---|
| `id()` | Unique identifier string (e.g. `"note.encrypt"`) |
| `name()` | Human-readable name for the palette |
| `description()` | Short help text shown in palette |
| `category()` | Grouping (Notes, Import, Append, Views, Settings) |
| `execute()` | Perform the action, mutating `App` state |

---

## Registration

Actions are registered in a static lazy vector in `src/actions/mod.rs`:

```rust
pub static ACTIONS: Lazy<Vec<Box<dyn Action>>> = Lazy::new(|| {
    vec![
        Box::new(encrypt::EncryptNoteAction),
        Box::new(decrypt::DecryptNoteAction),
        Box::new(graph::OpenGraphAction),
        Box::new(content_tree::OpenContentTreeAction),
        Box::new(draw::CreateDrawAction),
        Box::new(pinstar::CreateCanvasAction),
        Box::new(theme::SwitchThemeAction),
    ]
});
```

Action metadata is cached separately:

```rust
pub static ACTION_INFOS: Lazy<Vec<ActionInfo>> = Lazy::new(|| {
    ACTIONS.iter().map(|a| ActionInfo {
        id: a.id().into_owned(),
        name: a.name().into_owned(),
        description: a.description().into_owned(),
        category: a.category(),
        glyph: a.glyph().to_string(),
    }).collect()
});
```

---

## Available Actions

| ID | Name | Description | Category | Glyph |
|---|---|---|---|---|
| `note.encrypt` | Encrypt Note | Encrypt the selected note (.md → .clin) | Notes | `\u{f023}` |
| `note.decrypt` | Decrypt Note | Decrypt the selected note (.clin → .md) | Notes | `\u{f3c1}` |
| `content_tree.open` | Content Tree | Headers and content tree | Notes | `\u{f1bb}` |
| `graph.open` | Open Graph | Switch to graph view | Views | `\u{f0e8}` |
| `draw.create` | New Draw | Create a new drawing | Views | `\u{f1fc}` |
| `canvas.create` | New Canvas | Create a canvas map | Views | `\u{f005}` |
| `backup.open` | Open Backup | View backup dashboard | Views | `\u{f1d3}` |
| `ocr.paste` | OCR Paste | OCR clipboard image | Append | `\u{f03e}` |
| `switch_theme` | Switch Theme | Cycle themes | Settings | `\u{f042}` |
| `toggle_notes_layout` | Toggle Layout | Tree/Grid layout | Settings | `\u{f0c9}` |
| `external_editor.toggle`| Toggle Editor | Use $EDITOR | Settings | `\u{f120}` |
| `settings.preview_pane` | Toggle Preview Pane | Show or hide the preview pane in the notes list | Settings | `\u{f0db}` |
| `settings.preview_wrap` | Toggle Preview Word Wrap | Wrap long preview lines to the pane width | Settings | `\u{f036}` |
| `settings.calendar` | Toggle Calendar | Show or hide the month calendar in the notes list | Settings | `\u{f073}` |
| `settings.line_numbers` | Toggle Line Numbers | Show or hide line numbers in the note editor | Settings | `\u{f03a}` |
| `settings.confirm_delete` | Toggle Delete Confirmation | Ask for confirmation before moving notes to trash | Settings | `\u{f3ed}` |
| `settings.pinned_on_top` | Toggle Pinned on Top | Keep pinned notes above others in the list | Settings | `\u{f08d}` |
| `settings.confirm_quit` | Toggle Quit Confirmation | Ask for confirmation before quitting clin | Settings | `\u{f08b}` |
| `settings.preview_encryption` | Toggle Encrypted Note Preview | Show or hide previews of encrypted (.clin) notes | Settings | `\u{f06e}` |
| `settings.cycle_sort` | Cycle Sort Order | Cycle the notes sort field and order | Settings | `\u{f0dc}` |
| `insert.file_new` | Import File | Convert file as note | Import | `\u{f15b}` |
| `insert.file_append` | Append File | Convert file to note | Append | `\u{f15b}` |
| `insert.csv_new` | Import CSV | Convert CSV as note | Import | `\u{f0ce}` |
| `insert.csv_append` | Append CSV | Convert CSV to note | Append | `\u{f0ce}` |
| `insert.json_new` | Import JSON | Convert JSON as note | Import | `\u{f121}` |
| `insert.json_append` | Append JSON | Convert JSON to note | Append | `\u{f121}` |
| `insert.url_new` | Import URL | Convert URL as note | Import | `\u{f0ac}` |
| `insert.url_append` | Append URL | Convert URL to note | Append | `\u{f0ac}` |
| `insert.clipboard_new` | Import Clipboard | Clipboard as note | Import | `\u{f0ea}` |
| `insert.clipboard_append`| Append Clipboard | Clipboard to note | Append | `\u{f0ea}` |
**Note:** `insert.file_*` and `insert.url_*` require `markitdown` (pip install markitdown) or `pandoc` installed. `insert.url_*` also requires `curl`. CSV and JSON conversions are pure-Rust and always available.
---

## Execution

```rust
pub fn execute_action(
    action_id: &str,
    app: &mut App,
    context_note_id: Option<&str>,
) -> Result<()> {
    for action in get_all_actions() {
        if action.id() == action_id {
            return action.execute(app, context_note_id);
        }
}
```

The context note ID is the currently selected note (from `App::list.visual_index`), passed so actions know which note to operate on.

---

## Popup UI

The command palette is rendered by `CommandPalette` widget in `src/palette.rs`:

```
┌─────────────────────────────────────────────┐
│  > search_query                             │
├─────────────────────────────────────────────┤
│ All · Notes · Import · Append · Views · Set │
├─────────────────────────────────────────────┤
│   Encrypt Note               Encrypt..      │
│   Decrypt Note               Decrypt..      │
│   Content Tree               Headers..      │
│   Open Graph View            Switch..       │
│   Create Drawing             Create..       │
│   Create Canvas Map          Create..       │
└─────────────────────────────────────────────┘
```
### Search Behavior

- Real-time filtering by action `name` and `description`
- Case-insensitive substring match
- Results update on every keystroke
- If no results, shows "No matching actions" message

### Keyboard Navigation

| Key | Action |
|---|---|
| Type | Filter results |
| `Tab` | Next category |
| `Shift+Tab` | Previous category |
| `Up` / `Down` | Navigate list |
| `Enter` | Execute selected action |
| `Esc` | Close palette (cancel) |

---

## View Lifecycle

```
User presses Ctrl+P or Shift+Enter
  └─ app.command_palette = Some(CommandPalette::new())
  └─ draw_ui() renders palette overlay
  └─ handle_list_keys() / handle_edit_keys() checks for active palette
      ├─ If palette active → route keys to palette navigation
      │   ├─ Up/Down → change selection
      │   ├─ Type chars → filter
      │   └─ Enter → execute_action()
      └─ If not active → normal view navigation
```

The palette is modeless-modal: it's rendered as a centered popup over the current view, but key events are intercepted before reaching the view handler.

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
       fn glyph(&self) -> &'static str {
           "\u{f059}" // question-circle
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
4. Build and test — the palette will auto-discover the new action

---

## Connections

- [ARCHITECTURE.md](ARCHITECTURE.md) — event loop, App state
- [ENCRYPTION.md](ENCRYPTION.md) — `EncryptNoteAction`, `DecryptNoteAction`
- [THEME_SYSTEM.md](THEME_SYSTEM.md) — `SwitchThemeAction`
- [CANVAS.md](CANVAS.md) — `CreateCanvasAction`
- [DRAW.md](DRAW.md) — `CreateDrawAction`
- [GRAPH_VIEW.md](GRAPH_VIEW.md) — `OpenGraphAction`
