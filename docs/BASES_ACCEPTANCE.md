# Bases acceptance

Recorded on 2026-10-15 for the native core implementation targeting Obsidian
1.14.3 file/query semantics. This records clin evidence, not demonstrated Obsidian
runtime parity. The requested Obsidian runtime comparison was explicitly waived.

## Native design audit

Notes (`src/ui/list_view.rs`) is the design authority. Bases uses the existing
view title bar, theme, popup geometry, text-input machinery, Find widget, shared
command palette, configurable binding display, and scrollbar implementation.
The following matrix records the final source audit and regression surfaces.

| Surface | Design and interaction checked | Evidence |
| --- | --- | --- |
| Table | Shared title/count/footer, focused property, group headings, summaries, grapheme wrapping, pixel-width conversion, tall rows clipped to short terminals, horizontal property focus, hover, selection, scrollbar and paging | `layout_workflow_tests.rs`; four-layout PTY |
| List | Bullet/number/none, indentation, custom/default separator, grouped numbering, labels, long/Unicode properties, visible focused property, hover, mouse hits, scrollbar and paging | `layout_workflow_tests.rs`; four-layout PTY |
| Cards | Themed bordered grid, selected property visibility, bulk-selection border, hover, covers/colors/text fallback, grouped sections, clipping, scrollbar and paging | `workflow_tests.rs`, `media_workflow_tests.rs`, `layout_workflow_tests.rs`; four-layout PTY |
| Kanban | Themed columns, focused group/card/property, empty columns and add control, public typed drag/drop, read-only group rejection, manual visibility/order, covers, clipping, scrollbar and paging | `workflow_tests.rs`, `media_workflow_tests.rs`, `layout_workflow_tests.rs`; four-layout PTY |
| Notes preview | Native regions inside existing preview geometry; public containing-file context and no stale encrypted-note context | `embed_workflow_tests.rs` |
| Editor preview | Same renderer and input owner; unsaved host context; watcher refresh while editing; popup focus; original source/editor return path | `embed_workflow_tests.rs`; editor PTY |
| Active-file sidebar | Shared native view inside editor pane; configurable chord; public host context; close/toggle and focus return; private hosts reject | `embed_workflow_tests.rs`; sidebar PTY |
| Linked-note popup | Live region inside the existing padded popup; linked public note owns `this`; region dialogs remain modal | `embed_workflow_tests.rs` |
| Canvas | Existing Pinstar viewport projection; `.base` file and inline text nodes; clipping; public Canvas context; exact JSON string splice; native state reload and guarded history | `embed_workflow_tests.rs`; Canvas PTY |
| Configuration | Existing popup geometry/theme/input; scrolling and mouse selection; invalid source recovery; retained opening baseline; unrelated YAML and unsupported views preserved | `layout_workflow_tests.rs`, `workflow_tests.rs`; configuration PTY |
| Typed edit / clear / bulk | Shared input/selection conventions; typed validation, explicit clear confirmation, paste once, Enter apply, Shift+Enter multiline, Esc cancel, computed/attachment writes reject with diagnostics | `workflow_tests.rs`, `palette_workflow_tests.rs`; edit/cancel PTY |
| Creation / move | Native filename/format selector, background operation, visible collision/error state, public scope, no-clobber destination, group context and guarded history | `creation_workflow_tests.rs`, `workflow_tests.rs`, `layout_workflow_tests.rs`; earlier creation PTY |
| Export | Existing native prompt, explicit path, displayed public values, no private rows or cover-only hidden projections | `workflow_tests.rs` |
| Search | Notes Find widget, configurable binding, no-match/restore behavior, Unicode, group-collapse/focus continuity, popup precedence | `workflow_tests.rs`; four-layout PTY |
| Image | Existing native popup/protocol, unavailable/disabled/error and text fallback, bounded jobs/cache, cancellation and public local/remote guards | `media_workflow_tests.rs`, `workflow_tests.rs`, image-render tests; earlier cover PTY |
| Command palette / help | Shared public-scoped palette, configured bindings/sequences/counts and presets, popup precedence, quick-binding suppression, expansion and return stack | `palette_workflow_tests.rs`, `embed_workflow_tests.rs`, keybind tests; palette/editor PTY |

