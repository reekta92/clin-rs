//! Per-vault events. Note associations live in note properties so renames retain them.
use super::ical::{self, Occurrence};
use crate::storage::Storage;
use anyhow::{Context, Result, ensure};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, io::Read, path::Path, sync::mpsc};

const STORE_LIMIT: usize = 64 * 1024 * 1024;
pub const NOTE_PROPERTY: &str = "calendar-events";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Collection {
    pub account: String,
    pub url: String,
    pub name: String,
    pub writable: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Remote {
    pub href: String,
    pub etag: String,
    pub source: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Conflict {
    pub remote: Option<Remote>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Resource {
    pub id: String,
    pub source: String,
    pub collection: Option<String>,
    pub remote: Option<Remote>,
    pub deleted: bool,
    pub conflict: Option<Conflict>,
}

impl Resource {
    pub fn new(source: String, collection: Option<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            source,
            collection,
            remote: None,
            deleted: false,
            conflict: None,
        }
    }

    pub fn dirty(&self) -> bool {
        self.deleted || self.remote.as_ref().is_none_or(|r| r.source != self.source)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Data {
    pub version: u32,
    pub resources: Vec<Resource>,
    pub collections: Vec<Collection>,
}

impl Default for Data {
    fn default() -> Self {
        Self {
            version: 1,
            resources: Vec::new(),
            collections: Vec::new(),
        }
    }
}

pub struct Store {
    pub data: Data,
    baseline: Option<Vec<u8>>,
}

fn read_store(storage: &Storage) -> Result<Option<Vec<u8>>> {
    let path = storage.data_dir.join(".clin/calendar.json");
    match fs::symlink_metadata(&path) {
        Ok(metadata) => ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "Calendar store must be a regular non-symlink file"
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take((STORE_LIMIT + 1) as u64)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= STORE_LIMIT, "Calendar store exceeds 64 MiB");
    Ok(Some(bytes))
}

impl Store {
    pub fn load(storage: &Storage) -> Result<Self> {
        let baseline = read_store(storage)?;
        let data: Data = baseline
            .as_ref()
            .map(|b| serde_json::from_slice(b))
            .transpose()
            .context("Cannot read calendar store; original retained")?
            .unwrap_or_default();
        ensure!(
            data.version == 1,
            "Unsupported calendar store version; original retained"
        );
        let mut ids = std::collections::HashSet::new();
        for r in &data.resources {
            ensure!(
                ids.insert(&r.id) && uuid::Uuid::parse_str(&r.id).is_ok(),
                "Invalid or duplicate calendar resource ID"
            );
            ical::uid(&ical::parse(&r.source)?)?;
        }
        Ok(Self { data, baseline })
    }

    /// Caller holds calendar_guard through read/modify/write, including a sync's network requests.
    pub fn save_locked(&mut self, storage: &Storage, data: Data) -> Result<()> {
        ensure!(
            read_store(storage)? == self.baseline,
            "Calendar changed on disk; reload before saving"
        );
        let bytes = serde_json::to_vec_pretty(&data)?;
        ensure!(bytes.len() <= STORE_LIMIT, "Calendar store exceeds 64 MiB");
        crate::fsutil::atomic_write_with_mode(
            &storage.data_dir.join(".clin/calendar.json"),
            &bytes,
            0o600,
        )?;
        self.data = data;
        self.baseline = Some(bytes);
        Ok(())
    }
}

#[derive(Default)]
pub struct Service {
    pub store: Option<Store>,
    cache: Option<(NaiveDate, NaiveDate, Vec<Occurrence>, Vec<String>)>,
    job: Option<mpsc::Receiver<Result<String>>>,
}

impl Service {
    pub fn ensure_loaded(&mut self, storage: &Storage) -> Result<()> {
        if self.store.is_none() {
            self.store = Some(Store::load(storage)?);
        }
        Ok(())
    }

    pub fn data(&self) -> &Data {
        &self.store.as_ref().expect("calendar loaded").data
    }

    pub fn reload(&mut self, storage: &Storage) -> Result<()> {
        let store = Store::load(storage)?;
        self.store = Some(store);
        self.cache = None;
        Ok(())
    }

    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    pub fn update<T>(
        &mut self,
        storage: &Storage,
        edit: impl FnOnce(&mut Data) -> Result<T>,
    ) -> Result<T> {
        ensure!(!self.busy(), "Calendar sync in progress");
        self.ensure_loaded(storage)?;
        let _guard = storage.calendar_guard()?;
        let store = self.store.as_mut().expect("calendar loaded");
        let mut data = store.data.clone();
        let result = edit(&mut data)?;
        store.save_locked(storage, data)?;
        self.cache = None;
        Ok(result)
    }

    pub fn events(
        &mut self,
        storage: &Storage,
        from: NaiveDate,
        until: NaiveDate,
    ) -> Result<(&[Occurrence], &[String])> {
        self.ensure_loaded(storage)?;
        if self
            .cache
            .as_ref()
            .is_none_or(|(a, b, _, _)| *a != from || *b != until)
        {
            let mut events = Vec::new();
            let mut warnings = Vec::new();
            for r in self.data().resources.iter().filter(|r| !r.deleted) {
                match ical::occurrences(&r.source, &r.id, from, until) {
                    Ok(items) => events.extend(items),
                    Err(error) => warnings.push(format!(
                        "{}: {error:#} (source retained; export available)",
                        r.id
                    )),
                }
            }
            events.sort_by_key(|o| (o.start, !o.all_day, o.title.clone(), o.resource.clone()));
            self.cache = Some((from, until, events, warnings));
        }
        let (_, _, events, warnings) = self.cache.as_ref().expect("populated above");
        Ok((events, warnings))
    }

    pub fn import(&mut self, storage: &Storage, path: &Path) -> Result<usize> {
        let mut source = String::new();
        fs::File::open(path)?
            .take((ical::MAX_BYTES + 1) as u64)
            .read_to_string(&mut source)?;
        let resources = ical::split_import(&source)?;
        // Validate every series before committing any of the import.
        for source in &resources {
            ical::Draft::from_source(source)?;
            let date = chrono::Local::now().date_naive();
            ical::occurrences(
                source,
                "import",
                date,
                date.succ_opt().context("Date out of range")?,
            )?;
        }
        self.update(storage, |data| {
            let mut count = 0;
            for source in resources {
                let uid = ical::uid(&ical::parse(&source)?)?;
                if let Some(existing) = data.resources.iter().find(|r| !r.deleted && r.collection.is_none() && ical::parse(&r.source).and_then(|d| ical::uid(&d)).is_ok_and(|id| id == uid)) {
                    ensure!(ical::parse(&existing.source)?.to_string() == source, "Event UID {uid} already exists with different content; existing event retained");
                    continue;
                }
                data.resources.push(Resource::new(source, None));
                count += 1;
            }
            Ok(count)
        })
    }

    pub fn export(&mut self, storage: &Storage, path: &Path) -> Result<usize> {
        self.ensure_loaded(storage)?;
        let mut out = ical::parse(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//clin//Calendar//EN\r\nEND:VCALENDAR\r\n",
        )?;
        let mut count = 0;
        let mut zones = BTreeMap::new();
        let mut uids = std::collections::HashSet::new();
        for r in self.data().resources.iter().filter(|r| !r.deleted) {
            ensure!(
                uids.insert(ical::uid(&ical::parse(&r.source)?)?),
                "Calendars contain duplicate event UIDs; export would be ambiguous"
            );
            for c in ical::parse(&r.source)?.components {
                if c.name == "VTIMEZONE" {
                    let id = ical::value(&c, "TZID")
                        .context("VTIMEZONE requires TZID")?
                        .to_owned();
                    let source = ical::Document {
                        properties: Vec::new(),
                        components: vec![c.clone()],
                    }
                    .to_string();
                    if let Some(previous) = zones.insert(id, source.clone()) {
                        ensure!(
                            previous == source,
                            "Calendars contain conflicting timezone definitions"
                        );
                        continue;
                    }
                }
                out.components.push(c);
            }
            count += 1;
        }
        let source = out.to_string();
        ensure!(
            source.len() <= ical::MAX_BYTES,
            "Export exceeds 16 MiB; existing file retained"
        );
        crate::fsutil::atomic_create(path, source.as_bytes())
            .context("Export requires a new file path; existing files are never overwritten")?;
        Ok(count)
    }

    pub fn sync(&mut self, storage: &Storage, accounts: Vec<super::dav::Account>) -> Result<()> {
        ensure!(!self.busy(), "Calendar sync already running");
        ensure!(
            !accounts.is_empty(),
            "Configure [[calendar.caldav]] first; see docs/CALENDAR.md"
        );
        self.ensure_loaded(storage)?;
        let storage = storage.clone();
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("calendar-sync".into())
            .spawn(move || {
                let result = (|| {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()?;
                    runtime.block_on(super::dav::sync(&storage, &accounts))
                })();
                let _ = tx.send(result);
            })?;
        self.job = Some(rx);
        Ok(())
    }

    pub fn poll(&mut self, storage: &Storage) -> Option<Result<String>> {
        let result = match self.job.as_ref()?.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err(anyhow::anyhow!("Calendar sync worker stopped"))
            }
        };
        self.job = None;
        Some(match self.reload(storage) {
            Ok(()) => result,
            Err(error) => Err(error),
        })
    }
}

pub fn note_links(storage: &Storage) -> Result<BTreeMap<String, Vec<String>>> {
    let metadata = storage.load_vault_metadata(&std::sync::atomic::AtomicBool::new(false))?;
    let mut links: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for file in metadata.files() {
        if let Ok(properties) = file.properties()
            && let Some(ids) = properties
                .get(NOTE_PROPERTY)
                .and_then(serde_yaml_ng::Value::as_sequence)
        {
            for id in ids.iter().filter_map(serde_yaml_ng::Value::as_str) {
                links
                    .entry(id.to_owned())
                    .or_default()
                    .push(file.id.clone());
            }
        }
    }
    Ok(links)
}

pub fn toggle_note_link(storage: &Storage, note: &str, resource: &str) -> Result<()> {
    let baseline = storage.load_document(note)?;
    let parsed = crate::frontmatter::properties::PropertyDocument::parse(baseline.source())?;
    let mut ids = match parsed.properties.get(NOTE_PROPERTY) {
        None => Vec::new(),
        Some(value) => value
            .as_sequence()
            .context("calendar-events must be a list of event IDs")?
            .clone(),
    };
    ensure!(
        ids.iter().all(|v| v.as_str().is_some()),
        "calendar-events must contain strings"
    );
    if ids.iter().any(|v| v.as_str() == Some(resource)) {
        ids.retain(|v| v.as_str() != Some(resource));
    } else {
        ids.push(serde_yaml_ng::Value::String(resource.into()));
    }
    storage.save_properties(
        &baseline,
        &[(
            NOTE_PROPERTY.into(),
            (!ids.is_empty()).then_some(serde_yaml_ng::Value::Sequence(ids)),
        )],
    )?;
    Ok(())
}
