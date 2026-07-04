use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "clin",
    version,
    about = "Feature-packed terminal note management app inspired by Obsidian"
)]
pub struct Cli {
    /// Override the config file location for this run.
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    /// Override the storage/vault path for this run (~ and $VAR expanded).
    #[arg(long, global = true)]
    pub vault: Option<PathBuf>,

    /// Force the first-run setup wizard, even if config already exists.
    #[arg(long)]
    pub setup: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Note operations.
    Notes {
        #[command(subcommand)]
        action: NotesCmd,
    },
    /// Storage / vault path management.
    Storage {
        #[command(subcommand)]
        action: StorageCmd,
    },
    /// Keybind management.
    Keybinds {
        #[command(subcommand)]
        action: KeybindsCmd,
    },
    /// Template management.
    Templates {
        #[command(subcommand)]
        action: TemplatesCmd,
    },
    /// Config management.
    Config {
        #[command(subcommand)]
        action: ConfigCmd,
    },
}

#[derive(Subcommand, Debug)]
pub enum NotesCmd {
    /// List note titles.
    List,
    /// Create a new note and open it in the TUI.
    New {
        /// Create the note from this template.
        #[arg(short, long)]
        template: Option<String>,
        /// Initial body content. When set, the note is created and the TUI is not opened.
        #[arg(long)]
        body: Option<String>,
        /// Create the note and exit without opening the TUI.
        #[arg(long)]
        no_tui: bool,
        /// Optional title for the note.
        title: Option<String>,
    },
    /// Open a note by title in the TUI.
    Open {
        /// Title of the note to open.
        title: String,
    },
    /// Print a note's body to stdout.
    Cat {
        /// Title of the note to print (case-insensitive match).
        title: String,
    },
    /// Create a quick note from content and exit (no TUI).
    Quick {
        /// Body content of the note.
        content: String,
        /// Optional title for the note.
        title: Option<String>,
    },
    /// Search notes by title and content.
    Search {
        /// Query string.
        query: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum StorageCmd {
    /// Show the current storage path.
    Show,
    /// Set a custom (absolute) storage path.
    Set { path: PathBuf },
    /// Reset to the default storage path.
    Reset,
    /// Migrate data from a previous storage location.
    Migrate,
}

#[derive(Subcommand, Debug)]
pub enum KeybindsCmd {
    /// Show current keybindings.
    Show,
    /// Export keybinds as TOML.
    Export,
    /// Reset keybinds to defaults.
    Reset,
}

#[derive(Subcommand, Debug)]
pub enum TemplatesCmd {
    /// List available templates.
    List,
    /// Create example templates.
    Init,
}

#[derive(Subcommand, Debug)]
pub enum ConfigCmd {
    /// Print the config file path.
    Show,
    /// Open the config file in $VISUAL or $EDITOR.
    Edit,
    /// Reset the configuration to default values.
    Reset,
}
