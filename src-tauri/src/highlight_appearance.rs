//! Conservative authored-appearance admission. The reference is produced by
//! the existing highlight renderer, on an empty page; source artwork is never
//! rendered into or copied into that reference document.
use super::*;
use lopdf::{dictionary, Dictionary, Document, Object};
use sha2::{Digest, Sha256};

#[derive(Clone)]
pub(super) enum Evidence {
    Absent([u8; 32]),
    Authored([u8; 32]),
    Unsupported,
}

pub(super) struct Budget {
    work: usize,
    bytes: usize,
}
impl Default for Budget {
    fn default() -> Self { Self { work: 1_000_000, bytes: 16 * 1024 * 1024 } }
}
impl Budget {
    fn reserve(&mut self, work: usize, bytes: usize) -> Option<()> {
        if work > self.work || bytes > self.bytes { return None; }
        self.work -= work;
        self.bytes -= bytes;
        Some(())
    }
}

// Hash typed values, resolving references without expanding a canonical tree.
// Repeated aliases consume work again; cycles fail at the depth/work bound.
// Exact stream programs are intentional: alternate but equivalent programs
// stay native until a broader equivalence proof exists.
fn value(doc: &Document, object: &Object, hash: &mut Sha256, budget: &mut Budget, depth: usize) -> Option<()> {
    if depth > 64 { return None; }
    budget.reserve(1, 0)?;
    match object {
        Object::Reference(id) => value(doc, doc.objects.get(id)?, hash, budget, depth + 1)?,
        Object::Null => hash.update(b"z"),
        Object::Boolean(v) => hash.update(if *v { b"t" } else { b"f" }),
        Object::Integer(v) => {
            // Refuse integers that cannot be represented exactly as f64.
            if v.unsigned_abs() > (1u64 << 53) { return None; }
            hash.update(b"n"); hash.update((*v as f64).to_bits().to_le_bytes());
        }
        Object::Real(v) => {
            if !v.is_finite() { return None; }
            let number = if *v == 0.0 { 0.0f64 } else { f64::from(*v) };
            hash.update(b"n"); hash.update(number.to_bits().to_le_bytes());
        }
        Object::Name(v) | Object::String(v, _) => {
            budget.reserve(0, v.len())?;
            hash.update(if matches!(object, Object::Name(_)) { b"k" } else { b"s" });
            hash.update((v.len() as u64).to_le_bytes()); hash.update(v);
        }
        Object::Array(v) => {
            if v.len() > budget.work { return None; }
            hash.update(b"a"); hash.update((v.len() as u64).to_le_bytes());
            for item in v { value(doc, item, hash, budget, depth + 1)?; }
        }
        Object::Dictionary(v) => dict(doc, v, hash, budget, depth + 1, false)?,
        Object::Stream(v) => {
            // Length is transport metadata only after it agrees with the actual
            // encoded bytes. Permissive parsing must not authorize ambiguous AP.
            let mut length = v.dict.get(b"Length").ok()?;
            for _ in depth..=64 {
                budget.reserve(1, 0)?;
                if let Object::Reference(id) = length { length = doc.objects.get(id)?; }
                else { break; }
            }
            if usize::try_from(length.as_i64().ok()?).ok()? != v.content.len() { return None; }
            hash.update(b"stream");
            dict(doc, &v.dict, hash, budget, depth + 1, true)?;
            // The cumulative byte allowance also bounds decompression scratch.
            // Only Flate without decode parameters is admitted; unknown filters
            // remain native rather than relying on permissive decoder fallback.
            let content;
            let bytes = if v.dict.has(b"Filter") {
                if v.dict.get(b"Filter").ok()?.as_name().ok()? != b"FlateDecode"
                    || v.dict.has(b"DecodeParms") { return None; }
                budget.reserve(0, v.content.len())?;
                if budget.bytes == 0 { return None; }
                // A failed decoder may already have produced the full allowance.
                // Charge it before the call; refund unused space only on success.
                let allowance = std::mem::take(&mut budget.bytes);
                content = v.decompressed_content_with_limit(allowance).ok()?;
                budget.bytes = allowance.checked_sub(content.len())?;
                content.as_slice()
            } else {
                if v.dict.has(b"DecodeParms") { return None; }
                budget.reserve(0, v.content.len())?;
                v.content.as_slice()
            };
            hash.update((bytes.len() as u64).to_le_bytes()); hash.update(bytes);
        }
    }
    Some(())
}
fn dict(doc: &Document, dictionary: &Dictionary, hash: &mut Sha256, budget: &mut Budget, depth: usize, stream: bool) -> Option<()> {
    if depth > 64 { return None; }
    // Reserve sorting scratch and traversal before allocating borrowed entries.
    budget.reserve(dictionary.len(), dictionary.len().checked_mul(std::mem::size_of::<(&Vec<u8>, &Object)>())?)?;
    let mut entries = dictionary.iter().filter(|(key, _)| !stream || !matches!(key.as_slice(), b"Length" | b"Filter" | b"DecodeParms")).collect::<Vec<_>>();
    entries.sort_unstable_by(|a, b| a.0.cmp(b.0));
    hash.update(b"d"); hash.update((entries.len() as u64).to_le_bytes());
    for (key, item) in entries {
        budget.reserve(0, key.len())?;
        hash.update((key.len() as u64).to_le_bytes()); hash.update(key);
        value(doc, item, hash, budget, depth + 1)?;
    }
    Some(())
}
fn fingerprint(doc: &Document, annotation: &Dictionary, budget: &mut Budget) -> Option<[u8; 32]> {
    let mut hash = Sha256::new();
    value(doc, annotation.get(b"Rect").ok()?, &mut hash, budget, 0)?;
    value(doc, annotation.get(b"AP").ok()?, &mut hash, budget, 0)?;
    Some(hash.finalize().into())
}
fn unpainted_fingerprint(doc: &Document, annotation: &Dictionary, budget: &mut Budget) -> Option<[u8; 32]> {
    let mut hash = Sha256::new();
    // Without AP these public fields are the drawing, not merely advisory
    // metadata. Compare the actual native writer's values, including its color
    // and alpha quantization and coordinate serialization. No pixel tolerance
    // may authorize rounding, a different clip, or a skewed quadrilateral.
    for key in [b"Rect".as_slice(), b"QuadPoints", b"C", b"CA"] {
        hash.update(key);
        match annotation.get(key) {
            Ok(item) => value(doc, item, &mut hash, budget, 0)?,
            Err(_) if key == b"C" => value(doc, &Object::Array(vec![1.into(),1.into(),0.into()]), &mut hash, budget, 0)?,
            Err(_) if key == b"CA" => value(doc, &Object::Integer(1), &mut hash, budget, 0)?,
            Err(_) => return None,
        }
    }
    Some(hash.finalize().into())
}
pub(super) fn inspect(doc: &Document, annotation: &Dictionary, budget: &mut Budget) -> Evidence {
    // Optional content and selected appearance states have no overlay equivalent.
    if annotation.has(b"OC") || annotation.has(b"AS") { return Evidence::Unsupported; }
    if !annotation.has(b"AP") { return unpainted_fingerprint(doc, annotation, budget).map(Evidence::Absent).unwrap_or(Evidence::Unsupported); }
    fingerprint(doc, annotation, budget).map(Evidence::Authored).unwrap_or(Evidence::Unsupported)
}

