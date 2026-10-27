# CLI commands

Run `clin --help` for top-level options or `clin notes new --help` for command-specific options. Use global `--vault <PATH>` to target a particular vault.

## Command reference

```
clin                              Launch interactive TUI (default)
clin notes list                   List note titles
clin notes new [OPTIONS] [TITLE]   Create a note (--body, --stdin, --no-tui, --template, --folder)
clin notes open <TITLE>           Open a note by title
clin notes cat <TITLE>            Print a note's body to stdout
clin notes quick <CONTENT> [TITLE] Quick note, then exit
clin notes search <QUERY>         Search notes by title and content

clin storage show                 Show storage path
clin storage set <PATH>           Set a custom (absolute) storage path
clin storage reset                Reset to default storage path
clin storage migrate              Migrate data from a previous location
clin storage migrate --legacy-layout Convert the old notes/ + templates/ layout

clin keybinds show                Show keybindings
clin keybinds export              Export keybinds as TOML
clin keybinds reset               Reset keybinds to defaults

clin cache reset                  Delete current vault's note-summary cache

clin templates list               List templates
clin templates init               Create example templates

clin config show                  Print the config file path
clin config edit                  Open config in $VISUAL or $EDITOR
clin config reset                 Reset config to default values

clin --setup                      Force first-run setup wizard
clin --version                    Print version
clin --config <PATH>              Override config file (global)
clin --vault <VAULT>              Override the storage/vault path (global)
clin --help                       Show help
```

## Notes from shell input

Create a note from shell input without opening the TUI:

```sh
printf 'Meeting notes\n' | clin notes new --stdin 'Meeting'
clin notes new 'Imported' < notes.txt
```

Input must be UTF-8 and at most 16 MiB. `--stdin` conflicts with `--body`.
Explicit empty stdin creates an empty body; implicitly detected empty stdin
keeps template content. See [Editor](EDITOR.md#clipboard-transport) for
native clipboard, SSH/OSC 52, tmux, and app-local fallback behavior.

## Templates

Initialize examples with `clin templates init`, then use `clin notes new --template "Meeting Notes" --no-tui "Weekly Standup"`. CLI templates render built-in and folder-local variables; see [Templates](TEMPLATES.md#cli-rendering-and-precedence) for title/body precedence and destination folders.

## Configuration and shortcuts

Use `clin config show` to locate the active configuration file. Preset keybindings live at `<config-dir>/keybinds/<preset>.toml`. See the [configuration reference](CONFIG_REFERENCE.md), [keybind presets](KEYBIND_PRESETS.md), and [upgrade guide](UPGRADING.md) before changing or migrating an existing vault.
