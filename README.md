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
-  **Folders & Tags**  
-  **Continuous Auto-save** (with panic crash safety logic)

## Future Plans
### Configuration & Customization

* [ ] **Smart folders:** Automatic movement of specific tagged notes to specific folders.
* [x] **Custom storage path:** Change where the encrypted vault lives.
* [x] **User-defined templates:** Boilerplate for new notes.
* [ ] **Custom keybinds:** Remap controls to fit the user's workflow.
* [ ] **Git integration:** Automate backups and versioning via Git.

---

### Editor Enhancements

* [ ] **Contextual Cursor:** Changing cursor shape/color based on modes.
* [ ] **OCR Paste:** Pasting from screenshots using `ocrs` or `tesseract`.
* [ ] **Word & Character metrics:** Real-time word counts and progress goals.
* [ ] **Status line customization:** Flexible `status_fornat = "{title} | {word_count} words | {encryption_status}" )`
* [ ] **External editor support:** Opening notes in `nvim`, `helix`, etc.
* [ ] **Improved mouse support:** Right-click context menus within the editor.

---

### Note Management & Navigation

* [x] **Folders & Tags:** Hierarchical and metadata-based organization.
* [x] **Mouse support:** Navigation and selection within the notes list.
* [ ] **Enhanced UI:** Sorting options, line numbers, and a preview pane.
* [ ] **Fast Search:** Immediate note discovery using the `fd` utility.
* [ ] **Asset management:** Icon rendering and assigning icons to specific notes.
* [ ] **Data portability:** Easy backup/restore and text file importing.

---

### Integration & CLI Usage

* [ ] **Pre-piping:** Routing notes through external tools for custom rendering.
* [ ] **Markdown integration:** Using `glow`.
* [x] **Expanded CLI:** More argument options for command-line interactions.

---

### Experimental & Advanced

* [ ] **Lua Scripting:** Allowing users to write scripts to extend app functionality.
* [ ] **Steganography:** Hiding encrypted vaults inside other file types.

---

# Mouse support
![mouse](https://github.com/user-attachments/assets/8df42fc2-04f5-4f42-9e23-36bf6f5414d1)

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
clin                        -> Launch TUI
clin -q <text> [title]      -> Quick note & exit
clin -n [title]             -> New note
clin -n --template <name>   -> New note from template
clin -l                     -> List notes
clin -e <title>             -> Edit note
clin --storage-path         -> Show storage folder
clin --set-storage-path <p> -> Set storage folder
clin --keybinds             -> Show current keybinds
clin --list-templates       -> List all templates

```

## Controls
**Notes list** ->
`UP/DOWN` select | `Enter` open | `d` delete item | `n` create folder | `m` move item | `.` tags | `Tab` focus | `?` help | `q` quit

**Editor** ->
`Tab` cycle | `Esc` back | `Ctrl+Q` quit (autosaving always active)
