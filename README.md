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
- 💾 **Continuous Auto-save** (with panic crash safety logic)
- ⚡ Ultra-fast CLI flags: `-q -n -e -l -f`

## 🛠️ Future Plans
- [X] Better vim support for commands like `ci`, `di`, `ci(` etc.
- [ ] Easy backup/restore
- [ ] Text file import
- [ ] Folders, tags
- [ ] Icon rendering and icon assigning to notes
- [ ] Improved mouse support with right click context menu
- [ ] More keyboard shortcuts for easier accessibility
- [ ] More customization options
- [ ] More argument options

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
- Vertical jumps & scrolling: `gg`, `G`, `<NUMBER>gg`, `<NUMBER>G`, `PageDown`, `PageUp`, `Ctrl+d` (half page down), `Ctrl+u` (half page up).
- Operator motions: `d`, `y`, `c` with combinations like `dw`, `cw`, `yw`, plus `dd`, `yy`, `cc`.
- Inner object compatibility operators:
  - Change inner: `ciw`, `ci(`, `ci[`, `ci{`, `ci<`, `ci"`, `ci'`, `ci``
  - Delete inner: `diw`, `di(`, `di[`, `di{`, `di<`, `di"`, `di'`, `di``
  - Yank inner: `yiw`, `yi(`, `yi[`, `yi{`, `yi<`, `yi"`, `yi'`, `yi``
- Additional Vim edit actions: `x`, `p`, `u` (undo), `Ctrl+r` (redo).
- Command mode integration: `:q` (quit) notes continually autosave.

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
`Tab` cycle • `Esc` back • `Ctrl+Q` quit (autosaving always active)
_Vim mode_ toggleable + persistent
