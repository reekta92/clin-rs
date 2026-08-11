# Setup Wizard

Technical docs for the setup wizard (`ViewMode::Setup`) — a first-run / onboarding screen also reopenable via the command palette.

**Source:** `src/setup.rs` (state), `src/ui/setup.rs` (rendering), `src/events/setup.rs` (input handling)

---

## Overview

`ViewMode::Setup` is a first-run wizard, reopenable from command palette. It
selects vault plus appearance and keybind preset. Vault changes rebootstrap
same process; content is never copied. Use `clin storage migrate` explicitly.

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
| `COL_WIDTH` | 44 | Width of left column |
| `COL_HEIGHT` | 19 | Total centered-column height |
| `OPTION_ROWS` | 6 | Vault plus five cycle-in-place rows |
| `DONE_ROW` | 6 | Done button index |
| `PREVIEW_WIDTH` | 50 | Markdown preview width |

### Rows

| Row | Option | Behavior |
|---|---|---|
| Vault | `[Vault] [Select]` | Native directory picker; text fallback permits absolute paths |
| Theme | `[Theme]` | Built-in and custom themes |
| Background | `[Background]` | Transparent / Solid |
| Hint bar | `[Hint bar]` | Hint bar styles |
| Icons | `[Icons]` | Nerd / Unicode / None |
| Keybinds | `[Keybinds]` | Default / Helix / Vim / Emacs |
---

## Keybindings

| Key | Action |
|---|---|
| `Up` / `Down` | Move focus between rows |
| `Enter` / `Space` on Vault | Select vault |
| Left / Right | Cycle selected non-Vault option |
| `Esc` | Confirm setup exit, or cancel vault modal |
| `F2` | Toggle global QuickKeybinds |

Above Done, wizard always shows: `Remember: press ? for help or F2 for keybinds.`
`?` is display-only during setup; help opens after setup.

### Vault behavior

Picker cancellation leaves current path unchanged. Fallback validates absolute
paths, expands `~` and environment variables, preserves symlinks, and creates
missing directories only on Finish. Non-empty directories without `.clin`
require confirmation. `--vault` shows `[CLI override]`, disables Vault row,
and is never persisted to config.

---

## Connections

- [THEME_SYSTEM.md](THEME_SYSTEM.md) — theme options cycled in the Theme row
- [KEYBIND_PRESETS.md](KEYBIND_PRESETS.md) — keybind presets cycled in the Keybind Preset row
- [CONFIG_REFERENCE.md](CONFIG_REFERENCE.md) — all config.toml options affected by the wizard
- [ARCHITECTURE.md](ARCHITECTURE.md) — how Setup integrates into the view state machine and event loop
