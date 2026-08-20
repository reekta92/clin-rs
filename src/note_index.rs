use crate::config::structs::CustomSmartFolder;
use crate::storage::NoteSummary;
use chrono::{Datelike, Local, NaiveDate, TimeZone};
use std::collections::{BTreeSet, HashMap, HashSet};
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
    ) -> Self {
        let canonical_ids: Arc<[Arc<str>]> = notes
            .iter()
            .map(|n| Arc::from(n.id.as_str()))
            .collect::<Vec<_>>()
            .into_boxed_slice()
            .into();

        let mut by_id = HashMap::with_capacity(notes.len());

        for (i, (id_arc, _)) in canonical_ids.iter().zip(notes.iter()).enumerate() {
            by_id.insert(id_arc.clone(), i);
        }

        let mut notes_by_folder: HashMap<String, Vec<usize>> = HashMap::new();
        let mut notes_by_exact_tag: HashMap<String, Vec<usize>> = HashMap::new();
        let mut pinned_indices = Vec::new();
        let mut untagged_indices = Vec::new();
        let mut today_indices = Vec::new();
        let mut this_week_indices = Vec::new();
        let mut activity_by_day: HashMap<NaiveDate, usize> = HashMap::new();

        let now_local = Local
            .timestamp_opt(now_unix_secs as i64, 0)
            .single()
            .unwrap_or_else(Local::now);

        let today_date = now_local.date_naive();
        let days_since_mon = now_local.weekday().num_days_from_monday() as i64;
        let mon_date = today_date - chrono::Duration::days(days_since_mon);

        for (i, note) in notes.iter().enumerate() {
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

            if let Some(date_time) = Local.timestamp_opt(note.updated_at as i64, 0).single() {
                let note_date = date_time.date_naive();
                *activity_by_day.entry(note_date).or_default() += 1;

                if note_date == today_date {
                    today_indices.push(i);
                }
                if note_date >= mon_date {
                    this_week_indices.push(i);
                }
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
        for note in notes {
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
        let day_secs = 86400u64;

        // Custom smart folders
        let mut custom_smart_folder_indices = HashMap::new();
        for rule in custom_rules {
            let mut candidate_set: Option<HashSet<usize>> = None;

            if !rule.tags.is_empty() {
                let mut smallest_posting: Option<&Vec<usize>> = None;
                for tag in &rule.tags {
                    let posting = notes_by_exact_tag.get(tag);
                    match (smallest_posting, posting) {
                        (None, p) => smallest_posting = p,
                        (Some(cur), Some(p)) if p.len() < cur.len() => smallest_posting = Some(p),
                        _ => {}
                    }
                }
                if let Some(posting) = smallest_posting {
                    candidate_set = Some(posting.iter().copied().collect());
                } else {
                    candidate_set = Some(HashSet::new());
                }
            }

            let mut matched = Vec::new();
            let indices_to_check: Vec<usize> = match candidate_set {
                Some(set) => (0..notes.len()).filter(|i| set.contains(i)).collect(),
                None => (0..notes.len()).collect(),
            };

            for i in indices_to_check {
                let note = &notes[i];

                if !rule.tags.is_empty() && !rule.tags.iter().all(|t| note.tags.contains(t)) {
                    continue;
                }

                if let Some(ref title_query) = rule.title_contains
                    && !note
                        .title
                        .to_lowercase()
                        .contains(&title_query.to_lowercase())
                {
                    continue;
                }

                if let Some(ref folder_prefix) = rule.folder_prefix
                    && !note.folder.starts_with(folder_prefix)
                {
                    continue;
                }

                if let Some(days) = rule.updated_within_days {
                    let cutoff = now_unix_secs.saturating_sub(days * day_secs);
                    if note.updated_at < cutoff {
                        continue;
                    }
                    let expiry = note.updated_at + (days * day_secs);
                    if expiry > now_unix_secs {
                        min_membership_expiry = Some(
                            min_membership_expiry
                                .map(|m| m.min(expiry))
                                .unwrap_or(expiry),
                        );
                    }
                }

                matched.push(i);
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
        let folders = vec!["folder1".to_string(), "folder1/sub".to_string()];
        let index = NoteIndex::build(1, &notes, &folders, &[], now);

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
}
