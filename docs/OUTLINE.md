# Outline View

Technical docs for the Outline sub-view — a full-screen collapsible markdown outline parser and navigation tree.

---

## Overview

The Outline view provides a nested outline of a selected note's body, helping you navigate large notes. Opening the view displays note headers and non-header items (like paragraphs, list items, and code blocks) structured as a hierarchical tree. Selecting any node in the tree and hitting `Enter` immediately opens that section in the editor.

**Source:** `src/outline/` — modules: `app`, `input`, `parse`, `render`, `state`

---

## Nested Outline Parsing

The outline parser (`src/outline/parse.rs`) reads the Markdown content line-by-line in a single pass to produce a flat list of `TreeNode`s, each annotated with a `depth` and a source `line` number.

### Node Kinds

- **Header**: Matches ATX headers (`#` through `######`). The header level determines hierarchy. Note titles are treated as the root (depth 0).
- **ListItem**: Matches list item bullets (`-`, `*`, `+`) and ordered list numbers (`1.`, `2)`, etc.).
- **Paragraph**: Blocks of prose. The parser keeps a preview of the first line (up to 60 characters with trailing ellipsis `…`).
- **CodeBlock**: Collapses entire fenced code blocks (delimited by ```` ``` ```` or `~~~`) into a single node with the block's language specifier.

### Hierarchy & Depth Rules

- Note root is always at `depth = 0`.
- Headers are placed at `depth = header_level`. They do not undergo level normalization.
- Non-header elements (paragraphs, lists, code blocks) under a header have `depth = parent_header_depth + 1`.
- Any non-header elements appearing before the first header are attached directly under the note root (`depth = 1`).

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
| `toggle_collapse` | `Tab`, `Left`, `Right` | Toggle expand/collapse state of the selected header |
| `expand_all` | `e` | Expand all headers |
| `collapse_all` | `c` | Collapse all headers (keeps note title expanded) |
| `open` | `Enter` | Jump to the selected section's source line in the Editor |
| `back` | `Esc` | Return to the previous view mode |
| `help` | `?`, `F1` | Open the help screen at the Outline tab |

---

## Related Documentation

- [ARCHITECTURE.md](ARCHITECTURE.md) — View Mode state machine integration
- [CONFIG_REFERENCE.md](CONFIG_REFERENCE.md) — Key bindings configuration
