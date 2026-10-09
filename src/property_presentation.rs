use crate::app::App;
use crate::list_view::{SmartFolderKind, SortOrder, VisualItem};
use crate::property_model::{PropertyDefinitions, PropertyValue, compare_values};
use crate::storage::NoteSummary;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

const GROUP_PREFIX: &str = "__property_group__:";
fn sortable<'a>(
    note: &'a NoteSummary,
    key: &str,
    definitions: &PropertyDefinitions,
) -> Option<&'a PropertyValue> {
    let value = note.properties.get(key)?;
    if matches!(
        value,
        PropertyValue::Null | PropertyValue::Unsupported | PropertyValue::List(_)
    ) {
        return None;
    }
    if definitions
        .properties
        .get(key)
        .is_some_and(|definition| !definition.validate_indexed(value))
    {
        return None;
    }
    Some(value)
}
fn rank(value: &PropertyValue) -> u8 {
    match value {
        PropertyValue::Bool(_) => 0,
        PropertyValue::Integer(_) | PropertyValue::Unsigned(_) | PropertyValue::Number(_) => 1,
        PropertyValue::String(_) => 2,
        _ => 3,
    }
}
pub(crate) fn property_sort(
    a: &NoteSummary,
    b: &NoteSummary,
    key: Option<&str>,
    definitions: &PropertyDefinitions,
    order: SortOrder,
) -> Ordering {
    let Some(key) = key else {
        return a.id.cmp(&b.id);
    };
    let (a_value, b_value) = (sortable(a, key, definitions), sortable(b, key, definitions));
    match (a_value, b_value) {
        (Some(a_value), Some(b_value)) => {
            let comparison = rank(a_value).cmp(&rank(b_value)).then_with(|| {
                compare_values(
                    a_value,
                    b_value,
                    definitions
                        .properties
                        .get(key)
                        .map(|definition| definition.kind),
                )
                .unwrap_or(Ordering::Equal)
            });
            (if order == SortOrder::Descending {
                comparison.reverse()
            } else {
                comparison
            })
            .then_with(|| a.id.cmp(&b.id))
        }
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.id.cmp(&b.id),
    }
}
fn group_values<'a>(
    note: &'a NoteSummary,
    key: &str,
    definitions: &PropertyDefinitions,
) -> Vec<Option<&'a PropertyValue>> {
    let Some(value) = note.properties.get(key) else {
        return vec![None];
    };
    if definitions
        .properties
        .get(key)
        .is_some_and(|definition| !definition.validate_indexed(value))
    {
        return vec![None];
    }
    match value {
        PropertyValue::Null | PropertyValue::Unsupported => vec![None],
        PropertyValue::List(values) if values.is_empty() => vec![None],
        PropertyValue::List(values) => {
            let values: Vec<_> = values
                .iter()
                .filter(|value| {
                    !matches!(
                        value,
                        PropertyValue::Null | PropertyValue::Unsupported | PropertyValue::List(_)
                    )
                })
                .map(Some)
                .collect();
            if values.is_empty() {
                vec![None]
            } else {
                values
            }
        }
        _ => vec![Some(value)],
    }
}
impl App {
    pub(crate) fn apply_property_grouping(&mut self) {
        let Some(key) = self.config.list.property_group_key.as_deref() else {
            return;
        };
        // ponytail: O(n) grouping at list rebuild; add postings only if large-vault profiling needs them.
        let mut groups: BTreeMap<String, (String, BTreeSet<usize>)> = BTreeMap::new();
        for (index, note) in self.visible_notes() {
            if !matches!(
                std::path::Path::new(&note.id)
                    .extension()
                    .and_then(|ext| ext.to_str()),
                Some("md" | "txt" | "clin")
            ) {
                continue;
            }
            for value in group_values(note, key, &self.property_definitions) {
                let encoded = serde_json::to_string(&(key, value)).unwrap_or_default();
                let label = value.map_or_else(
                    || "(ungrouped)".into(),
                    |value| match value {
                        PropertyValue::String(text) => text.clone(),
                        _ => format!(
                            "{} ({})",
                            value.display(),
                            match value {
                                PropertyValue::Bool(_) => "Boolean",
                                _ => "Number",
                            }
                        ),
                    },
                );
                groups
                    .entry(encoded)
                    .or_insert_with(|| (label, BTreeSet::new()))
                    .1
                    .insert(index);
            }
        }
        let original = std::mem::take(&mut self.list.visual_list);
        let mut visual = Vec::new();
        let mut in_smart_folder = false;
        for item in original {
            match &item {
                VisualItem::SmartFolder {
                    kind: SmartFolderKind::Custom(name),
                    ..
                } if name.starts_with(GROUP_PREFIX) => {
                    in_smart_folder = false;
                }
                VisualItem::SmartFolder { .. } => {
                    in_smart_folder = true;
                    visual.push(item);
                }
                VisualItem::Note {
                    summary_idx, depth, ..
                } => {
                    if in_smart_folder && *depth > 0
                        || self.notes.get(*summary_idx).is_some_and(|note| {
                            !matches!(
                                std::path::Path::new(&note.id)
                                    .extension()
                                    .and_then(|ext| ext.to_str()),
                                Some("md" | "txt" | "clin")
                            )
                        })
                    {
                        visual.push(item);
                    } else {
                        in_smart_folder = false;
                    }
                }
                VisualItem::Folder { .. } => {
                    in_smart_folder = false;
                }
                _ => {
                    in_smart_folder = false;
                    visual.push(item);
                }
            }
        }
        let mut groups: Vec<_> = groups.into_iter().collect();
        groups.sort_by(|(a_key, (a_label, _)), (b_key, (b_label, _))| {
            a_label.cmp(b_label).then_with(|| a_key.cmp(b_key))
        });
        for (encoded, (label, indices)) in groups {
            let kind = SmartFolderKind::Custom(format!("{GROUP_PREFIX}{encoded}"));
            let path = kind.virtual_path();
            let expanded = self.list.folder_expanded.contains(&path);
            visual.push(VisualItem::SmartFolder {
                kind,
                label: format!("{key}: {label}"),
                note_count: indices.len(),
                depth: 0,
                is_expanded: expanded,
            });
            if expanded {
                visual.extend(indices.into_iter().map(|summary_idx| VisualItem::Note {
                    summary_idx,
                    depth: 1,
                    is_clin: self.notes[summary_idx].id.ends_with(".clin"),
                    is_draw: false,
                    is_canvas: false,
                    in_virtual_pinned_folder: false,
                }));
            }
        }
        self.list.visual_list = visual;
    }
    pub(crate) fn property_group_members(&self, name: &str) -> Option<Vec<String>> {
        let encoded = name.strip_prefix(GROUP_PREFIX)?;
        let (key, value): (String, Option<PropertyValue>) = serde_json::from_str(encoded).ok()?;
        Some(
            self.visible_notes()
                .filter(|(_, note)| {
                    matches!(
                        std::path::Path::new(&note.id)
                            .extension()
                            .and_then(|ext| ext.to_str()),
                        Some("md" | "txt" | "clin")
                    ) && group_values(note, &key, &self.property_definitions)
                        .into_iter()
                        .any(|candidate| candidate == value.as_ref())
                })
                .map(|(_, note)| note.id.clone())
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_values_stay_last_in_both_directions() {
        let note = |id: &str, value: Option<PropertyValue>| NoteSummary {
            id: id.into(),
            title: id.into(),
            updated_at: 0,
            folder: String::new(),
            tags: Vec::new(),
            pinned: false,
            links: Vec::new(),
            size_bytes: 0,
            properties: value
                .map(|value| BTreeMap::from([("priority".into(), value)]))
                .unwrap_or_default(),
            property_links: Vec::new(),
        };
        let valid = note("valid.md", Some(PropertyValue::Integer(2)));
        let missing = note("missing.md", None);
        for order in [SortOrder::Ascending, SortOrder::Descending] {
            assert_eq!(
                property_sort(
                    &valid,
                    &missing,
                    Some("priority"),
                    &PropertyDefinitions::default(),
                    order
                ),
                Ordering::Less
            );
        }
    }
}
