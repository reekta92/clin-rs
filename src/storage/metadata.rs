//! Immutable public-vault metadata. No decryption, virtual rows, or UI filters.

use super::{Storage, VaultScanPolicy};
use crate::frontmatter::properties::PropertyDocument;
use anyhow::{Context, Result, bail};
use comrak::nodes::NodeValue;
use comrak::{Arena, Options, parse_document};
use serde::Deserialize;
use serde_yaml_ng::{Mapping, Value};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

#[derive(Clone, Debug, Default)]
pub struct PropertyTypes {
    declarations: HashMap<String, String>,
}

impl PropertyTypes {
    pub fn declared_type(&self, property: &str) -> Option<&str> {
        if property == "tags" {
            return Some("tags");
        }
        self.declarations.get(property).map(String::as_str)
    }

    /// Validation belongs here rather than in a particular cell editor.
    pub fn validate(&self, property: &str, value: &Value) -> Result<()> {
        if value.is_null() {
            return Ok(());
        }
        let Some(kind) = self.declared_type(property) else {
            return Ok(());
        };
        let valid = match kind {
            "text" => value.is_string(),
            "number" => value.is_number(),
            "checkbox" => value.is_bool(),
            "multitext" => value
                .as_sequence()
                .is_some_and(|values| values.iter().all(Value::is_string)),
            "tags" => {
                value.is_string()
                    || value
                        .as_sequence()
                        .is_some_and(|values| values.iter().all(Value::is_string))
            }
            "date" => value
                .as_str()
                .is_some_and(|v| chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d").is_ok()),
            "datetime" => value.as_str().is_some_and(|v| {
                chrono::DateTime::parse_from_rfc3339(v).is_ok()
                    || chrono::NaiveDateTime::parse_from_str(v, "%Y-%m-%dT%H:%M:%S%.f").is_ok()
                    || chrono::NaiveDateTime::parse_from_str(v, "%Y-%m-%dT%H:%M").is_ok()
            }),
            _ => bail!("unsupported property type {kind:?}; declaration retained"),
        };
        if !valid {
            bail!("property {property:?} requires type {kind}");
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct FileMetadata {
    pub id: String,
    pub name: String,
    pub basename: String,
    pub extension: String,
    pub folder: String,
    pub size: u64,
    pub created: Option<SystemTime>,
    pub modified: Option<SystemTime>,
    pub tags: Vec<String>,
    pub links: Vec<String>,
    pub embeds: Vec<String>,
    pub backlinks: Vec<String>,
    properties: Mapping,
    property_error: Option<String>,
}

impl FileMetadata {
    pub fn properties(&self) -> Result<&Mapping> {
        if let Some(error) = &self.property_error {
            bail!("{}: {error}", self.id);
        }
        Ok(&self.properties)
    }

    pub fn property_error(&self) -> Option<&str> {
        self.property_error.as_deref()
    }
}

#[derive(Clone, Debug, Default)]
pub struct VaultMetadata {
    files: HashMap<String, FileMetadata>,
    paths: HashMap<String, String>,
    names: HashMap<String, Vec<String>>,
    pub property_types: PropertyTypes,
}

impl VaultMetadata {
    pub fn files(&self) -> impl Iterator<Item = &FileMetadata> {
        self.files.values()
    }
    pub fn file(&self, id: &str) -> Option<&FileMetadata> {
        self.files.get(id)
    }
    pub fn len(&self) -> usize {
        self.files.len()
    }
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Resolution never opens a file; private and outside paths are absent from this snapshot.
    pub fn resolve_link(&self, source_id: &str, target: &str) -> Option<&FileMetadata> {
        let target = target
            .strip_prefix("![[")
            .or_else(|| target.strip_prefix("[["))
            .map_or(target, |v| v.strip_suffix("]]").unwrap_or(v));
        let target = target.split('|').next()?.split('#').next()?;
        let target = decode_percent(target)?;
        if target.is_empty() {
            return self.file(source_id);
        }
        if is_external(&target) || target.starts_with('/') || target.starts_with('\\') {
            return None;
        }
        let folder = self.file(source_id).map_or("", |file| file.folder.as_str());
        let local = if folder.is_empty() {
            target.clone()
        } else {
            format!("{folder}/{target}")
        };
        let candidates = [local, target.clone()];
        for candidate in candidates {
            let Some(normalized) = normalize_path(&candidate) else {
                continue;
            };
            let key = normalized.to_lowercase();
            if let Some(id) = self
                .paths
                .get(&key)
                .or_else(|| self.paths.get(&format!("{key}.md")))
            {
                return self.file(id);
            }
        }
        let normalized = normalize_path(&target)?;
        let ids = self.names.get(&normalized.to_lowercase())?;
        if ids.len() != 1 {
            return None;
        }
        self.file(&ids[0])
    }
}

impl Storage {
    pub fn load_vault_metadata(&self, cancelled: &AtomicBool) -> Result<VaultMetadata> {
        if cancelled.load(Ordering::Relaxed) {
            bail!("metadata load cancelled");
        }
        let scan = self.scan_vault_with_policy(VaultScanPolicy::PublicFiles)?;
        if !scan.complete {
            bail!("public vault scan incomplete: {}", scan.warnings.join("; "));
        }
        let mut snapshot = VaultMetadata {
            property_types: self.load_property_types()?,
            ..VaultMetadata::default()
        };
        for entry in scan.files {
            if cancelled.load(Ordering::Relaxed) {
                bail!("metadata load cancelled");
            }
            let path = self.document_path(&entry.id)?;
            let stats = std::fs::metadata(&path).context("failed to read file metadata")?;
            let name = Path::new(&entry.id)
                .file_name()
                .and_then(|v| v.to_str())
                .context("invalid filename")?
                .to_owned();
            let basename = Path::new(&entry.id)
                .file_stem()
                .and_then(|v| v.to_str())
                .context("invalid filename")?
                .to_owned();
            let extension = Path::new(&entry.id)
                .extension()
                .and_then(|v| v.to_str())
                .unwrap_or("")
                .to_owned();
            let folder = Path::new(&entry.id)
                .parent()
                .and_then(|v| v.to_str())
                .unwrap_or("")
                .replace('\\', "/");
            let mut file = FileMetadata {
                id: entry.id.clone(),
                name,
                basename,
                extension,
                folder,
                size: stats.len(),
                created: stats.created().ok(),
                modified: stats.modified().ok(),
                tags: Vec::new(),
                links: Vec::new(),
                embeds: Vec::new(),
                backlinks: Vec::new(),
                properties: Mapping::new(),
                property_error: None,
            };
            if file.extension.eq_ignore_ascii_case("md") {
                match self.load_document(&file.id).and_then(|document| {
                    let parsed = PropertyDocument::parse(document.source())?;
                    let mut markdown = markdown_metadata(parsed.body);
                    for value in parsed.properties.values() {
                        collect_property_links(value, &mut markdown.links);
                    }
                    deduplicate(&mut markdown.links);
                    let mut tags = property_tags(parsed.properties.get("tags"));
                    tags.extend(markdown.tags);
                    deduplicate(&mut tags);
                    Ok((parsed.properties, tags, markdown.links, markdown.embeds))
                }) {
                    Ok((properties, tags, links, embeds)) => {
                        file.properties = properties;
                        file.tags = tags;
                        file.links = links;
                        file.embeds = embeds;
                    }
                    Err(error) => file.property_error = Some(error.to_string()),
                }
            }
            snapshot
                .paths
                .entry(file.id.to_lowercase())
                .or_insert_with(|| file.id.clone());
            for name in [&file.name, &file.basename] {
                let names = snapshot.names.entry(name.to_lowercase()).or_default();
                if !names.contains(&file.id) {
                    names.push(file.id.clone());
                }
            }
            snapshot.files.insert(file.id.clone(), file);
        }
        let mut reverse: HashMap<String, Vec<String>> = HashMap::new();
        for file in snapshot.files() {
            if cancelled.load(Ordering::Relaxed) {
                bail!("metadata load cancelled");
            }
            for link in file.links.iter().chain(&file.embeds) {
                if let Some(target) = snapshot.resolve_link(&file.id, link) {
                    reverse
                        .entry(target.id.clone())
                        .or_default()
                        .push(file.id.clone());
                }
            }
        }
        for (target, mut sources) in reverse {
            sources.sort();
            sources.dedup();
            if let Some(file) = snapshot.files.get_mut(&target) {
                file.backlinks = sources;
            }
        }
        Ok(snapshot)
    }

    pub(crate) fn load_property_types(&self) -> Result<PropertyTypes> {
        let root = self.notes_dir.canonicalize()?;
        let path = root.join(".obsidian/types.json");
        if !path.try_exists()? {
            return Ok(PropertyTypes::default());
        }
        // Only this known metadata file is authorized; no hidden-path read bypass.
        if !path.canonicalize()?.starts_with(&root)
            || std::fs::symlink_metadata(root.join(".obsidian"))?
                .file_type()
                .is_symlink()
            || std::fs::symlink_metadata(&path)?.file_type().is_symlink()
        {
            bail!("property registry symlinks are not supported");
        }
        let source = super::documents::read_bounded_text(&path, 1024 * 1024)?;
        #[derive(Deserialize)]
        struct Registry {
            #[serde(default)]
            types: HashMap<String, String>,
        }
        let registry: Registry =
            serde_json::from_str(&source).context("invalid property type registry")?;
        Ok(PropertyTypes {
            declarations: registry.types,
        })
    }
}

#[derive(Default)]
struct MarkdownMetadata {
    tags: Vec<String>,
    links: Vec<String>,
    embeds: Vec<String>,
}

fn markdown_metadata(content: &str) -> MarkdownMetadata {
    let arena = Arena::new();
    let mut options = Options::default();
    options.extension.wikilinks_title_after_pipe = true;
    let root = parse_document(&arena, content, &options);
    let mut metadata = MarkdownMetadata::default();
    for node in root.descendants() {
        let data = node.data.borrow();
        match &data.value {
            NodeValue::WikiLink(link) => {
                let embedded = node.previous_sibling().is_some_and(|previous| {
                    matches!(&previous.data.borrow().value, NodeValue::Text(text) if text.ends_with('!'))
                });
                if embedded {
                    metadata.embeds.push(link.url.clone());
                } else {
                    metadata.links.push(link.url.clone());
                }
            }
            NodeValue::Link(link) if !is_external(&link.url) => {
                metadata.links.push(link.url.clone())
            }
            NodeValue::Image(link) if !is_external(&link.url) => {
                metadata.embeds.push(link.url.clone())
            }
            NodeValue::Text(text) => {
                let in_link = node.ancestors().skip(1).any(|parent| {
                    matches!(
                        &parent.data.borrow().value,
                        NodeValue::Link(_) | NodeValue::WikiLink(_) | NodeValue::Image(_)
                    )
                });
                if !in_link {
                    collect_inline_tags(text, &mut metadata.tags);
                }
            }
            _ => {}
        }
    }
    deduplicate(&mut metadata.links);
    deduplicate(&mut metadata.embeds);
    metadata
}

fn is_external(target: &str) -> bool {
    target.contains(':') || target.starts_with("//")
}

fn collect_property_links(value: &Value, links: &mut Vec<String>) {
    match value {
        Value::String(value) if value.starts_with("[[") && value.ends_with("]]") => {
            links.push(
                value[2..value.len() - 2]
                    .split('|')
                    .next()
                    .unwrap_or("")
                    .to_owned(),
            );
        }
        Value::Sequence(values) => {
            for value in values {
                collect_property_links(value, links);
            }
        }
        Value::Mapping(values) => {
            for value in values.values() {
                collect_property_links(value, links);
            }
        }
        _ => {}
    }
}

fn property_tags(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Sequence(values)) => values
            .iter()
            .filter_map(Value::as_str)
            .map(|v| v.trim_start_matches('#').to_owned())
            .collect(),
        Some(Value::String(value)) => value
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|v| !v.is_empty())
            .map(|v| v.trim_start_matches('#').to_owned())
            .collect(),
        _ => Vec::new(),
    }
}

fn collect_inline_tags(text: &str, tags: &mut Vec<String>) {
    static TAG: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"#[\p{L}\p{N}_/-]+").expect("static tag expression"));
    for tag in TAG.find_iter(text) {
        let preceded_by_word = text[..tag.start()]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || matches!(c, '#' | '_' | '/' | '-'));
        let value = &tag.as_str()[1..];
        if !preceded_by_word && !value.chars().all(char::is_numeric) {
            tags.push(value.to_owned());
        }
    }
}

