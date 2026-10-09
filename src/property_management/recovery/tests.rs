use super::*;
use crate::property_management::{BindingChange, HeaderChange, ReferenceOperation};

fn storage(root: &Path) -> Storage {
    Storage {
        data_dir: root.into(),
        notes_dir: root.into(),
        config_dir: root.into(),
        templates_dir: root.join(".clin/templates"),
        key: [7; 32],
        skip_dir_patterns: Vec::new(),
        rename_on_title_change: true,
    }
}
fn fixture() -> Result<(tempfile::TempDir, Storage)> {
    let dir = tempfile::tempdir()?;
    fs::create_dir_all(dir.path().join(".clin/templates"))?;
    fs::write(
        dir.path().join(".clin/properties.toml"),
        "[properties.parent]\ntype = 'note_reference'\n",
    )?;
    fs::write(
        dir.path().join("old.md"),
        "---\ntitle: Old\n---\nSecret body",
    )?;
    fs::write(
        dir.path().join("child.md"),
        "---\ntitle: Child\nparent: '[[old.md#Heading|Alias]]' # keep\n---\nChild body",
    )?;
    let storage = storage(dir.path());
    Ok((dir, storage))
}
fn prepared(storage: &Storage) -> Result<PropertyBatch> {
    let mut batch = storage.prepare_reference_relocation(&std::collections::HashMap::from([(
        "old.md".into(),
        "new.md".into(),
    )]))?;
    batch.prepare_file(
        storage,
        "old.md",
        "new.md",
        b"---\ntitle: New\n---\nSecret body".to_vec(),
    )?;
    Ok(batch)
}

#[test]
fn interruption_after_destination_write_replays_without_duplicate_or_plaintext_artifacts()
-> Result<()> {
    let _guard = crate::config::ConfigTestGuard::lock();
    let (dir, mut storage) = fixture()?;
    crate::config::set_config_path_override(dir.path().join("config.toml"));
    storage.write_editor_draft("old.md", "New", "Secret body", &[])?;
    let batch = prepared(&storage)?;
    batch.checkpoint(&storage)?;
    let journal = internal_path(&storage, "property_batch.toml")?;
    assert!(!fs::read_to_string(&journal)?.contains("Secret body"));
    let artifact = artifact_path(&storage, &batch.writes[0].artifact)?;
    assert!(!String::from_utf8_lossy(&fs::read(&artifact)?).contains("Secret body"));
    let payload = batch.writes[0].payload(&storage)?;
    durable_write(&dir.path().join("new.md"), &payload.after)?;
    assert!(dir.path().join("old.md").exists());
    let mut resumed: PropertyBatch = toml::from_str(&fs::read_to_string(&journal)?)?;
    let (failures, writes) = resumed.apply_storage(&storage)?;
    assert_eq!(failures.len(), 0);
    assert!(writes > 0);
    assert!(!dir.path().join("old.md").exists());
    assert!(fs::read_to_string(dir.path().join("child.md"))?.contains("[[new.md#Heading|Alias]]"));
    storage.recover_editor_draft()?;
    assert_eq!(storage.load_note("new.md")?.content, "Secret body");
    assert!(!dir.path().join("New.md").exists());
    assert_eq!(storage.list_note_ids(true, false)?.len(), 2);
    assert!(!journal.exists());
    assert!(!artifact.exists());
    // Replaying a header already committed before its checkpoint writes nothing.
    let header = storage.load_frontmatter("child.md")?.expect("header");
    let mut legacy = PropertyBatch {
        notes: vec![HeaderChange {
            id: "child.md".into(),
            before: Some(header.replace("new.md", "old.md")),
            after: header,
        }],
        ..Default::default()
    };
    assert_eq!(legacy.apply_storage(&storage)?, (Vec::new(), 0));
    Ok(())
}

#[test]
fn conflicts_preserve_journal_artifacts_and_refuse_other_mutations() -> Result<()> {
    let _guard = crate::config::ConfigTestGuard::lock();
    let (dir, mut storage) = fixture()?;
    crate::config::set_config_path_override(dir.path().join("config.toml"));
    let original = fs::read(dir.path().join("old.md"))?;
    let mut batch = prepared(&storage)?;
    fs::write(dir.path().join("new.md"), "External destination")?;
    let (failures, writes) = batch.apply_storage(&storage)?;
    assert_eq!(writes, 0);
    assert_eq!(failures.len(), 1);
    let path = internal_path(&storage, "property_batch.toml")?;
    let saved = fs::read(&path)?;
    assert!(storage.move_note("child.md", "elsewhere").is_err());
    assert_eq!(fs::read(&path)?, saved);
    assert_eq!(fs::read(dir.path().join("old.md"))?, original);
    fs::remove_file(dir.path().join("new.md"))?;
    fs::write(dir.path().join("old.md"), "External source")?;
    assert_eq!(batch.apply_storage(&storage)?.0.len(), 1);
    assert!(!dir.path().join("new.md").exists());
    fs::write(dir.path().join("old.md"), original)?;
    assert_eq!(batch.apply_storage(&storage)?.0.len(), 0);
    assert!(!path.exists());
    Ok(())
}

