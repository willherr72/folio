use super::*;
use lopdf::{dictionary, Document, Object, Stream};
fn runtime() -> WorkerRuntime {
    WorkerRuntime::new(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
        Arc::new(FontRegistry::new()),
    )
    .unwrap()
}
fn source(rt: &mut WorkerRuntime) -> DocumentInfo {
    let mut d = Document::with_version("1.7");
    let tree = d.new_object_id();
    let font=d.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica","Encoding"=>"WinAnsiEncoding"});
    let mut kids = vec![];
    for content in [
        "BT /F1 12 Tf 20 240 Td (SECRET first) Tj ET",
        "BT /F1 12 Tf 20 240 Td (Second page stays) Tj ET",
    ] {
        let content = d.add_object(Stream::new(dictionary! {}, content.as_bytes().to_vec()));
        let page=d.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),500.into(),300.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});
        kids.push(Object::Reference(page));
    }
    d.objects.insert(
        tree,
        dictionary! {"Type"=>"Pages","Kids"=>kids,"Count"=>2}.into(),
    );
    let catalog = d.add_object(dictionary! {"Type"=>"Catalog","Pages"=>tree});
    d.trailer.set("Root", catalog);
    let mut bytes = vec![];
    d.save_to(&mut bytes).unwrap();
    rt.register_generated_document(bytes, "Source.pdf").unwrap()
}
fn page(source: &str, index: usize, id: &str) -> PagePlan {
    PagePlan {
        id: id.into(),
        source_id: source.into(),
        page_index: index,
        width: 500.,
        height: 300.,
        rotation: 0,
        crop: None,
        overlays: vec![],
    }
}
fn text(rt: &WorkerRuntime, id: &str, page: usize) -> String {
    rt.document(id)
        .unwrap()
        .pdf().unwrap()
        .pages()
        .get(page as i32)
        .unwrap()
        .text()
        .unwrap()
        .all()
}
fn overlay() -> Overlay {
    Overlay::Text(TextOverlay {
        id: "draft-text".into(),
        x: 30.,
        y: 140.,
        text: "Unsaved overlay".into(),
        font_size: 14.,
        font_name: TextFont::Helvetica,
        font_id: None,
        shaping: None,
        area: None,
        color: "#142536".into(),
        rotation: 0,
    })
}
#[test]
fn redaction_composition_prepares_ordered_independent_occurrences_without_changing_source() {
    let mut rt = runtime();
    let original = source(&mut rt);
    let owner = rt.begin_redaction_session().unwrap();
    let bytes = rt.document(&original.id).unwrap().bytes().unwrap();
    let pages = vec![
        page(&original.id, 1, "second"),
        page(&original.id, 0, "first"),
        page(&original.id, 0, "duplicate"),
    ];
    let result = rt.prepare_redaction_source(
        &owner,
        ExportRequest {
            pages,
            flatten: false,
        },
        None,
    );
    assert!(
        result.is_ok(),
        "edited snapshot preparation must succeed: {result:?}"
    );
    let prepared = result.unwrap();
    assert_eq!(prepared.document.pages.len(), 3);
    assert!(text(&rt, &prepared.document.id, 0).contains("Second page stays"));
    assert!(text(&rt, &prepared.document.id, 1).contains("SECRET first"));
    assert!(text(&rt, &prepared.document.id, 2).contains("SECRET first"));
    assert_eq!(&*bytes, &*rt.document(&original.id).unwrap().bytes().unwrap());
}
#[test]
fn redaction_composition_includes_unsaved_text_in_the_working_copy() {
    let mut rt = runtime();
    let original = source(&mut rt);
    let owner = rt.begin_redaction_session().unwrap();
    let mut plan = page(&original.id, 0, "edited");
    plan.overlays.push(overlay());
    let expected = rt.render_page_plan(plan.clone(), 1000).unwrap();
    let result = rt.prepare_redaction_source(
        &owner,
        ExportRequest {
            pages: vec![plan],
            flatten: false,
        },
        Some(vec![]),
    );
    assert!(
        result.is_ok(),
        "edited snapshot must include draft text: {result:?}"
    );
    let prepared = result.unwrap();
    let parsed =
        Document::load_mem(rt.document(&prepared.document.id).unwrap().bytes().unwrap().as_ref()).unwrap();
    let annotations = parsed.get_page_annotations(parsed.get_pages()[&1]).unwrap();
    assert!(annotations.iter().any(|a| a
        .get(b"Contents")
        .ok()
        .and_then(|v| lopdf::decode_text_string(v).ok())
        .as_deref()
        == Some("Unsaved overlay")));
    assert!(!text(&rt, &original.id, 0).contains("Unsaved overlay"));
    assert_eq!(
        rt.render_page(&prepared.document.id, 0, 1000).unwrap(),
        expected
    );
    assert_ne!(expected, rt.render_page(&original.id, 0, 1000).unwrap());
    // Editable overlay annotations are preserved, not flattened for admission.
    assert!(rt
        .preview_redaction(&owner, &prepared.document.id, &mark(0))
        .is_err());
    assert!(rt.document(&prepared.document.id).is_ok());
}

