use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "clin",
    version,
    about = "Encrypted terminal note-taking app inspired by Obsidian"
)]
pub struct Cli {
    /// Override the config file location for this run.
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

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
        /// Optional title for the note.
        title: Option<String>,
    },
    /// Open a note by title in the TUI.
    Open {
        /// Title of the note to open.
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
    /// Print the effective configuration as TOML.
    Show,
    /// Print the config file path.
    Path,
    /// Open the config file in $VISUAL or $EDITOR.
    Edit,
    /// Reset the configuration to default values.
    Reset,
}
