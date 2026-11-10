use folio_engine::{ExportRequest, ImageOverlay, Overlay, PagePlan, PdfEngine};
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use std::{io::Cursor, path::PathBuf};
static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}
fn image_bytes(red: u8) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(RgbaImage::from_pixel(4, 2, Rgba([red, 0, 0, 128])))
        .write_to(&mut bytes, ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}
fn plan(source: &folio_engine::DocumentInfo, overlay: ImageOverlay) -> PagePlan {
    PagePlan {
        crop: None,
        id: "page".into(),
        source_id: source.id.clone(),
        page_index: 0,
        width: source.pages[0].width,
        height: source.pages[0].height,
        rotation: 0,
        overlays: vec![Overlay::Image(overlay)],
    }
}

#[test]
fn image_overlay_save_reopen_alpha_and_source_leases() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    let source = engine.create_blank_document(200., 300.).unwrap();
    let bytes = image_bytes(201);
    let acquisition = engine.images().register(bytes.clone()).unwrap();
    let overlay = ImageOverlay {
        id: "photo".into(),
        asset_id: acquisition.image.id.clone(),
        x: 50.,
        y: 100.,
        width: 80.,
        height: 40.,
        rotation: 0,
    };
    let page = plan(&source, overlay.clone());
    let preview = image::load_from_memory(&engine.render_page_plan(page.clone(), 200).unwrap())
        .unwrap()
        .into_rgb8();
    let color = preview.get_pixel(90, 120).0;
    assert!(
        color[0].abs_diff(228) <= 2 && color[1].abs_diff(127) <= 2 && color[2].abs_diff(127) <= 2,
        "{color:?}"
    );
    let saved = temp.path().join("editable.pdf");
    engine
        .export_pdf(
            ExportRequest {
                pages: vec![page],
                flatten: false,
            },
            &saved,
        )
        .unwrap();
    engine.images().release(&acquisition.token).unwrap();
    let reopened = engine.open_document(&saved).unwrap();
    assert_eq!(
        reopened.pages[0].overlays,
        vec![Overlay::Image(overlay.clone())]
    );
    assert_eq!(
        engine.images().get(&overlay.asset_id).unwrap().bytes(),
        bytes
    );
    let another = engine.open_document(&saved).unwrap();
    let retained = engine.images().acquire(&overlay.asset_id).unwrap();
    engine.close_document(&reopened.id).unwrap();
    assert!(engine.images().get(&overlay.asset_id).is_ok());
    engine.images().release(&retained.token).unwrap();
    assert!(engine.images().get(&overlay.asset_id).is_ok());
    engine.close_document(&another.id).unwrap();
    assert!(engine.images().get(&overlay.asset_id).is_err());
    engine.close_document(&source.id).unwrap();
}

fn cropped_source(path: &std::path::Path, rotation: u16) {
    use lopdf::{dictionary, Document, Stream};
    let mut doc = Document::with_version("1.7");
    let pages = doc.new_object_id();
    let font =
        doc.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let contents=doc.add_object(Stream::new(dictionary! {},b"q 0 0 1 rg 25 35 200 300 re f Q BT /F1 12 Tf 1 1 1 rg 35 315 Td (Original native text) Tj ET".to_vec()));
    let page=doc.add_object(dictionary! {"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),250.into(),370.into()],
        "CropBox"=>vec![25.into(),35.into(),225.into(),335.into()],"Rotate"=>rotation as i64,
        "Resources"=>dictionary! {"Font"=>dictionary! {"F1"=>font}},"Contents"=>contents});
    doc.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into(),
    );
    let root = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    doc.trailer.set("Root", root);
    doc.save(path).unwrap();
}
fn rotated_point(x: f32, y: f32, w: f32, h: f32, r: u16) -> (f32, f32) {
    match r {
        90 => (h - y, x),
        180 => (w - x, h - y),
        270 => (y, w - x),
        _ => (x, y),
    }
}
fn artifact(name: &str, bytes: &[u8]) {
    if let Some(root) = std::env::var_os("FOLIO_IMAGE_OVERLAY_OUTPUT") {
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(PathBuf::from(root).join(name), bytes).unwrap();
    }
}

