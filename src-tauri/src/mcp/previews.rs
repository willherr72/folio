//! Session-owned capabilities for worker-created preview bundles, never client paths.
use crate::automation::{
    path_result,
    text_preview::{read_manifest, FileProof, MAX_BUNDLE},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};
use tempfile::TempDir;
const MAX_PREVIEWS: usize = 8;
const TTL: Duration = Duration::from_secs(15 * 60);
pub(super) struct Preview {
    _directory: TempDir,
    pub bundle: PathBuf,
    pub manifest_hash: String,
    pub candidate: FileProof,
    bytes: u64,
    created: Instant,
    result: Value,
}
#[derive(Default)]
pub(super) struct PreviewStore {
    items: BTreeMap<String, Preview>,
}
impl PreviewStore {
    #[cfg(test)]
    pub(super) fn expire_for_test(&mut self, id: &str) {
        self.items.get_mut(id).unwrap().created = Instant::now() - TTL - Duration::from_secs(1);
    }
    pub fn has_room(&self) -> bool {
        self.items.len() < MAX_PREVIEWS && self.bytes() < MAX_BUNDLE
    }
    fn bytes(&self) -> u64 {
        self.items.values().map(|p| p.bytes).sum()
    }
    pub fn get(&self, id: &str) -> Option<&Preview> {
        self.items.get(id)
    }
    pub fn prune(&mut self) {
        self.items.retain(|_, p| p.created.elapsed() < TTL);
    }
    pub fn list(&self) -> Value {
        json!(self
            .items
            .values()
            .map(|p| {
                let mut value = p.result.clone();
                value["expiresInSeconds"] =
                    json!(TTL.saturating_sub(p.created.elapsed()).as_secs());
                value
            })
            .collect::<Vec<_>>())
    }
    pub fn discard(&mut self, id: &str) -> Result<Value, &'static str> {
        self.items
            .remove(id)
            .ok_or("This preview is unavailable or expired. Create a fresh preview.")?;
        Ok(json!({"discarded":true}))
    }
    pub fn adopt(&mut self, directory: TempDir, result: Value) -> Result<Value, &'static str> {
        let bad = "The worker did not produce a valid owned preview bundle.";
        if !self.has_room() {
            return Err("Preview storage is full. Discard a preview before creating another.");
        }
        let name = result["bundleName"].as_str().ok_or(bad)?;
        if !name.starts_with("Folio preview-")
            || name.len() > 100
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b' ' || b == b'-')
        {
            return Err(bad);
        }
        let bundle = directory.path().join(name);
        let metadata = fs::symlink_metadata(&bundle).map_err(|_| bad)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(bad);
        }
        let expected = result["manifestSha256"].as_str().ok_or(bad)?.to_owned();
        // Only small manifest/metadata reads occur on the supervisor. Native worker
        // produced the files; Save rehashes their actual bytes in its bounded worker.
        let manifest = read_manifest(&bundle, &expected).map_err(|_| bad)?;
        for (name, size) in [
            ("candidate.pdf", manifest.candidate.bytes),
            ("before.png", manifest.before.bytes),
            ("after.png", manifest.after.bytes),
        ] {
            let meta = fs::symlink_metadata(bundle.join(name)).map_err(|_| bad)?;
            if !meta.is_file() || meta.file_type().is_symlink() || meta.len() != size {
                return Err(bad);
            }
        }
        let request_bytes = fs::metadata(directory.path().join("edit.json"))
            .map_err(|_| bad)?
            .len();
        if request_bytes > manifest.request_limit() {
            return Err(bad);
        }
        let bytes = manifest.bytes()
            + fs::metadata(bundle.join("manifest.json"))
                .map_err(|_| bad)?
                .len()
            + request_bytes;
        if bytes > MAX_BUNDLE.saturating_sub(self.bytes()) {
            return Err("Preview storage exceeds its 512 MiB limit. Discard a preview first.");
        }
        let id = format!("preview-{}", uuid::Uuid::new_v4());
        let mut response = json!({"previewId":id,"page":manifest.page,"sourceFingerprint":manifest.source_fingerprint,
            "oldText":manifest.old_text,"replacement":manifest.replacement,"candidateSha256":manifest.candidate.sha256,"expiresInSeconds":TTL.as_secs()});
        if let Some(paragraph)=&manifest.paragraph {response["paragraphId"]=json!(manifest.selection_id);response["paragraph"]=json!(paragraph);}
        if let Some(font)=&manifest.substitute_font {response["substituteFont"]=json!(font);}
        if let Some(form)=&manifest.form {response.as_object_mut().unwrap().remove("oldText");response.as_object_mut().unwrap().remove("replacement");response["fieldId"]=json!(manifest.selection_id);response["form"]=json!(form);}
        response = path_result(response, "before", &bundle.join("before.png"));
        response = path_result(response, "after", &bundle.join("after.png"));
        self.items.insert(
            id,
            Preview {
                candidate: manifest.candidate,
                _directory: directory,
                bundle,
                manifest_hash: expected,
                bytes,
                created: Instant::now(),
                result: response.clone(),
            },
        );
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dummy(bytes: u64, age: Duration) -> Preview {
        let directory = tempfile::tempdir().unwrap();
        let bundle = directory.path().join("bundle");
        fs::create_dir(&bundle).unwrap();
        fs::write(bundle.join("private.pdf"), b"owned").unwrap();
        Preview {
            candidate: FileProof {
                bytes: 1,
                sha256: "a".repeat(64),
            },
            _directory: directory,
            bundle,
            manifest_hash: "a".repeat(64),
            bytes,
            created: Instant::now() - age,
            result: json!({}),
        }
    }
    #[test]
    fn expiry_and_discard_release_only_owned_preview_storage() {
        let mut store = PreviewStore::default();
        let expired = dummy(10, TTL + Duration::from_secs(1));
        let expired_path = expired.bundle.clone();
        let live = dummy(10, Duration::ZERO);
        let live_path = live.bundle.clone();
        store.items.insert("expired".into(), expired);
        store.items.insert("live".into(), live);
        store.prune();
        assert!(!expired_path.exists());
        assert!(live_path.exists());
        assert!(store.discard("unknown").is_err());
        store.discard("live").unwrap();
        assert!(!live_path.exists());
        assert_eq!(store.bytes(), 0);
    }
    #[test]
    fn count_and_aggregate_capacity_are_independent() {
        let mut store = PreviewStore::default();
        store
            .items
            .insert("large".into(), dummy(MAX_BUNDLE, Duration::ZERO));
        assert!(!store.has_room());
        store.discard("large").unwrap();
        for n in 0..MAX_PREVIEWS {
            store.items.insert(n.to_string(), dummy(1, Duration::ZERO));
        }
        assert!(!store.has_room());
        store.discard("0").unwrap();
        assert!(store.has_room());
    }
    #[test]
    fn adoption_rejects_worker_paths_outside_its_directory() {
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("keep"), b"keep").unwrap();
        let mut store = PreviewStore::default();
        for name in [
            "../other",
            "Folio preview-../other",
            "Folio preview-/other",
            "C:/other",
        ] {
            assert!(store
                .adopt(
                    tempfile::tempdir().unwrap(),
                    json!({"bundleName":name,"manifestSha256":"a".repeat(64)})
                )
                .is_err());
        }
        assert_eq!(fs::read(outside.path().join("keep")).unwrap(), b"keep");
        assert_eq!(store.items.len(), 0);
    }
}
