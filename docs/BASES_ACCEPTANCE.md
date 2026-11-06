# Bases acceptance checks

Use these checks when changing native Bases queries, layouts, authoring, or live
embeds. They validate Clin behavior; automated file round trips do not establish
Obsidian runtime parity. Record candidate-specific results in the release issue.

## Automated checks

```sh
scripts/check.sh
cargo test --locked bases:: --lib
cargo test --locked storage::metadata --lib
cargo test --locked --release bases::workflow_tests::acceptance::native_5000_note_acceptance --lib -- --ignored --nocapture
```

The Bases suites cover query semantics, public-only metadata, lossless YAML and
frontmatter edits, stale-write rejection, rollback/recovery, layouts, images,
command-palette routing, and live Markdown/editor/Canvas hosts. The ignored
5,000-note benchmark checks query, rendering, refresh, cancellation, and resource
limits; see [Performance](PERFORMANCE.md) for its fixture and scope.

## Manual checks

Use a disposable vault and isolated configuration. Preserve fixture hashes and
permissions before starting.

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

## Compatibility limits

Target semantics are documented in [Bases](BASES.md). Compare against an actual
Obsidian runtime before claiming behavioral parity. Simultaneous Obsidian/Clin
writers are unsupported; ordinary stale-write guards still apply.

Test Windows/macOS filesystem behavior, image protocols, and real terminal input
separately. TestBackend and Linux-only checks cannot establish those results.
Remote-image fixtures exercise bounded transport and decoding, not every network
or codec implementation. See [Image Rendering](IMAGE_RENDERING.md),
[Upgrading](UPGRADING.md), and [Release acceptance](RELEASE_CHECKLIST.md).
