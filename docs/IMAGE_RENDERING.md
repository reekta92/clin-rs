# Image Rendering

## Overview

Native pixel image rendering via the `ratatui_image` crate. The protocol is auto-detected at startup from sixel, kitty, iTerm, or halfblocks (picker initialized during application startup).

**Source:** `src/image_render/` (`mod.rs`, `cache.rs`, `worker.rs`)

## Architecture

The image rendering pipeline consists of three layers:

- **Cache key** — image `PathBuf`; modification time is not part of the key. Replacing an image at the same path can leave cached pixels until the entry is evicted or the view cache is recreated.
- **ImageCache** (`cache.rs`) — per-view LRU with `request`, `install_decoded`, and `get_proto`. Entries start pending, then receive a terminal-protocol renderer; capacity is at least one entry.
- **Background worker** (`worker.rs::spawn()`) — consumes `ImageJob { key, max_dim }`, decodes/downscales the image, and returns `Result<DecodedImage>`. It drains queued jobs after each decode; the worker has no 150 ms debounce. Call sites set decode bounds; there is no `[image] max_dimension` config key.

## Integration Points

| Location | Usage |
|---|---|
| `src/ui/edit_view.rs` | Markdown editor preview images |
| `src/ui/list_view.rs` | Notes list previews, including image files |
| upstream `pinstar` crate (`images` feature), `src/pinstar_adapter.rs` | Canvas image file nodes |
| `src/app/loading.rs` | Install decoded images into active view caches |
| `src/app/notes.rs`, `src/app/views.rs` | Per-view cache initialization |

Draw v2 stores strokes, shapes, and text only. Legacy Draw image records are dropped during migration; Draw is not an image-rendering integration.

## Configuration

Use `[features] images` for the master toggle (default `true`, restart required). The `[image]` section configures rendering and attachment storage:

| Option | Type | Default | Description |
|---|---|---|---|
| `cache_size` | usize | `32` | LRU cache entry count |
| `preview_rows` | u8 | `8` | Rows occupied by preview images |
| `attachments_subdir` | String | `"attachments"` | Subdirectory for pasted/imported image attachments |

Example:

```toml
[features]
images = true

[image]
cache_size = 32
preview_rows = 8
attachments_subdir = "attachments"
```

## Fallbacks

When no pixel protocol is available, the picker can use halfblocks. Without an initialized picker, while decoding, or when `[features] images = false`, views use textual/placeholder fallbacks. Legacy `[image] enabled` is migrated and then ignored.

## Connections

- [CANVAS.md](CANVAS.md) — image nodes on canvas
- [DRAW.md](DRAW.md) — Draw v2 schema and legacy image migration
- [CONFIG_REFERENCE.md](CONFIG_REFERENCE.md) — full configuration reference
