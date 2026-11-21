use super::{
    Storage,
    documents::DocumentSnapshot,
    metadata::{VaultMetadata, decode_percent, normalize_path},
};
use crate::frontmatter::properties::{PropertyDocument, PropertyFrame, edit_properties};
use crate::markdown::rename_links::{Style, rewrite};
use anyhow::Result;
use std::fmt::Write as _;
use std::sync::atomic::AtomicBool;

impl Storage {
    pub(crate) fn rewrite_renamed_note_links(
        &self,
        metadata: &VaultMetadata,
        from: &str,
        to: &str,
    ) -> Result<usize> {
        if from == to {
            return Ok(0);
        }
        let mut plans = Vec::new();
        let mut bytes = 0;
        for file in metadata
            .files()
            .filter(|file| matches!(file.extension.as_str(), "md" | "txt"))
        {
            let id = if file.id == from { to } else { &file.id };
            let snapshot = self.load_document(id)?;
            if let Some(source) = retarget(
                &snapshot,
                &file.id,
                metadata,
                from,
                to,
                self.frontmatter_policy.links,
            )? {
                bytes += snapshot.source().len() + source.len();
                anyhow::ensure!(
                    bytes <= super::document_recovery::MAX_DOCUMENT_TRANSACTION_BYTES,
                    "link updates exceed the 32 MiB transaction limit"
                );
                plans.push((snapshot, source));
            }
        }
        let count = plans.len();
        if count != 0 {
            self.apply_document_changes(plans, &AtomicBool::new(false))?;
        }
        Ok(count)
    }
}

fn retarget(
    snapshot: &DocumentSnapshot,
    source_id: &str,
    metadata: &VaultMetadata,
    from: &str,
    to: &str,
    update_index: bool,
) -> Result<Option<String>> {
    let frame = PropertyFrame::parse(snapshot.source())?;
    let folder = source_id.rsplit_once('/').map_or("", |(folder, _)| folder);
    let body = rewrite(frame.body, |target, style| match style {
        Style::Wiki => {
            let resolved = metadata.resolve_link(source_id, target)?;
            if resolved.id != from {
                return None;
            }
            let mut relative = relative_path(folder, to);
            let target = target.split(['#', '|']).next().unwrap_or("");
            if resolved.extension == "md"
                && !target.to_ascii_lowercase().ends_with(".md")
                && let Some(stem) = relative.strip_suffix(".md")
                && metadata.file(&to[..to.len() - 3]).is_none()
            {
                relative = stem.to_owned();
            }
            Some(
                relative
                    .replace('%', "%25")
                    .replace('#', "%23")
                    .replace('|', "%7C")
                    .replace('[', "%5B")
                    .replace(']', "%5D"),
            )
        }
        Style::Markdown => {
            let split = target.find(['#', '?']).unwrap_or(target.len());
            let (path, suffix) = target.split_at(split);
            let path = decode_percent(path)?;
            if path.is_empty()
                || crate::markdown::structure::is_external(&path)
                || path.starts_with(['/', '\\'])
            {
                return None;
            }
            let normalized = normalize_path(&if folder.is_empty() {
                path
            } else {
                format!("{folder}/{path}")
            })?;
            if normalized != from {
                return None;
            }
            Some(format!(
                "{}{suffix}",
                encode_path(&relative_path(folder, to))
            ))
        }
    });
    if body == frame.body {
        return Ok(None);
    }
    let header = &snapshot.source()[..snapshot.source().len() - frame.body.len()];
    let source = format!("{header}{body}");
    // A links property that mirrors the body is Clin's derived link index.
    if update_index
        && let Ok(doc) = PropertyDocument::parse(snapshot.source())
        && let Some(index) = doc.properties.get("links").cloned()
        && serde_yaml_ng::from_value::<Vec<String>>(index)
            .ok()
            .as_ref()
            == Some(&crate::storage::extract_wikilinks(frame.body))
    {
        let links = serde_yaml_ng::to_value(crate::storage::extract_wikilinks(&body))?;
        return Ok(Some(edit_properties(
            &source,
            &[("links".into(), Some(links))],
        )?));
    }
    Ok(Some(source))
}

fn relative_path(folder: &str, target: &str) -> String {
    let mut folders = folder.split('/').filter(|part| !part.is_empty()).peekable();
    let mut parts = target.split('/').peekable();
    while folders.peek().is_some() && folders.peek() == parts.peek() {
        folders.next();
        parts.next();
    }
    folders
        .map(|_| "..")
        .chain(parts)
        .collect::<Vec<_>>()
        .join("/")
}

fn encode_path(path: &str) -> String {
    let mut encoded = String::new();
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'-' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            write!(encoded, "%{byte:02X}").expect("writing to String");
        }
    }
    encoded
}
