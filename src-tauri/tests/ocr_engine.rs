#![cfg(feature = "shaped-text")]
use folio_engine::{DocumentInfo, EngineError, ExportRequest, OcrPage, OcrUpdate, PdfEngine};
use lopdf::{dictionary, Document, Object, Stream};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
};
#[path = "support/protected_fixture.rs"]
mod protected_fixture;

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}
fn source(rotation: u16) -> Document {
    let mut d = Document::with_version("1.7");
    let pages = d.new_object_id();
    let font =
        d.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let image=d.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>20,"Height"=>10,"ColorSpace"=>"DeviceRGB","BitsPerComponent"=>8},
        (0..200).flat_map(|n|[n as u8,80,190]).collect()));
    let content = d.add_object(Stream::new(
        dictionary! {},
        b"q 180 0 0 60 30 40 cm /Im0 Do Q BT /FH 10 Tf 1 0 0 1 30 118 Tm (Native header) Tj ET\n"
            .to_vec(),
    ));
    let resources = d.add_object(
        dictionary! {"Font"=>dictionary!{"FH"=>font},"XObject"=>dictionary!{"Im0"=>image}},
    );
    let page=d.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),240.into(),160.into()],
        "CropBox"=>vec![20.into(),30.into(),220.into(),130.into()],"Rotate"=>i64::from(rotation),"Contents"=>content});
    let other = d.add_object(d.get_object(page).unwrap().clone());
    d.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![Object::Reference(page),Object::Reference(other)],"Count"=>2,"Resources"=>resources}.into());
    let catalog = d.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    d.trailer.set("Root", catalog);
    d
}
fn bytes(mut d: Document) -> Vec<u8> {
    let mut bytes = vec![];
    d.save_to(&mut bytes).unwrap();
    bytes
}
fn open(e: &PdfEngine, dir: &Path, name: &str, d: Document) -> DocumentInfo {
    let path = dir.join(name);
    fs::write(&path, bytes(d)).unwrap();
    e.open_document(path).unwrap()
}
fn recognized(width: u32, height: u32) -> OcrPage {
    serde_json::from_value(json!({"width":width,"height":height,"blocks":[{"paragraphs":[{"lines":[
        {"words":[{"text":"café niño","confidence":93,"bounds":{"x":width as f32*0.15,"y":height as f32*0.4,"width":width as f32*0.7,"height":height as f32*0.08}}]},
        {"words":[{"text":"𐐀 final","confidence":90,"bounds":{"x":width as f32*0.15,"y":height as f32*0.65,"width":width as f32*0.7,"height":height as f32*0.08}}]}
    ]}]}]})).unwrap()
}
fn request(info: &DocumentInfo) -> ExportRequest {
    serde_json::from_value(json!({"pages":info.pages.iter().enumerate().map(|(i,p)|json!({
    "id":format!("p{i}"),"sourceId":info.id,"pageIndex":i,"rotation":0,"width":p.width,"height":p.height,"overlays":p.overlays})).collect::<Vec<_>>()})).unwrap()
}
fn text(e: &PdfEngine, id: &str) -> String {
    e.page_text(id, 0)
        .unwrap()
        .characters
        .iter()
        .map(|c| c.text.as_str())
        .collect()
}

#[test]
fn native_exclusion_boxes_align_with_top_header_on_full_sized_page() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    let mut d = source(0);
    let ids: Vec<_> = d.get_pages().values().copied().collect();
    let content = d.add_object(Stream::new(
        dictionary! {},
        b"q BT 1 0 0 1 24 452 Tm /FH 12 Tf [(Native header)] TJ ET Q".to_vec(),
    ));
    for id in ids {
        let p = d.get_object_mut(id).unwrap().as_dict_mut().unwrap();
        p.set("MediaBox", vec![0.into(), 0.into(), 360.into(), 480.into()]);
        p.remove(b"CropBox");
        p.set("Contents", content);
    }
    let info = open(&e, dir.path(), "header.pdf", d);
    let prep = e.prepare_ocr_page(&info.id, 0, 0).unwrap();
    assert!(!prep.exclusions.is_empty());
    for b in prep.exclusions {
        assert!(b.y < 125., "native header must be near the top: {b:?}");
    }
}

