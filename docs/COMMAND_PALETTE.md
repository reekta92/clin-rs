# Command Palette

Technical docs for the command palette and Action trait — an extensible action system accessible from List with `:` or `Ctrl+p` in the default preset. In Editor, `Ctrl+p` toggles Markdown preview instead.

---

## Overview

The command palette provides a searchable list of actions. Users can invoke any registered action without navigating menus. The palette opens over the current view, intercepts input, and closes after executing or canceling.
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
| `id()` | Unique identifier string (e.g. `"note.encrypt"`) |
| `name()` | Human-readable name for the palette |
| `description()` | Short help text shown in palette |
| `category()` | Grouping (General, Notes, Import, Append, Views, Settings) |
| `glyph()` | `(&'static str, &'static str)` — Nerd Font + Unicode pair, selected by `IconMode` |
| `execute()` | Perform the action, mutating `App` state |

---

## Registration

Actions are registered in the static `ACTIONS` lazy vector in `src/actions/mod.rs`. The visible action list depends on feature toggles; it is not a fixed action count. Add an action there after implementing `Action`; the palette consumes the registry through `get_all_actions()` and `get_all_action_infos()`.

Action metadata is built from the registry, filtered by enabled features, with dynamic names/descriptions:

```rust
pub fn get_all_action_infos(app: &App) -> Vec<ActionInfo> {
    let icon_mode = app.config.ui.icon_mode;
    ACTIONS
        .iter()
        .filter(|a| action_feature_enabled(&a.id(), app))
        .map(|a| {
            let (nerd, unicode) = a.glyph();
            ActionInfo {
                id: a.id().to_string(),
                name: a.name_dynamic(app),
                description: a.description_dynamic(app),
                category: a.category(),
                glyph: crate::ui::get_icon(nerd, unicode, icon_mode).to_string(),
            }
        })
        .collect()
}
```


## Available Actions

Actions are grouped by category:

| Category | Shipped actions |
|---|---|
| **General** | Show info, paste image, insert image from file (shown under All) |
| **Notes** | Encrypt, decrypt, manage sub-notes, outline, insert date, rasterize |
| **Import** | File, CSV, JSON, URL, and clipboard imports to a new note |
| **Append** | File, CSV, JSON, URL, and clipboard imports appended to current note; OCR paste |
| **Views** | Graph, draw, canvas, backup, vault switcher |
| **Settings** | Setup wizard, theme, keybind preset, editor/list/preview controls, goals, icon and hint-bar styles, smart folders, and graph visual controls |

File-format conversion uses `markitdown` when available, otherwise `pandoc`; URL import requires `curl`. CSV and JSON conversions are handled in Rust. Tabs start with **All**, then available Notes/Import/Append/Views/Settings categories; General actions appear under All, not a separate General tab.

## Execution

```rust
pub fn execute_action(
    action_id: &str,
    app: &mut App,
    context_note_id: Option<&str>,
) -> Result<()> {
    if !action_feature_enabled(action_id, app) {
        anyhow::bail!("Action disabled: {action_id}");
    }
    for action in get_all_actions() {
        if action.id() == action_id {
            return action.execute(app, context_note_id);
        }
    }
    anyhow::bail!("Action not found: {action_id}")
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
│   Outline               Headers..      │
│   Open Graph View            Switch..       │
│   Create Drawing             Create..       │
│   Create Canvas Map          Create..       │
└─────────────────────────────────────────────┘
```
### Search Behavior

- Fuzzy matching against dynamic action **names**, not descriptions
- Results ranked by `SkimMatcherV2` score
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
User presses List's OpenCommandPalette binding (: or Ctrl+p by default)
  └─ app.command_palette = Some(CommandPalette::new(context_note_id, app))
  └─ draw_ui() renders palette overlay
  └─ global popup dispatch checks for active palette
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
4. Build and test — the palette will auto-discover the new action

---

## Connections

- [ARCHITECTURE.md](ARCHITECTURE.md) — event loop, App state
- [ENCRYPTION.md](ENCRYPTION.md) — `EncryptNoteAction`, `DecryptNoteAction`
- [THEME_SYSTEM.md](THEME_SYSTEM.md) — `SwitchThemeAction`
- [CANVAS.md](CANVAS.md) — `CreateCanvasAction`
- [DRAW.md](DRAW.md) — `CreateDrawAction`
- [GRAPH_VIEW.md](GRAPH_VIEW.md) — `OpenGraphAction`
