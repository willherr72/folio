//! A repeated workspace page can change without splitting its native source.
use super::*;
use lopdf::{Document, Object, ObjectId};

const MAX_PAGES: usize = 100_000;
fn bad(message: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidRequest(format!("Page replacement: {message}"))
}

/// The original document remains in this ID space. Reuse its appearance
/// references instead of trusting the page importer's resource-key rewriting.
fn restore_appended_appearances(pdf: &mut Document, old: ObjectId, new: ObjectId) -> EngineResult<()> {
    let annotations = |pdf: &Document, page| -> EngineResult<Vec<Object>> {
        let dict=pdf.get_dictionary(page).map_err(bad)?;
        let Ok(value)=dict.get(b"Annots") else {return Ok(vec![])};
        let values=pdf.dereference(value).map_err(bad)?.1.as_array().map_err(bad)?;
        if values.len()>10_000 {return Err(bad("too many copied annotations"));}
        // Bound direct dictionaries before cloning; indirect appearance bytes
        // are not copied and retain their original immutable objects.
        let mut budget=crate::form_graph::CopyBudget::default();
        for value in values {if let Object::Dictionary(dict)=value {budget.dictionary(dict)?;}}
        Ok(values.clone())
    };
    let original=annotations(pdf,old)?;
    let mut copied=annotations(pdf,new)?;
    if original.len()!=copied.len() {return Err(bad("copied annotation count changed"));}
    let mut budget=crate::form_graph::CopyBudget::default();
    for (source,target) in original.iter().zip(&mut copied) {
        let source=pdf.dereference(source).map_err(bad)?.1.as_dict().map_err(bad)?;
        let target_dict=pdf.dereference(target).map_err(bad)?.1.as_dict().map_err(bad)?;
        let kind=pdf.dereference(source.get(b"Subtype").map_err(bad)?).map_err(bad)?.1.as_name().map_err(bad)?;
        let copied_kind=pdf.dereference(target_dict.get(b"Subtype").map_err(bad)?).map_err(bad)?.1.as_name().map_err(bad)?;
        if kind!=copied_kind {return Err(bad("copied annotation identity changed"));}
        if kind==b"Widget" {continue;}
        budget.dictionary(source)?;
        let ap=source.get(b"AP").ok().cloned();
        let state=source.get(b"AS").ok().cloned();
        let target=match target {
            Object::Reference(id)=>pdf.get_dictionary_mut(*id).map_err(bad)?,
            value=>value.as_dict_mut().map_err(bad)?,
        };
        for (key,value) in [(b"AP".as_slice(),ap),(b"AS".as_slice(),state)] {
            if let Some(value)=value {target.set(key.to_vec(),value);} else {target.remove(key);}
        }
    }
    // The renderer may clone an annotation again through Popup.Parent. Restore
    // that typed back-edge from the explicit source/copy slot correspondence;
    // an equal detached dictionary is not the owner on the copied page.
    // Both arrays are capped at 10,000; reserve <= 256 bytes per map entry.
    let mut roots = std::collections::BTreeMap::new();
    for (source, target) in original.iter().zip(&copied) {
        let (source_id, _) = pdf.dereference(source).map_err(bad)?;
        let (target_id, _) = pdf.dereference(target).map_err(bad)?;
        if let Some(source_id) = source_id {
            roots.entry(source_id).and_modify(|entry| *entry = None).or_insert(target_id);
        }
    }
    for (source, target) in original.iter().zip(&mut copied) {
        let source = pdf.dereference(source).map_err(bad)?.1.as_dict().map_err(bad)?;
        let subtype = pdf.dereference(source.get(b"Subtype").map_err(bad)?).map_err(bad)?.1.as_name().map_err(bad)?;
        if subtype != b"Popup" {continue;}
        // Import accepts a forward link, a backlink, or both. Repair only the
        // backlink that actually exists; preserve missing/null optional fields.
        let Ok(parent) = source.get(b"Parent") else {continue};
        let (owner, value) = pdf.dereference(parent).map_err(bad)?;
        if matches!(value,Object::Null) {continue;}
        let owner = owner.ok_or_else(||bad("direct popup parent cannot be relocated"))?;
        let new_owner = roots.get(&owner).copied().flatten()
            .ok_or_else(||bad("popup owner is missing or ambiguous"))?;
        let target = match target {
            Object::Reference(id) => pdf.get_dictionary_mut(*id).map_err(bad)?,
            value => value.as_dict_mut().map_err(bad)?,
        };
        target.set("Parent",new_owner);
    }
    pdf.get_dictionary_mut(new).map_err(bad)?.set("Annots",copied);
    Ok(())
}
impl PdfEngine {
    pub fn replace_page_content_occurrence(
        &self,
        destination_id: &str,
        occurrences: &[usize],
        selected_occurrence: usize,
        incoming_id: &str,
        incoming_page: usize,
    ) -> EngineResult<PageContentReplacement> {
        if occurrences.is_empty()
            || occurrences.len() > MAX_PAGES
            || selected_occurrence >= occurrences.len()
        {
            return Err(bad("invalid page occurrence selection"));
        }
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::ReplacePageContentOccurrence {
            destination_id: destination_id.into(),
            occurrences: occurrences.to_vec(),
            selected_occurrence,
            incoming_id: incoming_id.into(),
            incoming_page,
            reply,
        })?;
        let mut result = receive.recv().map_err(|_| EngineError::WorkerStopped)??;
        result.document = self.document_in_points(Ok(result.document))?;
        Ok(result)
    }
}