fn request(id: &str) -> ExportRequest {
    ExportRequest {
        pages: vec![page(id, 0, "first"), page(id, 1, "second")],
        flatten: false,
    }
}
fn mark(page: usize) -> Vec<TextRedactionMark> {
    vec![TextRedactionMark {
        page_index: page,
        rect: TextMarkRect {
            x: 19.,
            y: 45.,
            width: 49.,
            height: 20.,
        },
    }]
}
fn variant(
    rt: &mut WorkerRuntime,
    original: &str,
    edit: impl FnOnce(&mut Document),
) -> DocumentInfo {
    let mut d = Document::load_mem(rt.document(original).unwrap().bytes().unwrap().as_ref()).unwrap();
    edit(&mut d);
    let mut bytes = vec![];
    d.save_to(&mut bytes).unwrap();
    rt.register_generated_document(bytes, "Variant.pdf")
        .unwrap()
}

#[test]
fn redaction_composition_removes_only_the_selected_duplicate_and_keeps_applied_copy() {
    let mut rt = runtime();
    let original = source(&mut rt);
    let owner = rt.begin_redaction_session().unwrap();
    let prepared = rt
        .prepare_redaction_source(
            &owner,
            ExportRequest {
                pages: vec![page(&original.id, 0, "one"), page(&original.id, 0, "two")],
                flatten: false,
            },
            None,
        )
        .unwrap();
    let p = rt
        .preview_redaction(&owner, &prepared.document.id, &mark(1))
        .unwrap();
    assert_eq!(p.removed_characters, 6);
    assert!(text(&rt, &p.candidate.id, 0).contains("SECRET"));
    assert!(!text(&rt, &p.candidate.id, 1).contains("SECRET"));
    let applied = rt.apply_redaction(&owner, &p.preview_id).unwrap();
    rt.release_redaction_owner(&owner);
    assert!(rt.document(&prepared.document.id).is_err());
    assert!(text(&rt, &applied.id, 0).contains("SECRET"));
    assert!(text(&rt, &original.id, 0).contains("SECRET"));
}

#[test]
fn redaction_composition_omissions_are_reported_before_loss_and_carried_to_preview() {
    let mut rt = runtime();
    let plain = source(&mut rt);
    let original = variant(&mut rt, &plain.id, |d| {
        let info = d.add_object(dictionary! {"Title"=>Object::string_literal("Private title")});
        d.trailer.set("Info", info);
        let payload = d.add_object(Stream::new(
            dictionary! {"Type"=>"EmbeddedFile"},
            b"Private attachment".to_vec(),
        ));
        let file=d.add_object(dictionary!{"Type"=>"Filespec","F"=>Object::string_literal("secret.txt"),"EF"=>dictionary!{"F"=>payload}});
        d.catalog_mut().unwrap().set("Names",dictionary!{"EmbeddedFiles"=>dictionary!{"Names"=>vec![Object::string_literal("secret.txt"),file.into()]}});
    });
    let owner = rt.begin_redaction_session().unwrap();
    let retained = rt
        .prepare_redaction_source(&owner, request(&original.id), None)
        .unwrap();
    assert!(retained.omitted_metadata.is_empty());
    assert_eq!(
        &*rt.document(&retained.document.id).unwrap().bytes().unwrap(),
        &*rt.document(&original.id).unwrap().bytes().unwrap()
    );
    let reversed = ExportRequest {
        pages: vec![page(&original.id, 1, "two"), page(&original.id, 0, "one")],
        flatten: false,
    };
    let composed = rt
        .prepare_redaction_source(&owner, reversed, Some(vec![]))
        .unwrap();
    assert_eq!(composed.omitted_metadata.len(), 2);
    let parsed =
        Document::load_mem(rt.document(&composed.document.id).unwrap().bytes().unwrap().as_ref()).unwrap();
    assert!(!parsed.catalog().unwrap().has(b"Names"));
    let preview = rt
        .preview_redaction(&owner, &composed.document.id, &mark(1))
        .unwrap();
    assert!(composed
        .omitted_metadata
        .iter()
        .all(|v| preview.omitted_metadata.contains(v)));
    let original_pdf =
        Document::load_mem(rt.document(&original.id).unwrap().bytes().unwrap().as_ref()).unwrap();
    assert!(original_pdf.catalog().unwrap().has(b"Names"));
    assert!(original_pdf.trailer.has(b"Info"));
}

