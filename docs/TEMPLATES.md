# Template System

Technical docs for the note template system — reusable templates with variable substitution for quick note creation.

---

## Overview

Templates allow users to create notes from predefined structures. Templates live in `<vault>/.clin/templates/` for both default and custom storage. Templates can include dynamic variables (`{date}`, `{time}`, etc.) that are substituted at creation time.

**Source:** `src/templates.rs` — `Template`, `TemplateSummary`, `Storage` templates API

- Defines the template schema + load/save/render logic.
- Performs variable substitution.
- Filename sanitization and orchestration (list, load, save, examples) are implemented on `Storage`.

---

## Directory

All vaults:

```text
<vault>/.clin/templates/
├── default.toml     (auto-loaded on new note if present)
├── meeting.toml
├── todo.toml
├── journal.toml
└── ... (any .toml file)
```

Create templates there manually or with `clin templates init`. The `[features] templates` flag controls the picker and `clin templates` commands; those commands report an error when disabled.

---

## File Format

Each template is a `.toml` file:

```toml
name = "Meeting Notes"

[title]
template = "Meeting - {date}"

[content]
template = """
# Meeting Notes

**Date:** {date}
**Time:** {time}

## Attendees

-

## Agenda

1.

## Discussion

## Action Items

- [ ]

## Next Meeting
"""
```

### Schema

| Section | Field | Type | Description |
|---|---|---|---|
| root | `name` | String | Human-readable template name (shown in popup) |
| `[title]` | `template` | String (optional) | Title template with variables; if absent, prompts for title |
| `[content]` | `template` | String | Body content template with variables |

### Rust Types

```rust
pub struct Template {
    pub name: String,
    pub title: TitleConfig,     // template: Option<String>
    pub content: ContentConfig, // template: String
}

pub struct RenderedTemplate {
    pub title: Option<String>,
    pub content: String,
}
```

---

## Template Variables

Available variables for `{variable_name}` substitution:

| Variable | Example Value | Description |
|---|---|---|
| `{date}` | `2026-05-10` | Current date (YYYY-MM-DD) |
| `{datetime}` | `2026-05-10 14:30` | Date and time |
| `{time}` | `14:30` | Current time (HH:MM) |
| `{weekday}` | `Saturday` | Full weekday name |
| `{year}` | `2026` | 4-digit year |
| `{month}` | `05` | Zero-padded month |
| `{day}` | `10` | Zero-padded day of month |

The renderer substitutes `{name}` tokens once in the template title and body. Unknown variables stay literal and produce a warning listing missing names. Inserted values are not expanded again, so a variable containing `{date}` retains that text.

### Folder-local custom variables

Create `.clin-template.toml` in any physical vault folder. Values must be strings. Names are case-sensitive ASCII identifiers (`[A-Za-z_][A-Za-z0-9_]*`); the seven built-in names above are reserved.

For example, put this config in `books/rust/`:

```toml
inherit_parent = true

[variables]
book = "The Rust Programming Language"
```

Then put this config in `books/rust/chapter-04/`:

```toml
inherit_parent = true

[variables]
chapter = "04"
chapter_title = "Understanding Ownership"
```

One shared template can use `{book}`, `{chapter}`, `{chapter_title}`, and `{date}` in both `[title].template` and `[content].template`. Custom values do not affect tags, frontmatter fields, or destination paths.

#### Inheritance

- `inherit_parent` defaults to `true` when omitted.
- Values flow from the vault root toward the destination folder. Nearer values override ancestor values with the same name.
- Folders without a config continue inheritance normally, including destinations not created yet.
- `inherit_parent = false` excludes all configs above that folder. Its own values still flow into inheriting descendants.
- Resolution stops at the vault root, never at the user's home or filesystem root. Symlinks escaping the vault are rejected; the config file itself cannot be a symlink.
- Malformed TOML, invalid names, non-string values, and attempts to override built-ins stop template-based creation before a note is written. Errors identify the config path.
- Changes apply to future creation only; existing notes are not re-expanded.

#### Edit configuration in the TUI

Select a physical folder (or a note inside it), open the command palette, and choose **Edit Folder Template Variables**. In grid layout, this edits the current folder. The action creates a starter config only when absent, then opens it in the existing template editor. Smart/virtual folders are not configuration destinations.

The built-in editor validates before saving; an invalid edit leaves the previous file intact and stays in the editor. With external-editor mode enabled, your editor writes directly to disk; Clin validates on return and reports errors. An invalid external edit must be corrected before template-based creation can continue.

