<div align="center">
<img width="512" height="512" alt="clin logo" src="https://github.com/user-attachments/assets/80248532-f055-4b8e-beda-1a3eaafbd0ba" />
</div>  

# ****clin is not a text editor!****

> `clin` was originally an app I made when I got into C. It was really rough and basic, so I decided to remake it in Rust with more features and an improved user experience to better fit your workflow!

---

`clin-rs` is a TUI reimagination of Obsidian. It is goal is to provide a feature complete note management tool like Obsidian does but as a TUI rather than a GUI.

---

## Highlights
- **Notes view** with folders, tags, preview pane, filtering, searching and file management(copy, paste, delete, write).
- **Editor view** with built in very simple text editor with mouse support and support for **external editors**.
> - Built in editor is a **placeholder** and it will be reworked in the future.

- **Interactable graph view** via `graf` for markdown files which works with wikilinks or tags.
- **Command palette** for more advanced actions, currently has **OCR** with `tesseract`, encrypt/decrypt and graph view.
> - Some features in the command palette will later be implemented in the reworked editor view for allowing users to use them in **any editor** or any cursor location.

- **Encryption** with ChaCha20-Poly1305, works completely on demand with encrypt/decrypt options.

## Roadmap

### Notes View
- [X] **Folders & tags:** folder management and assigning tags to notes.
- [X] **Searching & sorting & pinning:** search, filter by date, name etc. and pin notes at top.
- [ ] **Text search:** search for strings using `grep` or `ripgrep`.
- [ ] **Smart folders:** automatically move specific tagged notes to their specific folder.
- [ ] **Word & character metrics:** visual meter for how many words written or setup personal goals for word counts for a time period.
- [ ] **Batch tagging:** tag multiple lines at once.
- [ ] **Better template management:** better and easier to use template popup with more features.
- [X] **Markdown preview pane:** render markdown files with `glow` as a preview pane.

### Editor
- [X] **External editor:** allow users to use their own editor instead of the built in one.
- [X] **Improved mouse support:** right click context menu, proper selecting etc.
- [ ] **Rework as side panel:** rework the editor view as a feature list side panel from command palette and the external editor pane.
- [ ] **Cursor insert:** insert at the cursor location for related command palette features like OCR etc. 

### Graph View (graf)
- [ ] **Help page improvements:** unify the view of help page with other parts of the app.
- [ ] **Date/time linking:** link the date/time of the note to the node for categorization.
- [ ] **Create links:** create wikilinks directly from the graph view via a popup. Should also allow for batch creating links.
- [ ] **Assign tags:** assign tags directly from the graph view.
- [ ] **Mouse right click:** right click context menu for actions like creating links, assigning tags etc.

### Canvas
- [ ] **Drawable canvas:** alternative to Obsidian's canvases; drawable, interactable, writable(inserting text) TUI area with it's own file format.
> - [ ] **Obsidian canvas support:** try to import `.canvas` files from Obsidian.
- [ ] **Drawing:** mouse drawing similar to most paint style apps, same logic as the graph view.
- [ ] **Insert shapes:** insert pre defined shapes like rectangles, circles etc.
- [ ] **Link objects:** create links between objects.
- [ ] **Grouping objects:** create groups to merge multiple objects as a one object.
- [ ] **Insert note links:** insert note links into the drawing area as a object.

### Command Palette
- [X] **Command palette:** implement command palette for more advanced actions.
- [X] **OCR paste:** use `tesseract` for OCR processing of clipboard images into the note.
- [ ] **PDF to text/markdown:** import PDF files as text files, preferably markdown files with proper formatting.
- [ ] **Export as PDF:** export the note as a properly formatted PDF file.
- [ ] **CSV to markdown:** import CSV tables as markdown tables.
- [ ] **Import URL content:** import content from the article URL as formatted markdown file.
- [ ] **Linking notes:** create backlinks, forwardlinks between notes via [[note_name]] format.
- [ ] **Sub-notes:** create virtual notes which does not physically exists on the disk rather than store it as a encrypted file and assign it to physical notes.
- [ ] **Insert dynamic variables:** insert realtime values like date/time etc.
- [ ] **Advanced clipboard:** allow for copying/pasting multiple selections at once.
- [ ] **Merge notes:** merge 2 or more notes into one by appending them to before after the target note.
- [ ] **Split notes:** split notes according to their markdown formatting like headers, paragraphs etc.
- [ ] **Redact sections:** redact specific selections like ████.
- [ ] **Common words:** show most used words in the note.

