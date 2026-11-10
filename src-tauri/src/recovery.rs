//! Versioned local recovery. Only native-generated snapshot names become paths.
use crate::engine::{PdfEngine, MAX_SOURCE_BYTES};
use crate::private_storage::{self, Purpose};
use crate::types::{Overlay, PagePlan};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use crate::fonts::FontImportScope;
use uuid::Uuid;
#[path="recovery_images.rs"]
mod image_payloads;
use image_payloads::{ImageSnapshot,image_references};

#[cfg(test)]
thread_local! {
    static RESTORE_OBSERVER: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = std::cell::RefCell::new(None);
}
#[cfg(test)]
#[path="recovery_ownership_tests.rs"]
mod ownership_tests;
#[cfg(test)]
#[path="recovery_annotation_sources_tests.rs"]
mod annotation_sources_tests;

const MAX_MANIFEST_BYTES: u64 = 32 * 1024 * 1024;
const MAX_FONT_BYTES: u64 = 16 * 1024 * 1024;
const CLEAR_MARKER: &str = "cleared";
const NEWER_RECOVERY: &str = "This recovery was saved by a newer version of Folio. Open it with that version to continue.";

pub struct RecoveryStore {
    root: PathBuf,
    // OS lock ownership lasts for this File's lifetime, including on process exit.
    _lock: File,
    state: Mutex<HashMap<String, SourceSnapshot>>,
    // Hold the original directory lock too: older app builds use that lock.
    _compatibility_store: Option<Box<RecoveryStore>>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    policy: Option<crate::engine::document_policy::RecoveryPolicy>,
    #[serde(default, rename = "legacyCommentImport", skip_serializing_if = "Option::is_none")]
    legacy_comment_import: Option<bool>,
    // Version4 requires the key. Null explicitly means legacy/unproved ownership.
    #[serde(default, rename = "annotationSources")]
    annotation_sources: Option<crate::engine::imported_overlay_sources::Snapshot>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FontSnapshot {
    file: String,
    length: u64,
    checksum: String,
}

fn annotation_reservation(source: &SourceSnapshot) -> Result<usize,String> {
    source.annotation_sources.as_ref().map(|s|s.metadata_reservation().map_err(|e|e.to_string())).transpose().map(|n|n.unwrap_or(0))
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    #[serde(default, rename = "annotationImportVersion")]
    annotation_import_version: u32,
    workspace: Value,
    sources: BTreeMap<String, SourceSnapshot>,
    #[serde(default)]
    fonts: BTreeMap<String, FontSnapshot>,
    #[serde(default)]
    images: BTreeMap<String, ImageSnapshot>,
}

impl RecoveryStore {
    /// Incompatible annotation ownership is stored beyond older builds' cleanup
    /// paths. Migrate an existing draft once, before this store is published.
    pub fn for_application(root:PathBuf,engine:&PdfEngine)->Result<Self,String> {
        let legacy=Box::new(Self::new(root)?);
        let path=legacy.root.join("comment-format-3");
        fs::create_dir_all(&path).map_err(io_error)?;
        if fs::canonicalize(&path).map_err(io_error)? != path {
            return Err("Recovery format folder must not be redirected".into());
        }
        let mut current=Self::new(path)?;
        if current.generations()?.is_empty() && !current.root.join(CLEAR_MARKER).exists() {
            if let Some((workspace, font_imports))=legacy.load_with_scope(engine, false)? {
                let sources=validate_workspace(&workspace)?;
                let saved=current.save(engine,workspace);
                for id in sources {let _=engine.close_document(&id);}
                drop(font_imports);
                saved?;
                // Only retire the old slot after the private new checkpoint is durable.
                legacy.clear()?;
            }
        }
        current._compatibility_store=Some(legacy);
        Ok(current)
    }

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
        fs::create_dir_all(root.join("fonts")).map_err(io_error)?;
        if fs::canonicalize(root.join("fonts")).map_err(io_error)? != root.join("fonts") {
            return Err("Recovery font folder must be inside the recovery folder".into());
        }
        fs::create_dir_all(root.join("images")).map_err(io_error)?;
        if fs::canonicalize(root.join("images")).map_err(io_error)? != root.join("images") {
            return Err("Recovery image folder must be inside the recovery folder".into());
        }
        Ok(Self {
            root,
            _lock: lock,
            state: Mutex::new(HashMap::new()),
            _compatibility_store: None,
        })
    }

