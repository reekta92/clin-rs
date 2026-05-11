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
    println!("  Conflict: '{}' already exists at destination.", file_name);
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
    if !src.exists() {
        return Ok((0, 0, current_action));
    }

    if dst.exists() {
        let action = match current_action {
            Some(ConflictAction::SkipAll) => ConflictAction::SkipAll,
            Some(ConflictAction::OverwriteAll) => ConflictAction::OverwriteAll,
            _ => prompt_conflict_action(display_name)?,
        };

        match action {
            ConflictAction::Skip | ConflictAction::SkipAll => {
                println!("  Skipped: {}", display_name);
                let new_action = if matches!(action, ConflictAction::SkipAll) {
                    Some(ConflictAction::SkipAll)
                } else {
                    current_action
                };
                Ok((0, 1, new_action))
            }
            ConflictAction::Overwrite | ConflictAction::OverwriteAll => {
                fs::copy(src, dst).with_context(|| format!("failed to copy {}", src.display()))?;
                println!("  Overwritten: {}", display_name);
                let new_action = if matches!(action, ConflictAction::OverwriteAll) {
                    Some(ConflictAction::OverwriteAll)
                } else {
                    current_action
                };
                Ok((1, 0, new_action))
            }
        }
    } else {
        fs::copy(src, dst).with_context(|| format!("failed to copy {}", src.display()))?;
        println!("  Migrated: {}", display_name);
        Ok((1, 0, current_action))
    }
}

pub fn migrate_directory_with_conflict(
    src: &Path,
    dst: &Path,
    mut current_action: Option<ConflictAction>,
) -> Result<(usize, usize, Option<ConflictAction>)> {
    let mut migrated = 0;
    let mut skipped = 0;

    for entry in fs::read_dir(src).with_context(|| format!("failed to read {}", src.display()))? {
        let entry = entry?;
        let src_path = entry.path();
        let file_name = entry.file_name();
        let dst_path = dst.join(&file_name);
        let display_name = file_name.to_string_lossy().to_string();

        if src_path.is_dir() {
            fs::create_dir_all(&dst_path)
                .with_context(|| format!("failed to create {}", dst_path.display()))?;
            let (m, s, action) =
                migrate_directory_with_conflict(&src_path, &dst_path, current_action)?;
            migrated += m;
            skipped += s;
            current_action = action;
        } else if src_path.is_file() {
            let (m, s, action) =
                migrate_file_with_conflict(&src_path, &dst_path, &display_name, current_action)?;
            migrated += m;
            skipped += s;
            current_action = action;
        }
    }

    Ok((migrated, skipped, current_action))
}