### Configuration
- [X] **Custom storage path:** allow users to set their own notes path.
- [X] **User defined note templates:** allow users to create their own note templates(i.e diary, to do etc.) for quicker note taking.
- [X] **Custom keybinds:** allow users to set their own keybinds.
- [ ] **Status line customization:** allow users to customize the status line via preset variables, i.e `status_format = "{title} | {word_count} words | {encryption_status}"`

### Other
- [X] **Data portability:** make encryption on demand, allow external file import for markdowns.
- [X] **CLI arguments:** allow for creating quick notes, read and write notes, config management, vault path management etc.
- [ ] **Calculator:** basic calculator to insert the result.
- [ ] **Date/time calculator:** a calculator for processing commands like `now + 2 weeks` or `7 pm + 156 minutes` or `now -t 13.04.2028` results the amount of days.
* [ ] **Timezone Converter:** Inserts the converted timezone i.e `UTC+3 -> GMT+3`.
- [ ] **Tree outline:** show a tree structure showing headers as roots and notes/paragraphs as branches.
- [ ] **Git integration:** for versioning the notes vault and backup.
- [ ] **Beautify the UI:** use unicode glyphs, popups and user accessibility features to make the TUI/TUX better.

### Experimental
- [ ] **Pre/post piping notes:** allow for external tool piping onto notes.
- [ ] **AOD pinning:** pin the note as a seperate window to show it over any other window.
- [ ] **Plugin support:** scripting with Lua language.
- [ ] **Steganography:** hide notes as other filetypes, completely for the fun.


---
## Configuration

`~/.config/clin/config.toml` -> main configuration file
`~/.config/clin/keybinds.toml` -> keybind configuration file
`~/.config/clin/graf.toml` -> graph view `graf` configuration file

See the [full configuration reference](docs/CONFIG_REFERENCE.md) for all available options.

### config.toml example

```toml
# Custom vault storage path (default: ~/.local/share/clin)
storage_path = "/path/to/your/vault"

# Previous storage path, used for migration (cleared after successful migration)
previous_storage_path = "/old/vault/path"

# External editor command (e.g. "nvim", "code", "nano")
external_editor = "nvim"
external_editor_enabled = false

# Show the preview pane by default
preview_enabled = true

# Show the markdown preview panel in the editor by default
markdown_preview_enabled = false

# How node labels are displayed in the graph view
# Options: "selected" (default), "neighbors", "all"
graph_label_mode = "selected"
```

### keybinds.toml example

