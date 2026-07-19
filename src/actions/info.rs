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
            let summary = match app.notes.iter().find(|n| n.id == note_id) {
                Some(s) => s.clone(),
                None => app.storage.load_note_summary(&note_id)?,
            };

            let chars = note.content.chars().count();
            let lines = note.content.lines().count();
            let words = note.content.split_whitespace().count();
            let reading_time_mins = (words as f64 / 200.0).ceil() as usize;

            // Header count via outline parser (minus root node)
            let header_count = crate::outline::parse::parse_outline(&note.title, &note.content)
                .len()
                .saturating_sub(1);

            // Task count: lines matching - [ ], - [x], * [ ], * [x]
            let task_count = note
                .content
                .lines()
                .filter(|l| {
                    let t = l.trim_start();
                    t.starts_with("- [ ] ")
                        || t.starts_with("- [x] ")
                        || t.starts_with("* [ ] ")
                        || t.starts_with("* [x] ")
                })
                .count();

            // Top Words Algorithm (no regex dependency)
            let stop_words: std::collections::HashSet<&str> = vec![
                "the", "and", "a", "an", "is", "it", "to", "in", "of", "for", "on", "with", "that",
                "this", "as", "by", "at", "but", "not", "be", "are", "or", "from", "was", "we",
                "you", "i",
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
            top_words.sort_by_key(|b| std::cmp::Reverse(b.1));
            let top_5: Vec<String> = top_words
                .into_iter()
                .take(5)
                .map(|(w, c)| format!("\"{}\" ({})", w, c))
                .collect();

            let size_kb = summary.size_bytes as f64 / 1024.0;
            let modified = crate::ui::format_date(summary.updated_at, &app.date_format);

            use crate::popups::InfoItem;
            let items = vec![
                InfoItem::Metrics(vec![
                    ("Total words".to_string(), format!("{}", words)),
                    ("Characters".to_string(), format!("{}", chars)),
                    ("Lines".to_string(), format!("{}", lines)),
                    (
                        "Reading time".to_string(),
                        format!("~{} min", reading_time_mins),
                    ),
                    ("Headers".to_string(), format!("{}", header_count)),
                    ("Tasks".to_string(), format!("{}", task_count)),
                ]),
                InfoItem::Spacer,
                InfoItem::Metrics(vec![
                    ("Size".to_string(), format!("{:.1} KB", size_kb)),
                    ("Modified".to_string(), modified),
                    ("Tags".to_string(), format!("{}", summary.tags.len())),
                    ("Links".to_string(), format!("{}", summary.links.len())),
                ]),
                InfoItem::Spacer,
                InfoItem::Text {
                    heading: "Note ID / File Path".to_string(),
                    body: summary.id.clone(),
                },
                InfoItem::Text {
                    heading: "Folder Path".to_string(),
                    body: summary.folder.clone(),
                },
                InfoItem::Text {
                    heading: "Top Words".to_string(),
                    body: top_5.join(", "),
                },
            ];

            app.popups.active = Some(crate::popups::ActivePopup::Info(crate::popups::InfoPopup {
                title: format!("Info: {}", note.title),
                items,
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
            for summary in &app.notes {
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

            use crate::popups::InfoItem;
            let items = vec![
                InfoItem::Metrics(vec![
                    ("Total notes".to_string(), format!("{}", total_notes)),
                    ("Total size".to_string(), format!("{:.1} KB", size_kb)),
                ]),
                InfoItem::Spacer,
                InfoItem::Text {
                    heading: "Folder Path".to_string(),
                    body: folder_path.to_string(),
                },
                InfoItem::Text {
                    heading: "Most recently modified".to_string(),
                    body: format!("\"{}\" ({})", latest_title, modified),
                },
            ];

            let folder_name = folder_path.split('/').next_back().unwrap_or(&folder_path);
            app.popups.active = Some(crate::popups::ActivePopup::Info(crate::popups::InfoPopup {
                title: format!("Info: {}", folder_name),
                items,
            }));
            return Ok(());
        }

        app.set_temporary_status_static("Select a note or folder first");
        Ok(())
    }
}
