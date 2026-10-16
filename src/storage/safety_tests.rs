use super::*;

fn fixture(root: &Path) -> Storage {
    Storage {
        data_dir: root.to_owned(),
        notes_dir: root.to_owned(),
        config_dir: root.join("config"),
        templates_dir: root.join("templates"),
        skip_dir_patterns: Vec::new(),
        frontmatter_policy: crate::frontmatter::WritePolicy::default(),
    }
}

fn recovery_count(root: &Path) -> usize {
    fs::read_dir(root.join(".clin/document-recovery-v2"))
        .unwrap()
        .count()
}

#[test]
fn ordinary_saves_reject_external_body_and_frontmatter_changes() -> Result<()> {
    let root = tempfile::tempdir()?;
    let mut storage = fixture(root.path());
    let path = root.path().join("stable.md");
    fs::write(&path, "---\ntitle: Original\ncustom: retained\n---\nbefore")?;
    let mut note = storage.load_note("stable.md")?;
    note.content = "my edits".into();
    let external = "---\ntitle: Other editor\ncustom: external\n---\nexternal edits";
    fs::write(&path, external)?;
    assert!(storage.save_note("stable.md", &note).is_err());
    assert_eq!(fs::read_to_string(path)?, external);
    Ok(())
}

#[test]
fn title_edits_keep_filename_and_unknown_properties() -> Result<()> {
    let root = tempfile::tempdir()?;
    let mut storage = fixture(root.path());
    fs::write(
        root.path().join("linked.md"),
        "---\ntitle: Old title\ncustom: keep-me\n---\nbefore",
    )?;
    let mut note = storage.load_note("linked.md")?;
    note.title = "Different title".into();
    note.content = "after".into();
    assert_eq!(storage.save_note("linked.md", &note)?, "linked.md");
    assert!(!root.path().join("Different title.md").exists());
    let source = fs::read_to_string(root.path().join("linked.md"))?;
    assert!(source.contains("keep-me"));
    assert_eq!(storage.load_note("linked.md")?.content, "after");
    Ok(())
}

#[test]
fn unchanged_note_save_preserves_raw_frontmatter_and_comments() -> Result<()> {
    let root = tempfile::tempdir()?;
    let mut storage = fixture(root.path());
    let source = "---\n# preserve formatting\ntitle: 'Untouched'\ncustom: [a, b]\n---\nbody\n";
    fs::write(root.path().join("untouched.md"), source)?;
    let note = storage.load_note("untouched.md")?;
    storage.save_note("untouched.md", &note)?;
    assert_eq!(
        fs::read_to_string(root.path().join("untouched.md"))?,
        source
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn note_save_preserves_private_mode_and_rejects_readonly_replacement() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir()?;
    let mut storage = fixture(root.path());
    let path = root.path().join("private.md");
    fs::write(&path, "before")?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    let mut note = storage.load_note("private.md")?;
    note.content = "first edit".into();
    storage.save_note("private.md", &note)?;
    assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o600);
    let mut note = storage.load_note("private.md")?;
    note.content = "must not replace readonly".into();
    let before = fs::read(&path)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o400))?;
    assert!(storage.save_note("private.md", &note).is_err());
    assert_eq!(fs::read(&path)?, before);
    assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o400);
    Ok(())
}

#[test]
fn failed_and_conflicting_recovery_retains_original_records() -> Result<()> {
    let root = tempfile::tempdir()?;
    let storage = fixture(root.path());
    let path = root.path().join("conflict.md");
    fs::write(&path, "before")?;
    let baseline = storage.load_document("conflict.md")?;
    storage.write_document_draft(&baseline, "my draft")?;
    fs::write(&path, "external edits")?;
    assert_ne!(storage.recover_documents()?, Vec::<String>::new());
    assert_eq!(fs::read_to_string(&path)?, "external edits");
    assert_eq!(recovery_count(root.path()), 1);
    fs::write(&path, "before")?;
    assert_eq!(storage.recover_documents()?, Vec::<String>::new());
    assert_eq!(fs::read_to_string(&path)?, "my draft");
    assert_eq!(recovery_count(root.path()), 0);
    Ok(())
}

