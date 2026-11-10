//! Native policy follows immutable source replacements; frontend flags grant nothing.
#[cfg(test)]
#[path="bookmark_inheritance_tests.rs"]
mod bookmark_inheritance_tests;
use super::*;
use crate::protection::PdfProtection;
use serde::{Deserialize, Serialize};

impl PdfEngine {
    pub fn check_source_combination(&self, source_ids: &[String]) -> EngineResult<()> {
        if source_ids.is_empty() || source_ids.len() > 1000 {
            return Err(EngineError::InvalidRequest("Choose between 1 and 1,000 PDF sources.".into()));
        }
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::CheckSourceCombination { source_ids: source_ids.to_vec(), reply })?;
        receive.recv().map_err(|_| EngineError::WorkerStopped)?
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Origin {
    id: String,
    page_count: usize,
    bookmarks: Result<Vec<crate::DocumentBookmark>, String>,
}

#[derive(Clone)]
pub(super) struct SourcePolicy {
    pub protection: Arc<PdfProtection>,
    origin: Arc<Origin>,
    pages: Vec<usize>,
    overlays: Vec<Vec<Overlay>>,
    // Whole-document replacements retain their current outline graph. Legacy
    // single-page projections still need the protected origin's outline model.
    current_bookmarks: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum RecoveryPolicy {
    Unprotected,
    Protected {
        context: Vec<u8>,
        origin: Origin,
        pages: Vec<usize>,
        overlays: Vec<Vec<Overlay>>,
        #[serde(default)]
        current_bookmarks: bool,
    },
}

impl SourcePolicy {
    pub fn image_origin(&self)->&str {&self.origin.id}
    pub fn image_restriction(&self)->Option<&str> {(!self.protection.permissions().copy_content).then_some(self.origin.id.as_str())}
    pub fn new(
        protection: PdfProtection,
        pages: &[PageInfo],
        bookmarks: &Result<Vec<crate::DocumentBookmark>, String>,
    ) -> Self {
        Self {
            protection: Arc::new(protection),
            origin: Arc::new(Origin {
                id: Uuid::new_v4().to_string(),
                page_count: pages.len(),
                bookmarks: bookmarks.clone(),
            }),
            pages: (0..pages.len()).collect(),
            overlays: pages.iter().map(|p| p.overlays.clone()).collect(),
            current_bookmarks: true,
        }
    }
    pub fn info(&self) -> ProtectionInfo {
        ProtectionInfo {
            document_id:self.origin.id.clone(),
            owner: self.protection.is_owner(),
            revision: self.protection.revision(),
            permissions: self.protection.permissions(),
        }
    }
    pub fn same_origin(&self, other:&Self)->bool {
        self.origin==other.origin && self.protection.same_encryption(&other.protection)
    }
    pub fn with_owner(&self, owner:Arc<PdfProtection>)->Self {
        Self {protection:owner,..self.clone()}
    }
    pub fn page(&self, index: usize) -> EngineResult<Self> {
        Ok(Self {
            protection: self.protection.clone(),
            origin: self.origin.clone(),
            pages: vec![*self.pages.get(index).ok_or_else(invalid_policy)?],
            overlays: vec![self.overlays.get(index).ok_or_else(invalid_policy)?.clone()],
            current_bookmarks: false,
        })
    }
    pub fn duplicated_page(&self, index: usize) -> EngineResult<Self> {
        if !self.protection.permissions().assemble {
            return Err(EngineError::PermissionDenied("preparing a duplicated page"));
        }
        let mut result=self.clone();
        result.pages.push(*self.pages.get(index).ok_or_else(invalid_policy)?);
        result.overlays.push(self.overlays.get(index).ok_or_else(invalid_policy)?.clone());
        result.validate_pages(result.pages.len())?;
        Ok(result)
    }
    pub fn snapshot(&self, bytes: &[u8]) -> EngineResult<RecoveryPolicy> {
        Ok(RecoveryPolicy::Protected {
            context: self.protection.seal_recovery_context(bytes)?,
            origin: (*self.origin).clone(),
            pages: self.pages.clone(),
            overlays: self.overlays.clone(),
            current_bookmarks: self.current_bookmarks,
        })
    }
    pub fn validate_pages(&self, count: usize) -> EngineResult<()> {
        if count == 0
            || count != self.pages.len()
            || count != self.overlays.len()
            || self.origin.page_count == 0
            || self.origin.page_count > 100_000
            || self.pages.iter().any(|p| *p >= self.origin.page_count)
            || (!self.protection.permissions().assemble
                && self.pages.iter().collect::<HashSet<_>>().len() != count)
            || Uuid::parse_str(&self.origin.id).is_err()
            || self.overlays.iter().any(|o| o.len() > 10_000)
        {
            return Err(invalid_policy());
        }
        if let Ok(bookmarks) = &self.origin.bookmarks {
            crate::bookmarks::validate(
                bookmarks,
                &(0..self.origin.page_count).map(|i| i.to_string()).collect(),
            )?;
        }
        Ok(())
    }
}

impl RecoveryPolicy {
    pub(crate) fn image_origin(&self)->Option<&str>{match self{Self::Unprotected=>None,Self::Protected{origin,..}=>Some(&origin.id)}}
}
impl RecoveryPolicy {
    pub(super) fn restore(&self, bytes: &[u8]) -> EngineResult<Option<SourcePolicy>> {
        match self {
            Self::Unprotected => Ok(None),
            Self::Protected {
                context,
                origin,
                pages,
                overlays,
                current_bookmarks,
            } => {
                let policy = SourcePolicy {
                    protection: Arc::new(PdfProtection::from_recovery_context(context, bytes)?),
                    origin: Arc::new(origin.clone()),
                    pages: pages.clone(),
                    overlays: overlays.clone(),
                    current_bookmarks: *current_bookmarks,
                };
                policy.validate_pages(pages.len())?;
                Ok(Some(policy))
            }
        }
    }
}
fn invalid_policy() -> EngineError {
    EngineError::InvalidRequest("Protected recovery page provenance is invalid.".into())
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum CompositionIntent {
    Save,
    Preview,
    ImageExport,
    TextExport,
    Print,
}
pub(super) enum OpenSecurity {
    Authenticated(Option<PdfProtection>),
    Restored(Option<SourcePolicy>),
}

fn equivalent_overlays(a: &[Overlay], b: &[Overlay]) -> bool {
    let normalize = |items: &[Overlay]| {
        items
            .iter()
            .cloned()
            .map(|mut o| {
                match &mut o {
                    Overlay::Image(x) => x.id.clear(),
                    Overlay::Text(x) => x.id.clear(),
                    Overlay::Ink(x) => x.id.clear(),
                    Overlay::Highlight(x) => x.id.clear(),
                    Overlay::Comment(x) => x.id.clear(),
                };
                o
            })
            .collect::<Vec<_>>()
    };
    normalize(a) == normalize(b)
}
fn equivalent_bookmarks(a: &[crate::DocumentBookmark], b: &[crate::DocumentBookmark]) -> bool {
    let normalize = |items: &[crate::DocumentBookmark]| {
        items
            .iter()
            .cloned()
            .map(|mut b| {
                b.id.clear();
                b
            })
            .collect::<Vec<_>>()
    };
    normalize(a) == normalize(b)
}

impl WorkerRuntime {
    pub(super) fn check_source_combination(&self, source_ids: &[String]) -> EngineResult<()> {
        let mut protection: Option<&PdfProtection> = None;
        for id in source_ids {
            let source = self.document(id)?;
            if source.for_printing {
                return Err(EngineError::PermissionDenied("combining a prepared print document"));
            }
            if let Some(policy) = &source.policy {
                if !policy.protection.permissions().assemble {
                    return Err(EngineError::PermissionDenied("combining pages"));
                }
                if protection.is_some_and(|p| !p.same_encryption(&policy.protection)) {
                    return Err(EngineError::InvalidRequest("These PDFs use different protection. Save separate protected copies.".into()));
                }
                protection = Some(&policy.protection);
            }
        }
        Ok(())
    }
    pub(super) fn inherited_bookmarks(
        &self,
        request: &ExportRequest,
    ) -> EngineResult<Vec<crate::DocumentBookmark>> {
        self.inherited_bookmarks_controlled(request,&mut ||Ok(()))
    }
    pub(super) fn current_complete_bookmarks(&self,request:&ExportRequest,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Option<Vec<crate::DocumentBookmark>>>{
        let mut blocks=Vec::new();let mut indices=HashMap::new();
        // Establish current whole-source provenance before reading any outlines.
        // Selection/reorder does not turn a current source into a legacy origin.
        for page in &request.pages {
            check()?;let id=page.source_id.as_str();
            let source=self.document(id)?;let count=source.document.page_count() as usize;
            if count==0 || page.page_index>=count || source.policy.as_ref().is_some_and(|p|!p.current_bookmarks){return Ok(None);}
            let index=*indices.entry(id).or_insert_with(||{blocks.push((source,count,HashMap::new()));blocks.len()-1});
            blocks[index].2.entry(page.page_index.to_string()).or_insert_with(||page.id.clone());
        }
        let sources=blocks.into_iter().map(|(source,count,map)|{
            let original=source.source_bookmarks()?
                .as_ref().map_err(|error|EngineError::InvalidRequest(error.clone()))?;
            Ok((original.as_slice(),map,count))
        });
        crate::bookmarks::remap_selected(sources,check).map(Some)
    }
    pub(super) fn inherited_bookmarks_controlled(&self,request:&ExportRequest,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Vec<crate::DocumentBookmark>>{
        if let Some(items)=self.current_complete_bookmarks(request,check)?{return Ok(items);}
        // Edited pages are separate immutable sources. Their original outline
        // tree still addresses the whole document, including the edited page.
        let mut mappings = HashMap::<String, HashMap<String, String>>::new();
        let mut sources = Vec::new();
        for page in &request.pages {
            check()?;
            let source = self.document(&page.source_id)?;
            let (key, index) = match &source.policy {
                Some(p) => (
                    p.origin.id.clone(),
                    *p.pages.get(page.page_index).ok_or_else(invalid_policy)?,
                ),
                None => (page.source_id.clone(), page.page_index),
            };
            if !mappings.contains_key(&key) {
                sources.push((key.clone(), source));
            }
            mappings
                .entry(key)
                .or_default()
                .entry(index.to_string())
                .or_insert_with(|| page.id.clone());
        }
        let mut items = Vec::new();
        for (key, source) in sources {
            check()?;
            let original = match &source.policy {
                Some(p) => &p.origin.bookmarks,
                None => source.source_bookmarks()?,
            }
            .as_ref()
            .map_err(|e| EngineError::InvalidRequest(e.clone()))?;
            items.extend(crate::bookmarks::remap(original, &mappings[&key]));
        }
        Ok(items)
    }
    pub(super) fn editing_document(&self, id: &str) -> EngineResult<&OpenDocument> {
        let source = self.document(id)?;
        if source.for_printing
            || source
                .policy
                .as_ref()
                .is_some_and(|p| !p.protection.permissions().edit_content)
        {
            return Err(EngineError::PermissionDenied("editing existing text"));
        }
        Ok(source)
    }
    pub(super) fn copying_document(&self, id: &str) -> EngineResult<&OpenDocument> {
        let source = self.document(id)?;
        if source.for_printing
            || source
                .policy
                .as_ref()
                .is_some_and(|p| !p.protection.permissions().copy_content)
        {
            return Err(EngineError::PermissionDenied("copying text"));
        }
        Ok(source)
    }
    pub(super) fn authorize_composition(
        &self,
        request: &ExportRequest,
        bookmarks: &[crate::DocumentBookmark],
        intent: CompositionIntent,
    ) -> EngineResult<Option<Arc<PdfProtection>>> {
        self.authorize_composition_security(request,bookmarks,intent,false)
    }
    pub(super) fn authorize_composition_security(
        &self, request:&ExportRequest, bookmarks:&[crate::DocumentBookmark],
        intent:CompositionIntent, change_security:bool,
    )->EngineResult<Option<Arc<PdfProtection>>> {
        if change_security {
            if intent!=CompositionIntent::Save{return Err(EngineError::InvalidRequest("Security settings apply only to a saved PDF copy.".into()));}
            for plan in &request.pages {
                if self.document(&plan.source_id)?.policy.as_ref().is_some_and(|p|!p.protection.is_owner()) {
                    return Err(EngineError::PermissionDenied("changing PDF security without owner access"));
                }
            }
        }
        let mut output: Option<Arc<PdfProtection>> = None;
        let mut checked_origins = HashSet::new();
        for plan in &request.pages {
            let source = self.document(&plan.source_id)?;
            if source.for_printing {
                return Err(EngineError::PermissionDenied(
                    "saving a prepared print document",
                ));
            }
            let Some(policy) = &source.policy else {
                continue;
            };
            let rights = policy.protection.permissions();
            if plan.crop.is_some() && !rights.edit_content {
                return Err(EngineError::PermissionDenied("cropping pages"));
            }
            if intent == CompositionIntent::ImageExport && !rights.copy_content {
                return Err(EngineError::PermissionDenied("exporting pages as images"));
            }
            if intent == CompositionIntent::TextExport && !rights.copy_content {
                return Err(EngineError::PermissionDenied("exporting text"));
            }
            if intent == CompositionIntent::Print && !rights.print {
                return Err(EngineError::PermissionDenied("printing"));
            }
            let baseline = policy
                .overlays
                .get(plan.page_index)
                .ok_or_else(invalid_policy)?;
            if !rights.annotate && !equivalent_overlays(&plan.overlays, baseline) {
                return Err(EngineError::PermissionDenied("changing annotations"));
            }
            if request.flatten
                && intent != CompositionIntent::TextExport
                && !rights.edit_content
                && plan
                    .overlays
                    .iter()
                    .any(|o| matches!(o, Overlay::Text(_) | Overlay::Ink(_) | Overlay::Image(_)))
            {
                return Err(EngineError::PermissionDenied(
                    "flattening annotations into page content",
                ));
            }
            if intent != CompositionIntent::Save {
                continue;
            }
            if !change_security && output
                .as_ref()
                .is_some_and(|p| !p.same_encryption(&policy.protection))
            {
                return Err(EngineError::InvalidRequest(
                    "These PDFs use different protection. Save separate protected copies.".into(),
                ));
            }
            output = Some(policy.protection.clone());
            if !rights.assemble && checked_origins.insert(&policy.origin.id) {
                if request.pages.len() != policy.origin.page_count {
                    return Err(EngineError::PermissionDenied("adding or extracting pages"));
                }
                for (index, page) in request.pages.iter().enumerate() {
                    let other = self
                        .document(&page.source_id)?
                        .policy
                        .as_ref()
                        .ok_or(EngineError::PermissionDenied("combining pages"))?;
                    if other.origin != policy.origin
                        || other.pages.get(page.page_index) != Some(&index)
                        || page.rotation != 0
                    {
                        return Err(EngineError::PermissionDenied(
                            "reordering, rotating, or combining pages",
                        ));
                    }
                }
                let original = policy.origin.bookmarks.as_ref().map_err(|_| {
                    EngineError::PermissionDenied("discarding unreadable bookmarks")
                })?;
                let mapping = request
                    .pages
                    .iter()
                    .enumerate()
                    .map(|(i, p)| (i.to_string(), p.id.clone()))
                    .collect();
                if !equivalent_bookmarks(bookmarks, &crate::bookmarks::remap(original, &mapping)) {
                    return Err(EngineError::PermissionDenied("changing bookmarks"));
                }
            }
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicated_provenance_requires_native_assembly_rights() {
        let bytes=std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/protection/edit_only_aes_256.pdf")).unwrap();
        for password in ["reader","owner"] {
            let unlocked=crate::protection::unlock_pdf(&bytes,Some(password)).unwrap();
            let mut policy=SourcePolicy::new(unlocked.protection.unwrap(),&vec![PageInfo{width:200.,height:300.,overlays:vec![]};2],&Ok(vec![]));
            assert_eq!(policy.duplicated_page(0).is_ok(),password=="owner");
            policy.pages.push(0);policy.overlays.push(vec![]);
            assert_eq!(policy.validate_pages(3).is_ok(),password=="owner");
        }
    }
    #[test]
    fn print_enforces_permissions_and_dpi_and_cannot_be_reused_for_save_or_recovery() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let engine = PdfEngine::start(root.join("resources/pdfium/pdfium.dll")).unwrap();
        let temp = tempfile::tempdir().unwrap();
        let plan = |info: &DocumentInfo| ExportRequest {
            flatten: false,
            pages: vec![PagePlan {
                crop: None,
                id: "print-page".into(),
                source_id: info.id.clone(),
                page_index: 0,
                width: info.pages[0].width,
                height: info.pages[0].height,
                rotation: 0,
                overlays: info.pages[0].overlays.clone(),
            }],
        };
        let blocked = engine
            .open_document_with_password(
                root.join("../tests/fixtures/protection/edit_only_aes_256.pdf"),
                "reader",
            )
            .unwrap();
        assert!(matches!(
            engine.prepare_print(plan(&blocked)),
            Err(EngineError::PermissionDenied(_))
        ));
        let limited = engine
            .open_document_with_password(
                root.join("../tests/fixtures/protection/aes_256.pdf"),
                "reader",
            )
            .unwrap();
        let printed = engine.prepare_print(plan(&limited)).unwrap();
        assert!(matches!(engine.search_page(&printed.id, 0, "Folio"), Err(EngineError::PermissionDenied(_))));
        assert!(matches!(engine.check_source_combination(&[printed.id.clone()]), Err(EngineError::PermissionDenied(_))));
        assert!(matches!(engine.check_source_combination(&[blocked.id.clone()]), Err(EngineError::PermissionDenied(_))));
        assert!(!engine.source_original_path(&printed.id).unwrap().exists());
        assert_eq!(
            image::load_from_memory(&engine.render_page(&printed.id, 0, 2400).unwrap())
                .unwrap()
                .width(),
            750
        );
        assert!(matches!(
            engine.export_pdf(plan(&printed), temp.path().join("bypass.pdf")),
            Err(EngineError::PermissionDenied(_))
        ));
        assert!(matches!(
            engine.extract_text(&printed.id, 0),
            Err(EngineError::PermissionDenied(_))
        ));
        assert!(matches!(
            engine.source_recovery_policy(&printed.id),
            Err(EngineError::PermissionDenied(_))
        ));
        assert!(!temp.path().join("bypass.pdf").exists());
        let image_request=crate::PageImageExportRequest{pages:plan(&printed).pages,page_numbers:vec![1],workspace_page_count:1,options:crate::PageImageOptions{format:crate::PageImageFormat::Png,dpi:150,jpeg_quality:90}};
        assert!(matches!(engine.export_page_images(image_request,temp.path(),Arc::new(crate::PageImageControl::new()),Arc::new(|_|{})),Err(EngineError::PermissionDenied(_))));
        let owner = engine
            .open_document_with_password(
                root.join("../tests/fixtures/protection/aes_256.pdf"),
                "owner",
            )
            .unwrap();
        let high = engine.prepare_print(plan(&owner)).unwrap();
        assert_eq!(
            image::load_from_memory(&engine.render_page(&high.id, 0, 2400).unwrap())
                .unwrap()
                .width(),
            2400
        );
        // The low-quality cap is DPI per physical inch, including UserUnit.
        let original=std::fs::read(root.join("../tests/fixtures/protection/aes_256.pdf")).unwrap();
        for unit in [0.5_f32,2.] {
            let unlocked=crate::protection::unlock_pdf(&original,Some("reader")).unwrap();
            let mut pdf=lopdf::Document::load_mem(&unlocked.bytes).unwrap();
            for id in pdf.get_pages().values(){pdf.get_dictionary_mut(*id).unwrap().set("UserUnit",unit);}
            let mut bytes=Vec::new();pdf.save_to(&mut bytes).unwrap();
            let path=temp.path().join(format!("print-units-{unit}.pdf"));
            std::fs::write(&path,unlocked.protection.unwrap().protect(&bytes).unwrap()).unwrap();
            let source=engine.open_document_with_password(&path,"reader").unwrap();
            let print=engine.prepare_print(plan(&source)).unwrap();
            assert_eq!(print.pages[0].width,source.pages[0].width);
            let raster=image::load_from_memory(&engine.render_page(&print.id,0,2400).unwrap()).unwrap();
            assert_eq!(raster.width(),(750.*unit)as u32);
            assert!(matches!(engine.export_pdf(plan(&print),temp.path().join("scaled-bypass.pdf")),Err(EngineError::PermissionDenied(_))));
            engine.close_document(&print.id).unwrap();engine.close_document(&source.id).unwrap();
        }
        for id in [&blocked.id, &limited.id, &printed.id, &owner.id, &high.id] {
            engine.close_document(id).unwrap();
        }
    }
}
