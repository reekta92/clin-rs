# Performance checks and resource ownership

## Runtime ownership


- Static views draw only after input, a resize, or a relevant background update. Completions are polled before drawing. Resize and pointer-move bursts are coalesced; drag samples and keys following a view transition are retained.
- Hidden Markdown jobs release their result receivers and cancel work. Returning to a view resumes unfinished rendering while retaining the displayed document and scroll position. Result delivery observes cancellation even when its bounded channel is full.
- Hidden Graph physics stops through graf's existing worker lifecycle. Returning waits for the previous worker to stop before starting another.
- Notes/Edit image decoding and native protocol encoding use one worker, a 16-job queue, and a two-result queue. Request identities route results to their owning cache and reject stale resolution results. Eviction, view hiding, and offscreen pending images cancel work. Decode errors and worker disconnections complete pending state.
- Each host image cache honors `image.cache_size` and a 64 MiB decoded-pixel budget. This is not a total process-memory limit: protocol buffers, the active decoder, and upstream Canvas caches have separate storage.
- Selected preview files are read and parsed outside the input/render path. The loader retains one source, with one active read and one waiting request. Resizes reuse the retained source; watcher events invalidate it.
- Catalog events are merged in a batch, followed by one sort/index/list rebuild. Cursor and bulk selection follow stable item identities. The note worker pool is capped at four workers.
- Multi previews and Outline build viewport rows only. Outline visibility/connectors are cached between expansion changes. Canvas snapshots build one node lookup for all edges.
- Calendar expansion and note-link metadata scans run on the existing worker pool. Results are keyed by range/revision; event rendering and edits share the same immutable store snapshot. Reload/update cancels obsolete work, and hidden note-link jobs are cancelled.
- Editor serialization and status rendering share a revision-cached body snapshot. `{modified}` compares against the loaded save baseline without reading files during rendering and updates after undo, save, and pending property input.

## Benchmark scope

Benchmarks use synthetic disposable vaults and Ratatui TestBackend buffers.
Frame timings exclude terminal transport and most background I/O. Record the
candidate revision, OS, CPU, compiler, profile, fixture, and filesystem conditions
with results; measurements do not guarantee performance on other hardware.

| Benchmark | Workload | Enforced limits |
| --- | --- | --- |
| Mixed views | 1k/10k/50k notes, 5k-line note, 500-node Canvas, 500-element drawing, 120×40 buffer; nine views and four Notes layouts | With `CLIN_PERF_ENFORCE=1`, frame and non-editor navigation/frame p95 ≤16 ms |
| Editor persistence | Input, drawing, recovery and saves for 5k/100k-line plaintext notes | With `CLIN_PERF_ENFORCE=1`, input/render cycle p95 <50 ms |
| Bases | 5,000 notes, 500 attachments, four layouts, queries, watcher refresh and two live embeds | Cold query ≤3 s; warm query p95 ≤150 ms; refresh p95 ≤500 ms; render p95 ≤16 ms; input ≤50 ms; cancellation ≤100 ms; incremental RSS ≤128 MiB when measurable |
| Resource soak (Linux) | At least ten minutes and 100 cycles through nine views, 1k-note vault, actual image worker and watcher | Retained RSS growth ≤128 MiB; at most four additional threads/descriptors after warmup |

Bases RSS excludes decoded covers and standalone evaluator microbenchmarks;
the host catalog is available before its cold-query timer. Bases asserts that
all 5,000 rows remain present. The benchmark retains its temporary vault and
prints the path for inspection.

## Reproduction

Use temporary XDG directories so tests cannot mix their state with an installed app:

```sh
CLIN_PERF_ROOT=$(mktemp -d)
export XDG_CONFIG_HOME="$CLIN_PERF_ROOT/config"
export XDG_DATA_HOME="$CLIN_PERF_ROOT/data"
export XDG_STATE_HOME="$CLIN_PERF_ROOT/state"
export XDG_CACHE_HOME="$CLIN_PERF_ROOT/cache"
cargo test --locked --offline -- --test-threads=4
cargo clippy --locked --offline --all-targets -- -D warnings
scripts/check.sh fmt deny msrv

CLIN_PERF_ENFORCE=1 cargo test --locked --offline --release --lib \
  view_perf_tests::mixed_vault_view_performance -- --ignored --nocapture --test-threads=1
CLIN_PERF_ENFORCE=1 cargo test --locked --offline --release --lib \
  editor_session::safety_tests::full_editor_persistence_perf -- --ignored --nocapture --test-threads=1
cargo test --locked --offline --release --lib \
  bases::workflow_tests::acceptance::native_5000_note_acceptance -- --ignored --nocapture --test-threads=1
cargo test --locked --offline --release --lib \
  view_perf_tests::repeated_views_resource_soak -- --ignored --nocapture --test-threads=1
```

The Linux soak defaults to ten minutes and at least 100 complete cycles through nine views. It uses a 1k-note vault, images, Markdown jobs, Graph workers, and an actual session image worker/watcher. It samples `/proc/self` and checks retained RSS growth against 128 MiB, with at most four additional threads/descriptors after warmup. `CLIN_PERF_SOAK_SECONDS` can shorten local diagnostic runs; the full acceptance run uses the default. Run it alone. Run timing benchmarks after compilation finishes.

The ordinary regression suite also checks idle rendering (at most three frames in 650 ms), saturated image queues, cancellation/resume, stale image results, image-owner routing, Calendar snapshot consistency, catalog selection, and save/recovery behavior. Some image tests require permission to bind a loopback server. `cargo-deny` needs a writable advisory-database lock.

## Limits

- The body editor uses upstream ratatui-textarea. Whole-document screen-map work
  can dominate large mixed-content notes; the plaintext persistence benchmark
  measures a different workload.
- Some view activation remains synchronous. Check cold and network filesystems
  separately from warm synthetic fixtures.
- Canvas image queues and protocol encoding belong to Pinstar. Canvas polls
  while image support is present because the dependency exposes no pending-work
  query.
- Cancellation cannot interrupt an active filesystem syscall or codec operation.
  It prevents stale installation and cancels queued/result-delivery work.
- TestBackend does not validate real terminal graphics, input, or platform
  integration. Follow [release acceptance](RELEASE_CHECKLIST.md) for those checks.