Folders with their own `.clin-template.toml` display a gear glyph (`⚙` in Unicode mode) on the folder item: beside its name in tree layout, and in the tile's corner in grid layout, alongside any pin badge. This indicates a local config file, not merely inherited variables. The badge respects the icon-mode setting and does not appear on virtual folders or the `..` navigation tile.

Inside that folder, `.clin-template.toml` appears as a muted selectable item in both tree and grid layouts, even when hidden/all-files display is off. Select it and press Enter (or double-click) to edit using the same validated editor; its preview displays the TOML. The palette action remains available to create a config when none exists.

Config files are not indexed or counted as notes and cannot be pinned/tagged through note actions. They remain visible during selection mode without participating in bulk note selection. External config creation/removal updates the badge and item without moving the selected note.

No header indicator, Smart Folder template automation, prompts, expressions, or executable plugins are included.

```rust
impl TemplateVariables {
    pub fn now() -> Self {
        let now = Local::now();
        Self {
            date: now.format("%Y-%m-%d").to_string(),
            datetime: now.format("%Y-%m-%d %H:%M").to_string(),
            time: now.format("%H:%M").to_string(),
            weekday: now.format("%A").to_string(),
            year: now.format("%Y").to_string(),
            month: now.format("%m").to_string(),
            day: now.format("%d").to_string(),
        }
    }
}
```

---

## Default Template

If a template file named `default.toml` exists in the templates directory, the TUI note-creation flow uses it when templates are enabled. The default create-note key is `n`; the default template-picker key is `t`.

---

## CLI Commands

| Command | Description |
|---|---|
| `clin templates list` | List all available templates |
| `clin templates init` | Create meeting, todo, and journal example templates |
| `clin notes new -t <display-name> [--folder <relative-path>] [title]` | Render a template using destination-folder variables, then open the TUI unless `--no-tui` or `--body` is supplied |

---

## Usage

### From TUI

```
1. Press `n` on folder → note creation uses default template (if any)
   OR press `t` → template picker popup
2. Use popup search bar to filter templates (Tab switches Search/Results focus)
3. Select template with up/down in Results and press Enter
4. Press `?` inside template popup to open Templates help tab
```

### From CLI

```bash
# Initialize example templates and inspect their display names
clin templates init
clin templates list

# CLI matches the exact display name, not the filename stem
clin notes new -t "Meeting Notes" --no-tui "Weekly Standup"

# Create a blank note; CLI does not load default.toml automatically
clin notes new --no-tui "My Note"
```

---

### CLI rendering and precedence

The CLI matches the template's `name` exactly (for example `"Meeting Notes"`, not `meeting`). It renders built-ins and folder-local values using the same renderer as the TUI:

```bash
clin notes new -t "Book Chapter" --folder books/rust/chapter-04 --no-tui
```

`--folder` is relative to the active vault and defaults to its root. Absolute paths, `..`, backslashes, and symlinks escaping the vault are rejected. Missing destination folders are created when the note is saved.

An explicit title overrides the rendered template title. An explicit `--body` overrides template content and implies no TUI. Both explicit arguments stay literal (they are not interpolated), and unused template fields do not generate missing-variable warnings. Without an explicit or template title, the fallback is `"New Note"`.

Unknown variables remain literal and warnings go to stderr. Configuration/template errors exit unsuccessfully without creating a partial note. CLI creation without `-t` remains blank and does not automatically load `default.toml`.

## Storage Templates API

```rust
impl Storage {
    pub fn list_templates(&self) -> Result<Vec<TemplateSummary>>;
    pub fn load_template(&self, filename: &str) -> Result<Template>;
    pub fn save_template(&self, filename: &str, template: &Template) -> Result<()>;
    pub fn delete_template(&self, filename: &str) -> Result<()>;
    pub fn load_default_template(&self) -> Option<Template>;
    pub fn has_templates(&self) -> bool;
    pub fn create_example_templates(&self) -> Result<()>;
}
```

`TemplateSummary` provides filenames and human-readable names for searchable picker UI:

```rust
pub struct TemplateSummary {
    pub filename: String,  // e.g. "meeting.toml"
    pub name: String,      // e.g. "Meeting Notes"
}
```

---

## Connections

- [ARCHITECTURE.md](ARCHITECTURE.md) — how templates integrate with App note creation flow
- [COMMAND_PALETTE.md](COMMAND_PALETTE.md) — template picker interaction
