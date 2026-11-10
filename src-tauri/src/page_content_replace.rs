//! Immutable content grafting. Destination page and annotation identities survive.
use super::*;
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream};

fn bad(message: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidRequest(format!("Page replacement: {message}"))
}
impl PdfEngine {
    /// Returns a complete new destination source; neither input is changed.
    pub fn replace_page_content(
        &self,
        destination_id: &str,
        destination_page: usize,
        incoming_id: &str,
        incoming_page: usize,
    ) -> EngineResult<DocumentInfo> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::ReplacePageContent {
            destination_id: destination_id.into(),
            destination_page,
            incoming_id: incoming_id.into(),
            incoming_page,
            reply,
        })?;
        self.document_in_points(receive.recv().map_err(|_| EngineError::WorkerStopped)?)
    }
}

fn resolve<'a>(pdf: &'a Document, value: &'a Object) -> EngineResult<&'a Object> {
    let mut current = value;
    let mut seen = HashSet::new();
    while let Object::Reference(id) = current {
        if seen.len() >= 64 || !seen.insert(*id) {
            return Err(bad("cyclic or deep metadata reference"));
        }
        current = pdf
            .objects
            .get(id)
            .ok_or_else(|| bad("missing metadata reference"))?;
    }
    Ok(current)
}
fn guard(pdf: &Document, page: ObjectId, incoming: bool, context: Option<&text_edit::PublicationProof>) -> EngineResult<()> {
    let catalog = pdf.catalog().map_err(bad)?;
    if [
        b"StructTreeRoot".as_slice(),
        b"OCProperties",
        b"OutputIntents",
    ]
    .iter()
    .any(|key| catalog.has(key) && !(context.is_some() && *key==b"StructTreeRoot"))
    {
        return Err(bad(
            "tagged, layered, or output-intent documents are not supported yet",
        ));
    }
    let page = pdf.get_dictionary(page).map_err(bad)?;
    if [
        b"Group".as_slice(),
        b"StructParents",
        b"PieceInfo",
        b"SeparationInfo",
        b"PresSteps",
        b"VP",
        b"B",
    ]
    .iter()
    .any(|key| page.has(key) && !(context.is_some() && matches!(*key,b"Group"|b"StructParents")))
    {
        return Err(bad(
            "this page has document-linked content or transparency metadata",
        ));
    }
    for object in pdf.objects.values() {
        let dict = match object {
            Object::Dictionary(d) => d,
            Object::Stream(s) => &s.dict,
            _ => continue,
        };
        if dict
            .get(b"Type")
            .ok()
            .map(|v| resolve(pdf, v))
            .transpose()?
            .is_some_and(|v| v.as_name().is_ok_and(|n| matches!(n, b"OCG" | b"OCMD")))
        {
            return Err(bad("optional content cannot be transplanted safely"));
        }
        if incoming && (dict.has(b"FolioOCR") || dict.has(b"FolioOCREmpty")) {
            return Err(bad(
                "incoming Folio OCR requires a separate flattening workflow",
            ));
        }
    }
    Ok(())
}

fn point(g: PageGeometry, x: f64, y: f64, inverse: bool) -> (f64, f64) {
    let (l, b, r, t) = (g.left as f64, g.bottom as f64, g.right as f64, g.top as f64);
    match (inverse, g.rotation) {
        (false, 0) => (x - l, t - y),
        (false, 90) => (y - b, x - l),
        (false, 180) => (r - x, y - b),
        (false, 270) => (t - y, r - x),
        (true, 0) => (l + x, t - y),
        (true, 90) => (l + y, b + x),
        (true, 180) => (r - x, b + y),
        (true, 270) => (r - y, t - x),
        _ => unreachable!("validated quarter turn"),
    }
}
fn placement(source: PageGeometry, target: PageGeometry) -> EngineResult<([f32; 6], bool)> {
    for g in [source, target] {
        if !matches!(g.rotation, 0 | 90 | 180 | 270)
            || ![g.left, g.bottom, g.right, g.top]
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 1_000_000.)
            || g.right <= g.left
            || g.top <= g.bottom
        {
            return Err(bad("page geometry is invalid or exceeds fitting limits"));
        }
    }
    let size = |g: PageGeometry| {
        let w = g.right as f64 - g.left as f64;
        let h = g.top as f64 - g.bottom as f64;
        if g.rotation % 180 == 0 {
            (w, h)
        } else {
            (h, w)
        }
    };
    let (sw, sh) = size(source);
    let (tw, th) = size(target);
    let scale = (tw / sw).min(th / sh);
    let convert = |x, y| {
        let (x, y) = point(source, x, y, false);
        point(
            target,
            (tw - sw * scale) / 2. + x * scale,
            (th - sh * scale) / 2. + y * scale,
            true,
        )
    };
    let (e, f) = convert(0., 0.);
    let (x, y) = convert(1., 0.);
    let (u, v) = convert(0., 1.);
    let matrix = [
        (x - e) as f32,
        (y - f) as f32,
        (u - e) as f32,
        (v - f) as f32,
        e as f32,
        f as f32,
    ];
    if !matrix.iter().all(|v| v.is_finite()) || scale < 0.0001 || scale > 10_000. {
        return Err(bad("page fitting transform exceeds safe limits"));
    }
    // No epsilon: a slightly smaller incoming viewport still needs a clip.
    Ok((matrix, tw * sh != th * sw))
}