    pub fn save(&self, engine: &PdfEngine, workspace: Value) -> Result<(), String> {
        let mut cached = self
            .state
            .lock()
            .map_err(|_| "Recovery store lock failed")?;
        let referenced = validate_workspace(&workspace)?;
        for tab in workspace["tabs"].as_array().ok_or("Invalid recovery tabs")? {
            for page in tab["document"]["pages"].as_array().ok_or("Invalid recovery pages")? {
                if !page["crop"].is_null() {
                    let plan:PagePlan=serde_json::from_value(page.clone()).map_err(|e|e.to_string())?;
                    engine.validate_page_crop(&plan).map_err(|e|e.to_string())?;
                }
            }
        }
        if !self.root.join(CLEAR_MARKER).exists(){self.ensure_supported_generations()?;}
        self.remove_abandoned_temporaries()?;
        if self.root.join(CLEAR_MARKER).exists() {
            // Finish a discard interrupted before cleanup. Its generations must
            // never become fallbacks after the new checkpoint removes the marker.
            for (_, path) in self.generations()? {
                remove_if_exists(&path)?;
            }
            self.remove_source_files(&BTreeSet::new())?;
            self.remove_font_files(&BTreeSet::new())?;
            self.remove_image_files(&BTreeSet::new())?;
            cached.clear();
        }
        // Individual unusable legacy payloads must not prevent a new checkpoint.
        // Garbage collection removes unreferenced payloads only after commit.
        let _migration_errors = self.migrate_legacy()?;
        let mut sources = BTreeMap::new();
        let mut annotation_budget = crate::engine::imported_overlay_sources::MAX_METADATA;
        for id in referenced {
            let snapshot = match cached
                .get(&id)
                .filter(|snapshot| self.source_path(snapshot).is_ok())
            {
                Some(snapshot) => {
                    if annotation_reservation(snapshot)? > annotation_budget {
                        return Err("Imported annotation ownership metadata exceeds the recovery budget.".into());
                    }
                    let mut snapshot=snapshot.clone();
                    if snapshot.policy.is_none() {
                        snapshot.policy=Some(engine.source_recovery_policy(&id).map_err(|e|e.to_string())?);
                    }
                    if snapshot.legacy_comment_import.is_none() {
                        snapshot.legacy_comment_import=Some(engine.source_legacy_comment_import(&id).map_err(|e|e.to_string())?);
                    }
                    snapshot
                },
                None => {
                    let bytes = engine
                        .source_bytes(&id)
                        .map_err(|error| format!("Cannot save recovery source: {error}"))?;
                    if bytes.is_empty() || bytes.len() as u64 > MAX_SOURCE_BYTES {
                        return Err("Recovery sources must be between 1 byte and 512 MiB".into());
                    }
                    let snapshot = SourceSnapshot {
                        annotation_sources:engine.source_imported_overlay_snapshot_with_budget(&id,annotation_budget).map_err(|e|e.to_string())?,
                        legacy_comment_import:Some(engine.source_legacy_comment_import(&id).map_err(|e|e.to_string())?),
                        policy: Some(engine.source_recovery_policy(&id).map_err(|e|e.to_string())?),
                        file: format!("{}.pdf", Uuid::new_v4()),
                        length: bytes.len() as u64,
                        checksum: checksum(&bytes),
                        original_path: engine.source_original_path(&id).map_err(|error| {
                            format!("Cannot preserve recovery source identity: {error}")
                        })?,
                    };
                    private_write(
                        &self.source_directory()?.join(&snapshot.file),
                        Purpose::Source,
                        &bytes,
                    )?;
                    snapshot
                }
            };
            annotation_budget = annotation_budget.checked_sub(annotation_reservation(&snapshot)?)
                .ok_or("Imported annotation ownership metadata exceeds the recovery budget.")?;
            sources.insert(id, snapshot);
        }
        let mut fonts = BTreeMap::new();
        let font_directory = self.font_directory()?;
        for id in font_reference_ids(&workspace)? {
            let asset = engine
                .fonts()
                .get(&id)
                .map_err(|error| format!("Cannot save recovery font: {error}"))?;
            let snapshot = FontSnapshot {
                file: format!("{id}.font"),
                length: asset.bytes.len() as u64,
                checksum: id.clone(),
            };
            if self.font_path(&id, &snapshot).is_err() {
                private_write(
                    &font_directory.join(&snapshot.file),
                    Purpose::Font,
                    &asset.bytes,
                )?;
            }
            fonts.insert(id, snapshot);
        }
        let images=self.save_images(engine,&workspace,&sources)?;
        let manifest = Manifest {
            version: 4,
            annotation_import_version: 4,
            workspace,
            sources,
            fonts,
            images,
        };
        let mut writer = crate::pdf_output::BoundedWriter::new(MAX_MANIFEST_BYTES as usize);
        serde_json::to_writer(&mut writer,&manifest).map_err(|e|format!("Recovery workspace could not be encoded within 32 MiB: {e}"))?;
        let bytes = writer.finish().map_err(|e|format!("Recovery workspace exceeds its limit: {e}"))?;
        let generation = self
            .generations()?
            .last()
            .map(|(number, _)| *number)
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("Recovery generation limit reached")?;
        let path = self.root.join(format!("checkpoint-{generation:020}.json"));
        private_write(&path, Purpose::Workspace, &bytes)?;
        remove_if_exists(&self.root.join(CLEAR_MARKER))?;
        // Commit is complete. Cleanup failures must not turn a durable save into a failure.
        let _ = self.collect_garbage(&path);
        // Cache only current runtime handles, and only after durable publication.
        // Old disk generations retain their own source payloads independently.
        *cached = manifest.sources.into_iter().collect();
        Ok(())
    }