pub(super) fn admits(pdfium: &'static Pdfium, evidence: &Evidence, geometry: PageGeometry, unit: f32, highlight: &HighlightOverlay, budget: &mut Budget) -> bool {
    match evidence {
        Evidence::Unsupported => false,
        Evidence::Authored(expected) | Evidence::Absent(expected) => {
            let authored = matches!(evidence, Evidence::Authored(_));
            if reference(pdfium, geometry, unit, highlight, budget, authored).as_ref() == Some(expected) { return true; }
            // A workspace rotation changes the display frame without rewriting
            // the original AP's rectangle traversal order. Try the other native
            // frames only after proving identical raw PDF rectangle bounds.
            // This does not normalize or interpret arbitrary source programs.
            for rotation in [0,90,180,270] {
                if rotation == geometry.rotation { continue; }
                let frame=PageGeometry{rotation,..geometry};
                if budget.reserve(highlight.rects.len(),highlight.rects.len().saturating_mul(std::mem::size_of::<AnnotationRect>())+highlight.color.len()).is_none() { return false; }
                let Some(rects)=highlight.rects.iter().map(|rect|same_raw_rectangle(geometry,frame,rect)).collect::<Option<Vec<_>>>() else {continue;};
                let candidate=HighlightOverlay{id:String::new(),rects,color:highlight.color.clone(),text:None,opacity:highlight.opacity};
                if reference(pdfium, frame, unit, &candidate, budget, authored).as_ref() == Some(expected) { return true; }
            }
            false
        },
    }
}
fn same_raw_rectangle(original:PageGeometry, frame:PageGeometry, rect:&AnnotationRect)->Option<AnnotationRect> {
    let bounds=annotation_bounds(original,rect);
    let a=frame.pdf_to_displayed(bounds.left().value,bounds.bottom().value);
    let b=frame.pdf_to_displayed(bounds.right().value,bounds.top().value);
    let mapped=AnnotationRect{x:a.x.min(b.x),y:a.y.min(b.y),width:(a.x-b.x).abs(),height:(a.y-b.y).abs()};
    let other=annotation_bounds(frame,&mapped);
    let edges=|r:PdfRect|[r.left().value,r.bottom().value,r.right().value,r.top().value];
    (edges(bounds)==edges(other)).then_some(mapped)
}
fn reference(pdfium: &'static Pdfium, geometry: PageGeometry, unit: f32, highlight: &HighlightOverlay, budget: &mut Budget, authored: bool) -> Option<[u8; 32]> {
    let doc = render_references(pdfium, geometry, unit, &[highlight], budget)?;
    let page = doc.get_dictionary(*doc.get_pages().get(&1)?).ok()?;
    let annotations = doc.dereference(page.get(b"Annots").ok()?).ok()?.1.as_array().ok()?;
    if annotations.len() != 1 { return None; }
    let annotation = doc.dereference(&annotations[0]).ok()?.1.as_dict().ok()?;
    if authored { fingerprint(&doc, annotation, budget) } else { unpainted_fingerprint(&doc, annotation, budget) }
}
fn render_references(pdfium: &'static Pdfium, geometry: PageGeometry, unit: f32, highlights: &[&HighlightOverlay], budget: &mut Budget) -> Option<Document> {
    let rects = highlights.iter().try_fold(0usize, |count, item| count.checked_add(item.rects.len()))?;
    // Reserve construction/serialization work before native allocation. There is
    // one empty staging page per batch, never source page payload or artwork.
    budget.reserve(1024usize.checked_add(rects.checked_mul(64)?)?.checked_add(highlights.len().checked_mul(32)?)?, 4096usize.checked_add(rects.checked_mul(256)?)?.checked_add(highlights.len().checked_mul(128)?)?)?;
    let mut blank = Document::with_version("1.7");
    let tree = blank.new_object_id();
    let page_id = blank.add_object(dictionary! {
        "Type"=>"Page", "Parent"=>tree,
        "MediaBox"=>vec![Object::Real(geometry.left),Object::Real(geometry.bottom),Object::Real(geometry.right),Object::Real(geometry.top)],
        "Rotate"=>i64::from(geometry.rotation), "UserUnit"=>unit, "Resources"=>dictionary!{}
    });
    blank.objects.insert(tree, dictionary!{"Type"=>"Pages", "Kids"=>vec![Object::Reference(page_id)], "Count"=>1}.into());
    let root = blank.add_object(dictionary!{"Type"=>"Catalog", "Pages"=>tree}); blank.trailer.set("Root", root);
    let native = pdfium.load_pdf_from_reader(Cursor::new(crate::pdf_output::save(&mut blank).ok()?), None).ok()?;
    {
        let mut page = native.pages().get(0).ok()?;
        for highlight in highlights {
            // Notes do not paint. Do not copy arbitrary source note contents.
            let shape = HighlightOverlay { id: String::new(), rects: highlight.rects.clone(), color: highlight.color.clone(), text: None, opacity: highlight.opacity };
            add_highlight_annotation(&mut page, geometry, &shape).ok()?;
        }
        page.render_with_config(&PdfRenderConfig::new().set_target_width(1).set_target_height(1).render_annotations(true)).ok()?;
    }
    let bytes = crate::pdf_output::from_pdfium(&native).ok()?;
    budget.reserve(0, bytes.len())?;
    crate::pdf_output::load(&bytes).ok()
}

