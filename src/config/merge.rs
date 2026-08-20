use serde::{Deserialize, Serialize};

use super::structs::{
    ClinConfig, FilterConfig, InteractionConfig, PhysicsConfig, SearchConfig, UiConfig,
    VisualConfig,
};

/// Merge a serialized `toml_edit::Item` into an existing one, preserving comments/decor.
pub fn merge_edit_item(dst: &mut toml_edit::Item, src: toml_edit::Item) {
    if let (Some(dst_tbl), Some(src_tbl)) = (dst.as_table_mut(), src.as_table()) {
        let keys: Vec<String> = dst_tbl.iter().map(|(k, _)| k.to_string()).collect();
        for k in keys {
            if !src_tbl.contains_key(&k) {
                dst_tbl.remove(&k);
            }
        }
        for (k, v) in src_tbl {
            match dst_tbl.get_mut(k) {
                Some(d) => merge_edit_item(d, v.clone()),
                None => {
                    dst_tbl.insert(k, v.clone());
                }
            }
        }
    } else {
        // preserve decor on the replaced value
        let decor = match dst {
            toml_edit::Item::Value(v) => Some(v.decor().clone()),
            toml_edit::Item::Table(t) => Some(t.decor().clone()),
            _ => None,
        };
        *dst = src;
        if let Some(d) = decor {
            match dst {
                toml_edit::Item::Value(v) => *v.decor_mut() = d,
                toml_edit::Item::Table(t) => *t.decor_mut() = d,
                _ => {}
            }
        }
    }
}

/// Helper struct used during migration from old `graf.toml`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct GrafConfigOnly {
    #[serde(default)]
    pub visual: VisualConfig,
    #[serde(default)]
    pub physics: PhysicsConfig,
    #[serde(default)]
    pub interaction: InteractionConfig,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub filter: FilterConfig,
    #[serde(default)]
    pub search: SearchConfig,
}

/// Complete config generated from the current runtime defaults.
pub fn default_config_content() -> String {
    format!(
        "# Clin configuration.\n# Full reference: docs/CONFIG_REFERENCE.md\n\n{}",
        toml::to_string_pretty(&ClinConfig::default()).unwrap_or_default()
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn default_config_content_roundtrips() {
        let content = super::default_config_content();
        assert!(content.starts_with("# Clin configuration."));
        let parsed: crate::config::ClinConfig = toml::from_str(&content).unwrap();
        let default = crate::config::ClinConfig::default();
        assert_eq!(parsed.core, default.core);
        assert_eq!(parsed.ui, default.ui);
        assert_eq!(parsed.list, default.list);
        assert_eq!(parsed.editor, default.editor);
        assert_eq!(parsed.graf, default.graf);
        assert_eq!(parsed.goals, default.goals);
        assert_eq!(parsed.image, default.image);
        assert_eq!(parsed.backup, default.backup);
        assert_eq!(parsed.statusline, default.statusline);
    }
}
