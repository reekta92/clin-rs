use crate::actions::Action;
use crate::app::App;
use crate::popups::{ImportSource, ImportTarget};
use anyhow::{Context, Result, bail};
use std::borrow::Cow;
use std::collections::HashSet;
use std::io::Cursor;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

mod article;
mod process;
pub(super) const MAX_IMPORT_BYTES: usize = 16 * 1024 * 1024;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(60);

pub struct ImportAction {
    pub source: ImportSource,
    pub target: ImportTarget,
}

impl Action for ImportAction {
    fn id(&self) -> Cow<'static, str> {
        match (self.source, self.target) {
            (ImportSource::File, ImportTarget::NewNote) => Cow::Borrowed("insert.file_new"),
            (ImportSource::File, ImportTarget::AppendCurrent) => {
                Cow::Borrowed("insert.file_append")
            }
            (ImportSource::Csv, ImportTarget::NewNote) => Cow::Borrowed("insert.csv_new"),
            (ImportSource::Csv, ImportTarget::AppendCurrent) => Cow::Borrowed("insert.csv_append"),
            (ImportSource::Json, ImportTarget::NewNote) => Cow::Borrowed("insert.json_new"),
            (ImportSource::Json, ImportTarget::AppendCurrent) => {
                Cow::Borrowed("insert.json_append")
            }
            (ImportSource::Url, ImportTarget::NewNote) => Cow::Borrowed("insert.url_new"),
            (ImportSource::Url, ImportTarget::AppendCurrent) => Cow::Borrowed("insert.url_append"),
            (ImportSource::Clipboard, ImportTarget::NewNote) => {
                Cow::Borrowed("insert.clipboard_new")
            }
            (ImportSource::Clipboard, ImportTarget::AppendCurrent) => {
                Cow::Borrowed("insert.clipboard_append")
            }
        }
    }

    fn name(&self) -> Cow<'static, str> {
        let name = match self.source {
            ImportSource::File => "File",
            ImportSource::Csv => "CSV/TSV",
            ImportSource::Json => "JSON",
            ImportSource::Url => "URL",
            ImportSource::Clipboard => "Clipboard",
        };
        let action = match self.target {
            ImportTarget::NewNote => "Import",
            ImportTarget::AppendCurrent => "Append",
        };
        let suffix = match self.target {
            ImportTarget::NewNote => "as Note",
            ImportTarget::AppendCurrent => "to Note",
        };
        Cow::Owned(format!("{action} {name} {suffix}"))
    }

    fn description(&self) -> Cow<'static, str> {
        let source_desc = match self.source {
            ImportSource::File => "a file (PDF, DOCX, HTML…)",
            ImportSource::Csv => "a CSV/TSV file",
            ImportSource::Json => "a JSON file",
            ImportSource::Url => "a URL",
            ImportSource::Clipboard => "clipboard text",
        };
        let target_desc = match self.target {
            ImportTarget::NewNote => "create a note",
            ImportTarget::AppendCurrent => "append to the current note",
        };
        Cow::Owned(format!("Convert {source_desc} and {target_desc}"))
    }
    fn category(&self) -> crate::actions::ActionCategory {
        match self.target {
            ImportTarget::NewNote => crate::actions::ActionCategory::Import,
            ImportTarget::AppendCurrent => crate::actions::ActionCategory::Append,
        }
    }

    fn glyph(&self) -> (&'static str, &'static str) {
        match self.source {
            ImportSource::File => crate::ui::icons::FILE.pair(),
            ImportSource::Csv => crate::ui::icons::BASES.pair(),
            ImportSource::Json => crate::ui::icons::CODE.pair(),
            ImportSource::Url => crate::ui::icons::WEB.pair(),
            ImportSource::Clipboard => crate::ui::icons::CLIPBOARD.pair(),
        }
    }

    fn execute(&self, app: &mut App, context_note_id: Option<&str>) -> Result<()> {
        let note_id = match self.target {
            ImportTarget::AppendCurrent => {
                let id = context_note_id
                    .map(|s| s.to_string())
                    .or_else(|| app.get_selected_note_id());
                if id.is_none() {
                    app.set_temporary_status_static("Select a note first");
                    return Ok(());
                }
                id
            }
            ImportTarget::NewNote => None,
        };

        match self.source {
            ImportSource::Clipboard => {
                let (title, md) = clipboard_to_md()?;
                app.insert_content(self.target, note_id.as_deref(), title, md)?;
            }
            ImportSource::Url => {
                let folder = app.get_current_folder_context();
                app.begin_import(self.source, self.target, folder, note_id);
            }
            _ => {
                let (filter_name, filter_ext) = match self.source {
                    ImportSource::File => ("All Files", ".*"),
                    ImportSource::Csv => ("CSV Files", ".csv"),
                    ImportSource::Json => ("JSON Files", ".json"),
                    _ => unreachable!(),
                };

                if let Some(path) = crate::ui::pick_file(filter_name, filter_ext)? {
                    app.apply_import(self.source, self.target, note_id.as_deref(), &path)?;
                }
            }
        }

        Ok(())
    }
}

