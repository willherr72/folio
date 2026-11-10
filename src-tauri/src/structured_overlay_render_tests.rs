use super::*;
use lopdf::{dictionary, Object, Stream};

pub(in crate::engine) fn runtime() -> WorkerRuntime {
    WorkerRuntime::new(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
        Arc::new(FontRegistry::new()),
    )
    .unwrap()
}
pub(in crate::engine) fn source(rt: &mut WorkerRuntime, rotation: u16, unit: f32, content: &[u8]) -> DocumentInfo {
    let mut d = Document::with_version("1.7");
    let tree = d.new_object_id();
    let content = d.add_object(Stream::new(dictionary! {}, content.to_vec()));
    let page=d.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![(-20).into(),30.into(),300.into(),450.into()],"Rotate"=>i64::from(rotation),"UserUnit"=>unit,"Resources"=>dictionary!{},"Contents"=>content});
    d.objects.insert(
        tree,
        dictionary! {"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]}.into(),
    );
    let catalog=d.add_object(dictionary!{"Type"=>"Catalog","Pages"=>tree,"SourceOnly"=>Object::string_literal("not staging content")});
    d.trailer.set("Root", catalog);
    let bytes = crate::pdf_output::save(&mut d).unwrap();
    rt.register_generated_document(bytes, "Overlay source.pdf")
        .unwrap()
}
pub(in crate::engine) fn request(rt: &WorkerRuntime, source: &DocumentInfo) -> ExportRequest {
    let mut png = Cursor::new(vec![]);
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        3,
        2,
        image::Rgba([71, 151, 31, 150]),
    ))
    .write_to(&mut png, ImageFormat::Png)
    .unwrap();
    let image = rt.images.register(png.into_inner()).unwrap();
    ExportRequest {
        flatten: false,
        pages: vec![PagePlan {
            id: "page".into(),
            source_id: source.id.clone(),
            page_index: 0,
            width: source.pages[0].width,
            height: source.pages[0].height,
            rotation: 0,
            crop: None,
            overlays: vec![
                Overlay::Text(TextOverlay {
                    id: "text".into(),
                    x: 30.,
                    y: 70.,
                    text: "Added note".into(),
                    font_size: 12.,
                    font_name: TextFont::Helvetica,
                    font_id: None,
                    shaping: None,
                    area: None,
                    color: "#223344".into(),
                    rotation: 0,
                }),
                Overlay::Highlight(HighlightOverlay {
                    id: "highlight".into(),
                    rects: vec![AnnotationRect {
                        x: 25.,
                        y: 55.,
                        width: 100.,
                        height: 22.,
                    }],
                    color: "#ffff00".into(),
                    text: Some("Marked words".into()),
                    opacity: Some(0.35),
                }),
                Overlay::Ink(InkOverlay {
                    id: "ink".into(),
                    paths: vec![vec![Point { x: 22., y: 60. }, Point { x: 140., y: 100. }]],
                    color: "#bb2244".into(),
                    stroke_width: 2.,
                }),
                Overlay::Comment(CommentOverlay {
                    id: "comment".into(),
                    x: 170.,
                    y: 100.,
                    text: "Review note".into(),
                    color: "#ffcc00".into(),
                }),
                Overlay::Image(ImageOverlay {
                    id: "image".into(),
                    asset_id: image.image.id,
                    x: 70.,
                    y: 60.,
                    width: 70.,
                    height: 45.,
                    rotation: 0,
                }),
            ],
        }],
    }
}
pub(in crate::engine) fn pixels(rt: &WorkerRuntime, bytes: &[u8], page: i32) -> (u32, u32, Vec<u8>) {
    let doc = rt
        .pdfium
        .load_pdf_from_reader(Cursor::new(bytes.to_vec()), None)
        .unwrap();
    let page = doc.pages().get(page).unwrap();
    let image = page
        .render_with_config(
            &PdfRenderConfig::new()
                .set_target_width(500)
                .render_annotations(true),
        )
        .unwrap()
        .as_image()
        .unwrap()
        .to_rgba8();
    (image.width(), image.height(), image.into_raw())
}

