<div align="center">
<img width="512" height="512" alt="clin logo" src="https://github.com/user-attachments/assets/80248532-f055-4b8e-beda-1a3eaafbd0ba" />
</div>

# **clin - Your notes. Encrypted. Instant. Private. Simple.**  

# **clin is not a text editor!**

> `clin` is originally a app i made when i got into C it was really rough and basic, i decided to remake it in Rust with more features and improved user experience to better fit your workflow!

---

## Highlights
-  **ChaCha20-Poly1305** encryption
-  Binary `.clin` files
-  Full-screen TUI with list + editor + help views  
-  Mouse support + bracketed paste  
-  Optional **Vim mode** (persistent ON/OFF)  
-  **Continuous Auto-save** (with panic crash safety logic)

## Future Plans
### Configuration & Customization

* [ ] **Smart folders:** Automatic movement of specific tagged notes to specific folders.
* [ ] **Custom storage path:** Change where the encrypted vault lives.
* [ ] **User-defined templates:** Boilerplate for new notes.
* [ ] **Custom keybinds:** Remap controls to fit the user's workflow.
* [ ] **Git integration:** Automate backups and versioning via Git.

---

### Editor Enhancements

* [ ] **Contextual Cursor:** Changing cursor shape/color based on `NORMAL`, `INSERT`, or `VISUAL` modes.
* [ ] **OCR Paste:** Pasting from screenshots using `ocrs` or `tesseract`.
* [ ] **Word & Character metrics:** Real-time word counts and progress goals.
* [ ] **Status line customization:** Flexible `status_fornat = "{title} | {word_count} words | {encryption_status}" )`
* [ ] **External editor support:** Opening notes in `nvim`, `helix`, etc.
* [ ] **Improved mouse support:** Right-click context menus within the editor.

---

### Note Management & Navigation

* [ ] **Folders & Tags:** Hierarchical and metadata-based organization.
* [ ] **Mouse support:** Navigation and selection within the notes list.
* [ ] **Enhanced UI:** Sorting options, line numbers, and a preview pane.
* [ ] **Fast Search:** Immediate note discovery using the `fd` utility.
* [ ] **Asset management:** Icon rendering and assigning icons to specific notes.
* [ ] **Data portability:** Easy backup/restore and text file importing.

---

### Integration & CLI Usage

* [ ] **Pre-piping:** Routing notes through external tools for custom rendering.
* [ ] **Markdown integration:** Using `glow`.
* [ ] **Expanded CLI:** More argument options for command-line interactions.

---

### Experimental & Advanced

* [ ] **Lua Scripting:** Allowing users to write scripts to extend app functionality.
* [ ] **Steganography:** Hiding encrypted vaults inside other file types (e.g., images) (completely unnecessary!).
---

# Mouse support
![mouse](https://github.com/user-attachments/assets/8df42fc2-04f5-4f42-9e23-36bf6f5414d1)

# Vim mode (optional)
![vim](https://github.com/user-attachments/assets/dfa802bc-b2dd-4351-8a4b-0cea1f03ca0a)

- Vim mode can be toggled with a selectable Vim button in both notes and editor views.
- Vim mode toggle state persists across app restarts.
- Dynamic Vim indicator dynamically displaying active state (`[ Vim: NORMAL ]`, `[ Vim: INSERT ]`, `[ Vim: VISUAL ]`, etc.) integrated directly into the footer.
- Uses a **fully custom native state-machine** for handling Vim inputs directly within `tui-textarea` (replacing the previous `vim-line` crate for better stability).
- Modes: `NORMAL`, `INSERT`, `VISUAL`, `VISUAL LINE`, operator-pending (`d...`/`c...`/`y...`), and command (`:...`).
- Visual line mode compatibility: `V` enters linewise selection, `j`/`k` extends, `y`/`d`/`c`/`x`/`p` applies.
- Motions/navigation: `h`, `j`, `k`, `l`, `w`, `e`, `b`, `0`, `^`, `$`.
- Vertical jumps & scrolling: `gg`, `G`, `<NUMBER>g`, `<NUMBER>G`, `PageDown`, `PageUp`, `Ctrl+d` (20 lines down), `Ctrl+u` (20 lines up).
- Operator motions: `d`, `y`, `c` with combinations like `dw`, `cw`, `yw`, plus `dd`, `yy`, `cc`.
- Inner object compatibility operators:
  - Change inner: ``` ciw, ci(, ci[, ci{, ci<, ci", ci', ci` ```
  - Delete inner: ``` diw, di(, di[, di{, di<, di", di`, di` ```
  - Yank inner: ``` yiw, yi(, yi[, yi{, yi<, yi", yi`, yi` ```
- Additional Vim edit actions: `x`, `p`, `u` (undo), `Ctrl+r` (redo).
- Command mode integration: `:q` (quit) notes continually autosave.

# CLI flags
![arguments](https://github.com/user-attachments/assets/b6fb344a-79ef-47ae-aefb-8ae637f939d8)

---

## Storage
Notes are encrypted before hitting disk.  
Key is stored locally (`key.bin`).  
**Backup & Restore** -> copy both `.clin` files + `key.bin` from `~/.local/share/clin`

---

## Installation

### Debian/Ubuntu (.deb)
Download the latest `.deb` from the [Releases](https://github.com/reekta/clin/releases) page.
```bash
sudo dpkg -i clin_0.1.1-1_amd64.deb
```

### Fedora/RHEL (.rpm)
Download the latest `.rpm` from the [Releases](https://github.com/reekta/clin/releases) page.
```bash
sudo rpm -i clin-0.1.1-1.x86_64.rpm
```

### Arch Linux (PKGBUILD)
A `PKGBUILD` is included in the root of the repository.
```bash
git clone https://github.com/reekta/clin.git
cd clin
makepkg -si
```

### From Source (Cargo)
```bash
# Install Rust
curl https://sh.rustup.rs -sSf | sh

# Build & run
cargo run

# Install globally
cargo install --path .
```
## CLI Commands

```
clin                   → Launch TUI
clin -q <text> [title] → Quick note & exit
clin -n [title]        → New note
clin -l                → List notes
clin -e <title>        → Edit note
clin -f                → Show storage folder

```

## Controls
**Notes list** ->
`UP/DOWN` select • `Enter` open • `d` delete • `f` folder • `Tab` focus • `?` help • `q` quit

**Editor** ->
`Tab` cycle • `Esc` back • `Ctrl+Q` quit (autosaving always active)
_Vim mode_ toggleable + persistent