    pub fn load(&self, engine: &PdfEngine) -> Result<Option<Value>, String> {
        Ok(self.load_with_scope(engine, true)?.map(|(workspace, _)| workspace))
    }

    fn load_with_scope(&self, engine: &PdfEngine, adopt_fonts: bool)
        -> Result<Option<(Value, Arc<FontImportScope>)>, String>
    {
        let mut cached = self
            .state
            .lock()
            .map_err(|_| "Recovery store lock failed")?;
        if self.root.join(CLEAR_MARKER).exists() {
            self.remove_abandoned_temporaries()?;
            return Ok(None);
        }
        self.ensure_supported_generations()?;
        self.remove_abandoned_temporaries()?;
        let migration_errors = self.migrate_legacy()?;
        let mut errors = Vec::new();
        for (_, path) in self.generations()?.into_iter().rev() {
            match self
                .read_manifest(&path)
                .and_then(|manifest| self.restore(engine, manifest, adopt_fonts))
            {
                Ok((workspace, remapped, font_imports)) => {
                    *cached = remapped;
                    // Only clean up failed generations/orphans after finding usable
                    // recovery. Required migration failures remain available for retry
                    // when no generation can restore.
                    let _ = self.collect_garbage(&path);
                    return if workspace["tabs"].as_array().is_some_and(Vec::is_empty) {
                        Ok(None)
                    } else {
                        Ok(Some((workspace, font_imports)))
                    };
                }
                Err(error) => errors.push(error),
            }
        }
        if errors.is_empty() {
            Ok(None)
        } else {
            errors.extend(migration_errors);
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
        self.remove_abandoned_temporaries()?;
        for (_, path) in self.generations()? {
            remove_if_exists(&path)?;
        }
        self.remove_source_files(&BTreeSet::new())?;
        self.remove_font_files(&BTreeSet::new())?;
        self.remove_image_files(&BTreeSet::new())?;
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
        let bytes = read_private(path, Purpose::Workspace, MAX_MANIFEST_BYTES)
            .map_err(|error| format!("Recovery checkpoint is unreadable: {error}"))?;
        let mut value:Value = serde_json::from_slice(&bytes)
            .map_err(|error| format!("Recovery checkpoint is corrupt: {error}"))?;
        // Inspect versions before the strict schema: future formats may add keys.
        if value["version"].as_u64().is_some_and(|v|v>4)
            || value["annotationImportVersion"].as_u64().is_some_and(|v|v>4)
            || value["sources"].as_object().is_some_and(|sources| sources.values().any(|source|
                source["annotationSources"]["version"].as_u64().is_some_and(|v|v>1))) {
            return Err(NEWER_RECOVERY.into());
        }
        if value["annotationImportVersion"].as_u64().is_some_and(|v|v>=4)
            && value["sources"].as_object().is_some_and(|sources| sources.values().any(|s|
                s.as_object().is_none_or(|s|!s.contains_key("annotationSources")))) {
            return Err("Recovery checkpoint is missing annotation ownership metadata.".into());
        }
        let current_ownership=value["annotationImportVersion"].as_u64().is_some_and(|v|v>=4);
        let mut wire_budget=crate::engine::imported_overlay_sources::MAX_METADATA;
        if let Some(sources)=value["sources"].as_object_mut() {
            for source in sources.values_mut() {
                if !current_ownership {
                    if let Some(source)=source.as_object_mut(){source.remove("annotationSources");}
                } else if !source["annotationSources"].is_null() {
                    let charge=crate::engine::imported_overlay_sources::Snapshot::wire_reservation(&source["annotationSources"])
                        .map_err(|e|e.to_string())?;
                    wire_budget=wire_budget.checked_sub(charge)
                        .ok_or("Imported annotation ownership metadata exceeds the recovery budget.")?;
                }
            }
        }
        let mut manifest:Manifest=serde_json::from_value(value)
            .map_err(|error|format!("Recovery checkpoint is corrupt: {error}"))?;
        if !(1..=4).contains(&manifest.version) || manifest.annotation_import_version > 4 {
            return Err("Recovery checkpoint version is unsupported".into());
        }
        if manifest.version >= 2 && manifest.sources.values().any(|s|s.policy.is_none()) {
            return Err("Recovery checkpoint is missing source protection metadata".into());
        }
        if manifest.annotation_import_version >= 3 && manifest.sources.values().any(|s|s.legacy_comment_import.is_none()) {
            return Err("Recovery checkpoint is missing comment ownership metadata".into());
        }
        let mut annotation_budget=crate::engine::imported_overlay_sources::MAX_METADATA;
        for source in manifest.sources.values_mut() {
            if manifest.annotation_import_version < 4 { source.annotation_sources=None; }
            annotation_budget=annotation_budget.checked_sub(annotation_reservation(source)?)
                .ok_or("Imported annotation ownership metadata exceeds the recovery budget.")?;
        }
        let referenced = validate_workspace(&manifest.workspace)?;
        if referenced != manifest.sources.keys().cloned().collect() {
            return Err("Recovery source mapping does not match the workspace".into());
        }
        if font_reference_ids(&manifest.workspace)? != manifest.fonts.keys().cloned().collect()
            || manifest.fonts.len() > 64
            || manifest
                .fonts
                .values()
                .map(|font| font.length)
                .try_fold(0u64, |sum, length| sum.checked_add(length))
                .is_none_or(|sum| sum > 128 * 1024 * 1024)
        {
            return Err(
                "Recovery font mapping does not match the workspace or exceeds its limit".into(),
            );
        }
        for (id, font) in &manifest.fonts {
            self.font_path(id, font)?;
        }
        self.validate_images(&manifest)?;
        for snapshot in manifest.sources.values() {
            self.source_path(snapshot)?;
        }
        Ok(manifest)
    }

    fn font_directory(&self) -> Result<PathBuf, String> {
        let path = self.root.join("fonts");
        if fs::canonicalize(&path).map_err(io_error)? != path {
            return Err("Recovery font folder is redirected".into());
        }
        Ok(path)
    }
    fn font_path(&self, id: &str, snapshot: &FontSnapshot) -> Result<PathBuf, String> {
        if !valid_font_id(id)
            || snapshot.file != format!("{id}.font")
            || snapshot.checksum != id
            || snapshot.length == 0
            || snapshot.length > MAX_FONT_BYTES
        {
            return Err("Recovery font metadata is invalid".into());
        }
        let path = self.font_directory()?.join(&snapshot.file);
        let metadata = fs::symlink_metadata(&path).map_err(io_error)?;
        if !metadata.is_file()
            || private_length(&path, Purpose::Font, MAX_FONT_BYTES)? != snapshot.length
            || fs::canonicalize(&path).map_err(io_error)? != path
        {
            return Err("Recovery font is incomplete or redirected".into());
        }
        Ok(path)
    }
    fn read_font(&self, id: &str, snapshot: &FontSnapshot) -> Result<Vec<u8>, String> {
        use sha2::Digest;
        let bytes = read_private(
            &self.font_path(id, snapshot)?,
            Purpose::Font,
            MAX_FONT_BYTES,
        )?;
        if bytes.len() as u64 != snapshot.length {
            return Err("Recovery font length does not match".into());
        }
        if sha2::Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
            != id
        {
            return Err("Recovery font checksum does not match".into());
        }
        Ok(bytes)
    }
    fn remove_font_files(&self, retained: &BTreeSet<String>) -> Result<(), String> {
        for entry in fs::read_dir(self.font_directory()?).map_err(io_error)? {
            let entry = entry.map_err(io_error)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.strip_suffix(".font").is_some_and(valid_font_id) && !retained.contains(&name) {
                fs::remove_file(entry.path()).map_err(io_error)?;
            }
        }
        Ok(())
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
        let path = self.source_directory()?.join(&snapshot.file);
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            format!(
                "Recovery source {} is missing or unreadable: {error}",
                snapshot.file
            )
        })?;
        if !metadata.is_file()
            || private_length(&path, Purpose::Source, MAX_SOURCE_BYTES)? != snapshot.length
        {
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
        let bytes = read_private(&path, Purpose::Source, MAX_SOURCE_BYTES)
            .map_err(|error| format!("Recovery source {} is corrupt: {error}", snapshot.file))?;
        if bytes.len() as u64 != snapshot.length || checksum(&bytes) != snapshot.checksum {
            return Err(format!("Recovery source {} is corrupt", snapshot.file));
        }
        Ok(bytes)
    }

