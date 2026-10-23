# Image Rendering

## Overview

Native pixel image rendering via the `ratatui_image` crate. The protocol is auto-detected at startup from sixel, kitty, iTerm, or halfblocks (picker initialized during application startup).

**Source:** `src/image_render/{cache.rs,worker.rs}`; module declared in `src/lib.rs`.

## Architecture

The image rendering pipeline consists of three layers:

- **Cache key** — image `PathBuf`; modification time is not part of the key. Replacing an image at the same path can leave cached pixels until the entry is evicted or the view cache is recreated.
- **ImageCache** (`cache.rs`) — per-view LRU with `request`, `install_decoded`, and `get_proto`. Entries start pending, then receive a terminal-protocol renderer; capacity is at least one entry.
- **Background worker** (`worker.rs::spawn()`) — consumes `ImageJob { key, max_dim }`, decodes/downscales the image, and returns `Result<DecodedImage>`. It drains queued jobs after each decode; the worker has no 150 ms debounce. Call sites set decode bounds; there is no `[image] max_dimension` config key.

## Integration Points

List and Editor preview attachments must use public vault-relative paths.
Absolute paths, `..`, private vault files, and symlinks that resolve outside
the public vault are rejected. Attachment imports apply the same rule to
their destination and never overwrite an existing file.

| Location | Usage |
|---|---|
| `src/ui/edit_view.rs` | Markdown editor preview images |
| `src/ui/list_view.rs` | Notes list previews, including image files |
| upstream `pinstar` crate (`images` feature), `src/pinstar_adapter.rs` | Canvas image file nodes |
| `src/app/loading.rs` | Install decoded images into active view caches |
| `src/app/notes.rs`, `src/app/views.rs` | Per-view cache initialization |
| `src/bases/{write.rs,authoring.rs}` | Guarded public image activation and native image popup |

Draw v2 stores strokes, shapes, and text only. Legacy Draw image records are dropped during migration; Draw is not an image-rendering integration.

## Bases public-image activation

Bases can activate public PNG/JPEG/GIF/WebP rows and typed `image()` values.
Resolution stays within the public metadata snapshot; a background operation
then validates the vault path, reads at most 8 MiB, and decodes with a 16 MiB
allocation limit and maximum dimensions of 8192×8192. The result is downscaled
to at most 512 pixels per side and rendered through the existing terminal
picker/protocol. No notes are decrypted. Cards/Kanban also render configured
covers and solid hex colors; cover thumbnails are bounded to 256 pixels and
cached separately (64 entries, two concurrent background jobs).

Configured Bases Back binding closes the image popup without leaving Bases;
a full redraw clears terminal image artifacts. `[features] images = false` or
an unavailable picker produces an explicit diagnostic. Other attachment types
without a native preview also diagnose explicitly rather than entering the
ordinary note-loading path. These activation limits do not change the
normal List/Edit image worker policy.

### Remote Bases images

Configured covers and typed `image()` values may fetch credential-free public
HTTP(S) URLs when images and the terminal picker are enabled. Shared transport:
`src/image_render/remote.rs`, using reqwest/rustls with native trust roots and a
small Tokio runtime inside existing background jobs. No browser or plugin runtime.
No cookies, proxy environment, credentials, or Referer header are forwarded.
Every destination and DNS answer must be publicly routable; redirects are checked
again. Limits: 64 KiB URL, three redirects, eight-second total deadline, 8 MiB body,
10 ms cancellation polling, then the same bounded image decoder.

Requests reveal the authored URL and your public network address to its server.
Disable `[features] images` to prevent image requests. Prefer HTTPS; HTTP provides
no transport integrity. Errors remain explicit native placeholders. Ordinary
List/Edit/Canvas image policy is unchanged.

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