#[test]
fn owned_review_requires_proven_ordered_boxes_and_normalizes_before_noop_check() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    let info = open(&e, dir.path(), "review-proof.pdf", source(0));
    let prep = e.prepare_ocr_page(&info.id, 0, 0).unwrap();
    let ocr = recognized(prep.width, prep.height);
    let mut update = OcrUpdate {
        page_index: 0,
        rotation: 0,
        page: ocr,
        preserve_owned_orientation: true,
    };
    assert!(e.apply_ocr(&info.id, vec![update.clone()]).is_err());
    update.preserve_owned_orientation = false;
    let owned = e.apply_ocr(&info.id, vec![update]).unwrap();
    let prep = e.prepare_ocr_page(&owned.id, 0, 90).unwrap();
    let update = OcrUpdate {
        page_index: 0,
        rotation: 90,
        page: prep.owned.unwrap(),
        preserve_owned_orientation: true,
    };
    let mut rounding = update.clone();
    rounding.page.blocks[0].paragraphs[0].lines[0].words[0]
        .bounds
        .x += 0.001;
    assert!(
        e.apply_ocr(&owned.id, vec![rounding]).is_err(),
        "roundtrip tolerance must not create a no-op source"
    );
    let mut moved = update.clone();
    moved.page.blocks[0].paragraphs[0].lines[0].words[0]
        .bounds
        .x += 2.;
    assert!(e.apply_ocr(&owned.id, vec![moved]).is_err());
    let mut reordered = update.clone();
    reordered.page.blocks[0].paragraphs[0].lines.reverse();
    assert!(e.apply_ocr(&owned.id, vec![reordered]).is_err());
    let mut removed = update;
    removed.page.blocks[0].paragraphs[0].lines.remove(0);
    let changed = e.apply_ocr(&owned.id, vec![removed]).unwrap();
    assert!(!text(&e, &changed.id).contains("café"));
    assert!(text(&e, &changed.id).contains("𐐀 final"));
    for info in [info, owned, changed] {
        e.close_document(&info.id).unwrap();
    }
}

#[test]
fn correcting_owned_ocr_after_rotation_preserves_character_direction_and_neighbors() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    for intrinsic in [0, 90, 180, 270] {
        let info = open(
            &e,
            dir.path(),
            &format!("direction-{intrinsic}.pdf"),
            source(intrinsic),
        );
        let prepared = e.prepare_ocr_page(&info.id, 0, 0).unwrap();
        let mut ocr = recognized(prepared.width, prepared.height);
        ocr.blocks[0].paragraphs[0].lines[0].words[0].text = "ABCDE".into();
        ocr.blocks[0].paragraphs[0].lines[1].words[0].text = "VWXYZ".into();
        let owned = e
            .apply_ocr(
                &info.id,
                vec![OcrUpdate {
                    preserve_owned_orientation: false,
                    page_index: 0,
                    rotation: 0,
                    page: ocr,
                }],
            )
            .unwrap();
        let before = e.page_text(&owned.id, 0).unwrap();
        for extra in [90, 180, 270, 0] {
            let mut reviewed = e
                .prepare_ocr_page(&owned.id, 0, extra)
                .unwrap()
                .owned
                .unwrap();
            reviewed.blocks[0].paragraphs[0].lines[0].words[0].text = "ABCDF".into();
            let changed = e
                .apply_ocr(
                    &owned.id,
                    vec![OcrUpdate {
                        preserve_owned_orientation: true,
                        page_index: 0,
                        rotation: extra,
                        page: reviewed,
                    }],
                )
                .unwrap();
            let after = e.page_text(&changed.id, 0).unwrap();
            assert_eq!(before.characters.len(), after.characters.len());
            for (a, b) in before.characters.iter().zip(&after.characters) {
                assert!(
                    (a.x - b.x).abs() < 0.001
                        && (a.y - b.y).abs() < 0.001
                        && (a.width - b.width).abs() < 0.001
                        && (a.height - b.height).abs() < 0.001,
                    "character direction changed at {intrinsic}/{extra}: {a:?} vs {b:?}"
                );
            }
            e.close_document(&changed.id).unwrap();
        }
        e.close_document(&owned.id).unwrap();
        e.close_document(&info.id).unwrap();
    }
}

