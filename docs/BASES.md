# Bases

Terminal-native `.base` documents, targeting Obsidian 1.14.3 file and core query
semantics. Implementation is still in progress; this is not a claim of complete
Obsidian runtime parity.

## Open and navigate

Open a `.base` file from the notes list, or use `clin notes open TITLE`.
`[features] bases_view` enables the view (default enabled). The first saved view
opens by default. Configurable `bases` keybindings control navigation and editing;
press the Bases Help binding (`?` by default) for the current map.

Default bindings include:

| Binding | Action |
| --- | --- |
| `h/j/k/l`, arrows | Navigate columns/rows; Kanban horizontal navigation crosses groups |
| `Tab` / `Shift+Tab` | Next/previous saved view |
| `Enter` | Open public Markdown/Bases source or a supported native image preview |
| `e` / `F2`, `Delete` | Edit/clear a note property |
| `Space`, `Ctrl+a` | Toggle selection/select visible results |
| `m` | Set grouping property on selected notes; blank clears the property |
| `n` | Create a Markdown note; Kanban uses the focused column |
| `u` / `Ctrl+r` | Guarded session undo/redo of property and configuration edits |
| `:` / `Ctrl+p` | Bases configuration menu |
| `r`, `x`, `Esc` / `q` | Refresh, export, leave Bases |

Prompts accept Enter to apply, Shift+Enter for a newline, and Esc to cancel.

## Kanban columns and cards

A Kanban view requires `groupBy`. Missing/empty property values use the None
column. Configure `groupOrder` to retain empty columns and control visibility;
`null` represents None and `[]` hides every column.

- Drag a Markdown card into a writable property column to assign its typed value.
  Dragging a selected card moves the selected batch. Every target is preflighted;
  failures roll back completed changes. None removes the grouped property.
- Drag a writable column heading to persist manual order.
- Click the heading's `+`, or press the New Note binding, to create a note with
  the column's property. Number/checkbox/date/link values retain their types;
  links are retargeted relative to each destination note. Grouping by
  `file.folder` creates the note in that folder without adding a folder property.
- Configuration includes Add empty group, Hide focused group, Move focused group
  left/right, Hide all groups, and Automatic group order / show all. Empty-group
  creation changes only the `.base` document, not existing notes.
- Formula groups and file properties other than `file.folder` cannot receive
  cards, heading drags, or column-created notes. Their order and visibility can
  still be managed through configuration.

`file.folder` **card moves remain unimplemented**; they diagnose as read-only
rather than using ordinary encrypted-note move operations.

## Public data and guarded writes

Bases metadata, formulas, links, backlinks, summaries, images, and exports use a
public-only vault snapshot. `.clin` files, hidden/private files, virtual subnotes,
and escaping symlinks are excluded. Bases never decrypts notes. Public literal
links to private files may remain unresolved text.

Generic property/source edits do not insert clin fields or synchronize titles
with filenames. Unrelated YAML, comments, formatting, body bytes, and file
permissions are retained. Atomic writes check original bytes; configuration
prompts retain the baseline from opening the dialog, even when a watcher refresh
arrives. Stale edits/undo reject rather than overwrite newer files. Encrypted
recovery records are storage-owned under `.clin/document-recovery`.

Unknown plugin/view settings remain opaque and preserved, never executed.
Browser/plugin runtimes and official Maps are not provided. `html()` remains
inert text with diagnostics. See [IMAGE_RENDERING.md](IMAGE_RENDERING.md) for
public image activation limits; remote image URLs are not fetched.

## Remaining implementation and evidence

Live standalone/named/inline embeds, card cover/layout options, folder card moves,
remaining native authoring/integration, and complete end-to-end performance
acceptance remain unfinished. Existing 5,000-note indexing/query benchmarks and
native workflow/PTY tests are stage-specific evidence, not full UI/refresh/memory
acceptance or demonstrated Obsidian runtime parity.

Implementation: `src/bases/`; guarded document/property storage:
`src/storage/{documents.rs,metadata.rs,document_recovery.rs}` and
`src/frontmatter/properties.rs`. Dependency remediation and immutable Mermaid
compatibility pin: [DEPENDENCY_PATCHES.md](DEPENDENCY_PATCHES.md).