fn extract_html_title(html: &str) -> Option<String> {
    let mut cursor = 0;
    while let Some(start_bracket) = html[cursor..].find('<') {
        let absolute_start = cursor + start_bracket;
        let rest = &html[absolute_start..];

        if rest
            .as_bytes()
            .get(..6)
            .is_some_and(|s| s.eq_ignore_ascii_case(b"<title"))
        {
            let next_char = rest.as_bytes().get(6);
            if (next_char.is_none()
                || matches!(next_char, Some(b' ' | b'>' | b'\t' | b'\r' | b'\n')))
                && let Some(tag_end_offset) = rest.find('>')
            {
                let after_tag_start = absolute_start + tag_end_offset + 1;
                let after_tag = &html[after_tag_start..];

                let mut close_cursor = 0;
                while let Some(close_bracket_offset) = after_tag[close_cursor..].find('<') {
                    let absolute_close_start = close_cursor + close_bracket_offset;
                    let close_rest = &after_tag[absolute_close_start..];
                    if close_rest
                        .as_bytes()
                        .get(..8)
                        .is_some_and(|s| s.eq_ignore_ascii_case(b"</title>"))
                    {
                        let inner_text = &after_tag[..absolute_close_start];
                        return Some(inner_text.trim().to_string());
                    }
                    close_cursor = absolute_close_start + 1;
                }
                cursor = after_tag_start;
                continue;
            }
        }
        cursor = absolute_start + 1;
    }
    None
}

fn url_fallback_title(url: &str) -> String {
    url.trim_start_matches("http://")
        .trim_start_matches("https://")
        .split('/')
        .next()
        .unwrap_or("Imported URL")
        .to_string()
}
pub fn file_stem_title(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| {
            let mut title = s.replace(['_', '-'], " ");
            if let Some(first) = title.get_mut(0..1) {
                first.make_ascii_uppercase();
            }
            title
        })
        .unwrap_or_else(|| "Imported Note".to_string())
}

fn sanitize_import_content(content: String) -> String {
    if !content
        .chars()
        .any(|ch| ch == '\r' || (ch.is_control() && !matches!(ch, '\n' | '\t')))
    {
        return content;
    }

    let mut sanitized = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                sanitized.push('\n');
            }
            '\n' | '\t' => sanitized.push(ch),
            _ if ch.is_control() => {}
            _ => sanitized.push(ch),
        }
    }
    sanitized
}

fn sanitized(title: String, content: String) -> (String, String) {
    (
        crate::fsutil::sanitize_for_terminal(&title).into_owned(),
        sanitize_import_content(content),
    )
}

pub fn convert_file(path: &str) -> Result<(String, String)> {
    convert_file_cancellable(path, &AtomicBool::new(false))
}

fn convert_file_cancellable(path: &str, cancelled: &AtomicBool) -> Result<(String, String)> {
    let cmd = if crate::fsutil::can_run("markitdown") {
        let mut c = Command::new("markitdown");
        c.arg(path);
        c
    } else if crate::fsutil::can_run("pandoc") {
        let mut c = Command::new("pandoc");
        // If it looks like HTML, tell pandoc explicitly to avoid raw HTML output
        if path.to_lowercase().ends_with(".html") || path.to_lowercase().ends_with(".htm") {
            c.args(["-f", "html", "-t", "gfm", path]);
        } else {
            c.args(["-t", "gfm", path]);
        }
        c
    } else {
        bail!(
            "markitdown or pandoc is required. Install one: pip install markitdown  (or)  install pandoc"
        );
    };

    let output = process::run(cmd, None, cancelled, COMMAND_TIMEOUT)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Conversion failed: {stderr}");
    }

    let md = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if md.is_empty() {
        bail!("Conversion produced no content");
    }

    Ok(sanitized(file_stem_title(path), md))
}

fn detect_delimiter(first_line: &str) -> u8 {
    let comma = first_line.chars().filter(|&c| c == ',').count();
    let semi = first_line.chars().filter(|&c| c == ';').count();
    let tab = first_line.chars().filter(|&c| c == '\t').count();

    if tab >= comma && tab >= semi && tab > 0 {
        b'\t'
    } else if semi >= comma && semi > 0 {
        b';'
    } else {
        b','
    }
}