#[test]
fn redaction_composition_tracks_inputs_and_refuses_cross_owner_or_nested_preparations() {
    let mut rt = runtime();
    let original = source(&mut rt);
    let owner = rt.begin_redaction_session().unwrap();
    let other = rt.begin_redaction_session().unwrap();
    let prepared = rt
        .prepare_redaction_source(&owner, request(&original.id), None)
        .unwrap();
    assert!(rt
        .preview_redaction(&other, &prepared.document.id, &mark(0))
        .is_err());
    assert!(rt
        .prepare_redaction_source(&owner, request(&prepared.document.id), None)
        .is_err());
    let p = rt
        .preview_redaction(&owner, &prepared.document.id, &mark(0))
        .unwrap();
    assert!(rt
        .prepare_redaction_source(&owner, request(&p.candidate.id), None)
        .is_err());
    rt.documents.get_mut(&original.id).unwrap().for_printing = true;
    assert!(rt.apply_redaction(&owner, &p.preview_id).is_err());
    assert!(rt.document(&p.candidate.id).is_err());
    rt.documents.get_mut(&original.id).unwrap().for_printing = false;
    let bytes = rt.document(&original.id).unwrap().bytes().unwrap().to_vec();
    rt.documents.get_mut(&original.id).unwrap().source_bytes = crate::source_storage::SourceStore::new().capture(bytes.into()).unwrap();
    assert!(rt
        .preview_redaction(&owner, &prepared.document.id, &mark(0))
        .is_err());
    rt.release_redaction_owner(&owner);
    assert!(rt.document(&original.id).is_ok());
}

#[test]
fn redaction_composition_input_close_and_expiry_clean_dependents_without_touching_other_sources() {
    for expire in [false, true] {
        let mut rt = runtime();
        let original = source(&mut rt);
        let unrelated = source(&mut rt);
        let owner = rt.begin_redaction_session().unwrap();
        let prepared = rt
            .prepare_redaction_source(&owner, request(&original.id), None)
            .unwrap();
        let p = rt
            .preview_redaction(&owner, &prepared.document.id, &mark(0))
            .unwrap();
        if expire {
            rt.redaction_previews
                .prepared
                .get_mut(&prepared.document.id)
                .unwrap()
                .created = Instant::now() - TTL;
            rt.prune_redaction_previews();
        } else {
            rt.close_document(&original.id).unwrap();
        }
        assert!(rt.document(&prepared.document.id).is_err());
        assert!(rt.document(&p.candidate.id).is_err());
        assert!(rt.redaction_previews.prepared.is_empty());
        assert!(rt.redaction_previews.pending.is_empty());
        assert!(rt.document(&unrelated.id).is_ok());
        assert_eq!(rt.document(&original.id).is_ok(), expire);
    }
}

