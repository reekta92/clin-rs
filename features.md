# clin Features

## Notes And Storage
- Encrypted local note storage using ChaCha20-Poly1305.
- Notes are saved as binary `.clin` files and are not readable as plain text in editors.
- Automatic note listing sorted by last update time.
- Create new notes from the notes list.
- Open existing notes directly inside the terminal UI.
- Delete notes from notes view with confirmation (`y`/`Enter` to confirm, `n`/`Esc` to cancel).
- Relative time display for note updates in the notes list.

## Terminal UI
- Full-screen terminal UI using an alternate screen.
- Mouse support in editor mode.
- Focusable controls in notes view (notes list and Vim toggle button).
- Focusable controls in editor view (title, body, Vim toggle button).
- Contextual help/status line.

## Editing And Input
- In-terminal editing for both title and note body.
- Tab-based focus switching.
- Bracketed paste support.
- Mouse selection support in body editor.
- Clipboard-style shortcuts in normal (non-Vim) editing:
  - `Ctrl+C` copy
  - `Ctrl+X` cut
  - `Ctrl+V` paste
  - `Ctrl+Insert` copy
  - `Shift+Insert` paste
  - `Shift+Delete` cut
- Common editing shortcuts:
  - `Ctrl+A` select all
  - `Ctrl+Z` undo
  - `Ctrl+Y` redo
  - `Ctrl+Shift+Z` redo
  - `Ctrl+Backspace` delete previous word
  - `Ctrl+Delete` delete next word

## Save And Exit Behavior
- Auto-save on editor exit (`Esc` from editor view).
- Save-and-quit from editor using `Ctrl+Q`.
- Quit from notes view using `q`.
- Save failure protection: quit/back actions do not proceed if save fails.

## Vim Mode
- Vim mode can be toggled with a selectable Vim button in both notes and editor views.
- Vim mode toggle state persists across app restarts.
- Vim mode indicator (`[ Vim: ON ]` / `[ Vim: OFF ]`) shown in notes and editor help areas.
- Uses `vim-line` as the core Vim key handling engine (not a hand-rolled parser).
- Modes: `NORMAL`, `INSERT`, `VISUAL`, operator-pending (`d...`/`c...`/`y...`), and replace (`r...`).
- Visual line mode compatibility: `V` enters linewise selection, `j`/`k` extends, `y`/`d`/`c`/`x`/`p` applies.
- Motions/navigation: `h`, `j`, `k`, `l`, `w`, `e`, `b`, `0`, `^`, `$`, `%`.
- Operator motions: `d`, `y`, `c` with combinations like `dw`, `cw`, `yw`, plus `dd`, `yy`, `cc`.
- Inner object compatibility operators:
  - Change inner: `ciw`, `ci(`, `ci[`, `ci{`, `ci<`, `ci"`, `ci'`, `ci``
  - Delete inner: `diw`, `di(`, `di[`, `di{`, `di<`, `di"`, `di'`, `di``
  - Yank inner: `yiw`, `yi(`, `yi[`, `yi{`, `yi<`, `yi"`, `yi'`, `yi``
- Additional Vim edit actions: `x`, `D`, `C`, `r<char>`, `p`, `P`.

## Packaging And Installation
- Release build support via Cargo.
- Local install via `cargo install --path .`.
- Debian package support with cargo-deb configuration.
- Packaging helper script at `scripts/package-deb.sh`.
- RPM package support with cargo-generate-rpm configuration.
- Packaging helper script at `scripts/package-rpm.sh`.
