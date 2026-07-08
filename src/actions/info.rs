use crate::actions::{Action, ActionCategory};
use crate::app::App;
use anyhow::Result;
use std::borrow::Cow;

pub struct ShowInfoAction;

impl Action for ShowInfoAction {
    fn id(&self) -> Cow<'static, str> {
        Cow::Borrowed("info.show")
    }

    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("Show Info")
    }

    fn description(&self) -> Cow<'static, str> {
        Cow::Borrowed("Show detailed metrics for selected file or folder")
    }

    fn category(&self) -> ActionCategory {
        ActionCategory::General
    }

    fn glyph(&self) -> (&'static str, &'static str) {
        ("\u{f05a}", "ℹ️")
    }

    fn execute(&self, app: &mut App, _context_note_id: Option<&str>) -> Result<()> {
        // Notes
        if let Some(note_id) = app.get_selected_note_id() {
            let note = app.storage.load_note(&note_id)?;
            let summary = app
                .summary_cache
                .get(&note_id)
                .cloned()
                .unwrap_or_else(|| app.storage.load_note_summary(&note_id).unwrap());

            let chars = note.content.chars().count();
            let lines = note.content.lines().count();
            let words = note.content.split_whitespace().count();
            let reading_time_mins = (words as f64 / 200.0).ceil() as usize;

            // Top Words Algorithm (no regex dependency)
            let stop_words: std::collections::HashSet<&str> = vec![
                "the", "and", "a", "an", "is", "it", "to", "in", "of", "for", "on", "with",
                "that", "this", "as", "by", "at", "but", "not", "be", "are", "or", "from",
                "was", "we", "you", "i",
            ]
            .into_iter()
            .collect();

            let mut word_counts = std::collections::HashMap::new();
            for word in note.content.split(|c: char| !c.is_alphabetic()) {
                let w = word.to_lowercase();
                if w.len() > 1 && !stop_words.contains(w.as_str()) {
                    *word_counts.entry(w).or_insert(0) += 1;
                }
            }
            let mut top_words: Vec<_> = word_counts.into_iter().collect();
            top_words.sort_by(|a, b| b.1.cmp(&a.1));
            let top_5: Vec<String> = top_words
                .into_iter()
                .take(5)
                .map(|(w, c)| format!("\"{}\" ({})", w, c))
                .collect();

            let size_kb = summary.size_bytes as f64 / 1024.0;
            let modified = crate::ui::format_date(summary.updated_at, &app.date_format);

            let lines_vec = vec![
                format!("Total words: {}", words),
                format!("Total characters: {}", chars),
                format!("Total lines: {}", lines),
                format!("Estimated reading time: ~{} min", reading_time_mins),
                format!("Top words: {}", top_5.join(", ")),
                String::new(),
                format!("Size: {:.1} KB", size_kb),
                format!("Modified: {}", modified),
                format!("Tags: {}", summary.tags.len()),
                format!("Links: {}", summary.links.len()),
            ];

            app.popups.active = Some(crate::popups::ActivePopup::Info(crate::popups::InfoPopup {
                title: format!("Info: {}", note.title),
                lines: lines_vec,
            }));
            return Ok(());
        }

        // Folders
        if let Some(folder_path) = app.get_selected_folder_path() {
            let mut total_notes = 0;
            let mut total_size = 0;
            let mut latest_mtime = 0;
            let mut latest_title = String::new();

            let prefix = format!("{}/", folder_path);
            for summary in app.summary_cache.values() {
                if summary.folder == folder_path || summary.folder.starts_with(&prefix) {
                    total_notes += 1;
                    total_size += summary.size_bytes;
                    if summary.updated_at > latest_mtime {
                        latest_mtime = summary.updated_at;
                        latest_title = summary.title.clone();
                    }
                }
            }

            let size_kb = total_size as f64 / 1024.0;
            let modified = if latest_mtime > 0 {
                crate::ui::format_date(latest_mtime, &app.date_format)
            } else {
                "N/A".to_string()
            };

            let lines_vec = vec![
                format!("Total notes: {}", total_notes),
                format!("Total size: {:.1} KB", size_kb),
                String::new(),
                "Most recently modified:".to_string(),
                format!("  \"{}\" ({})", latest_title, modified),
            ];

            let folder_name = folder_path.split('/').last().unwrap_or(&folder_path);
            app.popups.active = Some(crate::popups::ActivePopup::Info(crate::popups::InfoPopup {
                title: format!("Info: {}", folder_name),
                lines: lines_vec,
            }));
            return Ok(());
        }

        app.set_temporary_status_static("Select a note or folder first");
        Ok(())
    }
}
