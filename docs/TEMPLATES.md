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

If a template file named `default.toml` exists in the templates directory, the TUI note-creation flow uses it when templates are enabled. The default create-note key is `n`; the default template-picker key is `t`.

---

## CLI Commands

| Command | Description |
|---|---|
| `clin templates list` | List all available templates |
| `clin templates init` | Create meeting, todo, and journal example templates |
| `clin notes new -t <display-name> [title]` | Insert a template's literal body, then open the TUI unless `--no-tui` or `--body` is supplied |

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

### Current CLI Limitations

The CLI matches the template's `name` exactly (for example `"Meeting Notes"`, not `meeting`). It copies `[content].template` literally: variables remain `{date}`, `{time}`, etc., and `[title].template` is not used. An omitted title becomes `"New Note"`; `--body` overrides template content. The TUI picker renders both title and body with variable substitution.

Use the TUI picker when you need rendered template variables. These are current implementation limits, not intended guarantees for future versions.

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