pub fn convert_csv(path: &str) -> Result<(String, String)> {
    convert_csv_cancellable(path, &AtomicBool::new(false))
}

fn check_import(cancelled: &AtomicBool, bytes: usize) -> Result<()> {
    anyhow::ensure!(!cancelled.load(Ordering::Relaxed), "Import cancelled");
    anyhow::ensure!(
        bytes <= MAX_IMPORT_BYTES,
        "Imported note exceeds 16 MiB safety limit"
    );
    Ok(())
}

fn convert_csv_cancellable(path: &str, cancelled: &AtomicBool) -> Result<(String, String)> {
    check_import(cancelled, 0)?;
    let text = crate::storage::documents::read_bounded_text(Path::new(path), MAX_IMPORT_BYTES)
        .context("Failed to read CSV file")?;
    if text.trim().is_empty() {
        bail!("CSV file is empty");
    }

    let first_line = text.lines().next().unwrap_or("");
    let mut delimiter = detect_delimiter(first_line);
    if path.ends_with(".tsv") || path.ends_with(".tab") {
        delimiter = b'\t';
    }

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .from_reader(Cursor::new(text.as_bytes()));

    let mut md = String::new();
    let header_len;

    if let Some(result) = rdr.records().next() {
        let record = result.context("Failed to read CSV header")?;
        header_len = record.len();
        md.push('|');
        for field in &record {
            md.push(' ');
            md.push_str(&field.replace('|', "\\|").replace('\n', " <br>"));
            md.push_str(" |");
            check_import(cancelled, md.len())?;
        }
        md.push('\n');

        md.push('|');
        for _ in 0..header_len {
            md.push_str(" --- |");
            check_import(cancelled, md.len())?;
        }
        md.push('\n');
    } else {
        bail!("CSV has no rows");
    }

    for result in rdr.records() {
        check_import(cancelled, md.len())?;
        let record = result.context("Failed to read CSV row")?;
        md.push('|');
        for i in 0..header_len {
            md.push(' ');
            if let Some(field) = record.get(i) {
                md.push_str(&field.replace('|', "\\|").replace('\n', " <br>"));
            }
            md.push_str(" |");
            check_import(cancelled, md.len())?;
        }
        md.push('\n');
    }

    check_import(cancelled, md.len())?;
    Ok(sanitized(file_stem_title(path), md))
}

pub fn convert_json(path: &str) -> Result<(String, String)> {
    convert_json_cancellable(path, &AtomicBool::new(false))
}

fn convert_json_cancellable(path: &str, cancelled: &AtomicBool) -> Result<(String, String)> {
    check_import(cancelled, 0)?;
    let text = crate::storage::documents::read_bounded_text(Path::new(path), MAX_IMPORT_BYTES)
        .context("Failed to read JSON file")?;
    let value: serde_json::Value = serde_json::from_str(&text).context("Invalid JSON")?;

    if let Some(arr) = value.as_array()
        && !arr.is_empty()
        && arr.iter().all(|v| v.is_object())
    {
        let mut keys = Vec::new();
        let mut seen = HashSet::new();
        for obj in arr {
            check_import(cancelled, 0)?;
            if let Some(map) = obj.as_object() {
                for key in map.keys() {
                    if seen.insert(key) {
                        keys.push(key.clone());
                    }
                }
            }
        }

        let mut md = String::new();
        md.push('|');
        for key in &keys {
            md.push(' ');
            md.push_str(&key.replace('|', "\\|").replace('\n', " <br>"));
            md.push_str(" |");
            check_import(cancelled, md.len())?;
        }
        md.push('\n');

        md.push('|');
        for _ in 0..keys.len() {
            md.push_str(" --- |");
            check_import(cancelled, md.len())?;
        }
        md.push('\n');

        for obj in arr {
            check_import(cancelled, md.len())?;
            md.push('|');
            if let Some(map) = obj.as_object() {
                for key in &keys {
                    md.push(' ');
                    if let Some(val) = map.get(key) {
                        let s = match val {
                            serde_json::Value::Null => "".to_string(),
                            serde_json::Value::String(s) => s.clone(),
                            _ => val.to_string(),
                        };
                        md.push_str(&s.replace('|', "\\|").replace('\n', " <br>"));
                    }
                    md.push_str(" |");
                    check_import(cancelled, md.len())?;
                }
            }
            md.push('\n');
        }
        check_import(cancelled, md.len())?;
        return Ok(sanitized(file_stem_title(path), md));
    }

    let md = format!("```json\n{}\n```", serde_json::to_string_pretty(&value)?);
    check_import(cancelled, md.len())?;
    Ok(sanitized(file_stem_title(path), md))
}