#[test]
fn redaction_composition_cleanup_checks_owned_allocation_and_shared_capacity() {
    let mut rt = runtime();
    let original = source(&mut rt);
    let unrelated = source(&mut rt);
    let owner = rt.begin_redaction_session().unwrap();
    let prepared = rt
        .prepare_redaction_source(&owner, request(&original.id), None)
        .unwrap();
    let used = rt.retained_redaction_bytes();
    rt.redaction_previews.byte_limit = used + 1;
    assert!(rt
        .prepare_redaction_source(&owner, request(&original.id), None)
        .is_err());
    assert!(rt
        .preview_redaction(&owner, &prepared.document.id, &mark(0))
        .is_err());
    assert_eq!(rt.documents.len(), 3);
    assert_eq!(rt.redaction_previews.prepared.len(), 1);
    rt.redaction_previews.byte_limit = 128 * 1024 * 1024;
    let foreign = rt.documents.remove(&unrelated.id).unwrap();
    let foreign_allocation = foreign.bytes().unwrap();
    rt.documents.insert(prepared.document.id.clone(), foreign);
    assert!(rt
        .preview_redaction(&owner, &prepared.document.id, &mark(0))
        .is_err());
    rt.release_redaction_owner(&owner);
    assert!(Arc::ptr_eq(
        &rt.document(&prepared.document.id).unwrap().bytes().unwrap(),
        &foreign_allocation
    ));
    let owner = rt.begin_redaction_session().unwrap();
    for _ in 0..4 {
        rt.prepare_redaction_source(&owner, request(&original.id), None)
            .unwrap();
    }
    assert!(rt
        .prepare_redaction_source(&owner, request(&original.id), None)
        .is_err());
    rt.release_redaction_owner(&owner);
    assert!(rt.redaction_previews.prepared.is_empty());
}

#[test]
fn redaction_composition_geometry_and_editable_annotations_match_page_plan_rendering() {
    let mut rt = runtime();
    let original = source(&mut rt);
    let owner = rt.begin_redaction_session().unwrap();
    for (index, rotation) in [0, 90, 180, 270].into_iter().enumerate() {
        let mut p = page(&original.id, 0, &format!("page-{index}"));
        p.rotation = rotation;
        p.crop = Some(AnnotationRect {
            x: 10.,
            y: 20.,
            width: 240.,
            height: 200.,
        });
        p.overlays.push(overlay());
        p.overlays.push(Overlay::Ink(InkOverlay {
            id: "ink".into(),
            paths: vec![vec![Point { x: 50., y: 90. }, Point { x: 120., y: 110. }]],
            color: "#2f8622".into(),
            stroke_width: 2.,
        }));
        p.overlays.push(Overlay::Highlight(HighlightOverlay {
            id: "highlight".into(),
            rects: vec![AnnotationRect {
                x: 18.,
                y: 46.,
                width: 100.,
                height: 20.,
            }],
            color: "#ffff00".into(),
            text: None,
            opacity: Some(0.35),
        }));
        p.overlays.push(Overlay::Comment(CommentOverlay {
            id: "comment".into(),
            x: 180.,
            y: 90.,
            text: "Draft note".into(),
            color: "#cd3311".into(),
        }));
        let expected = rt.render_page_plan(p.clone(), 800).unwrap();
        let prepared = rt
            .prepare_redaction_source(
                &owner,
                ExportRequest {
                    pages: vec![p],
                    flatten: false,
                },
                Some(vec![]),
            )
            .unwrap();
        let size = &prepared.document.pages[0];
        assert_eq!(
            (size.width, size.height),
            if rotation % 180 == 0 {
                (240., 200.)
            } else {
                (200., 240.)
            }
        );
        assert_eq!(
            rt.render_page(&prepared.document.id, 0, 800).unwrap(),
            expected
        );
        let d =
            Document::load_mem(rt.document(&prepared.document.id).unwrap().bytes().unwrap().as_ref()).unwrap();
        assert!(d.get_page_annotations(d.get_pages()[&1]).unwrap().len() >= 4);
        assert!(rt
            .preview_redaction(&owner, &prepared.document.id, &mark(0))
            .is_err());
    }
    rt.release_redaction_owner(&owner);
    assert_eq!(rt.documents.len(), 1);
}

