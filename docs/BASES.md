# Bases

Terminal-native `.base` documents, targeting Obsidian 1.14.3 file and core query
semantics. Core layouts, authoring, guarded writes, and live public embeds are
implemented. Obsidian runtime parity has not been demonstrated.

## Open and navigate

Open a `.base` file from the notes list, or use `clin notes open TITLE`.
`[features] bases_view` enables the view (default enabled). The first saved view
opens by default. Create a base through Notes' **Create New → Bases (.base)**
selector or command palette's **Create Bases File** action. The selector uses the
existing filename popup; the palette action uses the palette input. Both retain
the current public folder context. Creation runs in the background,
uses no-clobber public document storage, and opens the default table. Existing
files/private paths reject without note serialization or clin/title fields.

The command palette's **Bases** category contains all 34 configuration commands
and appears only in the Bases view. `:` opens that category directly; F6 or
Ctrl+p opens the shared palette. Configuration uses palette inputs instead of a
separate configuration menu. Property edits, public note creation, export and
group moves launched from the palette also keep their inputs there. Esc returns one step.
Failed writes retain the entered value. Removing a view requires typing DELETE
inside the palette; the last view cannot be removed. Search, cover previews,
typed CLEAR confirmations, and other direct shortcut dialogs keep their existing presentation.
In an embedded Bases region, `:` expands the region into Bases and opens its
palette category; leaving Bases restores the host view.

Layout, table row height, list markers, list property indentation, and image fit
use searchable option lists with the current value selected and marked. Choose
with Enter or the mouse; successful saves close the palette. Default choices
remove the setting and restore its native default. Other configuration commands
retain their value inputs.

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
| `e`, `Delete` | Edit/clear a note property |
| `Space`, `Ctrl+a` | Toggle selection/select visible results |
| `m` | Set grouping property on selected notes; blank clears the property |
| `n` | Create a Markdown note; Kanban uses the focused column |
| `u` / `Ctrl+r` | Guarded session undo/redo of property and configuration edits |
| `:` | Bases category in the command palette |
| `Ctrl+p` | Shared command palette with public Bases commands and approved globals |
| `r`, `x`, `Esc` / `q` | Refresh, export, leave Bases |

Prompts accept Enter to apply, Shift+Enter for a newline, and Esc to cancel.
Configuration's **Edit Bases source** opens the guarded public source editor,
including when parsing or view validation fails. Another valid saved view remains
usable and editable while an invalid view's settings remain preserved.
All presets inherit the Bases map; custom `bases` bindings remain supported.
The header shows plain tabs for the document’s saved views and the document ID. Click a tab or use the existing next/previous-view keys; switching preserves each saved layout and does not rewrite the document. Narrow headers keep the active tab visible. The footer shows counts, selection, search and refresh status through `{detail}`. Both bars follow global statusline templates and `ui.hint_bar_style`.

Statusline variables: `{bases_file}`, `{bases_rows}`, `{bases_matched}`, `{bases_selected}`,
`{bases_view}` and `{bases_property}`. Legacy count/selection/index variables use
public query state in Bases, separate from the ordinary note catalog.

## File and query format

A `.base` file contains standalone YAML without Markdown frontmatter delimiters.
For example, this table lists Markdown files with numeric `price` and `amount`
properties, calculates totals, and sorts largest first:

```yaml
filters:
  and:
    - 'file.ext == "md"'
    - 'file.hasProperty("price") && file.hasProperty("amount")'
formulas:
  total: 'note.price * note.amount'
views:
  - type: table
    name: Totals
    order: [file.name, price, amount, formula.total]
    sort:
      - property: formula.total
        direction: DESC
    summaries:
      formula.total: Sum
```

Global `filters` and each view's `filters` both apply. Filters accept an expression
or nested `and`, `or`, and `not` lists. `order` selects displayed properties;
`sort` orders rows using `ASC`/`DESC`. `limit` caps displayed rows, while `groupBy`
and `groupOrder` control groups. Property labels use top-level `properties` entries
with `displayName`; top-level `summaries` defines custom summary expressions.

Expressions can reference `file` metadata, `note` properties, `formula` values,
and the public host through `this`. They support typed arithmetic, comparisons,
boolean operations, lists, dates, links, and supported type methods. This is a
bounded native expression evaluator; arbitrary JavaScript and plugin code do not
execute. Unknown functions, cycles, invalid types, and exhausted resource budgets
produce diagnostics. Declared note-property types come from `.obsidian/types.json`.

## Table and List settings

Table configuration supports `rowHeight`: empty/default (one terminal row),
`medium` (two), `tall` (four), and `extra` (eight). `columnSize` maps property names
to pixel widths, for example `{file.name: 240, score: 120}`. Widths use terminal
cell geometry; unspecified properties use 20 columns. Cells wrap at grapheme
boundaries. Horizontal navigation exposes the focused property, including columns
wider than the viewport. Short terminals clip rows and retain usable focused
content; headers, group headings, and summaries have separate mouse regions.

List configuration supports `markers`: `bullet` (default), `number`, or `none`.
Numbering restarts in each group. `indentProperties: true` places secondary
properties on separate indented lines; otherwise `separator` joins values inline.
Its default, including empty text, is `", "`. The first property is the title.
Property navigation keeps the focused secondary value visible when earlier values
exceed the viewport. Click a displayed value to focus it for editing.