#[test]
fn images_roundtrip_all_crop_page_workspace_and_image_rotations() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    let acquisition = engine.images().register(image_bytes(204)).unwrap();
    for intrinsic in [0, 90, 180, 270] {
        let path = temp.path().join(format!("source-{intrinsic}.pdf"));
        cropped_source(&path, intrinsic);
        let original = std::fs::read(&path).unwrap();
        artifact(&format!("source-{intrinsic}.pdf"), &original);
        let source = engine.open_document(&path).unwrap();
        let (w, h) = (source.pages[0].width, source.pages[0].height);
        for workspace in [0, 90, 180, 270] {
            for rotation in [0, 90, 180, 270] {
                let overlay = ImageOverlay {
                    id: "photo".into(),
                    asset_id: acquisition.image.id.clone(),
                    x: w / 2.,
                    y: h / 2.,
                    width: 40.,
                    height: 20.,
                    rotation,
                };
                let mut page = plan(&source, overlay.clone());
                page.rotation = workspace;
                let output_width = if workspace == 90 || workspace == 270 {
                    h
                } else {
                    w
                } as u32;
                let preview = engine.render_page_plan(page.clone(), output_width).unwrap();
                let raster = image::load_from_memory(&preview).unwrap().into_rgb8();
                let offset = match rotation {
                    90 => (-10., 20.),
                    180 => (-20., -10.),
                    270 => (10., -20.),
                    _ => (20., 10.),
                };
                let center =
                    rotated_point(overlay.x + offset.0, overlay.y + offset.1, w, h, workspace);
                let color = raster.get_pixel(center.0 as u32, center.1 as u32).0;
                assert!(
                    color[0].abs_diff(102) <= 2 && color[1] <= 2 && color[2].abs_diff(127) <= 2,
                    "{intrinsic}/{workspace}/{rotation}: {color:?}"
                );
                let output = temp.path().join("saved.pdf");
                engine
                    .export_pdf(
                        ExportRequest {
                            pages: vec![page],
                            flatten: false,
                        },
                        &output,
                    )
                    .unwrap();
                artifact(
                    &format!("image-{intrinsic}-{workspace}-{rotation}.pdf"),
                    &std::fs::read(&output).unwrap(),
                );
                let reopened = engine.open_document(&output).unwrap();
                let expected_anchor = rotated_point(overlay.x, overlay.y, w, h, workspace);
                let expected = ImageOverlay {
                    x: expected_anchor.0,
                    y: expected_anchor.1,
                    rotation: (rotation + workspace) % 360,
                    ..overlay
                };
                assert_eq!(
                    reopened.pages[0].overlays,
                    vec![Overlay::Image(expected.clone())]
                );
                assert!(engine
                    .extract_text(&reopened.id, 0)
                    .unwrap()
                    .contains("Original native text"));
                let second = engine
                    .render_page_plan(plan(&reopened, expected), output_width)
                    .unwrap();
                assert_eq!(
                    image::load_from_memory(&second).unwrap().into_rgb8(),
                    raster,
                    "{intrinsic}/{workspace}/{rotation}"
                );
                engine.close_document(&reopened.id).unwrap();
            }
        }
        assert_eq!(std::fs::read(&path).unwrap(), original);
        engine.close_document(&source.id).unwrap();
    }
    engine.images().release(&acquisition.token).unwrap();
}

