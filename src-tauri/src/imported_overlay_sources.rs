//! Source-bound annotation lineage. Recovery/transfer integration follows capture.
#![allow(dead_code)]
use super::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[path = "imported_overlay_transfer.rs"]
mod transfer;
#[path = "imported_overlay_unit_semantics.rs"]
mod unit_semantics;

#[path = "imported_overlay_publication.rs"]
pub(super) mod publication;

pub(crate) const MAX_METADATA: usize = 16 * 1024 * 1024;
const MAX_WORK: usize = 1_000_000;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Snapshot {
    version: u8,
    source_sha256: String,
    bindings: Vec<Binding>,
}
impl Snapshot {
    /// Admit the borrowed JSON shape before serde builds owned binding vectors.
    pub(crate) fn wire_reservation(value: &serde_json::Value) -> EngineResult<usize> {
        let invalid = || {
            EngineError::InvalidRequest(
                "Imported annotation ownership metadata exceeds its budget or is invalid.".into(),
            )
        };
        let digest = |v: &serde_json::Value| {
            v.as_str().is_some_and(|s| {
                s.len() == 64
                    && s.bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
        };
        if value["version"].as_u64() != Some(1) || !digest(&value["sourceSha256"]) {
            return Err(invalid());
        }
        let bindings = value["bindings"].as_array().ok_or_else(invalid)?;
        if bindings.len() > MAX_METADATA / 512 {
            return Err(invalid());
        }
        bindings.iter().try_fold(512usize, |sum, b| {
            if !digest(&b["initialSemanticsSha256"]) {
                return Err(invalid());
            }
            let id = b["overlayId"].as_str().ok_or_else(invalid)?;
            let popups = b["popupIndices"].as_array().ok_or_else(invalid)?.len();
            sum.checked_add(reservation(id, popups).ok_or_else(invalid)?)
                .filter(|n| *n <= MAX_METADATA)
                .ok_or_else(invalid)
        })
    }
    pub(crate) fn metadata_reservation(&self) -> EngineResult<usize> {
        let invalid = || {
            EngineError::InvalidRequest(
                "Imported annotation ownership metadata exceeds its budget or is invalid.".into(),
            )
        };
        let digest = |s: &str| {
            s.len() == 64
                && s.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        };
        if self.version != 1 || !digest(&self.source_sha256) {
            return Err(invalid());
        }
        self.bindings.iter().try_fold(512usize, |sum, b| {
            if !digest(&b.initial_semantics_sha256) {
                return Err(invalid());
            }
            sum.checked_add(reservation(&b.overlay_id, b.popup_indices.len()).ok_or_else(invalid)?)
                .and_then(|n|n.checked_add(if b.public_echo_sha256.is_some(){128}else{0}))
                .filter(|n| *n <= MAX_METADATA)
                .ok_or_else(invalid)
        })
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Binding {
    page_index: usize,
    annotation_index: usize,
    annotation_id: Option<lopdf::ObjectId>,
    popup_indices: Vec<usize>,
    overlay_id: String,
    initial_semantics_sha256: String,
    // Fresh source-derived cache only. Never deserialize a claimed no-op proof.
    #[serde(skip)]
    public_echo_sha256: Option<String>,
}
pub(super) struct Rename {
    pub page_index: usize,
    pub from: String,
    pub to: String,
}
pub(super) struct Index {
    source: crate::source_storage::SourceIdentity,
    source_hash: OnceLock<String>,
    bindings: Vec<Binding>,
    trusted: bool,
    used: usize,
    limit: usize,
    slots: BTreeSet<(usize, usize)>,
    labels: BTreeSet<(usize, String)>,
    semantic_bytes: usize,
    work: usize,
}
impl Index {
    pub(super) fn bound_to(&self,source:&crate::source_storage::SourceIdentity)->bool {
        self.trusted && self.source.same_source(source)
    }
    pub(super) fn capturing(source: crate::source_storage::SourceIdentity) -> Self {
        Self {
            source,
            source_hash: OnceLock::new(),
            bindings: vec![],
            trusted: true,
            used: 0,
            limit: MAX_METADATA - 512,
            slots: BTreeSet::new(),
            labels: BTreeSet::new(),
            semantic_bytes: 0,
            work: 0,
        }
    }
    pub(super) fn unbound(source: crate::source_storage::SourceIdentity) -> Self {
        let mut value = Self::capturing(source);
        value.trusted = false;
        value
    }
    pub(super) fn capture(
        &mut self,
        page: usize,
        index: usize,
        id: Option<lopdf::ObjectId>,
        popups: &[usize],
        overlay: &Overlay,
    ) -> bool {
        self.capture_in_units(page,index,id,popups,overlay,1.)
    }
    pub(super) fn capture_in_units(&mut self,page:usize,index:usize,id:Option<lopdf::ObjectId>,popups:&[usize],overlay:&Overlay,unit:f32)->bool {
        let label = label(overlay);
        let Some(charge) = reservation(label, popups.len()).and_then(|n|n.checked_add(if unit==1.{0}else{128})) else {
            return false;
        };
        let Some(used) = self.used.checked_add(charge).filter(|n| *n <= self.limit) else {
            return false;
        };
        let Some(work) = semantic_work(overlay, popups.len(), MAX_WORK - self.work).and_then(|n|n.checked_mul(if unit==1.{1}else{2})).filter(|n|*n<=MAX_WORK-self.work) else {
            return false;
        };
        if id.is_some_and(|id| id.0 == 0)
            || self.slots.contains(&(page, index))
            || self.labels.contains(&(page, label.to_owned()))
            || popups.windows(2).any(|p| p[0] >= p[1])
            || popups
                .iter()
                .any(|p| *p == index || self.slots.contains(&(page, *p)))
        {
            return false;
        }
        let Some((hash, bytes)) = semantics(overlay, MAX_METADATA - self.semantic_bytes) else {
            return false;
        };
        let (public_echo_sha256,extra)=if unit==1. {(None,0)}else{
            let Some((hash,extra))=unit_semantics::semantics(overlay,unit,MAX_METADATA-self.semantic_bytes-bytes)else{return false;};
            (Some(hash),extra)
        };
        self.bindings.push(Binding {
            page_index: page,
            annotation_index: index,
            annotation_id: id,
            popup_indices: popups.to_vec(),
            overlay_id: label.to_owned(),
            initial_semantics_sha256: hash,
            public_echo_sha256,
        });
        self.slots.insert((page, index));
        self.slots.extend(popups.iter().map(|p| (page, *p)));
        self.labels.insert((page, label.to_owned()));
        self.used = used;
        self.semantic_bytes += bytes+extra;
        self.work += work;
        true
    }
    pub(super) fn popup_capacity(&self) -> usize {
        (self.limit - self.used) / 128
    }
    pub(super) fn reserve_scan(&mut self, steps: usize) -> bool {
        let Some(work) = self.work.checked_add(steps).filter(|n| *n <= MAX_WORK) else {
            return false;
        };
        self.work = work;
        true
    }
    pub(super) fn snapshot(&self) -> Option<Snapshot> {
        self.trusted.then(|| Snapshot {
            version: 1,
            source_sha256: self
                .source_hash
                .get_or_init(|| {
                    self.source.sha256()
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect()
                })
                .clone(),
            bindings: self.bindings.clone(),
        })
    }
    pub(super) fn snapshot_with_budget(&self, limit: usize) -> EngineResult<Option<Snapshot>> {
        if self.trusted && self.used.checked_add(512).is_none_or(|n| n > limit) {
            return Err(EngineError::InvalidRequest(
                "Imported annotation ownership metadata exceeds the recovery budget.".into(),
            ));
        }
        Ok(self.snapshot())
    }
    pub(super) fn restore(&self, snapshot: Snapshot) -> EngineResult<(Self, Vec<Rename>)> {
        let invalid = || {
            EngineError::InvalidRequest(
                "Imported annotation ownership does not match the original PDF.".into(),
            )
        };
        if !self.trusted
            || snapshot.version != 1
            || snapshot.bindings.len() != self.bindings.len()
            || snapshot.source_sha256
                != *self
                    .source_hash
                    .get_or_init(|| hex(self.source.sha256()))
        {
            return Err(invalid());
        }
        let current: BTreeMap<_, _> = self
            .bindings
            .iter()
            .map(|b| ((b.page_index, b.annotation_index), b))
            .collect();
        let mut restored = Self::capturing(self.source.clone());
        restored.limit = self.limit;
        restored.semantic_bytes = self.semantic_bytes;
        restored.work = self.work;
        let mut renames = Vec::new();
        for mut binding in snapshot.bindings {
            let key = (binding.page_index, binding.annotation_index);
            let expected = current.get(&key).ok_or_else(invalid)?;
            let charge = reservation(&binding.overlay_id, binding.popup_indices.len())
                .and_then(|n|n.checked_add(if expected.public_echo_sha256.is_some(){128}else{0}))
                .ok_or_else(invalid)?;
            restored.used = restored
                .used
                .checked_add(charge)
                .filter(|n| *n <= restored.limit)
                .ok_or_else(invalid)?;
            if binding.annotation_id != expected.annotation_id
                || binding.popup_indices != expected.popup_indices
                || binding.initial_semantics_sha256 != expected.initial_semantics_sha256
                || !restored.slots.insert(key)
                || !restored
                    .labels
                    .insert((binding.page_index, binding.overlay_id.clone()))
            {
                return Err(invalid());
            }
            for popup in &binding.popup_indices {
                if !restored.slots.insert((binding.page_index, *popup)) {
                    return Err(invalid());
                }
            }
            renames.push(Rename {
                page_index: binding.page_index,
                from: expected.overlay_id.clone(),
                to: binding.overlay_id.clone(),
            });
            // Even an in-memory snapshot cannot override the fresh source echo.
            binding.public_echo_sha256=expected.public_echo_sha256.clone();
            restored.bindings.push(binding);
        }
        restored
            .bindings
            .sort_by_key(|b| (b.page_index, b.annotation_index));
        Ok((restored, renames))
    }
}

impl WorkerRuntime {
    pub(super) fn restore_annotation_sources(
        &mut self,
        mut info: DocumentInfo,
        snapshot: Option<Snapshot>,
    ) -> EngineResult<DocumentInfo> {
        let id = info.id.clone();
        let result = (|| {
            let source = self.documents.get_mut(&id).ok_or_else(|| {
                EngineError::InvalidRequest("Recovered source is no longer open.".into())
            })?;
            if let Some(snapshot) = snapshot {
                snapshot.metadata_reservation()?;
                // Reconstruct the complete current version1 import set. Never
                // filter imports by untrusted saved slots before this comparison.
                let (restored, renames) = source.imported_overlay_sources.restore(snapshot)?;
                let mut renames: BTreeMap<_, _> = renames
                    .into_iter()
                    .map(|r| ((r.page_index, r.from), r.to))
                    .collect();
                for (page, info) in info.pages.iter_mut().enumerate() {
                    for overlay in &mut info.overlays {
                        let id = match overlay {
                            Overlay::Text(v) => &mut v.id,
                            Overlay::Image(v) => &mut v.id,
                            Overlay::Ink(v) => &mut v.id,
                            Overlay::Highlight(v) => &mut v.id,
                            Overlay::Comment(v) => &mut v.id,
                        };
                        *id = renames.remove(&(page, id.clone())).ok_or_else(|| {
                            EngineError::InvalidRequest(
                                "Imported annotation ownership does not match the recovered page."
                                    .into(),
                            )
                        })?;
                    }
                }
                if !renames.is_empty() {
                    return Err(EngineError::InvalidRequest(
                        "Imported annotation ownership has unmatched recovered annotations.".into(),
                    ));
                }
                source.imported_overlay_sources = restored;
            } else {
                // Legacy drafts retain their saved overlays. Fresh random IDs
                // cannot retroactively establish their original ownership.
                source.imported_overlay_sources = Index::unbound(source.source_bytes.identity().clone());
            }
            Ok(info)
        })();
        if result.is_err() {
            self.discard_unpublished_source(&id);
        }
        result
    }
}

fn label(overlay: &Overlay) -> &str {
    match overlay {
        Overlay::Text(v) => &v.id,
        Overlay::Image(v) => &v.id,
        Overlay::Ink(v) => &v.id,
        Overlay::Highlight(v) => &v.id,
        Overlay::Comment(v) => &v.id,
    }
}
fn reservation(label: &str, popups: usize) -> Option<usize> {
    if label.trim().is_empty() || label.len() > 1024 {
        return None;
    }
    // Covers the bounded wire record, retained index entries and duplicate labels.
    512usize
        .checked_add(label.len().checked_mul(8)?)?
        .checked_add(popups.checked_mul(128)?)
}
fn hex(bytes: impl AsRef<[u8]>) -> String {
    bytes.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}
fn semantic_work(overlay: &Overlay, popups: usize, limit: usize) -> Option<usize> {
    let mut work = 64usize.checked_add(popups.checked_mul(2)?)?;
    if work > limit {
        return None;
    }
    match overlay {
        Overlay::Highlight(v) => {
            work = work.checked_add(v.rects.len().checked_mul(8)?)?;
        }
        Overlay::Ink(v) => {
            // Include empty paths; a byte-only cap admits millions of them.
            for path in &v.paths {
                work = work.checked_add(4usize.checked_add(path.len().checked_mul(4)?)?)?;
                if work > limit {
                    return None;
                }
            }
        }
        _ => {}
    }
    (work <= limit).then_some(work)
}
struct SemanticHash {
    hash: Sha256,
    used: usize,
    limit: usize,
}
impl std::io::Write for SemanticHash {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.used = self
            .used
            .checked_add(bytes.len())
            .filter(|n| *n <= self.limit)
            .ok_or_else(|| std::io::Error::other("Annotation semantic budget exceeded"))?;
        self.hash.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn semantics(overlay: &Overlay, limit: usize) -> Option<(String, usize)> {
    let mut sink = SemanticHash {
        hash: Sha256::new(),
        used: 0,
        limit,
    };
    // Borrow every variable-sized field. Workspace IDs intentionally do not
    // participate; no AP, image payload or cloned overlay is retained here.
    match overlay {
        Overlay::Text(v) => serde_json::to_writer(
            &mut sink,
            &(
                "text",
                v.x,
                v.y,
                &v.text,
                v.font_size,
                v.font_name,
                &v.font_id,
                &v.shaping,
                &v.area,
                &v.color,
                v.rotation,
            ),
        ),
        Overlay::Image(v) => serde_json::to_writer(
            &mut sink,
            &(
                "image",
                &v.asset_id,
                v.x,
                v.y,
                v.width,
                v.height,
                v.rotation,
            ),
        ),
        Overlay::Ink(v) => {
            serde_json::to_writer(&mut sink, &("ink", &v.paths, &v.color, v.stroke_width))
        }
        Overlay::Highlight(v) => serde_json::to_writer(
            &mut sink,
            &("highlight", &v.rects, &v.color, &v.text, v.opacity),
        ),
        Overlay::Comment(v) => {
            serde_json::to_writer(&mut sink, &("comment", v.x, v.y, &v.text, &v.color))
        }
    }
    .ok()?;
    Some((hex(sink.hash.finalize()), sink.used))
}

#[cfg(test)]
#[path = "imported_overlay_sources_tests.rs"]
mod tests;
