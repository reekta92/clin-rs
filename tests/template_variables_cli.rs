#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Vault {
    dir: tempfile::TempDir,
    root: PathBuf,
}

impl Vault {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("vault");
        fs::create_dir_all(root.join(".clin/templates")).unwrap();
        fs::create_dir_all(dir.path().join("home")).unwrap();
        fs::write(
            root.join(".clin/templates/book.toml"),
            r#"name = "Book"
[title]
template = "{book} Chapter {chapter}"
[content]
template = "{book} / {chapter} / {date} / {missing}"
"#,
        )
        .unwrap();
        fs::write(
            root.join(".clin-template.toml"),
            "[variables]\nbook = 'Rust'\n",
        )
        .unwrap();
        fs::create_dir_all(root.join("books/04")).unwrap();
        fs::write(
            root.join("books/04/.clin-template.toml"),
            "inherit_parent = true\n[variables]\nchapter = '04'\n",
        )
        .unwrap();
        Self { dir, root }
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_clin"))
            .arg("--config")
            .arg(self.dir.path().join("config.toml"))
            .arg("--vault")
            .arg(&self.root)
            .args(["notes", "new", "--no-tui"])
            .args(args)
            .env("HOME", self.dir.path().join("home"))
            .env("USERPROFILE", self.dir.path().join("home"))
            .env("APPDATA", self.dir.path().join("appdata"))
            .env("LOCALAPPDATA", self.dir.path().join("local-appdata"))
            .env("XDG_CACHE_HOME", self.dir.path().join("xdg-cache"))
            .env("XDG_CONFIG_HOME", self.dir.path().join("xdg-config"))
            .env("XDG_DATA_HOME", self.dir.path().join("xdg-data"))
            .env("XDG_STATE_HOME", self.dir.path().join("xdg-state"))
            .output()
            .unwrap()
    }
}

fn notes(path: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    for entry in fs::read_dir(path).unwrap() {
        let path = entry.unwrap().path();
        if path.file_name().unwrap() == ".clin" {
            continue;
        }
        if path.is_dir() {
            result.extend(notes(&path));
        } else if path.extension().is_some_and(|ext| ext == "md") {
            result.push(path);
        }
    }
    result
}

#[test]
fn cli_renders_title_body_and_inherited_variables_in_destination() {
    let vault = Vault::new();
    let output = vault.run(&["-t", "Book", "--folder", "books/04"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let files = notes(&vault.root);
    assert_eq!(files.len(), 1);
    assert!(files[0].starts_with(vault.root.join("books/04")));
    let content = fs::read_to_string(&files[0]).unwrap();
    assert!(content.contains("Rust Chapter 04"), "{content}");
    assert!(content.contains("Rust / 04 / "), "{content}");
    assert!(!content.contains("{date}"));
    assert!(content.contains("{missing}"));
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("Unknown template variables (left literal): missing")
    );
}

#[test]
fn cli_explicit_title_and_body_stay_literal_and_override_template() {
    let vault = Vault::new();
    let output = vault.run(&[
        "-t",
        "Book",
        "--folder",
        "books/04",
        "--body",
        "Literal {book}",
        "Explicit {chapter}",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let files = notes(&vault.root);
    assert_eq!(files.len(), 1);
    let content = fs::read_to_string(&files[0]).unwrap();
    assert!(content.contains("Explicit {chapter}"));
    assert!(content.contains("Literal {book}"));
    assert!(!content.contains("Rust /"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("Unknown template variables"));
}

#[test]
fn cli_invalid_config_and_paths_create_no_partial_note() {
    let vault = Vault::new();
    fs::write(
        vault.root.join("books/04/.clin-template.toml"),
        "[variables]\nchapter = 4",
    )
    .unwrap();
    let output = vault.run(&["-t", "Book", "--folder", "books/04"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains(".clin-template.toml"));
    assert_eq!(notes(&vault.root).len(), 0);
    for folder in ["../outside", "/tmp/outside", "books/../../outside"] {
        let output = vault.run(&["--folder", folder, "Blank"]);
        assert!(!output.status.success(), "{folder}");
        assert_eq!(notes(&vault.root).len(), 0);
    }
    let output = vault.run(&["-t", "Missing", "--folder", "books/04"]);
    assert!(!output.status.success());
    assert_eq!(notes(&vault.root).len(), 0);
}

#[test]
fn cli_blank_notes_keep_default_title_and_create_new_destination() {
    let vault = Vault::new();
    let output = vault.run(&["--folder", "new/subfolder"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let files = notes(&vault.root);
    assert_eq!(files.len(), 1);
    assert!(files[0].starts_with(vault.root.join("new/subfolder")));
    assert!(fs::read_to_string(&files[0]).unwrap().contains("New Note"));
}