All settings use the existing lossless view configuration. Blank prompts remove
the setting and restore its default. Invalid types and ranges reject without
changing source. Rename, duplicate, and move from Notes preserve `.base` bytes;
rename/move use guarded public transactions and duplication preserves Unix modes.

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
Undo/redo checks the same baselines. Private plaintext move records retain staged rollback
data through publication and durable commit; startup recovery restores interrupted
moves or retains conflicting records. Authored links remain verbatim.

## Cards, Kanban and covers

The first displayed property is the card title. Remaining properties retain labels;
`[` / `]` exposes and focuses properties beyond the visible caption window.
Configuration supports core `cardSize`, `columnWidth`, `image`, `imageFit` and
`imageAspectRatio` fields. Pixel widths map to terminal columns using cell geometry;
`cover` crops, `contain` preserves the full image, and aspect ratio is width/height.
Geometry stays within the viewport. Unknown/plugin fields remain unchanged.

## Live embeds and sidebar

Public Markdown previews support `![[File.base]]`, `![[File.base#View]]`, and fenced
`base` definitions. Omit `#View` to use the first saved view. Missing or ambiguous
files/views show diagnostics. Inline definitions retain their source anchors;
they are live native views, including Table, List, Cards, and Kanban controls.
The same regions appear in the editor, Notes preview, linked-note preview,
and Canvas `.base` file nodes and text nodes containing Bases definitions.
Embedded regions render only the view body, using the full region for content.
The Bases header and footer appear when the region is expanded into the full view.

Click a region to focus its rows/properties, then use the configured Bases
bindings. Esc closes its popup first, then returns keyboard focus to the host.
Unmodified `F6` (unless custom-bound) or the Bases command-palette binding (`Ctrl+p` by default) expands the focused region
into the full view and opens its public-scoped palette. Ctrl+Enter also expands
a focused region. Leaving returns to the
original region. Source handoffs preserve an unsaved containing editor buffer.

In the editor, place the cursor on a `.base` wikilink and press **Ctrl+g Ctrl+b**
(`edit.ToggleBases`) to open its active-file sidebar. Repeat to close it. `this`
uses the public containing note or Canvas file; standalone bases use their own
file. Public unsaved editor text updates host properties and links without
replacing the editor buffer. Encrypted notes and virtual subnotes never create
live definitions or enter Bases query/cache state.

Regions reserve eight terminal rows in Markdown and clip to their host viewport.
There are at most 32 definitions per Markdown document and 32 resident live
regions per app; further regions show a resource diagnostic. A single cancellable
query worker shares public snapshots across regions. Markdown caches retain
source geometry; host identity, document revision, source anchor, view, metadata
revision, and volatile query context belong to the live query state. Private host
rendering disables Bases recognition before Markdown cache insertion.

Watcher changes refresh individual public files and their links; folder/schema
changes and explicit refresh rebuild the snapshot. Closing or replacing a region
cancels obsolete work. Bases cells do not recursively interpret Markdown, so result
values cannot introduce recursive embeds or execute scripts.

Inline configuration saves splice only the original YAML range. Canvas text-node
edits replace only that node's quoted JSON text value, retaining other JSON bytes.
Save an unsaved containing document/Canvas node before authoring its inline
configuration; stale baselines reject. Property edits use the same guarded
transactions and history as standalone views. Focusing a live region promotes an
ordinary public Markdown editor to guarded, lossless saving while keeping its
body-only presentation. Title edits update the YAML title without renaming the
file or injecting clin fields. Pending body edits survive metadata-only updates;
conflicting source replacements reject. Inline saves and history synchronize the
containing editor and Canvas state. Canvas's ordinary undo snapshots are cleared
after a guarded host replacement; Bases history owns that replacement.

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
arrives. Stale edits/undo reject rather than overwrite newer files. Private plaintext
recovery records are storage-owned under `.clin/document-recovery-v2`. See [Upgrading](UPGRADING.md) for privacy and legacy-data requirements.

Unknown plugin/view settings remain opaque and preserved, never executed.
Browser/plugin runtimes and official Maps are not provided. `html()` remains
inert text with diagnostics. See [IMAGE_RENDERING.md](IMAGE_RENDERING.md) for
public image activation and remote-request limits.

The editor's hidden **F7** properties/raw-YAML region can edit custom and managed fields above Body. Unsaved header changes feed live embed/sidebar host metadata without becoming Markdown text. Metadata-only revisions refresh host-dependent queries. A Bases write targeting an active or suspended editor with pending frontmatter rejects until that editor is saved. Metadata-only rebases may retain pending body edits when no local header changes would be replaced; source handoff, undo/redo, and return routes retain their document/recovery identity. The public-file boundary is unchanged.

## Acceptance and compatibility

[BASES_ACCEPTANCE.md](BASES_ACCEPTANCE.md) describes reproducible automated and
manual checks. Obsidian 1.14.3 runtime parity has not been established.
Simultaneous Obsidian/Clin writers are unsupported. Linux tests do not establish
Windows/macOS or Nix build parity.

Implementation: `src/bases/`; guarded document/property storage:
`src/storage/{documents.rs,metadata.rs,public_moves.rs,document_recovery.rs}` and
`src/frontmatter/properties.rs`. Dependency remediation and immutable Mermaid
compatibility pin: [DEPENDENCY_PATCHES.md](DEPENDENCY_PATCHES.md).

Shared theme/preset/settings and guarded navigation are available in the Bases palette. Private Notes actions and targets remain excluded. Creating another Bases file uses the active public file's parent folder. Navigation and vault commitment wait for an active write to finish; completion and undo state continue polling while the palette is open.
