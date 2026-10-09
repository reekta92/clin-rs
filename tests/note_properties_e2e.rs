use std::path::Path;
use std::process::{Command, Output};

fn cli(root: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_clin"))
        .args(["--config"])
        .arg(root.join("config.toml"))
        .arg("--vault")
        .arg(root.join("vault"))
        .args(arguments)
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env("XDG_CACHE_HOME", root.join("cache"))
        .output()
        .expect("run clin")
}

fn success(root: &Path, arguments: &[&str]) -> String {
    let output = cli(root, arguments);
    assert!(
        output.status.success(),
        "{arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("UTF-8 CLI output")
}

fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("fixture");
    std::fs::create_dir_all(root.path().join("vault/.clin/templates")).expect("vault");
    std::fs::write(
        root.path().join("config.toml"),
        "[features]\nbackup = false\n",
    )
    .expect("config");
    root
}

#[test]
fn cold_cli_queries_and_mutations_preserve_types_and_bytes() {
    let root = fixture();
    let root = root.path();
    let source = "---\r\ntitle: Probe\r\ntags: [research]\r\nflag: true # retain\r\ntext: 'true'\r\nnullable: null\r\nempty: []\r\ncode: '001'\r\n---\r\nBody with [[Target]].\r\n";
    let path = root.join("vault/probe.md");
    std::fs::write(&path, source).expect("note");
    std::fs::write(root.join("vault/plain.txt"), "Plain body.\n").expect("plain note");
    assert!(success(root, &["notes", "list"]).contains("Probe"));
    assert!(success(root, &["notes", "cat", "probe"]).contains("Body with [[Target]]."));
    for query in [
        "prop:flag=true",
        "prop:text=\"true\"",
        "has:nullable null:nullable empty:empty missing:absent",
    ] {
        assert_eq!(success(root, &["notes", "search", query]).trim(), "Probe");
    }
    for query in ["prop:flag=\"true\"", "null:absent", "prop:absent!=false"] {
        assert!(!success(root, &["notes", "search", query]).contains("Probe"));
    }
    success(
        root,
        &["notes", "properties", "unset", "plain.txt", "absent"],
    );
    assert_eq!(
        std::fs::read(root.join("vault/plain.txt")).expect("plain"),
        b"Plain body.\n"
    );
    let before = std::fs::read(&path).expect("before");
    for arguments in [
        vec!["notes", "properties", "set", "probe.md", "title", "Changed"],
        vec![
            "notes",
            "properties",
            "set",
            "probe.md",
            "due",
            "2026-02-30",
            "--type",
            "date",
        ],
        vec![
            "notes",
            "properties",
            "set",
            "../outside.md",
            "flag",
            "false",
            "--type",
            "boolean",
        ],
    ] {
        assert!(!cli(root, &arguments).status.success());
        assert_eq!(std::fs::read(&path).expect("unchanged"), before);
    }
    let value = "needs \"review\" >= 3";
    success(
        root,
        &[
            "notes",
            "properties",
            "set",
            "probe.md",
            "review status",
            value,
        ],
    );
    let result = success(
        root,
        &["notes", "properties", "get", "probe.md", "review status"],
    );
    assert_eq!(
        serde_yaml_ng::from_str::<String>(&result).expect("text value"),
        value
    );
    assert_eq!(
        success(
            root,
            &[
                "notes",
                "search",
                "prop:\"review status\"=\"needs \\\"review\\\" >= 3\""
            ]
        )
        .trim(),
        "Probe"
    );
    let updated = std::fs::read_to_string(&path).expect("updated");
    assert!(updated.contains("flag: true # retain\r\n"));
    assert!(updated.contains("code: '001'\r\n"));
    assert!(updated.ends_with("Body with [[Target]].\r\n"));
}

#[test]
fn cli_creation_precedence_and_vault_rename_update_consumers() {
    let root = fixture();
    let root = root.path();
    let definitions = "# retain definitions\n[properties.status]\ntype = 'select'\noptions = ['draft', 'review', 'done']\ndefault = 'draft'\n";
    std::fs::write(root.join("vault/.clin/properties.toml"), definitions).expect("definitions");
    let template = "name = 'Probe template'\n[title]\ntemplate = 'Template {prop:status}'\n[content]\ntemplate = '''---\nstatus: draft # retain header\n---\nBody {prop:status}.\n'''\n[properties]\nstatus = 'review' # retain table\n";
    std::fs::write(root.join("vault/.clin/templates/probe.toml"), template).expect("template");
    let hidden = "---\ntitle: Hidden\nstatus: draft # retain hidden\n---\nHidden body.\n";
    std::fs::write(root.join("vault/.hidden.md"), hidden).expect("hidden note");
    success(
        root,
        &[
            "notes",
            "new",
            "Explicit",
            "--template",
            "probe",
            "--no-tui",
            "--property",
            "status",
            "select",
            "done",
        ],
    );
    assert_eq!(
        success(root, &["notes", "cat", "Explicit"]).trim(),
        "Body done."
    );
    assert_eq!(
        success(root, &["notes", "properties", "get", "Explicit", "status"]).trim(),
        "done"
    );
    success(
        root,
        &[
            "notes",
            "new",
            "Template",
            "--template",
            "probe",
            "--no-tui",
        ],
    );
    assert_eq!(
        success(root, &["notes", "properties", "get", "Template", "status"]).trim(),
        "review"
    );
    success(root, &["notes", "quick", "Quick body.", "Quick"]);
    assert_eq!(
        success(root, &["notes", "properties", "get", "Quick", "status"]).trim(),
        "draft"
    );
    success(
        root,
        &["notes", "properties", "rename", "status", "stage", "--all"],
    );
    assert_eq!(
        std::fs::read_to_string(root.join("vault/.hidden.md")).expect("preview"),
        hidden
    );
    success(
        root,
        &[
            "notes",
            "properties",
            "rename",
            "status",
            "stage",
            "--all",
            "--apply",
        ],
    );
    let hidden = std::fs::read_to_string(root.join("vault/.hidden.md")).expect("renamed hidden");
    assert!(hidden.contains("stage: draft # retain hidden"));
    let renamed = clin::templates::Template::load(&root.join("vault/.clin/templates/probe.toml"))
        .expect("renamed template");
    assert!(renamed.properties.contains_key("stage"));
    assert!(
        renamed
            .content
            .template
            .contains("stage: draft # retain header")
    );
    assert!(renamed.content.template.contains("{prop:stage}"));
    success(
        root,
        &[
            "notes",
            "new",
            "After rename",
            "--template",
            "probe",
            "--no-tui",
        ],
    );
    assert_eq!(
        success(root, &["notes", "cat", "After rename"]).trim(),
        "Body review."
    );
    assert_eq!(
        success(root, &["notes", "properties", "get", "Explicit", "stage"]).trim(),
        "done"
    );
    assert!(
        !cli(root, &["notes", "properties", "get", "Explicit", "status"])
            .status
            .success()
    );
}
