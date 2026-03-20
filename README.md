<<<<<<< HEAD
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

Arch Linux package metadata is configured in `arch/PKGBUILD`.
For AUR workflows, `.SRCINFO` is tracked at `arch/.SRCINFO` and can be refreshed with:

```bash
cd arch && makepkg --printsrcinfo > .SRCINFO
```

Build `.pkg.tar.zst` package:

```bash
./scripts/package-arch.sh
```

This produces:

- `target/arch/clin-0.1.0-1-x86_64.pkg.tar.zst`

Install on Arch Linux:

```bash
sudo pacman -U target/arch/clin-0.1.0-1-x86_64.pkg.tar.zst
```

## Project Files

- `src/main.rs`: application implementation
- `features.md`: feature list
- `scripts/package-deb.sh`: Debian packaging helper
- `scripts/package-rpm.sh`: RPM packaging helper
- `progress.md`: implementation progress and validation summary
=======
<div align="center">
<img width="512" height="512" alt="clin logo" src="https://github.com/user-attachments/assets/80248532-f055-4b8e-beda-1a3eaafbd0ba" />
</div>

# **clin - Your notes. Encrypted. Instant. Private. Simple.**  

> `clin` is originally a app i made when i got into C it was really rough and basic, i decided to remake it in Rust with more features and improved user experience to better fit your workflow!

---

## ✨ Highlights
- 🔒 **ChaCha20-Poly1305** encryption at rest  
- 📦 Binary `.clin` files (completely unreadable)  
- 🖥️ Full-screen TUI with list + editor + help views  
- 🖱️ Mouse support + bracketed paste  
- ⌨️ Optional **Vim mode** (persistent ON/OFF)  
- ⚡ Ultra-fast CLI flags: `-q -n -e -l -f`

## 🛠️ Future Plans
- [ ] Vim colon commands
- [X] Better vim support for commands like `ci`, `di`, `ci(` etc.
- [ ] Easy backup/restore
- [ ] Text file import
- [ ] Windows release as executable
- [ ] Icon rendering and icon assigning to notes
- [ ] Improved mouse support with right click context menu
- [ ] More keyboard shortcuts for easier accessibility
- [ ] More customization options
- [ ] More argument options
- [ ] Creating folders in notes view

# Mouse support
![mouse](https://github.com/user-attachments/assets/8df42fc2-04f5-4f42-9e23-36bf6f5414d1)

# Vim mode (optional)
![vim](https://github.com/user-attachments/assets/dfa802bc-b2dd-4351-8a4b-0cea1f03ca0a)

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

# Quick actions
![arguments](https://github.com/user-attachments/assets/b6fb344a-79ef-47ae-aefb-8ae637f939d8)

---

## 🔐 Storage & Security
Notes are encrypted before hitting disk.  
Key is stored locally (`key.bin`).  
**Backup & Restore** -> copy both `.clin` files + `key.bin` from `~/.local/share/clin`

---

## 🚀 Quick Start

## For Debian and Fedora based distros refer to the releases page for `.deb` and `.rpm`

- - -

```bash
# Install Rust
curl https://sh.rustup.rs -sSf | sh

# Build & run
cargo run

# Install globally
cargo install --path .

# Run
clin

```
## 📋 CLI Commands

```
clin                   → Launch TUI
clin -q <text> [title] → Quick note & exit
clin -n [title]        → New note
clin -l                → List notes
clin -e <title>        → Edit note
clin -f                → Show storage folder

```

## 🎮 Controls
**Notes list** ->
`UP/DOWN` select • `Enter` open • `d` delete • `f` folder • `Tab` focus • `?` help • `q` quit

**Editor** ->
`Tab` cycle • `Es` save & back • `Ctrl+Q` save & quit
_Vim mode_ toggleable + persistent
>>>>>>> 41ee4543292daab38c988cb9aa124ea81a8c9273