#[test]
fn tampered_image_appearance_or_attachment_remains_native_without_publication() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    use lopdf::{Document, Object};
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    let source = engine.create_blank_document(200., 300.).unwrap();
    let acquisition = engine.images().register(image_bytes(205)).unwrap();
    let overlay = ImageOverlay {
        id: "photo".into(),
        asset_id: acquisition.image.id.clone(),
        x: 50.,
        y: 100.,
        width: 80.,
        height: 40.,
        rotation: 0,
    };
    let saved = temp.path().join("saved.pdf");
    engine
        .export_pdf(
            ExportRequest {
                pages: vec![plan(&source, overlay.clone())],
                flatten: false,
            },
            &saved,
        )
        .unwrap();
    engine.images().release(&acquisition.token).unwrap();
    let original = std::fs::read(&saved).unwrap();
    for alteration in 0..4 {
        let mut doc = Document::load_mem(&original).unwrap();
        let page = *doc.get_pages().values().next().unwrap();
        let annotation = doc
            .get_dictionary(page)
            .unwrap()
            .get(b"Annots")
            .unwrap()
            .as_array()
            .unwrap()[0]
            .as_reference()
            .unwrap();
        let file = doc
            .get_dictionary(annotation)
            .unwrap()
            .get(b"FolioImageAsset")
            .unwrap()
            .as_reference()
            .unwrap();
        let ap = doc
            .get_dictionary(annotation)
            .unwrap()
            .get(b"AP")
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"N")
            .unwrap()
            .as_reference()
            .unwrap();
        match alteration {
            0 => {
                doc.get_object_mut(file)
                    .unwrap()
                    .as_stream_mut()
                    .unwrap()
                    .content[20] ^= 1
            }
            1 => doc
                .get_object_mut(ap)
                .unwrap()
                .as_stream_mut()
                .unwrap()
                .dict
                .get_mut(b"Resources")
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set("Extra", true),
            2 => {
                let dictionary = doc.get_dictionary_mut(annotation).unwrap();
                let mut metadata: serde_json::Value =
                    serde_json::from_slice(dictionary.get(b"Folio").unwrap().as_str().unwrap())
                        .unwrap();
                metadata["overlay"]["width"] = 90.into();
                dictionary.set(
                    "Folio",
                    Object::string_literal(serde_json::to_vec(&metadata).unwrap()),
                );
            }
            _ => doc
                .get_dictionary_mut(annotation)
                .unwrap()
                .set("FolioImageAsset", Object::Reference(ap)),
        }
        let path = temp.path().join(format!("changed-{alteration}.pdf"));
        doc.save(&path).unwrap();
        let before = std::fs::read(&path).unwrap();
        let reopened = engine.open_document(&path).unwrap();
        assert!(reopened.pages[0].overlays.is_empty());
        assert!(engine.images().get(&overlay.asset_id).is_err());
        let pixels = image::load_from_memory(&engine.render_page(&reopened.id, 0, 200).unwrap())
            .unwrap()
            .into_rgb8();
        assert!(pixels.get_pixel(90, 120).0[1] < 200);
        assert_eq!(std::fs::read(&path).unwrap(), before);
        engine.close_document(&reopened.id).unwrap();
    }
    engine.close_document(&source.id).unwrap();
}

#[test]
fn flattened_image_is_native_and_invalid_geometry_does_not_publish_output() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    let source = engine.create_blank_document(200., 300.).unwrap();
    let acquisition = engine.images().register(image_bytes(202)).unwrap();
    let overlay = ImageOverlay {
        id: "photo".into(),
        asset_id: acquisition.image.id.clone(),
        x: 100.,
        y: 100.,
        width: 40.,
        height: 20.,
        rotation: 90,
    };
    let flat = temp.path().join("flat.pdf");
    engine
        .export_pdf(
            ExportRequest {
                pages: vec![plan(&source, overlay.clone())],
                flatten: true,
            },
            &flat,
        )
        .unwrap();
    let opened = engine.open_document(&flat).unwrap();
    assert!(opened.pages[0].overlays.is_empty());
    let rendered = image::load_from_memory(&engine.render_page(&opened.id, 0, 200).unwrap())
        .unwrap()
        .into_rgb8();
    assert!(rendered.get_pixel(90, 120).0[1] < 200);
    let invalid = ImageOverlay { x: 0., ..overlay };
    let output = temp.path().join("invalid.pdf");
    assert!(engine
        .export_pdf(
            ExportRequest {
                pages: vec![plan(&source, invalid)],
                flatten: false
            },
            &output
        )
        .is_err());
    assert!(!output.exists());
    engine.close_document(&opened.id).unwrap();
    engine.close_document(&source.id).unwrap();
    engine.images().release(&acquisition.token).unwrap();
}