#[test]
fn raster_mapping_reading_correction_and_pixels_at_all_rotations() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    for intrinsic in [0, 90, 180, 270] {
        for extra in [0, 90, 180, 270] {
            let info = open(
                &e,
                dir.path(),
                &format!("source-{intrinsic}-{extra}.pdf"),
                source(intrinsic),
            );
            let original = e.source_bytes(&info.id).unwrap();
            let before = e.render_page(&info.id, 0, 600).unwrap();
            let prep = e.prepare_ocr_page(&info.id, 0, extra).unwrap();
            let total = (intrinsic + extra) % 360;
            let expected = if total == 90 || total == 270 {
                (416, 833)
            } else {
                (833, 416)
            };
            assert_eq!((prep.width, prep.height), expected, "{intrinsic}/{extra}");
            assert!((prep.effective_dpi - 300.).abs() < 1.);
            assert!(!prep.exclusions.is_empty());
            assert!(prep.owned.is_none());
            let decoded = image::load_from_memory(&prep.png).unwrap();
            assert_eq!((decoded.width(), decoded.height()), expected);
            let pixels = decoded.to_rgb8();
            for b in &prep.exclusions {
                let ink = (b.y.floor().max(0.) as u32
                    ..(b.y + b.height).ceil().min(prep.height as f32) as u32)
                    .any(|y| {
                        (b.x.floor().max(0.) as u32
                            ..(b.x + b.width).ceil().min(prep.width as f32) as u32)
                            .any(|x| pixels.get_pixel(x, y).0.iter().all(|v| *v < 100))
                    });
                assert!(
                    ink,
                    "raster ink must align with native glyph at {intrinsic}/{extra}: {b:?}"
                );
            }
            let ocr = recognized(prep.width, prep.height);
            let changed = e
                .apply_ocr(
                    &info.id,
                    vec![OcrUpdate {
                        preserve_owned_orientation: false,
                        page_index: 0,
                        rotation: extra,
                        page: ocr.clone(),
                    }],
                )
                .unwrap();
            assert_eq!(changed.pages.len(), 2);
            assert_eq!(before, e.render_page(&changed.id, 0, 600).unwrap());
            assert_eq!(
                original.as_ref(),
                e.source_bytes(&info.id).unwrap().as_ref()
            );
            let extracted = text(&e, &changed.id);
            assert!(extracted.contains("Native header"));
            assert_eq!(
                extracted
                    .replace("Native header", "")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
                "café niño 𐐀 final",
                "{intrinsic}/{extra}"
            );
            assert_eq!(
                e.search_page(&changed.id, 0, "niño 𐐀")
                    .unwrap()
                    .matches
                    .len(),
                1
            );
            let prepared = e.prepare_ocr_page(&changed.id, 0, extra).unwrap();
            assert_eq!(prepared.owned.unwrap(), ocr);
            assert_eq!(prepared.exclusions, prep.exclusions);
            let mut corrected = ocr.clone();
            corrected.blocks[0].paragraphs[0].lines[0].words[0].text = "corrected".into();
            let corrected = e
                .apply_ocr(
                    &changed.id,
                    vec![OcrUpdate {
                        preserve_owned_orientation: false,
                        page_index: 0,
                        rotation: extra,
                        page: corrected,
                    }],
                )
                .unwrap();
            assert!(text(&e, &corrected.id).contains("corrected"));
            assert!(!text(&e, &corrected.id).contains("café"));
            let mut empty = ocr;
            empty.blocks.clear();
            let removed = e
                .apply_ocr(
                    &corrected.id,
                    vec![OcrUpdate {
                        preserve_owned_orientation: false,
                        page_index: 0,
                        rotation: extra,
                        page: empty,
                    }],
                )
                .unwrap();
            assert!(!text(&e, &removed.id).contains("corrected"));
            assert_eq!(before, e.render_page(&removed.id, 0, 600).unwrap());
            for doc in [info, changed, corrected, removed] {
                e.close_document(&doc.id).unwrap();
            }
        }
    }
}
#[test]
fn filters_existing_native_text_and_rejects_stale_or_malformed_updates() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    let info = open(&e, dir.path(), "mixed.pdf", source(0));
    let prep = e.prepare_ocr_page(&info.id, 0, 0).unwrap();
    let mut ocr = recognized(prep.width, prep.height);
    let mut duplicate = ocr.blocks[0].paragraphs[0].lines[0].words[0].clone();
    duplicate.text = "duplicate".into();
    duplicate.bounds = prep.exclusions[0].clone();
    ocr.blocks[0].paragraphs[0].lines[0]
        .words
        .insert(0, duplicate);
    let changed = e
        .apply_ocr(
            &info.id,
            vec![OcrUpdate {
                preserve_owned_orientation: false,
                page_index: 0,
                rotation: 0,
                page: ocr.clone(),
            }],
        )
        .unwrap();
    assert!(!text(&e, &changed.id).contains("duplicate"));
    assert!(text(&e, &changed.id).contains("café niño"));
    for case in 0..5 {
        let mut update = OcrUpdate {
            preserve_owned_orientation: false,
            page_index: 0,
            rotation: 0,
            page: ocr.clone(),
        };
        match case {
            0 => update.page.width -= 1,
            1 => update.rotation = 1,
            2 => update.page_index = 2,
            3 => update.page.blocks[0].paragraphs[0].lines[0].words[0].text = "bad\ntext".into(),
            _ => {}
        }
        let updates = if case == 4 {
            vec![update.clone(), update]
        } else {
            vec![update]
        };
        assert!(e.apply_ocr(&info.id, updates).is_err());
    }
    assert!(e.apply_ocr(&info.id, vec![]).is_err());
    e.close_document(&changed.id).unwrap();
    e.close_document(&info.id).unwrap();
}
#[test]
fn permission_checks_and_immutable_protected_replacement_survive_export() {
    use lopdf::encryption::Permissions;
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    for permissions in [
        Permissions::empty(),
        Permissions::MODIFIABLE,
        Permissions::COPYABLE,
        Permissions::MODIFIABLE | Permissions::COPYABLE,
    ] {
        let path = dir.path().join("protected.pdf");
        let original = protected_fixture::encrypt(&bytes(source(0)), permissions);
        fs::write(&path, &original).unwrap();
        let reader = e.open_document_with_password(&path, "reader").unwrap();
        let allowed = permissions.contains(Permissions::MODIFIABLE | Permissions::COPYABLE);
        assert_eq!(e.prepare_ocr_page(&reader.id, 0, 0).is_ok(), allowed);
        let owner = e.open_document_with_password(&path, "owner").unwrap();
        let prep = e.prepare_ocr_page(&owner.id, 0, 0).unwrap();
        let update = OcrUpdate {
            preserve_owned_orientation: false,
            page_index: 0,
            rotation: 0,
            page: recognized(prep.width, prep.height),
        };
        if !allowed {
            assert!(matches!(
                e.apply_ocr(&reader.id, vec![update.clone()]),
                Err(EngineError::PermissionDenied(_))
            ));
        }
        let changed = e.apply_ocr(&owner.id, vec![update]).unwrap();
        assert_eq!(
            changed.protection.as_ref().unwrap().document_id,
            owner.protection.as_ref().unwrap().document_id
        );
        let output = dir.path().join("output.pdf");
        e.export_pdf(request(&changed), &output).unwrap();
        let reopened = e.open_document_with_password(&output, "owner").unwrap();
        assert!(text(&e, &reopened.id).contains("café niño"));
        assert_eq!(fs::read(&path).unwrap(), original);
        for info in [reader, owner, changed, reopened] {
            e.close_document(&info.id).unwrap();
        }
    }
}

