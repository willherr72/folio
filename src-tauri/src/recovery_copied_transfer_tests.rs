use super::*;

fn fixture(path: &Path, image: bool) {
    form_fixture(path, false);
    if image {
        image_fixture(path);
    }
    let mut pdf = Document::load(path).unwrap();
    let page = pdf.get_pages()[&1];
    let mut annots = pdf
        .get_dictionary(page)
        .unwrap()
        .get(b"Annots")
        .unwrap()
        .as_array()
        .unwrap()
        .clone();
    let owner = annots[0].as_reference().unwrap();
    annots[1] = pdf
        .get_dictionary(annots[1].as_reference().unwrap())
        .unwrap()
        .clone()
        .into();
    let popup = pdf.add_object(dictionary! {"Type"=>"Annot","Subtype"=>"Popup","P"=>page,
    "Parent"=>owner,"Rect"=>vec![180.into(),280.into(),280.into(),350.into()]});
    pdf.get_dictionary_mut(owner).unwrap().set("Popup", popup);
    annots.push(popup.into());
    pdf.get_dictionary_mut(page).unwrap().set("Annots", annots);
    fs::write(path, crate::pdf_output::save(&mut pdf).unwrap()).unwrap();
}

fn check(kind: &str) {
    for selected in [0, 1] {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("source.pdf");
        fixture(&path, kind == "image");
        let forward_only = matches!(kind, "text-forward-popup" | "text-direct-popup");
        if kind.starts_with("text-") {
            let mut pdf = Document::load(&path).unwrap();
            let page = pdf.get_pages()[&1];
            let mut annots = pdf
                .get_dictionary(page)
                .unwrap()
                .get(b"Annots")
                .unwrap()
                .as_array()
                .unwrap()
                .clone();
            let popup = annots[4].as_reference().unwrap();
            if forward_only {
                pdf.get_dictionary_mut(popup).unwrap().remove(b"Parent");
            }
            if kind == "text-null-parent" {
                pdf.get_dictionary_mut(popup)
                    .unwrap()
                    .set("Parent", lopdf::Object::Null);
            }
            let owner = annots[0].as_reference().unwrap();
            if kind == "text-null-forward-popup" {
                pdf.get_dictionary_mut(owner)
                    .unwrap()
                    .set("Popup", lopdf::Object::Null);
            }
            if kind == "text-backlink-popup" {
                pdf.get_dictionary_mut(owner).unwrap().remove(b"Popup");
            }
            if kind == "text-direct-popup" {
                annots[0] = pdf
                    .get_dictionary(annots[0].as_reference().unwrap())
                    .unwrap()
                    .clone()
                    .into();
                pdf.get_dictionary_mut(page).unwrap().set("Annots", annots);
            }
            fs::write(&path, crate::pdf_output::save(&mut pdf).unwrap()).unwrap();
        }
        let original_bytes = fs::read(&path).unwrap();
        let engine = PdfEngine::start(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
        )
        .unwrap();
        let source = engine.open_document(&path).unwrap();
        let before = engine.render_page(&source.id, 0, 450).unwrap();
        let ownership =
            serde_json::to_value(engine.source_imported_overlay_snapshot(&source.id).unwrap())
                .unwrap();
        assert_eq!(source.pages[0].overlays.len(), 2);
        assert_eq!(ownership["bindings"][0]["popupIndices"], json!([4]));
        let changed = match kind {
            kind if kind.starts_with("text") => {
                let run = engine
                    .list_occurrence_text_runs(&source.id, 0)
                    .unwrap()
                    .runs
                    .into_iter()
                    .find(|r| r.text == "Contact team")
                    .unwrap();
                engine
                    .replace_text_occurrence(
                        &source.id,
                        &[0, 0],
                        selected,
                        run.object_index,
                        &run.text,
                        "Contact this team",
                        None,
                    )
                    .unwrap()
            }
            "position" => {
                let field = engine
                    .list_form_fields(&source.id, 0)
                    .unwrap()
                    .into_iter()
                    .find(|f| f.name == "answer")
                    .unwrap();
                let mut bounds = field.bounds.clone().unwrap();
                bounds.x += 17.;
                engine.position_form_field_occurrence(&source.id,&[0,0],selected,serde_json::from_value(json!({
                    "annotationIndex":field.annotation_index,"expectedBounds":field.bounds,"expectedValue":field.value,
                    "bounds":bounds,"reflowText":false})).unwrap()).unwrap()
            }
            "image" => {
                let image = engine
                    .list_existing_images(&source.id, 0)
                    .unwrap()
                    .images
                    .remove(0);
                engine
                    .preview_image_edit_occurrence(
                        &source.id,
                        &[0, 0],
                        selected,
                        &image.object_path,
                        &image.selection_id,
                        crate::ExistingImageEdit::Transform {
                            transform: crate::ImageTransform {
                                delta_x: 8.5,
                                delta_y: 0.,
                                scale_x: 0.9,
                                scale_y: 1.,
                                rotation_degrees: 0.,
                            },
                        },
                    )
                    .unwrap()
            }
            "page" => {
                let incoming = engine.create_blank_document(300., 400.).unwrap();
                let result = engine
                    .replace_page_content_occurrence(&source.id, &[0, 0], selected, &incoming.id, 0)
                    .unwrap();
                engine.close_document(&incoming.id).unwrap();
                result
            }
            _ => unreachable!(),
        };
        assert_eq!(changed.page_indices, vec![0, 1]);
        let edited = changed.document;
        let next =
            serde_json::to_value(engine.source_imported_overlay_snapshot(&edited.id).unwrap())
                .unwrap();
        if let Some(root) = std::env::var_os("FOLIO_STRUCTURED_OVERLAY_CAPTURE_DIR") {
            let dir = PathBuf::from(root).join(format!("copy-{kind}-{selected}"));
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("original.pdf"), &original_bytes).unwrap();
            fs::write(
                dir.join("edited.pdf"),
                engine.source_bytes(&edited.id).unwrap(),
            )
            .unwrap();
            fs::write(
                dir.join("bindings.json"),
                serde_json::to_vec_pretty(&json!({"old":ownership,"new":next})).unwrap(),
            )
            .unwrap();
        }
        let published = Document::load_mem(&engine.source_bytes(&edited.id).unwrap()).unwrap();
        for page in published.get_pages().values() {
            let annots = published
                .get_dictionary(*page)
                .unwrap()
                .get(b"Annots")
                .unwrap()
                .as_array()
                .unwrap();
            let popup = annots[4].as_reference().unwrap();
            if forward_only {
                assert!(
                    !published.get_dictionary(popup).unwrap().has(b"Parent"),
                    "missing optional backlink must stay absent"
                );
                continue;
            }
            if kind == "text-null-parent" {
                assert!(matches!(
                    published
                        .get_dictionary(popup)
                        .unwrap()
                        .get(b"Parent")
                        .unwrap(),
                    lopdf::Object::Null
                ));
                continue;
            }
            let owner = annots[0].as_reference().unwrap();
            if kind == "text-null-forward-popup" {
                assert!(matches!(
                    published
                        .get_dictionary(owner)
                        .unwrap()
                        .get(b"Popup")
                        .unwrap(),
                    lopdf::Object::Null
                ));
            }
            if kind == "text-backlink-popup" {
                assert!(!published.get_dictionary(owner).unwrap().has(b"Popup"));
            }
            assert_eq!(
                published
                    .get_dictionary(popup)
                    .unwrap()
                    .get(b"Parent")
                    .unwrap()
                    .as_reference()
                    .unwrap(),
                owner,
                "copied popup must refer to its actual page annotation"
            );
        }
        assert_eq!(
            next["bindings"].as_array().map(Vec::len),
            Some(4),
            "{kind}/{selected}: copied ownership must be proved"
        );
        for page in 0..2 {
            assert_eq!(
                serde_json::to_value(&edited.pages[page].overlays).unwrap(),
                serde_json::to_value(&source.pages[0].overlays).unwrap()
            );
            for (i, old) in ownership["bindings"].as_array().unwrap().iter().enumerate() {
                let binding = &next["bindings"][page * 2 + i];
                for key in [
                    "annotationIndex",
                    "popupIndices",
                    "overlayId",
                    "initialSemanticsSha256",
                ] {
                    assert_eq!(binding[key], old[key], "{kind}/{selected}/{page}/{key}");
                }
                assert_eq!(binding["pageIndex"], page);
                if page == 0 {
                    assert_eq!(binding["annotationId"], old["annotationId"]);
                }
            }
        }
        assert_eq!(
            engine.render_page(&edited.id, 1 - selected, 450).unwrap(),
            before,
            "unchanged copy pixels"
        );
        assert_ne!(
            engine.render_page(&edited.id, selected, 450).unwrap(),
            before,
            "selected copy must change"
        );
        let mut pages = vec![];
        for page in 0..2 {
            let mut drafts = serde_json::to_value(&source.pages[0].overlays).unwrap();
            if page == selected {
                drafts.as_array_mut().unwrap().pop();
                drafts[0]["text"] = json!("Only this copy has a changed draft");
            }
            pages.push(json!({"id":format!("page-{page}"),"sourceId":edited.id,"pageIndex":page,
                "width":edited.pages[page].width,"height":edited.pages[page].height,"rotation":0,"overlays":drafts}));
        }
        let workspace = json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","dirty":true,"savedDigest":"","zoom":100,
            "scrollPosition":{"top":0,"left":0},"document":{"name":"Copies.pdf","pages":pages,"selectedPageId":"page-0","selectedOverlayId":null}}]});
        let store = RecoveryStore::new(temp.path().join("recovery")).unwrap();
        let native_bytes = engine.source_bytes(&edited.id).unwrap();
        store.save(&engine, workspace.clone()).unwrap();
        engine.close_document(&edited.id).unwrap();
        let restored = store.load(&engine).unwrap().unwrap();
        assert_eq!(
            engine.source_bytes(source_id(&restored)).unwrap(),
            native_bytes
        );
        assert_eq!(
            serde_json::to_value(
                engine
                    .source_imported_overlay_snapshot(source_id(&restored))
                    .unwrap()
            )
            .unwrap(),
            next
        );
        for page in 0..2 {
            assert_eq!(
                restored["tabs"][0]["document"]["pages"][page]["overlays"],
                workspace["tabs"][0]["document"]["pages"][page]["overlays"]
            );
        }
        assert_eq!(fs::read(&path).unwrap(), original_bytes);
        assert_eq!(engine.render_page(&source.id, 0, 450).unwrap(), before);
        engine.close_document(source_id(&restored)).unwrap();
        engine.close_document(&source.id).unwrap();
    }
}
#[test]
fn recovery_annotation_sources_copied_text_edits_keep_each_annotation_owner() {
    check("text");
}
#[test]
fn recovery_annotation_sources_copied_field_edits_keep_each_annotation_owner() {
    check("position");
}
#[test]
fn recovery_annotation_sources_copied_image_edits_keep_each_annotation_owner() {
    check("image");
}
#[test]
fn recovery_annotation_sources_copied_page_replacement_keeps_each_annotation_owner() {
    check("page");
}

#[test]
fn recovery_annotation_sources_copied_forward_popup_keeps_optional_backlink_absent() {
    check("text-forward-popup");
}
#[test]
fn recovery_annotation_sources_copied_direct_popup_owner_keeps_optional_backlink_absent() {
    check("text-direct-popup");
}

#[test]
fn recovery_annotation_sources_copied_null_parent_stays_null() {
    check("text-null-parent");
}
#[test]
fn recovery_annotation_sources_copied_null_forward_link_stays_null() {
    check("text-null-forward-popup");
}
#[test]
fn recovery_annotation_sources_copied_backlink_only_popup_preserves_owner() {
    check("text-backlink-popup");
}
