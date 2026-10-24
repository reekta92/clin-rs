# Outline View

Technical docs for the Outline sub-view — a full-screen collapsible markdown outline parser and navigation tree.

---

## Overview

The Outline view provides a nested outline of a selected note's body, helping you navigate large notes. Opening the view displays note headers and non-header items (like paragraphs, list items, and code blocks) structured as a hierarchical tree. Selecting any node in the tree and hitting `Enter` immediately opens that section in the editor.

**Source:** `src/outline/` — modules: `app`, `input`, `parse`, `render`, `state`

---

## Nested Outline Parsing

The outline parser (`src/outline/parse.rs`) uses the same Comrak syntax options as the Markdown preview to produce a flat list of `TreeNode`s, each annotated with a `depth` and a source `line` number. Source positions come from Comrak, including Setext headings and nested lists.

### Node Kinds

- **Header**: Matches ATX headers (`#` through `######`) and Setext headings (underlined with `=` or `-`). The header level determines hierarchy. Note titles are treated as the root (depth 0).
- **ListItem**: Matches list item bullets (`-`, `*`, `+`) and ordered list numbers (`1.`, `2)`, etc.).
- **Paragraph**: Blocks of prose, tables and raw HTML blocks. The parser keeps a preview of the first line (up to 60 characters with trailing ellipsis `…`). Tables and HTML retain their source text.
- **CodeBlock**: Collapses fenced or indented code into one node. Fence characters and lengths follow CommonMark; unfinished blocks remain navigable.

### Hierarchy & Depth Rules

- Note root is always at `depth = 0`.
- Header depth follows the active heading hierarchy; skipped heading levels do not add empty tree levels.
- Non-header elements (paragraphs, lists, code blocks) under a header have `depth = parent_header_depth + 1`.
- Any non-header elements appearing before the first header are attached directly under the note root (`depth = 1`).
- Nested list items add their list nesting depth beneath the current header.
- Headings inside list items or block quotes stay within that container; following outside blocks return to the enclosing section.

---

## Interactive Tree State

The view state (`src/outline/state.rs`) keeps track of:
- All parsed tree nodes.
- The currently selected node.
- A set of expanded headers. Collapsing a header hides all descendant nodes (any subsequent nodes whose `depth` is greater than the collapsed header's `depth`).

---

## Key Bindings

The following default actions are supported and configured in the active preset’s keybind file under the `[outline]` section:

| Action | Default Keys | Description |
|---|---|---|
| `move_up` | `k`, `Up` | Move selection to previous visible node |
| `move_down` | `j`, `Down` | Move selection to next visible node |
| `toggle_collapse` | `Tab`, `Left`, `Right`, `h`, `l` | Toggle expand/collapse state of the selected header |
| `expand_all` | `e` | Expand all headers |
| `collapse_all` | `c` | Collapse all headers (keeps note title expanded) |
| `open` | `Enter`, `o` | Jump to the selected section's source line in the Editor |
| `back` | `Esc`, `q` | Return to the previous view mode |
| `help` | `?`, `F1` | Open contextual help (Outline currently maps to Notes; no separate Outline tab exists) |

---

## Related Documentation

- [ARCHITECTURE.md](ARCHITECTURE.md) — View Mode state machine integration
- [CONFIG_REFERENCE.md](CONFIG_REFERENCE.md) — Key bindings configuration