#[test]
fn annotations_shared_fields_bookmarks_and_unselected_contents_survive_ocr_and_recovery() {
    use folio_engine::{FillFormFieldRequest, FormFieldValue, RecoveryStore};
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    let mut d = source(0);
    let ids: Vec<_> = d.get_pages().values().copied().collect();
    let field = d.new_object_id();
    let mut widgets = vec![];
    let font =
        d.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    // The positive preservation fixture must paint its declared nonempty answer.
    let original_appearance = b"q 1 0 0 rg 0 0 30 10 re f Q /Tx BMC q BT 0 g /F1 5 Tf 1 2 Td (original) Tj ET Q EMC";
    let appearance=d.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),30.into(),10.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}},original_appearance.to_vec()));
    for page in &ids {
        let widget=d.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","Parent"=>field,"P"=>*page,"Rect"=>vec![40.into(),34.into(),70.into(),44.into()],"F"=>4,"AP"=>dictionary!{"N"=>appearance}});
        widgets.push(Object::Reference(widget));
        d.get_dictionary_mut(*page)
            .unwrap()
            .set("Annots", vec![Object::Reference(widget)]);
    }
    d.objects.insert(field,dictionary!{"FT"=>"Tx","T"=>Object::string_literal("Shared"),"V"=>Object::string_literal("original"),"Kids"=>widgets}.into());
    let form=d.add_object(dictionary!{"Fields"=>vec![Object::Reference(field)],"DA"=>Object::string_literal("/F1 8 Tf 0 g"),"DR"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}});
    d.catalog_mut().unwrap().set("AcroForm", form);
    let comment=d.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","P"=>ids[0],"Rect"=>vec![175.into(),45.into(),195.into(),65.into()],"Contents"=>Object::string_literal("Keep this note"),"F"=>4});
    d.get_dictionary_mut(ids[0])
        .unwrap()
        .get_mut(b"Annots")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .push(comment.into());
    let outlines = d.new_object_id();
    let item=d.add_object(dictionary!{"Title"=>Object::string_literal("Second page"),"Parent"=>outlines,"Dest"=>vec![Object::Reference(ids[1]),Object::Name(b"Fit".to_vec())]});
    d.objects.insert(
        outlines,
        dictionary! {"Type"=>"Outlines","First"=>item,"Last"=>item,"Count"=>1}.into(),
    );
    d.catalog_mut().unwrap().set("Outlines", outlines);
    let unselected = d.get_page_content(ids[1]);
    let info = open(&e, dir.path(), "complex.pdf", d);
    let prep = e.prepare_ocr_page(&info.id, 0, 0).unwrap();
    assert!(info.pages[0].overlays.is_empty());
    assert_eq!(e.list_native_comments(&info.id).unwrap()[0].text,"Keep this note");
    assert_eq!(info.bookmarks.len(), 1);
    // Prepared recognition excludes widgets and native comment appearances.
    let mut without = Document::load_mem(&e.source_bytes(&info.id).unwrap()).unwrap();
    for id in without.get_pages().values() {
        without.get_dictionary_mut(*id).unwrap().remove(b"Annots");
    }
    without.catalog_mut().unwrap().remove(b"AcroForm");
    let plain = open(&e, dir.path(), "no-annots.pdf", without);
    assert_eq!(prep.png, e.prepare_ocr_page(&plain.id, 0, 0).unwrap().png);
    let result = e
        .apply_ocr(
            &info.id,
            vec![OcrUpdate {
                preserve_owned_orientation: false,
                page_index: 0,
                rotation: 0,
                page: recognized(prep.width, prep.height),
            }],
        )
        .unwrap();
    let mut before_bookmarks = info.bookmarks.clone();
    let mut after_bookmarks = result.bookmarks.clone();
    for b in before_bookmarks.iter_mut().chain(&mut after_bookmarks) {
        b.id.clear();
    }
    assert_eq!(after_bookmarks, before_bookmarks);
    assert!(result.pages[0].overlays.is_empty());
    assert_eq!(e.list_native_comments(&result.id).unwrap()[0].text,"Keep this note");
    let changed = Document::load_mem(&e.source_bytes(&result.id).unwrap()).unwrap();
    let other = *changed.get_pages().values().nth(1).unwrap();
    assert_eq!(changed.get_page_content(other), unselected);
    assert_eq!(changed.get_object(appearance).unwrap().as_stream().unwrap().get_plain_content().unwrap(), original_appearance);
    let field = e.list_form_fields(&result.id, 0).unwrap().remove(0);
    assert_eq!(field.value, FormFieldValue::Text { text: "original".into() });
    let filled = e
        .fill_form_field(
            &result.id,
            FillFormFieldRequest {
                page_index: 0,
                annotation_index: field.annotation_index,
                expected_value: field.value,
                value: FormFieldValue::Text {
                    text: "after OCR".into(),
                },
            },
        )
        .unwrap();
    assert_eq!(
        e.list_form_fields(&filled.id, 1).unwrap()[0].value,
        FormFieldValue::Text {
            text: "after OCR".into()
        }
    );
    let filled_path = dir.path().join("filled-after-ocr.pdf");
    e.export_pdf(request(&filled), &filled_path).unwrap();
    let filled_reopened = e.open_document(&filled_path).unwrap();
    for page in 0..2 {
        assert_eq!(e.list_form_fields(&filled_reopened.id, page).unwrap()[0].value,
            FormFieldValue::Text { text: "after OCR".into() });
    }
    let plans = request(&result).pages;
    let recovery = RecoveryStore::new(dir.path().join("recovery")).unwrap();
    recovery.save(&e,json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","dirty":true,"savedDigest":"old","zoom":100,
        "scrollPosition":{"top":0,"left":0},"document":{"name":"OCR.pdf","pages":plans,"selectedPageId":"p0","selectedOverlayId":null}}]})).unwrap();
    let restored = recovery.load(&e).unwrap().unwrap();
    let id = restored["tabs"][0]["document"]["pages"][0]["sourceId"]
        .as_str()
        .unwrap();
    assert!(text(&e, id).contains("café niño"));
    assert_eq!(e.list_form_fields(id, 1).unwrap().len(), 1);
    let output = dir.path().join("saved.pdf");
    e.export_pdf(request(&result), &output).unwrap();
    let reopened = e.open_document(output).unwrap();
    assert!(reopened.pages[0].overlays.is_empty());
    assert_eq!(e.list_native_comments(&reopened.id).unwrap()[0].text,"Keep this note");
    assert_eq!(reopened.bookmarks.len(), 1);
    e.close_document(id).unwrap();
    for doc in [info, plain, result, filled, reopened, filled_reopened] {
        e.close_document(&doc.id).unwrap();
    }
}

