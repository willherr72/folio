//! Versioned local recovery. Only native-generated snapshot names become paths.
use crate::engine::{PdfEngine, MAX_SOURCE_BYTES};
use crate::types::{Overlay, PagePlan};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use uuid::Uuid;

const MAX_MANIFEST_BYTES: u64 = 32 * 1024 * 1024;
const CLEAR_MARKER: &str = "cleared";

pub struct RecoveryStore {
    root: PathBuf,
    // OS lock ownership lasts for this File's lifetime, including on process exit.
    _lock: File,
    state: Mutex<HashMap<String, SourceSnapshot>>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceSnapshot {
    file: String,
    length: u64,
    checksum: String,
    // Native engine metadata only. Never interpreted as a path to load during recovery.
    #[serde(rename = "originalPath")]
    original_path: PathBuf,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    workspace: Value,
    sources: BTreeMap<String, SourceSnapshot>,
}

impl RecoveryStore {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&root).map_err(io_error)?;
        let root = fs::canonicalize(root).map_err(io_error)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join("recovery.lock"))
            .map_err(io_error)?;
        lock.try_lock().map_err(|error| format!("Recovery is unavailable because another Folio window is using it, or the folder cannot be locked: {error}"))?;
        fs::create_dir_all(root.join("sources")).map_err(io_error)?;
        let sources = fs::canonicalize(root.join("sources")).map_err(io_error)?;
        if sources.parent() != Some(root.as_path()) {
            return Err("Recovery source folder must be inside the recovery folder".into());
        }
        Ok(Self {
            root,
            _lock: lock,
            state: Mutex::new(HashMap::new()),
        })
    }

    pub fn save(&self, engine: &PdfEngine, workspace: Value) -> Result<(), String> {
        let mut cached = self
            .state
            .lock()
            .map_err(|_| "Recovery store lock failed")?;
        let referenced = validate_workspace(&workspace)?;
        if self.root.join(CLEAR_MARKER).exists() {
            // Finish a discard interrupted before cleanup. Its generations must
            // never become fallbacks after the new checkpoint removes the marker.
            for (_, path) in self.generations()? {
                remove_if_exists(&path)?;
            }
            self.remove_source_files(&BTreeSet::new())?;
            cached.clear();
        }
        let mut sources = BTreeMap::new();
        for id in referenced {
            let snapshot = match cached
                .get(&id)
                .filter(|snapshot| self.source_path(snapshot).is_ok())
            {
                Some(snapshot) => snapshot.clone(),
                None => {
                    let bytes = engine
                        .source_bytes(&id)
                        .map_err(|error| format!("Cannot save recovery source: {error}"))?;
                    if bytes.is_empty() || bytes.len() as u64 > MAX_SOURCE_BYTES {
                        return Err("Recovery sources must be between 1 byte and 512 MiB".into());
                    }
                    let snapshot = SourceSnapshot {
                        file: format!("{}.pdf", Uuid::new_v4()),
                        length: bytes.len() as u64,
                        checksum: checksum(&bytes),
                        original_path: engine.source_original_path(&id).map_err(|error| {
                            format!("Cannot preserve recovery source identity: {error}")
                        })?,
                    };
                    atomic_write(&self.root.join("sources").join(&snapshot.file), &bytes)?;
                    cached.insert(id.clone(), snapshot.clone());
                    snapshot
                }
            };
            sources.insert(id, snapshot);
        }
        let manifest = Manifest {
            version: 1,
            workspace,
            sources,
        };
        let bytes = serde_json::to_vec(&manifest).map_err(|error| error.to_string())?;
        if bytes.len() as u64 > MAX_MANIFEST_BYTES {
            return Err("Recovery workspace exceeds the 32 MiB limit".into());
        }
        let generation = self
            .generations()?
            .last()
            .map(|(number, _)| *number)
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("Recovery generation limit reached")?;
        let path = self.root.join(format!("checkpoint-{generation:020}.json"));
        atomic_write(&path, &bytes)?;
        remove_if_exists(&self.root.join(CLEAR_MARKER))?;
        // Commit is complete. Cleanup failures must not turn a durable save into a failure.
        let _ = self.collect_garbage(&path);
        cached.retain(|_, snapshot| self.root.join("sources").join(&snapshot.file).is_file());
        Ok(())
    }

    pub fn load(&self, engine: &PdfEngine) -> Result<Option<Value>, String> {
        let mut cached = self
            .state
            .lock()
            .map_err(|_| "Recovery store lock failed")?;
        if self.root.join(CLEAR_MARKER).exists() {
            return Ok(None);
        }
        let mut errors = Vec::new();
        for (_, path) in self.generations()?.into_iter().rev() {
            match self
                .read_manifest(&path)
                .and_then(|manifest| self.restore(engine, manifest))
            {
                Ok((workspace, remapped)) => {
                    cached.extend(remapped);
                    return if workspace["tabs"].as_array().is_some_and(Vec::is_empty) {
                        Ok(None)
                    } else {
                        Ok(Some(workspace))
                    };
                }
                Err(error) => errors.push(error),
            }
        }
        if errors.is_empty() {
            Ok(None)
        } else {
            Err(format!("Saved recovery could not be restored. {}. You can discard this recovery and open your PDFs again.", errors[0]))
        }
    }

    pub fn clear(&self) -> Result<(), String> {
        let mut cached = self
            .state
            .lock()
            .map_err(|_| "Recovery store lock failed")?;
        // Commit discard before cleanup, so interrupted cleanup never exposes old work.
        atomic_write(&self.root.join(CLEAR_MARKER), b"1")?;
        for (_, path) in self.generations()? {
            remove_if_exists(&path)?;
        }
        self.remove_source_files(&BTreeSet::new())?;
        cached.clear();
        Ok(())
    }

    fn generations(&self) -> Result<Vec<(u64, PathBuf)>, String> {
        let mut generations = Vec::new();
        for entry in fs::read_dir(&self.root).map_err(io_error)? {
            let entry = entry.map_err(io_error)?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Some(number) = name
                .strip_prefix("checkpoint-")
                .and_then(|name| name.strip_suffix(".json"))
            {
                if number.len() == 20 && number.bytes().all(|byte| byte.is_ascii_digit()) {
                    if let Ok(number) = number.parse::<u64>() {
                        generations.push((number, entry.path()));
                    }
                }
            }
        }
        generations.sort_by_key(|(number, _)| *number);
        Ok(generations)
    }

    fn read_manifest(&self, path: &Path) -> Result<Manifest, String> {
        let bytes = read_bounded(path, MAX_MANIFEST_BYTES)
            .map_err(|error| format!("Recovery checkpoint is unreadable: {error}"))?;
        let manifest: Manifest = serde_json::from_slice(&bytes)
            .map_err(|error| format!("Recovery checkpoint is corrupt: {error}"))?;
        if manifest.version != 1 {
            return Err("Recovery checkpoint version is unsupported".into());
        }
        let referenced = validate_workspace(&manifest.workspace)?;
        if referenced != manifest.sources.keys().cloned().collect() {
            return Err("Recovery source mapping does not match the workspace".into());
        }
        for snapshot in manifest.sources.values() {
            self.source_path(snapshot)?;
        }
        Ok(manifest)
    }

    fn source_path(&self, snapshot: &SourceSnapshot) -> Result<PathBuf, String> {
        if !valid_source_name(&snapshot.file)
            || snapshot.length == 0
            || snapshot.length > MAX_SOURCE_BYTES
            || !snapshot.original_path.is_absolute()
            || snapshot
                .original_path
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err("Recovery source metadata is invalid".into());
        }
        let path = self.root.join("sources").join(&snapshot.file);
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            format!(
                "Recovery source {} is missing or unreadable: {error}",
                snapshot.file
            )
        })?;
        if !metadata.is_file() || metadata.len() != snapshot.length {
            return Err(format!(
                "Recovery source {} is incomplete or redirected",
                snapshot.file
            ));
        }
        let resolved = fs::canonicalize(&path).map_err(io_error)?;
        if resolved.parent() != Some(self.root.join("sources").as_path()) {
            return Err("Recovery source path is outside the recovery folder".into());
        }
        Ok(path)
    }

    fn read_source(&self, snapshot: &SourceSnapshot) -> Result<Vec<u8>, String> {
        let path = self.source_path(snapshot)?;
        let bytes = read_bounded(&path, MAX_SOURCE_BYTES)?;
        if checksum(&bytes) != snapshot.checksum {
            return Err(format!("Recovery source {} is corrupt", snapshot.file));
        }
        Ok(bytes)
    }

    fn restore(
        &self,
        engine: &PdfEngine,
        mut manifest: Manifest,
    ) -> Result<(Value, HashMap<String, SourceSnapshot>), String> {
        let mut opened = HashMap::new();
        let result = (|| {
            for (old_id, snapshot) in &manifest.sources {
                // Full source reads and checksums happen only when restoring;
                // routine autosaves reuse immutable snapshots using metadata.
                self.read_source(snapshot)?;
                let source = engine
                    .open_recovery_document(
                        self.root.join("sources").join(&snapshot.file),
                        &snapshot.original_path,
                    )
                    .map_err(|error| format!("Recovery source could not be opened: {error}"))?;
                opened.insert(old_id.clone(), source);
            }
            for tab in manifest.workspace["tabs"]
                .as_array_mut()
                .ok_or("Invalid recovery tabs")?
            {
                for page in tab["document"]["pages"]
                    .as_array_mut()
                    .ok_or("Invalid recovery pages")?
                {
                    let source = opened
                        .get(page["sourceId"].as_str().ok_or("Invalid recovery source")?)
                        .ok_or("Missing recovery source")?;
                    let index = page["pageIndex"]
                        .as_u64()
                        .ok_or("Invalid recovery page index")?
                        as usize;
                    let size = source
                        .pages
                        .get(index)
                        .ok_or("Recovery page is missing from its source")?;
                    if (page["width"].as_f64().unwrap_or(0.0) - size.width as f64).abs() > 0.25
                        || (page["height"].as_f64().unwrap_or(0.0) - size.height as f64).abs()
                            > 0.25
                    {
                        return Err("Recovery page dimensions do not match its source".into());
                    }
                    page["sourceId"] = json!(source.id);
                }
            }
            Ok(())
        })();
        if let Err(error) = result {
            for source in opened.values() {
                let _ = engine.close_document(&source.id);
            }
            return Err(error);
        }
        let remapped = opened
            .into_iter()
            .map(|(old, source)| (source.id, manifest.sources[&old].clone()))
            .collect();
        Ok((manifest.workspace, remapped))
    }

    fn collect_garbage(&self, latest: &Path) -> Result<(), String> {
        let mut retained = Vec::new();
        let mut sources = BTreeSet::new();
        for (_, path) in self.generations()?.into_iter().rev() {
            if retained.len() < 2 {
                if let Ok(manifest) = self.read_manifest(&path) {
                    sources.extend(manifest.sources.into_values().map(|source| source.file));
                    retained.push(path.clone());
                    continue;
                }
            }
            if path == latest {
                return Err("New recovery checkpoint could not be verified".into());
            }
        }
        for (_, path) in self.generations()? {
            if !retained.contains(&path) {
                remove_if_exists(&path)?;
            }
        }
        self.remove_source_files(&sources)
    }

    fn remove_source_files(&self, retained: &BTreeSet<String>) -> Result<(), String> {
        for entry in fs::read_dir(self.root.join("sources")).map_err(io_error)? {
            let entry = entry.map_err(io_error)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if valid_source_name(&name) && !retained.contains(&name) {
                // Remove individual owned files only; never recurse through a path.
                fs::remove_file(entry.path()).map_err(io_error)?;
            }
        }
        Ok(())
    }
}

