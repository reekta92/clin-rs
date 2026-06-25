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
        Box::new(OpenGraphAction),
        Box::new(content_tree::OpenContentTreeAction),
        Box::new(OpenBackupAction),
        Box::new(CreateDrawAction),
        Box::new(CreateCanvasAction),
        Box::new(DebugDumpAction),
        Box::new(ocr::OcrPasteAction),
        Box::new(SwitchThemeAction),
        Box::new(SwitchKeybindPresetAction),
        Box::new(ToggleExternalEditorAction),
        Box::new(ToggleLayoutAction),
        Box::new(settings::ToggleLayoutEditModeAction),
        Box::new(settings::TogglePreviewPaneAction),
        Box::new(settings::TogglePreviewWrapAction),
        Box::new(settings::ToggleCalendarAction),
        Box::new(settings::ToggleLineNumbersAction),
        Box::new(settings::ToggleConfirmDeleteAction),
        Box::new(settings::TogglePinnedOnTopAction),
        Box::new(settings::ToggleConfirmQuitAction),
        Box::new(settings::TogglePreviewEncryptionAction),
        Box::new(settings::CycleSortAction),
        Box::new(settings::ToggleShowHiddenFilesAction),
        Box::new(settings::ToggleTabIconsOnlyAction),
        Box::new(settings::SetWordGoalAction),
        Box::new(settings::SetNoteGoalAction),
        Box::new(settings::CycleIconModeAction),
        Box::new(settings::CycleHintBarStyleAction),
        Box::new(import::ImportAction { source: ImportSource::File, target: ImportTarget::NewNote }),
        Box::new(import::ImportAction { source: ImportSource::File, target: ImportTarget::AppendCurrent }),
        Box::new(import::ImportAction { source: ImportSource::Csv, target: ImportTarget::NewNote }),
        Box::new(import::ImportAction { source: ImportSource::Csv, target: ImportTarget::AppendCurrent }),
        Box::new(import::ImportAction { source: ImportSource::Json, target: ImportTarget::NewNote }),
        Box::new(import::ImportAction { source: ImportSource::Json, target: ImportTarget::AppendCurrent }),
        Box::new(import::ImportAction { source: ImportSource::Url, target: ImportTarget::NewNote }),
        Box::new(import::ImportAction { source: ImportSource::Url, target: ImportTarget::AppendCurrent }),
        Box::new(import::ImportAction { source: ImportSource::Clipboard, target: ImportTarget::NewNote }),
        Box::new(import::ImportAction { source: ImportSource::Clipboard, target: ImportTarget::AppendCurrent }),
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

Actions are grouped by category. See the `ACTIONS` registry in `src/actions/mod.rs` for the complete list (currently ~40 actions).

| Category | Example Actions |
|---|---|
| **Notes** | Encrypt, Decrypt, Content Tree |
| **Views** | Graph, Draw, Canvas, Backup |
| **Settings** | Theme, Keybind Preset, Layout Toggle, External Editor Toggle, Preview Toggle, Sort Cycle, Calendar Toggle, Word/Note Goal, Icon Mode, Hint Bar Style |
| **Import** | File/CSV/JSON/URL/Clipboard → New Note |
| **Append** | File/CSV/JSON/URL/Clipboard → Append to Current, OCR Paste |
| **General** | Debug Dump |

**Note:** Import and URL actions require `markitdown` (pip install markitdown) or `pandoc` installed. URL import also requires `curl`. CSV and JSON conversions are pure-Rust and always available.

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