pub fn convert_url(url: &str) -> Result<(String, String)> {
    convert_url_cancellable(url, &AtomicBool::new(false))
}

fn convert_url_cancellable(source: &str, cancelled: &AtomicBool) -> Result<(String, String)> {
    let url = reqwest::Url::parse(source).context("Invalid import URL")?;
    anyhow::ensure!(
        matches!(url.scheme(), "http" | "https"),
        "URL imports require HTTP(S)"
    );
    let (url, content_type, bytes) = crate::http::run(cancelled, Duration::from_secs(30), async {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() > 5 {
                    attempt.error("URL import exceeds five redirects")
                } else if !matches!(attempt.url().scheme(), "http" | "https") {
                    attempt.error("URL imports require HTTP(S) redirects")
                } else {
                    attempt.follow()
                }
            }))
            .build()?;
        let response = client
            .get(url)
            .send()
            .await
            .map_err(reqwest::Error::without_url)?;
        response
            .error_for_status_ref()
            .map_err(reqwest::Error::without_url)?;
        let url = response.url().clone();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_owned();
        let bytes = crate::http::read_bounded(response, MAX_IMPORT_BYTES).await?;
        Ok((url, content_type, bytes))
    })?;
    let ext = match content_type.split(';').next().unwrap_or("").trim() {
        "text/html" | "application/xhtml+xml" => "html",
        "application/pdf" => "pdf",
        _ => Path::new(url.path())
            .extension()
            .and_then(|ext| ext.to_str())
            .filter(|ext| ext.len() <= 5 && ext.chars().all(|c| c.is_ascii_alphanumeric()))
            .unwrap_or("html"),
    };
    let directory = process::ImportDirectory::new()?;
    let downloaded = directory.path.join(format!("download.{ext}"));
    let temp_path = downloaded.to_str().context("temp path is not UTF-8")?;
    let title = std::str::from_utf8(&bytes)
        .ok()
        .and_then(extract_html_title)
        .unwrap_or_else(|| url_fallback_title(source));
    crate::fsutil::atomic_write_with_mode(&downloaded, &bytes, 0o600)?;
    drop(bytes);
    if matches!(ext.to_ascii_lowercase().as_str(), "html" | "htm")
        && crate::fsutil::can_run("trafilatura")
    {
        if let Ok((extracted_title, content)) = article::extract(&downloaded, &url, cancelled) {
            return Ok(sanitized(extracted_title.unwrap_or(title), content));
        }
        check_import(cancelled, 0)?;
    }
    // Convert the downloaded bytes once; title and body come from the same response.
    let (_, content) = convert_file_cancellable(temp_path, cancelled)?;
    Ok(sanitized(title, content))
}

pub(crate) fn convert_source(
    source: ImportSource,
    input: &str,
    cancelled: &AtomicBool,
) -> Result<(String, String)> {
    anyhow::ensure!(!cancelled.load(Ordering::Relaxed), "Import cancelled");
    let result = match source {
        ImportSource::File => convert_file_cancellable(input, cancelled),
        ImportSource::Csv => convert_csv_cancellable(input, cancelled),
        ImportSource::Json => convert_json_cancellable(input, cancelled),
        ImportSource::Url => convert_url_cancellable(input, cancelled),
        ImportSource::Clipboard => bail!("Clipboard imports do not use a prompt"),
    }?;
    anyhow::ensure!(
        result.1.len() <= MAX_IMPORT_BYTES,
        "Imported note exceeds 16 MiB safety limit"
    );
    Ok(result)
}

