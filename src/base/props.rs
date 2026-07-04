use crate::frontmatter::Frontmatter;
use crate::storage::Storage;
use anyhow::Result;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct FileProps {
    pub name: String,
    pub basename: String,
    pub path: String,
    pub folder: String,
    pub ext: String,
    pub size: u64,
    pub ctime: i64,
    pub mtime: i64,
    pub tags: Vec<String>,
    pub links: Vec<String>,
    pub properties: BTreeMap<String, serde_yaml_ng::Value>,
}

impl FileProps {
    pub fn from_storage(storage: &Storage, id: &str, fm: &Frontmatter) -> Result<Self> {
        let path_buf = storage.note_path(id);
        let p = std::path::Path::new(id);
        let name = p
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let basename = p
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let folder = p
            .parent()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let ext = p
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        let (size, mtime, ctime) = if let Ok(meta) = std::fs::metadata(&path_buf) {
            let size = meta.len();
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            let ctime = meta
                .created()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64)
                .unwrap_or(mtime);
            (size, mtime, ctime)
        } else {
            (0, 0, 0)
        };

        let links = fm.links.clone().unwrap_or_default();
        let properties = fm.extra.clone();

        Ok(Self {
            name,
            basename,
            path: id.to_string(),
            folder,
            ext,
            size,
            ctime,
            mtime,
            tags: fm.tags.clone(),
            links,
            properties,
        })
    }
}
