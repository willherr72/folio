use folio_engine::{DocumentInfo, ExportRequest, PagePlan, PdfEngine};
use lopdf::{dictionary, Document, Stream};
use std::path::{Path, PathBuf};
fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}
fn fixture(path: &Path, first: &str) {
    let mut d = Document::with_version("1.7");
    let pages = d.new_object_id();
    let f=d.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica","Encoding"=>"WinAnsiEncoding"});
    let resources = dictionary! {"Font"=>dictionary!{"F1"=>f}};
    let leaf=d.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),240.into(),40.into()]},b"BT /F1 16 Tf 1 0 0 1 5 10 Tm (Old) Tj ET".to_vec()));
    let wrapper=d.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),250.into(),50.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>f},"XObject"=>dictionary!{"Leaf"=>leaf}}},b"q 1 0 0 1 3 4 cm /Leaf Do Q".to_vec()));
    let leaf2=d.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),240.into(),40.into()]},format!("BT /F1 16 Tf 1 0 0 1 5 10 Tm ({first}) Tj ET").into_bytes()));
    let wrapper2=d.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),250.into(),50.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>f},"XObject"=>dictionary!{"Leaf"=>leaf2}}},b"q 1 0 0 1 3 4 cm /Leaf Do Q".to_vec()));
    let mut kids = vec![];
    for i in 0..2 {
        let content = d.add_object(Stream::new(
            dictionary! {},
            if i == 0 {
                format!(
                    "q 1 0 0 1 20 280 cm /{} Do Q q 0.8 0 0 0.8 20 180 cm /Shared Do Q",
                    if first == "Old" { "Shared" } else { "First" }
                )
                .into_bytes()
            } else {
                b"q 1 0 0 1 20 280 cm /Shared Do Q".to_vec()
            },
        ));
        let mut r = resources.clone();
        r.set("XObject",dictionary!{"First"=>if first=="Old"{wrapper}else{wrapper2},"Shared"=>wrapper,"FolioForm"=>wrapper});
        kids.push(d.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),400.into(),400.into()],"Resources"=>r,"Contents"=>content}).into());
    }
    d.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Kids"=>kids,"Count"=>2}.into(),
    );
    let root = d.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    d.trailer.set("Root", root);
    d.save(path).unwrap();
}
#[test]
fn shared_nested_occurrence_isolated_and_source_immutable() {
    let t = tempfile::tempdir().unwrap();
    let path = t.path().join("original.pdf");
    fixture(&path, "Old");
    let original = std::fs::read(&path).unwrap();
    let e = engine();
    let s = e.open_document(&path).unwrap();
    let runs = e.list_form_text_runs(&s.id, 0).unwrap();
    assert_eq!(runs.runs.len(), 2, "{:?}", runs);
    assert_eq!(runs.runs[0].object_path, vec![0, 0, 0]);
    assert_eq!(runs.runs[1].object_path, vec![1, 0, 0]);
    assert!(runs.runs.iter().all(|r| r.supported), "{:?}", runs);
    for replacement in ["Old", "New", "Longer", "O"] {
        let changed = e
            .replace_form_text(&s.id, 0, &runs.runs[0].object_path, "Old", replacement)
            .unwrap();
        assert_eq!(changed.pages.len(), 1);
        let after = e.list_form_text_runs(&changed.id, 0).unwrap();
        assert_eq!(after.runs[0].text, replacement);
        assert_eq!(after.runs[1].text, "Old");
        assert_eq!(e.list_form_text_runs(&s.id, 1).unwrap().runs[0].text, "Old");
        assert_eq!(std::fs::read(&path).unwrap(), original);
        let reference = t.path().join("reference.pdf");
        fixture(&reference, replacement);
        let r = e.open_document(&reference).unwrap();
        assert_eq!(
            e.render_page(&changed.id, 0, 800).unwrap(),
            e.render_page(&r.id, 0, 800).unwrap()
        );
        if let Some(root) = std::env::var_os("FOLIO_FORM_ARTIFACTS") {
            let root = PathBuf::from(root);
            std::fs::create_dir_all(&root).unwrap();
            let case = format!("shared-{}", replacement.to_lowercase());
            std::fs::copy(&path, root.join(format!("{case}-original.pdf"))).unwrap();
            std::fs::copy(&reference, root.join(format!("{case}-reference.pdf"))).unwrap();
            e.export_pdf(
                ExportRequest {
                    flatten: false,
                    pages: vec![plan(&changed, 0), plan(&s, 1)],
                },
                root.join(format!("{case}-exported.pdf")),
            )
            .unwrap();
            std::fs::write(root.join(format!("{case}-manifest.json")),serde_json::to_vec_pretty(&serde_json::json!({"case":case,"replacement":replacement,"selectedPath":[0,0,0],"reference":"authored original resource structure with first leaf text changed","pages":2,"sourceUnchanged":true})).unwrap()).unwrap();
        }
        e.close_document(&r.id).unwrap();
        e.close_document(&changed.id).unwrap();
    }
    e.close_document(&s.id).unwrap();
}

