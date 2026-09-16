# Image Rendering

## Overview

Native pixel image rendering via the `ratatui_image` crate. The protocol is auto-detected at startup from sixel, kitty, iTerm, or halfblocks (picker initialized in `src/lib.rs:785`).

**Source:** `src/image_render/` (`mod.rs`, `cache.rs`, `worker.rs`)

## Architecture

The image rendering pipeline consists of three layers:

- **ImageKey** (`mod.rs`) — composite key of `{ path, mtime }` used for cache lookups and staleness checks.
- **ImageCache** (`cache.rs`) — LRU cache with methods `request`, `install_decoded`, `get_proto`, `evict_stale`. Decoded images are stored by key and evicted when the cache exceeds its configured entry count.
- **Background worker** (`worker.rs::spawn()`) — spawns a thread communicating via `(tx, rx)` channels, processing `ImageJob::Decode` requests and returning `DecodedImage` results. A `TRANSFORM_SETTLE = 150ms` debounce prevents redundant decode requests during rapid view changes.

## Integration Points

| Location | Usage |
|---|---|
| `src/ui/edit_view.rs:382-385` | Editor preview pane |
| `src/ui/list_view.rs:1752-1797` | Notes list preview pane |
| `src/draw/render.rs`, upstream `pinstar` crate (`render.rs`, `images` feature) | Canvas/draw image nodes |
| `src/app/loading.rs:839` | `install_image` helper |
| `src/app/notes.rs:919-921` | View-level `ImageCache` initialization |
| `src/app/views.rs:174` | Per-view `ImageCache` creation with `config.image.cache_size` |

## Configuration

The `[image]` section in `config.toml`:

| Option | Type | Default | Description |
|---|---|---|---|
| `enabled` | bool | `true` | Master toggle for native pixel image rendering |
| `cache_size` | usize | `32` | LRU cache entry count |
| `preview_rows` | u8 | `8` | Rows occupied by preview images |
| `attachments_subdir` | String | `"attachments"` | Subdirectory for pasted/imported image attachments |

Example:

```toml
[image]
enabled = true
cache_size = 32
preview_rows = 8
attachments_subdir = "attachments"
```

## Fallbacks

When the terminal supports no pixel protocol, images fall back to placeholder blocks/icons (the existing behavior). When `enabled = false`, no decode work is scheduled and all images render as placeholders.

## Connections

- [CANVAS.md](CANVAS.md) — image nodes on canvas
- [DRAW.md](DRAW.md) — image rendering in draw view
- [CONFIG_REFERENCE.md](CONFIG_REFERENCE.md) — full configuration reference
