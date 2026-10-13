//! Guarded property transactions and bounded session undo. No note serialization or decryption.

use crate::{
    frontmatter::properties::{PropertyDocument, edit_properties, parse_mapping},
    storage::{
        Storage,
        documents::DocumentSnapshot,
        metadata::{PropertyTypes, VaultMetadata},
    },
};
use anyhow::{Context, Result, bail};
use serde_yaml_ng::Value;
use std::sync::atomic::{AtomicBool, Ordering};

pub const MAX_HISTORY_BYTES: usize = 32 * 1024 * 1024;
#[derive(Clone, Debug)]
pub struct Change {
    pub before: DocumentSnapshot,
    pub after: DocumentSnapshot,
}
#[derive(Clone, Debug)]
pub struct Transaction {
    pub changes: Vec<Change>,
}
impl Transaction {
    pub fn bytes(&self) -> usize {
        self.changes
            .iter()
            .map(|change| change.before.source().len() + change.after.source().len())
            .sum()
    }
    pub fn replay(&self, storage: &Storage, forward: bool, cancelled: &AtomicBool) -> Result<()> {
        let plans = self
            .changes
            .iter()
            .map(|change| {
                if forward {
                    (change.before.clone(), change.after.source().to_owned())
                } else {
                    (change.after.clone(), change.before.source().to_owned())
                }
            })
            .collect();
        apply(storage, plans, cancelled).map(|_| ())
    }
}

pub fn property_key(column: &str) -> Result<&str> {
    if column.starts_with("file.") || column.starts_with("formula.") {
        bail!("file metadata and formulas are read-only");
    }
    let key = column.strip_prefix("note.").unwrap_or(column);
    if key.is_empty() || key.chars().any(char::is_control) {
        bail!("invalid note property name");
    }
    Ok(key)
}

pub fn parse_input(
    types: &PropertyTypes,
    key: &str,
    source: &str,
    current: Option<&Value>,
) -> Result<Value> {
    let text = matches!(types.declared_type(key), Some("text" | "date" | "datetime"))
        || (types.declared_type(key).is_none() && current.is_some_and(Value::is_string));
    let value = if text {
        Value::String(source.to_owned())
    } else {
        let mapping = parse_mapping(&format!("value: {source}"))?;
        if mapping.len() != 1 {
            bail!("cell input must contain one YAML value");
        }
        mapping
            .get(Value::String("value".into()))
            .cloned()
            .context("cell input requires a value")?
    };
    types.validate(key, &value)?;
    Ok(value)
}

/// Called when edit starts, not when it finishes: prompt input cannot silently refresh its baseline.
pub fn prepare(
    storage: &Storage,
    metadata: &VaultMetadata,
    ids: &[String],
    cancelled: &AtomicBool,
) -> Result<Vec<DocumentSnapshot>> {
    let mut snapshots = Vec::with_capacity(ids.len());
    let mut bytes = 0usize;
    for id in ids {
        if cancelled.load(Ordering::Relaxed) {
            bail!("property edit cancelled");
        }
        let file = metadata
            .file(id)
            .context("edit target is absent from public metadata")?;
        if file.extension != "md" {
            bail!("{id}: only Markdown note properties are editable");
        }
        let snapshot = storage.load_document(id)?;
        if &PropertyDocument::parse(snapshot.source())?.properties != file.properties()? {
            bail!("{id}: properties changed; refresh Bases before editing");
        }
        bytes = bytes
            .checked_add(snapshot.source().len() * 2)
            .context("edit size overflow")?;
        if bytes > MAX_HISTORY_BYTES {
            bail!("bulk edit exceeds 32 MiB undo budget; select a smaller batch");
        }
        snapshots.push(snapshot);
    }
    if snapshots.is_empty() {
        bail!("no editable note selected");
    }
    Ok(snapshots)
}

pub fn commit(
    storage: &Storage,
    snapshots: Vec<DocumentSnapshot>,
    changes: &[(String, Option<Value>)],
    cancelled: &AtomicBool,
) -> Result<Transaction> {
    let types = storage.load_property_types()?;
    for (key, value) in changes {
        if let Some(value) = value {
            types.validate(key, value)?;
        }
    }
    let plans = snapshots
        .into_iter()
        .map(|snapshot| {
            let source = edit_properties(snapshot.source(), changes)?;
            Ok((snapshot, source))
        })
        .collect::<Result<Vec<_>>>()?;
    let bytes: usize = plans
        .iter()
        .map(|(baseline, source)| baseline.source().len() + source.len())
        .sum();
    if bytes > MAX_HISTORY_BYTES {
        bail!("bulk edit exceeds 32 MiB undo budget; original files retained");
    }
    apply(storage, plans, cancelled)
}

