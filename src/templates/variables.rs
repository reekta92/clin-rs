use super::{RenderedTemplate, Template};
use crate::storage::Storage;
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub const FOLDER_TEMPLATE_CONFIG: &str = ".clin-template.toml";
pub const DEFAULT_FOLDER_TEMPLATE_CONFIG: &str =
    "inherit_parent = true\n\n[variables]\n# book = \"Example Book\"\n# chapter = \"01\"\n";
const BUILTIN_NAMES: [&str; 7] = [
    "date", "datetime", "time", "weekday", "year", "month", "day",
];

pub(super) fn is_variable_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn inherit_by_default() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FolderTemplateConfig {
    #[serde(default = "inherit_by_default")]
    pub inherit_parent: bool,
    #[serde(default)]
    pub variables: BTreeMap<String, String>,
}

impl FolderTemplateConfig {
    pub fn parse(content: &str) -> Result<Self> {
        let config: Self =
            toml::from_str(content).context("invalid folder template configuration")?;
        for name in config.variables.keys() {
            ensure!(
                is_variable_name(name),
                "invalid template variable name: {name:?}"
            );
            ensure!(
                !BUILTIN_NAMES.contains(&name.as_str()),
                "template variable {name:?} is reserved for a built-in"
            );
        }
        Ok(config)
    }
}

impl Storage {
    /// Presence only: malformed configs remain discoverable so they can be repaired.
    pub fn has_local_template_config(&self, folder: &str) -> bool {
        fs::symlink_metadata(self.notes_dir.join(folder).join(FOLDER_TEMPLATE_CONFIG))
            .is_ok_and(|metadata| metadata.is_file() || metadata.file_type().is_symlink())
    }

    /// Resolve a vault-relative destination, including folders not created yet.
    /// Existing ancestors and symlinks must resolve inside the vault.
    pub fn template_folder_path(&self, folder: &str) -> Result<PathBuf> {
        let root =
            fs::canonicalize(&self.notes_dir).context("failed to resolve vault directory")?;
        let mut path = root.clone();
        // Reject backslashes too: note IDs use slash-separated paths on every platform.
        ensure!(!folder.contains('\\'), "folder must use forward slashes");
        for component in Path::new(folder).components() {
            match component {
                Component::Normal(name) => path.push(name),
                Component::CurDir => continue,
                _ => bail!("folder must be relative to the vault, without '..'"),
            }
            match fs::symlink_metadata(&path) {
                Ok(_) => {
                    let resolved = fs::canonicalize(&path)
                        .with_context(|| format!("failed to resolve {}", path.display()))?;
                    ensure!(
                        resolved.starts_with(&root),
                        "folder escapes the vault: {}",
                        path.display()
                    );
                    ensure!(resolved.is_dir(), "not a folder: {}", path.display());
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    return Err(e).with_context(|| format!("failed to inspect {}", path.display()));
                }
            }
        }
        Ok(path)
    }

    pub fn folder_template_config_path(&self, folder: &str) -> Result<PathBuf> {
        let path = self
            .template_folder_path(folder)?
            .join(FOLDER_TEMPLATE_CONFIG);
        self.check_folder_template_config_path(&path)?;
        Ok(path)
    }

    pub(crate) fn check_folder_template_config_path(&self, path: &Path) -> Result<()> {
        let root =
            fs::canonicalize(&self.notes_dir).context("failed to resolve vault directory")?;
        ensure!(path.starts_with(&root), "template config escapes the vault");
        match fs::symlink_metadata(path) {
            Ok(metadata) => {
                // Editing an alias is surprising and atomic writes would replace the alias.
                ensure!(
                    !metadata.file_type().is_symlink(),
                    "folder template config cannot be a symlink: {}",
                    path.display()
                );
                ensure!(
                    metadata.is_file(),
                    "folder template config is not a file: {}",
                    path.display()
                );
                let resolved = fs::canonicalize(path)?;
                ensure!(
                    resolved.starts_with(root),
                    "template config escapes the vault: {}",
                    path.display()
                );
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Check the parent as well, including after an editor session.
                let parent = path.parent().context("template config has no parent")?;
                if parent.exists() {
                    ensure!(
                        fs::canonicalize(parent)?.starts_with(root),
                        "template config escapes the vault"
                    );
                }
            }
            Err(e) => {
                return Err(e).with_context(|| format!("failed to inspect {}", path.display()));
            }
        }
        Ok(())
    }

    pub fn folder_template_variables(&self, folder: &str) -> Result<BTreeMap<String, String>> {
        let root =
            fs::canonicalize(&self.notes_dir).context("failed to resolve vault directory")?;
        let mut current = self.template_folder_path(folder)?;
        let mut configs = Vec::new();
        loop {
            let path = current.join(FOLDER_TEMPLATE_CONFIG);
            self.check_folder_template_config_path(&path)?;
            match fs::read_to_string(&path) {
                Ok(content) => {
                    let config = FolderTemplateConfig::parse(&content)
                        .with_context(|| format!("failed to load {}", path.display()))?;
                    let inherit = config.inherit_parent;
                    configs.push(config.variables);
                    if !inherit {
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    return Err(e).with_context(|| format!("failed to read {}", path.display()));
                }
            }
            if current == root {
                break;
            }
            ensure!(current.pop(), "failed to find vault root");
        }
        let mut variables = BTreeMap::new();
        for config in configs.into_iter().rev() {
            variables.extend(config);
        }
        Ok(variables)
    }

    pub fn render_template_in_folder(
        &self,
        template: &Template,
        folder: &str,
    ) -> Result<RenderedTemplate> {
        Ok(template.render_with_variables(&self.folder_template_variables(folder)?))
    }
}