fn form_source(rt: &mut WorkerRuntime, original: &str) -> DocumentInfo {
    variant(rt, original, |d| {
        let first = d.get_pages()[&1];
        let font = d
            .get_dictionary(first)
            .unwrap()
            .get(b"Resources")
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"Font")
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"F1")
            .unwrap()
            .clone();
        let appearance=d.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),160.into(),26.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font.clone()}}},b"q 0.95 g 0 0 160 26 re f 0 g BT /F1 12 Tf 4 8 Td (Saved answer) Tj ET Q".to_vec()));
        let widget=d.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Tx","T"=>Object::string_literal("answer"),"V"=>Object::string_literal("Saved answer"),"Rect"=>vec![20.into(),100.into(),180.into(),126.into()],"F"=>4,"P"=>first,"DA"=>Object::string_literal("/F1 12 Tf 0 g"),"AP"=>dictionary!{"N"=>appearance}});
        d.get_object_mut(first)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("Annots", vec![Object::Reference(widget)]);
        d.catalog_mut().unwrap().set("AcroForm",dictionary!{"Fields"=>vec![Object::Reference(widget)],"DR"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"NeedAppearances"=>false});
    })
}
fn bookmark() -> crate::DocumentBookmark {
    serde_json::from_value(serde_json::json!({"id":"answer-bookmark","title":"Answer section","level":0,"destination":{"kind":"array","value":[{"kind":"page","value":"field"},{"kind":"name","value":[70,105,116]}]}})).unwrap()
}
#[test]
fn redaction_composition_keeps_forms_answers_and_explicit_bookmark_targets() {
    let mut rt = runtime();
    let plain = source(&mut rt);
    let original = form_source(&mut rt, &plain.id);
    let owner = rt.begin_redaction_session().unwrap();
    let plans = vec![
        page(&original.id, 1, "other"),
        page(&original.id, 0, "field"),
    ];
    let expected = rt.render_page_plan(plans[1].clone(), 800).unwrap();
    let prepared = rt
        .prepare_redaction_source(
            &owner,
            ExportRequest {
                pages: plans,
                flatten: false,
            },
            Some(vec![bookmark()]),
        )
        .unwrap();
    assert_eq!(
        rt.render_page(&prepared.document.id, 1, 800).unwrap(),
        expected
    );
    assert_eq!(prepared.document.bookmarks.len(), 1);
    assert_eq!(prepared.document.bookmarks[0].title, "Answer section");
    assert_eq!(
        prepared.document.bookmarks[0].destination,
        Some(crate::bookmarks::PdfValue::Array(vec![
            crate::bookmarks::PdfValue::Page("1".into()),
            crate::bookmarks::PdfValue::Name(b"Fit".to_vec())
        ]))
    );
    let d = Document::load_mem(rt.document(&prepared.document.id).unwrap().bytes().unwrap().as_ref()).unwrap();
    let annots = d.get_page_annotations(d.get_pages()[&2]).unwrap();
    let widget = annots
        .iter()
        .find(|a| a.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Widget"))
        .unwrap();
    // Save separates the terminal field from its widget. The answer belongs to
    // that field, whose Kids must still point back to this page's annotation.
    let parent_id = widget.get(b"Parent").unwrap().as_reference().unwrap();
    let parent = d.get_object(parent_id).unwrap().as_dict().unwrap();
    assert_eq!(lopdf::decode_text_string(parent.get(b"V").unwrap()).unwrap(), "Saved answer");
    assert_eq!(lopdf::decode_text_string(parent.get(b"T").unwrap()).unwrap(), "answer");
    let kid = parent.get(b"Kids").unwrap().as_array().unwrap();
    assert_eq!(kid.len(), 1);
    assert_eq!(d.get_object(kid[0].as_reference().unwrap()).unwrap().as_dict().unwrap(), *widget);
    assert!(d.catalog().unwrap().has(b"AcroForm"));
    assert!(rt
        .preview_redaction(&owner, &prepared.document.id, &mark(1))
        .is_err());
    assert_eq!(
        rt.render_page(&prepared.document.id, 1, 800).unwrap(),
        expected
    );
}

#[test]
fn redaction_composition_captured_fonts_images_survive_caller_release_and_failed_replies() {
    let engine = PdfEngine::start(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )
    .unwrap();
    let mut rt = WorkerRuntime::new(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
        engine.fonts(),
    )
    .unwrap();
    rt.images = engine.images();
    let original = source(&mut rt);
    let owner = rt.begin_redaction_session().unwrap();
    let font = engine
        .fonts()
        .register(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec())
        .unwrap();
    let mut png = Cursor::new(vec![]);
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        3,
        2,
        image::Rgba([17, 91, 173, 255]),
    ))
    .write_to(&mut png, ImageFormat::Png)
    .unwrap();
    let image = engine.images().register(png.into_inner()).unwrap();
    let mut p = page(&original.id, 0, "resources");
    let mut custom = overlay();
    if let Overlay::Text(ref mut t) = custom {
        t.font_id = Some(font.id.clone());
    }
    p.overlays.push(custom);
    p.overlays.push(Overlay::Image(ImageOverlay {
        id: "image".into(),
        asset_id: image.image.id.clone(),
        x: 200.,
        y: 120.,
        width: 70.,
        height: 50.,
        rotation: 0,
    }));
    let expected = rt.render_page_plan(p.clone(), 800).unwrap();
    let captured_request = ExportRequest {
        pages: vec![p],
        flatten: false,
    };
    let resources = engine
        .capture_export_resources(&captured_request.pages)
        .unwrap();
    engine.fonts().remove(&font.id).unwrap();
    engine.images().release(&image.token).unwrap();
    assert!(engine.fonts().get(&font.id).is_err());
    assert!(engine.images().get(&image.image.id).is_ok());
    let (reply, receive) = mpsc::channel();
    rt.handle_text_redaction(Request::Prepare {
        owner: owner.clone(),
        request: captured_request,
        bookmarks: Some(vec![]),
        resources,
        reply,
    });
    let prepared = receive.recv().unwrap().unwrap();
    assert!(Arc::ptr_eq(&rt.fonts, &engine.fonts()));
    assert!(engine.images().get(&image.image.id).is_err());
    assert_eq!(
        rt.render_page(&prepared.document.id, 0, 800).unwrap(),
        expected
    );
    let before = rt.documents.len();
    let request = request(&original.id);
    let resources = engine.capture_export_resources(&request.pages).unwrap();
    let (reply, receive) = mpsc::channel();
    drop(receive);
    rt.handle_text_redaction(Request::Prepare {
        owner: owner.clone(),
        request,
        bookmarks: None,
        resources,
        reply,
    });
    assert_eq!(rt.documents.len(), before);
    assert_eq!(rt.redaction_previews.prepared.len(), 1);
    rt.release_redaction_owner(&owner);
    assert_eq!(rt.documents.len(), 1);
}

