use crate::app::App;
use crate::base::io::{default_base_file, serialize_base};
use crate::popups::{ActivePopup, BaseCreatePopup, BasePickerPopup};
use ratatui_textarea::TextArea;

impl App {
    pub fn begin_create_base(&mut self) {
        let folder = self.get_current_folder_context();
        let input = TextArea::default();
        self.popups.active = Some(ActivePopup::BaseCreate(BaseCreatePopup { folder, input }));
    }

    pub fn confirm_create_base(&mut self) {
        if let Some(ActivePopup::BaseCreate(popup)) = self.popups.active.take() {
            let mut name = popup.input.lines().join("").trim().to_string();
            if name.is_empty() {
                name = "Untitled base".to_string();
            }
            if !name.ends_with(".base") {
                name.push_str(".base");
            }
            let id = if popup.folder.is_empty() {
                name
            } else {
                format!("{}/{}", popup.folder, name)
            };

            let default_base = default_base_file();
            if let Ok(serialized) = serialize_base(&default_base) {
                let note_path = self.storage.note_path(&id);
                if let Some(parent) = note_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if crate::fsutil::atomic_write_str(&note_path, &serialized).is_ok() {
                    let _ = self.refresh_notes();
                    self.open_base_view(id);
                } else {
                    self.set_temporary_status_static("Failed to create base file");
                }
            }
        }
    }

    pub fn begin_open_base(&mut self) {
        let ids = match self.storage.list_note_ids(true, true) {
            Ok(all_ids) => {
                let mut base_ids = Vec::new();
                for id in all_ids {
                    if id.ends_with(".base") {
                        base_ids.push(id);
                    }
                }
                base_ids.sort();
                base_ids
            }
            Err(_) => Vec::new(),
        };

        if ids.is_empty() {
            self.set_temporary_status_static("No bases in vault");
        } else if ids.len() == 1 {
            self.open_base_view(ids[0].clone());
        } else {
            self.popups.active = Some(ActivePopup::BasePicker(BasePickerPopup {
                ids: ids.clone(),
                filtered: ids,
                selected: 0,
                input: TextArea::default(),
            }));
        }
    }
}
