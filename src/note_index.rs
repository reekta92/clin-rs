use crate::config::structs::CustomSmartFolder;
use crate::storage::NoteSummary;
use chrono::{Local, NaiveDate, TimeZone};
use std::collections::HashMap;
use std::sync::Arc;

pub struct NoteIndex {
    pub revision: u64,
    pub min_membership_expiry: Option<u64>,
    pub by_id: HashMap<Arc<str>, usize>,
    pub pinned_indices: Vec<usize>,
    pub activity_by_day: HashMap<NaiveDate, usize>,
}

pub(crate) fn matches_custom_rule(note: &NoteSummary, rule: &CustomSmartFolder, now: u64) -> bool {
    rule.tags.iter().all(|tag| note.tags.contains(tag))
        && rule
            .title_contains
            .as_ref()
            .is_none_or(|query| note.title.to_lowercase().contains(&query.to_lowercase()))
        && rule
            .folder_prefix
            .as_ref()
            .is_none_or(|prefix| note.folder.starts_with(prefix))
        && rule
            .updated_within_days
            .is_none_or(|days| now.saturating_sub(note.updated_at) < days.saturating_mul(86_400))
}

impl NoteIndex {
    pub fn build(
        revision: u64,
        notes: &[NoteSummary],
        custom_rules: &[CustomSmartFolder],
        now_unix_secs: u64,
        features: &crate::config::FeaturesConfig,
    ) -> Self {
        let mut index = Self {
            revision,
            min_membership_expiry: None,
            by_id: HashMap::new(),
            pinned_indices: Vec::new(),
            activity_by_day: HashMap::new(),
        };
        for (i, note) in notes
            .iter()
            .enumerate()
            .filter(|(_, note)| features.file_view_enabled(&note.id))
        {
            index.by_id.insert(Arc::from(note.id.as_str()), i);
            if note.pinned {
                index.pinned_indices.push(i);
            }
            if features.calendar.is_enabled()
                && let Some(date_time) = Local.timestamp_opt(note.updated_at as i64, 0).single()
            {
                *index
                    .activity_by_day
                    .entry(date_time.date_naive())
                    .or_default() += 1;
            }
            if features.smart_folders.is_enabled() {
                let mut expire_after = |seconds| {
                    if let Some(expiry) = note
                        .updated_at
                        .checked_add(seconds)
                        .filter(|expiry| *expiry > now_unix_secs)
                    {
                        index.min_membership_expiry = Some(
                            index
                                .min_membership_expiry
                                .map_or(expiry, |old| old.min(expiry)),
                        );
                    }
                };
                expire_after(86_400);
                expire_after(604_800);
                // Custom smart folders
                for rule in custom_rules {
                    if matches_custom_rule(note, rule, now_unix_secs)
                        && let Some(days) = rule.updated_within_days
                    {
                        expire_after(days.saturating_mul(86_400));
                    }
                }
            }
        }
        index
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
            },
        ];
        let index = NoteIndex::build(
            1,
            &notes,
            &[],
            now,
            &crate::config::FeaturesConfig::default(),
        );

        assert_eq!(index.by_id.len(), 2);
        assert_eq!(index.by_id.get("folder1/a.md").copied(), Some(0));
        assert_eq!(index.pinned_indices, vec![0]);
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
        }];
        let with_cal = NoteIndex::build(
            1,
            &notes,
            &[],
            now,
            &crate::config::FeaturesConfig::default(),
        );
        let without_cal = NoteIndex::build(
            1,
            &notes,
            &[],
            now,
            &crate::config::FeaturesConfig {
                calendar: crate::config::FeatureState::Disabled,
                ..Default::default()
            },
        );
        let today = Local
            .timestamp_opt(now as i64, 0)
            .single()
            .unwrap()
            .date_naive();
        assert_eq!(with_cal.activity_by_day, HashMap::from([(today, 1)]));
        assert!(without_cal.activity_by_day.is_empty());
        // Today/week expiry is computed regardless of the calendar flag.
        assert_eq!(without_cal.min_membership_expiry, Some(now + 86_400));
    }

    #[test]
    fn expiry_tracks_builtins_and_matching_custom_rules_at_the_boundary() {
        let now = 1_700_000_000;
        let mut note = NoteSummary {
            id: "work/note.md".into(),
            title: "Project".into(),
            folder: "work".into(),
            updated_at: now,
            tags: vec!["rust".into()],
            pinned: false,
            links: Vec::new(),
            size_bytes: 0,
        };
        let mut rule = CustomSmartFolder {
            name: "Projects".into(),
            tags: vec!["rust".into()],
            title_contains: Some("PROJECT".into()),
            folder_prefix: Some("work".into()),
            updated_within_days: Some(10),
        };
        let features = crate::config::FeaturesConfig::default();
        let expiry = |note: &NoteSummary, rule: &CustomSmartFolder, features: &_| {
            NoteIndex::build(
                1,
                std::slice::from_ref(note),
                std::slice::from_ref(rule),
                now,
                features,
            )
            .min_membership_expiry
        };
        assert_eq!(expiry(&note, &rule, &features), Some(now + 86_400));
        note.updated_at = now - 86_400;
        assert_eq!(expiry(&note, &rule, &features), Some(now + 6 * 86_400));
        note.updated_at = now - 7 * 86_400;
        assert_eq!(expiry(&note, &rule, &features), Some(now + 3 * 86_400));
        rule.tags.push("missing".into());
        assert_eq!(expiry(&note, &rule, &features), None);
        rule.tags.pop();
        note.updated_at = now - 10 * 86_400;
        assert!(!matches_custom_rule(&note, &rule, now));
        assert_eq!(expiry(&note, &rule, &features), None);
        rule.updated_within_days = Some(u64::MAX);
        assert!(matches_custom_rule(&note, &rule, now));
        assert_eq!(expiry(&note, &rule, &features), None);
        note.updated_at = now;
        let disabled = crate::config::FeaturesConfig {
            smart_folders: crate::config::FeatureState::Disabled,
            ..features
        };
        assert_eq!(expiry(&note, &rule, &disabled), None);
    }
}