Loading, empty, no-match, invalid/unsupported configuration, stale writes,
read-only mutation, unavailable images, tiny terminals, resizing and terminal-text
sanitization are covered across the layout, query, worker, image and workflow
suites. Bases does not present Notes' encrypted-note sidebar/count context.
Card and Kanban sizes follow saved layout settings; their palette, typography,
focus, theme and popup conventions follow Notes rather than browser styling.

The audit fixed popup mouse dismissal/selection, Table/List clipping and focus,
viewport-based paging, Cards/Kanban scrollbar and hover gaps, private sidebar
activation, first-key loss after background source handoff, expanded-result
routing, and restoration of the original Bases/editor return stack.

## Safety and lifecycle

The deterministic tests cover public-only metadata, all typed values and query
controls, aliases/tags/duplicates and malformed YAML, lossless configuration and
frontmatter edits, CRLF/BOM/comments, private/hidden/symlink boundaries,
ECMAScript regex/UTF-16/date semantics, bounded evaluation, stale dialog/history
rejection, permission preservation, atomic publication, binary folder moves,
rollback, encrypted recovery and retained conflicts. Existing encrypted Note and
bincode compatibility tests remain in the full suite.

Live-host regressions cover named/inline/missing/ambiguous definitions, multiple
references on one line, unsaved/external host changes, metadata/schema changes,
obsolete generations/cancellation, close/reopen, separate vaults sharing Markdown
geometry, pre-cache private-render exclusion, modal input and public-only source
handoffs. One app-owned query worker and immutable public metadata snapshots serve
all regions. There are no per-embed scans or threads. A one-entry result cache
shares identical ordinary queries; clock/random/relative-date expressions bypass
reuse. Full rebuilds reread public files and share unchanged file metadata;
backlink changes use copy-on-write and never mutate older snapshots.

Inline save/undo/redo tests assert exact Markdown/Canvas bytes and synchronized
native editors. Pending host body/title/raw changes reject configuration writes.
Metadata-only updates preserve pending body edits and migrate encrypted drafts,
including replay using the same recovery identity. Ordinary public Markdown hosts
retain their real opening baseline when promoted to guarded saving. No-op saves
retain trailing bytes and CRLF; title changes update YAML without renaming files.
Canvas ordinary undo snapshots are cleared after guarded host replacements because
Bases history owns those writes. Pending/invalid Canvas editors remain intact.

Remote image tests enforce destination/redirect/DNS checks, TLS/body/deadline and
decode bounds, feature/picker gates and cancellation. They do not establish safety
of every possible network or image implementation. Public URLs disclose requests
to remote servers as described in [BASES.md](BASES.md).

## Reproducible checks

```sh
development/scripts/check.sh --strict
cargo test --locked bases:: --lib
cargo test --locked storage::metadata --lib
cargo test --locked --release bases::workflow_tests::acceptance::native_5000_note_acceptance --lib -- --ignored --nocapture
```

Final result: **six passed, zero failed, zero skipped**; **596 library tests** and
**8 integration tests** passed. Five opt-in tests remain ignored by the ordinary
test command; the complete native release acceptance above passed separately.

The six required gates are formatting, locked check, all-target Clippy with denied
warnings, complete tests, dependency policy/advisories, and Rust 1.90 locked check.
Ignored performance tests run separately; none of the six gates is skipped.

Actual disposable-vault PTY checks used the built clin binary, a 100×30
`xterm-256color` pseudo-terminal and decoded screen assertions. The native check
opened Table/List/Cards/Kanban, focused properties, opened configuration, edited
and cancelled, searched, resized to 8×4 and back, compared every public/private
fixture byte and exited cleanly. The host check entered public source from Bases,
opened editor preview and the active-file sidebar, changed/restored a public note
externally while the editor remained open, expanded the public command palette,
returned through the original editor/Bases stack, rendered Canvas file/inline
regions, edited/cancelled and compared fixture bytes after clean exit.

