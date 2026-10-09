# Template System

Technical docs for the note template system — reusable templates with variable substitution for quick note creation.

---

## Overview

Templates allow users to create notes from predefined structures. In native storage, templates live in its `templates/` directory; for a custom vault, they live in `<vault>/.clin/templates/`. Templates can include dynamic variables (`{date}`, `{time}`, etc.) that are substituted at creation time.

**Source:** `src/templates.rs` — `Template`, `TemplateSummary`, `Storage` templates API

- Defines the template schema + load/save/render logic.
- Performs variable substitution.
- Filename sanitization and orchestration (list, load, save, examples) are implemented on `Storage`.

---

## Directory

Native storage:

```text
<native-storage>/templates/
├── default.toml     (auto-loaded on new note if present)
├── meeting.toml
├── todo.toml
├── journal.toml
└── ... (any .toml file)
```

For a custom vault, use `<vault>/.clin/templates/` instead. Create templates there manually or with `clin templates init`.

---

## File Format

Each template is a `.toml` file:

```toml
name = "Meeting Notes"

[title]
template = "Meeting - {date}"

[properties]
status = "active"
priority = 1
history_related = true

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
| `[properties]` | `*` | TOML Value | Key-value pairs of initial typed note properties |
### Rust Types

```rust
pub struct Template {
    pub name: String,
    pub title: TitleConfig,     // template: Option<String>
    pub content: ContentConfig, // template: String
    pub properties: BTreeMap<String, toml::Value>,
}

pub struct RenderedTemplate {
    pub title: Option<String>,
    pub content: String,
    pub properties: Vec<FrontmatterEdit>,
    pub header: Option<String>,
    pub tags: Vec<String>,
}
```

### Template Rendering & Precedence

Templates are rendered through `Template::render`:

```rust
impl Template {
    pub fn render(
        &self,
        definitions: &PropertyDefinitions,
        overrides: &[FrontmatterEdit],
    ) -> Result<RenderedTemplate>;
}
```

**Property Evaluation Precedence:**
1. **Explicit Creation Edits:** Command-line `--property` flags or programmatic creation overrides.
2. **Template Properties & Frontmatter:** Values in the template `[properties]` table or embedded YAML frontmatter.
3. **Schema Defaults:** Property defaults configured in `<vault>/.clin/properties.toml`.

> **Note:** Property defaults are applied exclusively during note creation or via the explicit `properties.defaults` command palette action. Opening an existing note never automatically populates default values.

Built-in keys (`title`, `tags`) cannot be defined under `[properties]`. Legacy YAML frontmatter blocks inside `[content].template` are preserved.

---

## Template Variables

Available variables for `{variable_name}` substitution:

| Variable | Example Value | Description |
|---|---|---|
| `{date}` | `2026-05-10` | Current date (YYYY-MM-DD) |
| `{datetime}` | `2026-05-10 14:30` | Date and time |
| `{iso_datetime}` | `2026-05-10T14:30:00+00:00` | ISO 8601 / RFC 3339 timestamp |
| `{weekday}` | `Saturday` | Full weekday name |
| `{year}` | `2026` | 4-digit year |
| `{month}` | `05` | Zero-padded month |
| `{day}` | `10` | Zero-padded day of month |


### Property Placeholders

Body and title templates can interpolate property values using `{prop:KEY}`:

```toml
[title]
template = "[{prop:status}] {date} - Standup"

[content]
template = """
# Project Notes

Priority: {prop:priority}
Created: {iso_datetime}
"""
```

- `{prop:KEY}` fetches the resolved property value.
- Missing, unknown, complex (nested mapping/matrix), or unsupported property placeholders produce a render error. Property formulas and expressions are not supported.
- Date variables inside template property strings (e.g. `due = "{date}"`) are substituted before schema validation.
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
| `clin templates list` | List all available templates |
| `clin templates init` | Create meeting, todo, and journal example templates |
| `clin notes new -t <name> [title]` | Create a new note from a specific template |

---

## Usage

### From TUI

```
1. Press `a` on folder → creates note from default template (if any)
   OR press `t` → template picker popup
2. Use popup search bar to filter templates (Tab switches Search/Results focus)
3. Select template with up/down in Results and press Enter
4. Press `?` inside template popup to open Templates help tab
```

### From CLI

```bash
# Create a note from "meeting" template
clin notes new -t meeting "Weekly Standup"

# Create with default template
clin notes new "My Note"
```

---

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
