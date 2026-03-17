<div align="center">
<img width="512" height="512" alt="clin logo" src="https://github.com/user-attachments/assets/80248532-f055-4b8e-beda-1a3eaafbd0ba" />
</div>

# **clin**  
**clin — Your notes. Encrypted. Instant. Private. Simple.**  
*Built in Rust • From C prototype to production-ready TUI*

---

## ✨ Highlights
- 🔒 **ChaCha20-Poly1305** encryption at rest  
- 📦 Binary `.clin` files (completely unreadable)  
- 🖥️ Full-screen TUI with list + editor + help views  
- 🖱️ Mouse support + bracketed paste  
- ⌨️ Optional **Vim mode** (persistent ON/OFF)  
- ⚡ Ultra-fast CLI flags: `-q -n -e -l -f`

---

## 🔐 Storage & Security
Notes are encrypted before hitting disk.  
Key stored locally (`key.bin`).  
No cloud. No network.  
**Backup & Restore** -> copy both `.clin` files + `key.bin` from `~/.local/share/clin`

---

## 🚀 Quick Start
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
- Full Vim commands
- Easy backup/restore
- Text file import
- Windows support
- App store releases
- Icons & context menu