#[test]
fn image_stamps_respect_annotation_and_flattening_permissions() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/protection");
    let source = engine
        .open_document_with_password(root.join("aes_256.pdf"), "reader")
        .unwrap();
    let rights = &source.protection.as_ref().unwrap().permissions;
    assert!(rights.annotate && !rights.edit_content);
    let acquisition = engine.images().register(image_bytes(203)).unwrap();
    let overlay = ImageOverlay {
        id: "photo".into(),
        asset_id: acquisition.image.id.clone(),
        x: 50.,
        y: 100.,
        width: 40.,
        height: 20.,
        rotation: 0,
    };
    let editable = temp.path().join("protected.pdf");
    engine
        .export_pdf(
            ExportRequest {
                pages: vec![plan(&source, overlay.clone())],
                flatten: false,
            },
            &editable,
        )
        .unwrap();
    let reopened = engine
        .open_document_with_password(&editable, "reader")
        .unwrap();
    assert_eq!(
        reopened.pages[0].overlays,
        vec![Overlay::Image(overlay.clone())]
    );
    let flat = temp.path().join("forbidden.pdf");
    assert!(matches!(
        engine.export_pdf(
            ExportRequest {
                pages: vec![plan(&source, overlay.clone())],
                flatten: true
            },
            &flat
        ),
        Err(folio_engine::EngineError::PermissionDenied(_))
    ));
    assert!(!flat.exists());
    let blocked = engine
        .open_document_with_password(root.join("edit_only_aes_256.pdf"), "reader")
        .unwrap();
    assert!(matches!(
        engine.export_pdf(
            ExportRequest {
                pages: vec![plan(&blocked, overlay)],
                flatten: false
            },
            temp.path().join("no-annotations.pdf")
        ),
        Err(folio_engine::EngineError::PermissionDenied(_))
    ));
    for source in [source, reopened, blocked] {
        engine.close_document(&source.id).unwrap();
    }
    engine.images().release(&acquisition.token).unwrap();
}

#[test]
fn images_text_and_ink_retain_drawing_order_in_editable_and_flattened_output() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    let source = engine.create_blank_document(200., 300.).unwrap();
    let acquisition = engine.images().register(image_bytes(206)).unwrap();
    let overlay = ImageOverlay {
        id: "photo".into(),
        asset_id: acquisition.image.id.clone(),
        x: 40.,
        y: 40.,
        width: 40.,
        height: 40.,
        rotation: 0,
    };
    let ink:Overlay=serde_json::from_value(serde_json::json!({"type":"ink","id":"ink","color":"#0000ff","strokeWidth":10,"paths":[[{"x":30,"y":60},{"x":90,"y":60}]]})).unwrap();
    let text:Overlay=serde_json::from_value(serde_json::json!({"type":"text","id":"text","x":20,"y":20,"text":"Overlay text","fontSize":12,"color":"#000000"})).unwrap();
    for flatten in [false, true] {
        for image_last in [false, true] {
            let mut page = plan(&source, overlay.clone());
            let image = Overlay::Image(overlay.clone());
            page.overlays = if image_last {
                vec![text.clone(), ink.clone(), image]
            } else {
                vec![text.clone(), image, ink.clone()]
            };
            let path = temp
                .path()
                .join(format!("order-{flatten}-{image_last}.pdf"));
            engine
                .export_pdf(
                    ExportRequest {
                        pages: vec![page],
                        flatten,
                    },
                    &path,
                )
                .unwrap();
            // Recompose imported overlays, or render flattened page content directly.
            let reopened = engine.open_document(&path).unwrap();
            let page = PagePlan {
                crop: None,
                id: "p".into(),
                source_id: reopened.id.clone(),
                page_index: 0,
                width: 200.,
                height: 300.,
                rotation: 0,
                overlays: reopened.pages[0].overlays.clone(),
            };
            let rendered = image::load_from_memory(&engine.render_page_plan(page, 200).unwrap())
                .unwrap()
                .into_rgb8();
            let expected = if image_last {
                [103, 0, 127]
            } else {
                [0, 0, 255]
            };
            let actual = rendered.get_pixel(60, 60).0;
            assert!(
                actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
                "flatten={flatten}, image_last={image_last}: {actual:?}"
            );
            if flatten {
                assert!(reopened.pages[0].overlays.is_empty());
                assert!(engine
                    .extract_text(&reopened.id, 0)
                    .unwrap()
                    .contains("Overlay text"));
            } else {
                assert_eq!(reopened.pages[0].overlays.len(), 3);
            }
            artifact(
                &format!("order-{flatten}-{image_last}.pdf"),
                &std::fs::read(&path).unwrap(),
            );
            engine.close_document(&reopened.id).unwrap();
        }
    }
    engine.close_document(&source.id).unwrap();
    engine.images().release(&acquisition.token).unwrap();
}