#[test]
fn refuses_signed_xfa_and_prepared_print_sources_and_bounds_large_rasters() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    for case in ["xfa", "signature", "certified"] {
        let mut d = source(0);
        if case == "xfa" {
            let form=d.add_object(dictionary!{"Fields"=>Vec::<Object>::new(),"XFA"=>Object::string_literal("unsupported")});
            d.catalog_mut().unwrap().set("AcroForm", form);
        } else if case == "certified" {
            d.catalog_mut().unwrap().set("Perms", dictionary! {});
        } else {
            let sig=d.add_object(dictionary!{"Type"=>"Sig","ByteRange"=>vec![0.into(),1.into(),2.into(),3.into()],"Contents"=>Object::string_literal("signature")});
            let field = d.add_object(
                dictionary! {"FT"=>"Sig","T"=>Object::string_literal("signed"),"V"=>sig},
            );
            let form = d.add_object(dictionary! {"Fields"=>vec![Object::Reference(field)]});
            d.catalog_mut().unwrap().set("AcroForm", form);
        }
        let info = open(&e, dir.path(), &format!("{case}.pdf"), d);
        assert!(e.prepare_ocr_page(&info.id, 0, 0).is_err(), "{case}");
        assert!(e
            .apply_ocr(
                &info.id,
                vec![OcrUpdate {
                    preserve_owned_orientation: false,
                    page_index: 0,
                    rotation: 0,
                    page: recognized(833, 416)
                }]
            )
            .is_err());
        e.close_document(&info.id).unwrap();
    }
    let mut large = source(0);
    let ids: Vec<_> = large.get_pages().values().copied().collect();
    for id in ids {
        let page = large.get_dictionary_mut(id).unwrap();
        page.set(
            "MediaBox",
            vec![0.into(), 0.into(), 10_000.into(), 10_000.into()],
        );
        page.remove(b"CropBox");
    }
    let info = open(&e, dir.path(), "large.pdf", large);
    let prep = e.prepare_ocr_page(&info.id, 0, 0).unwrap();
    assert_eq!((prep.width, prep.height), (4472, 4472));
    assert!(prep.effective_dpi < 33.);
    assert!(prep.png.len() <= 32 * 1024 * 1024);
    e.close_document(&info.id).unwrap();
}


