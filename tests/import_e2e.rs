#[test]
fn url_import_process_entry() -> anyhow::Result<()> {
    let Ok(case) = std::env::var("CLIN_IMPORT_E2E_CASE") else {
        return Ok(());
    };
    let result = clin::actions::import::convert_url("https://example.com/page.html");
    if case == "success" {
        let (title, body) = result?;
        assert_eq!(title, "Résumé");
        assert_eq!(body, "converted body");
    } else {
        assert!(
            result
                .expect_err("HTTP errors must reject the import")
                .to_string()
                .contains("HTTP failure")
        );
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn url_import_downloads_once_converts_local_bytes_and_checks_http_failures() -> anyhow::Result<()> {
    use std::{fs, os::unix::fs::PermissionsExt, process::Command};
    let root = tempfile::tempdir()?;
    let log = root.path().join("calls");
    let curl = root.path().join("curl");
    let markitdown = root.path().join("markitdown");
    fs::write(
        &curl,
        "#!/bin/sh\nprintf c >> \"$CLIN_IMPORT_E2E_LOG\"\nprintf '%s' '<!-- é --><title>Résumé</title>'\n",
    )?;
    fs::write(
        &markitdown,
        "#!/bin/sh\nprintf m >> \"$CLIN_IMPORT_E2E_LOG\"\ncase \"$1\" in http*) exit 3;; esac\nprintf 'converted body'\n",
    )?;
    for path in [&curl, &markitdown] {
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    for case in ["success", "failure"] {
        fs::write(&log, "")?;
        if case == "failure" {
            fs::write(
                &curl,
                "#!/bin/sh\nprintf c >> \"$CLIN_IMPORT_E2E_LOG\"\nprintf 'HTTP failure' >&2\nexit 22\n",
            )?;
        }
        let output = Command::new(std::env::current_exe()?)
            .args(["--exact", "url_import_process_entry", "--nocapture"])
            .env("PATH", root.path())
            .env("CLIN_IMPORT_E2E_LOG", &log)
            .env("CLIN_IMPORT_E2E_CASE", case)
            .output()?;
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            fs::read_to_string(&log)?,
            if case == "success" { "cm" } else { "c" }
        );
    }
    Ok(())
}
