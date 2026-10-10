# Editable frontmatter toggle — implementation plan

Status: planned, not implemented. Related issue: [#181](https://github.com/reekta92/clin-rs/issues/181).

## Scope

Implement option C from #181: one toggleable multiline YAML field at the top of the built-in note editor. Opening it immediately focuses it. Users can add, change, and remove arbitrary user-defined frontmatter fields without leaving Edit view.

This is deliberately not an Obsidian-style properties system. No typed widgets, definitions, schema database, bulk operations, new dependency, or persistent configuration option. The body stays a separate Markdown document.

The first release addresses metadata editing, not every request in #181. Exact YAML formatting/order/comment preservation, configurable managed fields, link deduplication, whole-file external editing, and filename/link migration remain separate work. Existing save serialization can normalize YAML; document this explicitly rather than claiming lossless round-tripping. Do not close #181 solely because this slice ships.

## Current behavior and ownership

Verified against the current `main` source, not the larger properties plans on other branches:

| Owner | Current behavior / required extension |
| --- | --- |
| `src/editor.rs` | `NoteEditor` owns title `TextArea` and body `EditorDocument`; `EditFocus` has Title, Body, Sidebar. Add the frontmatter field here. |
| `src/app/notes.rs` | `load_and_open_note` loads a body-only `Note`. Load the header separately through storage; initialize/reset metadata on every note-open/new-note path. |
| `src/frontmatter.rs` | `Frontmatter.extra` preserves unknown YAML values semantically. `parse` silently falls back on invalid input; editing needs a strict, fallible parser. `serialize` normalizes formatting. |
| `src/storage.rs` | `save_note` rebuilds managed fields and rereads `extra`, pinned, and alignment from disk. A UI-only field cannot persist edits or deletions. Extend this save path, not a second file writer. |
| `src/app.rs` | `autosave`, `tick_autosave`, and encrypted draft persistence own saving. Metadata-only changes must enter the same lifecycle. |
| `src/events/edit.rs`, `src/events/mod.rs` | Key/mouse focus and bracketed paste currently route to title/body/sidebar. Add metadata routing, including selection/clipboard shortcuts. |
| `src/editor_session.rs` | Dedicated Edit loop owns mutation bookkeeping, save scheduling, and redraws. Include metadata mutations without scheduling Markdown work unnecessarily. |
| `src/ui/edit_view.rs` | Header/body/footer layout, preview, and zen behavior. Insert the YAML field above the editor content and reuse existing text-area rendering. |
| `src/keybinds/types.rs`, `src/keybinds/defaults.rs`, `src/keybinds/help_meta.rs` | Per-view edit actions, defaults, and generated help metadata. Add one remappable toggle. |

The graph was refreshed while preparing this plan. Source remains authoritative.

## User-visible behavior

- Add `EditAction::ToggleFrontmatter`, with default `F7` (currently unused in the edit map), plus an edit-view hint/help entry. Do not change existing shortcuts.
- Hidden by default when opening a note. Visibility is editor-session state, not saved configuration. A different note starts hidden with its own header.
- Opening shows a labeled **Frontmatter (YAML)** field below the title/header and above the body/preview content. Focus moves to Frontmatter immediately, preserving its cursor when reopening within the same note.
- Closing returns focus to Body. Closing only hides the field: it does not discard edits, commit them separately, or rebuild the buffer from disk.
- The field edits YAML contents, not the surrounding `---` delimiters. Delimiters are presentation/storage framing, avoiding accidental insertion of a second header into the body.
- Use a bounded, scrollable height (for example, at most one third of available content height), with body space reserved. Small terminals use saturating dimensions and never panic or hide a focused field off-screen.
- Focus cycle becomes Title → Frontmatter, if visible → Body → Sidebar, if visible → Title. Clicking the YAML area focuses it; keyboard-only access remains sufficient. `Tab` inserts a tab rather than changing focus, matching the body editor.
- Typing, multiline paste, selection, cut/copy/paste, undo/redo, and cursor movement operate on the focused field. Existing body-only Markdown actions must not accidentally mutate Body while Frontmatter is focused. Find/go-to-line can remain body-only and explicitly focus Body.
- Preview, outline, body line numbers, body search, wikilink extraction, and word counts continue to consume only body text. Preview/sidebar toggles do not hide or discard metadata. Explicitly opened metadata stays visible in zen mode.
- Save, autosave, and Esc save title/body/frontmatter together. Metadata-only edits are real changes; toggling visibility or moving the cursor is not.
- Invalid YAML remains editable in memory and recovery drafts. A save failure shows a useful error, does not report success, does not leave Edit view, and does not modify the note file.

## Data contract

### Header loading and validation

Add narrow storage/frontmatter helpers that return the actual header separately from the body. Reuse existing delimiter handling, including LF/CRLF input, rather than parsing the already-stripped `Note.content` or doing filesystem reads in rendering.

Keep the existing permissive reader compatible for its existing callers. The editor-specific parser returns `Result<Frontmatter, ...>` and distinguishes no header from a malformed header. Load malformed existing header text into the field for repair; do not silently replace it with empty metadata or save it as body text.

An empty field means no user-defined properties. Otherwise require a YAML mapping, reject duplicate top-level keys rather than silently choosing a value, and validate known fields against their existing types. Preserve arbitrary supported unknown values: strings, numbers, booleans, null, lists, and nested mappings. Do not invent a schema or coerce quoted strings into numbers/booleans.

### Editable versus generated fields

The raw field can display the whole header, but current managed-field rules still apply:

- User-defined keys: edits and removals are authoritative. Never merge deleted keys back from the old file.
- `title`, `tags`, `pinned`, and `text_align`: valid edits update the existing note/editor state and persist. Existing title editing remains available.
- `updated_at`, `links`, and `original_ext`: remain application-managed according to the existing save/encryption behavior. Explain beside the field/help that these may be rewritten on save. Choosing which managed fields are written is outside this slice.
- Removing a managed editable field uses its existing default/fallback; it does not promise that the serializer will omit every managed key. Removing the entire YAML text clears custom keys, tags, pinning, and per-note alignment while retaining the existing title control and save defaults.

Keep title synchronization deterministic: after a successful parse/save or a valid focus transition out of Frontmatter, synchronize its title to the title control. A subsequent title-control edit updates the YAML title through the same helper. If YAML is invalid, block conflicting title edits until it is repaired, while allowing body edits. Avoid two unsynchronized sources of truth and do not reserialize the text area on every keystroke.

### One atomic save path

Extend `Storage::save_note` through a narrow companion/helper that accepts an explicit validated header for editor saves. Existing callers without an override retain current behavior. Both paths share serialization, collision handling, encryption rules, atomic writes, filename policy, and returned note ID.

When the header changed, use the explicit header's `extra`, tags, pinned, and alignment instead of rereading those values from disk. When it did not change, retain the existing disk-preserving path. Title/body/metadata validation finishes before any write or rename. Do not add YAML state to the encrypted `Note` payload or create a separate metadata file.

Capture the loaded header as a baseline. If it changed externally while the user also edited metadata, fail with a conflict instead of silently replacing another writer's metadata. No merge UI is needed for this slice. Compare/reset the baseline only after a successful save; keep pending text intact on failure.

Respect `rename_on_title_change` exactly as today. Document `rename_on_title_change = false` for stable filename-based wikilinks, as raised in #181. Do not silently change users' configuration or rewrite references.

### Dirty state, recovery, and exit safety

Use one mutation bookkeeping path for title/body/frontmatter: invalidate modified-status cache, mark unsaved, schedule autosave, and persist the encrypted recovery draft. Only body edits need Markdown preview/highlight invalidation. Do not count YAML words toward writing goals.

`tick_autosave` currently ignores the save result before setting RecentlySaved. Change this required failure path so malformed YAML or I/O failures remain unsaved, retain the draft, and show an error. Avoid retrying/reporting the same invalid buffer every frame; retry after another mutation or explicit Save.

Extend encrypted draft data with raw YAML text, including invalid intermediate edits, using an explicitly versioned format with legacy `(id, title, body)` decoding. Preserve old drafts without adding plaintext scratch files. A valid recovered draft uses the same save helper. An invalid new draft must remain recoverable for correction in Edit view rather than being written into the note or deleted because validation failed. Delete it only after a successful save or an explicit discard.

Audit editor navigation/handoff paths that save before leaving, including sidebar-linked note activation (`src/app/edit_panes.rs`), because it currently ignores an autosave error. A failed save must prevent switching notes or launching a handoff that loses pending metadata. This is required exit safety, not a broader navigation refactor.

Keep existing decrypt-first behavior for `.clin` notes and existing specialized canvas/draw/subnote/template workflows unchanged. The toggle applies to ordinary built-in `.md`/`.txt` note editing, not arbitrary file buffers. Frontmatter remains plaintext in note files; this feature does not extend encryption protection to metadata.

## Implementation sequence

### 1. Establish header and save semantics

Files: `src/frontmatter.rs`, `src/storage.rs`, focused storage/frontmatter tests.

- Add header extraction and strict YAML validation without breaking existing permissive callers.
- Add the editor-header override to the existing atomic save implementation.
- Test arbitrary-value round-trips, deliberate deletion, managed-field precedence, invalid-input no-write behavior, external-header conflicts, and filename policy.

### 2. Add editor state and lifecycle

Files: `src/editor.rs`, `src/app/notes.rs`, `src/app.rs`, `src/editor_session.rs`, `src/app/edit_panes.rs`, `src/storage.rs` recovery code.

- Add one frontmatter `TextArea`, visibility, saved baseline, and geometry/viewport state needed by rendering/input. Reuse title-field patterns; do not move Body off `EditorDocument`.
- Initialize/reset it for every ordinary note lifecycle; synchronize editable managed fields through one helper.
- Include metadata in modified detection, autosave, encrypted drafts, and recovery; block navigation on failure.
- Test metadata-only edits, no-op toggles, save failures, legacy/new draft recovery, and note-to-note reset.

### 3. Wire toggle, input, and layout

Files: `src/keybinds/{types,defaults,help_meta}.rs`, `src/events/{edit,mod}.rs`, `src/editor_session.rs`, `src/ui/edit_view.rs`, existing text-area helpers in `src/ui/mod.rs` where needed.

- Add the remappable toggle and focus variant; update exhaustive focus matches.
- Route key/mouse/paste/selection actions to the frontmatter field and record actual text mutations.
- Render the bounded top field with focused styling/cursor and reuse the same geometry for mouse hit-testing. Keep body/preview/sidebar geometry consistent.
- Test focus on toggle, hide/reopen retention, focus cycling, multiline paste, Unicode input, undo/redo, and small-terminal layout with preview/sidebar/zen combinations.

### 4. Document and prove the slice

Update `docs/EDITOR.md`, `docs/KEYBIND_PRESETS.md` where relevant, generated-help metadata, and the local manual checklist `development/TESTS.md`. Do not add config keys, bump versions, or edit generated `CHANGELOG.md`.

Run:

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

Run the TUI against a throwaway vault. Use the #181 example header (`id`, `type`, `status`, `confidence`, `created`, `updated`): open with F7, edit `status`, add a list/nested value, remove `confidence`, hide/reopen, edit Body, then save/reopen. Verify YAML values and deletions on disk, filename behavior with rename disabled, and unchanged body line addressing. Repeat with autosave, invalid YAML, a failed write, externally changed metadata, and draft recovery. Never use the user's live vault for these checks.

## Done criteria

- One optional YAML field at the top; opening it focuses it immediately.
- Arbitrary supported custom fields can be added, changed, and removed and survive save/reopen.
- Hidden-field edits, metadata-only edits, and invalid intermediate drafts are not lost.
- Invalid YAML, conflicts, or write failures cannot overwrite the note, falsely report Saved, or let navigation discard pending changes.
- Body editing, preview, sidebars, filename configuration, old keybind configs, encrypted drafts, and specialized non-note editors retain their existing contracts.
- Tests and throwaway-vault smoke checks pass; documentation clearly states normalization and managed-field limitations.
