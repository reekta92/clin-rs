<div align="center">
<img width="512" height="512" alt="clin logo" src="https://github.com/user-attachments/assets/80248532-f055-4b8e-beda-1a3eaafbd0ba" />
</div>

# **clin - Your notes. Encrypted. Instant. Private. Simple.**  

# **clin is not a text editor!**

> `clin` is originally a app i made when i got into C it was really rough and basic, i decided to remake it in Rust with more features and improved user experience to better fit your workflow!

---

## Highlights
-  **ChaCha20-Poly1305** encryption (optional)
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
# User Defined Templates
<img width="553" height="565" alt="image" src="https://github.com/user-attachments/assets/aa39bf58-c40d-48a8-86f1-c277d1cf67ab" />

https://github.com/user-attachments/assets/ba827fee-eff2-40e1-97ba-0429410b915e

Template popup can be accessed with the default `t` key.

You can create your own templates in `~/.local/share/clin/templates` folder with `.toml` format to quickly use in your editor!
Template format should simply be:
```toml
name = "<TEMPLATE_NAME>"
[title]
template = "<TEMPLATE_TITLE>"
[content]
template = """
<TEMPLATE_CONTENT>
```

# Folders
https://github.com/user-attachments/assets/820cc746-adce-4f9f-b0c6-793193720333

You can move files in folders using the default `m` key via it's popup!

# Tags
https://github.com/user-attachments/assets/96af8aa9-f37f-4075-af31-4d67d83ccf3b

You can add tags to the files using the default `.` key via it's popup!

# Right-Click Context Menu
https://github.com/user-attachments/assets/eba945d4-d3fd-4c0c-a194-13bdca0cc6c4

In editor mode, right-click context menu has basic features such as `Copy`, `Paste`, `Cut`, `Select All`. For now system clipboard **doesn't** integrate with this system. This will be changed in the future as a config option!

# Optional Encryption Using ChaCha20-Poly1305
https://github.com/user-attachments/assets/5f67e240-87b7-4ac6-996c-ca58a542792b

Encryption can be toggled with selecting it with `Tab` and pressing `Enter` to turn it on/off. 
### Encryption ON
- Created notes will be `.clin` files, encrypted and assigned to `[ENC]` tag(invisible when encryption is on).
- When trying to open a unencrypted note(`[UENC]`) app will require a confirmation since it will **overwrite** the original file and encrypt it! This behaviour will be changed to create a encrypted copy of the original file instead and it will be a config option to customize it's behaviour.

### Encryption OFF
- Created notes will be `.md` files and assigned to `[UENC]` tag.
- Encrypted notes will be shown with their [ENC] tag and they will be **unaccessible**.

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
# Clone the repo
git clone https://github.com/reekta/clin.git
cd clin

# Install
makepkg -si
```
### Other
Download the latest `.tar.gz` from [Releases](https://github.com/reekta/clin/releases) page for manual installation.
```bash
# Extract the archive
tar -xzf clin-0.2.1-2-x86_64.tar.gz
cd clin-0.2.1-2-x86_64.tar.gz

# Give executable permission
chmod +x clin
./clin

# Install
mkdir -p ~/.local/bin # If not exists
mv clin ~/.local/bin/
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
NOTE OPERATIONS:
  clin                        Launch interactive app
  -n [TITLE]                Create a new note and open it
  -n -t, --template <NAME> [TITLE]
                              Create a new note from a template
  -q <CONTENT> [TITLE]      Create a quick note and exit
  -e <TITLE>                Open a specific note by title
  -l                        List note titles
  -h, --help                Show this help message

CONFIGURATION:
  --storage-path            Show current storage path
  --set-storage-path <PATH> Set custom storage path
  --reset-storage-path      Reset to default storage path

KEYBINDS:
  --keybinds                Show current keybindings
  --export-keybinds         Export keybinds as TOML
  --reset-keybinds          Reset keybinds to defaults

TEMPLATES:
  --list-templates          List available templates
  --create-example-templates Create example templates

```

## Controls
**Notes list** ->
`UP/DOWN` select | `Enter` open | `d` delete item | `n` create folder | `m` move item | `.` tags | `Tab` focus | `?` help | `q` quit

**Editor** ->
`Tab` cycle | `Esc` back | `Ctrl+Q` quit (autosaving always active)