fn apply(
    storage: &Storage,
    plans: Vec<(DocumentSnapshot, String)>,
    cancelled: &AtomicBool,
) -> Result<Transaction> {
    // Preflight every target. Each actual write rechecks baseline and uses permission-preserving atomic replacement.
    for (baseline, _) in &plans {
        if storage.load_document(baseline.id())?.source() != baseline.source() {
            bail!(
                "{}: document changed on disk; reload before saving",
                baseline.id()
            );
        }
        if std::fs::metadata(storage.document_path(baseline.id())?)?
            .permissions()
            .readonly()
        {
            bail!("{}: document is read-only", baseline.id());
        }
    }
    if cancelled.load(Ordering::Relaxed) {
        bail!("property edit cancelled");
    }
    let journal = storage.begin_document_transaction(&plans)?;
    let mut transaction = Transaction {
        changes: Vec::new(),
    };
    for (before, source) in plans {
        let result = if cancelled.load(Ordering::Relaxed) {
            Err(anyhow::anyhow!("property edit cancelled"))
        } else {
            storage.save_document(&before, &source)
        };
        match result {
            Ok(after) => {
                if before.source() != after.source() {
                    transaction.changes.push(Change { before, after });
                }
            }
            Err(error) => {
                let mut retain = true;
                if let Ok(after) = storage.load_document(before.id()) {
                    retain = after.source() != before.source() && after.source() != source;
                    if after.source() == source && before.source() != source {
                        transaction.changes.push(Change { before, after });
                    }
                }
                return abort(storage, transaction, journal, error, retain);
            }
        }
    }
    if cancelled.load(Ordering::Relaxed) {
        return abort(
            storage,
            transaction,
            journal,
            anyhow::anyhow!("property edit cancelled"),
            false,
        );
    }
    let entries = transaction
        .changes
        .iter()
        .map(|change| {
            (
                change.before.id(),
                change.before.source(),
                change.after.source(),
            )
        })
        .collect();
    if let Err(error) = storage.finish_document_transaction(&journal, entries) {
        return abort(storage, transaction, journal, error, false);
    }
    Ok(transaction)
}

fn abort(
    storage: &Storage,
    transaction: Transaction,
    journal: crate::storage::document_recovery::DocumentTransaction,
    error: anyhow::Error,
    retain: bool,
) -> Result<Transaction> {
    let mut failures = Vec::new();
    for change in transaction.changes.iter().rev() {
        if let Err(rollback) = storage.save_document(&change.after, change.before.source()) {
            failures.push(format!("{}: {rollback:#}", change.before.id()));
        }
    }
    if retain || !failures.is_empty() {
        bail!(
            "{error:#}; encrypted recovery record retained; inspect affected files: {}",
            failures.join("; ")
        );
    }
    storage.abort_document_transaction(journal)?;
    Err(error.context("property transaction aborted; completed writes rolled back"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_bulk_edits_undo_redo_preserve_bodies_and_reject_stale_or_private_targets() {
        let (vault, storage) = super::super::tests::vault();
        for id in ["a.md", "b.md"] {
            std::fs::write(
                vault.path().join(id),
                "---\n# retain\nscore: 1\nunknown: {x: 2}\n---\nBody\n",
            )
            .unwrap();
        }
        std::fs::write(vault.path().join("secret.clin"), "private").unwrap();
        let cancelled = AtomicBool::new(false);
        let metadata = storage.load_vault_metadata(&cancelled).unwrap();
        assert!(prepare(&storage, &metadata, &["secret.clin".into()], &cancelled).is_err());
        assert!(property_key("formula.score").is_err());
        let snapshots = prepare(
            &storage,
            &metadata,
            &["a.md".into(), "b.md".into()],
            &cancelled,
        )
        .unwrap();
        let original = snapshots[0].source().to_owned();
        let transaction = commit(
            &storage,
            snapshots,
            &[("score".into(), Some(Value::Number(2.into())))],
            &cancelled,
        )
        .unwrap();
        for change in &transaction.changes {
            assert!(change.after.source().contains("# retain"));
            assert!(change.after.source().ends_with("Body\n"));
            assert!(!change.after.source().contains("updated_at"));
        }
        transaction.replay(&storage, false, &cancelled).unwrap();
        assert_eq!(storage.load_document("a.md").unwrap().source(), original);
        transaction.replay(&storage, true, &cancelled).unwrap();
        std::fs::write(vault.path().join("b.md"), "External edit\n").unwrap();
        assert!(transaction.replay(&storage, false, &cancelled).is_err());
        assert_eq!(
            storage.load_document("a.md").unwrap().source(),
            transaction.changes[0].after.source()
        );
    }
    #[test]
    fn type_input_and_cancelled_bulk_commit_keep_original_files() {
        let (vault, storage) = super::super::tests::vault();
        std::fs::create_dir(vault.path().join(".obsidian")).unwrap();
        std::fs::write(
            vault.path().join(".obsidian/types.json"),
            r#"{"types":{"score":"number","title":"text","done":"checkbox"}}"#,
        )
        .unwrap();
        std::fs::write(vault.path().join("a.md"), "---\nscore: 1\n---\nBody\n").unwrap();
        let types = storage.load_property_types().unwrap();
        assert!(parse_input(&types, "score", "wrong", None).is_err());
        assert_eq!(
            parse_input(&types, "title", "[literal]", None).unwrap(),
            Value::String("[literal]".into())
        );
        let baseline = storage.load_document("a.md").unwrap();
        assert!(
            commit(
                &storage,
                vec![baseline.clone()],
                &[("score".into(), Some(Value::Number(2.into())))],
                &AtomicBool::new(true)
            )
            .is_err()
        );
        assert_eq!(
            storage.load_document("a.md").unwrap().source(),
            baseline.source()
        );
    }
}