#[test]
fn jpeg_corpus_and_sixteen_bit_png_save_reopen_and_share_resources() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    use lopdf::Document;
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    let source = engine.create_blank_document(200., 300.).unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/image-assets");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("manifest.json")).unwrap()).unwrap();
    let mut sources = manifest["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| {
            let name = case["name"].as_str().unwrap();
            (
                name.to_owned(),
                std::fs::read(root.join(format!("{name}.jpg"))).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let mut png = Cursor::new(Vec::new());
    DynamicImage::ImageRgba16(image::ImageBuffer::from_pixel(
        4,
        2,
        Rgba([65535, 8193, 0, 32769]),
    ))
    .write_to(&mut png, ImageFormat::Png)
    .unwrap();
    sources.push(("rgba16".into(), png.into_inner()));
    for (name, bytes) in sources {
        let acquisition = engine.images().register(bytes.clone()).unwrap();
        let overlay = ImageOverlay {
            id: "one".into(),
            asset_id: acquisition.image.id.clone(),
            x: 20.,
            y: 40.,
            width: 80.,
            height: 40.,
            rotation: 0,
        };
        let mut page = plan(&source, overlay.clone());
        page.overlays.push(Overlay::Image(ImageOverlay {
            id: "two".into(),
            y: 100.,
            ..overlay
        }));
        let request = ExportRequest {
            pages: vec![page.clone()],
            flatten: false,
        };
        let preview = engine.render_page_plan(page, 200).unwrap();
        let path = temp.path().join(format!("roundtrip-{name}.pdf"));
        engine.export_pdf(request.clone(), &path).unwrap();
        let reopened = engine.open_document(&path).unwrap();
        assert_eq!(
            reopened.pages[0].overlays, request.pages[0].overlays,
            "{name}"
        );
        assert_eq!(
            engine.images().get(&acquisition.image.id).unwrap().bytes(),
            bytes
        );
        let pdf = Document::load(&path).unwrap();
        let annotations = pdf
            .get_dictionary(*pdf.get_pages().values().next().unwrap())
            .unwrap()
            .get(b"Annots")
            .unwrap()
            .as_array()
            .unwrap();
        let first = pdf
            .get_dictionary(annotations[0].as_reference().unwrap())
            .unwrap();
        let second = pdf
            .get_dictionary(annotations[1].as_reference().unwrap())
            .unwrap();
        assert_eq!(
            first
                .get(b"FolioImageAsset")
                .unwrap()
                .as_reference()
                .unwrap(),
            second
                .get(b"FolioImageAsset")
                .unwrap()
                .as_reference()
                .unwrap()
        );
        let image_ref = |annotation: &lopdf::Dictionary| {
            let ap = annotation
                .get(b"AP")
                .unwrap()
                .as_dict()
                .unwrap()
                .get(b"N")
                .unwrap()
                .as_reference()
                .unwrap();
            pdf.get_object(ap)
                .unwrap()
                .as_stream()
                .unwrap()
                .dict
                .get(b"Resources")
                .unwrap()
                .as_dict()
                .unwrap()
                .get(b"XObject")
                .unwrap()
                .as_dict()
                .unwrap()
                .get(b"FolioImage")
                .unwrap()
                .as_reference()
                .unwrap()
        };
        assert_eq!(image_ref(first), image_ref(second));
        artifact(
            &format!("roundtrip-{name}.pdf"),
            &std::fs::read(&path).unwrap(),
        );
        artifact(&format!("roundtrip-{name}-pdfium.png"), &preview);
        engine.close_document(&reopened.id).unwrap();
        engine.images().release(&acquisition.token).unwrap();
    }
    engine.close_document(&source.id).unwrap();
}

#[test]
fn failed_source_open_at_image_or_font_capacity_preserves_existing_owners() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    let source = engine.create_blank_document(200., 300.).unwrap();
    let font_bytes = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf"),
    )
    .unwrap();
    let font = engine.fonts().register(font_bytes.clone()).unwrap();
    let acquisition = engine.images().register(image_bytes(207)).unwrap();
    let overlay = ImageOverlay {
        id: "photo".into(),
        asset_id: acquisition.image.id.clone(),
        x: 40.,
        y: 40.,
        width: 40.,
        height: 20.,
        rotation: 0,
    };
    let mut page = plan(&source, overlay);
    page.overlays.push(serde_json::from_value(serde_json::json!({"type":"text","id":"custom","x":20,"y":100,"text":"Hello","fontSize":12,"fontId":font.id,"color":"#000000"})).unwrap());
    let path = temp.path().join("both-assets.pdf");
    engine
        .export_pdf(
            ExportRequest {
                pages: vec![page],
                flatten: false,
            },
            &path,
        )
        .unwrap();
    engine.images().release(&acquisition.token).unwrap();
    engine.fonts().remove(&font.id).unwrap();
    assert!(engine.images().ids().unwrap().is_empty());
    let owners = (0..64)
        .map(|value| engine.images().register(image_bytes(value)).unwrap())
        .collect::<Vec<_>>();
    let full = engine.images().ids().unwrap();
    let fonts_before = engine.fonts().ids().unwrap();
    assert!(engine.open_document(&path).is_err());
    assert_eq!(engine.images().ids().unwrap(), full);
    assert_eq!(engine.fonts().ids().unwrap(), fonts_before);
    engine.images().release_all(&owners).unwrap();
    let retained = engine.images().register(image_bytes(208)).unwrap();
    let retained_ids = engine.images().ids().unwrap();
    for index in 0..(64 - fonts_before.len()) {
        let mut bytes = font_bytes.clone();
        bytes.extend_from_slice(format!("image-open-capacity-{index}").as_bytes());
        engine.fonts().register(bytes).unwrap();
    }
    let full_fonts = engine.fonts().ids().unwrap();
    assert!(engine.open_document(&path).is_err());
    assert_eq!(engine.images().ids().unwrap(), retained_ids);
    assert_eq!(engine.fonts().ids().unwrap(), full_fonts);
    for id in full_fonts.difference(&fonts_before) {
        engine.fonts().remove(id).unwrap();
    }
    let reopened = engine.open_document(&path).unwrap();
    assert_eq!(reopened.pages[0].overlays.len(), 2);
    assert!(engine.images().get(&acquisition.image.id).is_ok());
    engine.close_document(&reopened.id).unwrap();
    assert_eq!(engine.images().ids().unwrap(), retained_ids);
    engine.fonts().remove(&font.id).unwrap();
    engine.images().release(&retained.token).unwrap();
    engine.close_document(&source.id).unwrap();
}