fn plan(s: &DocumentInfo, index: usize) -> PagePlan {
    PagePlan {
        id: format!("page-{index}"),
        source_id: s.id.clone(),
        page_index: index,
        width: s.pages[index].width,
        height: s.pages[index].height,
        rotation: 0,
        overlays: vec![],
    }
}
#[test]
fn stale_identity_overflow_and_failed_edits_leave_original_untouched() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("original.pdf");
    fixture(&p, "Old");
    let bytes = std::fs::read(&p).unwrap();
    let e = engine();
    let s = e.open_document(&p).unwrap();
    for (path, expected, value) in [
        (vec![8, 0, 0], "Old", "New"),
        (vec![0, 0], "Old", "New"),
        (vec![0, 0, 0], "Stale", "New"),
        (vec![0, 0, 0], "Old", ""),
        (
            vec![0, 0, 0],
            "Old",
            "Extraordinarily long replacement that extends beyond every ancestor form box",
        ),
    ] {
        assert!(e
            .replace_form_text(&s.id, 0, &path, expected, value)
            .is_err());
        assert_eq!(e.source_bytes(&s.id).unwrap().as_ref(), bytes);
    }
    let changed = e
        .replace_form_text(&s.id, 0, &[0, 0, 0], "Old", "New")
        .unwrap();
    assert!(e
        .export_pdf(
            ExportRequest {
                flatten: false,
                pages: vec![plan(&changed, 0)]
            },
            &p
        )
        .is_err());
    assert_eq!(std::fs::read(&p).unwrap(), bytes);
    e.close_document(&changed.id).unwrap();
    e.close_document(&s.id).unwrap();
    assert!(e.source_bytes(&s.id).is_err());
}
#[test]
fn included_practice_edits_only_first_occurrence() {
    let e = engine();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../examples/Edit one shared occurrence.pdf");
    let s = e.open_document(path).unwrap();
    let before = e.list_form_text_runs(&s.id, 0).unwrap();
    assert_eq!(before.runs.len(), 2, "{before:?}");
    assert_eq!(e.list_form_text_runs(&s.id, 1).unwrap().runs.len(), 1);
    for replacement in ["New words", "Revised words", "New shared words"] {
        let changed = e
            .replace_form_text(
                &s.id,
                0,
                &before.runs[0].object_path,
                &before.runs[0].text,
                replacement,
            )
            .unwrap();
        let after = e.list_form_text_runs(&changed.id, 0).unwrap();
        assert_eq!(after.runs[0].text, replacement);
        assert_eq!(after.runs[1].text, before.runs[1].text);
        assert_eq!(
            e.list_form_text_runs(&s.id, 1).unwrap().runs[0].text,
            before.runs[0].text
        );
        e.close_document(&changed.id).unwrap();
    }
    e.close_document(&s.id).unwrap();
}
#[test]
fn repeated_edits_do_not_accumulate_private_form_chains() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("repeat.pdf");
    fixture(&p, "Old");
    let e = engine();
    let mut s = e.open_document(&p).unwrap();
    let mut expected = "Old";
    let mut counts = vec![];
    for i in 0..8 {
        let next = if i % 2 == 0 { "New" } else { "Old" };
        let changed = e
            .replace_form_text(&s.id, 0, &[0, 0, 0], expected, next)
            .unwrap();
        let bytes = e.source_bytes(&changed.id).unwrap();
        counts.push(Document::load_mem(&bytes).unwrap().objects.len());
        e.close_document(&s.id).unwrap();
        assert!(e.source_bytes(&s.id).is_err());
        s = changed;
        expected = next;
    }
    assert!(
        counts.windows(2).all(|v| v[0] == v[1]),
        "Private form object growth: {counts:?}"
    );
    e.close_document(&s.id).unwrap();
    if let Some(root) = std::env::var_os("FOLIO_FORM_ARTIFACTS") {
        let root = PathBuf::from(root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("repeated-edits.json"),serde_json::to_vec_pretty(&serde_json::json!({"edits":8,"objectCounts":counts,"closedSourcesUnavailable":true})).unwrap()).unwrap();
    }
}
fn mutate_fixture(path: &Path, kind: &str) {
    fixture(path, "Old");
    let mut d = Document::load(path).unwrap();
    if kind == "tagged" {
        let root = d.trailer.get(b"Root").unwrap().as_reference().unwrap();
        d.get_dictionary_mut(root)
            .unwrap()
            .set("MarkInfo", dictionary! {"Marked"=>true});
    } else {
        for object in d.objects.values_mut() {
            if let Ok(s) = object.as_stream_mut() {
                if s.content.windows(4).any(|w| w == b"/F1 ") {
                    match kind {
                        "spacing" => s.set_plain_content(
                            b"BT /F1 16 Tf 1 Tc 1 0 0 1 5 10 Tm (Old) Tj ET".to_vec(),
                        ),
                        "clip" => s.set_plain_content(
                            b"0 0 200 30 re W n BT /F1 16 Tf 1 0 0 1 5 10 Tm (Old) Tj ET".to_vec(),
                        ),
                        "bbox" => s
                            .dict
                            .set("BBox", vec![0.into(), 0.into(), 8.into(), 40.into()]),
                        _ => {}
                    }
                }
            }
        }
    }
    d.save(path).unwrap();
}
#[test]
fn unsupported_state_tags_and_ancestor_clipping_are_refused() {
    let t = tempfile::tempdir().unwrap();
    let e = engine();
    for kind in ["tagged", "spacing", "clip", "bbox"] {
        let p = t.path().join(format!("{kind}.pdf"));
        mutate_fixture(&p, kind);
        let bytes = std::fs::read(&p).unwrap();
        let s = e.open_document(&p).unwrap();
        let listed = e.list_form_text_runs(&s.id, 0).unwrap();
        assert!(
            listed.runs.is_empty() && listed.reason.is_some(),
            "{kind}: {listed:?}"
        );
        assert!(
            e.replace_form_text(&s.id, 0, &[0, 0, 0], "Old", "New")
                .is_err(),
            "{kind}"
        );
        assert_eq!(std::fs::read(&p).unwrap(), bytes);
        e.close_document(&s.id).unwrap();
    }
}
#[test]
fn intermediate_inherited_resource_scope_is_refused_without_changing_reader_semantics() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("inherited-intermediate.pdf");
    fixture(&p, "Old");
    let mut d = Document::load(&p).unwrap();
    let mut leaf = None;
    for object in d.objects.values_mut() {
        if let Ok(s) = object.as_stream_mut() {
            if s.content.windows(8).any(|v| v == b"/Leaf Do") {
                let r = s.dict.remove(b"Resources").unwrap();
                leaf = Some(
                    r.as_dict()
                        .unwrap()
                        .get(b"XObject")
                        .unwrap()
                        .as_dict()
                        .unwrap()
                        .get(b"Leaf")
                        .unwrap()
                        .clone(),
                );
            }
        }
    }
    let leaf = leaf.unwrap();
    for id in d.get_pages().into_values() {
        let page = d.get_dictionary_mut(id).unwrap();
        page.get_mut(b"Resources")
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .get_mut(b"XObject")
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set("Leaf", leaf.clone());
    }
    d.save(&p).unwrap();
    let e = engine();
    let s = e.open_document(&p).unwrap();
    let runs = e.list_form_text_runs(&s.id, 0).unwrap();
    assert!(
        runs.runs.is_empty()
            && runs
                .reason
                .as_deref()
                .is_some_and(|r| r.contains("intermediate")),
        "{runs:?}"
    );
    e.close_document(&s.id).unwrap();
}
#[test]
fn newly_overlapping_another_occurrence_is_refused() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("collision.pdf");
    fixture(&p, "Old");
    let mut d = Document::load(&p).unwrap();
    let first = *d.get_pages().values().next().unwrap();
    let id = d
        .get_dictionary(first)
        .unwrap()
        .get(b"Contents")
        .unwrap()
        .as_reference()
        .unwrap();
    d.get_object_mut(id)
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .set_plain_content(
            b"q 1 0 0 1 20 280 cm /Shared Do Q q 1 0 0 1 60 280 cm /Shared Do Q".to_vec(),
        );
    d.save(&p).unwrap();
    let e = engine();
    let s = e.open_document(&p).unwrap();
    let error = e
        .replace_form_text(&s.id, 0, &[0, 0, 0], "Old", "Longer replacement")
        .unwrap_err();
    assert!(error.to_string().contains("overlap"), "{error}");
    e.close_document(&s.id).unwrap();
}
fn apply_form_matrices(path: &Path) {
    let mut d = Document::load(path).unwrap();
    for object in d.objects.values_mut() {
        if let Ok(s) = object.as_stream_mut() {
            if s.dict.get(b"Subtype").and_then(lopdf::Object::as_name).ok() == Some(b"Form") {
                s.dict.set(
                    "Matrix",
                    if s.content.windows(8).any(|w| w == b"/Leaf Do") {
                        vec![1.into(), 0.into(), 0.into(), 1.into(), 4.into(), 5.into()]
                    } else {
                        vec![
                            lopdf::Object::Real(0.9),
                            0.into(),
                            0.into(),
                            lopdf::Object::Real(1.1),
                            2.into(),
                            3.into(),
                        ]
                    },
                );
            }
        }
    }
    d.save(path).unwrap();
}
#[test]
fn ancestor_and_leaf_dictionary_matrices_are_applied_once() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("matrices.pdf");
    fixture(&p, "Old");
    apply_form_matrices(&p);
    let e = engine();
    let s = e.open_document(&p).unwrap();
    let runs = e.list_form_text_runs(&s.id, 0).unwrap();
    assert_eq!(runs.runs.len(), 2, "{runs:?}");
    let edited = e
        .replace_form_text(&s.id, 0, &[0, 0, 0], "Old", "New")
        .unwrap();
    let reference = t.path().join("reference.pdf");
    fixture(&reference, "New");
    apply_form_matrices(&reference);
    let r = e.open_document(&reference).unwrap();
    assert_eq!(
        e.render_page(&edited.id, 0, 800).unwrap(),
        e.render_page(&r.id, 0, 800).unwrap()
    );
    for id in [s.id, edited.id, r.id] {
        e.close_document(&id).unwrap();
    }
}
#[test]
fn form_page_annotations_are_explicitly_unsupported() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("annotation.pdf");
    fixture(&p, "Old");
    let mut d = Document::load(&p).unwrap();
    let annotation=d.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![300.into(),300.into(),310.into(),310.into()],"Contents"=>lopdf::Object::string_literal("note")});
    let page = *d.get_pages().values().next().unwrap();
    d.get_dictionary_mut(page)
        .unwrap()
        .set("Annots", vec![annotation.into()]);
    d.save(&p).unwrap();
    let e = engine();
    let s = e.open_document(&p).unwrap();
    let listed = e.list_form_text_runs(&s.id, 0).unwrap();
    assert!(
        listed
            .reason
            .as_deref()
            .is_some_and(|r| r.contains("annotation")),
        "{listed:?}"
    );
    e.close_document(&s.id).unwrap();
}
fn explicit_leaf_resources(path: &Path) {
    let mut d = Document::load(path).unwrap();
    let font = d
        .objects
        .iter()
        .find_map(|(id, o)| {
            o.as_dict()
                .ok()
                .filter(|v| {
                    v.get(b"BaseFont").and_then(lopdf::Object::as_name).ok() == Some(b"Helvetica")
                })
                .map(|_| *id)
        })
        .unwrap();
    for object in d.objects.values_mut() {
        if let Ok(s) = object.as_stream_mut() {
            if s.content.windows(4).any(|v| v == b"/F1 ") {
                s.dict
                    .set("Resources", dictionary! {"Font"=>dictionary!{"F1"=>font}});
            }
        }
    }
    d.save(path).unwrap();
}
#[test]
fn explicit_leaf_font_resources_retain_independent_reader_text() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("original.pdf");
    fixture(&p, "Old");
    explicit_leaf_resources(&p);
    let e = engine();
    let s = e.open_document(&p).unwrap();
    let changed = e
        .replace_form_text(&s.id, 0, &[0, 0, 0], "Old", "New")
        .unwrap();
    let reference = t.path().join("reference.pdf");
    fixture(&reference, "New");
    explicit_leaf_resources(&reference);
    let r = e.open_document(&reference).unwrap();
    assert_eq!(
        e.render_page(&changed.id, 0, 800).unwrap(),
        e.render_page(&r.id, 0, 800).unwrap()
    );
    if let Some(root) = std::env::var_os("FOLIO_FORM_ARTIFACTS") {
        let root = PathBuf::from(root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::copy(&p, root.join("explicit-new-original.pdf")).unwrap();
        std::fs::copy(&reference, root.join("explicit-new-reference.pdf")).unwrap();
        e.export_pdf(
            ExportRequest {
                flatten: false,
                pages: vec![plan(&changed, 0), plan(&s, 1)],
            },
            root.join("explicit-new-exported.pdf"),
        )
        .unwrap();
        std::fs::write(root.join("explicit-new-manifest.json"),serde_json::to_vec_pretty(&serde_json::json!({"case":"explicit-new","replacement":"New","selectedPath":[0,0,0],"explicitLeafResources":true,"pages":2})).unwrap()).unwrap();
    }
    for id in [s.id, changed.id, r.id] {
        e.close_document(&id).unwrap();
    }
}
#[test]
fn custom_form_font_encodings_are_not_assumed_ascii() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("encoding.pdf");
    fixture(&p, "Old");
    let mut d = Document::load(&p).unwrap();
    for object in d.objects.values_mut() {
        if let Ok(font) = object.as_dict_mut() {
            if font.has(b"BaseFont") {
                font.set("Encoding",dictionary!{"Type"=>"Encoding","BaseEncoding"=>"WinAnsiEncoding","Differences"=>vec![65.into(),lopdf::Object::Name(b"Z".to_vec())]});
            }
        }
    }
    d.save(&p).unwrap();
    let e = engine();
    let s = e.open_document(&p).unwrap();
    let runs = e.list_form_text_runs(&s.id, 0).unwrap();
    assert!(
        runs.reason
            .as_deref()
            .is_some_and(|v| v.contains("encoding")),
        "{runs:?}"
    );
    assert!(e
        .replace_form_text(&s.id, 0, &[0, 0, 0], "Old", "ABC")
        .is_err());
    e.close_document(&s.id).unwrap();
}
#[test]
fn image_only_page_has_no_form_editing_refusal() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("image.pdf");
    let mut d = Document::with_version("1.7");
    let pages = d.new_object_id();
    let contents = d.add_object(Stream::new(
        dictionary! {},
        b"q BI /W 1 /H 1 /BPC 8 /CS /G ID \0 EI Q".to_vec(),
    ));
    let page=d.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),200.into(),200.into()],"Resources"=>dictionary!{},"Contents"=>contents});
    d.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into(),
    );
    let root = d.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    d.trailer.set("Root", root);
    d.save(&p).unwrap();
    let e = engine();
    let s = e.open_document(&p).unwrap();
    let listed = e.list_form_text_runs(&s.id, 0).unwrap();
    assert!(
        listed.runs.is_empty() && listed.reason.is_none(),
        "{listed:?}"
    );
    e.close_document(&s.id).unwrap();
}
#[test]
fn page_crop_and_intrinsic_rotation_preserve_form_occurrence_identity() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("rotated.pdf");
    let reference = t.path().join("reference.pdf");
    fixture(&p, "Old");
    fixture(&reference, "New");
    for path in [&p, &reference] {
        let mut d = Document::load(path).unwrap();
        for id in d.get_pages().into_values() {
            let page = d.get_dictionary_mut(id).unwrap();
            page.set("Rotate", 90);
            page.set(
                "CropBox",
                vec![10.into(), 20.into(), 380.into(), 380.into()],
            );
        }
        d.save(path).unwrap();
    }
    let e = engine();
    let s = e.open_document(&p).unwrap();
    let r = e.open_document(&reference).unwrap();
    let runs = e.list_form_text_runs(&s.id, 0).unwrap();
    assert_eq!(runs.runs.len(), 2, "{runs:?}");
    let changed = e
        .replace_form_text(&s.id, 0, &[0, 0, 0], "Old", "New")
        .unwrap();
    assert_eq!(
        e.render_page(&changed.id, 0, 800).unwrap(),
        e.render_page(&r.id, 0, 800).unwrap()
    );
    for id in [s.id, r.id, changed.id] {
        e.close_document(&id).unwrap();
    }
}
