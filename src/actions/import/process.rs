use anyhow::{Context, Result, bail, ensure};
use std::{
    fs::{File, OpenOptions},
    io::Read,
    path::Path,
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

impl Drop for RunningChild {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            // The child owns its process group; cancellation also stops converter subprocesses.
            #[cfg(unix)]
            if let Ok(pid) = std::ffi::c_int::try_from(self.0.id()) {
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
    cancelled: &AtomicBool,
    timeout: Duration,
) -> Result<Output> {
    ensure!(!cancelled.load(Ordering::Relaxed), "Import cancelled");
    let stdout = crate::fsutil::TempFileGuard::new(crate::fsutil::unique_temp_path("stdout"));
    let stderr = crate::fsutil::TempFileGuard::new(crate::fsutil::unique_temp_path("stderr"));
    command
        .stdin(Stdio::null())
        .stdout(output_file(stdout.path())?)
        .stderr(output_file(stderr.path())?);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = RunningChild(command.spawn().context("Failed to start import command")?);
    let deadline = Instant::now() + timeout;
    let status = loop {
        ensure!(!cancelled.load(Ordering::Relaxed), "Import cancelled");
        if Instant::now() >= deadline {
            bail!("Import command timed out");
        }
        ensure!(
            stdout.path().metadata()?.len() <= super::MAX_IMPORT_BYTES as u64
                && stderr.path().metadata()?.len() <= STDERR_LIMIT,
            "import command output exceeds safety limit"
        );
        if let Some(status) = child.0.try_wait()? {
            break status;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    Ok(Output {
        status,
        stdout: read_output(stdout.path(), super::MAX_IMPORT_BYTES as u64)?,
        stderr: read_output(stderr.path(), STDERR_LIMIT)?,
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn import_commands_are_bounded_cancellable_and_reaped() -> Result<()> {
        let mut command = Command::new("sh");
        command.args(["-c", "printf body; printf warning >&2"]);
        let output = run(command, &AtomicBool::new(false), Duration::from_secs(2))?;
        assert_eq!(output.stdout, b"body");
        assert_eq!(output.stderr, b"warning");
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 10"]);
        let start = Instant::now();
        assert!(
            run(command, &AtomicBool::new(false), Duration::from_millis(30))
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
                run(command, &cancelled, Duration::from_secs(2))
                    .unwrap_err()
                    .to_string()
                    .contains("cancelled")
            );
        });
        assert!(
            run(
                Command::new("must-not-start"),
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
            run(command, &AtomicBool::new(false), Duration::from_secs(2))
                .unwrap_err()
                .to_string()
                .contains("safety limit")
        );
        Ok(())
    }
}
