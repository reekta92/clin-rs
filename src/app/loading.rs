use super::*;
use crate::list_view::*;
use crate::storage::NoteSummary;
use std::borrow::Cow;
use std::collections::HashMap;
use std::time::{Duration, Instant};

struct SmartFolderData {
    kind: SmartFolderKind,
    label: String,
    matches: Vec<usize>,
}
impl App {
    pub fn refresh_visual_list(&mut self) {
        let mut visual = Vec::new();
        // Subnotes view cache — computed first (before any &self.notes borrow) to avoid conflict.
        let subnotes_cache = if self.subnotes_view_cache_sig
            == self.notes.len() * 31
                + self
                    .subnotes_view_cache
                    .iter()
                    .map(|(_, v)| v.len())
                    .sum::<usize>()
        {
            self.subnotes_view_cache.clone()
        } else {
            self.refresh_subnotes_view_cache();
            self.subnotes_view_cache.clone()
        };
        // Map parent_id -> summary_idx for title/icon/action lookup. Try exact id
        // first; fall back to matching by file stem so subnotes attached before
        // the id-migration fix still resolve after a title or folder change.
        let subnote_parent_idx: std::collections::HashMap<&str, usize> = subnotes_cache
            .iter()
            .filter_map(|(pid, _)| {
                if let Some(i) = self.notes.iter().position(|n| n.id == *pid) {
                    return Some((pid.as_str(), i));
                }
                let pid_stem = std::path::Path::new(pid)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(pid);
                self.notes
                    .iter()
                    .position(|n| {
                        std::path::Path::new(&n.id)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            == Some(pid_stem)
                    })
                    .map(|i| (pid.as_str(), i))
            })
            .collect();

        let mut by_folder: HashMap<&str, Vec<(usize, &NoteSummary)>> = HashMap::new();
        let mut pinned_notes: Vec<(usize, &NoteSummary)> = Vec::new();
        for (i, note) in self.notes.iter().enumerate() {
            by_folder
                .entry(note.folder.as_str())
                .or_default()
                .push((i, note));
            if note.pinned {
                pinned_notes.push((i, note));
            }
        }

        let all_folders = &self.catalog_folders;

        // Build subfolders map: group each folder by parent path for recursive traversal
        let mut subfolders_map: std::collections::HashMap<&str, Vec<&String>> =
            std::collections::HashMap::new();
        for folder in all_folders {
            let parent = if let Some(slash) = folder.rfind('/') {
                &folder[..slash]
            } else {
                ""
            };
            subfolders_map.entry(parent).or_default().push(folder);
        }

        let mut recursive_count = std::collections::HashMap::new();

        fn compute_subtree<'a>(
            folder: &'a str,
            subfolders_map: &std::collections::HashMap<&'a str, Vec<&'a String>>,
            by_folder: &std::collections::HashMap<&'a str, Vec<(usize, &'a NoteSummary)>>,
            recursive_count: &mut std::collections::HashMap<&'a str, usize>,
        ) -> usize {
            let direct_count = by_folder.get(folder).map_or(0, |v| v.len());
            let mut total_count = direct_count;

            if let Some(children) = subfolders_map.get(folder) {
                for child in children {
                    total_count +=
                        compute_subtree(child.as_str(), subfolders_map, by_folder, recursive_count);
                }
            }

            recursive_count.insert(folder, total_count);
            total_count
        }

        compute_subtree("", &subfolders_map, &by_folder, &mut recursive_count);

        visual.push(VisualItem::Folder {
            path: VIRTUAL_PINNED_PATH.to_string(),
            name: VIRTUAL_PINNED_LABEL.to_string(),
            depth: 0,
            is_expanded: self.list.folder_expanded.contains(VIRTUAL_PINNED_PATH),
            note_count: pinned_notes.len(),
            recursive_count: pinned_notes.len(),
            stale: false,
            is_pinned: false,
        });

        if self.list.folder_expanded.contains(VIRTUAL_PINNED_PATH) {
            for (idx, note) in &pinned_notes {
                visual.push(VisualItem::Note {
                    summary_idx: *idx,
                    depth: 1,
                    is_clin: note.id.ends_with(".clin"),
                    is_draw: note.id.ends_with(".draw"),
                    is_canvas: note.id.ends_with(".canvas"),
                    in_virtual_pinned_folder: true,
                });
            }
        }

        #[allow(clippy::too_many_arguments)]
        fn push_tree<'a>(
            current_folder: &'a str,
            depth: usize,
            visual: &mut Vec<VisualItem>,
            expanded_folders: &std::collections::HashSet<String>,
            subfolders_map: &std::collections::HashMap<&'a str, Vec<&'a String>>,
            by_folder: &std::collections::HashMap<&'a str, Vec<(usize, &'a NoteSummary)>>,
            folders_first: bool,
            recursive_count: &std::collections::HashMap<&'a str, usize>,
            pinned_folders: &'a std::collections::HashSet<String>,
            select_mode: bool,
        ) {
            let notes = by_folder.get(current_folder);
            let subfolders = subfolders_map.get(current_folder);

            if folders_first {
                if let Some(folders) = subfolders {
                    for folder in folders {
                        let parts: Vec<&str> = folder.split('/').collect();
                        let name = parts.last().unwrap_or(&"").to_string();
                        let is_expanded = expanded_folders.contains(folder.as_str());
                        let direct = by_folder.get(folder.as_str()).map_or(0, |v| v.len());
                        let rec_count = recursive_count
                            .get(folder.as_str())
                            .copied()
                            .unwrap_or(direct);
                        let stale = rec_count == 0;
                        visual.push(VisualItem::Folder {
                            path: folder.to_string(),
                            name,
                            depth,
                            is_expanded,
                            note_count: direct,
                            recursive_count: rec_count,
                            stale,
                            is_pinned: pinned_folders.contains(folder.as_str()),
                        });
                        if is_expanded {
                            push_tree(
                                folder,
                                depth + 1,
                                visual,
                                expanded_folders,
                                subfolders_map,
                                by_folder,
                                folders_first,
                                recursive_count,
                                pinned_folders,
                                select_mode,
                            );
                        }
                    }
                }
                if let Some(notes) = notes {
                    for (idx, note) in notes {
                        visual.push(VisualItem::Note {
                            summary_idx: *idx,
                            depth,
                            is_clin: note.id.ends_with(".clin"),
                            is_draw: note.id.ends_with(".draw"),
                            is_canvas: note.id.ends_with(".canvas"),
                            in_virtual_pinned_folder: false,
                        });
                    }
                }
                if !select_mode {
                    visual.push(VisualItem::CreateNew {
                        path: current_folder.to_string(),
                        depth,
                    });
                }
            } else {
                if let Some(notes) = notes {
                    for (idx, note) in notes {
                        visual.push(VisualItem::Note {
                            summary_idx: *idx,
                            depth,
                            is_clin: note.id.ends_with(".clin"),
                            is_draw: note.id.ends_with(".draw"),
                            is_canvas: note.id.ends_with(".canvas"),
                            in_virtual_pinned_folder: false,
                        });
                    }
                }
                if !select_mode {
                    visual.push(VisualItem::CreateNew {
                        path: current_folder.to_string(),
                        depth,
                    });
                }
                if let Some(folders) = subfolders {
                    for folder in folders {
                        let parts: Vec<&str> = folder.split('/').collect();
                        let name = parts.last().unwrap_or(&"").to_string();
                        let is_expanded = expanded_folders.contains(folder.as_str());
                        let direct = by_folder.get(folder.as_str()).map_or(0, |v| v.len());
                        let rec_count = recursive_count
                            .get(folder.as_str())
                            .copied()
                            .unwrap_or(direct);
                        let stale = rec_count == 0;
                        visual.push(VisualItem::Folder {
                            path: folder.to_string(),
                            name,
                            depth,
                            is_expanded,
                            note_count: direct,
                            recursive_count: rec_count,
                            stale,
                            is_pinned: pinned_folders.contains(folder.as_str()),
                        });
                        if is_expanded {
                            push_tree(
                                folder,
                                depth + 1,
                                visual,
                                expanded_folders,
                                subfolders_map,
                                by_folder,
                                folders_first,
                                recursive_count,
                                pinned_folders,
                                select_mode,
                            );
                        }
                    }
                }
            }
        }

        let mut sorted_pinned: Vec<String> = self
            .list
            .pinned_folders
            .iter()
            .filter(|p| !p.is_empty() && !p.starts_with('@'))
            .cloned()
            .collect();
        sorted_pinned.sort();

        for pinned_path in &sorted_pinned {
            let name = if let Some(slash) = pinned_path.rfind('/') {
                pinned_path[slash + 1..].to_string()
            } else {
                pinned_path.clone()
            };

            let is_expanded = self.list.folder_expanded.contains(pinned_path);
            let direct = by_folder.get(pinned_path.as_str()).map_or(0, |v| v.len());
            let rec_count = recursive_count
                .get(pinned_path.as_str())
                .copied()
                .unwrap_or(direct);
            let stale = rec_count == 0;

            visual.push(VisualItem::Folder {
                path: pinned_path.clone(),
                name,
                depth: 0,
                is_expanded,
                note_count: direct,
                recursive_count: rec_count,
                stale,
                is_pinned: true,
            });

            if is_expanded {
                push_tree(
                    pinned_path.as_str(),
                    1,
                    &mut visual,
                    &self.list.folder_expanded,
                    &subfolders_map,
                    &by_folder,
                    self.list.folders_first,
                    &recursive_count,
                    &self.list.pinned_folders,
                    self.list.list_mode == crate::list_view::ListMode::Select,
                );
            }
        }
        let mut computed_smart_folders = Vec::new();
        if self.config.list.smart_folders_enabled {
            let today_matches = self.notes_in_smart_folder(&SmartFolderKind::Today);
            if !today_matches.is_empty() {
                computed_smart_folders.push(SmartFolderData {
                    kind: SmartFolderKind::Today,
                    label: "Today".to_string(),
                    matches: today_matches,
                });
            }
            let week_matches = self.notes_in_smart_folder(&SmartFolderKind::ThisWeek);
            if !week_matches.is_empty() {
                computed_smart_folders.push(SmartFolderData {
                    kind: SmartFolderKind::ThisWeek,
                    label: "This Week".to_string(),
                    matches: week_matches,
                });
            }
            let untagged_matches = self.notes_in_smart_folder(&SmartFolderKind::Untagged);
            if !untagged_matches.is_empty() {
                computed_smart_folders.push(SmartFolderData {
                    kind: SmartFolderKind::Untagged,
                    label: "Untagged".to_string(),
                    matches: untagged_matches,
                });
            }

            for rule in &self.config.list.custom_smart_folders {
                let matches =
                    self.notes_in_smart_folder(&SmartFolderKind::Custom(rule.name.clone()));
                if !matches.is_empty() {
                    computed_smart_folders.push(SmartFolderData {
                        kind: SmartFolderKind::Custom(rule.name.clone()),
                        label: rule.name.clone(),
                        matches,
                    });
                }
            }

            let mut tag_set: std::collections::HashSet<String> = std::collections::HashSet::new();
            for note in &self.notes {
                for tag in &note.tags {
                    tag_set.insert(tag.clone());
                }
            }
            let mut sorted_tags: Vec<String> = tag_set.into_iter().collect();
            sorted_tags.sort();
            for tag in sorted_tags {
                let matches = self.notes_in_smart_folder(&SmartFolderKind::Tag(tag.clone()));
                if !matches.is_empty() {
                    computed_smart_folders.push(SmartFolderData {
                        kind: SmartFolderKind::Tag(tag.clone()),
                        label: tag,
                        matches,
                    });
                }
            }
            // Separate tag-based smart folders from non-tag ones
            let mut non_tag_folders: Vec<&SmartFolderData> = Vec::new();
            let mut tag_folders: Vec<&SmartFolderData> = Vec::new();
            for data in &computed_smart_folders {
                if matches!(data.kind, SmartFolderKind::Tag(_)) {
                    tag_folders.push(data);
                } else {
                    non_tag_folders.push(data);
                }
            }

            // Non-tag smart folders at depth 0
            for data in &non_tag_folders {
                let virtual_path = data.kind.virtual_path();
                let is_expanded = self.list.folder_expanded.contains(&virtual_path);
                visual.push(VisualItem::SmartFolder {
                    kind: data.kind.clone(),
                    label: data.label.clone(),
                    depth: 0,
                    is_expanded,
                    note_count: data.matches.len(),
                });
                if is_expanded {
                    for idx in &data.matches {
                        let note = &self.notes[*idx];
                        visual.push(VisualItem::Note {
                            summary_idx: *idx,
                            depth: 1,
                            is_clin: note.id.ends_with(".clin"),
                            is_draw: note.id.ends_with(".draw"),
                            is_canvas: note.id.ends_with(".canvas"),
                            in_virtual_pinned_folder: true,
                        });
                    }
                }
            }

            // Tagged parent folder (depth 0), only when tag folders exist
            if !tag_folders.is_empty() {
                let tagged_path = SmartFolderKind::Tagged.virtual_path();
                let tagged_expanded = self.list.folder_expanded.contains(&tagged_path);
                let total_tag_notes: usize = tag_folders.iter().map(|d| d.matches.len()).sum();
                visual.push(VisualItem::SmartFolder {
                    kind: SmartFolderKind::Tagged,
                    label: "Tagged".to_string(),
                    depth: 0,
                    is_expanded: tagged_expanded,
                    note_count: total_tag_notes,
                });
                if tagged_expanded {
                    for data in &tag_folders {
                        let virtual_path = data.kind.virtual_path();
                        let is_expanded = self.list.folder_expanded.contains(&virtual_path);
                        visual.push(VisualItem::SmartFolder {
                            kind: data.kind.clone(),
                            label: data.label.clone(),
                            depth: 1,
                            is_expanded,
                            note_count: data.matches.len(),
                        });
                        if is_expanded {
                            for idx in &data.matches {
                                let note = &self.notes[*idx];
                                visual.push(VisualItem::Note {
                                    summary_idx: *idx,
                                    depth: 2,
                                    is_clin: note.id.ends_with(".clin"),
                                    is_draw: note.id.ends_with(".draw"),
                                    is_canvas: note.id.ends_with(".canvas"),
                                    in_virtual_pinned_folder: true,
                                });
                            }
                        }
                    }
                }
            }
        }
        let subnotes_total: usize = subnotes_cache.iter().map(|(_, v)| v.len()).sum();
        visual.push(VisualItem::Folder {
            path: VIRTUAL_SUBNOTES_PATH.to_string(),
            name: VIRTUAL_SUBNOTES_LABEL.to_string(),
            depth: 0,
            is_expanded: self.list.folder_expanded.contains(VIRTUAL_SUBNOTES_PATH),
            note_count: subnotes_cache.len(),
            recursive_count: subnotes_total,
            stale: subnotes_cache.is_empty(),
            is_pinned: false,
        });
        if self.list.folder_expanded.contains(VIRTUAL_SUBNOTES_PATH) {
            for (parent_id, subs) in &subnotes_cache {
                let pidx = subnote_parent_idx.get(parent_id.as_str()).copied();
                let note = pidx.and_then(|i| self.notes.get(i));
                let name = note
                    .map(|n| n.title.clone())
                    .unwrap_or_else(|| parent_id.clone());
                let parent_expanded = self
                    .list
                    .folder_expanded
                    .contains(&format!("subnotes:{parent_id}"));
                visual.push(VisualItem::Folder {
                    path: format!("subnotes:{parent_id}"),
                    name,
                    depth: 1,
                    is_expanded: parent_expanded,
                    note_count: subs.len(),
                    recursive_count: subs.len(),
                    stale: false,
                    is_pinned: false,
                });
                if parent_expanded {
                    for (i, _sub) in subs.iter().enumerate() {
                        visual.push(VisualItem::Subnote {
                            parent_id: parent_id.clone(),
                            subnote_idx: i,
                            depth: 2,
                        });
                    }
                }
            }
        }
        let vault_direct = by_folder.get("").map_or(0, |v| v.len());
        let vault_recursive = recursive_count.get("").copied().unwrap_or(vault_direct);
        visual.push(VisualItem::Folder {
            path: String::new(),
            name: String::from("Vault"),
            depth: 0,
            is_expanded: self.list.folder_expanded.contains(""),
            note_count: vault_direct,
            recursive_count: vault_recursive,
            stale: false,
            is_pinned: false,
        });

        if self.list.folder_expanded.contains("") {
            push_tree(
                "",
                1,
                &mut visual,
                &self.list.folder_expanded,
                &subfolders_map,
                &by_folder,
                self.list.folders_first,
                &recursive_count,
                &self.list.pinned_folders,
                self.list.list_mode == crate::list_view::ListMode::Select,
            );
        }

        if self.list.notes_layout == crate::config::NotesLayout::Grid {
            // Discard the tree-view items (Pinned/Vault folders) built above.
            visual.clear();
            let gf = &self.list.grid_folder;
            if gf == VIRTUAL_PINNED_PATH {
                // Pinned tab: show pinned folders first, then pinned notes
                for pinned_path in &sorted_pinned {
                    let name = if let Some(slash) = pinned_path.rfind('/') {
                        pinned_path[slash + 1..].to_string()
                    } else {
                        pinned_path.clone()
                    };
                    let direct = by_folder.get(pinned_path.as_str()).map_or(0, |v| v.len());
                    let rec_count = recursive_count
                        .get(pinned_path.as_str())
                        .copied()
                        .unwrap_or(direct);
                    visual.push(VisualItem::Folder {
                        path: pinned_path.clone(),
                        name,
                        depth: 0,
                        is_expanded: false,
                        note_count: direct,
                        recursive_count: rec_count,
                        stale: rec_count == 0,
                        is_pinned: true,
                    });
                }
                for (idx, note) in &pinned_notes {
                    visual.push(VisualItem::Note {
                        summary_idx: *idx,
                        depth: 0,
                        is_clin: note.id.ends_with(".clin"),
                        is_draw: note.id.ends_with(".draw"),
                        is_canvas: note.id.ends_with(".canvas"),
                        in_virtual_pinned_folder: true,
                    });
                }
            } else if gf == VIRTUAL_SMART_PATH {
                // Smart Folders tab: separate non-tag and tag-based smart folders
                let mut non_tag: Vec<&SmartFolderData> = Vec::new();
                let mut tag: Vec<&SmartFolderData> = Vec::new();
                for data in &computed_smart_folders {
                    if matches!(data.kind, SmartFolderKind::Tag(_)) {
                        tag.push(data);
                    } else {
                        non_tag.push(data);
                    }
                }
                for data in &non_tag {
                    visual.push(VisualItem::SmartFolder {
                        kind: data.kind.clone(),
                        label: data.label.clone(),
                        depth: 0,
                        is_expanded: false,
                        note_count: data.matches.len(),
                    });
                }
                if !tag.is_empty() {
                    let total: usize = tag.iter().map(|d| d.matches.len()).sum();
                    visual.push(VisualItem::SmartFolder {
                        kind: SmartFolderKind::Tagged,
                        label: "Tagged".to_string(),
                        depth: 0,
                        is_expanded: false,
                        note_count: total,
                    });
                }
            } else if gf == "@tagged" {
                // Inside Tagged: show per-tag smart folders with ".." back to Smart tab
                visual.push(VisualItem::Folder {
                    path: VIRTUAL_SMART_PATH.to_string(),
                    name: "..".to_string(),
                    depth: 0,
                    is_expanded: false,
                    note_count: 0,
                    recursive_count: 0,
                    stale: false,
                    is_pinned: false,
                });
                for data in &computed_smart_folders {
                    if matches!(data.kind, SmartFolderKind::Tag(_)) {
                        visual.push(VisualItem::SmartFolder {
                            kind: data.kind.clone(),
                            label: data.label.clone(),
                            depth: 0,
                            is_expanded: false,
                            note_count: data.matches.len(),
                        });
                    }
                }
            } else if gf.starts_with('@') {
                // User is inside a smart folder.
                // 1. Push ".." pointing back to Smart tab root
                visual.push(VisualItem::Folder {
                    path: VIRTUAL_SMART_PATH.to_string(),
                    name: "..".to_string(),
                    depth: 0,
                    is_expanded: false,
                    note_count: 0,
                    recursive_count: 0,
                    stale: false,
                    is_pinned: false,
                });
                // 2. Find the matching smart folder by virtual path and render its notes
                if let Some(folder_data) = computed_smart_folders
                    .iter()
                    .find(|d| d.kind.virtual_path() == *gf)
                {
                    for idx in &folder_data.matches {
                        let note = &self.notes[*idx];
                        visual.push(VisualItem::Note {
                            summary_idx: *idx,
                            depth: 0,
                            is_clin: note.id.ends_with(".clin"),
                            is_draw: note.id.ends_with(".draw"),
                            is_canvas: note.id.ends_with(".canvas"),
                            in_virtual_pinned_folder: false,
                        });
                    }
                }
            } else if gf == VIRTUAL_SUBNOTES_PATH {
                // Subnotes tab root: list parent notes as virtual-folder tiles.
                for (parent_id, subs) in &subnotes_cache {
                    let pidx = subnote_parent_idx.get(parent_id.as_str()).copied();
                    let name = pidx
                        .and_then(|i| self.notes.get(i))
                        .map(|n| n.title.clone())
                        .unwrap_or_else(|| parent_id.clone());
                    visual.push(VisualItem::Folder {
                        path: format!("subnotes:{parent_id}"),
                        name,
                        depth: 0,
                        is_expanded: false,
                        note_count: subs.len(),
                        recursive_count: subs.len(),
                        stale: false,
                        is_pinned: false,
                    });
                }
            } else if Self::is_subnotes_parent_grid_path(gf) {
                // Inside a subnotes parent: ".." back + subnote tiles.
                let parent_id = Self::subnotes_parent_id_from_grid_path(gf).to_string();
                visual.push(VisualItem::Folder {
                    path: VIRTUAL_SUBNOTES_PATH.to_string(),
                    name: "..".to_string(),
                    depth: 0,
                    is_expanded: false,
                    note_count: 0,
                    recursive_count: 0,
                    stale: false,
                    is_pinned: false,
                });
                if let Some((_, subs)) = subnotes_cache.iter().find(|(p, _)| *p == parent_id) {
                    for (i, _sub) in subs.iter().enumerate() {
                        visual.push(VisualItem::Subnote {
                            parent_id: parent_id.clone(),
                            subnote_idx: i,
                            depth: 0,
                        });
                    }
                }
            } else {
                // Vault tab or a subfolder: show only the contents of this folder.
                // ".." only appears when inside a subfolder (not at Vault root "").
                if !gf.is_empty() {
                    let parent_path = if let Some(slash) = gf.rfind('/') {
                        &gf[..slash]
                    } else {
                        ""
                    };
                    visual.push(VisualItem::Folder {
                        path: parent_path.to_string(),
                        name: "..".to_string(),
                        depth: 0,
                        is_expanded: false,
                        note_count: 0,
                        recursive_count: 0,
                        stale: false,
                        is_pinned: false,
                    });
                }

                // Pinned folders as top-level shortcuts in root tab when pinned_on_top
                if gf.is_empty() && self.pinned_on_top {
                    for pinned_path in &sorted_pinned {
                        let name = if let Some(slash) = pinned_path.rfind('/') {
                            pinned_path[slash + 1..].to_string()
                        } else {
                            pinned_path.clone()
                        };
                        let direct = by_folder.get(pinned_path.as_str()).map_or(0, |v| v.len());
                        let rec_count = recursive_count
                            .get(pinned_path.as_str())
                            .copied()
                            .unwrap_or(direct);
                        visual.push(VisualItem::Folder {
                            path: pinned_path.clone(),
                            name,
                            depth: 0,
                            is_expanded: false,
                            note_count: direct,
                            recursive_count: rec_count,
                            stale: rec_count == 0,
                            is_pinned: true,
                        });
                    }
                }

                // Direct subfolders / notes of the current folder, respecting folders_first
                if self.list.folders_first {
                    for folder in all_folders {
                        let parent_path = if let Some(slash) = folder.rfind('/') {
                            &folder[..slash]
                        } else {
                            ""
                        };
                        if parent_path == gf {
                            let parts: Vec<&str> = folder.split('/').collect();
                            let name = parts.last().unwrap_or(&"").to_string();
                            let direct = by_folder.get(folder.as_str()).map_or(0, |v| v.len());
                            let rec_count = recursive_count
                                .get(folder.as_str())
                                .copied()
                                .unwrap_or(direct);
                            visual.push(VisualItem::Folder {
                                path: folder.clone(),
                                name,
                                depth: 0,
                                is_expanded: false,
                                note_count: direct,
                                recursive_count: rec_count,
                                stale: false,
                                is_pinned: self.list.pinned_folders.contains(folder.as_str()),
                            });
                        }
                    }
                    if let Some(notes) = by_folder.get(gf.as_str()) {
                        for (idx, note) in notes {
                            visual.push(VisualItem::Note {
                                summary_idx: *idx,
                                depth: 0,
                                is_clin: note.id.ends_with(".clin"),
                                is_draw: note.id.ends_with(".draw"),
                                is_canvas: note.id.ends_with(".canvas"),
                                in_virtual_pinned_folder: false,
                            });
                        }
                    }
                } else {
                    if let Some(notes) = by_folder.get(gf.as_str()) {
                        for (idx, note) in notes {
                            visual.push(VisualItem::Note {
                                summary_idx: *idx,
                                depth: 0,
                                is_clin: note.id.ends_with(".clin"),
                                is_draw: note.id.ends_with(".draw"),
                                is_canvas: note.id.ends_with(".canvas"),
                                in_virtual_pinned_folder: false,
                            });
                        }
                    }
                    for folder in all_folders {
                        let parent_path = if let Some(slash) = folder.rfind('/') {
                            &folder[..slash]
                        } else {
                            ""
                        };
                        if parent_path == gf {
                            let parts: Vec<&str> = folder.split('/').collect();
                            let name = parts.last().unwrap_or(&"").to_string();
                            let direct = by_folder.get(folder.as_str()).map_or(0, |v| v.len());
                            let rec_count = recursive_count
                                .get(folder.as_str())
                                .copied()
                                .unwrap_or(direct);
                            visual.push(VisualItem::Folder {
                                path: folder.clone(),
                                name,
                                depth: 0,
                                is_expanded: false,
                                note_count: direct,
                                recursive_count: rec_count,
                                stale: false,
                                is_pinned: self.list.pinned_folders.contains(folder.as_str()),
                            });
                        }
                    }
                }
                if self.list.list_mode != crate::list_view::ListMode::Select {
                    visual.push(VisualItem::CreateNew {
                        path: gf.clone(),
                        depth: 0,
                    });
                }
            }

            self.list.visual_list = visual;
            self.request_preview_update_immediate();
            return;
        }

        self.list.visual_list = visual;
        self.request_preview_update_immediate();
    }

    /// Poll only state owned by Edit mode. Generic list/setup work stays out of
    /// the editor's input-to-draw path.
    pub(crate) fn poll_editor_renderers(&mut self) -> bool {
        let mut updated = false;
        if let Some((_, instant)) = self.editor.pending_markdown_resize
            && instant.elapsed() >= Duration::from_millis(50)
        {
            self.editor.pending_markdown_resize = None;
            self.editor.preview_content_width = None;
            self.update_editor_markdown_preview();
            updated = true;
        }
        if self.editor.pending_editor_preview_update
            && self.editor.preview_scheduler.due(Instant::now())
        {
            self.update_editor_markdown_preview();
            self.editor.pending_editor_preview_update = false;
            self.editor.last_editor_change = None;
            self.editor.preview_scheduler.clear();
            updated = true;
        }
        let edit_active = self.editor.editor_preview_enabled || self.preview_fullscreen;
        if edit_active
            && self.editor.pending_markdown_resize.is_none()
            && (self.editor.preview_content_width != Some(self.desired_editor_preview_width())
                || self.editor.preview_content_height != Some(self.desired_editor_preview_height()))
        {
            self.update_editor_markdown_preview();
            updated = true;
        }
        if let Some(renderer) = &mut self.editor.md_preview_renderer
            && renderer.poll()
        {
            updated = true;
        }
        if let Some(renderer) = &mut self.editor.link_preview_renderer
            && renderer.poll()
        {
            updated = true;
        }
        updated
    }

    pub(crate) fn poll_editor_image_results(&mut self) -> bool {
        let results: Vec<anyhow::Result<crate::image_render::worker::DecodedImage>> =
            match &self.image_decode_rx {
                Some(receiver) => std::iter::from_fn(|| receiver.try_recv().ok()).collect(),
                None => Vec::new(),
            };
        let mut updated = false;
        for result in results {
            match result {
                Ok(image) => self.install_image(image),
                Err(error) => {
                    let text = format!("Image decode failed: {error}");
                    self.set_temporary_status(&text);
                    self.messages
                        .push(text, crate::app::messages::MessageSeverity::Warning);
                }
            }
            updated = true;
        }
        updated
    }

    pub fn poll_renderers(&mut self) -> bool {
        let mut updated = false;

        // Check resize debounces
        if let Some((_, inst)) = self.list.pending_markdown_resize
            && inst.elapsed() >= Duration::from_millis(50)
        {
            self.list.pending_markdown_resize = None;
            self.list.preview_content_width = None;
            self.update_preview();
            updated = true;
        }
        if let Some((_, inst)) = self.editor.pending_markdown_resize
            && inst.elapsed() >= Duration::from_millis(50)
        {
            self.editor.pending_markdown_resize = None;
            self.editor.preview_content_width = None;
            self.update_editor_markdown_preview();
            updated = true;
        }
        if let Some(ref mut setup) = self.setup_state
            && let Some((_, inst)) = setup.pending_preview_resize
            && inst.elapsed() >= Duration::from_millis(50)
        {
            updated = true; // Trigger redraw so draw loop handles it
        }

        if let Some(last) = self.editor.last_editor_change
            && last.elapsed() > Duration::from_millis(150)
            && self.editor.pending_editor_preview_update
        {
            self.update_editor_markdown_preview();
            self.editor.pending_editor_preview_update = false;
            self.editor.last_editor_change = None;
            updated = true;
        }

        let list_active = self.list.preview_enabled || self.preview_fullscreen;
        if list_active
            && self.list.pending_markdown_resize.is_none()
            && (self.list.preview_content_width != Some(self.desired_list_preview_width())
                || self.list.preview_content_height != Some(self.desired_list_preview_height())
                || self.list.preview_content_scale != Some(self.list.preview_scale)
                || self.list.preview_content_offset_x != Some(self.list.preview_offset_x)
                || self.list.preview_content_offset_y != Some(self.list.preview_offset_y))
        {
            self.update_preview();
            updated = true;
        }
        let edit_active = self.editor.editor_preview_enabled || self.preview_fullscreen;
        if edit_active
            && self.editor.pending_markdown_resize.is_none()
            && (self.editor.preview_content_width != Some(self.desired_editor_preview_width())
                || self.editor.preview_content_height != Some(self.desired_editor_preview_height()))
        {
            self.update_editor_markdown_preview();
            updated = true;
        }

        // Poll renderers
        if let Some(PreviewContent::Markdown(renderer)) = &mut self.list.preview_content
            && renderer.poll()
        {
            updated = true;
        }
        if let Some(renderer) = &mut self.editor.md_preview_renderer
            && renderer.poll()
        {
            updated = true;
        }
        if let Some(renderer) = &mut self.editor.link_preview_renderer
            && renderer.poll()
        {
            updated = true;
        }
        if let Some(ref mut setup) = self.setup_state
            && setup.preview_renderer.poll()
        {
            updated = true;
        }

        updated
    }

    /// Install a completed decode into the active view's image cache.
    pub fn install_image(&mut self, decoded: crate::image_render::worker::DecodedImage) {
        let picker = match self.image_picker.as_ref() {
            Some(p) => p,
            None => return,
        };

        match self.mode {
            crate::app::ViewMode::Canvas => {
                if let Some(state) = &mut self.canvas_state {
                    state.image_cache.install_decoded(decoded, picker);
                }
            }
            crate::app::ViewMode::Edit => {
                self.editor.image_cache.install_decoded(decoded, picker);
            }
            crate::app::ViewMode::List => {
                self.list.image_cache.install_decoded(decoded, picker);
            }
            _ => {}
        }
    }

    pub fn request_preview_update(&mut self) {
        if !(self.list.preview_enabled || self.preview_fullscreen) {
            return;
        }

        self.update_preview();
        self.list.pending_preview_update = false;
        self.list.last_selection_change = None;
    }

    pub fn request_preview_update_immediate(&mut self) {
        if !(self.list.preview_enabled || self.preview_fullscreen) {
            return;
        }

        self.update_preview();
        self.list.pending_preview_update = false;
        self.list.last_selection_change = None;
    }

    pub fn request_editor_preview_update(&mut self) {
        let now = Instant::now();
        self.editor.last_editor_change = Some(now);
        self.editor
            .preview_scheduler
            .schedule(self.editor.body.revision(), now);
        if self.editor.editor_preview_enabled || self.preview_fullscreen {
            self.editor.pending_editor_preview_update = true;
        }
    }

    /// Returns indices into `self.notes` that match the given smart folder kind.
    /// Respects `smart_folders_enabled` (empty when disabled).
    pub(crate) fn notes_in_smart_folder(&self, kind: &SmartFolderKind) -> Vec<usize> {
        if !self.config.list.smart_folders_enabled {
            return Vec::new();
        }
        let now = crate::ui::now_unix_secs();
        match kind {
            SmartFolderKind::Today => self
                .notes
                .iter()
                .enumerate()
                .filter(|(_, n)| now.saturating_sub(n.updated_at) < 86_400)
                .map(|(i, _)| i)
                .collect(),
            SmartFolderKind::ThisWeek => self
                .notes
                .iter()
                .enumerate()
                .filter(|(_, n)| now.saturating_sub(n.updated_at) < 604_800)
                .map(|(i, _)| i)
                .collect(),
            SmartFolderKind::Untagged => self
                .notes
                .iter()
                .enumerate()
                .filter(|(_, n)| n.tags.is_empty())
                .map(|(i, _)| i)
                .collect(),
            SmartFolderKind::Tag(tag) => self
                .notes
                .iter()
                .enumerate()
                .filter(|(_, n)| n.tags.contains(tag))
                .map(|(i, _)| i)
                .collect(),
            SmartFolderKind::Custom(name) => {
                let rule = self
                    .config
                    .list
                    .custom_smart_folders
                    .iter()
                    .find(|r| &r.name == name);
                let Some(rule) = rule else {
                    return Vec::new();
                };
                self.notes
                    .iter()
                    .enumerate()
                    .filter(|(_, n)| {
                        for t in &rule.tags {
                            if !n.tags.contains(t) {
                                return false;
                            }
                        }
                        if let Some(txt) = &rule.title_contains
                            && !n.title.to_lowercase().contains(&txt.to_lowercase())
                        {
                            return false;
                        }
                        if let Some(prefix) = &rule.folder_prefix
                            && !n.folder.starts_with(prefix)
                        {
                            return false;
                        }
                        if let Some(days) = rule.updated_within_days {
                            let diff = now.saturating_sub(n.updated_at);
                            if diff >= days * 86_400 {
                                return false;
                            }
                        }
                        true
                    })
                    .map(|(i, _)| i)
                    .collect()
            }
            SmartFolderKind::Tagged => Vec::new(),
        }
    }

    /// Returns children of the focused folder for the FolderGraph preview.
    /// `(nodes_without_positions, focused_label)`. Positions are filled by the renderer.
    pub(crate) fn folder_graph_children(
        &self,
        focused_path: &str,
    ) -> (Vec<crate::list_view::FolderGraphNode>, String) {
        use crate::list_view::FolderGraphNode;
        if focused_path == crate::app::VIRTUAL_PINNED_PATH {
            let children: Vec<FolderGraphNode> = self
                .notes
                .iter()
                .filter(|n| n.pinned)
                .map(|n| FolderGraphNode {
                    label: n.title.clone(),
                    is_note: true,
                    x: 0.0,
                    y: 0.0,
                    links: n.links.clone(),
                })
                .collect();
            return (children, crate::app::VIRTUAL_PINNED_LABEL.to_string());
        }
        if focused_path.starts_with('@') {
            if let Some(kind) = SmartFolderKind::from_virtual_path(focused_path) {
                let label = match &kind {
                    SmartFolderKind::Today => "Today".to_string(),
                    SmartFolderKind::ThisWeek => "This Week".to_string(),
                    SmartFolderKind::Untagged => "Untagged".to_string(),
                    SmartFolderKind::Tag(t) => t.clone(),
                    SmartFolderKind::Custom(name) => name.clone(),
                    SmartFolderKind::Tagged => "Tagged".to_string(),
                };
                let indices = self.notes_in_smart_folder(&kind);
                let children: Vec<FolderGraphNode> = indices
                    .iter()
                    .filter_map(|&i| self.notes.get(i))
                    .map(|n| FolderGraphNode {
                        label: n.title.clone(),
                        is_note: true,
                        x: 0.0,
                        y: 0.0,
                        links: n.links.clone(),
                    })
                    .collect();
                return (children, label);
            }
            return (Vec::new(), focused_path.to_string());
        }
        if focused_path.starts_with("subnotes:") {
            return (Vec::new(), String::new());
        }
        // Real vault folder (including "" for root).
        let all_folders = &self.catalog_folders;
        let subfolders: Vec<FolderGraphNode> = all_folders
            .iter()
            .filter(|f| {
                let parent = if let Some(slash) = f.rfind('/') {
                    &f[..slash]
                } else {
                    ""
                };
                parent == focused_path
            })
            .map(|f| {
                let name = f.split('/').next_back().unwrap_or("").to_string();
                FolderGraphNode {
                    label: name,
                    is_note: false,
                    x: 0.0,
                    y: 0.0,
                    links: Vec::new(),
                }
            })
            .collect();
        let notes: Vec<FolderGraphNode> = self
            .notes
            .iter()
            .filter(|n| n.folder == focused_path)
            .map(|n| FolderGraphNode {
                label: n.title.clone(),
                is_note: true,
                x: 0.0,
                y: 0.0,
                links: n.links.clone(),
            })
            .collect();
        let label = if focused_path.is_empty() {
            "Vault (Root)".to_string()
        } else {
            focused_path
                .rsplit('/')
                .next()
                .unwrap_or(focused_path)
                .to_string()
        };
        let mut children: Vec<FolderGraphNode> = Vec::new();
        children.extend(subfolders);
        children.extend(notes);
        (children, label)
    }

    pub fn update_preview(&mut self) {
        if !(self.list.preview_enabled || self.preview_fullscreen) {
            return;
        }

        let item = self.list.visual_list.get(self.list.visual_index);
        match item {
            Some(VisualItem::Note {
                summary_idx,
                is_draw,
                is_canvas,
                ..
            }) => {
                let summary_idx = *summary_idx;
                let is_draw = *is_draw;
                let is_canvas = *is_canvas;
                let id = &self.notes[summary_idx].id;
                let is_clin = id.ends_with(".clin");

                if self.preview_encryption && is_clin {
                    self.list.preview_content = None;
                    self.list.preview_content_index = Some(self.list.visual_index);
                    return;
                }

                if is_draw {
                    // Reuse cached DrawData for in-memory re-render (avoids disk I/O)
                    if self.list.preview_content_index == Some(self.list.visual_index)
                        && let Some(PreviewContent::DrawGrid { data, .. }) =
                            self.list.preview_content.take()
                    {
                        let width = self.desired_list_preview_width();
                        let height = self.desired_list_preview_height();
                        let scale = self.list.preview_scale;
                        let offset_x = self.list.preview_offset_x;
                        let offset_y = self.list.preview_offset_y;
                        let grid = crate::snapshot::render_draw_snapshot_with_size(
                            &data,
                            &self.app_theme,
                            self.config.ui.icon_mode,
                            width,
                            height,
                            scale,
                            offset_x,
                            offset_y,
                        );
                        self.list.preview_content = Some(PreviewContent::DrawGrid { data, grid });
                        self.list.preview_content_width = Some(width);
                        self.list.preview_content_height = Some(height);
                        self.list.preview_content_scale = Some(scale);
                        self.list.preview_content_offset_x = Some(offset_x);
                        self.list.preview_content_offset_y = Some(offset_y);
                        self.list.preview_content_index = Some(self.list.visual_index);
                        return;
                    }
                    let path = self.storage.note_path(id);
                    match std::fs::read_to_string(&path) {
                        Ok(content) => {
                            match serde_json::from_str::<crate::draw::state::DrawData>(&content) {
                                Ok(data) => {
                                    let width = self.desired_list_preview_width();
                                    let height = self.desired_list_preview_height();
                                    let scale = self.list.preview_scale;
                                    let offset_x = self.list.preview_offset_x;
                                    let offset_y = self.list.preview_offset_y;
                                    let grid = crate::snapshot::render_draw_snapshot_with_size(
                                        &data,
                                        &self.app_theme,
                                        self.config.ui.icon_mode,
                                        width,
                                        height,
                                        scale,
                                        offset_x,
                                        offset_y,
                                    );
                                    self.list.preview_content = Some(PreviewContent::DrawGrid {
                                        data: Box::new(data),
                                        grid,
                                    });
                                    self.list.preview_content_width = Some(width);
                                    self.list.preview_content_height = Some(height);
                                    self.list.preview_content_scale = Some(scale);
                                    self.list.preview_content_offset_x = Some(offset_x);
                                    self.list.preview_content_offset_y = Some(offset_y);
                                }
                                Err(e) => {
                                    self.list.preview_content = None;
                                    self.status = Cow::Owned(format!("Failed to parse draw: {e}"));
                                }
                            }
                        }
                        Err(_) => {
                            self.list.preview_content = None;
                        }
                    }
                    self.list.preview_content_index = Some(self.list.visual_index);
                    return;
                }
                if is_canvas {
                    // Reuse cached CanvasData for in-memory re-render (avoids disk I/O)
                    if self.list.preview_content_index == Some(self.list.visual_index)
                        && let Some(PreviewContent::CanvasGrid { data, .. }) =
                            self.list.preview_content.take()
                    {
                        let width = self.desired_list_preview_width();
                        let height = self.desired_list_preview_height();
                        let scale = self.list.preview_scale;
                        let offset_x = self.list.preview_offset_x;
                        let offset_y = self.list.preview_offset_y;
                        let grid = crate::snapshot::render_canvas_snapshot(
                            &data,
                            &self.app_theme,
                            self.config.ui.icon_mode,
                            width,
                            height,
                            scale,
                            offset_x,
                            offset_y,
                        );
                        self.list.preview_content = Some(PreviewContent::CanvasGrid { data, grid });
                        self.list.preview_content_width = Some(width);
                        self.list.preview_content_height = Some(height);
                        self.list.preview_content_scale = Some(scale);
                        self.list.preview_content_offset_x = Some(offset_x);
                        self.list.preview_content_offset_y = Some(offset_y);
                        self.list.preview_content_index = Some(self.list.visual_index);
                        return;
                    }
                    let path = self.storage.note_path(id);
                    match std::fs::read_to_string(&path) {
                        Ok(content) => {
                            match serde_json::from_str::<crate::pinstar::data::CanvasData>(&content)
                            {
                                Ok(data) => {
                                    let width = self.desired_list_preview_width();
                                    let height = self.desired_list_preview_height();
                                    let scale = self.list.preview_scale;
                                    let offset_x = self.list.preview_offset_x;
                                    let offset_y = self.list.preview_offset_y;
                                    let grid = crate::snapshot::render_canvas_snapshot(
                                        &data,
                                        &self.app_theme,
                                        self.config.ui.icon_mode,
                                        width,
                                        height,
                                        scale,
                                        offset_x,
                                        offset_y,
                                    );
                                    self.list.preview_content = Some(PreviewContent::CanvasGrid {
                                        data: Box::new(data),
                                        grid,
                                    });
                                    self.list.preview_content_width = Some(width);
                                    self.list.preview_content_height = Some(height);
                                    self.list.preview_content_scale = Some(scale);
                                    self.list.preview_content_offset_x = Some(offset_x);
                                    self.list.preview_content_offset_y = Some(offset_y);
                                }
                                Err(e) => {
                                    self.list.preview_content = None;
                                    self.status =
                                        Cow::Owned(format!("Failed to parse canvas: {e}"));
                                }
                            }
                        }
                        Err(_) => {
                            self.list.preview_content = None;
                        }
                    }
                    self.list.preview_content_index = Some(self.list.visual_index);
                    return;
                }

                let ext = std::path::Path::new(id)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("");
                if crate::storage::is_image_ext(ext) {
                    let path = self.storage.note_path(id);
                    self.list.preview_content = Some(PreviewContent::Image(path));
                    self.list.preview_content_width = Some(self.desired_list_preview_width());
                    self.list.preview_content_height = Some(self.desired_list_preview_height());
                    self.list.preview_content_scale = Some(self.list.preview_scale);
                    self.list.preview_content_offset_x = Some(self.list.preview_offset_x);
                    self.list.preview_content_offset_y = Some(self.list.preview_offset_y);
                    self.list.preview_content_index = Some(self.list.visual_index);
                    return;
                }

                if let Ok(note) = self.storage.load_note(id) {
                    let width = self.desired_list_preview_width();
                    let mut renderer = match self.list.preview_content.take() {
                        Some(PreviewContent::Markdown(r)) => *r,
                        _ => MarkdownRenderer::new(),
                    };
                    let opts = crate::markdown::MdRenderOpts::from_config(&self.config);
                    let height = self.desired_list_preview_height();
                    renderer.set_page_height(height as usize);
                    let viewport = crate::markdown::RenderViewport {
                        start: renderer.visible_start(),
                        height: height as usize,
                    };

                    let content_changed =
                        renderer.is_changed(&note.content, &self.app_theme, &opts);
                    let mut should_render = false;
                    if content_changed || renderer.document().is_none() {
                        should_render = true;
                    } else if let Some(old_w) = self.list.preview_content_width {
                        if old_w == width {
                            renderer.set_viewport(viewport.start, viewport.height);
                        } else {
                            let now = std::time::Instant::now();
                            if let Some((w, _)) = self.list.pending_markdown_resize {
                                if w != width {
                                    self.list.pending_markdown_resize = Some((width, now));
                                }
                            } else {
                                self.list.pending_markdown_resize = Some((width, now));
                            }
                        }
                    } else {
                        should_render = true;
                    }

                    if should_render {
                        renderer.render_with(
                            &note.content,
                            width,
                            &self.app_theme,
                            &opts,
                            viewport,
                        );
                        self.list.preview_content_width = Some(width);
                        self.list.pending_markdown_resize = None;
                    }
                    self.list.preview_content = Some(PreviewContent::Markdown(Box::new(renderer)));
                    self.list.preview_content_height = Some(height);
                    self.list.preview_content_scale = Some(self.list.preview_scale);
                    self.list.preview_content_offset_x = Some(self.list.preview_offset_x);
                    self.list.preview_content_offset_y = Some(self.list.preview_offset_y);
                } else {
                    self.list.preview_content = None;
                }
                self.list.preview_content_index = Some(self.list.visual_index);
            }
            Some(VisualItem::Folder { path, name, .. })
                if !Self::is_virtual_subnotes_path(path)
                    && !Self::is_subnotes_parent_grid_path(path) =>
            {
                let folder_path = path.clone();
                let is_pinned = folder_path == crate::app::VIRTUAL_PINNED_PATH;
                if self.config.list.folder_graph_preview {
                    self.list.preview_content = Some(PreviewContent::FolderGraph {
                        root_path: folder_path.clone(),
                        focused_path: folder_path,
                    });
                    self.list.preview_content_index = Some(self.list.visual_index);
                    self.list.preview_content_width = Some(self.desired_list_preview_width());
                    self.list.preview_content_height = Some(self.desired_list_preview_height());
                    self.list.preview_content_scale = Some(self.list.preview_scale);
                    self.list.preview_content_offset_x = Some(self.list.preview_offset_x);
                    self.list.preview_content_offset_y = Some(self.list.preview_offset_y);
                    return;
                }

                let all_folders = self.catalog_folders.clone();

                let mut subfolders = Vec::new();
                if !is_pinned {
                    for f in &all_folders {
                        let parent_path = if let Some(slash) = f.rfind('/') {
                            &f[..slash]
                        } else {
                            ""
                        };
                        if parent_path == folder_path {
                            let name = f.split('/').next_back().unwrap_or("").to_string();
                            subfolders.push(name);
                        }
                    }
                    subfolders.sort();
                }

                let mut notes = Vec::new();
                for note in &self.notes {
                    let matches = if is_pinned {
                        note.pinned
                    } else {
                        note.folder == folder_path
                    };
                    if matches {
                        notes.push(note.title.clone());
                    }
                }
                notes.sort();

                let display_title = if is_pinned {
                    "Pinned Notes".to_string()
                } else if name == ".." {
                    format!(
                        "Parent: {}",
                        if folder_path.is_empty() {
                            "Vault"
                        } else {
                            &folder_path
                        }
                    )
                } else if folder_path.is_empty() {
                    "Vault (Root)".to_string()
                } else {
                    name.clone()
                };

                let mut md = format!("# {display_title}\n\n");

                if !subfolders.is_empty() {
                    md.push_str("## Folders\n");
                    for sub in &subfolders {
                        md.push_str(&format!("- \u{f07b} {sub}\n"));
                    }
                    md.push('\n');
                }

                if !notes.is_empty() {
                    md.push_str("## Notes\n");
                    for note in &notes {
                        md.push_str(&format!("- \u{f15c} {note}\n"));
                    }
                } else if subfolders.is_empty() {
                    md.push_str("*This folder is empty.*\n");
                }

                let width = self.desired_list_preview_width();
                let mut renderer = match self.list.preview_content.take() {
                    Some(PreviewContent::Markdown(r)) => *r,
                    _ => MarkdownRenderer::new(),
                };
                let opts = crate::markdown::MdRenderOpts::from_config(&self.config);
                let height = self.desired_list_preview_height();
                renderer.set_page_height(height as usize);
                let viewport = crate::markdown::RenderViewport {
                    start: renderer.visible_start(),
                    height: height as usize,
                };

                let content_changed = renderer.is_changed(&md, &self.app_theme, &opts);
                let mut should_render = false;
                if content_changed || renderer.document().is_none() {
                    should_render = true;
                } else if let Some(old_w) = self.list.preview_content_width {
                    if old_w == width {
                        renderer.set_viewport(viewport.start, viewport.height);
                    } else {
                        let now = std::time::Instant::now();
                        if let Some((w, _)) = self.list.pending_markdown_resize {
                            if w != width {
                                self.list.pending_markdown_resize = Some((width, now));
                            }
                        } else {
                            self.list.pending_markdown_resize = Some((width, now));
                        }
                    }
                } else {
                    should_render = true;
                }

                if should_render {
                    renderer.render_with(&md, width, &self.app_theme, &opts, viewport);
                    self.list.preview_content_width = Some(width);
                    self.list.pending_markdown_resize = None;
                }
                self.list.preview_content = Some(PreviewContent::Markdown(Box::new(renderer)));
                self.list.preview_content_height = Some(height);
                self.list.preview_content_scale = Some(self.list.preview_scale);
                self.list.preview_content_offset_x = Some(self.list.preview_offset_x);
                self.list.preview_content_offset_y = Some(self.list.preview_offset_y);
                self.list.preview_content_index = Some(self.list.visual_index);
            }
            Some(VisualItem::Folder { path, .. }) if Self::is_virtual_subnotes_path(path) => {
                // Subnotes tab root selected: show summary markdown.
                let mut md = String::from("# Subnotes\n\n");
                for (pid, subs) in &self.subnotes_view_cache {
                    let title = self
                        .notes
                        .iter()
                        .find(|n| n.id == *pid)
                        .map(|n| n.title.clone())
                        .unwrap_or_else(|| pid.clone());
                    md.push_str(&format!("- \u{f02c} {title} ({})\n", subs.len()));
                }
                if self.subnotes_view_cache.is_empty() {
                    md.push_str("*No subnotes.*\n");
                }
                let width = self.desired_list_preview_width();
                let mut renderer = match self.list.preview_content.take() {
                    Some(PreviewContent::Markdown(r)) => *r,
                    _ => MarkdownRenderer::new(),
                };
                let opts = crate::markdown::MdRenderOpts::from_config(&self.config);
                let height = self.desired_list_preview_height();
                renderer.set_page_height(height as usize);
                let viewport = crate::markdown::RenderViewport {
                    start: renderer.visible_start(),
                    height: height as usize,
                };

                let content_changed = renderer.is_changed(&md, &self.app_theme, &opts);
                let mut should_render = false;
                if content_changed || renderer.document().is_none() {
                    should_render = true;
                } else if let Some(old_w) = self.list.preview_content_width {
                    if old_w == width {
                        renderer.set_viewport(viewport.start, viewport.height);
                    } else {
                        let now = std::time::Instant::now();
                        if let Some((w, _)) = self.list.pending_markdown_resize {
                            if w != width {
                                self.list.pending_markdown_resize = Some((width, now));
                            }
                        } else {
                            self.list.pending_markdown_resize = Some((width, now));
                        }
                    }
                } else {
                    should_render = true;
                }

                if should_render {
                    renderer.render_with(&md, width, &self.app_theme, &opts, viewport);
                    self.list.preview_content_width = Some(width);
                    self.list.pending_markdown_resize = None;
                }
                self.list.preview_content = Some(PreviewContent::Markdown(Box::new(renderer)));
                self.list.preview_content_height = Some(height);
                self.list.preview_content_scale = Some(self.list.preview_scale);
                self.list.preview_content_offset_x = Some(self.list.preview_offset_x);
                self.list.preview_content_offset_y = Some(self.list.preview_offset_y);
                self.list.preview_content_index = Some(self.list.visual_index);
            }
            Some(VisualItem::Folder { path, .. }) if Self::is_subnotes_parent_grid_path(path) => {
                // Parent-note subfolder selected: show local graph.
                let parent_id = Self::subnotes_parent_id_from_grid_path(path).to_string();
                self.list.preview_content = Some(PreviewContent::SubnoteGraph { parent_id });
                self.list.preview_content_index = Some(self.list.visual_index);
            }
            Some(VisualItem::Subnote {
                parent_id,
                subnote_idx,
                ..
            }) => {
                let sub = self
                    .subnotes_view_cache
                    .iter()
                    .find(|(p, _)| p == parent_id)
                    .and_then(|(_, subs)| subs.get(*subnote_idx));
                if let Some(sub) = sub {
                    let width = self.desired_list_preview_width();
                    let mut renderer = match self.list.preview_content.take() {
                        Some(PreviewContent::Markdown(r)) => *r,
                        _ => MarkdownRenderer::new(),
                    };
                    let opts = crate::markdown::MdRenderOpts::from_config(&self.config);
                    let md = format!("# {}\n\n{}", sub.title, sub.content);
                    let height = self.desired_list_preview_height();
                    renderer.set_page_height(height as usize);
                    let viewport = crate::markdown::RenderViewport {
                        start: renderer.visible_start(),
                        height: height as usize,
                    };

                    let content_changed = renderer.is_changed(&md, &self.app_theme, &opts);
                    let mut should_render = false;
                    if content_changed || renderer.document().is_none() {
                        should_render = true;
                    } else if let Some(old_w) = self.list.preview_content_width {
                        if old_w == width {
                            renderer.set_viewport(viewport.start, viewport.height);
                        } else {
                            let now = std::time::Instant::now();
                            if let Some((w, _)) = self.list.pending_markdown_resize {
                                if w != width {
                                    self.list.pending_markdown_resize = Some((width, now));
                                }
                            } else {
                                self.list.pending_markdown_resize = Some((width, now));
                            }
                        }
                    } else {
                        should_render = true;
                    }

                    if should_render {
                        renderer.render_with(&md, width, &self.app_theme, &opts, viewport);
                        self.list.preview_content_width = Some(width);
                        self.list.pending_markdown_resize = None;
                    }
                    self.list.preview_content = Some(PreviewContent::Markdown(Box::new(renderer)));
                    self.list.preview_content_height = Some(height);
                    self.list.preview_content_scale = Some(self.list.preview_scale);
                    self.list.preview_content_offset_x = Some(self.list.preview_offset_x);
                    self.list.preview_content_offset_y = Some(self.list.preview_offset_y);
                }
                self.list.preview_content_index = Some(self.list.visual_index);
            }
            Some(VisualItem::SmartFolder {
                kind,
                label,
                note_count,
                ..
            }) => {
                let kind = kind.clone();
                let label = label.clone();
                let note_count = *note_count;
                let conditions: Vec<String> = match &kind {
                    SmartFolderKind::Today => {
                        vec!["Notes modified today".into()]
                    }
                    SmartFolderKind::ThisWeek => {
                        vec!["Notes modified this week".into()]
                    }
                    SmartFolderKind::Untagged => {
                        vec!["Notes with no tags".into()]
                    }
                    SmartFolderKind::Tag(t) => {
                        vec![format!("Tag: {t}")]
                    }
                    SmartFolderKind::Tagged => {
                        vec!["Tag-based smart folders grouped together".into()]
                    }
                    SmartFolderKind::Custom(name) => {
                        let mut conds = Vec::new();
                        if let Some(rule) = self
                            .config
                            .list
                            .custom_smart_folders
                            .iter()
                            .find(|r| r.name == *name)
                        {
                            if !rule.tags.is_empty() {
                                conds.push(format!("Tags: {}", rule.tags.join(", ")));
                            }
                            if let Some(ref ti) = rule.title_contains {
                                conds.push(format!("Title contains: \"{ti}\""));
                            }
                            if let Some(ref fp) = rule.folder_prefix {
                                conds.push(format!("Folder prefix: {fp}"));
                            }
                            if let Some(days) = rule.updated_within_days {
                                conds.push(format!(
                                    "Updated within {} {}",
                                    days,
                                    if days == 1 { "day" } else { "days" }
                                ));
                            }
                        }
                        if conds.is_empty() {
                            conds.push("No conditions configured".into());
                        }
                        conds
                    }
                };
                self.list.preview_content = Some(PreviewContent::SmartFolderInfo {
                    kind,
                    label,
                    note_count,
                    conditions,
                });
                self.list.preview_content_index = Some(self.list.visual_index);
            }
            _ => {
                self.list.preview_content = None;
                self.list.preview_content_index = None;
            }
        }
    }

    pub fn update_editor_markdown_preview(&mut self) {
        if !(self.editor.editor_preview_enabled || self.preview_fullscreen) {
            return;
        }

        let content = self.editor.body.snapshot();
        let width = self.desired_editor_preview_width();
        let mut renderer = self.editor.md_preview_renderer.take().unwrap_or_default();
        let mut opts = crate::markdown::MdRenderOpts::from_config(&self.config);
        opts.wrap = self.config.editor.soft_wrap;
        let height = self.desired_editor_preview_height();
        let viewport = crate::markdown::RenderViewport {
            start: renderer.visible_start(),
            height: height as usize,
        };

        let content_changed = renderer.is_changed(&content, &self.app_theme, &opts);
        let mut should_render = false;
        if content_changed || renderer.document().is_none() {
            should_render = true;
        } else if let Some(old_w) = self.editor.preview_content_width {
            if old_w == width {
                renderer.set_viewport(viewport.start, viewport.height);
            } else {
                let now = std::time::Instant::now();
                if let Some((w, _)) = self.editor.pending_markdown_resize {
                    if w != width {
                        self.editor.pending_markdown_resize = Some((width, now));
                    }
                } else {
                    self.editor.pending_markdown_resize = Some((width, now));
                }
            }
        } else {
            should_render = true;
        }

        if should_render {
            renderer.render_with(&content, width, &self.app_theme, &opts, viewport);
            self.editor.preview_content_width = Some(width);
            self.editor.pending_markdown_resize = None;
        }
        self.editor.md_preview_renderer = Some(renderer);
        self.editor.preview_content_height = Some(height);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_app() -> App {
        let temp_dir = tempfile::tempdir().unwrap();
        let data_dir = temp_dir.path().join("data");
        let config_dir = temp_dir.path().join("config");
        let notes_dir = temp_dir.path().join("notes");
        let templates_dir = temp_dir.path().join("templates");
        std::fs::create_dir_all(&data_dir).unwrap();
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(&notes_dir).unwrap();
        std::fs::create_dir_all(&templates_dir).unwrap();

        let storage = crate::storage::Storage {
            data_dir,
            config_dir,
            notes_dir,
            templates_dir,
            key: [0u8; 32],
            skip_dir_patterns: Vec::new(),
        };
        App::new(storage).unwrap()
    }

    #[test]
    fn folder_graph_children_real_folder() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let mut app = make_app();

        // Create folder structure: docs/ with a.md, b.md; docs/sub/ with c.md
        let docs_dir = app.storage.notes_dir.join("docs");
        let sub_dir = app.storage.notes_dir.join("docs/sub");
        std::fs::create_dir_all(&docs_dir).unwrap();
        std::fs::create_dir_all(&sub_dir).unwrap();
        std::fs::write(docs_dir.join("a.md"), "# A\n\n[[b]]").unwrap();
        std::fs::write(docs_dir.join("b.md"), "# B\n\nContent").unwrap();
        std::fs::write(sub_dir.join("c.md"), "# C\n\nContent").unwrap();

        let load = crate::app::catalog::load_notes_blocking(
            &app.storage,
            &app.notes_worker_pool,
            false,
            false,
        )
        .unwrap();
        app.notes = load.summaries;
        app.catalog_folders = load.folders;
        app.sort_notes();
        app.refresh_visual_list();

        // docs/ should have 3 children: 1 subfolder + 2 notes
        let (children, label) = app.folder_graph_children("docs");
        assert_eq!(label, "docs");
        assert_eq!(
            children.len(),
            3,
            "expected 3 children (1 subfolder + 2 notes), got {children:?}"
        );

        let subfolder_count = children.iter().filter(|c| !c.is_note).count();
        let note_count = children.iter().filter(|c| c.is_note).count();
        assert_eq!(
            subfolder_count, 1,
            "expected 1 subfolder, got {subfolder_count}"
        );
        assert_eq!(note_count, 2, "expected 2 notes, got {note_count}");

        // docs/sub should have 1 note child (no subfolders)
        let (children, label) = app.folder_graph_children("docs/sub");
        assert_eq!(label, "sub");
        assert_eq!(children.len(), 1);
        assert!(children[0].is_note);
    }

    #[test]
    fn test_request_editor_preview_update_sets_change_timestamp() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let mut app = make_app();

        assert!(app.editor.last_editor_change.is_none());

        app.editor.editor_preview_enabled = false;
        app.preview_fullscreen = false;

        app.request_editor_preview_update();

        assert!(app.editor.last_editor_change.is_some());
    }

    #[test]
    fn preview_resize_debounce_rerenders_list_and_editor() {
        let _lock = crate::config::ConfigTestGuard::lock();
        let mut app = make_app();
        std::fs::create_dir_all(&app.storage.notes_dir).unwrap();
        std::fs::write(
            app.storage.notes_dir.join("preview.md"),
            "# Preview\n\nBody",
        )
        .unwrap();
        let load = crate::app::catalog::load_notes_blocking(
            &app.storage,
            &app.notes_worker_pool,
            false,
            false,
        )
        .unwrap();
        app.notes = load.summaries;
        app.catalog_folders = load.folders;
        app.sort_notes();
        app.refresh_visual_list();
        app.list.preview_enabled = true;
        app.list.last_preview_pane_width = 80;
        app.list.last_preview_pane_height = 24;
        app.update_preview();
        app.list.last_preview_pane_width = 100;
        app.list.pending_markdown_resize = Some((
            100,
            Instant::now()
                .checked_sub(Duration::from_millis(51))
                .unwrap(),
        ));
        app.poll_renderers();
        assert_eq!(
            app.list.preview_content_width,
            Some(app.desired_list_preview_width())
        );
        assert!(app.list.pending_markdown_resize.is_none());

        let mut app = make_app();
        app.editor.editor_preview_enabled = true;
        app.editor.body = crate::editor_document::EditorDocument::from_text("# Preview\n\nBody");
        app.editor.last_preview_pane_width = 80;
        app.editor.last_preview_pane_height = 24;
        app.update_editor_markdown_preview();
        app.editor.last_preview_pane_width = 100;
        app.editor.pending_markdown_resize = Some((
            100,
            Instant::now()
                .checked_sub(Duration::from_millis(51))
                .unwrap(),
        ));
        app.poll_editor_renderers();
        assert_eq!(
            app.editor.preview_content_width,
            Some(app.desired_editor_preview_width())
        );
        assert!(app.editor.pending_markdown_resize.is_none());
    }
}