impl WorkerRuntime {
    pub(super) fn append_page_copy(&self, source: &OpenDocument, page_index: usize) -> EngineResult<Vec<u8>> {
        crate::form_graph::preflight(source.bytes()?.as_ref())?;
        let mut original = crate::pdf_output::load(source.bytes()?.as_ref())?;
        let count = original.get_pages().len();
        if count >= MAX_PAGES || page_index >= count {
            return Err(bad("native page count exceeds limits"));
        }
        let original_page=*original.get_pages().values().nth(page_index).ok_or_else(||bad("source page missing"))?;
        let mut mapping: Vec<_> = (0..count).map(|i| (i, i)).collect();
        mapping.push((page_index, count));
        let source_bytes = source.bytes()?;
        let sources = [crate::acroforms::SourceForm::new(&source_bytes, mapping.clone())];
        crate::acroforms::preflight(&sources)?;
        let links = crate::bookmarks::links::prepare(&sources)?;

        // Load immutable bytes, including imported Folio additions. The live
        // renderer source has those annotations removed for workspace editing.
        let input = self
            .pdfium
            .load_pdf_from_reader(Cursor::new(source.bytes()?), None)?;
        let mut output = self.pdfium.create_new_pdf()?;
        output
            .pages_mut()
            .copy_page_from_document(&input, page_index_i32(page_index)?, 0)?;
        let bytes = crate::pdf_output::from_pdfium(&output)?;
        drop(output);
        drop(input);
        let mut copied = crate::pdf_output::load(&bytes)?;
        let start = original
            .max_id
            .checked_add(1)
            .ok_or_else(|| bad("object identifier limit"))?;
        let size =
            u32::try_from(copied.objects.len()).map_err(|_| bad("object identifier limit"))?;
        start
            .checked_add(size)
            .ok_or_else(|| bad("object identifier limit"))?;
        copied.renumber_objects_with(start);
        let new_page = *copied
            .get_pages()
            .values()
            .next()
            .ok_or_else(|| bad("copied page is missing"))?;
        let root = original
            .catalog()
            .map_err(bad)?
            .get(b"Pages")
            .map_err(bad)?
            .as_reference()
            .map_err(bad)?;
        let kids = original
            .get_dictionary(root)
            .map_err(bad)?
            .get(b"Kids")
            .map_err(bad)?;
        let mut kids = original
            .dereference(kids)
            .map_err(bad)?
            .1
            .as_array()
            .map_err(bad)?
            .clone();
        copied
            .get_dictionary_mut(new_page)
            .map_err(bad)?
            .set("Parent", root);
        original.max_id = original.max_id.max(copied.max_id);
        original.objects.extend(copied.objects);
        kids.push(Object::Reference(new_page));
        let tree = original.get_dictionary_mut(root).map_err(bad)?;
        tree.set("Kids", kids);
        tree.set("Count", (count + 1) as i64);

        // Original pages retain their IDs/catalog. Only the appended page needs
        // resource import repair; interaction repair binds the full shared graph.
        crate::page_resources::preserve(
            &mut original,
            &[crate::acroforms::SourceForm::new(source.bytes()?.as_ref(), vec![(page_index, count)])],
        )?;
        restore_appended_appearances(&mut original, original_page, new_page)?;
        let excluded = HashMap::new();
        native_comments::preserve_geometry(
            &mut original,
            &[native_comments::SourceComments {
                source: &sources[0],
                excluded: &excluded,
            }],
        )?;
        native_comments::repair_popup_parents(&mut original)?;
        crate::acroforms::preserve_retained_document(&mut original, &sources)?;
        links.apply(&mut original)?;
        crate::page_resources::prune_unreachable(&mut original)?;
        crate::pdf_output::save(&mut original)
    }

