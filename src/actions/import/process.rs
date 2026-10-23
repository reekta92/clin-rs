use anyhow::{Context, Result, bail, ensure};
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

const STDERR_LIMIT: u64 = 64 * 1024;

#[cfg(unix)]
unsafe extern "C" {
    fn kill(pid: std::ffi::c_int, signal: std::ffi::c_int) -> std::ffi::c_int;
}

struct RunningChild(Child);

pub(super) struct ImportDirectory {
    pub(super) path: PathBuf,
}

impl ImportDirectory {
    pub(super) fn new() -> Result<Self> {
        let path = std::env::temp_dir().join(format!("clin-import-{}", uuid::Uuid::new_v4()));
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            fs::DirBuilder::new().mode(0o700).create(&path)?;
        }
        #[cfg(not(unix))]
        fs::create_dir(&path)?;
        Ok(Self { path })
    }
}

impl Drop for ImportDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

impl Drop for RunningChild {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            // The child owns its process group; cancellation also stops converter subprocesses.
            #[cfg(unix)]
            if let Ok(pid) = std::ffi::c_int::try_from(self.0.id())
                && pid > 1
            {
                // SAFETY: process_group(0) gives this unreaped child its own group.
                let _ = unsafe { kill(-pid, signal_hook::consts::SIGKILL) };
            }
            #[cfg(windows)]
            let _ = Command::new("taskkill")
                .args(["/F", "/T", "/PID", &self.0.id().to_string()])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn output_file(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    Ok(options.open(path)?)
}

fn read_output(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= limit,
        "import command output exceeds safety limit"
    );
    Ok(bytes)
}

pub(super) fn run(
    mut command: Command,
    input: Option<&Path>,
    cancelled: &AtomicBool,
    timeout: Duration,
) -> Result<Output> {
    ensure!(!cancelled.load(Ordering::Relaxed), "Import cancelled");
    let directory = ImportDirectory::new()?;
    let stdout = directory.path.join("stdout");
    let stderr = directory.path.join("stderr");
    command
        .stdin(match input {
            Some(path) => Stdio::from(File::open(path)?),
            None => Stdio::null(),
        })
        .stdout(output_file(&stdout)?)
        .stderr(output_file(&stderr)?);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = RunningChild(command.spawn().context("Failed to start import command")?);
    drop(command);
    let deadline = Instant::now() + timeout;
    let status = loop {
        ensure!(!cancelled.load(Ordering::Relaxed), "Import cancelled");
        if Instant::now() >= deadline {
            bail!("Import command timed out");
        }
        ensure!(
            stdout.metadata()?.len() <= super::MAX_IMPORT_BYTES as u64
                && stderr.metadata()?.len() <= STDERR_LIMIT,
            "import command output exceeds safety limit"
        );
        if let Some(status) = child.0.try_wait()? {
            break status;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    Ok(Output {
        status,
        stdout: read_output(&stdout, super::MAX_IMPORT_BYTES as u64)?,
        stderr: read_output(&stderr, STDERR_LIMIT)?,
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn import_commands_are_bounded_cancellable_and_reaped() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let directory = ImportDirectory::new()?;
        assert_eq!(
            directory.path.metadata()?.permissions().mode() & 0o777,
            0o700
        );
        let scratch = directory.path.clone();
        fs::write(scratch.join("private"), "content")?;
        drop(directory);
        assert!(!scratch.exists());
        let mut command = Command::new("sh");
        command.args(["-c", "printf body; printf warning >&2"]);
        let output = run(
            command,
            None,
            &AtomicBool::new(false),
            Duration::from_secs(2),
        )?;
        assert_eq!(output.stdout, b"body");
        assert_eq!(output.stderr, b"warning");
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 10"]);
        let start = Instant::now();
        assert!(
            run(
                command,
                None,
                &AtomicBool::new(false),
                Duration::from_millis(30)
            )
            .unwrap_err()
            .to_string()
            .contains("timed out")
        );
        assert!(start.elapsed() < Duration::from_secs(2));
        let cancelled = AtomicBool::new(false);
        std::thread::scope(|scope| {
            scope.spawn(|| {
                std::thread::sleep(Duration::from_millis(30));
                cancelled.store(true, Ordering::Relaxed);
            });
            let mut command = Command::new("sh");
            command.args(["-c", "sleep 10"]);
            assert!(
                run(command, None, &cancelled, Duration::from_secs(2))
                    .unwrap_err()
                    .to_string()
                    .contains("cancelled")
            );
        });
        assert!(
            run(
                Command::new("must-not-start"),
                None,
                &AtomicBool::new(true),
                Duration::from_secs(2)
            )
            .unwrap_err()
            .to_string()
            .contains("cancelled")
        );
        let mut command = Command::new("sh");
        command.args(["-c", "head -c 16777217 /dev/zero"]);
        assert!(
            run(
                command,
                None,
                &AtomicBool::new(false),
                Duration::from_secs(2)
            )
            .unwrap_err()
            .to_string()
            .contains("safety limit")
        );
        Ok(())
    }
}
