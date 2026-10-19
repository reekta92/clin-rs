use super::*;
use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};

fn fixture(root: &std::path::Path) -> Storage {
    Storage {
        data_dir: root.into(),
        notes_dir: root.into(),
        config_dir: root.join("config"),
        templates_dir: root.join("templates"),
        skip_dir_patterns: Vec::new(),
        frontmatter_policy: crate::frontmatter::WritePolicy::default(),
    }
}

#[test]
fn recovery_waits_until_all_live_workers_release_the_vault() -> Result<()> {
    let root = tempfile::tempdir()?;
    let storage = fixture(root.path());
    fs::write(root.path().join("note.md"), "before")?;
    let first = DraftWorker::spawn(storage.clone())?;
    let second = DraftWorker::spawn(storage.clone())?;
    first.submit(storage.load_document("note.md")?, "after".into())?;
    first.flush()?;
    assert!(storage.editor_drafts()?.is_empty());
    drop(first);
    assert_eq!(storage.recover_documents()?, Vec::<String>::new());
    assert_eq!(fs::read_to_string(root.path().join("note.md"))?, "before");
    drop(second);
    assert_eq!(storage.recover_documents()?, Vec::<String>::new());
    assert_eq!(fs::read_to_string(root.path().join("note.md"))?, "after");
    Ok(())
}

#[test]
fn flush_persists_latest_snapshot_once_and_recovers_it() -> Result<()> {
    let root = tempfile::tempdir()?;
    let storage = fixture(root.path());
    fs::write(root.path().join("note.md"), "before")?;
    let baseline = storage.load_document("note.md")?;
    let worker = DraftWorker::spawn(storage.clone())?;
    for version in 0..100 {
        worker.submit(baseline.clone(), format!("after {version}"))?;
    }
    worker.flush()?;
    assert_eq!(
        fs::read_dir(root.path().join(".clin/document-recovery-v2"))?.count(),
        1
    );
    assert_eq!(fs::read_to_string(root.path().join("note.md"))?, "before");
    assert_eq!(storage.recover_documents()?, Vec::<String>::new());
    assert_eq!(fs::read_to_string(root.path().join("note.md"))?, "before");
    drop(worker);
    assert_eq!(storage.recover_documents()?, Vec::<String>::new());
    assert_eq!(fs::read_to_string(root.path().join("note.md"))?, "after 99");
    Ok(())
}

#[test]
fn failed_jobs_remain_retryable_and_saved_jobs_cannot_resurrect_stale_drafts() -> Result<()> {
    let root = tempfile::tempdir()?;
    let storage = fixture(root.path());
    fs::write(root.path().join("note.md"), "before")?;
    fs::create_dir_all(root.path().join(".clin"))?;
    let blocker = root.path().join(".clin/document-recovery-v2");
    fs::write(&blocker, "not a directory")?;
    let baseline = storage.load_document("note.md")?;
    let worker = DraftWorker::spawn(storage.clone())?;
    worker.submit(baseline.clone(), "pending".into())?;
    assert!(worker.flush().is_err());
    assert!(worker.take_error().is_some());
    fs::remove_file(&blocker)?;
    worker.flush()?;
    assert_eq!(storage.recover_documents()?, Vec::<String>::new());
    assert_eq!(fs::read_to_string(root.path().join("note.md"))?, "before");
    drop(worker);
    assert_eq!(storage.recover_documents()?, Vec::<String>::new());
    assert_eq!(fs::read_to_string(root.path().join("note.md"))?, "pending");

    let baseline = storage.load_document("note.md")?;
    let worker = DraftWorker::spawn(storage.clone())?;
    fs::remove_dir(&blocker)?;
    fs::write(&blocker, "not a directory")?;
    worker.submit(baseline.clone(), "obsolete draft".into())?;
    assert!(worker.flush().is_err());
    let saved = storage.save_document(&baseline, "newer durable content")?;
    worker.forget(saved.recovery_id())?;
    fs::remove_file(&blocker)?;
    worker.flush()?;
    assert!(worker.take_error().is_none());
    assert!(!blocker.exists());
    assert_eq!(
        fs::read_to_string(root.path().join("note.md"))?,
        "newer durable content"
    );
    Ok(())
}

#[test]
fn continuous_input_does_not_postpone_recovery_until_typing_stops() -> Result<()> {
    let root = tempfile::tempdir()?;
    let storage = fixture(root.path());
    fs::write(root.path().join("note.md"), "before")?;
    let baseline = storage.load_document("note.md")?;
    let worker = DraftWorker::spawn(storage)?;
    let stop = AtomicBool::new(false);
    let persisted = thread::scope(|scope| {
        let producer = scope.spawn(|| {
            while !stop.load(Ordering::Relaxed) {
                worker.submit(baseline.clone(), "typing".into()).unwrap();
                thread::sleep(Duration::from_millis(20));
            }
        });
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut persisted = false;
        while Instant::now() < deadline {
            persisted =
                fs::read_dir(root.path().join(".clin/document-recovery-v2")).is_ok_and(|entries| {
                    entries
                        .filter_map(std::result::Result::ok)
                        .any(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
                });
            if persisted {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        stop.store(true, Ordering::Relaxed);
        producer.join().unwrap();
        persisted
    });
    worker.flush()?;
    assert!(
        persisted,
        "recovery must persist while input is still arriving"
    );
    Ok(())
}

#[test]
fn forget_acknowledges_prior_writes_before_record_cleanup() -> Result<()> {
    let root = tempfile::tempdir()?;
    let storage = fixture(root.path());
    fs::write(root.path().join("note.md"), "before")?;
    let baseline = storage.load_document("note.md")?;
    let worker = DraftWorker::spawn(storage.clone())?;
    for _ in 0..20 {
        worker.submit(baseline.clone(), "obsolete".repeat(100_000))?;
        worker.forget(baseline.recovery_id())?;
        storage.clear_document_draft(&baseline)?;
        worker.flush()?;
        assert_eq!(storage.recover_documents()?, Vec::<String>::new());
        assert_eq!(fs::read_to_string(root.path().join("note.md"))?, "before");
    }
    Ok(())
}