#[derive(Default)]
struct AppearanceCopies {
    budget: crate::form_graph::CopyBudget,
    // One instance per destination page: every clone has the same inherited
    // context, and aliases on other pages still refer to their untouched APs.
    memo: HashMap<ObjectId, ObjectId>,
    visits: usize,
}
fn appearance(
    pdf: &mut Document,
    value: &Object,
    resources: &Object,
    depth: usize,
    copies: &mut AppearanceCopies,
) -> EngineResult<Object> {
    copies.visits += 1;
    if depth > 64 || copies.visits > 100_000 {
        return Err(bad("annotation appearances exceed preservation limits"));
    }
    let original_id = value.as_reference().ok();
    if let Some(id) = original_id.and_then(|id| copies.memo.get(&id)) {
        return Ok(Object::Reference(*id));
    }
    let copied = match resolve(pdf, value)? {
        Object::Stream(stream) => {
            copies.budget.stream(stream)?;
            if !stream.dict.has(b"Resources") {
                if let Object::Dictionary(dict) = resources {
                    copies.budget.dictionary(dict)?;
                }
            }
            let mut stream = stream.clone();
            if !stream.dict.has(b"Resources") {
                stream.dict.set("Resources", resources.clone());
            }
            Object::Reference(pdf.add_object(stream))
        }
        Object::Dictionary(dict) => {
            copies.budget.dictionary(dict)?;
            let dict = dict.clone();
            let mut output = Dictionary::new();
            for (key, value) in dict.iter() {
                output.set(
                    key.clone(),
                    appearance(pdf, value, resources, depth + 1, copies)?,
                );
            }
            output.into()
        }
        _ => return Err(bad("malformed annotation appearance")),
    };
    if let Some(original_id) = original_id {
        let id = match copied {
            Object::Reference(id) => id,
            object => pdf.add_object(object),
        };
        copies.memo.insert(original_id, id);
        Ok(Object::Reference(id))
    } else {
        Ok(copied)
    }
}
fn field_value<'a>(
    pdf: &'a Document,
    annotation: &'a Dictionary,
    key: &[u8],
) -> EngineResult<Option<&'a Object>> {
    let mut field = annotation;
    for _ in 0..64 {
        if let Ok(value) = field.get(key) {
            return Ok(Some(resolve(pdf, value)?));
        }
        match field.get(b"Parent") {
            Ok(parent) => field = resolve(pdf, parent)?.as_dict().map_err(bad)?,
            Err(_) => return Ok(None),
        }
    }
    Err(bad("field inheritance exceeds preservation limits"))
}
fn resource_font<'a>(pdf: &'a Document, resources: Option<&'a Object>, name: &[u8]) -> EngineResult<Option<&'a Dictionary>> {
    let Some(resources) = resources else { return Ok(None); };
    let resources = resolve(pdf, resources)?.as_dict().map_err(bad)?;
    let Ok(fonts) = resources.get(b"Font") else { return Ok(None); };
    let fonts = resolve(pdf, fonts)?.as_dict().map_err(bad)?;
    let Ok(font) = fonts.get(name) else { return Ok(None); };
    let font = resolve(pdf, font)?.as_dict().map_err(bad)?;
    let named = |dict: &Dictionary, key: &[u8]| -> EngineResult<Vec<u8>> {
        let name = resolve(pdf, dict.get(key).map_err(bad)?)?.as_name().map_err(bad)?;
        if name.is_empty() || name.len() > 1024 { return Err(bad("widget font name exceeds limits")); }
        Ok(name.to_vec())
    };
    if named(font, b"Type")? != b"Font" { return Err(bad("widget resource is not a font")); }
    match named(font, b"Subtype")?.as_slice() {
        b"Type0" => {
            named(font, b"BaseFont")?;
            let descendants = resolve(pdf, font.get(b"DescendantFonts").map_err(bad)?)?.as_array().map_err(bad)?;
            if descendants.len() != 1 { return Err(bad("widget composite font needs one descendant")); }
            let child = resolve(pdf, &descendants[0])?.as_dict().map_err(bad)?;
            if named(child, b"Type")? != b"Font" || !matches!(named(child, b"Subtype")?.as_slice(), b"CIDFontType0" | b"CIDFontType2") {
                return Err(bad("widget composite font descendant is malformed"));
            }
            named(child, b"BaseFont")?;
            if !matches!(resolve(pdf, font.get(b"Encoding").map_err(bad)?)?, Object::Name(_) | Object::Stream(_)) {
                return Err(bad("widget composite font encoding is malformed"));
            }
        }
        b"Type1" | b"MMType1" | b"TrueType" => { named(font, b"BaseFont")?; }
        b"Type3" => {
            resolve(pdf, font.get(b"CharProcs").map_err(bad)?)?.as_dict().map_err(bad)?;
            for (key, count) in [(b"FontMatrix".as_slice(),6),(b"FontBBox".as_slice(),4)] {
                let values = resolve(pdf, font.get(key).map_err(bad)?)?.as_array().map_err(bad)?;
                if values.len() != count || values.iter().any(|v| resolve(pdf,v).and_then(|v|v.as_float().map_err(bad)).map_or(true,|v|!v.is_finite())) {
                    return Err(bad("widget Type3 font geometry is malformed"));
                }
            }
        }
        _ => return Err(bad("widget font subtype is unsupported or malformed")),
    }
    Ok(Some(font))
}
fn normal_appearance<'a>(pdf: &'a Document, annotation: &'a Dictionary) -> EngineResult<Option<&'a Stream>> {
    let Ok(ap) = annotation.get(b"AP") else { return Ok(None); };
    let ap = resolve(pdf, ap)?.as_dict().map_err(bad)?;
    let Ok(normal) = ap.get(b"N") else { return Ok(None); };
    // A state dictionary does not itself supply a text appearance's fonts.
    // Button states are retained separately by appearance().
    match resolve(pdf, normal)? {
        Object::Stream(stream) => Ok(Some(stream)),
        Object::Dictionary(_) => Ok(None),
        _ => Err(bad("malformed normal widget appearance")),
    }
}
fn default_appearance_context(pdf: &Document, annotation: &Dictionary, page_resources: &Object, budget: &mut crate::form_graph::CopyBudget) -> EngineResult<()> {
    let form = pdf
        .catalog()
        .map_err(bad)?
        .get(b"AcroForm")
        .map_err(|_| bad("widget default resources are missing"))?;
    let form = resolve(pdf, form)?.as_dict().map_err(bad)?;
    if field_value(pdf, annotation, b"FT")?.is_some_and(|v| {
        v.as_name()
            .is_ok_and(|name| matches!(name, b"Btn" | b"Sig"))
    }) {
        return Ok(());
    }
    let da = field_value(pdf, annotation, b"DA")?
        .or_else(|| form.get(b"DA").ok())
        .ok_or_else(|| bad("widget default appearance is missing"))?;
    let data = resolve(pdf, da)?.as_str().map_err(bad)?;
    if data.len() > 64 * 1024 {
        return Err(bad("widget default appearance exceeds limits"));
    }
    let content = lopdf::content::Content::decode(data).map_err(bad)?;
    let field_resources = field_value(pdf, annotation, b"DR")?;
    let form_resources = form.get(b"DR").ok();
    let normal = normal_appearance(pdf, annotation)?;
    let mut found = false;
    for op in content.operations.iter().filter(|op| op.operator == "Tf") {
        let [Object::Name(name), size] = op.operands.as_slice() else {
            return Err(bad("widget default font is malformed"));
        };
        if !size.as_float().is_ok_and(|v| v.is_finite() && v >= 0.) {
            return Err(bad("widget default font size is malformed"));
        }
        // These namespaces survive the graft independently of the new page
        // resources. Local AP fonts are common in producer-generated forms;
        // preserve them without manufacturing global aliases or changing DA.
        let field_font = resource_font(pdf, field_resources, name)?;
        let form_font = resource_font(pdf, form_resources, name)?;
        let local_font = resource_font(pdf, normal.and_then(|s|s.dict.get(b"Resources").ok()), name)?;
        if field_font.is_none() && form_font.is_none() && local_font.is_none() {
            return Err(bad("widget default font depends on page resources"));
        }
        // appearance() supplies absent AP resources for unchanged rendering.
        // PDFium's filling map consults AP fonts, then global DR (not field DR).
        // Do not let that new AP binding override a different original fill
        // font, including an unspecified platform fallback.
        if normal.is_some_and(|s|!s.dict.has(b"Resources")) {
            if let Some(page_font) = resource_font(pdf, Some(page_resources), name)? {
                let Some(original_font) = form_font else { return Err(bad("preserving the appearance would change its later fill font")); };
                if !std::ptr::eq(page_font,original_font) {
                    budget.dictionary(page_font)?;budget.dictionary(original_font)?;
                    if page_font != original_font { return Err(bad("preserving the appearance would change its later fill font")); }
                }
            }
        }
        found = true;
    }
    if !found {
        return Err(bad("widget default font cannot be preserved"));
    }
    Ok(())
}
pub(super) fn retain_appearance_contexts(pdf: &mut Document, page: ObjectId) -> EngineResult<()> {
    let mut copies = AppearanceCopies::default();
    let annotations = match pdf.get_dictionary(page).map_err(bad)?.get(b"Annots") {
        Ok(value) => {
            let array = resolve(pdf, value)?.as_array().map_err(bad)?;
            if array.len() > 10_000 {
                return Err(bad("too many destination annotations"));
            }
            for value in array {
                if let Object::Dictionary(dict) = value {
                    copies.budget.dictionary(dict)?;
                }
            }
            array.clone()
        }
        Err(_) => return Ok(()),
    };
    if annotations.is_empty() {
        return Ok(());
    }
    let resources = crate::page_resources::inherited_page_resources(pdf, page)?;
    if let Some(Object::Dictionary(dict)) = resources {
        copies.budget.dictionary(dict)?;
    }
    let resources = resources.cloned().unwrap_or_else(|| dictionary! {}.into());
    resolve(pdf, &resources)?.as_dict().map_err(bad)?;
    let mut owners = HashMap::<ObjectId, usize>::new();
    let mut annotation_count = 0usize;
    for other in pdf.get_pages().values() {
        if let Ok(value) = pdf.get_dictionary(*other).map_err(bad)?.get(b"Annots") {
            for value in resolve(pdf, value)?.as_array().map_err(bad)? {
                annotation_count += 1;
                if annotation_count > 100_000 {
                    return Err(bad("annotation ownership exceeds preservation limits"));
                }
                if let Object::Reference(id) = value {
                    *owners.entry(*id).or_default() += 1;
                }
            }
        }
    }
    let mut output = Vec::new();
    for value in annotations {
        if let Object::Reference(id) = value {
            if owners.get(&id) != Some(&1) {
                return Err(bad("an annotation is shared by multiple page entries"));
            }
        }
        let annotation = resolve(pdf, &value)?.as_dict().map_err(bad)?;
        copies.budget.dictionary(annotation)?;
        let mut annotation = annotation.clone();
        if annotation
            .get(b"Subtype")
            .ok()
            .map(|value| resolve(pdf, value))
            .transpose()?
            .is_some_and(|value| value.as_name().is_ok_and(|n| n == b"Widget"))
        {
            default_appearance_context(pdf, &annotation, &resources, &mut copies.budget)?;
        }
        if let Ok(ap) = annotation.get(b"AP") {
            let ap = appearance(pdf, ap, &resources, 0, &mut copies)?;
            annotation.set("AP", ap);
        }
        if let Object::Reference(id) = value {
            pdf.objects.insert(id, annotation.into());
            output.push(value);
        } else {
            output.push(annotation.into());
        }
    }
    pdf.get_dictionary_mut(page)
        .map_err(bad)?
        .set("Annots", output);
    Ok(())
}

