use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

fn command(root: &std::path::Path) -> Result<Command> {
    let vault = root.join("vault");
    fs::create_dir_all(&vault)?;
    let config = root.join("config.toml");
    fs::write(
        &config,
        format!(
            "[core]\nstorage_path = {}\n",
            serde_json::to_string(&vault)?
        ),
    )?;
    let mut command = Command::new(env!("CARGO_BIN_EXE_clin"));
    command
        .arg("--config")
        .arg(config)
        .env("XDG_CONFIG_HOME", root.join("xdg-config"))
        .env("XDG_DATA_HOME", root.join("xdg-data"))
        .env("XDG_CACHE_HOME", root.join("xdg-cache"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    Ok(command)
}

#[test]
fn piped_body_creates_note_without_entering_tui() -> Result<()> {
    let root = tempfile::tempdir()?;
    let body = "hello from pipe\n世界\n";
    let mut child = command(root.path())?
        .args(["notes", "new", "Pipe title"])
        .spawn()
        .context("start clin with piped body")?;
    child
        .stdin
        .take()
        .context("child stdin must be piped")?
        .write_all(body.as_bytes())?;
    let output = child.wait_with_output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let note = fs::read_dir(root.path().join("vault"))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .find(|path| path.extension().is_some_and(|ext| ext == "md"))
        .context("stdin creation must publish a Markdown note")?;
    assert!(fs::read_to_string(note)?.ends_with(body));
    assert!(!root.path().join("vault/.git").exists());
    Ok(())
}

#[test]
fn invalid_utf8_stdin_fails_before_creating_note() -> Result<()> {
    let root = tempfile::tempdir()?;
    let mut child = command(root.path())?
        .args(["notes", "new", "--stdin", "Bad input"])
        .spawn()
        .context("start clin with invalid input")?;
    child
        .stdin
        .take()
        .context("child stdin must be piped")?
        .write_all(b"invalid\xff")?;
    let output = child.wait_with_output()?;
    assert!(!output.status.success());
    assert_eq!(fs::read_dir(root.path().join("vault"))?.count(), 0);
    Ok(())
}