#[test]
fn validate_all_targets_versions_vault_identity_and_locks_before_writes() -> Result<()> {
    let _guard = crate::config::ConfigTestGuard::lock();
    let (dir, storage) = fixture()?;
    crate::config::set_config_path_override(dir.path().join("config.toml"));
    let original = fs::read(dir.path().join("child.md"))?;
    let mut batch = prepared(&storage)?;
    batch.version = Some(99);
    batch.checkpoint(&storage)?;
    let journal = internal_path(&storage, "property_batch.toml")?;
    let saved = fs::read(&journal)?;
    assert!(batch.apply_storage(&storage).is_err());
    assert_eq!(fs::read(&journal)?, saved);
    let other = tempfile::tempdir()?;
    let other_storage = self::storage(other.path());
    assert!(batch.apply_storage(&other_storage).is_err());
    batch.version = Some(1);
    batch.checkpoint(&storage)?;
    let lock_path = internal_path(&storage, "property_batch.lock")?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path)?;
    lock.lock()?;
    assert!(batch.apply_storage(&storage).is_err());
    lock.unlock()?;
    fs::remove_file(&journal)?;
    let mut unsafe_batch = PropertyBatch {
        notes: vec![HeaderChange {
            id: "child.md".into(),
            before: storage.load_frontmatter("child.md")?,
            after: "---\ntitle: Child\nparent: new.md\n---\n".into(),
        }],
        bindings: vec![BindingChange {
            path: dir.path().join("old.md"),
            create: false,
            before: String::new(),
            after: "Overwrite body".into(),
        }],
        ..Default::default()
    };
    assert!(unsafe_batch.apply_storage(&storage).is_err());
    assert_eq!(fs::read(dir.path().join("child.md"))?, original);
    unsafe_batch.bindings.clear();
    unsafe_batch.notes[0].id = "../escape.md".into();
    assert!(unsafe_batch.apply_storage(&storage).is_err());
    #[cfg(unix)]
    {
        let outside = tempfile::tempdir()?;
        fs::write(outside.path().join("note.md"), "External")?;
        std::os::unix::fs::symlink(outside.path(), dir.path().join("link"))?;
        unsafe_batch.notes[0].id = "link/note.md".into();
        assert!(unsafe_batch.apply_storage(&storage).is_err());
        assert_eq!(
            fs::read_to_string(outside.path().join("note.md"))?,
            "External"
        );
    }
    assert!(!dir.path().join("new.md").exists());
    Ok(())
}

#[test]
fn folder_intent_detects_changes_and_preserves_hidden_files_and_empty_dirs() -> Result<()> {
    let _guard = crate::config::ConfigTestGuard::lock();
    let (dir, storage) = fixture()?;
    crate::config::set_config_path_override(dir.path().join("config.toml"));
    fs::create_dir_all(dir.path().join("folder/empty"))?;
    fs::rename(dir.path().join("old.md"), dir.path().join("folder/old.md"))?;
    fs::write(dir.path().join("folder/.asset.bin"), [0, 255, 1])?;
    fs::write(
        dir.path().join("child.md"),
        "---\nparent: folder/old.md\n---\nBody",
    )?;
    let relocations =
        std::collections::HashMap::from([("folder/old.md".into(), "archive/old.md".into())]);
    let mut batch = storage.prepare_reference_relocation(&relocations)?;
    batch.parents.extend(relocations);
    batch.prepare_folder(&storage, "folder", "archive")?;
    fs::write(dir.path().join("folder/later.bin"), "Added after preview")?;
    assert_eq!(batch.apply_storage(&storage)?.0.len(), 1);
    assert!(dir.path().join("folder/old.md").exists());
    assert!(!dir.path().join("archive").exists());
    fs::remove_file(dir.path().join("folder/later.bin"))?;
    assert_eq!(batch.apply_storage(&storage)?.0.len(), 0);
    assert_eq!(
        fs::read(dir.path().join("archive/.asset.bin"))?,
        [0, 255, 1]
    );
    assert!(dir.path().join("archive/empty").is_dir());
    assert!(fs::read_to_string(dir.path().join("child.md"))?.contains("parent: archive/old.md"));
    Ok(())
}

