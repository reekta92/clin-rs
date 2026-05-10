# Template System

Technical docs for the note template system — reusable templates with variable substitution for quick note creation.

---

## Overview

Templates allow users to create notes from predefined structures. They are TOML files stored in `~/.config/clin/templates/`. Templates can include dynamic variables (`{date}`, `{time}`, etc.) that are substituted at creation time.

**Source:** `src/templates.rs` — `Template`, `TemplateVariables`, `TemplateManager`

---

## Directory

```
~/.config/clin/templates/
├── default.toml     (auto-loaded on new note if present)
├── meeting.toml
├── todo.toml
├── journal.toml
└── ... (any .toml file)
```

Users can create templates at this path manually or via `clin --create-example-templates`.

---

## File Format

Each template is a `.toml` file:

```toml
[template]
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
| `[template]` | `name` | String | Human-readable template name (shown in palette) |
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

Variables are substituted by `TemplateVariables::substitute()` which scans for `{name}` patterns and replaces them. Unknown variables are left as-is.

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

If a template file named `default.toml` exists in the templates directory, it is automatically used when creating a new note via `a` (without opening the template picker).

---

## CLI Commands

| Command | Description |
|---|---|
| `clin --list-templates` | List all available templates |
| `clin --create-example-templates` | Create meeting, todo, and journal example templates |
| `clin -n -t <name> [title]` | Create a new note from a specific template |

---

## Usage

### From TUI

```
1. Press `a` on a folder → creates note from default template (if any)
   OR press `t` → template picker popup
2. Select template with up/down
3. Press Enter → note created with substituted content
```

### From CLI

```bash
# Create a note from "meeting" template
clin -n -t meeting "Weekly Standup"

# Create with default template
clin -n "My Note"
```

---

## TemplateManager API

```rust
pub struct TemplateManager {
    templates_dir: PathBuf,
}

impl TemplateManager {
    pub fn list(&self) -> Result<Vec<TemplateSummary>>;
    pub fn load(&self, filename: &str) -> Result<Template>;
    pub fn save(&self, filename: &str, template: &Template) -> Result<()>;
    pub fn load_default(&self) -> Option<Template>;
    pub fn has_templates(&self) -> bool;
    pub fn create_examples(&self) -> Result<()>;
}
```

`TemplateSummary` provides filenames and human-readable names for the picker UI:

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