pub(super) fn replace(
    destination: &[u8],
    destination_page: usize,
    incoming: &[u8],
    incoming_page: usize,
    target_geometry: PageGeometry,
    source_geometry: PageGeometry,
) -> EngineResult<Vec<u8>> {
    replace_impl(destination, destination_page, incoming, incoming_page, target_geometry, source_geometry, false, None)
        .map(page_admission::Publication::into_bytes)
}

/// Exact text publication has no fitting transform and carries disposable
/// paragraph hints alongside the operators to which they are bound.
pub(super) fn replace_edited_text(
    destination: &[u8], destination_page: usize, incoming: &[u8],
    target_geometry: PageGeometry, source_geometry: PageGeometry,
) -> EngineResult<page_admission::Publication> {
    replace_impl(destination, destination_page, incoming, 0, target_geometry, source_geometry, true, None)
}

pub(super) fn replace_context_text(
    destination: &[u8], destination_page: usize, incoming: &[u8],
    target_geometry: PageGeometry, source_geometry: PageGeometry, context: &text_edit::PublicationProof,
) -> EngineResult<page_admission::Publication> {
    replace_impl(destination,destination_page,incoming,0,target_geometry,source_geometry,true,Some(context))
}

fn replace_impl(
    destination: &[u8], destination_page: usize, incoming: &[u8], incoming_page: usize,
    target_geometry: PageGeometry, source_geometry: PageGeometry, exact_text: bool, context: Option<&text_edit::PublicationProof>,
) -> EngineResult<page_admission::Publication> {
    crate::form_graph::preflight(destination)?;
    crate::form_graph::preflight(incoming)?;
    let mut target = crate::pdf_output::load(destination)?;
    let source = crate::pdf_output::load(incoming)?;
    let target_page = *target
        .get_pages()
        .values()
        .nth(destination_page)
        .ok_or_else(|| bad("destination page is missing"))?;
    let source_page = *source
        .get_pages()
        .values()
        .nth(incoming_page)
        .ok_or_else(|| bad("incoming page is missing"))?;
    if let Some(context)=context {context.publication(&target,target_page,&source,source_page)?;}
    guard(&target, target_page, false, context)?;
    guard(&source, source_page, true, context)?;
    let (matrix, clip) = placement(source_geometry, target_geometry)?;
    if exact_text && (matrix != [1., 0., 0., 1., 0., 0.] || clip
        || target_geometry.rotation != source_geometry.rotation
        || [target_geometry.left,target_geometry.bottom,target_geometry.right,target_geometry.top]
            != [source_geometry.left,source_geometry.bottom,source_geometry.right,source_geometry.top]
        || physical_units::page_unit(&target,target_page)? != physical_units::page_unit(&source,source_page)?) {
        return Err(bad("edited text must retain the exact page geometry and units"));
    }
    let mut hints = Vec::new();
    if exact_text {
        for (key, limit) in [(b"FolioParagraphs".as_slice(),512*1024),(b"FolioStyledParagraphs".as_slice(),4*1024*1024)] {
            if let Ok(value)=source.get_dictionary(source_page).map_err(bad)?.get(key) {
                // Reader-owned hints are optional. Retain only the bounded direct
                // strings consumed by the independent replay validators.
                if let Object::String(bytes,_) = value {
                    if bytes.len() <= limit {hints.push((key,value.clone()));}
                }
            }
        }
    }
    let source_content = source
        .get_page_content_with_limit(source_page, 16 * 1024 * 1024)
        .map_err(bad)?;
    // An unmatched Q could consume the fitting wrapper's state and escape its
    // crop. Require balanced state before preserving the original stream bytes.
    let parsed = lopdf::content::Content::decode(&source_content).map_err(bad)?;
    if parsed.operations.len() > 100_000 {
        return Err(bad("incoming content exceeds operation limits"));
    }
    let (mut graphics, mut marked, mut text) = (0usize, 0usize, false);
    for op in &parsed.operations {
        match op.operator.as_str() {
            "q" => graphics += 1,
            "Q" => {
                graphics = graphics
                    .checked_sub(1)
                    .ok_or_else(|| bad("unbalanced graphics state"))?
            }
            "BT" if !text => text = true,
            "ET" if text => text = false,
            "BT" | "ET" => return Err(bad("unbalanced text state")),
            "BMC" | "BDC" => marked += 1,
            "EMC" => {
                marked = marked
                    .checked_sub(1)
                    .ok_or_else(|| bad("unbalanced marked content"))?
            }
            _ => {}
        }
        if graphics > 64 || marked > 64 {
            return Err(bad("incoming content exceeds nesting limits"));
        }
    }
    if graphics != 0 || marked != 0 || text {
        return Err(bad("unbalanced incoming content"));
    }
    retain_appearance_contexts(&mut target, target_page)?;
    let resources =
        crate::page_resources::import_page_resources(&source, source_page, &mut target)?;
    let mut content = Vec::new();
    if matrix == [1., 0., 0., 1., 0., 0.] && !clip {
        content = source_content;
    } else {
        content.extend_from_slice(
            format!(
                "q {} {} {} {} {} {} cm\n",
                matrix[0], matrix[1], matrix[2], matrix[3], matrix[4], matrix[5]
            )
            .as_bytes(),
        );
        if clip {
            content.extend_from_slice(
                format!(
                    "{} {} {} {} re W n\n",
                    source_geometry.left,
                    source_geometry.bottom,
                    source_geometry.raw_width(),
                    source_geometry.raw_height()
                )
                .as_bytes(),
            );
        }
        content.extend_from_slice(&source_content);
        content.extend_from_slice(b"\nQ");
    }
    let contents = target.add_object(Stream::new(dictionary! {}, content));
    let page = target.get_dictionary_mut(target_page).map_err(bad)?;
    page.set("Contents", contents);
    page.set("Resources", resources);
    for key in [
        b"FolioParagraphs".as_slice(),
        b"FolioStyledParagraphs",
        b"Thumb",
    ] {
        page.remove(key);
    }
    for (key,value) in hints {page.set(key,value);}
    crate::page_resources::prune_unreachable(&mut target)?;
    if let Some(context)=context {context.published(&target,target_page)?;}
    page_admission::Publication::save(&mut target,exact_text)
}