#[test]
fn committed_output_without_checkpoint_is_idempotent_and_conversion_requires_preview() -> Result<()>
{
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    let _guard = crate::config::ConfigTestGuard::lock();
    let (dir, mut storage) = fixture()?;
    crate::config::set_config_path_override(dir.path().join("config.toml"));
    let mut batch = prepared(&storage)?;
    batch.checkpoint(&storage)?;
    let payload = batch.writes[0].payload(&storage)?;
    assert!(batch.writes[0].apply(&mut storage, &payload)?);
    assert!(!dir.path().join("old.md").exists());
    assert_eq!(batch.apply_storage(&storage)?, (Vec::new(), 1));
    let mut app = App::new(storage)?;
    app.ensure_catalog_ready()?;
    let original = fs::read(dir.path().join("new.md"))?;
    app.convert_note_with_preview("new.md", true)?;
    assert!(app.property_manager.is_some());
    app.handle_property_manager_event(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
    assert_eq!(fs::read(dir.path().join("new.md"))?, original);
    app.convert_note_with_preview("new.md", true)?;
    app.handle_property_manager_event(Event::Key(KeyEvent::new(
        KeyCode::Char('s'),
        KeyModifiers::CONTROL,
    )));
    assert!(app.property_manager.is_none());
    let encrypted = fs::read(dir.path().join("new.clin"))?;
    assert!(!String::from_utf8_lossy(&encrypted).contains("Secret body"));
    assert!(
        fs::read_to_string(dir.path().join("child.md"))?.contains("[[new.clin#Heading|Alias]]")
    );
    app.convert_note_with_preview("new.clin", false)?;
    assert!(app.property_manager.is_some());
    app.handle_property_manager_event(Event::Key(KeyEvent::new(
        KeyCode::Char('s'),
        KeyModifiers::CONTROL,
    )));
    assert!(app.property_manager.is_none());
    assert_eq!(app.storage.load_note("new.md")?.content, "Secret body");
    assert!(fs::read_to_string(dir.path().join("child.md"))?.contains("[[new.md#Heading|Alias]]"));
    Ok(())
}

#[test]
fn title_autosave_cancel_retains_draft_blocks_exit_and_explicit_save_reprompts() -> Result<()> {
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    let _guard = crate::config::ConfigTestGuard::lock();
    let (dir, storage) = fixture()?;
    crate::config::set_config_path_override(dir.path().join("config.toml"));
    let original = fs::read(dir.path().join("old.md"))?;
    let child = fs::read(dir.path().join("child.md"))?;
    let mut app = App::new(storage)?;
    app.ensure_catalog_ready()?;
    app.load_and_open_note("old.md", None);
    app.editor.title_editor.select_all();
    app.editor.title_editor.insert_str("New");
    assert!(app.autosave().is_err());
    assert!(matches!(
        app.property_manager
            .as_ref()
            .and_then(|manager| manager.preview.as_ref()),
        Some(crate::property_management::PropertyPreview::References {
            operation: ReferenceOperation::SaveDraft { .. },
            ..
        })
    ));
    assert!(app.storage.editor_draft_path().exists());
    app.handle_property_manager_event(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
    assert!(app.autosave().is_err());
    assert!(app.property_manager.is_none());
    app.back_to_list(Some("old.md"), Some("old.md"));
    assert_eq!(app.mode, crate::app::ViewMode::Edit);
    app.load_and_open_note("child.md", None);
    assert_eq!(app.editor.editing_id.as_deref(), Some("old.md"));
    assert_eq!(fs::read(dir.path().join("old.md"))?, original);
    assert_eq!(fs::read(dir.path().join("child.md"))?, child);
    // Explicit save clears suppression; confirmation uses same frozen preflight.
    app.editor.reference_save_decision = None;
    assert!(app.autosave().is_err());
    app.handle_property_manager_event(Event::Key(KeyEvent::new(
        KeyCode::Char('s'),
        KeyModifiers::CONTROL,
    )));
    assert!(app.property_manager.is_none());
    assert_eq!(app.editor.editing_id.as_deref(), Some("New.md"));
    assert!(!app.storage.editor_draft_path().exists());
    assert!(!dir.path().join("old.md").exists());
    assert!(fs::read_to_string(dir.path().join("child.md"))?.contains("[[New.md#Heading|Alias]]"));
    Ok(())
}
