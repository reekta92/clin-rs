use serde::{Deserialize, Serialize};

use super::structs::{
    ClinConfig, FilterConfig, InteractionConfig, PhysicsConfig, SearchConfig, UiConfig,
    VisualConfig,
};

fn extract_decor(item: &toml_edit::Item) -> Option<toml_edit::Decor> {
    match item {
        toml_edit::Item::Value(v) => Some(v.decor().clone()),
        toml_edit::Item::Table(t) => Some(t.decor().clone()),
        _ => None,
    }
}

/// Merge a `toml::Value` into a `toml_edit::Item`, preserving comments/decor.
pub fn merge_toml_value(edit_item: &mut toml_edit::Item, toml_val: &toml::Value) {
    match toml_val {
        toml::Value::Table(toml_tbl) => {
            if !edit_item.is_table() {
                let decor = extract_decor(edit_item);
                let mut new_table = toml_edit::Table::new();
                if let Some(d) = decor {
                    *new_table.decor_mut() = d;
                }
                *edit_item = toml_edit::Item::Table(new_table);
            }
            if let Some(edit_tbl) = edit_item.as_table_mut() {
                let keys_to_remove: Vec<String> = edit_tbl
                    .iter()
                    .map(|(k, _)| k.to_string())
                    .filter(|k| !toml_tbl.contains_key(k))
                    .collect();
                for k in keys_to_remove {
                    edit_tbl.remove(&k);
                }

                for (k, v) in toml_tbl {
                    if let Some(edit_item) = edit_tbl.get_mut(k) {
                        merge_toml_value(edit_item, v);
                    } else {
                        let new_item = toml_value_to_item(v);
                        edit_tbl.insert(k, new_item);
                    }
                }
            }
        }
        toml::Value::Array(toml_arr) => {
            let is_existing_aot = matches!(edit_item, toml_edit::Item::ArrayOfTables(_));
            let is_new_aot = toml_arr.iter().any(|v| v.is_table());
            if is_existing_aot || is_new_aot {
                let mut new_aot = toml_edit::ArrayOfTables::new();
                for val in toml_arr {
                    if let toml_edit::Item::Table(t) = toml_value_to_item(val) {
                        new_aot.push(t);
                    }
                }
                *edit_item = toml_edit::Item::ArrayOfTables(new_aot);
            } else {
                let decor = extract_decor(edit_item);
                let mut edit_arr = toml_edit::Array::new();
                for val in toml_arr {
                    edit_arr.push(
                        toml_value_to_item(val)
                            .as_value()
                            .expect("toml_value_to_item for non-table/non-array returns value")
                            .clone(),
                    );
                }
                let mut new_item = toml_edit::Item::Value(toml_edit::Value::Array(edit_arr));
                if let Some(d) = decor
                    && let Some(v) = new_item.as_value_mut()
                {
                    *v.decor_mut() = d;
                }
                *edit_item = new_item;
            }
        }
        _ => {
            let decor = extract_decor(edit_item);
            let mut new_item = toml_value_to_item(toml_val);
            if let Some(d) = decor {
                match &mut new_item {
                    toml_edit::Item::Value(v) => *v.decor_mut() = d,
                    toml_edit::Item::Table(t) => *t.decor_mut() = d,
                    _ => {}
                }
            }
            *edit_item = new_item;
        }
    }
}

/// Convert a `toml::Value` into `toml_edit::Item`.
pub fn toml_value_to_item(v: &toml::Value) -> toml_edit::Item {
    match v {
        toml::Value::String(s) => toml_edit::value(s),
        toml::Value::Integer(i) => toml_edit::value(*i),
        toml::Value::Float(f) => toml_edit::value(*f),
        toml::Value::Boolean(b) => toml_edit::value(*b),
        toml::Value::Datetime(dt) => toml_edit::value(dt.to_string()),
        toml::Value::Array(arr) => {
            if arr.iter().any(|v| v.is_table()) {
                let mut edit_aot = toml_edit::ArrayOfTables::new();
                for val in arr {
                    if let toml_edit::Item::Table(t) = toml_value_to_item(val) {
                        edit_aot.push(t);
                    } else {
                        panic!("Expected table in array of tables");
                    }
                }
                toml_edit::Item::ArrayOfTables(edit_aot)
            } else {
                let mut edit_arr = toml_edit::Array::new();
                for val in arr {
                    edit_arr.push(
                        toml_value_to_item(val)
                            .as_value()
                            .expect("toml_value_to_item for non-table/non-array returns value")
                            .clone(),
                    );
                }
                toml_edit::Item::Value(toml_edit::Value::Array(edit_arr))
            }
        }
        toml::Value::Table(tbl) => {
            let mut edit_tbl = toml_edit::Table::new();
            for (k, v) in tbl {
                edit_tbl.insert(k, toml_value_to_item(v));
            }
            toml_edit::Item::Table(edit_tbl)
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
