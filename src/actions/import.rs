use crate::actions::Action;
use crate::app::App;
use crate::popups::{ImportSource, ImportTarget};
use anyhow::{Context, Result, bail};
use once_cell::sync::Lazy;
use std::borrow::Cow;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;

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

    fn glyph(&self) -> &'static str {
        match self.source {
            ImportSource::File => "\u{f15b}",
            ImportSource::Csv => "\u{f0ce}",
            ImportSource::Json => "\u{f121}",
            ImportSource::Url => "\u{f0ac}",
            ImportSource::Clipboard => "\u{f0ea}",
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
                    let result = match self.source {
                        ImportSource::File => convert_file(&path),
                        ImportSource::Csv => convert_csv(&path),
                        ImportSource::Json => convert_json(&path),
                        _ => unreachable!(),
                    };

                    match result {
                        Ok((title, md)) => {
                            app.insert_content(self.target, note_id.as_deref(), title, md)?;
                        }
                        Err(e) => {
                            app.set_temporary_status(&format!("Import failed: {e:#}"));
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

static TITLE_RE: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"(?i)<title[^>]*>(.*?)</title>").expect("static regex"));

fn extract_html_title(html: &str) -> Option<String> {
    TITLE_RE.captures(html).map(|caps| {
        caps.get(1)
            .expect("title group captured")
            .as_str()
            .trim()
            .to_string()
    })
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

pub fn convert_file(path: &str) -> Result<(String, String)> {
    let mut cmd = if which::which("markitdown").is_ok() {
        let mut c = Command::new("markitdown");
        c.arg(path);
        c
    } else if which::which("pandoc").is_ok() {
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

    let output = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("Failed to run conversion tool")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Conversion failed: {stderr}");
    }

    let md = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if md.is_empty() {
        bail!("Conversion produced no content");
    }

    Ok((file_stem_title(path), md))
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
    let text = fs::read_to_string(path).context("Failed to read CSV file")?;
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
        .flexible(true)
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
        }
        md.push('\n');

        md.push('|');
        for _ in 0..header_len {
            md.push_str(" --- |");
        }
        md.push('\n');
    } else {
        bail!("CSV has no rows");
    }

    for result in rdr.records() {
        let record = result.context("Failed to read CSV row")?;
        md.push('|');
        for i in 0..header_len {
            md.push(' ');
            if let Some(field) = record.get(i) {
                md.push_str(&field.replace('|', "\\|").replace('\n', " <br>"));
            }
            md.push_str(" |");
        }
        md.push('\n');
    }

    Ok((file_stem_title(path), md))
}

pub fn convert_json(path: &str) -> Result<(String, String)> {
    let text = fs::read_to_string(path).context("Failed to read JSON file")?;
    let value: serde_json::Value = serde_json::from_str(&text).context("Invalid JSON")?;

    if let Some(arr) = value.as_array()
        && !arr.is_empty()
        && arr.iter().all(|v| v.is_object())
    {
        let mut keys = Vec::new();
        for obj in arr {
            if let Some(map) = obj.as_object() {
                for key in map.keys() {
                    if !keys.contains(key) {
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
        }
        md.push('\n');

        md.push('|');
        for _ in 0..keys.len() {
            md.push_str(" --- |");
        }
        md.push('\n');

        for obj in arr {
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
                }
            }
            md.push('\n');
        }
        return Ok((file_stem_title(path), md));
    }

    let md = format!("```json\n{}\n```", serde_json::to_string_pretty(&value)?);
    Ok((file_stem_title(path), md))
}

pub fn convert_url(url: &str) -> Result<(String, String)> {
    if which::which("curl").is_err() {
        bail!("curl is required to fetch URLs");
    }

    let ext = url
        .split('?')
        .next()
        .unwrap_or(url)
        .split('.')
        .next_back()
        .filter(|e| e.len() <= 5 && e.chars().all(|c| c.is_alphanumeric()))
        .map(|e| format!(".{e}"))
        .unwrap_or_else(|| ".html".to_string());

    let temp_file = NamedTempFile::with_suffix(&ext).context("Failed to create temp file")?;
    let temp_path = temp_file
        .path()
        .to_str()
        .expect("temp path is UTF-8")
        .to_string();

    let output = Command::new("curl")
        .args(["-sL", "-o", &temp_path, url])
        .stderr(Stdio::piped())
        .output()
        .context("Failed to run curl")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("curl failed: {stderr}");
    }

    // If markitdown is available, let it handle the URL directly for better results
    if which::which("markitdown").is_ok() {
        let output = Command::new("markitdown")
            .arg(url)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .context("Failed to run markitdown on URL")?;

        if output.status.success() {
            let md = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !md.is_empty() {
                // Still need title extraction for the note name
                let mut title = None;
                let temp_file = NamedTempFile::new()
                    .context("Failed to create temp file for title extraction")?;
                let temp_path = temp_file
                    .path()
                    .to_str()
                    .expect("temp path is UTF-8")
                    .to_string();
                let _ = Command::new("curl")
                    .args(["-sL", "-o", &temp_path, url])
                    .status();
                if let Ok(html) = fs::read_to_string(&temp_path) {
                    title = extract_html_title(&html);
                }
                let final_title = title.unwrap_or_else(|| url_fallback_title(url));
                return Ok((final_title, md));
            }
        }
    }

    let (_, md) = convert_file(&temp_path)?;
    let mut title = None;
    if (ext == ".html" || ext == ".htm")
        && let Ok(html) = fs::read_to_string(&temp_path)
    {
        title = extract_html_title(&html);
    }

    let title = title.unwrap_or_else(|| url_fallback_title(url));

    Ok((title, md))
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
    Ok((title, text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_csv_conversion() -> Result<()> {
        let mut tmp = NamedTempFile::new()?;
        writeln!(tmp, "a,b,c")?;
        writeln!(tmp, "1,2,3")?;
        writeln!(tmp, "4,5|x,6")?;

        let (title, md) = convert_csv(tmp.path().to_str().unwrap())?;
        assert!(title.contains("tmp"));
        println!("MD: {md:?}");
        assert!(md.contains("| a | b | c |") || md.contains("| a | b | c |"));
        assert!(md.contains("| --- | --- | --- |"));
        assert!(md.contains("| 1 | 2 | 3 |"));
        assert!(md.contains("| 4 | 5\\|x | 6 |"));
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
        assert!(md.contains("| a | b | c |"));
        assert!(md.contains("| 1 | x |  |"));
        assert!(md.contains("| 2 |  | true |"));
        Ok(())
    }

    #[test]
    fn test_json_block_conversion() -> Result<()> {
        let mut tmp = NamedTempFile::new()?;
        write!(tmp, "{{\"not\": \"array\"}}")?;

        let (_, md) = convert_json(tmp.path().to_str().unwrap())?;
        assert!(md.starts_with("```json"));
        assert!(md.contains("\"not\": \"array\""));
        Ok(())
    }

    #[test]
    fn test_file_stem_title() {
        assert_eq!(file_stem_title("/path/to/my_note.txt"), "My note");
        assert_eq!(file_stem_title("simple-test"), "Simple test");
        assert_eq!(file_stem_title(".hidden"), ".hidden");
    }
}