fn deduplicate(values: &mut Vec<String>) {
    let mut seen = HashSet::new();
    values.retain(|value| !value.is_empty() && seen.insert(value.clone()));
}

fn normalize_path(path: &str) -> Option<String> {
    if path.starts_with('/') || path.starts_with('\\') {
        return None;
    }
    let normalized = path.replace('\\', "/");
    let mut parts = Vec::new();
    for part in normalized.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            _ => {
                parts.push(part);
            }
        }
    }
    Some(parts.join("/"))
}

fn decode_percent(target: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(target.len());
    let mut source = target.as_bytes().iter().copied();
    while let Some(byte) = source.next() {
        if byte == b'%' {
            let high = char::from(source.next()?).to_digit(16)?;
            let low = char::from(source.next()?).to_digit(16)?;
            bytes.push((high * 16 + low) as u8);
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn vault() -> (tempfile::TempDir, Storage) {
        let dir = tempfile::tempdir().unwrap();
        let storage = Storage {
            data_dir: dir.path().to_owned(),
            config_dir: dir.path().to_owned(),
            notes_dir: dir.path().to_owned(),
            templates_dir: dir.path().join("templates"),
            key: [0; 32],
            skip_dir_patterns: vec![regex::Regex::new("^ignored$").unwrap()],
            rename_on_title_change: true,
        };
        (dir, storage)
    }

    fn load(storage: &Storage) -> VaultMetadata {
        storage
            .load_vault_metadata(&AtomicBool::new(false))
            .unwrap()
    }

    #[test]
    fn metadata_is_independent_of_note_visibility_and_never_reads_ciphertext() {
        let (dir, storage) = vault();
        fs::create_dir(dir.path().join("ignored")).unwrap();
        fs::create_dir(dir.path().join(".clin")).unwrap();
        for (id, source) in [
            ("a.base", "views: []"),
            (
                "a.md",
                "---\ntitle: 123\nupdated_at: 1\nprice: 3\n---\nBody",
            ),
            ("ignored/b.md", "B"),
            ("secret.clin", "not even valid ciphertext"),
            (".clin/subnotes.bin", "private"),
            (".hidden.md", "private"),
        ] {
            fs::write(dir.path().join(id), source).unwrap();
        }
        fs::write(dir.path().join("attachment.pdf"), [0xff, 0x00]).unwrap();
        let snapshot = load(&storage);
        assert_eq!(snapshot.len(), 4);
        assert_eq!(
            snapshot.file("a.md").unwrap().properties().unwrap()["title"],
            Value::Number(123.into())
        );
        assert!(snapshot.file("ignored/b.md").is_some());
        assert!(
            snapshot
                .file("attachment.pdf")
                .unwrap()
                .properties()
                .unwrap()
                .is_empty()
        );
        assert!(snapshot.file("secret.clin").is_none());
        assert!(snapshot.resolve_link("a.md", "secret.clin").is_none());
        assert!(
            !storage
                .list_note_ids(false, false)
                .unwrap()
                .contains(&"ignored/b.md".to_owned())
        );
        assert!(snapshot.file("a.md").unwrap().modified.is_some());
    }

    #[test]
    fn markdown_metadata_uses_ast_and_resolves_links_and_backlinks() {
        let (dir, storage) = vault();
        fs::create_dir(dir.path().join("folder")).unwrap();
        fs::write(dir.path().join("folder/Note A.md"), "A").unwrap();
        fs::write(dir.path().join("a.md"), "---\nlinks: '[[folder/Note A]]'\ntags: ['work', 'nested/child']\n---\n# Heading\n[[folder/Note A#Header|alias]] [A](folder/Note%20A.md) ![cover](cover.png) ![[cover.png]]\n#inline #nested/child #123 word#not\n`[[fake]] #code`\n```\n[[fake2]] #fence\n```\nhttps://example.test/#no [x](https://example.test/#external)\n").unwrap();
        fs::write(dir.path().join("cover.png"), [0xff]).unwrap();
        let snapshot = load(&storage);
        let note = snapshot.file("a.md").unwrap();
        assert_eq!(note.tags, ["work", "nested/child", "inline"]);
        assert!(note.links.contains(&"folder/Note A#Header".to_owned()));
        assert!(!note.links.iter().any(|l| l.starts_with("fake")));
        assert_eq!(note.embeds, ["cover.png"]);
        assert_eq!(
            snapshot.file("folder/Note A.md").unwrap().backlinks,
            ["a.md"]
        );
        assert_eq!(
            snapshot
                .resolve_link("a.md", "folder/Note%20A.md")
                .unwrap()
                .id,
            "folder/Note A.md"
        );
        assert!(snapshot.resolve_link("a.md", "../outside.md").is_none());
        assert!(snapshot.resolve_link("a.md", "%2e%2e/outside.md").is_none());
        assert!(snapshot.resolve_link("a.md", "/outside.md").is_none());
    }

    #[test]
    fn malformed_property_data_is_visible_as_error_not_empty_properties() {
        let (dir, storage) = vault();
        fs::write(dir.path().join("a.md"), "---\nx: [\n---\nBody").unwrap();
        let snapshot = load(&storage);
        let note = snapshot.file("a.md").unwrap();
        assert!(note.property_error().is_some());
        assert!(note.properties().is_err());
        assert_eq!(note.name, "a.md");
    }

    #[test]
    fn type_registry_preserves_declared_text_and_validates_edits() {
        let (dir, storage) = vault();
        fs::create_dir(dir.path().join(".obsidian")).unwrap();
        fs::write(dir.path().join(".obsidian/types.json"), r#"{"types":{"when":"text","day":"date","done":"checkbox","n":"number","items":"multitext"}}"#).unwrap();
        fs::write(dir.path().join("a.md"), "---\nwhen: 2024-01-01\n---\nBody").unwrap();
        let snapshot = load(&storage);
        let types = &snapshot.property_types;
        assert_eq!(types.declared_type("when"), Some("text"));
        assert!(
            types
                .validate("when", &Value::String("2024-01-01".into()))
                .is_ok()
        );
        assert!(
            types
                .validate("done", &Value::String("true".into()))
                .is_err()
        );
        assert!(types.validate("done", &Value::Bool(false)).is_ok());
        assert!(
            types
                .validate("day", &Value::String("2024-02-30".into()))
                .is_err()
        );
        assert!(types.validate("n", &Value::Number(0.into())).is_ok());
        assert!(
            types
                .validate(
                    "items",
                    &Value::Sequence(vec![Value::String("[[A]]".into())])
                )
                .is_ok()
        );
        assert!(snapshot.file(".obsidian/types.json").is_none());
    }

    #[test]
    fn cancelled_metadata_load_installs_nothing() {
        let (_dir, storage) = vault();
        assert!(storage.load_vault_metadata(&AtomicBool::new(true)).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_cannot_expand_dataset_or_read_private_metadata() {
        use std::os::unix::fs::symlink;
        let (dir, storage) = vault();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("x.md"), "private").unwrap();
        fs::write(dir.path().join("secret.clin"), "ciphertext").unwrap();
        symlink(outside.path(), dir.path().join("escape")).unwrap();
        symlink(dir.path(), dir.path().join("loop")).unwrap();
        symlink(dir.path().join("secret.clin"), dir.path().join("fake.md")).unwrap();
        assert!(load(&storage).is_empty());
        fs::create_dir(dir.path().join(".obsidian")).unwrap();
        symlink(
            outside.path().join("x.md"),
            dir.path().join(".obsidian/types.json"),
        )
        .unwrap();
        assert!(
            storage
                .load_vault_metadata(&AtomicBool::new(false))
                .is_err()
        );
    }

    #[test]
    #[ignore = "5,000-note release benchmark; run with --release --ignored --nocapture"]
    fn metadata_5000_notes_benchmark() {
        let (dir, storage) = vault();
        for n in 0..5000 {
            let mut body = format!(
                "---\ntitle: Note {n}\nprice: {}\ndone: false\nday: 2024-01-01\ntags: [work, bench/group]\n",
                n % 50
            );
            for property in 0..15 {
                body.push_str(&format!("property{property}: value{property}\n"));
            }
            body.push_str("---\n# Heading\n");
            for link in 1..=10 {
                body.push_str(&format!("[[note{}]] ", (n + link) % 5000));
            }
            body.push_str(&"Body text. ".repeat(900));
            fs::write(dir.path().join(format!("note{n}.md")), body).unwrap();
        }
        let started = std::time::Instant::now();
        let snapshot = load(&storage);
        let elapsed = started.elapsed();
        assert_eq!(snapshot.len(), 5000);
        assert_eq!(snapshot.file("note0.md").unwrap().backlinks.len(), 10);
        eprintln!("5,000-note metadata build: {elapsed:?}");
    }
}
