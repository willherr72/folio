//! Generated-only appearances for the graph-preserving overlay adapter.
#![allow(dead_code)] // Public attachment follows ownership and tag integration.
use super::*;
use lopdf::{dictionary, Document, Object, ObjectId};

#[path="structured_overlay_selection.rs"]
mod selection;

const MAX_METADATA_BYTES: usize = 16 * 1024 * 1024;
fn bad(message: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidRequest(format!("structured overlay rendering: {message}"))
}

// Count serialized request bytes without allocating a second copy. Perform this
// before validation, shaping, asset preparation or cloning the editable request.
struct MetadataBudget(usize);
impl std::io::Write for MetadataBudget {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 = self
            .0
            .checked_add(bytes.len())
            .filter(|n| *n <= MAX_METADATA_BYTES)
            .ok_or_else(|| std::io::Error::other("cumulative overlay metadata exceeds 16 MiB"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn overlay_id(overlay: &Overlay) -> &str {
    match overlay {
        Overlay::Text(v) => &v.id,
        Overlay::Ink(v) => &v.id,
        Overlay::Image(v) => &v.id,
        Overlay::Highlight(v) => &v.id,
        Overlay::Comment(v) => &v.id,
    }
}
fn native(overlay: &Overlay) -> bool {
    matches!(overlay, Overlay::Highlight(_) | Overlay::Comment(_))
}
fn name(value: &str) -> Object {
    let bytes = std::iter::once(0xfeffu16)
        .chain(value.encode_utf16())
        .flat_map(u16::to_be_bytes)
        .collect::<Vec<_>>();
    Object::String(bytes, lopdf::StringFormat::Hexadecimal)
}
fn annotations(document: &Document, page: ObjectId) -> EngineResult<Vec<ObjectId>> {
    let page = document.get_dictionary(page).map_err(bad)?;
    let Ok(value) = page.get(b"Annots") else {
        return Ok(vec![]);
    };
    document
        .dereference(value)
        .map_err(bad)?
        .1
        .as_array()
        .map_err(bad)?
        .iter()
        .map(|v| v.as_reference().map_err(bad))
        .collect()
}

// Bound primitive expansion before validation can shape text and before native
// annotation/PDF value construction. This is a work admission limit, not a
// claim about process RSS or the final retained resource graph (checked below).
fn preflight_work(
    request: &ExportRequest,
    check: &mut impl FnMut() -> EngineResult<()>,
) -> EngineResult<()> {
    let mut remaining = 1_000_000usize;
    let mut reserve = |count: usize, weight: usize| -> EngineResult<()> {
        let amount = count
            .checked_mul(weight)
            .ok_or_else(|| bad("rendering work overflow"))?;
        remaining = remaining
            .checked_sub(amount)
            .ok_or_else(|| bad("rendering work exceeds the cumulative limit"))?;
        Ok(())
    };
    for page in &request.pages {
        check()?;
        reserve(1, 32)?;
        for overlay in &page.overlays {
            check()?;
            reserve(1, 64)?;
            match overlay {
                Overlay::Text(text) if text.shaping.is_some() => {
                    // Native shaping already bounds glyph and outline expansion.
                    // This additionally bounds repeated preparations in a batch.
                    reserve(text.text.chars().count(), 64)?;
                }
                Overlay::Text(text) => {
                    // A plain line produces six operators and their operands.
                    for (index, _) in text.text.lines().enumerate() {
                        if index % 256 == 0 {
                            check()?;
                        }
                        reserve(1, 32)?;
                    }
                }
                Overlay::Ink(ink) => {
                    for path in &ink.paths {
                        check()?;
                        reserve(path.len(), 12)?;
                    }
                }
                Overlay::Highlight(highlight) => reserve(highlight.rects.len(), 16)?,
                Overlay::Image(_) | Overlay::Comment(_) => (),
            }
        }
    }
    Ok(())
}

pub(super) struct GeneratedOverlay {
    pub overlay_id: String,
    pub annotation_id: ObjectId,
}
pub(super) struct GeneratedPage {
    pub page_id: ObjectId,
    pub geometry: PageGeometry,
    pub unit: f32,
    pub overlays: Vec<GeneratedOverlay>,
}
pub(super) struct PreparedOverlays {
    pub document: Document,
    pub pages: Vec<GeneratedPage>,
}
impl WorkerRuntime {
    pub(super) fn prepare_structured_overlays(
        &self,
        request: &ExportRequest,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<PreparedOverlays> {
        check()?;
        serde_json::to_writer(MetadataBudget(0), request).map_err(bad)?;
        preflight_work(request, check)?;
        check()?;
        self.validate_export(request)?;
        let mut blank = Document::with_version("1.7");
        let tree = blank.new_object_id();
        let mut kids = Vec::with_capacity(request.pages.len());
        let mut geometries = Vec::with_capacity(request.pages.len());
        let mut units = Vec::with_capacity(request.pages.len());
        for plan in &request.pages {
            check()?;
            let source = self.document(&plan.source_id)?;
            let page = source
                .pdf()?
                .pages()
                .get(page_index_i32(plan.page_index)?)?;
            let geometry = page_geometry(&page)?;
            let unit = self.source_page_unit(&plan.source_id, plan.page_index)?;
            let id=blank.add_object(dictionary!{
                "Type"=>"Page","Parent"=>tree,
                "MediaBox"=>vec![Object::Real(geometry.left),Object::Real(geometry.bottom),Object::Real(geometry.right),Object::Real(geometry.top)],
                "Rotate"=>i64::from(geometry.rotation),"UserUnit"=>unit,"Resources"=>dictionary!{}
            });
            kids.push(Object::Reference(id));
            geometries.push(geometry);
            units.push(unit);
        }
        blank.objects.insert(
            tree,
            dictionary! {"Type"=>"Pages","Count"=>kids.len() as i64,"Kids"=>kids}.into(),
        );
        let catalog = blank.add_object(dictionary! {"Type"=>"Catalog","Pages"=>tree});
        blank.trailer.set("Root", catalog);
        check()?;
        let bytes = crate::pdf_output::save(&mut blank)?;
        drop(blank);
        let stage = self.pdfium.load_pdf_from_reader(Cursor::new(bytes), None)?;
        let mut highlight_publication = highlight_appearance::Publication::default();
        for (index, plan) in request.pages.iter().enumerate() {
            check()?;
            let mut page = stage.pages().get(page_index_i32(index)?)?;
            for (overlay_index,overlay) in plan.overlays.iter().enumerate() {
                check()?;
                match overlay {
                    Overlay::Highlight(value) => {
                        highlight_publication.record(index,page.annotations().len() as usize,overlay_index)?;
                        add_highlight_annotation(&mut page, geometries[index], value)?
                    }
                    Overlay::Comment(value) => {
                        add_comment_annotation(&mut page, geometries[index], value, units[index])?
                    }
                    _ => (),
                }
            }
            page.regenerate_content()?;
        }
        check()?;
        let bytes = crate::pdf_output::from_pdfium(&stage)?;
        drop(stage);
        let mut document = crate::pdf_output::load(&bytes)?;
        highlight_publication.apply(self.pdfium,&mut document,request,&geometries,check)?;
        let page_ids = document.get_pages().into_values().collect::<Vec<_>>();
        if page_ids.len() != request.pages.len() {
            return Err(bad("generated page mapping differs"));
        }
        // These pages started empty, so native annotation creation order is
        // authoritative. Bind before portable finalization changes paint order.
        for (index, page_id) in page_ids.iter().copied().enumerate() {
            check()?;
            // PDFium may serialize newly created annotations inline. Move those
            // private dictionaries into objects before assigning stable bindings.
            if let Some(value) = document
                .get_dictionary_mut(page_id)
                .map_err(bad)?
                .remove(b"Annots")
            {
                let values = match value {
                    Object::Array(values) => values,
                    value => document
                        .dereference(&value)
                        .map_err(bad)?
                        .1
                        .as_array()
                        .map_err(bad)?
                        .clone(),
                };
                let mut refs = Vec::with_capacity(values.len());
                for value in values {
                    check()?;
                    let id = match value {
                        Object::Dictionary(dictionary) => document.add_object(dictionary),
                        Object::Reference(id) => {
                            document.get_dictionary(id).map_err(bad)?;
                            id
                        }
                        _ => return Err(bad("invalid generated annotation")),
                    };
                    refs.push(Object::Reference(id));
                }
                document
                    .get_dictionary_mut(page_id)
                    .map_err(bad)?
                    .set("Annots", refs);
            }
            let ids = annotations(&document, page_id)?;
            let expected = request.pages[index]
                .overlays
                .iter()
                .filter(|v| native(v))
                .collect::<Vec<_>>();
            if ids.len() != expected.len() {
                return Err(bad("native annotation mapping differs"));
            }
            for (id, overlay) in ids.into_iter().zip(expected) {
                let node = document.get_dictionary_mut(id).map_err(bad)?;
                let subtype = if matches!(overlay, Overlay::Highlight(_)) {
                    b"Highlight".as_slice()
                } else {
                    b"Text".as_slice()
                };
                if node
                    .get(b"Subtype")
                    .and_then(Object::as_name)
                    .map_err(bad)?
                    != subtype
                {
                    return Err(bad("native annotation kind differs"));
                }
                node.set("NM", name(overlay_id(overlay)));
                node.set("P", page_id);
            }
        }
        // Defer flattening, workspace crop and additional rotation to graph
        // attachment. Staging must retain every appearance and editable asset.
        let mut editable = request.clone();
        editable.flatten = false;
        persistence::finalize_generated(
            &mut document,
            &editable,
            &geometries,
            &self.fonts,
            &self.images,
            persistence::GeneratedLimits::default(),
            |_, _| check(),
        )?;
        let mut pages = Vec::with_capacity(page_ids.len());
        for (index, page_id) in page_ids.into_iter().enumerate() {
            check()?;
            let mut bindings = HashMap::new();
            for id in annotations(&document, page_id)? {
                let node = document.get_dictionary(id).map_err(bad)?;
                let key = lopdf::decode_text_string(node.get(b"NM").map_err(bad)?).map_err(bad)?;
                if node.get(b"P").and_then(Object::as_reference).map_err(bad)? != page_id
                    || bindings.insert(key, id).is_some()
                {
                    return Err(bad("ambiguous generated annotation ownership"));
                }
            }
            let mut overlays = Vec::with_capacity(request.pages[index].overlays.len());
            for overlay in &request.pages[index].overlays {
                let key = overlay_id(overlay);
                let annotation_id = bindings
                    .remove(key)
                    .ok_or_else(|| bad("missing generated annotation"))?;
                overlays.push(GeneratedOverlay {
                    overlay_id: key.to_owned(),
                    annotation_id,
                });
            }
            if !bindings.is_empty() {
                return Err(bad("unexpected generated annotation"));
            }
            pages.push(GeneratedPage {
                page_id,
                geometry: geometries[index],
                unit: units[index],
                overlays,
            });
        }
        crate::composition_graph::validate_retained(&document, check)?;
        check()?;
        Ok(PreparedOverlays { document, pages })
    }
}

#[cfg(test)]
#[path = "structured_overlay_render_tests.rs"]
pub(in crate::engine) mod tests;
