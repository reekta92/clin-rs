# Command Palette

Technical docs for the command palette and Action trait — an extensible action system accessible via Ctrl+P or Shift+Enter.

---

## Overview

The command palette provides a searchable list of actions. Users can invoke any registered action without navigating menus. The palette is modeless — it opens over any view and closes after executing or canceling.

**Source:** `src/actions/mod.rs` (Action trait + registry), `src/palette.rs` (popup widget)

---

## Action Trait

```rust
pub trait Action: Send + Sync {
    fn id(&self) -> Cow<'static, str>;
    fn name(&self) -> Cow<'static, str>;
    fn description(&self) -> Cow<'static, str>;
    fn execute(&self, app: &mut App, context_note_id: Option<&str>) -> Result<()>;
}
```

| Method | Description |
|---|---|
| `id()` | Unique identifier string (e.g. `"note.encrypt"`) |
| `name()` | Human-readable name for the palette |
| `description()` | Short help text shown in palette |
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
    }).collect()
});
```

---

## Available Actions

| ID | Name | Description |
|---|---|---|
| `note.encrypt` | Encrypt Note | Encrypt the selected note (.md → .clin) |
| `note.decrypt` | Decrypt Note | Decrypt the selected note (.clin → .md) |
| `graph.open` | Open Graph | Switch to the force-directed graph view |
| `content_tree.open` | Content Tree | Show the selected note's headers and content as a navigable tree |
| `draw.create` | New Draw | Create a new drawing file and open it |
| `canvas.create` | New Canvas | Create a new Obsidian-compatible canvas |
| `ocr.paste` | OCR Paste | OCR clipboard image into the current note |
| `insert.file_new` | Insert File as Note | Convert a file (PDF, DOCX, HTML…) to markdown and create a note |
| `insert.file_append` | Append File to Note | Convert a file (PDF, DOCX, HTML…) to markdown and append to current note |
| `insert.csv_new` | Insert CSV as Note | Convert a CSV/TSV file to a markdown table and create a note |
| `insert.csv_append` | Append CSV to Note | Convert a CSV/TSV file to a markdown table and append to current note |
| `insert.json_new` | Insert JSON as Note | Convert a JSON file to markdown (table or code block) and create a note |
| `insert.json_append` | Append JSON to Note | Convert a JSON file to markdown (table or code block) and append to current note |
| `insert.url_new` | Insert URL as Note | Fetch a URL, convert to markdown, and create a note |
| `insert.url_append` | Append URL to Note | Fetch a URL, convert to markdown, and append to current note |
| `insert.clipboard_new` | Insert Clipboard as Note | Create a new note from clipboard text |
| `insert.clipboard_append` | Append Clipboard to Note | Append clipboard text to the current note |

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
    anyhow::bail!("Action not found: {}", action_id)
}
```

The context note ID is the currently selected note (from `App::list.visual_index`), passed so actions know which note to operate on.

---

## Popup UI

The command palette is rendered by `CommandPalette` widget in `src/palette.rs`:

```
┌─────────────────────────────────────┐
│  > search_query                     │
├─────────────────────────────────────┤
│  Encrypt Note            Encrypt.. │
│  Decrypt Note            Decrypt.. │
│  Open Graph              Switch..  │
│  New Draw                Create..  │
│  New Canvas              Create..  │
│  OCR Paste               OCR cl..  │
│  Switch Theme            Open th.. │
└─────────────────────────────────────┘
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
   use super::Action;
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
       fn execute(&self, app: &mut App, context_note_id: Option<&str>) -> Result<()> {
           // your logic here
           Ok(())
       }
   }
   ```
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
