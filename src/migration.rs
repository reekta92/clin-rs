use anyhow::{Context, Result};
use std::fs;
use std::io::{self, Write};
use std::path::Path;

#[derive(Clone, Copy)]
pub enum ConflictAction {
    Skip,
    SkipAll,
    Overwrite,
    OverwriteAll,
}

fn prompt_conflict_action(file_name: &str) -> Result<ConflictAction> {
    println!("  Conflict: '{file_name}' already exists at destination.");
    print!("  Action? [s]kip, skip [a]ll, [o]verwrite, overwrite a[l]l: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    match input.trim().to_lowercase().as_str() {
        "s" | "skip" | "" => Ok(ConflictAction::Skip),
        "a" | "skip all" => Ok(ConflictAction::SkipAll),
        "o" | "overwrite" => Ok(ConflictAction::Overwrite),
        "l" | "overwrite all" => Ok(ConflictAction::OverwriteAll),
        _ => {
            println!("  Unknown option, skipping...");
            Ok(ConflictAction::Skip)
        }
    }
}

pub fn migrate_file_with_conflict(
    src: &Path,
    dst: &Path,
    display_name: &str,
    current_action: Option<ConflictAction>,
) -> Result<(usize, usize, Option<ConflictAction>)> {
    if dst.exists() {
        let action = match current_action {
            Some(ConflictAction::SkipAll) => ConflictAction::SkipAll,
            Some(ConflictAction::OverwriteAll) => ConflictAction::OverwriteAll,
            _ => prompt_conflict_action(display_name)?,
        };

        match action {
            ConflictAction::Skip | ConflictAction::SkipAll => {
                println!("  Skipped: {display_name}");
                let new_action = if matches!(action, ConflictAction::SkipAll) {
                    Some(ConflictAction::SkipAll)
                } else {
                    current_action
                };
                Ok((0, 1, new_action))
            }
            ConflictAction::Overwrite | ConflictAction::OverwriteAll => {
                crate::fsutil::atomic_copy(src, dst, true)
                    .with_context(|| format!("failed to copy {}", src.display()))?;
                println!("  Overwritten: {display_name}");
                let new_action = if matches!(action, ConflictAction::OverwriteAll) {
                    Some(ConflictAction::OverwriteAll)
                } else {
                    current_action
                };
                Ok((1, 0, new_action))
            }
        }
    } else {
        crate::fsutil::atomic_copy(src, dst, false)
            .with_context(|| format!("failed to copy {}", src.display()))?;
        println!("  Migrated: {display_name}");
        Ok((1, 0, current_action))
    }
}

pub fn migrate_directory_with_conflict(
    src: &Path,
    dst: &Path,
    current_action: Option<ConflictAction>,
) -> Result<(usize, usize, Option<ConflictAction>)> {
    let source = src.canonicalize()?;
    fs::create_dir_all(dst)?;
    let destination = dst.canonicalize()?;
    anyhow::ensure!(
        !destination.starts_with(&source),
        "Migration destination must not be inside the source"
    );
    migrate_tree(&source, &destination, current_action)
}

fn migrate_tree(
    src: &Path,
    dst: &Path,
    mut current_action: Option<ConflictAction>,
) -> Result<(usize, usize, Option<ConflictAction>)> {
    anyhow::ensure!(
        !fs::symlink_metadata(dst)?.file_type().is_symlink(),
        "Migration destination must not be a symlink: {}",
        dst.display()
    );
    let mut migrated = 0;
    let mut skipped = 0;
    for entry in fs::read_dir(src).with_context(|| format!("failed to read {}", src.display()))? {
        let entry = entry?;
        let name = entry.file_name();
        if src.file_name().is_some_and(|name| name == ".clin")
            && matches!(
                name.to_str(),
                Some("write.lock" | "recovery.lock" | "calendar.lock")
            )
        {
            continue;
        }
        let source = entry.path();
        let target = dst.join(&name);
        let kind = entry.file_type()?;
        anyhow::ensure!(
            !kind.is_symlink(),
            "Migration cannot copy symlink: {}",
            source.display()
        );
        let (m, s, action) = if kind.is_dir() {
            match fs::symlink_metadata(&target) {
                Ok(metadata) => anyhow::ensure!(
                    metadata.is_dir() && !metadata.file_type().is_symlink(),
                    "Migration directory collision: {}",
                    target.display()
                ),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
                        fs::DirBuilder::new()
                            .mode(entry.metadata()?.permissions().mode())
                            .create(&target)?;
                    }
                    #[cfg(not(unix))]
                    fs::create_dir(&target)?;
                }
                Err(error) => return Err(error.into()),
            }
            migrate_tree(&source, &target, current_action)?
        } else {
            anyhow::ensure!(
                kind.is_file(),
                "Migration source is not a regular file: {}",
                source.display()
            );
            migrate_file_with_conflict(&source, &target, &name.to_string_lossy(), current_action)?
        };
        migrated += m;
        skipped += s;
        current_action = action;
    }
    Ok((migrated, skipped, current_action))
}

