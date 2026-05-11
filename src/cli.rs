use std::path::PathBuf;

pub enum CliCommand {
    Run {
        edit_title: Option<String>,
    },
    NewAndOpen {
        title: Option<String>,
        template: Option<String>,
    },
    QuickNote {
        content: String,
        title: Option<String>,
    },
    ListNoteTitles,
    Help,

    ShowStoragePath,
    SetStoragePath {
        path: PathBuf,
    },
    ResetStoragePath,
    MigrateStorage,

    ShowKeybinds,
    ExportKeybinds,
    ResetKeybinds,

    ListTemplates,
    CreateExampleTemplates,
}