    pub(super) fn replace_page_content_occurrence(
        &mut self,
        destination_id: &str,
        occurrences: &[usize],
        selected_occurrence: usize,
        incoming_id: &str,
        incoming_page: usize,
    ) -> EngineResult<PageContentReplacement> {
        if occurrences.is_empty()
            || occurrences.len() > MAX_PAGES
            || selected_occurrence >= occurrences.len()
        {
            return Err(bad("invalid page occurrence selection"));
        }
        let target = self.editing_document(destination_id)?;
        let count = target.document.page_count() as usize;
        if occurrences.iter().any(|index| *index >= count) {
            return Err(bad("a page occurrence is outside its source"));
        }
        let selected_page = occurrences[selected_occurrence];
        if occurrences
            .iter()
            .filter(|index| **index == selected_page)
            .count()
            == 1
        {
            return Ok(PageContentReplacement {
                document: self.replace_page_content(
                    destination_id,
                    selected_page,
                    incoming_id,
                    incoming_page,
                )?,
                page_indices: occurrences.to_vec(),
            });
        }
        let policy = target
            .policy
            .as_ref()
            .map(|p| p.duplicated_page(selected_page))
            .transpose()?;
        let incoming = self.copying_document(incoming_id)?;
        if incoming.policy.is_some() {
            return Err(bad("incoming protected PDFs are not supported yet"));
        }
        if incoming
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
                .get(page_index_i32(selected_page)?)?,
        )?;
        let source_geometry = page_geometry(
            &incoming
                .pdf()?
                .pages()
                .get(page_index_i32(incoming_page)?)?,
        )?;
        let first = occurrences
            .iter()
            .position(|index| *index == selected_page)
            .unwrap();
        let replace_index = if first == selected_occurrence {
            selected_page
        } else {
            count
        };
        let page_indices = occurrences
            .iter()
            .enumerate()
            .map(|(i, p)| {
                if *p != selected_page {
                    *p
                } else if (i == selected_occurrence) == (first == selected_occurrence) {
                    selected_page
                } else {
                    count
                }
            })
            .collect();
        let expanded = self.append_page_copy(target, selected_page)?;
        let bytes = page_content_replace::replace(
            &expanded,
            replace_index,
            incoming.bytes()?.as_ref(),
            incoming_page,
            target_geometry,
            source_geometry,
        )?;
        let bookmarks = target.source_bookmarks()?
            .clone();
        let original_path = target.original_path.clone();
        let legacy = target.legacy_comment_import;
        let path =
            std::env::temp_dir().join(format!("folio-page-occurrence-{}.pdf", Uuid::new_v4()));
        let mut document = self.open_bytes(
            bytes,
            path,
            original_path,
            true,
            true,
            legacy,
            OpenSecurity::Restored(policy),
        )?;
        document = self.retain_annotation_sources_with_copy(destination_id, document, Some(selected_page))?;
        document.name = "Replaced page content.pdf".into();
        document.bookmarks = bookmarks.as_ref().cloned().unwrap_or_default();
        document.bookmark_error = bookmarks.as_ref().err().cloned();
        self.documents.get_mut(&document.id).unwrap().bookmarks = OnceLock::from(bookmarks);
        Ok(PageContentReplacement {
            document,
            page_indices,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn abandoned_occurrence_reply_releases_candidate_source_and_image_lease() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut runtime = WorkerRuntime::new(
            &root.join("resources/pdfium/pdfium.dll"),
            Arc::new(FontRegistry::new()),
        )
        .unwrap();
        let mut encoded = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            2,
            2,
            image::Rgba([19, 81, 130, 255]),
        ))
        .write_to(&mut encoded, ImageFormat::Png)
        .unwrap();
        let image = runtime.images.register(encoded.into_inner()).unwrap();
        let blank = runtime.create_blank_document(200., 300.).unwrap();
        let plan = PagePlan {
            crop: None,
            id: "p".into(),
            source_id: blank.id.clone(),
            page_index: 0,
            width: 200.,
            height: 300.,
            rotation: 0,
            overlays: vec![Overlay::Image(ImageOverlay {
                id: "retained-image".into(),
                asset_id: image.image.id.clone(),
                x: 20.,
                y: 20.,
                width: 60.,
                height: 60.,
                rotation: 0,
            })],
        };
        let bytes = runtime
            .compose_pdf(
                ExportRequest {
                    pages: vec![plan],
                    flatten: false,
                },
                Some(vec![]),
                CompositionIntent::Save,
            )
            .unwrap();
        let path =
            std::env::temp_dir().join(format!("folio-occurrence-test-{}.pdf", Uuid::new_v4()));
        let source = runtime
            .open_bytes(
                bytes,
                path.clone(),
                path,
                true,
                true,
                false,
                OpenSecurity::Restored(None),
            )
            .unwrap();
        let before = runtime.documents.len();
        let candidate = runtime
            .replace_page_content_occurrence(&source.id, &[0, 0], 1, &blank.id, 0)
            .unwrap();
        assert_eq!(candidate.document.pages.len(), 2);
        runtime.discard_unpublished_source(&candidate.document.id);
        assert_eq!(runtime.documents.len(), before);
        let (reply, receive) = mpsc::channel();
        drop(receive);
        let (send, queue) = mpsc::channel();
        send.send(WorkerRequest::ReplacePageContentOccurrence {
            destination_id: source.id.clone(),
            occurrences: vec![0, 0],
            selected_occurrence: 1,
            incoming_id: blank.id.clone(),
            incoming_page: 0,
            reply,
        })
        .unwrap();
        drop(send);
        runtime.run(queue);
        assert_eq!(
            runtime.documents.len(),
            before,
            "abandoned candidate leaked"
        );
        runtime.documents.remove(&source.id);
        runtime.documents.remove(&blank.id);
        runtime.images.release(&image.token).unwrap();
        assert!(
            runtime.images.get(&image.image.id).is_err(),
            "abandoned candidate retained an image"
        );
    }
}