    fn restore(
        &self,
        engine: &PdfEngine,
        mut manifest: Manifest,
        adopt_fonts: bool,
    ) -> Result<(Value, HashMap<String, SourceSnapshot>, Arc<FontImportScope>), String> {
        // Validate every image before acquiring any registry resources or opening
        // sources. Each restored source subsequently owns its own native tokens.
        let images=self.read_images(&manifest)?;
        let image_refs=image_references(&manifest.workspace)?;
        let mut opened = HashMap::new();
        let font_imports = FontImportScope::new(engine.fonts());
        let result = (|| {
            for (id, snapshot) in &manifest.fonts {
                let bytes = self.read_font(id, snapshot)?;
                let info = font_imports
                    .register(bytes)
                    .map_err(|error| format!("Recovery font could not be loaded: {error}"))?;
                if info.id != *id {
                    return Err("Recovery font identity mismatch".into());
                }
            }
            for (old_id, snapshot) in &manifest.sources {
                // Full source reads and checksums happen only when restoring;
                // routine autosaves reuse immutable snapshots using metadata.
                let bytes = self.read_source(snapshot)?;
                let source = engine
                    .open_recovery_document(
                        bytes,
                        self.root.join("sources").join(&snapshot.file),
                        &snapshot.original_path,
                        manifest.annotation_import_version < 2,
                        snapshot.legacy_comment_import.unwrap_or(true),
                        snapshot.policy.clone(),
                        snapshot.annotation_sources.clone(),
                        image_refs.get(old_id).into_iter().flatten().map(|id|images[id].clone()).collect(),
                        font_imports.clone(),
                    )
                    .map_err(|error| format!("Recovery source could not be opened: {error}"))?;
                opened.insert(old_id.clone(), source);
            }
            #[cfg(test)]
            RESTORE_OBSERVER.with(|slot| {
                let observer = slot.borrow_mut().take();
                if let Some(observer) = observer { observer(); }
            });
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
                    if manifest.version < 3 {
                        // Older drafts store raw PDF lengths. Convert every
                        // overlay together with the page, using authenticated
                        // source units. Zoom, scroll pixels and dirty state stay
                        // unchanged; the client rebuilds the saved digest.
                        let raw:PagePlan=serde_json::from_value(page.clone()).map_err(|e|e.to_string())?;
                        let physical=engine.recovery_page_in_points(&source.id,raw).map_err(|e|e.to_string())?;
                        page["width"]=json!(physical.width);
                        page["height"]=json!(physical.height);
                        page["overlays"]=serde_json::to_value(physical.overlays).map_err(|e|e.to_string())?;
                    }
                    if (page["width"].as_f64().unwrap_or(0.0) - size.width as f64).abs() > 0.25
                        || (page["height"].as_f64().unwrap_or(0.0) - size.height as f64).abs()
                            > 0.25
                    {
                        return Err("Recovery page dimensions do not match its source".into());
                    }
                    page["sourceId"] = json!(source.id);
                    if !page["crop"].is_null() {
                        if manifest.version<4 {return Err("Legacy recovery cannot contain a page crop.".into());}
                        let plan:PagePlan=serde_json::from_value(page.clone()).map_err(|e|e.to_string())?;
                        engine.validate_page_crop(&plan).map_err(|e|e.to_string())?;
                    }
                    // Display metadata is rebuilt from native authentication;
                    // workspace JSON never grants permission.
                    page["protection"] = json!(source.protection);
                    if manifest.annotation_import_version < 2 {
                        // Import newly supported native annotations exactly once:
                        // v0.4 has all kinds; v0.5 has only text and ink remaining.
                        let overlays = page["overlays"]
                            .as_array_mut()
                            .ok_or("Invalid recovery overlays")?;
                        for overlay in &size.overlays {
                            if manifest.annotation_import_version == 1
                                && !matches!(overlay, Overlay::Text(_) | Overlay::Ink(_))
                            {
                                continue;
                            }
                            let mut value =
                                serde_json::to_value(overlay).map_err(|error| error.to_string())?;
                            if overlays
                                .iter()
                                .any(|existing| existing["id"] == value["id"])
                            {
                                value["id"] = json!(Uuid::new_v4().to_string());
                            }
                            overlays.push(value);
                        }
                    }
                }
            }
            validate_workspace(&manifest.workspace)?;
            if adopt_fonts {
                font_imports.adopt(&font_reference_ids(&manifest.workspace)?)
                    .map_err(|error| error.to_string())?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            for source in opened.values() {
                let _ = engine.close_document(&source.id);
            }
            // Release before trying an older generation, even if the worker's
            // completed reply still temporarily holds its Arc to this scope.
            let _ = font_imports.adopt(&BTreeSet::new());
            return Err(error);
        }
        let remapped = opened
            .into_iter()
            // Legacy encrypted inputs may have been decrypted during opening.
            // Do not bind their new context to an old ciphertext snapshot.
            .filter(|(old,_)|manifest.sources[old].policy.is_some())
            .map(|(old, source)| (source.id, manifest.sources[&old].clone()))
            .collect();
        Ok((manifest.workspace, remapped, font_imports))
    }