#[test]
fn redaction_composition_facade_preserves_physical_units_and_rejects_invalid_requests() {
    let mut rt = runtime();
    let plain = source(&mut rt);
    let units = variant(&mut rt, &plain.id, |d| {
        for id in d.get_pages().values() {
            d.get_object_mut(*id)
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set("UserUnit", 1.7f32);
        }
    });
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("units.pdf");
    std::fs::write(&path, rt.document(&units.id).unwrap().bytes().unwrap().as_ref()).unwrap();
    let engine = PdfEngine::start(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )
    .unwrap();
    let original = engine.open_document(&path).unwrap();
    let owner = engine.begin_text_redaction_session().unwrap();
    let mut input = request(&original.id);
    for p in &mut input.pages {
        p.width = 850.;
        p.height = 510.;
    }
    let prepared = engine
        .prepare_text_redaction_source(&owner, input.clone(), None)
        .unwrap();
    assert!((prepared.document.pages[0].width - 850.).abs() < 0.001);
    assert!((prepared.document.pages[0].height - 510.).abs() < 0.001);
    assert_eq!(
        engine.render_page(&prepared.document.id, 0, 800).unwrap(),
        engine.render_page(&original.id, 0, 800).unwrap()
    );
    input.flatten = true;
    assert!(engine
        .prepare_text_redaction_source(&owner, input, None)
        .is_err());
    assert!(engine
        .prepare_text_redaction_source(
            &owner,
            ExportRequest {
                pages: vec![page(&original.id, 0, "too-many"); 1001],
                flatten: false
            },
            None
        )
        .is_err());
    let mut p = page(&original.id, 0, "too-many-overlays");
    p.overlays = vec![overlay(); 10001];
    assert!(engine
        .prepare_text_redaction_source(
            &owner,
            ExportRequest {
                pages: vec![p],
                flatten: false
            },
            None
        )
        .is_err());
    engine.release_text_redaction_owner(&owner).unwrap();
    assert!(engine.page_text(&prepared.document.id, 0).is_err());
    assert!(engine.page_text(&original.id, 0).is_ok());
}