For a manual repeat:

1. Create a disposable vault with public typed Markdown, an opaque private `.clin`
   sentinel and a `.base` with all four layouts. Retain source hashes and modes.
2. Open each saved view, exercise focus, group collapse, search, configuration,
   typed edit/clear/cancel, bulk selection, undo/redo, creation, move and export.
3. Open public Markdown containing named references and a fenced `base`. Exercise
   Notes/editor/linked-note previews and the `Ctrl+g Ctrl+b` sidebar. Refresh an
   external note, then edit unsaved host text; verify freshness and intact buffers.
4. Open a Canvas with a `.base` file node and inline text node. Change inline
   configuration, undo/redo, then try pending invalid raw input. Verify exact
   unrelated JSON bytes and rejection without losing the pending input.
5. Open popups, resize to tiny dimensions, test mouse/scrollbar and Unicode,
   return through every source/full-view handoff, close/reopen, and switch vaults.
6. Confirm unchanged sentinel bytes, no private results/export/cache entries,
   guarded stale rejection, permissions and recoverable pending edits.

## Release measurement

Linux x86_64, kernel `7.2.10-1-cachyos`, AMD Ryzen 7 7840HS (16 logical CPUs).
Cargo release profile; 120×40 TestBackend excludes terminal I/O, image decode and
network. Fixture: 5,000 Markdown notes with approximately 10 KiB bodies, 1 KiB
frontmatter, 20 properties, 50,000 links, nested tags/duplicate basenames and 500
attachments. All layouts exercise grouping, multisort, formulas and summaries.
Host catalog availability precedes the cold-query timer and RSS baseline.

| Measurement | Observed | Required |
| --- | --- | --- |
| Cold metadata + first query | 409.350 ms | ≤3 s |
| Uncached ordinary full query | p50 57.030 / p95 61.535 / max 67.638 ms | p95 ≤150 ms |
| Single-note watcher-to-results, including 250 ms debounce | p50 390.551 / p95 426.309 / max 426.309 ms | p95 ≤500 ms |
| Visible Table/List/Cards/Kanban render CPU | p95 0.174 / 0.871 / 0.328 / 0.171 ms | p95 ≤16 ms |
| Input during indexing | 0.047 ms | ≤50 ms |
| Cancellation | 0.115 ms | ≤100 ms |
| Incremental native Bases RSS, after repeated refresh and two live embeds | 121,132 KiB (118.293 MiB) | ≤128 MiB |

Render p50/max (ms): Table 0.158/0.782, List 0.835/5.067, Cards 0.126/1.212,
Kanban 0.163/0.297. Host RSS was 35,368 KiB; final process RSS 156,500 KiB.
Four two-embed close/reopen cycles took 414–727 ms. The final two cycles differed
by 64 KiB RSS; no rows were dropped. Repeated runs on this host passed the same
limits; timings vary with background load.

RSS is measured before standalone evaluator/cancellation microbenchmarks create
additional allocator arenas. It includes actual native full refreshes, rendering,
watcher updates and repeated close/reopen. No covers are decoded in this fixture;
image and syntax caches remain separately bounded by their existing owners. Rows
are never capped to satisfy the measurement. Limits describe this reproducible
baseline, not every vault, terminal, hardware or query.

## Compatibility limits

Automated file round trips, native terminal workflows and release resource gates
are demonstrated. Obsidian 1.14.3 runtime execution was waived and was not run.
Table/List option research included static inspection of older 1.10.6 assets;
that is not 1.14.3 behavioral runtime proof. Simultaneous Obsidian/clin writers
remain unsupported; ordinary stale-write guards still apply.

Windows/macOS runtime and terminal protocols, Nix build, full upstream Mermaid
SVG DOM parity and upstream Mermaid benchmarks were not run here. Linux tests
include binary moves and cross-device paths where the fixture can provide them;
this does not establish other-platform filesystem semantics. Browser/plugin/Maps
execution is excluded. The immutable Mermaid compatibility patch and dependency
policy remain documented in [DEPENDENCY_PATCHES.md](DEPENDENCY_PATCHES.md).