#[test]
fn physical_page_units_control_ocr_dpi_without_changing_raw_owned_mapping() {
    let e=engine();let dir=tempfile::tempdir().unwrap();
    for rotation in [0,90] {for unit in [0.5_f32,2.] {
        let mut pdf=source(rotation);let first=*pdf.get_pages().values().next().unwrap();pdf.get_dictionary_mut(first).unwrap().set("UserUnit",unit);
        let info=open(&e,dir.path(),&format!("units-{unit}-{rotation}.pdf"),pdf);
        let prep=e.prepare_ocr_page(&info.id,0,0).unwrap();let(w,h)=if rotation==0{(200.,100.)}else{(100.,200.)};
        assert_eq!((prep.width,prep.height),((w*unit*300./72.)as u32,(h*unit*300./72.)as u32));assert!((prep.effective_dpi-300.).abs()<2.);
        let baseline=e.render_page(&info.id,0,700).unwrap();
        let changed=e.apply_ocr(&info.id,vec![OcrUpdate{page_index:0,rotation:0,preserve_owned_orientation:false,page:recognized(prep.width,prep.height)}]).unwrap();
        assert_eq!((changed.pages[0].width,changed.pages[0].height),(w*unit,h*unit));assert_eq!(baseline,e.render_page(&changed.id,0,700).unwrap());
        let output=dir.path().join(format!("units-{unit}-{rotation}-ocr.pdf"));e.export_pdf(request(&changed),&output).unwrap();let reopened=e.open_document(&output).unwrap();let again=e.prepare_ocr_page(&reopened.id,0,0).unwrap();
        assert_eq!((again.width,again.height),(prep.width,prep.height));assert!(again.owned.is_some());assert_eq!(baseline,e.render_page(&reopened.id,0,700).unwrap());assert!(text(&e,&reopened.id).contains("café"));
        let mut owned=again.owned.unwrap();
        let no_change=e.apply_ocr(&reopened.id,vec![OcrUpdate{page_index:0,rotation:0,preserve_owned_orientation:true,page:owned.clone()}]).unwrap_err();assert!(no_change.to_string().contains("No new searchable text"));
        owned.blocks[0].paragraphs[0].lines[0].words[0].text="Corrected".into();
        let corrected=e.apply_ocr(&reopened.id,vec![OcrUpdate{page_index:0,rotation:0,preserve_owned_orientation:true,page:owned}]).unwrap();assert_eq!(baseline,e.render_page(&corrected.id,0,700).unwrap());assert!(text(&e,&corrected.id).contains("Corrected"));
        for id in [&info.id,&changed.id,&reopened.id,&corrected.id]{e.close_document(id).unwrap();}
    }}
}