#[cfg_attr(test, track_caller)]
fn publication_error() -> EngineError {
    #[cfg(test)] eprintln!("highlight publication refusal at {}", std::panic::Location::caller());
    EngineError::InvalidRequest("The generated highlight appearance could not be preserved within rendering limits.".into())
}

/// Exact slots recorded when creating annotations in a private native document.
/// Never discover publication targets by geometry, text, names or source IDs.
#[derive(Default)]
pub(super) struct Publication {
    slots: Vec<(usize, usize, usize)>,
    budget: Budget,
}
impl Publication {
    pub(super) fn record(&mut self, page: usize, annotation: usize, overlay: usize) -> EngineResult<()> {
        self.budget.reserve(1, std::mem::size_of::<(usize,usize,usize)>()).ok_or_else(||publication_error())?;
        self.slots.push((page, annotation, overlay));
        Ok(())
    }
    pub(super) fn apply(&mut self, pdfium: &'static Pdfium, document: &mut Document, request: &ExportRequest, geometries: &[PageGeometry], check: &mut impl FnMut()->EngineResult<()>) -> EngineResult<bool> {
        if self.slots.is_empty() { return Ok(false); }
        self.budget.reserve(request.pages.len(),request.pages.len().checked_mul(32).ok_or_else(||publication_error())?).ok_or_else(||publication_error())?;
        let page_ids = document.get_pages();
        if page_ids.len()!=request.pages.len() { return Err(publication_error()); }
        let mut start = 0;
        while start < self.slots.len() {
            check()?;
            let index = self.slots[start].0;
            let mut end = start + 1;
            while end < self.slots.len() && self.slots[end].0 == index { end += 1; }
            self.budget.reserve(end-start, (end-start).checked_mul(std::mem::size_of::<&HighlightOverlay>()).ok_or_else(||publication_error())?).ok_or_else(||publication_error())?;
            let mut highlights = Vec::with_capacity(end-start);
            for &(_, _, overlay) in &self.slots[start..end] {
                check()?;
                let Some(Overlay::Highlight(highlight)) = request.pages.get(index).and_then(|p|p.overlays.get(overlay)) else {return Err(publication_error());};
                highlights.push(highlight);
            }
            let page_id = *page_ids.get(&((index+1) as u32)).ok_or_else(||publication_error())?;
            let unit = physical_units::page_unit(document,page_id)?;
            let generated = render_references(pdfium,*geometries.get(index).ok_or_else(||publication_error())?,unit,&highlights,&mut self.budget).ok_or_else(||publication_error())?;
            let generated_page = generated.get_dictionary(*generated.get_pages().get(&1).ok_or_else(||publication_error())?).map_err(|_|publication_error())?;
            let generated_annotations = generated.dereference(generated_page.get(b"Annots").map_err(|_|publication_error())?).map_err(|_|publication_error())?.1.as_array().map_err(|_|publication_error())?;
            if generated_annotations.len() != highlights.len() { return Err(publication_error()); }
            for (offset, &(_, slot, _)) in self.slots[start..end].iter().enumerate() {
                check()?;
                let expected = generated.dereference(&generated_annotations[offset]).map_err(|_|publication_error())?.1.as_dict().map_err(|_|publication_error())?;
                let (target_id,array_id) = {
                    let page = document.get_dictionary(page_id).map_err(|_|publication_error())?;
                    let (array_id,array) = document.dereference(page.get(b"Annots").map_err(|_|publication_error())?).map_err(|_|publication_error())?;
                    let annotations = array.as_array().map_err(|_|publication_error())?;
                    let (id,target) = document.dereference(annotations.get(slot).ok_or_else(||publication_error())?).map_err(|_|publication_error())?;
                    let target = target.as_dict().map_err(|_|publication_error())?;
                    let actual_hash=shape_hash(document,target,&mut self.budget).ok_or_else(||publication_error())?;
                    let expected_hash=shape_hash(&generated,expected,&mut self.budget).ok_or_else(||publication_error())?;
                    if actual_hash != expected_hash { return Err(publication_error()); }
                    (id,array_id)
                };
                let ap = generated.dereference(expected.get(b"AP").map_err(|_|publication_error())?).map_err(|_|publication_error())?.1.as_dict().map_err(|_|publication_error())?;
                let normal = generated.dereference(ap.get(b"N").map_err(|_|publication_error())?).map_err(|_|publication_error())?.1;
                // This renderer's highlight AP is a closed stream with inline
                // ExtGState. Reserve every cloned value/byte before copying it;
                // a future renderer with external dependencies must be handled
                // explicitly, never copied with dangling source references.
                if !matches!(normal,Object::Stream(_)) { return Err(publication_error()); }
                closed_copy_budget(normal,&mut self.budget,0).ok_or_else(||publication_error())?;
                let normal_id = document.add_object(normal.clone());
                let target = if let Some(id)=target_id {
                    document.get_dictionary_mut(id).map_err(|_|publication_error())?
                } else {
                    let array = if let Some(id)=array_id { document.get_object_mut(id).map_err(|_|publication_error())? }
                        else { document.get_dictionary_mut(page_id).map_err(|_|publication_error())?.get_mut(b"Annots").map_err(|_|publication_error())? };
                    array.as_array_mut().map_err(|_|publication_error())?.get_mut(slot).ok_or_else(||publication_error())?.as_dict_mut().map_err(|_|publication_error())?
                };
                target.set("AP",dictionary!{"N"=>normal_id});
            }
            start = end;
        }
        Ok(true)
    }
}
fn shape_hash(doc:&Document, annotation:&Dictionary, budget:&mut Budget)->Option<[u8;32]> {
    let mut hash=Sha256::new();
    for key in [b"Subtype".as_slice(),b"Rect",b"QuadPoints",b"C",b"CA"] {
        value(doc,annotation.get(key).ok()?,&mut hash,budget,0)?;
    }
    Some(hash.finalize().into())
}
fn closed_copy_budget(object:&Object,budget:&mut Budget,depth:usize)->Option<()> {
    if depth>64 {return None;}
    budget.reserve(1,std::mem::size_of::<Object>())?;
    match object {
        Object::Reference(_)=>return None,
        Object::Name(v)|Object::String(v,_)=>budget.reserve(0,v.len())?,
        Object::Array(v)=>{for item in v {closed_copy_budget(item,budget,depth+1)?;}},
        Object::Dictionary(v)=>closed_dict_budget(v,budget,depth+1)?,
        Object::Stream(v)=>{budget.reserve(0,v.content.len())?;closed_dict_budget(&v.dict,budget,depth+1)?;},
        Object::Real(v) if !v.is_finite()=>return None,
        _=>(),
    }
    Some(())
}
fn closed_dict_budget(dictionary:&Dictionary,budget:&mut Budget,depth:usize)->Option<()> {
    if depth>64 {return None;}
    for (key,value) in dictionary.iter() {
        budget.reserve(1,key.len().checked_add(std::mem::size_of::<Vec<u8>>())?)?;
        closed_copy_budget(value,budget,depth+1)?;
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlight_admission_publication_reserves_before_recording_or_copying() {
        let mut publication=Publication::default();publication.budget.work=0;
        assert!(publication.record(0,0,0).is_err());assert!(publication.slots.is_empty());
        let mut budget=Budget{work:100,bytes:16};
        assert!(closed_copy_budget(&Object::String(vec![0;128],lopdf::StringFormat::Literal),&mut budget,0).is_none());
        assert!(closed_copy_budget(&Object::Reference((1,0)),&mut Budget::default(),0).is_none(),"generated appearance dependencies cannot escape their staging document");
    }

    #[test]
    fn highlight_admission_malformed_stream_length_is_not_equivalence_evidence() {
        let doc = Document::with_version("1.7");
        let mut stream = lopdf::Stream::new(dictionary!{}, b"q Q".to_vec());
        stream.dict.set("Length", 90);
        assert!(value(&doc, &Object::Stream(stream), &mut Sha256::new(), &mut Budget::default(), 0).is_none());
    }

    #[test]
    fn highlight_admission_failed_decompression_consumes_its_allowance() {
        let doc = Document::with_version("1.7");
        let mut stream = lopdf::Stream::new(dictionary!{}, vec![b' '; 32_768]);
        stream.compress().unwrap();
        assert!(stream.dict.has(b"Filter"));
        let mut budget = Budget { work: 1000, bytes: 2048 };
        assert!(value(&doc, &Object::Stream(stream), &mut Sha256::new(), &mut budget, 0).is_none());
        assert_eq!(budget.bytes, 0, "failed expansion must not repeatedly reuse the cumulative decoding allowance");
    }

    #[test]
    fn highlight_admission_cycles_and_oversized_graphs_refuse_with_bounded_work() {
        let mut doc = Document::with_version("1.7");
        let id = doc.new_object_id();
        doc.objects.insert(id, Object::Reference(id));
        let mut budget = Budget::default();
        assert!(value(&doc, &Object::Reference(id), &mut Sha256::new(), &mut budget, 0).is_none());
        assert!(budget.work >= 1_000_000 - 65);
        let mut budget = Budget { work: 3, bytes: 256 };
        assert!(value(&doc, &Object::Array(vec![Object::Null; 4]), &mut Sha256::new(), &mut budget, 0).is_none());
    }
}
