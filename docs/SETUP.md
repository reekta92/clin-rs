# Setup Wizard

Technical docs for the setup wizard (`ViewMode::Setup`) — a first-run / onboarding screen also reopenable via the command palette.

**Source:** `src/setup.rs` (state), `src/ui/setup.rs` (rendering), `src/events/setup.rs` (input handling)

---

## Overview

`ViewMode::Setup` is a single-centered onboarding screen that lets the user configure appearance and keybind preset before diving into the app. It is shown on first launch and can be reopened at any time via the command palette (`OpenSetupWizardAction`, id `setup.open`).

The wizard has no title bar, no status bar, and no preview pane chrome — it is a dedicated full-screen form with a live markdown preview pane.

---

## Layout

The screen is centered both horizontally and vertically. The left column contains the CLIN ASCII logo and option rows; the right column (when space permits) shows a live markdown preview pane.

```
┌─────────────────────────────────────────────┬──────────────────────────────────┐
│             CLIN ASCII logo (6 rows)         │                                  │
│                                              │         Live Markdown            │
│  [Theme]           Tokyo Night     [next]    │         Preview Pane             │
│  [Background]      Transparent     [next]    │                                  │
│  [Hint Bar Style]  Classic         [next]    │  (rendered by built-in           │
│  [Icon Mode]       Nerd            [next]    │   comrak/syntect renderer)       │
│  [Keybind Preset]  Default         [next]    │                                  │
│                                              │                                  │
│        [ Done — Finish Setup ]               │                                  │
└─────────────────────────────────────────────┴──────────────────────────────────┘
```

### Constants (from `src/ui/setup.rs`)

| Constant | Value | Description |
|---|---|---|
| `COL_WIDTH` | 44 | Width of the left column (logo + options) |
| `COL_HEIGHT` | 16 | Total height of the centered column |
| `OPTION_ROWS` | 5 | Number of cycle-in-place option rows |
| `DONE_ROW` | 5 | Index of the Done button (0-indexed from first option) |
| `PREVIEW_WIDTH` | 50 | Width of the markdown preview pane |
| `VALUE_WIDTH` | 18 | Max width of the displayed option value |
| `SETUP_PREVIEW_MD` | *inline string* | Markdown content rendered in the preview pane |

---

## The 5 Option Rows

Each row cycles through values in place when the user presses `CycleNext` / `CyclePrev`. Changing a row calls `App::apply_setup_live()` so the preview updates immediately.

| Row | Option | Cycles Through |
|---|---|---|
| Theme | `[Theme]` | All 19 built-in themes (see `SETUP_THEMES` in `src/setup.rs`) |
| Background | `[Background]` | Transparent / Solid |
| Hint Bar Style | `[Hint Bar Style]` | Classic / Accent / PowerlineSharp / PowerlineRounded / PowerlineSlanted |
| Icon Mode | `[Icon Mode]` | Nerd / Unicode / None |
| Keybind Preset | `[Keybind Preset]` | Default / Helix / Vim / Emacs |

---

## Keybindings

The setup wizard is driven by `SetupAction` (defined in `src/keybinds/types.rs`), with defaults in `src/keybinds/defaults.rs` and the scope `[setup]` in `keybinds.toml`.

| Key | Action |
|---|---|
| `Up` / `Down` | Move focus between rows |
| `CycleNext` / `CyclePrev` | Cycle the focused row's value |
| `Activate` | Select / activate the focused row (Done button) |
| `Finish` / `Esc` | Opens a confirm-exit overlay that either commits all changes to `config.toml` (via `App::finish_setup()`) or aborts |

### Mouse

| Gesture | Action |
|---|---|
| Click a row label | Focus that row |
| Click a row value | Cycle the value |
| Click Done button | Finish setup and apply all changes |

Mouse handling is in `src/events/setup.rs:91`.

---

## Vault Path

The wizard configures appearance and keybind preset only. The storage / vault path is set separately via:
- `clin storage set <PATH>` CLI command, or
- `storage_path` in the `[core]` section of `config.toml`

Both support `~` and `$VAR` / `${VAR}` expansion.

---

## Connections

- [THEME_SYSTEM.md](THEME_SYSTEM.md) — theme options cycled in the Theme row
- [KEYBIND_PRESETS.md](KEYBIND_PRESETS.md) — keybind presets cycled in the Keybind Preset row
- [CONFIG_REFERENCE.md](CONFIG_REFERENCE.md) — all config.toml options affected by the wizard
- [ARCHITECTURE.md](ARCHITECTURE.md) — how Setup integrates into the view state machine and event loop
