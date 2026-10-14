# Bases

Terminal-native `.base` documents, targeting Obsidian 1.14.3 file and core query
semantics. Implementation is still in progress; this is not a claim of complete
Obsidian runtime parity.

## Open and navigate

Open a `.base` file from the notes list, or use `clin notes open TITLE`.
`[features] bases_view` enables the view (default enabled). The first saved view
opens by default. Create a base through Notes' **Create New → Bases (.base)**
selector or command palette's **Create Bases File** action. Both reuse the native
filename popup and current public folder context. Creation runs in the background,
uses no-clobber public document storage, and opens the default table. Existing
files/private paths reject without note serialization or clin/title fields.

Configurable `bases` keybindings control navigation and editing;
press the Bases Help binding (`?` by default) for the current map.

Default bindings include:

| Binding | Action |
| --- | --- |
| `h/j/k/l`, arrows | Navigate columns/rows; Kanban horizontal navigation crosses groups |
| `Tab` / `Shift+Tab` | Next/previous saved view |
| `Enter` | Open public source/native image; toggle a grouped Table/List/Cards header |
| `/` | Search displayed public property values with the Notes-style Find bar |
| `z` | Collapse/expand focused Table/List/Cards group |
| `[` / `]` | Focus previous/next displayed property, including inside Kanban cards |
| `v` | Preview configured public/remote cover image |
| `e` / `F2`, `Delete` | Edit/clear a note property |
| `Space`, `Ctrl+a` | Toggle selection/select visible results |
| `m` | Set grouping property on selected notes; blank clears the property |
| `n` | Create a Markdown note; Kanban uses the focused column |
| `u` / `Ctrl+r` | Guarded session undo/redo of property and configuration edits |
| `:` | Bases configuration menu |
| `Ctrl+p` | Notes' shared command palette, scoped to public Bases commands |
| `r`, `x`, `Esc` / `q` | Refresh, export, leave Bases |

Prompts accept Enter to apply, Shift+Enter for a newline, and Esc to cancel.
All presets inherit the Bases map; custom `bases` bindings remain supported.
Statusline variables: `{bases_rows}`, `{bases_matched}`, `{bases_selected}`,
`{bases_view}` and `{bases_property}`. Legacy count/selection/index variables use
public query state in Bases, not the ordinary encrypted-note catalog.

## Search and grouped sections

Search uses the same themed Find bar, text input, keyboard routing, and mouse
selection as Notes. Literal case-insensitive text matches displayed property and
formula values, not note bodies or hidden sort/summary properties. Search runs in
the cancellable query worker before sorting, grouping, limits, and summaries;
explicit empty groups remain available. Input is bounded to 64 KiB and search
work shares the formula evaluator's resource/cancellation budget.

Enter keeps the search and focuses a selected public result. Esc restores the
previous search and row/group focus. Clear input and press Enter to remove the
search. Watcher refreshes and saved-view switching retain search text. Search is
session-only: it never edits `.base` filters or public source bytes.

Grouped Table, List, and Cards sections collapse with their header's Enter,
mouse click, or the configured Toggle Group binding. Header focus uses theme
selection colors; `[+]`/`[-]` show collapsed/expanded state. Collapse follows typed
group identity across refresh and manual reordering, resets on saved-view or
grouping-property changes, and never changes query rows, summaries, or exports.
Choosing a search result expands its section. Kanban columns do not collapse.

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

`file.folder` card moves use guarded public file transactions, including binary
attachments. Basenames, bytes, modification times and permissions are preserved.
Same-filesystem moves retain file identity; cross-filesystem moves stream staged
copies without loading attachments into memory. Destination collisions, changed
source bytes/timestamps/permissions, hidden/private paths and symlinks reject the
batch before publication. Blank keyboard Move To Group moves to the vault root.
Undo/redo checks the same baselines. Encrypted move records retain staged rollback
data through publication and durable commit; startup recovery restores interrupted
moves or retains conflicting records. Authored links remain verbatim.

## Cards, Kanban and covers

The first displayed property is the card title. Remaining properties retain labels;
`[` / `]` exposes and focuses properties beyond the visible caption window.
Configuration supports core `cardSize`, `columnWidth`, `image`, `imageFit` and
`imageAspectRatio` fields. Pixel widths map to terminal columns using cell geometry;
`cover` crops, `contain` preserves the full image, and aspect ratio is width/height.
Geometry stays within the viewport. Unknown/plugin fields remain unchanged.

The image property can contain a public attachment link, typed `image()`/link
value, HTTP(S) URL, or `#RGB`/`#RRGGBB` color. Cover-only values are projected but
do not enter displayed-property search or exports. Local images use public-only
storage guards. Colors need no image feature or file/network access. Decode runs
in the background: two concurrent cover jobs, four queued results, 64-entry LRU,
256-pixel thumbnails. Explicit refresh cancels and invalidates cached generations.
The preview binding opens the same native image popup without leaving Bases.

Remote covers require enabled images and an initialized picker. Requests use
credential-free HTTP(S), native TLS trust roots, no cookies/proxy/Referer, and only
public destination addresses. Every DNS answer and redirect destination is checked;
loopback, private, link-local, multicast and reserved targets are rejected. Bounds:
64 KiB URLs, three redirects, eight-second total deadline, 8 MiB body, shared image
decode limits. Cancellation is checked every 10 ms during transfer. Source URLs
are sent to their servers, which can observe your public network address; disable
`[features] images` to prevent image requests. HTTP lacks HTTPS transport integrity.

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
public image activation and remote-request limits.

## Remaining implementation and evidence

Live standalone/named/inline embeds, remaining native layout/authoring/integration, comprehensive Notes-design alignment,
and complete end-to-end performance acceptance remain unfinished. Existing 5,000-note indexing/query benchmarks and
native workflow/PTY tests are stage-specific evidence, not full UI/refresh/memory
acceptance or demonstrated Obsidian runtime parity.

Implementation: `src/bases/`; guarded document/property storage:
`src/storage/{documents.rs,metadata.rs,public_moves.rs,document_recovery.rs}` and
`src/frontmatter/properties.rs`. Dependency remediation and immutable Mermaid
compatibility pin: [DEPENDENCY_PATCHES.md](DEPENDENCY_PATCHES.md).
