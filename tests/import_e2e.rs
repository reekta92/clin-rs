#[test]
fn url_import_process_entry() -> anyhow::Result<()> {
    let Ok(case) = std::env::var("CLIN_IMPORT_E2E_CASE") else {
        return Ok(());
    };
    let result = clin::actions::import::convert_url(&std::env::var("CLIN_IMPORT_E2E_URL")?);
    if case == "success" {
        let (title, body) = result?;
        assert_eq!(title, "Résumé");
        assert_eq!(body, "converted body");
    } else {
        assert!(
            result
                .expect_err("HTTP errors must reject the import")
                .to_string()
                .contains("500")
        );
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn url_import_downloads_once_converts_local_bytes_and_checks_http_failures() -> anyhow::Result<()> {
    use std::{
        fs,
        io::{Read, Write},
        net::TcpListener,
        os::unix::fs::PermissionsExt,
        process::Command,
    };
    let root = tempfile::tempdir()?;
    let log = root.path().join("calls");
    let markitdown = root.path().join("markitdown");
    fs::write(
        &markitdown,
        "#!/bin/sh\nprintf m >> \"$CLIN_IMPORT_E2E_LOG\"\ntest -f \"$1\" || exit 3\nprintf 'converted body'\n",
    )?;
    fs::set_permissions(&markitdown, fs::Permissions::from_mode(0o755))?;
    for case in ["success", "failure"] {
        fs::write(&log, "")?;
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let url = format!("http://{}/page.html", listener.local_addr()?);
        let server = std::thread::spawn(move || -> std::io::Result<()> {
            let (mut stream, _) = listener.accept()?;
            stream.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
            let mut request = [0; 4096];
            let count = stream.read(&mut request)?;
            assert!(
                std::str::from_utf8(&request[..count])
                    .expect("HTTP request is UTF-8")
                    .starts_with("GET /page.html ")
            );
            let body = "<!-- é --><title>Résumé</title>";
            let status = if case == "success" {
                "200 OK"
            } else {
                "500 Internal Server Error"
            };
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )?;
            Ok(())
        });
        let output = Command::new(std::env::current_exe()?)
            .args(["--exact", "url_import_process_entry", "--nocapture"])
            .env("PATH", root.path())
            .env("CLIN_IMPORT_E2E_LOG", &log)
            .env("CLIN_IMPORT_E2E_CASE", case)
            .env("CLIN_IMPORT_E2E_URL", url)
            .env("NO_PROXY", "127.0.0.1")
            .env("no_proxy", "127.0.0.1")
            .output()?;
        server.join().expect("HTTP fixture thread panicked")?;
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            fs::read_to_string(&log)?,
            if case == "success" { "m" } else { "" }
        );
    }
    Ok(())
}

#[test]
#[ignore = "requires the local imports Postman simulation and MarkItDown or Pandoc"]
fn url_import_simulation() -> anyhow::Result<()> {
    let base = std::env::var("CLIN_IMPORT_SIMULATION_URL")?;
    for path in ["article.html", "redirect"] {
        let (title, body) = clin::actions::import::convert_url(&format!("{base}/{path}"))?;
        assert_eq!(title, "Clipping fixture");
        assert!(body.contains("ARTICLEMARKER"));
    }
    for (path, message) in [
        ("failed", "500"),
        ("loop", "five redirects"),
        ("oversized", "16777216 bytes"),
        ("chunked", "16777216 bytes"),
    ] {
        let error = clin::actions::import::convert_url(&format!("{base}/{path}"))
            .expect_err("HTTP errors and size limits must reject the import");
        assert!(format!("{error:#}").contains(message), "{path}: {error:#}");
    }
    assert!(clin::actions::import::convert_url(&format!("{base}/unsafe")).is_err());
    Ok(())
}
