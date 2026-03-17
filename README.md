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

# Mouse support!
![mouse](https://github.com/user-attachments/assets/8df42fc2-04f5-4f42-9e23-36bf6f5414d1)


# Vim mode! (experimental)
![vim](https://github.com/user-attachments/assets/4e5e3915-6926-4e88-a1e5-6888641fc34d)
> Vim Mode
- Vim mode can be toggled with a selectable Vim button in both notes and editor views.
- Vim mode toggle state persists across app restarts.
- Vim-like modal editing states:
  - Normal
  - Insert
  - Visual
  - Visual Line
  - Operator-pending
- Vim-style motions and operators (implemented subset), including:
  - Navigation (`h`, `j`, `k`, `l`, word/head/end/top/bottom motions)
  - Operators/motions (`d`, `y`, `c`, including linewise `dd`, `yy`, `cc`)
  - Visual actions (`v`, `V`, `y`, `d`, `c`, `x`, `p`)


# Quick actions!
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

- - -

## 🛠️ Future Plans
- Vim colon commands
- Easy backup/restore
- Text file import
- Windows release as executable
- App store releases (for Linux)
- Icon rendering and icon assigning to notes
- Improved mouse support with right click context menu
- More keyboard shortcuts for easier accessibility
- More customization options