pub fn clipboard_to_md() -> Result<(String, String)> {
    let mut clipboard = arboard::Clipboard::new().context("Failed to open clipboard")?;
    let text = clipboard
        .get_text()
        .context("Clipboard is empty or does not contain text")?;
    if text.trim().is_empty() {
        bail!("Clipboard is empty");
    }

    let title = format!(
        "Clipboard {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M")
    );
    Ok(sanitized(title, text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    #[ignore = "requires the local imports Postman simulation"]
    fn url_download_can_cancel_a_pending_response() -> Result<()> {
        let base = std::env::var("CLIN_IMPORT_SIMULATION_URL")?;
        let cancelled = AtomicBool::new(false);
        std::thread::scope(|scope| {
            scope.spawn(|| {
                std::thread::sleep(Duration::from_millis(200));
                cancelled.store(true, Ordering::Relaxed);
            });
            let started = std::time::Instant::now();
            let error = convert_url_cancellable(&format!("{base}/slow"), &cancelled).unwrap_err();
            assert!(error.to_string().contains("cancelled"));
            assert!(started.elapsed() < Duration::from_secs(1));
        });
        Ok(())
    }

    #[test]
    fn html_titles_allow_unicode_before_and_inside_tags() {
        for html in [
            "<!-- é --><TITLE>Résumé</TITLE>",
            "<title>Résumé<!-- éé --> end</title>",
        ] {
            assert!(extract_html_title(html).unwrap().starts_with("Résumé"));
        }
        assert_eq!(extract_html_title("<!-- é -->"), None);
    }

    #[test]
    fn import_sanitization_preserves_structure() {
        let (title, body) = sanitized("Ti\x07tle".into(), "α\r\nβ\rγ\n\tδ\x07".into());
        assert_eq!(title, "Title");
        assert_eq!(body, "α\nβ\nγ\n\tδ");
        assert_eq!(sanitize_import_content("\nbody\n".into()), "\nbody\n");
    }

    #[test]
    fn test_csv_conversion() -> Result<()> {
        let mut tmp = NamedTempFile::new()?;
        writeln!(tmp, "a,b,c")?;
        writeln!(tmp, "1,2,3")?;
        writeln!(tmp, "4,5|x,6")?;

        let (title, md) = convert_csv(tmp.path().to_str().unwrap())?;
        assert!(title.contains("tmp"));
        assert_eq!(
            md.lines().collect::<Vec<_>>(),
            [
                "| a | b | c |",
                "| --- | --- | --- |",
                "| 1 | 2 | 3 |",
                "| 4 | 5\\|x | 6 |",
            ]
        );
        assert!(md.ends_with('\n'));
        Ok(())
    }

    #[test]
    fn csv_rejects_rows_that_would_lose_fields() -> Result<()> {
        let mut tmp = NamedTempFile::new()?;
        writeln!(tmp, "a,b\n1,2,valuable data")?;
        let error = convert_csv(tmp.path().to_str().unwrap()).unwrap_err();
        assert!(format!("{error:#}").contains("3 fields"));
        Ok(())
    }

    #[test]
    fn test_json_table_conversion() -> Result<()> {
        let mut tmp = NamedTempFile::new()?;
        write!(
            tmp,
            "[{{\"a\": 1, \"b\": \"x\"}}, {{\"a\": 2, \"c\": true}}]"
        )?;

        let (_, md) = convert_json(tmp.path().to_str().unwrap())?;
        assert_eq!(
            md.lines().collect::<Vec<_>>(),
            [
                "| a | b | c |",
                "| --- | --- | --- |",
                "| 1 | x |  |",
                "| 2 |  | true |",
            ]
        );
        Ok(())
    }

    #[test]
    fn sparse_json_tables_stop_at_the_output_budget_and_can_cancel() -> Result<()> {
        let mut tmp = NamedTempFile::new()?;
        let objects = (0..3000)
            .map(|i| format!("{{\"column{i}\":{i}}}"))
            .collect::<Vec<_>>()
            .join(",");
        write!(tmp, "[{objects}]")?;
        let path = tmp.path().to_str().unwrap();
        assert!(
            convert_json(path)
                .unwrap_err()
                .to_string()
                .contains("16 MiB")
        );
        let cancelled = AtomicBool::new(false);
        std::thread::scope(|scope| {
            scope.spawn(|| {
                std::thread::sleep(Duration::from_millis(5));
                cancelled.store(true, Ordering::Relaxed);
            });
            assert!(
                convert_json_cancellable(path, &cancelled)
                    .unwrap_err()
                    .to_string()
                    .contains("cancelled")
            );
        });
        assert!(
            convert_csv_cancellable(path, &AtomicBool::new(true))
                .unwrap_err()
                .to_string()
                .contains("cancelled")
        );
        Ok(())
    }

    #[test]
    fn test_json_block_conversion() -> Result<()> {
        let mut tmp = NamedTempFile::new()?;
        write!(tmp, "{{\"not\": \"array\"}}")?;

        let (_, md) = convert_json(tmp.path().to_str().unwrap())?;
        assert_eq!(md, "```json\n{\n  \"not\": \"array\"\n}\n```");
        Ok(())
    }

    #[test]
    fn test_file_stem_title() {
        assert_eq!(file_stem_title("/path/to/my_note.txt"), "My note");
        assert_eq!(file_stem_title("simple-test"), "Simple test");
        assert_eq!(file_stem_title(".hidden"), ".hidden");
    }
}
