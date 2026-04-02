<div align="center">
<img width="512" height="512" alt="clin logo" src="https://github.com/user-attachments/assets/80248532-f055-4b8e-beda-1a3eaafbd0ba" />
</div>

# **clin - Your notes. Encrypted. Instant. Private. Simple.**  

# **clin is not a text editor!**

> `clin` was originally an app I made when I got into C. It was really rough and basic, so I decided to remake it in Rust with more features and an improved user experience to better fit your workflow!

---

## Highlights
-  Unicode glyphs, **requires a Nerd Font**
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
* [x] **Custom keybinds:** Remap controls to fit the user's workflow.
* [ ] **Git integration:** Automate backups and versioning via Git.

---

### Editor Enhancements

* [ ] **Word & Character metrics:** Real-time word counts and progress goals.
* [ ] **Status line customization:** Flexible `status_format = "{title} | {word_count} words | {encryption_status}"`
* [X] **External editor support:** Opening notes in `nvim`, `helix`, etc.
* [X] **Improved mouse support:** Right-click context menus within the editor.

---

### Note Management & Navigation
* [ ] **Batch Tagging:** Improve tag manager popup to batch tag multiple notes.
* [ ] **Better Templates:** Improvements for templates popup with searching, easier template creation, setting up a default template to always use on new notes.
* [x] **Folders & Tags:** Hierarchical and metadata-based organization.
* [x] **Mouse Support:** Navigation and selection within the notes list.
* [X] **Enhanced UI:** Sorting options, pinned notes, confirmation dialogs, and a preview pane.
* [X] **Fast Search:** Immediate note discovery using the `fd` utility. (Implemented without `fd` for now.)
* [X] **Asset management:** Icon rendering and assigning icons to specific notes. **DISCONTINUED Reason: implemented using unicode glyphs, users will be able to config their own glyphs via config.**
* [X] **Data portability:** Easy backup/restore and text file importing by making encryption optional.

---

### Integration & CLI Usage

* [ ] **Pre-piping:** Routing notes through external tools for custom rendering.
* [ ] **Markdown integration:** Markdown preview in preview pane and in editor mode as a seperate *markdown view* readonly mode. Using `glow` or alternatives.
* [x] **Expanded CLI:** More argument options for command-line interactions.

---

### Command Palette
* [X] **Command palette:** Implement command palette for doing special actions(OCR, graph view, back/forward-link checks etc. on notes.
* [X] **OCR Paste:** Insert text from a copied image at clipboard into a note, should support external editors. Using `tesseract`.
* [ ] **PDF to Text:** Insert PDF contents as `.md` format. Using `pdftotext` or `poppler`.
* [ ] **Export as PDF:** Export as formatted PDF. Using `pandoc` or `weasyprint`.
* [ ] **CSV to Table:** Insert CSV contents as `.md` table format.
* [ ] **URL to Article:** Insert article containing URL's as `.md`. Using `ureq` or `html2md`.
* [ ] **Linking Notes:** Link notes with `[[<note_name>]]`.
* [ ] **Backlinks:** Related to **Linking notes**, allow for following backlinks. Using indexing caching.
* [ ] **Forwardlinks:** Related to **Linking notes**, allow for following forward links.
* [ ] **Sub-notes:** Orphan notes, creating virtual sub-notes that are attached to a note without physically existing on the disk. Using **backlinks**.
* [ ] **Graph View:** Visual graph of linked notes, related to **Linking notes**.
* [ ] **Insert Date/Time:** Insert date/time.
* [ ] **Calculator:** Simple calculator, inserts the simple mathematical calculations result.
* [ ] **Calendar Picker:** Calendar UI to pick and insert a specific date.
* [ ] **Date/Time Calculator:** Inserts results of calculations like `now + 2 weeks` or `7 pm + 156 minutes` or `now -t 13.04.2028`(results the amount of days).
* [ ] **Date/Time Linking:** Link the date/time of the created note for **graph view**.
* [ ] **Clipboard History:** Access clipboard items and allow for multi-pasting, formatting before pasting, pasting as plain text, pasting as code etc. **HIGH SECURITY AND PRIVACY CONCERN MIGHT BE DISCONTINUED.**
* [ ] **Merge Notes:** Merge Two or more notes together.
* [ ] **Split Notes:** Split notes according to headlines from `.md` format.
* [ ] **Redact Paste:** Replace wanted sections of a text with ████ while pasting from clipboard.
* [ ] **Tree Outline:** Show the note as a treeview with each root being a headling from `.md` format and notes being branches.
* [ ] **Text Search:** Search for a specific text. Using `ripgrep` or `grep`.
* [ ] **Common Words:** Extract most used words from a note.


### Experimental & Advanced

* [ ] **Lua Scripting:** Allowing users to write scripts to extend app functionality.
* [ ] **Steganography:** Hiding encrypted vaults inside other file types.
* [ ] **History:** Access change history of a note. Using `git2`, `crate`. **Will conflict with auto-save.**

---
# Configuration

clin uses `~/.config/clin/config.toml` for app configuration and `~/.config/clin/keybinds.toml` for custom keybinds.

### App Configuration (`config.toml`)
This file is generated automatically. You can edit it to change your settings:
```toml
storage_path = "/custom/path/to/vault" # Optional: Change where notes are stored
external_editor = "nvim" # Optional: Command to use for the external editor
external_editor_enabled = false
encryption_enabled = true # Whether new notes should be encrypted by default
```

### Custom Keybinds (`keybinds.toml`)
You can fully customize clin's keybindings! To get started, generate the default configuration using the CLI:
```bash
clin --export-keybinds > ~/.config/clin/keybinds.toml
```
Then, edit the file to change bindings for the list, editor, or help menus.

---
# User Defined Templates

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
- When trying to open an unencrypted note (`[UENC]`), the app will require confirmation since it will **overwrite** the original file and encrypt it! This behaviour will be changed to create an encrypted copy of the original file instead, and it will be a config option to customize its behaviour.

### Encryption OFF
- Created notes will be `.md` files and assigned to `[UENC]` tag.
- Encrypted notes will be shown with their `[ENC]` tag and they will be **inaccessible**.

---

## Installation

### Debian/Ubuntu (.deb)
Download the latest `.deb` from the [Releases](https://github.com/reekta/clin/releases) page.
```bash
sudo dpkg -i clin-rs_0.2.1-1_amd64.deb
```

### Fedora/RHEL (.rpm)
Download the latest `.rpm` from the [Releases](https://github.com/reekta/clin/releases) page.
```bash
sudo rpm -i clin-rs-0.2.1-1.x86_64.rpm
```

### Arch Linux (PKGBUILD)
A `PKGBUILD` is included in the root of the repository.
```bash
# Clone the repo
git clone https://github.com/reekta/clin-rs.git
cd clin

# Install
makepkg -si
```
### Other
Download the latest `.tar.gz` from [Releases](https://github.com/reekta/clin/releases) page for manual installation.
```bash
# Extract the archive
tar -xzf clin-rs-0.2.1-x86_64.tar.gz
cd clin-rs-0.2.1-x86_64.tar.gz

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

### With Cargo
```bash
# Install Rust
curl https://sh.rustup.rs -sSf | sh

# Install clin
cargo install clin-rs
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
  --migrate-storage         Migrate data from previous storage location

KEYBINDS:
  --keybinds                Show current keybindings
  --export-keybinds         Export keybinds as TOML
  --reset-keybinds          Reset keybinds to defaults

TEMPLATES:
  --list-templates          List available templates
  --create-example-templates Create example templates

```
