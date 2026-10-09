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
    /// Cached data management.
    Cache {
        #[command(subcommand)]
        action: CacheCmd,
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
        /// Initial typed property; repeat as --property KEY TYPE VALUE.
        #[arg(long = "property", num_args = 3, action = clap::ArgAction::Append, allow_hyphen_values = true)]
        properties: Vec<String>,
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
        /// Initial typed property; repeat as --property KEY TYPE VALUE.
        #[arg(long = "property", num_args = 3, action = clap::ArgAction::Append, allow_hyphen_values = true)]
        properties: Vec<String>,
    },
    /// Search notes by title and content.
    Search {
        /// Query string.
        query: String,
    },
    /// Inspect or mutate custom YAML properties without rewriting note body.
    Properties {
        #[command(subcommand)]
        action: PropertyCmd,
    },
}

#[derive(Subcommand, Debug)]
pub enum PropertyCmd {
    /// Print custom metadata as YAML.
    List { note: String },
    /// Print one property; missing key is an error.
    Get { note: String, key: String },
    /// Set a property (declared type, or string unless --type given).
    Set {
        note: String,
        key: String,
        value: String,
        #[arg(long = "type", value_enum)]
        kind: Option<crate::property_model::PropertyKind>,
    },
    /// Remove one property; other header/body/ciphertext bytes remain intact.
    Unset { note: String, key: String },
    /// Preview a key rename; --apply confirms note and binding changes.
    Rename {
        old: String,
        new: String,
        #[arg(long, conflicts_with = "all", required_unless_present = "all")]
        note: Option<String>,
        #[arg(long)]
        all: bool,
        #[arg(long)]
        apply: bool,
    },
    /// Preview/retry unfinished batch stored in vault; --apply confirms.
    Resume {
        #[arg(long)]
        apply: bool,
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

#[derive(Subcommand, Debug)]
pub enum CacheCmd {
    /// Delete the vault's scoped note-summary cache and legacy cache locations. It rebuilds on next launch.
    Reset,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cache_reset() {
        let cli = Cli::try_parse_from(["clin", "cache", "reset"]).unwrap();
        let command = cli.command.unwrap();
        assert!(
            matches!(
                command,
                Command::Cache {
                    action: CacheCmd::Reset
                }
            ),
            "expected Cache::Reset, got {command:?}"
        );
    }
}
