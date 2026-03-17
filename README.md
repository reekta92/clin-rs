# clin

Encrypted terminal note-taking app written in Rust.

`clin` gives you a fast in-terminal notes workflow with local encrypted storage, in-app editing, optional Vim-style editing, mouse support, and CLI shortcuts for quick operations.

## Highlights

- Encrypted notes at rest (ChaCha20-Poly1305)
- Notes stored as unreadable binary `.clin` files
- Full-screen TUI with list, editor, and help views
- In-terminal title/body editing (no external editor)
- Mouse selection and bracketed paste support
- Optional Vim mode with persistent ON/OFF setting
- Quick CLI actions (`-q`, `-n`, `-e`, `-l`, `-f`, `-h`)

## Requirements

- Rust toolchain (stable)
- Linux/macOS terminal (Linux used in this workspace)

Install Rust if needed:

```bash
curl https://sh.rustup.rs -sSf | sh
```

## Build And Run

```bash
cargo run
```

Release build:

```bash
cargo build --release
```

Install locally:

```bash
cargo install --path .
```

Then run:

```bash
clin
```

## CLI Usage

```text
clin                Launch interactive app
clin -q <CONTENT> [TITLE]
                    Create a quick note and exit
clin -n [TITLE]     Create a new note and open it in editor
clin -f             Print notes directory location
clin -l             List note titles
clin -e <TITLE>     Open a specific note title directly in editor
clin -h             Show this help
```

## Interactive Controls

Notes view:

- `Up/Down` move selection
- `Enter` open selected note (or create when selecting New Note)
- `d` / `Delete` request note delete
- `f` open selected note file location in file manager
- `Tab` switch focus (notes list / Vim toggle button)
- `?` or `F1` open help page
- `q` quit

Editor view:

- `Tab` cycle focus (Title -> Body -> Vim toggle)
- `Esc` autosave and return to notes view
- `Ctrl+Q` save and quit app
- Standard editing shortcuts with Vim OFF:
  - `Ctrl+C`, `Ctrl+X`, `Ctrl+V`
  - `Ctrl+Insert`, `Shift+Insert`, `Shift+Delete`
  - `Ctrl+A`, `Ctrl+Z`, `Ctrl+Y`, `Ctrl+Shift+Z`
  - `Ctrl+Backspace`, `Ctrl+Delete`

Vim mode (optional):

- Toggle using the Vim button in list/editor views
- Persistent setting across launches
- Modes: Normal, Insert, Visual, Visual Line, Operator-pending
- Subset motions/operators (`h/j/k/l`, `w/e/b`, `gg/G`, `d/y/c`, `dd/yy/cc`, etc.)

## Storage And Security

- Notes are encrypted before writing to disk.
- Files are stored under the app data directory (use `clin -f` to print notes folder).
- Encryption key is generated locally and stored in app data (`key.bin`).
- No cloud sync or network service is used by default.

## Packaging

Debian package metadata is configured in `Cargo.toml`.

Build `.deb` package:

```bash
./scripts/package-deb.sh
```

Expected output:

- `target/debian/clin_0.1.0-1_amd64.deb`

RPM package metadata is configured in `Cargo.toml` for `cargo-generate-rpm`.

Build `.rpm` package:

```bash
./scripts/package-rpm.sh
```

This produces:

- `target/generate-rpm/clin-0.1.0-1.x86_64.rpm`

## Project Files

- `src/main.rs`: application implementation
- `features.md`: feature list
- `scripts/package-deb.sh`: Debian packaging helper
- `scripts/package-rpm.sh`: RPM packaging helper
- `progress.md`: implementation progress and validation summary