#[test]
fn image_highlight_compositing_agrees_between_editable_and_flattened_output() {
    let _guard = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    let source = engine.create_blank_document(200., 300.).unwrap();
    let acquisition = engine.images().register(image_bytes(209)).unwrap();
    let overlay = ImageOverlay {
        id: "photo".into(),
        asset_id: acquisition.image.id.clone(),
        x: 40.,
        y: 40.,
        width: 40.,
        height: 40.,
        rotation: 0,
    };
    let mut page = plan(&source, overlay);
    page.overlays.push(serde_json::from_value(serde_json::json!({"type":"highlight","id":"highlight","rects":[{"x":40,"y":40,"width":40,"height":40}],"color":"#00ffff","opacity":0.4})).unwrap());
    let editable = image::load_from_memory(&engine.render_page_plan(page.clone(), 200).unwrap())
        .unwrap()
        .into_rgb8();
    page.overlays.reverse();
    let reversed = image::load_from_memory(&engine.render_page_plan(page.clone(), 200).unwrap())
        .unwrap()
        .into_rgb8();
    assert_eq!(
        editable, reversed,
        "Annotation markup stays above the content layer regardless of insertion order"
    );
    let editable_path = temp.path().join("highlight-editable.pdf");
    engine
        .export_pdf(
            ExportRequest {
                pages: vec![page.clone()],
                flatten: false,
            },
            &editable_path,
        )
        .unwrap();
    artifact(
        "highlight-editable.pdf",
        &std::fs::read(&editable_path).unwrap(),
    );
    let path = temp.path().join("highlight-flat.pdf");
    engine
        .export_pdf(
            ExportRequest {
                pages: vec![page],
                flatten: true,
            },
            &path,
        )
        .unwrap();
    artifact("highlight-flat.pdf", &std::fs::read(&path).unwrap());
    let flat = engine.open_document(&path).unwrap();
    let composed = PagePlan {
        crop: None,
        id: "flat".into(),
        source_id: flat.id.clone(),
        page_index: 0,
        width: 200.,
        height: 300.,
        rotation: 0,
        overlays: flat.pages[0].overlays.clone(),
    };
    let flattened = image::load_from_memory(&engine.render_page_plan(composed, 200).unwrap())
        .unwrap()
        .into_rgb8();
    assert_eq!(editable.get_pixel(60, 60), flattened.get_pixel(60, 60));
    engine.close_document(&flat.id).unwrap();
    engine.close_document(&source.id).unwrap();
    engine.images().release(&acquisition.token).unwrap();
}