pub(in crate::engine) fn same_pixels(rt: &WorkerRuntime, actual: &[u8], expected: &[u8], label: &str) {
    let a = pixels(rt, actual, 0);
    let b = pixels(rt, expected, 0);
    assert_eq!((a.0, a.1), (b.0, b.1), "{label}: dimensions");
    let differences = a.2.iter().zip(&b.2).filter(|(a, b)| a != b).count();
    assert_eq!(differences, 0, "{label}: differing channels");
    if let Some(root) = std::env::var_os("FOLIO_STRUCTURED_OVERLAY_CAPTURE_DIR") {
        use std::io::Write;
        let root = PathBuf::from(root);
        std::fs::create_dir_all(&root).unwrap();
        for (suffix, bytes) in [("stage", actual), ("reference", expected)] {
            let mut output = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(root.join(format!("{label}-{suffix}.pdf")))
                .unwrap();
            output.write_all(bytes).unwrap();
        }
    }
}

#[test]
fn structured_overlay_render_cropped_image_free_page_preserves_layering_and_names() {
    let mut rt = runtime();
    let first = source(&mut rt, 90, 1.7, b"");
    let bytes = rt.document(&first.id).unwrap().bytes().unwrap();
    let mut input = Document::load_mem(&bytes).unwrap();
    let page = input.get_pages()[&1];
    input.get_dictionary_mut(page).unwrap().set(
        "CropBox",
        vec![10.into(), 50.into(), 280.into(), 400.into()],
    );
    let bytes = crate::pdf_output::save(&mut input).unwrap();
    let info = rt
        .register_generated_document(bytes, "cropped.pdf")
        .unwrap();
    let mut r = request(&rt, &info);
    r.pages[0]
        .overlays
        .retain(|o| !matches!(o, Overlay::Image(_)));
    if let Overlay::Highlight(h) = &mut r.pages[0].overlays[1] {
        h.id = "révision-段落".into();
    }
    let reference = rt
        .compose_imported_pdf(&r, &[], true, CompositionIntent::Save, None)
        .unwrap();
    let mut stage = rt.prepare_structured_overlays(&r, &mut || Ok(())).unwrap();
    let page = &stage.pages[0];
    assert_eq!(
        (
            page.geometry.left,
            page.geometry.bottom,
            page.geometry.right,
            page.geometry.top
        ),
        (10., 50., 280., 400.)
    );
    assert_eq!(page.overlays[1].overlay_id, "révision-段落");
    let paint_order = annotations(&stage.document, page.page_id)
        .unwrap()
        .into_iter()
        .map(|id| {
            lopdf::decode_text_string(
                stage
                    .document
                    .get_dictionary(id)
                    .unwrap()
                    .get(b"NM")
                    .unwrap(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(paint_order, vec!["révision-段落", "comment", "text", "ink"]);
    let bytes = crate::pdf_output::save(&mut stage.document).unwrap();
    same_pixels(&rt, &bytes, &reference, "cropped-image-free");
}

#[test]
fn structured_overlay_render_bounds_work_before_loading_or_rendering() {
    let mut rt = runtime();
    let source = source(&mut rt, 0, 1., b"");
    let mut r = request(&rt, &source);
    r.pages[0].overlays.truncate(1);
    if let Overlay::Text(text) = &mut r.pages[0].overlays[0] {
        text.text = "A\n".repeat(40_000);
    }
    // An invalid source is intentional: cumulative work admission must happen
    // before opening a page, shaping text, or building millions of PDF values.
    r.pages[0].source_id = "missing".into();
    let error = rt
        .prepare_structured_overlays(&r, &mut || Ok(()))
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("rendering work"), "{error}");
    let mut at = 0;
    let result = preflight_work(&r, &mut || {
        at += 1;
        Err(EngineError::InvalidRequest("cancel work admission".into()))
    });
    assert!(result.is_err());
    assert_eq!(at, 1);
}

#[cfg(feature = "shaped-text")]
#[test]
fn structured_overlay_render_shaped_areas_and_empty_pages_defer_workspace_changes() {
    let mut rt = runtime();
    let info = source(&mut rt, 270, 1.7, b"");
    let font = rt
        .fonts
        .register(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec())
        .unwrap();
    let mut r = request(&rt, &info);
    r.pages[0].overlays[0]=serde_json::from_value(serde_json::json!({
        "type":"text","id":"text","x":30,"y":70,"text":"office café\nsecond line",
        "fontSize":12,"fontId":font.id,"color":"#223344","rotation":90,
        "shaping":{"version":1,"direction":"ltr","ligatures":true},
        "area":{"version":1,"width":140,"height":90,"inset":3,"lineSpacing":1.2,"alignment":"center"}
    })).unwrap();
    let reference = rt
        .compose_imported_pdf(&r, &[], true, CompositionIntent::Save, None)
        .unwrap();
    // Crop/extra rotation/flatten are attachment operations. Staging always
    // produces the original frame and all editable annotation appearances.
    r.flatten = true;
    r.pages[0].rotation = 180;
    r.pages[0].crop =
        serde_json::from_value(serde_json::json!({"x":10,"y":20,"width":240,"height":200}))
            .unwrap();
    let mut empty = r.pages[0].clone();
    empty.id = "empty".into();
    empty.overlays.clear();
    r.pages.push(empty);
    let mut stage = rt.prepare_structured_overlays(&r, &mut || Ok(())).unwrap();
    assert_eq!(stage.pages.len(), 2);
    assert!(stage.pages[1].overlays.is_empty());
    assert_eq!(stage.pages[0].geometry.rotation, 270);
    assert!(annotations(&stage.document, stage.pages[1].page_id)
        .unwrap()
        .is_empty());
    assert_eq!(
        annotations(&stage.document, stage.pages[0].page_id)
            .unwrap()
            .len(),
        5
    );
    let bytes = crate::pdf_output::save(&mut stage.document).unwrap();
    same_pixels(
        &rt,
        &bytes,
        &reference,
        "shaped area with deferred page changes",
    );
    let fresh = runtime();
    let path = std::env::temp_dir().join("folio-shaped-stage.pdf");
    let mut fresh = fresh;
    let reopened = fresh
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
    let restored = reopened.pages[0]
        .overlays
        .iter()
        .find(|o| matches!(o, Overlay::Text(_)))
        .unwrap();
    assert_eq!(restored, &r.pages[0].overlays[0]);
    assert!(fresh.fonts.get(&font.id).is_ok());
}
#[test]
fn structured_overlay_render_matches_existing_all_variant_geometry() {
    let mut rt = runtime();
    for rotation in [0, 90, 180, 270] {
        for unit in [1., 1.7] {
            let source = source(&mut rt, rotation, unit, b"");
            let request = request(&rt, &source);
            let before = rt.document(&source.id).unwrap().bytes().unwrap();
            let reference = rt
                .compose_imported_pdf(&request, &[], true, CompositionIntent::Save, None)
                .unwrap();
            let mut prepared = rt
                .prepare_structured_overlays(&request, &mut || Ok(()))
                .expect("all overlay types must render without source-page import");
            assert_eq!(prepared.pages.len(), 1);
            let page = &prepared.pages[0];
            assert_eq!(page.geometry.rotation, rotation);
            assert_eq!(page.unit, unit);
            assert_eq!(
                page.overlays
                    .iter()
                    .map(|o| o.overlay_id.as_str())
                    .collect::<Vec<_>>(),
                vec!["text", "highlight", "ink", "comment", "image"]
            );
            let annots = prepared
                .document
                .get_dictionary(page.page_id)
                .unwrap()
                .get(b"Annots")
                .unwrap()
                .as_array()
                .unwrap();
            assert_eq!(annots.len(), 5);
            for binding in &page.overlays {
                assert!(annots.contains(&Object::Reference(binding.annotation_id)));
                assert_eq!(
                    prepared
                        .document
                        .get_dictionary(binding.annotation_id)
                        .unwrap()
                        .get(b"P")
                        .unwrap(),
                    &Object::Reference(page.page_id)
                );
            }
            let output = crate::pdf_output::save(&mut prepared.document).unwrap();
            same_pixels(
                &rt,
                &output,
                &reference,
                &format!("rotation-{rotation}-unit-{unit}"),
            );
            assert_eq!(
                rt.document(&source.id).unwrap().bytes().unwrap().as_ref(),
                before.as_ref()
            );
        }
    }
}
#[test]
fn structured_overlay_render_shares_assets_and_keeps_occurrences_independent() {
    let mut rt = runtime();
    let source = source(
        &mut rt,
        90,
        1.7,
        b"% ORIGINAL SOURCE MUST NOT BE COPIED\n0 0 30 30 re f",
    );
    let mut request = request(&rt, &source);
    let font = rt
        .fonts
        .register(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec())
        .unwrap();
    if let Overlay::Text(t) = &mut request.pages[0].overlays[0] {
        t.font_id = Some(font.id.clone());
    }
    let mut copy = request.pages[0].clone();
    copy.id = "copy".into();
    request.pages.push(copy);
    let mut prepared = rt
        .prepare_structured_overlays(&request, &mut || Ok(()))
        .unwrap();
    assert_eq!(prepared.pages.len(), 2);
    assert!(!prepared.document.catalog().unwrap().has(b"SourceOnly"));
    for object in prepared.document.objects.values() {
        if let Object::Stream(stream) = object {
            let content = stream
                .decompressed_content()
                .unwrap_or_else(|_| stream.content.clone());
            assert!(!content.windows(15).any(|w| w == b"ORIGINAL SOURCE"));
        }
    }
    for (a, b) in prepared.pages[0]
        .overlays
        .iter()
        .zip(&prepared.pages[1].overlays)
    {
        assert_eq!(a.overlay_id, b.overlay_id);
        assert_ne!(a.annotation_id, b.annotation_id);
    }
    let font_files = prepared
        .document
        .objects
        .values()
        .filter(|v| v.as_dict().is_ok_and(|d| d.has(b"FontFile2")))
        .count();
    assert_eq!(font_files, 1, "shared full font must embed once");
    let images = prepared
        .document
        .objects
        .values()
        .filter(|v| v.as_stream().is_ok_and(|s| s.dict.has(b"FolioImageId")))
        .count();
    assert_eq!(
        images, 1,
        "original image attachment shared across occurrences"
    );
    let bytes = crate::pdf_output::save(&mut prepared.document).unwrap();
    assert_eq!(pixels(&rt, &bytes, 0), pixels(&rt, &bytes, 1));
    let path = std::env::temp_dir().join("folio-stage-reopen.pdf");
    let reopened = rt
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
    assert_eq!(
        reopened.pages[0].overlays.len(),
        4,
        "text, ink, image and highlight reimport; comment stays native"
    );
    assert_eq!(reopened.pages[1].overlays.len(), 4);
}
#[test]
fn structured_overlay_render_refusal_and_cancellation_do_not_publish_sources() {
    let mut rt = runtime();
    let source = source(&mut rt, 0, 1., b"");
    let request = request(&rt, &source);
    let count = rt.documents.len();
    let original = rt.document(&source.id).unwrap().bytes().unwrap();
    let mut calls = 0;
    rt.prepare_structured_overlays(&request, &mut || {
        calls += 1;
        Ok(())
    })
    .unwrap();
    for stop in [1, calls / 2, calls] {
        let mut at = 0;
        let result = rt.prepare_structured_overlays(&request, &mut || {
            at += 1;
            if at == stop {
                Err(EngineError::InvalidRequest("cancel".into()))
            } else {
                Ok(())
            }
        });
        assert!(result.is_err(), "checkpoint {stop}");
    }
    let mut bad = request.clone();
    let duplicate = bad.pages[0].overlays[0].clone();
    bad.pages[0].overlays.push(duplicate);
    assert!(rt
        .prepare_structured_overlays(&bad, &mut || Ok(()))
        .is_err());
    let mut big = request.clone();
    big.pages[0].overlays = (0..17)
        .map(|i| {
            Overlay::Comment(CommentOverlay {
                id: format!("large-{i}"),
                x: 10.,
                y: 10.,
                text: "a".repeat(999_999),
                color: "#ffcc00".into(),
            })
        })
        .collect();
    let error = rt
        .prepare_structured_overlays(&big, &mut || Ok(()))
        .err()
        .expect("cumulative metadata must be bounded")
        .to_string();
    assert!(error.contains("metadata"), "{error}");
    assert_eq!(rt.documents.len(), count);
    assert_eq!(
        rt.document(&source.id).unwrap().bytes().unwrap().as_ref(),
        original.as_ref()
    );
}

fn noise_image(rt: &WorkerRuntime, side: u32, mut seed: u32) -> String {
    let mut pixels = image::RgbaImage::new(side, side);
    for pixel in pixels.pixels_mut() {
        let mut v = [0u8; 4];
        for c in &mut v {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            *c = (seed >> 24) as u8;
        }
        *pixel = image::Rgba(v);
    }
    let mut png = Cursor::new(vec![]);
    image::DynamicImage::ImageRgba8(pixels)
        .write_to(&mut png, ImageFormat::Png)
        .unwrap();
    rt.images.register(png.into_inner()).unwrap().image.id
}

#[test]
fn structured_overlay_render_rejects_large_assets_before_installing_them() {
    let mut rt = runtime();
    let info = source(&mut rt, 0, 1., b"");
    let font = rt
        .fonts
        .register(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec())
        .unwrap();
    let image = noise_image(&rt, 64, 7);
    for kind in ["font", "image"] {
        let mut r = request(&rt, &info);
        if kind == "font" {
            r.pages[0].overlays.truncate(1);
            if let Overlay::Text(t) = &mut r.pages[0].overlays[0] {
                t.font_id = Some(font.id.clone());
            }
        } else {
            let mut overlay = r.pages[0].overlays.pop().unwrap();
            if let Overlay::Image(v) = &mut overlay {
                v.asset_id = image.clone();
            }
            r.pages[0].overlays = vec![overlay];
        }
        let mut doc = Document::load_mem(rt.document(&info.id).unwrap().bytes().unwrap().as_ref()).unwrap();
        let original = doc.objects.clone();
        let geometry = page_geometry(
            &rt.document(&info.id)
                .unwrap()
                .pdf().unwrap()
                .pages()
                .get(0)
                .unwrap(),
        )
        .unwrap();
        let result = persistence::finalize_generated(
            &mut doc,
            &r,
            &[geometry],
            &rt.fonts,
            &rt.images,
            persistence::GeneratedLimits {
                bytes: 4096,
                ..Default::default()
            },
            |_, _| Ok(()),
        );
        let error = result
            .err()
            .unwrap_or_else(|| panic!("{kind} bypassed the generation resource budget"))
            .to_string();
        assert!(
            error.contains("generated overlay resources"),
            "{kind}: {error}"
        );
        assert_eq!(
            doc.objects, original,
            "{kind} was installed before admission"
        );
    }
}

#[test]
fn structured_overlay_render_admission_keeps_shared_fonts_and_images_within_budget() {
    let mut rt = runtime();
    let info = source(&mut rt, 0, 1., b"");
    let font = rt
        .fonts
        .register(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec())
        .unwrap();
    let mut r = request(&rt, &info);
    let mut image = r.pages[0].overlays.last().unwrap().clone();
    if let Overlay::Image(v) = &mut image {
        v.asset_id = noise_image(&rt, 256, 42);
    }
    r.pages[0].overlays.truncate(1);
    if let Overlay::Text(t) = &mut r.pages[0].overlays[0] {
        t.font_id = Some(font.id.clone());
    }
    let first = r.pages[0].overlays[0].clone();
    r.pages[0].overlays.push(image.clone());
    for i in 1..12 {
        let mut o = first.clone();
        if let Overlay::Text(t) = &mut o {
            t.id = format!("text-{i}");
            t.y += i as f32 * 12.;
        }
        r.pages[0].overlays.push(o);
        let mut copy = image.clone();
        if let Overlay::Image(v) = &mut copy {
            v.id = format!("image-{i}");
        }
        r.pages[0].overlays.push(copy);
    }
    let mut doc = Document::load_mem(rt.document(&info.id).unwrap().bytes().unwrap().as_ref()).unwrap();
    let geometry = page_geometry(
        &rt.document(&info.id)
            .unwrap()
            .pdf().unwrap()
            .pages()
            .get(0)
            .unwrap(),
    )
    .unwrap();
    persistence::finalize_generated(
        &mut doc,
        &r,
        &[geometry],
        &rt.fonts,
        &rt.images,
        persistence::GeneratedLimits {
            bytes: 2 * 1024 * 1024,
            ..Default::default()
        },
        |_, _| Ok(()),
    )
    .unwrap();
    assert_eq!(
        doc.objects
            .values()
            .filter(|v| v.as_dict().is_ok_and(|d| d.has(b"FontFile2")))
            .count(),
        1
    );
    assert_eq!(
        doc.objects
            .values()
            .filter(|v| v.as_stream().is_ok_and(|s| s.dict.has(b"FolioImageId")))
            .count(),
        1
    );
    assert_eq!(annotations(&doc, doc.get_pages()[&1]).unwrap().len(), 24);
}

#[test]
fn structured_overlay_render_refuses_second_unique_image_before_copying_it() {
    let mut rt = runtime();
    let info = source(&mut rt, 0, 1., b"");
    let mut r = request(&rt, &info);
    let first = noise_image(&rt, 256, 7);
    let second = noise_image(&rt, 256, 8);
    let mut image = r.pages[0].overlays.pop().unwrap();
    if let Overlay::Image(v) = &mut image {
        v.asset_id = first.clone();
    }
    let mut copy = image.clone();
    if let Overlay::Image(v) = &mut copy {
        v.id = "second".into();
        v.asset_id = second.clone();
    }
    r.pages[0].overlays = vec![image, copy];
    let mut doc = Document::load_mem(rt.document(&info.id).unwrap().bytes().unwrap().as_ref()).unwrap();
    let geometry = page_geometry(
        &rt.document(&info.id)
            .unwrap()
            .pdf().unwrap()
            .pages()
            .get(0)
            .unwrap(),
    )
    .unwrap();
    let error = persistence::finalize_generated(
        &mut doc,
        &r,
        &[geometry],
        &rt.fonts,
        &rt.images,
        persistence::GeneratedLimits {
            bytes: 800 * 1024,
            ..Default::default()
        },
        |_, _| Ok(()),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("generated overlay resources"), "{error}");
    let installed = doc
        .objects
        .values()
        .filter_map(|v| {
            v.as_stream()
                .ok()?
                .dict
                .get(b"FolioImageId")
                .ok()?
                .as_str()
                .ok()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        installed,
        vec![first.as_bytes()],
        "second original image must not be copied after cumulative refusal"
    );
    assert_eq!(
        rt.documents.len(),
        1,
        "private refusal must not publish runtime sources"
    );
}

#[test]
fn structured_overlay_render_refuses_font_complexity_before_installation() {
    let mut rt = runtime();
    let info = source(&mut rt, 0, 1., b"");
    let mut r = request(&rt, &info);
    r.pages[0].overlays.truncate(1);
    let font = rt
        .fonts
        .register(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec())
        .unwrap();
    if let Overlay::Text(t) = &mut r.pages[0].overlays[0] {
        t.font_id = Some(font.id);
    }
    let geometry = page_geometry(
        &rt.document(&info.id)
            .unwrap()
            .pdf().unwrap()
            .pages()
            .get(0)
            .unwrap(),
    )
    .unwrap();
    for (label, limits) in [
        (
            "objects",
            persistence::GeneratedLimits {
                objects: 8,
                ..Default::default()
            },
        ),
        (
            "values",
            persistence::GeneratedLimits {
                values: 100,
                ..Default::default()
            },
        ),
        (
            "stream",
            persistence::GeneratedLimits {
                stream_bytes: 4096,
                ..Default::default()
            },
        ),
        (
            "depth",
            persistence::GeneratedLimits {
                depth: 3,
                ..Default::default()
            },
        ),
    ] {
        let mut doc = Document::load_mem(rt.document(&info.id).unwrap().bytes().unwrap().as_ref()).unwrap();
        let before = doc.objects.clone();
        let error = persistence::finalize_generated(
            &mut doc,
            &r,
            &[geometry],
            &rt.fonts,
            &rt.images,
            limits,
            |_, _| Ok(()),
        )
        .unwrap_err()
        .to_string();
        assert!(
            error.contains("generated overlay resources"),
            "{label}: {error}"
        );
        assert_eq!(doc.objects, before, "{label} admission mutated the graph");
    }
}

#[test]
fn structured_overlay_render_work_admits_one_full_supported_shaped_input() {
    let mut rt = runtime();
    let info = source(&mut rt, 0, 1., b"");
    let mut r = request(&rt, &info);
    r.pages[0].overlays.truncate(1);
    if let Overlay::Text(t) = &mut r.pages[0].overlays[0] {
        t.text = "a".repeat(4096);
        t.shaping = Some(TextShaping {
            version: 1,
            direction: ShapingDirection::Ltr,
            ligatures: true,
        });
    }
    preflight_work(&r, &mut || Ok(()))
        .expect("admission must not narrow the existing 4096-scalar shaping contract");
}