#[cfg(unix)]
#[test]
fn failed_readonly_recovery_keeps_private_draft_until_retry_succeeds() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir()?;
    let storage = fixture(root.path());
    let path = root.path().join("readonly.md");
    fs::write(&path, "before")?;
    storage.write_document_draft(&storage.load_document("readonly.md")?, "after")?;
    let directory = root.path().join(".clin/document-recovery-v2");
    assert_eq!(
        fs::metadata(&directory)?.permissions().mode() & 0o777,
        0o700
    );
    let record = fs::read_dir(&directory)?.next().unwrap()?.path();
    assert_eq!(fs::metadata(&record)?.permissions().mode() & 0o777, 0o600);
    let original_record = fs::read(&record)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o400))?;
    assert_ne!(storage.recover_documents()?, Vec::<String>::new());
    assert_eq!(fs::read_to_string(&path)?, "before");
    assert_eq!(fs::read(&record)?, original_record);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    assert_eq!(storage.recover_documents()?, Vec::<String>::new());
    assert_eq!(fs::read_to_string(path)?, "after");
    assert!(!record.exists());
    Ok(())
}

#[test]
fn malformed_recovery_and_legacy_data_remain_untouched() -> Result<()> {
    let root = tempfile::tempdir()?;
    let mut storage = fixture(root.path());
    let directory = root.path().join(".clin/document-recovery-v2");
    fs::create_dir_all(&directory)?;
    fs::create_dir_all(root.path().join(".clin/document-recovery"))?;
    fs::create_dir_all(root.path().join(".git"))?;
    let files = [
        "legacy.CLIN",
        "key.bin",
        ".clin/editor_draft.bin",
        ".clin/document-recovery/old.bin",
        ".git/marker",
        ".clin/document-recovery-v2/corrupt.bin",
    ];
    for id in files {
        fs::write(root.path().join(id), b"opaque legacy or corrupt data\xff")?;
    }
    assert_ne!(storage.recover_documents()?, Vec::<String>::new());
    assert!(storage.load_note("legacy.CLIN").is_err());
    assert!(storage.rename_note("legacy.CLIN", "new name").is_err());
    assert!(storage.duplicate_note("legacy.CLIN", "").is_err());
    for id in files {
        assert_eq!(
            fs::read(root.path().join(id))?,
            b"opaque legacy or corrupt data\xff"
        );
    }
    Ok(())
}

#[test]
fn corrupt_subnotes_database_is_never_reset_by_plaintext_writes() -> Result<()> {
    let root = tempfile::tempdir()?;
    let mut storage = fixture(root.path());
    fs::create_dir_all(root.path().join(".clin"))?;
    let path = storage.subnotes_db_path();
    let corrupt = b"not a subnotes database\xff";
    fs::write(&path, corrupt)?;
    assert!(storage.set_subnotes("plain.md", &[]).is_err());
    assert_eq!(fs::read(&path)?, corrupt);
    Ok(())
}

#[test]
fn unrelated_plaintext_subnote_writes_preserve_opaque_legacy_payloads() -> Result<()> {
    let root = tempfile::tempdir()?;
    let mut storage = fixture(root.path());
    fs::create_dir_all(root.path().join(".clin"))?;
    let payload = vec![255, 0, 42, 127];
    let map = HashMap::from([(
        "renamed.md".to_owned(),
        SubNotePayload::LegacyEncrypted(payload.clone()),
    )]);
    let mut bytes = bincode::serde::encode_to_vec(&map, bincode::config::standard())?;
    obfuscate(&mut bytes);
    fs::write(storage.subnotes_db_path(), &bytes)?;
    assert!(storage.set_subnotes("renamed.md", &[]).is_err());
    assert_eq!(fs::read(storage.subnotes_db_path())?, bytes);
    storage.set_subnotes(
        "plain.md",
        &[SubNote {
            id: "sub".into(),
            title: "Plain".into(),
            content: "body".into(),
            updated_at: 1,
        }],
    )?;
    let mut after = fs::read(storage.subnotes_db_path())?;
    obfuscate(&mut after);
    let (map, _): (HashMap<String, SubNotePayload>, _) =
        bincode::serde::decode_from_slice(&after, bincode::config::standard())?;
    assert!(
        matches!(map.get("renamed.md"), Some(SubNotePayload::LegacyEncrypted(bytes)) if bytes == &payload)
    );
    assert!(storage.set_subnotes("legacy.CLIN", &[]).is_err());
    Ok(())
}