impl WorkerRuntime {
    pub(super) fn replace_page_content(
        &mut self,
        destination_id: &str,
        destination_page: usize,
        incoming_id: &str,
        incoming_page: usize,
    ) -> EngineResult<DocumentInfo> {
        let target = self.editing_document(destination_id)?;
        let source = self.copying_document(incoming_id)?;
        if source.policy.is_some() {
            return Err(bad("incoming protected PDFs are not supported yet"));
        }
        if source
            .imported_annotations
            .get(&incoming_page)
            .is_some_and(|items| !items.is_empty())
        {
            return Err(bad(
                "incoming Folio additions require a separate flattening workflow",
            ));
        }
        let target_geometry = page_geometry(
            &target
                .pdf()?
                .pages()
                .get(page_index_i32(destination_page)?)?,
        )?;
        let source_geometry = page_geometry(
            &source
                .pdf()?
                .pages()
                .get(page_index_i32(incoming_page)?)?,
        )?;
        let bytes = replace(
            target.bytes()?.as_ref(),
            destination_page,
            source.bytes()?.as_ref(),
            incoming_page,
            target_geometry,
            source_geometry,
        )?;
        let bookmarks = target.source_bookmarks()?
            .clone();
        let policy = target.policy.clone();
        let original_path = target.original_path.clone();
        let legacy = target.legacy_comment_import;
        let path =
            std::env::temp_dir().join(format!("folio-page-replacement-{}.pdf", Uuid::new_v4()));
        // Use normal source registration so imported overlay assets and all metadata
        // receive the same ownership and recovery handling as an opened document.
        let mut info = self.open_bytes(
            bytes,
            path,
            original_path,
            true,
            true,
            legacy,
            OpenSecurity::Restored(policy),
        )?;
        info = self.retain_annotation_sources(destination_id, info)?;
        info.name = "Replaced page content.pdf".into();
        info.bookmarks = bookmarks.as_ref().cloned().unwrap_or_default();
        info.bookmark_error = bookmarks.as_ref().err().cloned();
        self.documents.get_mut(&info.id).unwrap().bookmarks = OnceLock::from(bookmarks);
        Ok(info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_thousand_annotations_share_one_isolated_megabyte_appearance() {
        let mut pdf = Document::with_version("1.7");
        let pages = pdf.new_object_id();
        let page=pdf.add_object(dictionary! {"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),100.into(),100.into()],"Resources"=>dictionary! {}});
        pdf.objects.insert(
            pages,
            dictionary! {"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1}.into(),
        );
        let catalog = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
        pdf.trailer.set("Root", catalog);
        let source = crate::pdf_output::save(&mut pdf).unwrap();
        let ap=pdf.add_object(Stream::new(dictionary! {"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),10.into(),10.into()]},vec![b' ';1024*1024]));
        let mut annotations = Vec::new();
        for _ in 0..1024 {
            annotations.push(Object::Reference(pdf.add_object(dictionary! {"Type"=>"Annot","Subtype"=>"Stamp","Rect"=>vec![0.into(),0.into(),10.into(),10.into()],"AP"=>dictionary! {"N"=>ap}})));
        }
        pdf.get_dictionary_mut(page)
            .unwrap()
            .set("Annots", annotations.clone());
        let target = crate::pdf_output::save(&mut pdf).unwrap();
        let geometry = PageGeometry {
            left: 0.,
            bottom: 0.,
            right: 100.,
            top: 100.,
            rotation: 0,
        };
        let output = replace(&target, 0, &source, 0, geometry, geometry).unwrap();
        assert!(
            output.len() < 2 * 1024 * 1024,
            "shared APs must not multiply stream bytes"
        );
        let output = crate::pdf_output::load(&output).unwrap();
        let mut appearances = HashSet::new();
        for annotation in annotations {
            let annotation = resolve(&output, &annotation).unwrap().as_dict().unwrap();
            let ap = resolve(&output, annotation.get(b"AP").unwrap())
                .unwrap()
                .as_dict()
                .unwrap();
            appearances.insert(ap.get(b"N").unwrap().as_reference().unwrap());
        }
        assert_eq!(appearances.len(), 1);
    }
    #[test]
    fn oversized_annotation_appearance_is_rejected_before_cloning() {
        let mut pdf = Document::with_version("1.7");
        let pages = pdf.new_object_id();
        let page=pdf.add_object(dictionary! {"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),100.into(),100.into()],"Resources"=>dictionary! {}});
        pdf.objects.insert(
            pages,
            dictionary! {"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1}.into(),
        );
        let catalog = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
        pdf.trailer.set("Root", catalog);
        let source = crate::pdf_output::save(&mut pdf).unwrap();
        let ap=pdf.add_object(Stream::new(dictionary! {"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),10.into(),10.into()]},vec![b' ';16*1024*1024+1]));
        let note=pdf.add_object(dictionary! {"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![0.into(),0.into(),10.into(),10.into()],"AP"=>dictionary! {"N"=>ap}});
        pdf.get_dictionary_mut(page)
            .unwrap()
            .set("Annots", vec![Object::Reference(note)]);
        let target = crate::pdf_output::save(&mut pdf).unwrap();
        let geometry = PageGeometry {
            left: 0.,
            bottom: 0.,
            right: 100.,
            top: 100.,
            rotation: 0,
        };
        assert!(replace(&target, 0, &source, 0, geometry, geometry).is_err());
    }
    #[test]
    fn rejects_recursive_incoming_form_before_publication() {
        let mut pdf = Document::with_version("1.7");
        let pages = pdf.new_object_id();
        let form = pdf.new_object_id();
        pdf.objects.insert(form,Stream::new(dictionary! {"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),100.into(),100.into()],"Resources"=>dictionary! {"XObject"=>dictionary! {"F"=>form}}},b"/F Do".to_vec()).into());
        let content = pdf.add_object(Stream::new(dictionary! {}, b"/F Do".to_vec()));
        let page=pdf.add_object(dictionary! {"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),100.into(),100.into()],"Resources"=>dictionary! {"XObject"=>dictionary! {"F"=>form}},"Contents"=>content});
        pdf.objects.insert(
            pages,
            dictionary! {"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1}.into(),
        );
        let catalog = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
        pdf.trailer.set("Root", catalog);
        let source = crate::pdf_output::save(&mut pdf).unwrap();
        pdf.get_dictionary_mut(page).unwrap().remove(b"Contents");
        pdf.get_dictionary_mut(page)
            .unwrap()
            .set("Resources", dictionary! {});
        let target = crate::pdf_output::save(&mut pdf).unwrap();
        let geometry = PageGeometry {
            left: 0.,
            bottom: 0.,
            right: 100.,
            top: 100.,
            rotation: 0,
        };
        assert!(replace(&target, 0, &source, 0, geometry, geometry).is_err());
    }
}