/// Copy the complete vault, including attachments, subnotes and recovery records.
/// Native coordination files stay local; active writers block migration.
pub fn migrate_note_files_with_conflict(
    src: &Path,
    dst: &Path,
    current_action: Option<ConflictAction>,
) -> Result<(usize, usize, Option<ConflictAction>)> {
    migrate_vault(src, dst, current_action, false)
}

pub(crate) fn migrate_legacy_vault_with_conflict(
    src: &Path,
    dst: &Path,
    current_action: Option<ConflictAction>,
) -> Result<(usize, usize, Option<ConflictAction>)> {
    migrate_vault(src, dst, current_action, true)
}

fn migrate_vault(
    src: &Path,
    dst: &Path,
    current_action: Option<ConflictAction>,
    legacy_layout: bool,
) -> Result<(usize, usize, Option<ConflictAction>)> {
    let source = src.canonicalize()?;
    fs::create_dir_all(dst)?;
    let destination = dst.canonicalize()?;
    anyhow::ensure!(
        !destination.starts_with(&source) && !source.starts_with(&destination),
        "Migration vaults must not contain each other"
    );
    let storage = |root: &Path| crate::storage::Storage {
        data_dir: root.into(),
        notes_dir: root.into(),
        config_dir: root.into(),
        templates_dir: root.join(".clin/templates"),
        skip_dir_patterns: Vec::new(),
        frontmatter_policy: crate::frontmatter::WritePolicy::default(),
    };
    let source_storage = storage(&source);
    let destination_storage = storage(&destination);
    let _source_write = source_storage.write_guard()?;
    let _source_calendar = source_storage.calendar_guard()?;
    let _source_recovery = source_storage.recovery_guard(true).context(
        "Source vault has live recovery writers; close other clin sessions before migrating",
    )?;
    let _destination_write = destination_storage.write_guard()?;
    let _destination_calendar = destination_storage.calendar_guard()?;
    let _destination_recovery = destination_storage.recovery_guard(true).context(
        "Destination vault has live recovery writers; close other clin sessions before migrating",
    )?;
    if !legacy_layout {
        return migrate_tree(&source, &destination, current_action);
    }
    anyhow::ensure!(
        source.join("notes").is_dir(),
        "Legacy migration requires a notes/ directory"
    );
    let mut migrated = 0;
    let mut skipped = 0;
    let mut action = current_action;
    for (from, to) in [
        ("notes", ""),
        (".clin", ".clin"),
        ("templates", ".clin/templates"),
        (".obsidian", ".obsidian"),
        (".git", ".git"),
    ] {
        let from = source.join(from);
        let to = destination.join(to);
        match fs::symlink_metadata(&from) {
            Ok(metadata) => anyhow::ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "Legacy migration source must be a real directory: {}",
                from.display()
            ),
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        }
        if let Ok(metadata) = fs::symlink_metadata(&to) {
            anyhow::ensure!(
                metadata.is_dir() && !metadata.file_type().is_symlink(),
                "Legacy migration destination must be a real directory: {}",
                to.display()
            );
        }
        fs::create_dir_all(&to)?;
        let (m, s, next) = migrate_tree(&from, &to, action)?;
        migrated += m;
        skipped += s;
        action = next;
    }
    Ok((migrated, skipped, action))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vault_migration_rejects_live_calendar_sync_and_keeps_coordination_files_local() -> Result<()>
    {
        let source = tempfile::tempdir()?;
        let destination = tempfile::tempdir()?;
        let storage = crate::storage::Storage {
            data_dir: source.path().into(),
            notes_dir: source.path().into(),
            config_dir: source.path().into(),
            templates_dir: source.path().join(".clin/templates"),
            skip_dir_patterns: Vec::new(),
            frontmatter_policy: crate::frontmatter::WritePolicy::default(),
        };
        fs::write(source.path().join("a.md"), "body")?;
        let lock = storage.calendar_guard()?;
        assert!(migrate_note_files_with_conflict(source.path(), destination.path(), None).is_err());
        drop(lock);
        assert_eq!(
            migrate_note_files_with_conflict(source.path(), destination.path(), None)?.0,
            1
        );
        Ok(())
    }

    #[test]
    fn vault_migration_rejects_live_recovery_owners() -> Result<()> {
        let source = tempfile::tempdir()?;
        let destination = tempfile::tempdir()?;
        let storage = crate::storage::Storage {
            data_dir: source.path().into(),
            notes_dir: source.path().into(),
            config_dir: source.path().into(),
            templates_dir: source.path().join("templates"),
            skip_dir_patterns: Vec::new(),
            frontmatter_policy: crate::frontmatter::WritePolicy::default(),
        };
        fs::write(source.path().join("note.md"), "before")?;
        let worker = crate::storage::drafts::DraftWorker::spawn(storage)?;
        assert!(
            format!(
                "{:#}",
                migrate_note_files_with_conflict(source.path(), destination.path(), None)
                    .err()
                    .unwrap()
            )
            .contains("live recovery writers")
        );
        assert!(!destination.path().join("note.md").exists());
        drop(worker);
        assert_eq!(
            migrate_note_files_with_conflict(source.path(), destination.path(), None)?.0,
            1
        );
        Ok(())
    }

    #[test]
    fn vault_migration_retains_attachments_bases_hidden_files_and_private_records() -> Result<()> {
        let source = tempfile::tempdir()?;
        let destination = tempfile::tempdir()?;
        for id in [
            "notes/note.md",
            "table.base",
            "attachments/photo.png",
            ".clin/subnotes.bin",
            ".clin/document-recovery-v2/draft.bin",
            ".clin/templates/meeting.toml",
            ".obsidian/types.json",
            ".git/HEAD",
        ] {
            let path = source.path().join(id);
            fs::create_dir_all(path.parent().unwrap())?;
            fs::write(path, id)?;
        }
        let (count, skipped, _) =
            migrate_note_files_with_conflict(source.path(), destination.path(), None)?;
        assert_eq!((count, skipped), (8, 0));
        for id in [
            "notes/note.md",
            "table.base",
            "attachments/photo.png",
            ".clin/subnotes.bin",
            ".clin/document-recovery-v2/draft.bin",
            ".clin/templates/meeting.toml",
            ".obsidian/types.json",
            ".git/HEAD",
        ] {
            assert_eq!(fs::read_to_string(destination.path().join(id))?, id);
        }
        assert!(
            migrate_note_files_with_conflict(source.path(), &source.path().join("nested"), None)
                .is_err()
        );
        assert!(
            migrate_directory_with_conflict(
                &source.path().join("missing"),
                destination.path(),
                None
            )
            .is_err()
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn migration_rejects_symlink_escapes_and_preserves_conflicting_files() -> Result<()> {
        let source = tempfile::tempdir()?;
        let destination = tempfile::tempdir()?;
        let outside = tempfile::tempdir()?;
        fs::write(outside.path().join("secret"), "private")?;
        std::os::unix::fs::symlink(outside.path(), source.path().join("escape"))?;
        assert!(migrate_directory_with_conflict(source.path(), destination.path(), None).is_err());
        assert!(!destination.path().join("escape/secret").exists());
        fs::write(source.path().join("note.md"), "source")?;
        fs::write(destination.path().join("note.md"), "destination")?;
        let (_, skipped, _) = migrate_file_with_conflict(
            &source.path().join("note.md"),
            &destination.path().join("note.md"),
            "note.md",
            Some(ConflictAction::SkipAll),
        )?;
        assert_eq!(skipped, 1);
        assert_eq!(
            fs::read_to_string(destination.path().join("note.md"))?,
            "destination"
        );
        Ok(())
    }
}