#[test]
fn file_rename_keeps_source_and_migrates_plain_subnotes_without_overwriting_records() -> Result<()>
{
    let root = tempfile::tempdir()?;
    let mut storage = fixture(root.path());
    let source = "---\n# retained\ntitle: 'Display title'\ncustom: [a, b]\n---\nbody\n";
    fs::write(root.path().join("old.md"), source)?;
    let notes = [SubNote {
        id: "sub".into(),
        title: "Attached".into(),
        content: "body".into(),
        updated_at: 1,
    }];
    storage.set_subnotes("old.md", &notes)?;
    let renamed = storage.rename_note("old.md", "new")?;
    assert_eq!(renamed, "new.md");
    assert_eq!(fs::read_to_string(root.path().join(&renamed))?, source);
    assert_eq!(storage.get_subnotes(&renamed)?.len(), 1);
    assert_eq!(storage.get_subnotes("old.md")?.len(), 0);
    assert!(!root.path().join("old.md").exists());
    storage.set_subnotes("collision.md", &notes)?;
    assert!(storage.rename_note(&renamed, "collision").is_err());
    assert_eq!(fs::read_to_string(root.path().join(&renamed))?, source);
    assert_eq!(storage.get_subnotes(&renamed)?.len(), 1);
    assert_eq!(storage.get_subnotes("collision.md")?.len(), 1);
    fs::write(storage.subnotes_db_path(), b"corrupt")?;
    assert!(storage.rename_note(&renamed, "other").is_err());
    assert!(storage.move_note(&renamed, "folder").is_err());
    assert_eq!(fs::read_to_string(root.path().join(renamed))?, source);
    Ok(())
}

#[test]
fn body_edits_preserve_frontmatter_comments_quotes_and_unknown_values() -> Result<()> {
    let root = tempfile::tempdir()?;
    let mut storage = fixture(root.path());
    let source = "---\r\n# keep header\r\ntitle: 'Display title' # keep quote\r\ncustom: [a, b] # plugin data\r\n---\r\nbody\r\n";
    fs::write(root.path().join("note.md"), source)?;
    let mut note = storage.load_note("note.md")?;
    note.content = "edited body\n".into();
    storage.save_note("note.md", &note)?;
    let saved = fs::read_to_string(root.path().join("note.md"))?;
    assert!(saved.contains("# keep header\r\n"));
    assert!(saved.contains("title: 'Display title' # keep quote\r\n"));
    assert!(saved.contains("custom: [a, b] # plugin data\r\n"));
    assert!(saved.ends_with("---\r\nedited body\r\n"));
    Ok(())
}

#[test]
fn format_migration_never_touches_git_history_or_private_legacy_records() -> Result<()> {
    let root = tempfile::tempdir()?;
    let storage = fixture(root.path());
    for directory in [".git/refs/heads", ".clin/document-recovery"] {
        fs::create_dir_all(root.path().join(directory))?;
        fs::write(root.path().join(directory).join("legacy.pinstar"), "opaque")?;
    }
    storage.migrate_extensions();
    for directory in [".git/refs/heads", ".clin/document-recovery"] {
        assert_eq!(
            fs::read_to_string(root.path().join(directory).join("legacy.pinstar"))?,
            "opaque"
        );
        assert!(!root.path().join(directory).join("legacy.canvas").exists());
    }
    Ok(())
}

#[test]
fn issue_181_body_edit_preserves_schema_and_missing_managed_fields() -> Result<()> {
    let root = tempfile::tempdir()?;
    let mut storage = fixture(root.path());
    let header = "---\r\n# user schema\r\nid: CL-01\r\ntype: claim\r\nstatus: 'supported'\r\nconfidence: high\r\ncreated: 2026-07-20\r\nupdated: 2026-09-19\r\n\r\n---\r\n";
    fs::write(root.path().join("CL-01.md"), format!("{header}Before"))?;
    let mut note = storage.load_note("CL-01.md")?;
    note.content = "After [[CL-02]] [[CL-02|again]] [[CL-03]]".into();
    assert_eq!(storage.save_note("CL-01.md", &note)?, "CL-01.md");
    assert_eq!(
        fs::read_to_string(root.path().join("CL-01.md"))?,
        format!("{header}{}", note.content)
    );
    assert_eq!(
        storage.load_note_summary("CL-01.md")?.links,
        ["CL-02", "CL-03"]
    );
    Ok(())
}