```toml
# List view keybinds
[list]
move_up = ["Up", "k"]
move_down = ["Down", "j"]
open = ["Enter"]
delete = ["d", "Delete"]
quit = ["q"]
help = ["?", "F1"]
open_location = ["f"]
cycle_focus = ["Tab"]
confirm = ["y", "Enter"]
cancel = ["n", "Esc"]
toggle_button = ["Enter", "Space"]
new_from_template = ["t"]
create_folder = ["n"]
create_note = ["a"]
rename_folder = ["r"]
move_note = ["m"]
manage_tags = ["."]
filter_tags = ["/"]
collapse_folder = ["h"]
expand_folder = ["l"]
open_command_palette = ["Ctrl+p", "Shift+Enter"]
open_graph = ["Ctrl+g"]

# QoL bindings
rename = ["r"]
duplicate = ["y"]
toggle_pin = ["p"]
cycle_sort = ["s"]
search = ["Ctrl+f"]
jump_to_top = ["Shift+G"]
page_up = ["Ctrl+u"]
page_down = ["Ctrl+d"]
open_trash = ["Shift+T"]
toggle_preview = ["Shift+P"]

# Editor view keybinds
[edit]
quit = ["Ctrl+q"]
back = ["Esc"]
cycle_focus = ["Tab"]
toggle_button = ["Enter", "Space"]
select_all = ["Ctrl+a"]
copy = ["Ctrl+c", "Ctrl+Insert"]
cut = ["Ctrl+x", "Shift+Delete"]
paste = ["Ctrl+v", "Shift+Insert"]
undo = ["Ctrl+z"]
redo = ["Ctrl+y", "Ctrl+Shift+z"]
delete_word = ["Ctrl+Backspace"]
delete_next_word = ["Ctrl+Delete"]
move_to_top = ["Ctrl+Home"]
move_to_bottom = ["Ctrl+End"]
toggle_markdown_preview = ["Ctrl+p"]

# Help view keybinds
[help]
close = ["Esc", "q", "?", "F1"]
scroll_up = ["Up", "k"]
scroll_down = ["Down", "j"]

# Graph view keybinds
[graph]
quit = ["Esc"]
pan_up = ["Up", "k"]
pan_down = ["Down", "j"]
pan_left = ["Left", "h"]
pan_right = ["Right", "l"]
zoom_in = ["+", "Ctrl+j"]
zoom_out = ["-", "Ctrl+k"]
open_note = ["Enter"]
auto_fit = ["a"]
help = ["?", "F1"]
toggle_search = ["f"]
toggle_minimap = ["Shift+M"]
toggle_legend = ["Shift+L"]
toggle_grid = ["Shift+G"]
toggle_status = ["Shift+S"]
refresh = ["r"]
reload_config = ["Ctrl+r"]
```

### graf.toml example

```toml
[visual]
theme = "onedark"
background = "transparent"
node_color_mode = "folder"
edge_color_mode = "uniform"
label_mode = "selected"
label_max_length = 20
node_size = 2.0
node_size_mode = "fixed"
edge_thickness = 1
show_legend = true
show_grid = false
show_minimap = false
minimap_position = "top_right"
minimap_width = 24
minimap_height = 12
canvas_marker = "braille"
minimap_marker = "half_block"
node_shape = "circle"
label_offset = 4.0
grid_divisions = 10

# Color overrides (uncomment to customize)
# [visual.colors]
# node_color = "#ff6600"
# edge_color = "#445566"
# label_color = "#aabbcc"
# selection_ring_color = "#ff00ff"
# border_color = "#334455"
# title_color = "#66ffcc"
# grid_color = "#222233"
# legend_text_color = "#ccddee"
# status_bar_color = "#556677"
# background_color = "#1a1a2e"

[physics]
ideal_distance = 80.0
damping = 0.95
max_iterations = 800
gravity = 0.01
cooling = true
prevent_overlapping = true
timestep = 0.016
thread_sleep_ms = 16

[interaction]
double_click_ms = 300
zoom_factor = 1.15
drag_sensitivity = 1.0
auto_fit_padding = 1.4
drag_scale = 200.0

[display]
show_status_bar = true
# status_format = "{files} files | {links} links | {selected}"
border_style = "rounded"
border_title = "graf"

[filter]
# exclude_tags = ["draft", "private"]
# exclude_patterns = ["templates/", "private/"]
min_links = 0
max_nodes = 500

[legend]
position = "bottom_right"
max_items = 10

[search]
max_results = 20
max_visible = 10
popup_width = 50
popup_y = 3
cursor_glyph = "▎"

[editor]
# command = "nano"
```

<FEATURES>

## Installation

### Debian/Ubuntu (.deb)
Download the latest `.deb` from the [Releases](https://github.com/reekta/clin/releases) page.
```bash
sudo dpkg -i clin-rs_0.7.0-43-1_amd64.deb
```

### Fedora/RHEL (.rpm)
Download the latest `.rpm` from the [Releases](https://github.com/reekta/clin/releases) page.
```bash
sudo rpm -i clin-rs-0.7.0-43-1.x86_64.rpm
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
tar -xzf clin-rs-0.7.0-43-x86_64.tar.gz
cd clin-rs-0.7.0-43-x86_64.tar.gz

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

---
