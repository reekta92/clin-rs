use super::{COMMAND_TIMEOUT, process};
use anyhow::{Context, Result, ensure};
use comrak::{Arena, format_commonmark, nodes::NodeValue, parse_document};
use reqwest::Url;
use std::{path::Path, process::Command, sync::atomic::AtomicBool};

pub(super) fn extract(
    path: &Path,
    url: &Url,
    cancelled: &AtomicBool,
) -> Result<(Option<String>, String)> {
    let mut command = Command::new("trafilatura");
    command.args([
        "--markdown",
        "--links",
        "--images",
        "--no-comments",
        "--with-metadata",
    ]);
    command.current_dir(path.parent().context("article file has no parent")?);
    let output = process::run(command, Some(path), cancelled, COMMAND_TIMEOUT)?;
    ensure!(output.status.success(), "Article extraction failed");
    parse_output(std::str::from_utf8(&output.stdout)?, url)
}

fn parse_output(source: &str, url: &Url) -> Result<(Option<String>, String)> {
    use crate::frontmatter::properties::{PropertyDocument, PropertyFrame};
    let frame = PropertyFrame::parse(source)?;
    ensure!(
        !frame.body.trim().is_empty(),
        "Article extraction produced no content"
    );
    let title = PropertyDocument::parse(source).ok().and_then(|document| {
        document
            .properties
            .get("title")
            .and_then(serde_yaml_ng::Value::as_str)
            .filter(|title| !title.trim().is_empty())
            .map(str::to_owned)
    });
    let arena = Arena::new();
    let options = crate::markdown::structure::options();
    let root = parse_document(&arena, frame.body, &options);
    let mut expanded_bytes = frame.body.len();
    for node in root.descendants() {
        match &mut node.data.borrow_mut().value {
            NodeValue::Link(link) | NodeValue::Image(link) => {
                if let Ok(absolute) = url.join(&link.url) {
                    expanded_bytes = expanded_bytes
                        .saturating_add(absolute.as_str().len().saturating_sub(link.url.len()));
                    ensure!(
                        expanded_bytes <= super::MAX_IMPORT_BYTES,
                        "Article exceeds 16 MiB"
                    );
                    link.url = absolute.into();
                }
            }
            _ => {}
        }
    }
    let mut markdown = BoundedMarkdown(String::new());
    format_commonmark(root, &options, &mut markdown).context("Article exceeds 16 MiB")?;
    Ok((title, markdown.0.trim().to_owned()))
}

struct BoundedMarkdown(String);

impl std::fmt::Write for BoundedMarkdown {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        if text.len() > super::MAX_IMPORT_BYTES - self.0.len() {
            return Err(std::fmt::Error);
        }
        self.0.push_str(text);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_and_relative_links_survive_article_conversion() -> Result<()> {
        let url = Url::parse("https://example.org/news/article.html")?;
        let (title, body) = parse_output(
            "---\ntitle: 'Résumé: été'\n---\n# Article\n\n[report](../report) ![photo](/image.png) [web](https://other.org/)\n\n```text\n[code](/unchanged)\n```\n",
            &url,
        )?;
        assert_eq!(title.as_deref(), Some("Résumé: été"));
        assert!(body.contains("https://example.org/report"));
        assert!(body.contains("https://example.org/image.png"));
        assert!(body.contains("https://other.org/"));
        assert!(body.contains("[code](/unchanged)"));
        assert!(!body.contains("title:"));
        assert!(parse_output("---\ntitle: Empty\n---\n", &url).is_err());
        let (title, body) = parse_output("---\ntitle: unquoted: metadata\n---\nUseful body", &url)?;
        assert!(title.is_none());
        assert_eq!(body, "Useful body");
        Ok(())
    }

    #[test]
    fn relative_url_expansion_cannot_exceed_the_import_budget() -> Result<()> {
        let url = Url::parse(&format!(
            "https://example.org/{}/",
            "a".repeat(super::super::MAX_IMPORT_BYTES)
        ))?;
        assert!(
            parse_output("[link](relative)", &url)
                .expect_err("URL expansion must remain bounded")
                .to_string()
                .contains("16 MiB")
        );
        Ok(())
    }
}