    fn collect_garbage(&self, latest: &Path) -> Result<(), String> {
        self.ensure_supported_generations()?;
        let mut retained = Vec::new();
        let mut sources = BTreeSet::new();
        let mut fonts = BTreeSet::new();
        let mut images = BTreeSet::new();
        for (_, path) in self.generations()?.into_iter().rev() {
            if retained.len() < 2 {
                if let Ok(manifest) = self.read_manifest(&path) {
                    sources.extend(manifest.sources.into_values().map(|source| source.file));
                    fonts.extend(manifest.fonts.into_values().map(|font| font.file));
                    images.extend(manifest.images.into_values().map(|image| image.file));
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
        self.remove_source_files(&sources)?;
        self.remove_font_files(&fonts)?;
        self.remove_image_files(&images)
    }

    fn ensure_supported_generations(&self)->Result<(),String> {
        for (_,path) in self.generations()? {
            if self.read_manifest(&path).is_err_and(|error|error==NEWER_RECOVERY) {
                return Err(NEWER_RECOVERY.into());
            }
        }
        Ok(())
    }

    fn remove_source_files(&self, retained: &BTreeSet<String>) -> Result<(), String> {
        for entry in fs::read_dir(self.source_directory()?).map_err(io_error)? {
            let entry = entry.map_err(io_error)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if valid_source_name(&name) && !retained.contains(&name) {
                // Remove individual owned files only; never recurse through a path.
                fs::remove_file(entry.path()).map_err(io_error)?;
            }
        }
        Ok(())
    }

    fn source_directory(&self) -> Result<PathBuf, String> {
        let path = self.root.join("sources");
        if fs::canonicalize(&path).map_err(io_error)? != path {
            return Err("Recovery source folder is redirected".into());
        }
        Ok(path)
    }

    // Called under the store mutex and OS lock. Each atomic replacement contains
    // ciphertext only, so an interrupted migration can be resumed file by file.
    fn migrate_legacy(&self) -> Result<Vec<String>, String> {
        let mut candidates: Vec<_> = self
            .generations()?
            .into_iter()
            .map(|(_, path)| (path, Purpose::Workspace, MAX_MANIFEST_BYTES))
            .collect();
        for (directory, purpose, limit) in [
            (self.source_directory()?, Purpose::Source, MAX_SOURCE_BYTES),
            (self.font_directory()?, Purpose::Font, MAX_FONT_BYTES),
        ] {
            for entry in fs::read_dir(directory).map_err(io_error)? {
                let entry = entry.map_err(io_error)?;
                let name = entry.file_name().to_string_lossy().into_owned();
                if (purpose == Purpose::Source && valid_source_name(&name))
                    || (purpose == Purpose::Font
                        && name.strip_suffix(".font").is_some_and(valid_font_id))
                {
                    candidates.push((entry.path(), purpose, limit));
                }
            }
        }
        let mut errors = Vec::new();
        for (path, purpose, limit) in candidates {
            let result = (|| {
                checked_private_file(&path)?;
                let mut file = File::open(&path).map_err(io_error)?;
                let mut prefix = [0; 13];
                let count = file.read(&mut prefix).map_err(io_error)?;
                drop(file); // Windows replacement must not retain a handle to the old file.
                if prefix[..count].starts_with(private_storage::FAMILY) {
                    return Ok(());
                }
                let bytes = read_bounded(&path, limit)?;
                private_write(&path, purpose, &bytes)
            })();
            if let Err(error) = result {
                errors.push(error);
            }
        }
        Ok(errors)
    }

    // The OS lock excludes an active writer. These are unpublished atomic-write
    // files, including plaintext left by pre-encryption releases. Never recurse
    // through matching directories or follow redirected files/directories.
    fn remove_abandoned_temporaries(&self) -> Result<(), String> {
        for directory in [
            &self.root,
            &self.source_directory()?,
            &self.font_directory()?,
            &self.image_directory()?,
        ] {
            for entry in fs::read_dir(directory).map_err(io_error)? {
                let entry = entry.map_err(io_error)?;
                if entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".folio-recovery-")
                    && entry.file_type().map_err(io_error)?.is_file()
                {
                    checked_private_file(&entry.path())?;
                    fs::remove_file(entry.path()).map_err(io_error)?;
                }
            }
        }
        Ok(())
    }
}

fn valid_font_id(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn font_reference_ids(workspace: &Value) -> Result<BTreeSet<String>, String> {
    let mut ids = BTreeSet::new();
    for tab in workspace["tabs"]
        .as_array()
        .ok_or("Invalid recovery tabs")?
    {
        for page in tab["document"]["pages"]
            .as_array()
            .ok_or("Invalid recovery pages")?
        {
            for overlay in page["overlays"]
                .as_array()
                .ok_or("Invalid recovery overlays")?
            {
                if overlay["type"] == "text" && !overlay["fontId"].is_null() {
                    let id = overlay["fontId"]
                        .as_str()
                        .filter(|id| valid_font_id(id))
                        .ok_or("Invalid recovery font reference")?;
                    ids.insert(id.to_owned());
                }
            }
        }
    }
    if ids.len() > 64 {
        return Err("Recovery workspace has too many fonts".into());
    }
    Ok(ids)
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
            page.validate_crop().map_err(|error| error.to_string())?;
            for overlay in &page.overlays {
                let (id, color):(&String,&str) = match overlay {
                    Overlay::Image(image) => {
                        image.validate(page.width,page.height).map_err(|e|e.to_string())?;
                        (&image.id, "#000000")
                    },
                    Overlay::Text(text) => {
                        text.validate_area_settings()
                            .map_err(|error| error.to_string())?;
                        if !matches!(text.rotation, 0 | 90 | 180 | 270) {
                            return Err("Invalid recovery text rotation".into());
                        }
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
                    Overlay::Highlight(highlight) => {
                        crate::engine::validate_highlight(highlight)
                            .map_err(|error| error.to_string())?;
                        (&highlight.id, &highlight.color)
                    }
                    Overlay::Comment(comment) => {
                        crate::engine::validate_comment(comment)
                            .map_err(|error| error.to_string())?;
                        (&comment.id, &comment.color)
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
        if let Some(items) = document.get("bookmarks") {
            let items: Vec<crate::DocumentBookmark> = serde_json::from_value(items.clone())
                .map_err(|e| format!("Invalid recovery bookmarks: {e}"))?;
            crate::bookmarks::validate(&items, &page_ids.iter().cloned().collect())
                .map_err(|e| e.to_string())?;
        }
        if document.get("bookmarkError").is_some() {
            string_field(document, "bookmarkError", 4096)?;
        }
        if let Some(metadata)=document.get("metadata"){
            let metadata:crate::DocumentMetadataEdit=serde_json::from_value(metadata.clone()).map_err(|e|format!("Invalid recovery document properties: {e}"))?;
            metadata.validate().map_err(|e|e.to_string())?;
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

fn checked_private_file(path: &Path) -> Result<(), String> {
    if !fs::symlink_metadata(path).map_err(io_error)?.is_file()
        || fs::canonicalize(path).map_err(io_error)? != path
    {
        return Err("Recovery payload is incomplete or redirected".into());
    }
    Ok(())
}

fn private_length(path: &Path, purpose: Purpose, limit: u64) -> Result<u64, String> {
    checked_private_file(path)?;
    let mut file = File::open(path).map_err(io_error)?;
    let stored = file.metadata().map_err(io_error)?.len();
    let mut header = [0; private_storage::HEADER_LEN];
    file.read_exact(&mut header).map_err(io_error)?;
    private_storage::plaintext_length(purpose, &header, stored, limit as usize).map(|n| n as u64)
}

fn read_private(path: &Path, purpose: Purpose, limit: u64) -> Result<Vec<u8>, String> {
    checked_private_file(path)?;
    let bytes = read_bounded(path, limit + private_storage::MAX_OVERHEAD)?;
    private_storage::unseal(purpose, &bytes, limit as usize)
}

fn private_write(path: &Path, purpose: Purpose, bytes: &[u8]) -> Result<(), String> {
    // No plaintext temporary file exists, even when encryption or publication fails.
    let protected = private_storage::seal(purpose, bytes)?;
    atomic_write(path, &protected)
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
