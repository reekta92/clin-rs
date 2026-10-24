# Performance checks and resource ownership

This pass optimizes clin-rs without changing the pinstar, graf, Draw, or textarea dependencies. It preserves visible Graph physics, image filtering, editor save/conflict checks, and recovery flushing.

## Changes

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

## Measurements

Measured on Linux, Ryzen 7 7840HS (16 logical CPUs), approximately 31 GiB RAM, Rust release builds. Fixtures are disposable and synthetic; filesystem caches are warm. These measurements are diagnostic results, not guarantees for other hardware or network filesystems.

The mixed-view benchmark uses 1k/10k/50k notes, a selected 5k-line document, a 500-node/499-edge Canvas, a 500-element drawing, and a 120×40 terminal buffer. Graph uses its normal configured limits. It measures 100 frames per view, 100 alternating navigation keys plus drawing, and the time to enter each view. The four Notes layouts also render a populated folder containing the full note count. Frame timings exclude terminal transport and most background I/O. Bases has a separate 5k-note acceptance benchmark.

At 50k notes, all tested frames and navigation/frame pairs passed the enforced 16 ms p95 gate:

| View | Enter once | Frame p95 | Navigation + frame p95 |
| --- | ---: | ---: | ---: |
| Notes | 0.011 ms | 0.087 ms | 0.125 ms |
| Edit | 2.176 ms | 0.318 ms | See persistence workload below |
| Graph | 41.476 ms | 1.398 ms | 1.671 ms |
| Canvas | 2.861 ms | 0.571 ms | 0.661 ms |
| Draw | 0.685 ms | 0.103 ms | 0.159 ms |
| Outline | 0.641 ms | 0.145 ms | 0.199 ms |
| Help | 0.271 ms | 0.293 ms | 0.301 ms |
| Calendar | 0.035 ms | 0.115 ms | 0.127 ms |
| Setup | 0.021 ms | 0.102 ms | 0.158 ms |

Populated 50k-note Grid/Tree/List/Multi layouts measured 0.131–0.136 ms frame p95. These rendering figures do not include rebuilding the catalog or the visual list.

The ten-minute resource run completed 7,983 full cycles (71,847 view entries). After warmup, RSS went from 42,212 to 42,604 KiB (+392 KiB); threads stayed at 14 and descriptors at 7 (peak 8). A final-release 100-cycle confirmation ended at 42,080 KiB, with 14 threads and 7 descriptors.

The editor benchmark includes input, drawing, and durable recovery/save handling over 400 plain-text edit cycles. Repeated runs measured 0.676–0.798 ms p95 at 5k lines and 9.522–10.973 ms at 100k lines, against pre-change baselines of 0.666 ms and 10.944 ms. This does not establish a reliable keystroke speedup; the main gains here are reduced idle work, bounded background work, and avoiding UI-thread I/O/encoding. The separate mixed-content textarea limit remains below.

Bases acceptance passed: four layouts measured 0.136–0.891 ms frame p95; cold metadata plus first query took 404 ms; full warm queries measured 64 ms p95. Watcher-to-result latency was 333 ms p95 including the existing 250 ms debounce. Retained Bases RSS was 122,768 KiB over the host baseline, within its 128 MiB gate. Reopening two live embeds plateaued around 160 MiB total RSS.

Direct-open PTY checks confirmed Editor, Canvas, Draw, and Bases terminal output. The 1024×768 checkerboard image produced halfblock image glyphs. Their three-second idle samples emitted no further terminal bytes and used 0–1 CPU ticks (0–0.33% of one core at this sampling resolution). The PTY keyboard driver did not reliably navigate to the other views; those interactions are covered by the in-process event tests and benchmarks, not claimed as native-terminal validation.

Verification passed: 778 unit tests, 20 integration tests, formatting, Clippy with warnings denied, dependency policy, and all-target Rust 1.90 checks. Nine opt-in tests remain ignored in the ordinary suite; the view, editor persistence, Bases acceptance, and resource tests above were run explicitly.

## Reproduction

Use temporary XDG directories so tests cannot mix their state with an installed app:

```sh
export XDG_DATA_HOME=/tmp/clin-performance-data
export XDG_STATE_HOME=/tmp/clin-performance-state
cargo test --locked --offline -- --test-threads=4
cargo clippy --locked --offline --all-targets -- -D warnings
scripts/check.sh fmt deny msrv

CLIN_PERF_ENFORCE=1 cargo test --locked --offline --release --lib \
  view_perf_tests::mixed_vault_view_performance -- --ignored --nocapture --test-threads=1
cargo test --locked --offline --release --lib \
  editor_session::safety_tests::full_editor_persistence_perf -- --ignored --nocapture --test-threads=1
cargo test --locked --offline --release --lib \
  bases::workflow_tests::acceptance::native_5000_note_acceptance -- --ignored --nocapture --test-threads=1
cargo test --locked --offline --release --lib \
  view_perf_tests::repeated_views_resource_soak -- --ignored --nocapture --test-threads=1
```

The Linux soak defaults to ten minutes and at least 100 complete cycles through nine views. It uses a 1k-note vault, images, Markdown jobs, Graph workers, and an actual session image worker/watcher. It samples `/proc/self` and checks retained RSS growth against 128 MiB, with at most four additional threads/descriptors after warmup. `CLIN_PERF_SOAK_SECONDS` can shorten local diagnostic runs; the full acceptance run uses the default. Run it alone. Run timing benchmarks after compilation finishes.

The ordinary regression suite also checks idle rendering (at most three frames in 650 ms), saturated image queues, cancellation/resume, stale image results, image-owner routing, Calendar snapshot consistency, catalog selection, and save/recovery behavior. Some image tests require permission to bind a loopback server. `cargo-deny` needs a writable advisory-database lock.

## Remaining limits

- The 100k-line mixed-content textarea edit-pair baseline remains approximately 37–38 ms p95. The raw upstream textarea has the same behavior; its whole-document screen-map rebuild is outside this clin-rs-only change. The plain-text persistence benchmark below exercises a different workload.
- View activation remains synchronous where measured local openings fit the 50 ms budget. Cold/network-filesystem activation is unmeasured.
- Canvas image queues and native Canvas protocol encoding remain owned by pinstar. clin-rs removes unconditional Canvas redraws and repeated host checks; upstream cache/queue behavior is unchanged. Canvas still polls quickly when image support is present because the dependency exposes no pending-work query.
- Cancellation cannot interrupt a filesystem syscall or a decoder already inside a native operation. It prevents stale installation and cancels queued/result-delivery work.
- Sixel/iTerm and macOS/Windows native terminal rendering were not validated on this Linux host. Automated tests cover Kitty first-transmission behavior and halfblock image encoding. A GUI Kitty launch did not expose its requested control socket, so no GUI visual claim is made.