#[test]
fn redaction_composition_exports_owned_snapshot_corpus() {
    let Some(root) = std::env::var_os("FOLIO_REDACTION_COMPOSITION_FIXTURES") else {
        return;
    };
    let root = PathBuf::from(root);
    std::fs::create_dir(&root).unwrap();
    let mut rt = runtime();
    let original = source(&mut rt);
    let owner = rt.begin_redaction_session().unwrap();
    let write = |rt: &WorkerRuntime, name: &str, id: &str| {
        std::fs::write(root.join(name), rt.document(id).unwrap().bytes().unwrap().as_ref()).unwrap()
    };
    write(&rt, "original.pdf", &original.id);
    let prepared = rt
        .prepare_redaction_source(
            &owner,
            ExportRequest {
                pages: vec![
                    page(&original.id, 1, "second"),
                    page(&original.id, 0, "first"),
                    page(&original.id, 0, "duplicate"),
                ],
                flatten: false,
            },
            None,
        )
        .unwrap();
    write(&rt, "ordered.pdf", &prepared.document.id);
    let preview = rt
        .preview_redaction(&owner, &prepared.document.id, &mark(2))
        .unwrap();
    write(&rt, "removed.pdf", &preview.candidate.id);
    let bounds = rt
        .redaction_bounds(&owner, &preview.preview_id, 2, 0, 1000)
        .unwrap();
    std::fs::write(
        root.join("bounds.json"),
        serde_json::to_vec_pretty(&bounds).unwrap(),
    )
    .unwrap();
    rt.release_redaction_owner(&owner);
    let owner = rt.begin_redaction_session().unwrap();
    let mut p = page(&original.id, 0, "overlay");
    p.overlays.push(overlay());
    let decorated = rt
        .prepare_redaction_source(
            &owner,
            ExportRequest {
                pages: vec![p],
                flatten: false,
            },
            Some(vec![]),
        )
        .unwrap();
    write(&rt, "overlay.pdf", &decorated.document.id);
    let form = form_source(&mut rt, &original.id);
    write(&rt, "form-original.pdf", &form.id);
    let fields = rt
        .prepare_redaction_source(
            &owner,
            ExportRequest {
                pages: vec![page(&form.id, 1, "other"), page(&form.id, 0, "field")],
                flatten: false,
            },
            Some(vec![bookmark()]),
        )
        .unwrap();
    write(&rt, "form-ordered.pdf", &fields.document.id);
    std::fs::write(root.join("receipt.json"),serde_json::to_vec_pretty(&serde_json::json!({"ordered":prepared.omitted_metadata,"overlay":decorated.omitted_metadata,"forms":fields.omitted_metadata})).unwrap()).unwrap();
}

#[test]
fn redaction_composition_discards_malformed_document_metadata_without_preserving_it() {
    let mut rt=runtime();let plain=source(&mut rt);
    let original=variant(&mut rt,&plain.id,|doc|{
        let metadata=doc.add_object(Stream::new(dictionary!{"Type"=>"Metadata","Subtype"=>"XML"},b"<broken-private-metadata>".to_vec()));
        doc.catalog_mut().unwrap().set("Metadata",metadata);
        let info=doc.add_object(dictionary!{"Title"=>Object::string_literal("Private title")});doc.trailer.set("Info",info);
    });
    let before=rt.document(&original.id).unwrap().bytes().unwrap();
    let reversed=ExportRequest{flatten:false,pages:vec![page(&original.id,1,"two"),page(&original.id,0,"one")]};
    assert!(rt.compose_pdf(reversed.clone(),Some(vec![]),CompositionIntent::Save).is_err());
    let owner=rt.begin_redaction_session().unwrap();
    let composed=rt.prepare_redaction_source(&owner,reversed,Some(vec![])).unwrap();
    let preview=rt.preview_redaction(&owner,&composed.document.id,&mark(1)).unwrap();
    let applied=rt.apply_redaction(&owner,&preview.preview_id).unwrap();
    let saved=Document::load_mem(rt.document(&applied.id).unwrap().bytes().unwrap().as_ref()).unwrap();
    assert!(!saved.catalog().unwrap().has(b"Metadata"));assert!(!saved.trailer.has(b"Info"));
    assert!(!text(&rt,&applied.id,1).contains("SECRET"));assert_eq!(rt.document(&original.id).unwrap().bytes().unwrap().as_ref(),before.as_ref());
}
