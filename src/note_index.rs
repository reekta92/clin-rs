use crate::config::structs::CustomSmartFolder;
use crate::property_model::PropertyDefinitions;
use crate::storage::NoteSummary;
use chrono::{Local, NaiveDate, TimeZone};
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

pub struct NoteIndex {
    pub revision: u64,
    pub now_unix_secs: u64,
    pub min_membership_expiry: Option<u64>,
    pub canonical_ids: Arc<[Arc<str>]>,
    pub by_id: HashMap<Arc<str>, usize>,
    pub notes_by_folder: HashMap<String, Vec<usize>>,
    pub child_folders_by_parent: HashMap<String, Vec<String>>,
    pub recursive_note_counts: HashMap<String, usize>,
    pub notes_by_exact_tag: HashMap<String, Vec<usize>>,
    pub pinned_indices: Vec<usize>,
    pub today_indices: Vec<usize>,
    pub this_week_indices: Vec<usize>,
    pub untagged_indices: Vec<usize>,
    pub custom_smart_folder_indices: HashMap<String, Vec<usize>>,
    pub activity_by_day: HashMap<NaiveDate, usize>,
}

impl NoteIndex {
    pub fn build(
        revision: u64,
        notes: &[NoteSummary],
        folders: &[String],
        custom_rules: &[CustomSmartFolder],
        now_unix_secs: u64,
        features: &crate::config::FeaturesConfig,
        definitions: &PropertyDefinitions,
        calendar_date_property: Option<&str>,
    ) -> Self {
        let visible_notes: Vec<(usize, &NoteSummary)> = notes
            .iter()
            .enumerate()
            .filter(|(_, n)| features.file_view_enabled(&n.id))
            .collect();

        let canonical_ids: Arc<[Arc<str>]> = visible_notes
            .iter()
            .map(|(_, n)| Arc::from(n.id.as_str()))
            .collect::<Vec<_>>()
            .into_boxed_slice()
            .into();

        let mut by_id = HashMap::with_capacity(visible_notes.len());

        for ((idx, _), id_arc) in visible_notes.iter().zip(canonical_ids.iter()) {
            by_id.insert(id_arc.clone(), *idx);
        }

        let mut notes_by_folder: HashMap<String, Vec<usize>> = HashMap::new();
        let mut notes_by_exact_tag: HashMap<String, Vec<usize>> = HashMap::new();
        let mut pinned_indices = Vec::new();
        let mut untagged_indices = Vec::new();
        let mut today_indices = Vec::new();
        let mut this_week_indices = Vec::new();
        let mut activity_by_day: HashMap<NaiveDate, usize> = HashMap::new();
        for (i, note) in &visible_notes {
            let i = *i;
            notes_by_folder
                .entry(note.folder.clone())
                .or_default()
                .push(i);

            if note.pinned {
                pinned_indices.push(i);
            }

            if note.tags.is_empty() {
                untagged_indices.push(i);
            } else {
                for tag in &note.tags {
                    notes_by_exact_tag.entry(tag.clone()).or_default().push(i);
                }
            }

            if features.calendar.is_enabled() {
                let note_date = if let Some(key) = calendar_date_property {
                    definitions.date_value(key, &note.properties)
                } else {
                    Local
                        .timestamp_opt(note.updated_at as i64, 0)
                        .single()
                        .map(|dt| dt.date_naive())
                };
                if let Some(date) = note_date {
                    *activity_by_day.entry(date).or_default() += 1;
                }
            }

            if now_unix_secs.saturating_sub(note.updated_at) < 86_400 {
                today_indices.push(i);
            }
            if now_unix_secs.saturating_sub(note.updated_at) < 604_800 {
                this_week_indices.push(i);
            }
        }

        let mut all_folder_paths: BTreeSet<String> = folders.iter().cloned().collect();
        for note in notes {
            if !note.folder.is_empty() {
                let mut current = note.folder.as_str();
                while !current.is_empty() {
                    all_folder_paths.insert(current.to_string());
                    if let Some(slash) = current.rfind('/') {
                        current = &current[..slash];
                    } else {
                        break;
                    }
                }
            }
        }

        let mut child_folders_by_parent: HashMap<String, Vec<String>> = HashMap::new();
        for f in &all_folder_paths {
            let parent = if let Some(slash) = f.rfind('/') {
                &f[..slash]
            } else {
                ""
            };
            child_folders_by_parent
                .entry(parent.to_string())
                .or_default()
                .push(f.clone());
        }

        for children in child_folders_by_parent.values_mut() {
            children.sort();
        }

        let mut recursive_note_counts: HashMap<String, usize> = HashMap::new();
        for (_, note) in &visible_notes {
            let mut current = note.folder.as_str();
            loop {
                *recursive_note_counts
                    .entry(current.to_string())
                    .or_default() += 1;
                if current.is_empty() {
                    break;
                }
                if let Some(slash) = current.rfind('/') {
                    current = &current[..slash];
                } else {
                    current = "";
                }
            }
        }

        let mut min_membership_expiry: Option<u64> = None;
        let mut custom_smart_folder_indices = HashMap::new();
        for rule in custom_rules {
            let mut matched = Vec::new();
            for (i, note) in &visible_notes {
                if crate::property_query::custom_folder_matches(
                    rule,
                    note,
                    definitions,
                    now_unix_secs,
                ) {
                    matched.push(*i);
                    if let Some(days) = rule.updated_within_days {
                        let expiry = note.updated_at.saturating_add(days.saturating_mul(86_400));
                        if expiry > now_unix_secs {
                            min_membership_expiry =
                                Some(min_membership_expiry.map_or(expiry, |m| m.min(expiry)));
                        }
                    }
                }
            }
            custom_smart_folder_indices.insert(rule.name.clone(), matched);
        }

        NoteIndex {
            revision,
            now_unix_secs,
            min_membership_expiry,
            canonical_ids,
            by_id,
            notes_by_folder,
            child_folders_by_parent,
            recursive_note_counts,
            notes_by_exact_tag,
            pinned_indices,
            today_indices,
            this_week_indices,
            untagged_indices,
            custom_smart_folder_indices,
            activity_by_day,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::NoteSummary;

    #[test]
    fn note_index_matches_bruteforce() {
        let now = 1_700_000_000_u64;
        let notes = vec![
            NoteSummary {
                id: "folder1/a.md".to_string(),
                title: "A Note".to_string(),
                updated_at: now,
                folder: "folder1".to_string(),
                tags: vec!["rust".to_string()],
                pinned: true,
                links: vec![],
                size_bytes: 10,
                properties: Default::default(),
                property_links: Vec::new(),
            },
            NoteSummary {
                id: "folder1/sub/b.md".to_string(),
                title: "B Note".to_string(),
                updated_at: now - 86400,
                folder: "folder1/sub".to_string(),
                tags: vec!["cli".to_string()],
                pinned: false,
                links: vec![],
                size_bytes: 20,
                properties: Default::default(),
                property_links: Vec::new(),
            },
        ];
        let folders = vec!["folder1".to_string(), "folder1/sub".to_string()];
        let index = NoteIndex::build(
            1,
            &notes,
            &folders,
            &[],
            now,
            &crate::config::FeaturesConfig::default(),
            &PropertyDefinitions::default(),
            None,
        );
        assert_eq!(index.canonical_ids.len(), 2);
        assert_eq!(index.by_id.get("folder1/a.md").copied(), Some(0));
        assert_eq!(index.pinned_indices, vec![0]);
        assert_eq!(index.recursive_note_counts.get("folder1").copied(), Some(2));
        assert_eq!(
            index.recursive_note_counts.get("folder1/sub").copied(),
            Some(1)
        );
        assert_eq!(index.notes_by_exact_tag.get("rust").unwrap(), &vec![0]);
    }

    #[test]
    fn calendar_flag_controls_activity_map() {
        let now = 1_700_000_000;
        let notes = vec![NoteSummary {
            id: "a.md".to_string(),
            title: "A".to_string(),
            updated_at: now,
            folder: String::new(),
            tags: vec![],
            pinned: false,
            links: vec![],
            size_bytes: 1,
            properties: Default::default(),
            property_links: Vec::new(),
        }];
        let with_cal = NoteIndex::build(
            1,
            &notes,
            &[],
            &[],
            now,
            &crate::config::FeaturesConfig::default(),
            &PropertyDefinitions::default(),
            None,
        );
        let without_cal = NoteIndex::build(
            1,
            &notes,
            &[],
            &[],
            now,
            &crate::config::FeaturesConfig {
                calendar: crate::config::FeatureState::Disabled,
                ..Default::default()
            },
            &PropertyDefinitions::default(),
            None,
        );
        let today = Local
            .timestamp_opt(now as i64, 0)
            .single()
            .unwrap()
            .date_naive();
        assert_eq!(with_cal.activity_by_day, HashMap::from([(today, 1)]));
        assert!(without_cal.activity_by_day.is_empty());
        // Today/week indices are computed regardless of the calendar flag.
        assert_eq!(without_cal.today_indices, vec![0]);
    }

    #[test]
    fn calendar_custom_date_property_and_custom_smart_folder() {
        let now = 1_700_000_000;
        let mut props = std::collections::BTreeMap::new();
        props.insert(
            "due".to_string(),
            crate::property_model::PropertyValue::String("2025-01-15".to_string()),
        );
        props.insert(
            "status".to_string(),
            crate::property_model::PropertyValue::String("done".to_string()),
        );
        let notes = vec![NoteSummary {
            id: "task.md".to_string(),
            title: "Task".to_string(),
            updated_at: now,
            folder: String::new(),
            tags: vec![],
            pinned: false,
            links: vec![],
            size_bytes: 1,
            properties: props,
            property_links: Vec::new(),
        }];
        let custom_rule = CustomSmartFolder {
            name: "Done Tasks".to_string(),
            tags: vec![],
            title_contains: None,
            folder_prefix: None,
            updated_within_days: None,
            all: vec![
                crate::property_query::PropertyPredicate::new(
                    "status".into(),
                    crate::property_query::PropertyOperator::Eq,
                    Some(toml::Value::String("done".into())),
                )
                .unwrap(),
            ],
            any: None,
        };
        let mut definitions = PropertyDefinitions::default();
        definitions.properties.insert(
            "due".into(),
            crate::property_model::PropertyDefinition {
                kind: crate::property_model::PropertyKind::Date,
                ..Default::default()
            },
        );
        let index = NoteIndex::build(
            1,
            &notes,
            &[],
            &[custom_rule],
            now,
            &crate::config::FeaturesConfig::default(),
            &definitions,
            Some("due"),
        );
        let expected_date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();
        assert_eq!(index.activity_by_day, HashMap::from([(expected_date, 1)]));
        assert_eq!(
            index.custom_smart_folder_indices.get("Done Tasks"),
            Some(&vec![0])
        );
    }
}
