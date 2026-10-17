//! Public source identity and byte-preserving inline configuration anchors.
use anyhow::{Context, Result, bail};
use std::{ops::Range, sync::Arc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Definition {
    File {
        target: String,
        view: Option<String>,
    },
    Inline {
        source: Arc<str>,
        lines: Range<usize>,
    },
}
impl Definition {
    pub(crate) fn reference(text: &str) -> Option<Self> {
        let text = text.trim().strip_prefix("![[")?.strip_suffix("]]")?;
        if text.contains("[[") || text.contains("]]") {
            return None;
        }
        let text = text.split('|').next()?;
        let (target, view) = text.split_once('#').map_or((text, None), |(target, view)| {
            (target, Some(view.to_owned()))
        });
        target
            .to_ascii_lowercase()
            .ends_with(".base")
            .then(|| Self::File {
                target: target.to_owned(),
                view,
            })
    }
}

pub(crate) const SLOT_HEIGHT: usize = 8;
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Slot {
    pub rows: Range<usize>,
    pub source_line: usize,
    pub definition: Definition,
}

#[derive(Clone, Debug)]
pub(crate) struct Request {
    pub key: u64,
    pub session: u64,
    pub anchor: usize,
    pub definition: Definition,
    pub host: Option<String>,
    pub view: Option<String>,
    pub host_source: Option<Arc<str>>,
    pub body_only: bool,
    pub canvas_node: Option<String>,
}

impl Request {
    pub(crate) fn after_save(&self, host: &str) -> Result<Self> {
        let Definition::Inline { lines, .. } = &self.definition else {
            return Ok(self.clone());
        };
        let source = if let Some(node_id) = &self.canvas_node {
            let json: serde_json::Value = serde_json::from_str(host)?;
            json.get("nodes")
                .and_then(serde_json::Value::as_array)
                .and_then(|nodes| {
                    nodes.iter().find(|node| {
                        node.get("id").and_then(serde_json::Value::as_str) == Some(node_id)
                    })
                })
                .and_then(|node| node.get("text"))
                .and_then(serde_json::Value::as_str)
                .context("saved Canvas text node unavailable")?
                .to_owned()
        } else if self.body_only {
            crate::frontmatter::properties::PropertyDocument::parse(host)?
                .body
                .to_owned()
        } else {
            host.to_owned()
        };
        let arena = comrak::Arena::new();
        let root = comrak::parse_document(&arena, &source, &comrak::Options::default());
        let definition = root
            .descendants()
            .find_map(|node| {
                let data = node.data.borrow();
                if let comrak::nodes::NodeValue::CodeBlock(block) = &data.value
                    && block.fenced
                    && block.info.split_whitespace().next() == Some("base")
                    && data.sourcepos.start.line == lines.start
                {
                    Some(Definition::Inline {
                        source: Arc::from(block.literal.as_str()),
                        lines: data.sourcepos.start.line..data.sourcepos.end.line.saturating_sub(1),
                    })
                } else {
                    None
                }
            })
            .context("saved inline base source anchor unavailable")?;
        let mut request = self.clone();
        request.definition = definition;
        request.host_source = Some(Arc::from(source));
        Ok(request)
    }
    pub fn load(
        &self,
        storage: &crate::storage::Storage,
        metadata: &crate::storage::metadata::VaultMetadata,
    ) -> Result<(
        crate::storage::documents::DocumentSnapshot,
        crate::bases::format::BaseDocument,
        Option<InlineOrigin>,
    )> {
        use crate::bases::format::BaseDocument;
        if let Some(host) = &self.host
            && metadata.file(host).is_none()
        {
            bail!("containing file is unavailable in public metadata");
        }
        match &self.definition {
            Definition::File { target, .. } => {
                let file = metadata
                    .resolve_link(self.host.as_deref().unwrap_or(""), target)
                    .filter(|file| file.extension.eq_ignore_ascii_case("base"))
                    .context("embedded base is missing or ambiguous in public metadata")?;
                let snapshot = storage.load_document(&file.id)?;
                let document = BaseDocument::parse(snapshot.source())?;
                Ok((snapshot, document, None))
            }
            Definition::Inline { source, lines } => {
                let host = self
                    .host
                    .as_deref()
                    .context("inline base requires a public containing file")?;
                let snapshot = storage.load_document(host)?;
                let host_source = self
                    .host_source
                    .clone()
                    .context("inline base source anchor unavailable")?;
                let mut origin = InlineOrigin::new(host_source, lines.clone())?;
                if let Some(node) = &self.canvas_node {
                    if let Err(error) = origin.rebase_canvas(snapshot.source(), node) {
                        origin.write_error = Some(error.to_string());
                    }
                } else if snapshot.source().ends_with(&*origin.host_source) {
                    let prefix = snapshot.source().len() - origin.host_source.len();
                    origin.range = origin.range.start + prefix..origin.range.end + prefix;
                    origin.host_source = Arc::from(snapshot.source());
                }
                let document = BaseDocument::parse(source)?;
                Ok((snapshot, document, Some(origin)))
            }
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct InlineOrigin {
    pub range: Range<usize>,
    pub host_source: Arc<str>,
    json_text: Option<(Range<usize>, Arc<str>)>,
    write_error: Option<String>,
}
impl InlineOrigin {
    pub fn new(source: Arc<str>, lines: Range<usize>) -> Result<Self> {
        let offsets: Vec<_> = std::iter::once(0)
            .chain(source.match_indices('\n').map(|(offset, _)| offset + 1))
            .chain(std::iter::once(source.len()))
            .collect();
        let start = *offsets
            .get(lines.start)
            .context("inline base opening anchor is unavailable")?;
        let end = *offsets
            .get(lines.end)
            .context("inline base closing anchor is unavailable")?;
        if start > end {
            bail!("invalid inline base anchor");
        }
        Ok(Self {
            range: start..end,
            host_source: source,
            json_text: None,
            write_error: None,
        })
    }
    pub fn rebase_canvas(&mut self, baseline: &str, id: &str) -> Result<()> {
        #[derive(serde::Deserialize)]
        struct Canvas<'a> {
            #[serde(borrow)]
            nodes: Vec<&'a serde_json::value::RawValue>,
        }
        #[derive(serde::Deserialize)]
        struct Node<'a> {
            id: String,
            #[serde(borrow)]
            text: Option<&'a serde_json::value::RawValue>,
        }
        let canvas: Canvas<'_> = serde_json::from_str(baseline)?;
        let nodes = canvas
            .nodes
            .iter()
            .map(|node| serde_json::from_str::<Node<'_>>(node.get()))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut matches = nodes.iter().filter(|node| node.id == id);
        let raw = matches
            .next()
            .and_then(|node| node.text)
            .context("inline Canvas text node is unavailable")?;
        if matches.next().is_some() {
            bail!("Canvas node id is ambiguous");
        }
        let text: String = serde_json::from_str(raw.get())?;
        if text != *self.host_source {
            bail!("save the Canvas text node before editing its inline base configuration");
        }
        let offset = raw.get().as_ptr() as usize - baseline.as_ptr() as usize;
        self.json_text = Some((offset..offset + raw.get().len(), self.host_source.clone()));
        self.host_source = Arc::from(baseline);
        Ok(())
    }
    pub fn replace(&self, baseline: &str, definition: &str) -> Result<String> {
        if let Some(error) = &self.write_error {
            bail!("{error}");
        }
        if baseline != &*self.host_source {
            bail!("save the containing public document before editing inline base configuration");
        }
        let mut source = baseline.to_owned();
        let original = self
            .json_text
            .as_ref()
            .map_or(baseline, |(_, text)| &**text);
        let definition = if original.contains("\r\n") {
            definition.replace("\r\n", "\n").replace('\n', "\r\n")
        } else {
            definition.to_owned()
        };
        if let Some((range, text)) = &self.json_text {
            let mut text = text.to_string();
            text.replace_range(self.range.clone(), &definition);
            source.replace_range(range.clone(), &serde_json::to_string(&text)?);
        } else {
            source.replace_range(self.range.clone(), &definition);
        }
        Ok(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn references_keep_named_view_and_inline_edits_keep_host_bytes() {
        assert_eq!(
            Definition::reference("![[folder/Board.base#My view]]"),
            Some(Definition::File {
                target: "folder/Board.base".into(),
                view: Some("My view".into())
            })
        );
        assert!(Definition::reference("![[private.clin]]").is_none());
        let source: Arc<str> = "Before\r\n```base\r\nviews: []\r\n```\r\nAfter\r\n".into();
        let origin = InlineOrigin::new(source.clone(), 2..3).unwrap();
        assert_eq!(
            origin.replace(&source, "views: [table]\n").unwrap(),
            "Before\r\n```base\r\nviews: [table]\r\n```\r\nAfter\r\n"
        );
        assert!(origin.replace("external change", "views: []").is_err());
    }
}