fn validate_workspace(workspace: &Value) -> Result<BTreeSet<String>, String> {
    if serde_json::to_vec(workspace)
        .map_err(|error| error.to_string())?
        .len() as u64
        > MAX_MANIFEST_BYTES
    {
        return Err("Recovery workspace exceeds the 32 MiB limit".into());
    }
    if workspace["version"].as_u64() != Some(1) {
        return Err("Unsupported recovery workspace version".into());
    }
    let tabs = workspace["tabs"]
        .as_array()
        .ok_or("Recovery workspace requires tabs")?;
    if tabs.len() > 100 {
        return Err("Recovery workspace has too many tabs".into());
    }
    let mut tab_ids = BTreeSet::new();
    let mut sources = BTreeSet::new();
    let mut total_pages = 0;
    for tab in tabs {
        let id = string_field(tab, "id", 1024)?;
        if !tab_ids.insert(id) {
            return Err("Duplicate recovery tab id".into());
        }
        string_field(tab, "savedDigest", MAX_MANIFEST_BYTES as usize)?;
        if !tab["dirty"].is_boolean() {
            return Err("Recovery tab requires its dirty state".into());
        }
        number_field(tab, "zoom", 50.0, 200.0)?;
        number_field(&tab["scrollPosition"], "top", 0.0, 1_000_000_000.0)?;
        number_field(&tab["scrollPosition"], "left", 0.0, 1_000_000_000.0)?;
        let document = &tab["document"];
        string_field(document, "name", 32_768)?;
        let pages = document["pages"]
            .as_array()
            .ok_or("Recovery document requires pages")?;
        total_pages += pages.len();
        if pages.is_empty() || total_pages > 100_000 {
            return Err("Invalid recovery page count".into());
        }
        let mut page_ids = BTreeSet::new();
        let mut overlay_ids = BTreeSet::new();
        for value in pages {
            let mut page_overlay_ids = BTreeSet::new();
            let page: PagePlan = serde_json::from_value(value.clone())
                .map_err(|error| format!("Invalid recovery page: {error}"))?;
            if page.id.is_empty()
                || page.id.len() > 1024
                || !page_ids.insert(page.id.clone())
                || page.source_id.is_empty()
                || page.source_id.len() > 1024
                || !matches!(page.rotation, 0 | 90 | 180 | 270)
                || !page.width.is_finite()
                || page.width <= 0.0
                || !page.height.is_finite()
                || page.height <= 0.0
                || page.page_index > i32::MAX as usize
                || page.overlays.len() > 10_000
            {
                return Err("Invalid recovery page metadata".into());
            }
            for overlay in &page.overlays {
                let (id, color) = match overlay {
                    Overlay::Text(text) => {
                        if text.text.len() > 1_000_000
                            || !finite_coordinate(text.x)
                            || !finite_coordinate(text.y)
                            || !text.font_size.is_finite()
                            || text.font_size <= 0.0
                            || text.font_size > 512.0
                        {
                            return Err("Invalid recovery text overlay".into());
                        }
                        (&text.id, &text.color)
                    }
                    Overlay::Ink(ink) => {
                        if !ink.stroke_width.is_finite()
                            || ink.stroke_width <= 0.0
                            || ink.stroke_width > 100.0
                            || ink.paths.is_empty()
                            || ink.paths.len() > 100_000
                            || ink.paths.iter().any(|path| {
                                path.len() < 2
                                    || path.len() > 1_000_000
                                    || path.iter().any(|point| {
                                        !finite_coordinate(point.x) || !finite_coordinate(point.y)
                                    })
                            })
                        {
                            return Err("Invalid recovery ink overlay".into());
                        }
                        (&ink.id, &ink.color)
                    }
                };
                if id.is_empty()
                    || id.len() > 1024
                    || !page_overlay_ids.insert(id.clone())
                    || color.len() != 7
                    || !color.starts_with('#')
                    || !color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
                {
                    return Err("Invalid recovery overlay metadata".into());
                }
                overlay_ids.insert(id.clone());
            }
            sources.insert(page.source_id);
        }
        for (key, allowed) in [
            ("selectedPageId", &page_ids),
            ("selectedOverlayId", &overlay_ids),
        ] {
            if !document.get(key).is_some_and(|value| {
                value.is_null() || value.as_str().is_some_and(|id| allowed.contains(id))
            }) {
                return Err(format!("Invalid recovery {key}"));
            }
        }
    }
    let active = workspace
        .get("activeId")
        .ok_or("Recovery workspace requires activeId")?;
    if (tabs.is_empty() && !active.is_null())
        || (!tabs.is_empty() && !active.as_str().is_some_and(|id| tab_ids.contains(id)))
    {
        return Err("Invalid recovery active tab".into());
    }
    Ok(sources)
}

