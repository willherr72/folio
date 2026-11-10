//! Session-owned text-only removal previews, exposed through the trusted host bridge.
use super::redaction_selection::TextMarkRect;
use super::*;
use crate::redaction_content::{invalid, require};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
#[path="redaction_composition.rs"]
mod composition;
pub(crate) use composition::TextRedactionSource;
#[path = "redaction_content_preview.rs"]
mod content;
pub(crate) use content::{ContentRedactionPage, ContentRedactionPreview};
#[path = "redaction_content_raster.rs"]
mod content_raster;
pub(crate) use content_raster::ContentRedactionRaster;
const TTL: Duration = Duration::from_secs(15 * 60);
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
pub(crate) struct TextRedactionMark {
    pub page_index: usize,
    pub rect: TextMarkRect,
}
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct TextRedactionPage {
    pub page_index: usize,
    pub width: f64,
    pub height: f64,
    pub selected_glyph_count: usize,
    pub extent: TextMarkRect,
}
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct TextRedactionPreview {
    pub preview_id: String,
    pub source_id: String,
    pub candidate: DocumentInfo,
    pub removed_characters: usize,
    pub affected_blocks: usize,
    pub omitted_metadata: Vec<&'static str>,
    pub expires_in_seconds: u64,
    pub pages: Vec<TextRedactionPage>,
}
#[derive(Debug, serde::Serialize)]
#[serde(rename_all="camelCase")]
pub(crate) struct TextRedactionBounds {
    pub page_index: usize,
    pub offset: usize,
    pub total: usize,
    pub boxes: Vec<TextMarkRect>,
}
struct Pending {
    owner: String,
    source: String,
    source_hash: [u8; 32],
    candidate: DocumentInfo,
    candidate_hash: [u8; 32],
    allocation: crate::source_storage::SourceIdentity,
    created: Instant,
    pages: Vec<(TextRedactionPage, Vec<TextMarkRect>)>,
    content: Option<content::ContentPending>,
}
pub(super) struct PreviewStore {
    owners: HashSet<String>,
    pending: HashMap<String, Pending>,
    prepared: HashMap<String, composition::PreparedSource>,
    byte_limit: usize,
    geometry_limit: usize,
}
impl Default for PreviewStore {
    fn default() -> Self {
        Self {
            owners: HashSet::new(),
            pending: HashMap::new(),
            prepared: HashMap::new(),
            byte_limit: 128 * 1024 * 1024,
            geometry_limit: 32 * 1024 * 1024,
        }
    }
}
pub(super) enum Request {
    PreviewContent {
        owner: String, source: String, marks: Vec<TextRedactionMark>,
        reply: mpsc::Sender<EngineResult<ContentRedactionPreview>>,
    },
    RenderContent {
        owner: String, source: String, preview: Option<String>, page: usize, width: u32,
        reply: mpsc::Sender<EngineResult<ContentRedactionRaster>>,
    },
    Prepare {
        owner:String,
        request:ExportRequest,
        bookmarks:Option<Vec<crate::DocumentBookmark>>,
        resources:super::page_image_render::Resources,
        reply:mpsc::Sender<EngineResult<TextRedactionSource>>,
    },
    Begin {
        reply: mpsc::Sender<EngineResult<String>>,
    },
    Preview {
        owner: String,
        source: String,
        marks: Vec<TextRedactionMark>,
        reply: mpsc::Sender<EngineResult<TextRedactionPreview>>,
    },
    Bounds {
        owner: String,
        preview: String,
        page: usize,
        offset: usize,
        limit: usize,
        reply: mpsc::Sender<EngineResult<TextRedactionBounds>>,
    },
    Apply {
        owner: String,
        preview: String,
        reply: mpsc::Sender<EngineResult<DocumentInfo>>,
    },
    Discard {
        owner: String,
        preview: String,
        reply: mpsc::Sender<EngineResult<()>>,
    },
    Release {
        owner: String,
        reply: mpsc::Sender<EngineResult<()>>,
    },
}
fn extent(boxes: &[TextMarkRect]) -> EngineResult<TextMarkRect> {
    require(!boxes.is_empty())?;
    let mut x = f64::INFINITY;
    let mut y = f64::INFINITY;
    let mut right = f64::NEG_INFINITY;
    let mut bottom = f64::NEG_INFINITY;
    for b in boxes {
        require(b.valid())?;
        x = x.min(b.x);
        y = y.min(b.y);
        right = right.max(b.x + b.width);
        bottom = bottom.max(b.y + b.height);
    }
    Ok(TextMarkRect {
        x,
        y,
        width: right - x,
        height: bottom - y,
    })
}
fn checked_id(id: &str) -> EngineResult<()> {
    require(id.len() == 36 && Uuid::parse_str(id).is_ok())
}
fn checked_marks(marks: &[TextRedactionMark]) -> EngineResult<()> {
    require(
        (1..=64).contains(&marks.len())
            && marks.iter().all(|m| m.page_index < 1000 && m.rect.valid()),
    )
}
impl PdfEngine {
    pub(crate) fn preview_content_redaction(&self, owner: &str, source: &str, marks: &[TextRedactionMark]) -> EngineResult<ContentRedactionPreview> {
        checked_id(owner)?;checked_id(source)?;checked_marks(marks)?;
        self.text_redaction_request(|reply| Request::PreviewContent {owner:owner.into(),source:source.into(),marks:marks.to_vec(),reply})
    }
    pub(crate) fn render_content_redaction(&self,owner:&str,source:&str,preview:Option<&str>,page:usize,width:u32) -> EngineResult<ContentRedactionRaster> {
        checked_id(owner)?;checked_id(source)?;if let Some(preview)=preview {checked_id(preview)?;}
        require(page<1000&&(64..=1200).contains(&width))?;
        self.text_redaction_request(|reply| Request::RenderContent {owner:owner.into(),source:source.into(),preview:preview.map(str::to_owned),page,width,reply})
    }
    fn text_redaction_request<T>(
        &self,
        make: impl FnOnce(mpsc::Sender<EngineResult<T>>) -> Request,
    ) -> EngineResult<T> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::TextRedaction(make(reply)))?;
        receive.recv().map_err(|_| EngineError::WorkerStopped)?
    }
    pub(crate) fn begin_text_redaction_session(&self) -> EngineResult<String> {
        self.text_redaction_request(|reply| Request::Begin { reply })
    }
    pub(crate) fn preview_text_redaction(
        &self,
        owner: &str,
        source: &str,
        marks: &[TextRedactionMark],
    ) -> EngineResult<TextRedactionPreview> {
        checked_id(owner)?;
        checked_id(source)?;
        checked_marks(marks)?;
        self.text_redaction_request(|reply| Request::Preview {
            owner: owner.into(),
            source: source.into(),
            marks: marks.to_vec(),
            reply,
        })
    }
    pub(crate) fn text_redaction_preview_bounds(
        &self,
        owner: &str,
        preview: &str,
        page: usize,
        offset: usize,
        limit: usize,
    ) -> EngineResult<TextRedactionBounds> {
        checked_id(owner)?;
        checked_id(preview)?;
        require((1..=1000).contains(&limit))?;
        self.text_redaction_request(|reply| Request::Bounds {
            owner: owner.into(),
            preview: preview.into(),
            page,
            offset,
            limit,
            reply,
        })
    }
    pub(crate) fn apply_text_redaction_preview(
        &self,
        owner: &str,
        preview: &str,
    ) -> EngineResult<DocumentInfo> {
        checked_id(owner)?;
        checked_id(preview)?;
        self.text_redaction_request(|reply| Request::Apply {
            owner: owner.into(),
            preview: preview.into(),
            reply,
        })
    }
    pub(crate) fn discard_text_redaction_preview(
        &self,
        owner: &str,
        preview: &str,
    ) -> EngineResult<()> {
        checked_id(owner)?;
        checked_id(preview)?;
        self.text_redaction_request(|reply| Request::Discard {
            owner: owner.into(),
            preview: preview.into(),
            reply,
        })
    }
    pub(crate) fn release_text_redaction_owner(&self, owner: &str) -> EngineResult<()> {
        checked_id(owner)?;
        self.text_redaction_request(|reply| Request::Release {
            owner: owner.into(),
            reply,
        })
    }
}
impl WorkerRuntime {
    fn clean_redaction_pending(&mut self, p: Pending) {
        if self
            .documents
            .get(&p.candidate.id)
            .is_some_and(|d| d.source_bytes.identity().same_source(&p.allocation))
        {
            self.discard_unpublished_source_unhooked(&p.candidate.id);
        }
    }
    fn prune_redaction_previews(&mut self) {
        self.prune_redaction_prepared();
        let expired = self
            .redaction_previews
            .pending
            .iter()
            .filter(|(_, p)| p.created.elapsed() >= TTL)
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        for id in expired {
            if let Some(p) = self.redaction_previews.pending.remove(&id) {
                self.clean_redaction_pending(p);
            }
        }
    }
    fn begin_redaction_session(&mut self) -> EngineResult<String> {
        self.prune_redaction_previews();
        require(self.redaction_previews.owners.len() < 16)?;
        let id = Uuid::new_v4().to_string();
        self.redaction_previews.owners.insert(id.clone());
        Ok(id)
    }
    fn preview_redaction(
        &mut self,
        owner: &str,
        source: &str,
        marks: &[TextRedactionMark],
    ) -> EngineResult<TextRedactionPreview> {
        checked_id(owner)?;
        checked_id(source)?;
        checked_marks(marks)?;
        self.prune_redaction_previews();
        require(self.redaction_previews.owners.contains(owner))?;
        require(
            self.redaction_previews.pending.len() < 4
                && !self
                    .redaction_previews
                    .pending
                    .values()
                    .any(|p| p.candidate.id == source),
        )?;
        self.validate_prepared_redaction_source(owner,source)?;
        let original = self.redaction_source(source)?;
        let source_hash = Sha256::digest(original.bytes()?.as_ref()).into();
        let original_path = original.original_path.clone();
        let mut groups = std::collections::BTreeMap::<usize, Vec<TextMarkRect>>::new();
        for mark in marks {
            groups.entry(mark.page_index).or_default().push(mark.rect);
        }
        let used_geometry = self.retained_redaction_geometry()?;
        let mut geometry = used_geometry;
        let mut ranges = 0usize;
        let mut plans = vec![];
        for (page, rects) in groups {
            let plan = self
                .inspect_redaction_text_page(source, page)?
                .mark_many(&rects)?;
            geometry = geometry
                .checked_add(plan.affected_glyphs().len() * 32)
                .ok_or_else(invalid)?;
            ranges = ranges.checked_add(plan.range_count()).ok_or_else(invalid)?;
            require(geometry <= self.redaction_previews.geometry_limit && ranges <= 4096)?;
            plans.push(plan);
        }
        let output = self.redact_text_marks(&plans)?;
        let used_bytes = self.retained_redaction_bytes();
        require(
            used_bytes
                .checked_add(output.bytes.len())
                .is_some_and(|n| n <= self.redaction_previews.byte_limit),
        )?;
        let mut pages = vec![];
        for plan in plans {
            let (page_index, width, height) = plan.page_geometry();
            let extent = extent(plan.affected_glyphs())?;
            let summary = TextRedactionPage {
                page_index,
                width,
                height,
                extent,
                selected_glyph_count: plan.affected_glyphs().len(),
            };
            pages.push((summary, plan.into_affected_glyphs()));
        }
        let candidate_hash = Sha256::digest(&output.bytes).into();
        // All fallible selection and capacity work is complete before registration.
        let candidate =
            self.register_generated_document(output.bytes, "Text removal preview.pdf")?;
        let document = self
            .documents
            .get_mut(&candidate.id)
            .expect("new candidate registration");
        document.original_path = original_path;
        let allocation = document.source_bytes.identity().clone();
        let preview_id = Uuid::new_v4().to_string();
        let mut omitted_metadata=output.omitted_metadata;
        if let Some(prepared)=self.redaction_previews.prepared.get(source) {
            for item in &prepared.omitted_metadata {if !omitted_metadata.contains(item){omitted_metadata.push(item);}}
        }
        let info = TextRedactionPreview {
            preview_id: preview_id.clone(),
            source_id: source.into(),
            candidate: candidate.clone(),
            removed_characters: output.removed_characters,
            affected_blocks: output.affected_blocks,
            omitted_metadata,
            expires_in_seconds: TTL.as_secs(),
            pages: pages.iter().map(|(p, _)| p.clone()).collect(),
        };
        self.redaction_previews.pending.insert(
            preview_id,
            Pending {
                owner: owner.into(),
                source: source.into(),
                source_hash,
                candidate,
                candidate_hash,
                allocation,
                created: Instant::now(),
                pages,
                content: None,
            },
        );
        Ok(info)
    }
    fn validate_redaction_preview(&mut self, owner: &str, preview: &str) -> EngineResult<()> {
        checked_id(owner)?;
        checked_id(preview)?;
        self.prune_redaction_previews();
        require(self.redaction_previews.owners.contains(owner))?;
        let p = self
            .redaction_previews
            .pending
            .get(preview)
            .ok_or_else(invalid)?;
        require(p.owner == owner)?;
        let valid = (|| {
            self.validate_prepared_redaction_source(owner,&p.source)?;
            require(p.created.elapsed() < TTL)?;
            let source = self.redaction_source(&p.source)?;
            let hash = *source.source_bytes.identity().sha256();
            require(hash == p.source_hash)?;
            let candidate = self.redaction_source(&p.candidate.id)?;
            let hash = *candidate.source_bytes.identity().sha256();
            require(candidate.source_bytes.identity().same_source(&p.allocation) && hash == p.candidate_hash)
        })();
        if valid.is_err() {
            if let Some(p) = self.redaction_previews.pending.remove(preview) {
                self.clean_redaction_pending(p);
            }
        }
        valid
    }
    fn redaction_bounds(
        &mut self,
        owner: &str,
        preview: &str,
        page: usize,
        offset: usize,
        limit: usize,
    ) -> EngineResult<TextRedactionBounds> {
        require((1..=1000).contains(&limit))?;
        self.validate_redaction_preview(owner, preview)?;
        let p = &self.redaction_previews.pending[preview];
        let (_, boxes) = p
            .pages
            .iter()
            .find(|(p, _)| p.page_index == page)
            .ok_or_else(invalid)?;
        require(offset <= boxes.len())?;
        let end = offset.saturating_add(limit).min(boxes.len());
        Ok(TextRedactionBounds {
            page_index: page,
            offset,
            total: boxes.len(),
            boxes: boxes[offset..end].to_vec(),
        })
    }
    fn apply_redaction(&mut self, owner: &str, preview: &str) -> EngineResult<DocumentInfo> {
        self.validate_redaction_preview(owner, preview)?;
        require(self.redaction_previews.pending[preview].content.as_ref()
            .is_none_or(|content| content.reviewed_pages.iter().all(|value| *value)))?;
        Ok(self
            .redaction_previews
            .pending
            .remove(preview)
            .ok_or_else(invalid)?
            .candidate)
    }
    fn discard_redaction(&mut self, owner: &str, preview: &str) -> EngineResult<()> {
        checked_id(owner)?;
        checked_id(preview)?;
        self.prune_redaction_previews();
        require(self.redaction_previews.owners.contains(owner))?;
        if let Some(p) = self.redaction_previews.pending.get(preview) {
            require(p.owner == owner)?;
        }
        if let Some(p) = self.redaction_previews.pending.remove(preview) {
            self.clean_redaction_pending(p);
        }
        Ok(())
    }
    fn release_redaction_owner(&mut self, owner: &str) {
        self.prune_redaction_previews();
        self.redaction_previews.owners.remove(owner);
        let ids = self
            .redaction_previews
            .pending
            .iter()
            .filter(|(_, p)| p.owner == owner)
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        for id in ids {
            if let Some(p) = self.redaction_previews.pending.remove(&id) {
                self.clean_redaction_pending(p);
            }
        }
        self.release_redaction_prepared(owner);
    }
    pub(super) fn detach_redaction_document(&mut self, id: &str) {
        self.detach_redaction_prepared_input(id);
        let ids = self
            .redaction_previews
            .pending
            .iter()
            .filter(|(_, p)| p.source == id || p.candidate.id == id)
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        // Detach all leases first, then roll back only other owned candidates.
        // The caller removes `id`, preserving close_document's return semantics.
        let detached = ids
            .into_iter()
            .filter_map(|id| self.redaction_previews.pending.remove(&id))
            .collect::<Vec<_>>();
        for p in detached {
            if p.candidate.id != id {
                self.clean_redaction_pending(p);
            }
        }
    }
    pub(super) fn handle_text_redaction(&mut self, request: Request) {
        match request {
            Request::PreviewContent {owner,source,marks,reply}=>{
                let result=self.preview_content_redaction(&owner,&source,&marks);
                if let Err(mpsc::SendError(Ok(p)))=reply.send(result) {let _=self.discard_redaction(&owner,&p.preview_id);}
            },
            Request::RenderContent {owner,source,preview,page,width,reply}=>{
                let prior=preview.as_ref().and_then(|id|self.redaction_previews.pending.get(id))
                    .and_then(|p|p.content.as_ref()).and_then(|content|content.pages.iter().position(|p|p.page_index==page)
                        .map(|index|(index,content.reviewed_pages[index])));
                let result=self.render_content_redaction(&owner,&source,preview.as_deref(),page,width);
                if let Err(mpsc::SendError(Ok(_)))=reply.send(result) {
                    if let Some((index,reviewed))=prior {
                        if let Some(content)=preview.as_ref().and_then(|id|self.redaction_previews.pending.get_mut(id)).and_then(|p|p.content.as_mut()) {
                            content.reviewed_pages[index]=reviewed;
                        }
                    }
                }
            },
            Request::Prepare{owner,request,bookmarks,resources,reply}=>{
                let previous=std::mem::replace(&mut self.fonts,resources.fonts.clone());
                let result=self.prepare_redaction_source(&owner,request,bookmarks);
                self.fonts=previous;
                if let Err(mpsc::SendError(Ok(prepared)))=reply.send(result) {
                    self.discard_redaction_prepared(&prepared.document.id);
                }
            },
            Request::Begin { reply } => {
                let result = self.begin_redaction_session();
                if let Err(mpsc::SendError(Ok(owner))) = reply.send(result) {
                    self.release_redaction_owner(&owner);
                }
            }
            Request::Preview {
                owner,
                source,
                marks,
                reply,
            } => {
                let result = self.preview_redaction(&owner, &source, &marks);
                if let Err(mpsc::SendError(Ok(p))) = reply.send(result) {
                    let _ = self.discard_redaction(&owner, &p.preview_id);
                }
            }
            Request::Bounds {
                owner,
                preview,
                page,
                offset,
                limit,
                reply,
            } => {
                let _ = reply.send(self.redaction_bounds(&owner, &preview, page, offset, limit));
            }
            Request::Apply {
                owner,
                preview,
                reply,
            } => {
                let result = self.apply_redaction(&owner, &preview);
                self.reply_document(reply, result);
            }
            Request::Discard {
                owner,
                preview,
                reply,
            } => {
                let _ = reply.send(self.discard_redaction(&owner, &preview));
            }
            Request::Release { owner, reply } => {
                self.release_redaction_owner(&owner);
                let _ = reply.send(Ok(()));
            }
        }
    }
}
#[cfg(test)]
#[path = "redaction_preview_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "redaction_content_preview_tests.rs"]
mod content_tests;

#[cfg(test)]
#[path = "redaction_embedded_tests.rs"]
mod redaction_embedded_tests;