fn string_field<'a>(value: &'a Value, key: &str, limit: usize) -> Result<&'a str, String> {
    value[key]
        .as_str()
        .filter(|text| text.len() <= limit)
        .ok_or_else(|| format!("Invalid recovery {key}"))
}

fn number_field(value: &Value, key: &str, minimum: f64, maximum: f64) -> Result<(), String> {
    if value[key]
        .as_f64()
        .is_some_and(|number| number.is_finite() && number >= minimum && number <= maximum)
    {
        Ok(())
    } else {
        Err(format!("Invalid recovery {key}"))
    }
}

fn finite_coordinate(number: f32) -> bool {
    number.is_finite() && number.abs() <= 10_000_000.0
}

fn valid_source_name(name: &str) -> bool {
    name.strip_suffix(".pdf")
        .is_some_and(|id| Uuid::parse_str(id).is_ok_and(|uuid| uuid.to_string() == id))
}

// Stable FNV-1a detects accidental corruption; this is an integrity check, not authentication.
fn checksum(bytes: &[u8]) -> String {
    let hash = bytes.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ *byte as u64).wrapping_mul(0x100000001b3)
    });
    format!("{hash:016x}")
}

fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(io_error)?;
    if file.metadata().map_err(io_error)?.len() > limit {
        return Err("Recovery file exceeds its size limit".into());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() as u64 > limit {
        return Err("Recovery file exceeds its size limit".into());
    }
    Ok(bytes)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("Recovery destination has no parent")?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".folio-recovery-")
        .tempfile_in(parent)
        .map_err(io_error)?;
    temporary.write_all(bytes).map_err(io_error)?;
    temporary.as_file().sync_all().map_err(io_error)?;
    temporary
        .persist(path)
        .map_err(|error| io_error(error.error))?;
    #[cfg(unix)]
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(io_error)?;
    Ok(())
}

fn remove_if_exists(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(error)),
    }
}

fn io_error(error: std::io::Error) -> String {
    format!("Recovery file operation failed: {error}")
}
