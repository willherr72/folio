use folio_engine::{ExportRequest, InkOverlay, Overlay, PagePlan, PdfEngine, Point, TextOverlay};
use image::GenericImageView;
use std::fs;
use std::path::{Path, PathBuf};

fn pdfium_path() -> PathBuf {
    std::env::var_os("FOLIO_PDFIUM_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")
        })
}

fn escape_pdf_text(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

fn write_pdf(path: &Path, pages: &[(&str, [f32; 4], Option<[f32; 4]>, u16)]) {
    let font_id = 3 + pages.len() * 2;
    let kids = (0..pages.len())
        .map(|index| format!("{} 0 R", 3 + index * 2))
        .collect::<Vec<_>>()
        .join(" ");
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids, pages.len()),
    ];

    for (index, (label, media, crop, rotation)) in pages.iter().enumerate() {
        let content_id = 4 + index * 2;
        let visible = (*crop).unwrap_or(*media);
        let crop = crop
            .map(|b| format!(" /CropBox [{} {} {} {}]", b[0], b[1], b[2], b[3]))
            .unwrap_or_default();
        let rotate = if *rotation == 0 {
            String::new()
        } else {
            format!(" /Rotate {rotation}")
        };
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [{} {} {} {}]{}{} /Resources << /Font << /F1 {} 0 R >> >> /Contents {} 0 R >>",
            media[0], media[1], media[2], media[3], crop, rotate, font_id, content_id
        ));
        let stream = format!(
            "BT /F1 20 Tf {} {} Td ({}) Tj ET",
            visible[0] + 18.0,
            visible[3] - 35.0,
            escape_pdf_text(label)
        );
        objects.push(format!(
            "<< /Length {} >>\nstream\n{}\nendstream",
            stream.len(),
            stream
        ));
    }
    objects.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string());

    let mut pdf = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = vec![0usize];
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", index + 1, object).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets.iter().skip(1) {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    fs::write(path, pdf).unwrap();
}

#[test]
fn export_roundtrip_preserves_pages_and_places_overlays_on_rotated_crop() {
    let temp = tempfile::tempdir().unwrap();
    let source_a = temp.path().join("alpha.pdf");
    let source_b = temp.path().join("beta.pdf");
    let output = temp.path().join("edited.pdf");
    write_pdf(
        &source_a,
        &[
            ("FIRST", [0.0, 0.0, 200.0, 300.0], None, 0),
            (
                "ROTATED",
                [0.0, 0.0, 300.0, 400.0],
                Some([40.0, 50.0, 240.0, 350.0]),
                90,
            ),
        ],
    );
    write_pdf(&source_b, &[("BETA", [0.0, 0.0, 320.0, 180.0], None, 0)]);

    let engine = PdfEngine::start(pdfium_path()).unwrap();
    assert_eq!(engine.status().unwrap(), "Native PDF engine ready");
    let alpha = engine.open_document(&source_a).unwrap();
    let beta = engine.open_document(&source_b).unwrap();
    assert_eq!(
        (alpha.pages[1].width, alpha.pages[1].height),
        (300.0, 200.0)
    );
    assert!(engine
        .extract_text(&alpha.id, 1)
        .unwrap()
        .contains("ROTATED"));

    engine
        .export_pdf(
            ExportRequest {
                flatten: true,
                pages: vec![
                    PagePlan {
                        id: "beta-page".into(),
                        source_id: beta.id.clone(),
                        page_index: 0,
                        width: 320.0,
                        height: 180.0,
                        rotation: 90,
                        overlays: vec![],
                    },
                    PagePlan {
                        id: "rotated-page".into(),
                        source_id: alpha.id.clone(),
                        page_index: 1,
                        width: 300.0,
                        height: 200.0,
                        rotation: 0,
                        overlays: vec![
                            Overlay::Text(TextOverlay {
                                font_id: None,
                                font_name: Default::default(),
                                rotation: 0,
                                id: "text-1".into(),
                                x: 30.0,
                                y: 20.0,
                                text: "Overlay\nTwo".into(),
                                font_size: 16.0,
                                color: "#0044CC".into(),
                            }),
                            Overlay::Ink(InkOverlay {
                                id: "ink-1".into(),
                                paths: vec![vec![
                                    Point { x: 30.0, y: 90.0 },
                                    Point { x: 75.0, y: 125.0 },
                                    Point { x: 125.0, y: 90.0 },
                                ]],
                                color: "#E02020".into(),
                                stroke_width: 6.0,
                            }),
                        ],
                    },
                    PagePlan {
                        id: "first-page".into(),
                        source_id: alpha.id.clone(),
                        page_index: 0,
                        width: 200.0,
                        height: 300.0,
                        rotation: 0,
                        overlays: vec![],
                    },
                ],
            },
            &output,
        )
        .unwrap();

    let reopened = engine.open_document(&output).unwrap();
    assert_eq!(reopened.pages.len(), 3);
    assert_eq!(
        (reopened.pages[0].width, reopened.pages[0].height),
        (180.0, 320.0)
    );
    assert_eq!(
        (reopened.pages[1].width, reopened.pages[1].height),
        (300.0, 200.0)
    );
    assert!(engine
        .extract_text(&reopened.id, 0)
        .unwrap()
        .contains("BETA"));
    let edited_text = engine.extract_text(&reopened.id, 1).unwrap();
    assert!(edited_text.contains("ROTATED"));
    assert!(edited_text.contains("Overlay"));
    assert!(edited_text.contains("Two"));
    assert!(engine
        .extract_text(&reopened.id, 2)
        .unwrap()
        .contains("FIRST"));

    let png = engine.render_page(&reopened.id, 1, 600).unwrap();
    let image = image::load_from_memory_with_format(&png, image::ImageFormat::Png).unwrap();
    assert_eq!(image.width(), 600);
    assert_eq!(image.height(), 400);
    let red_pixels = image
        .pixels()
        .filter(|(x, y, pixel)| {
            (50..270).contains(x)
                && (160..280).contains(y)
                && pixel[0] > 180
                && pixel[1] < 90
                && pixel[2] < 90
        })
        .count();
    assert!(
        red_pixels > 100,
        "expected visible red ink in overlay region, found {red_pixels} pixels"
    );
    let final_segment_red_pixels = image
        .pixels()
        .filter(|(x, y, pixel)| {
            (180..245).contains(x)
                && (185..235).contains(y)
                && pixel[0] > 180
                && pixel[1] < 90
                && pixel[2] < 90
        })
        .count();
    assert!(
        final_segment_red_pixels > 20,
        "expected the final ink segment to render, found {final_segment_red_pixels} pixels"
    );

    engine.close_document(&alpha.id).unwrap();
    assert!(engine.render_page(&alpha.id, 0, 200).is_err());
}

#[test]
fn render_width_is_clamped_and_export_validation_rejects_unsafe_payloads() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.pdf");
    write_pdf(&source, &[("SAFE", [0.0, 0.0, 200.0, 300.0], None, 0)]);
    let engine = PdfEngine::start(pdfium_path()).unwrap();
    let document = engine.open_document(&source).unwrap();

    let png = engine.render_page(&document.id, 0, 1).unwrap();
    let image = image::load_from_memory_with_format(&png, image::ImageFormat::Png).unwrap();
    assert_eq!(image.width(), 64);

    let valid_page = PagePlan {
        id: "page".into(),
        source_id: document.id.clone(),
        page_index: 0,
        width: 200.0,
        height: 300.0,
        rotation: 0,
        overlays: vec![],
    };
    assert!(engine
        .export_pdf(
            ExportRequest {
                flatten: true,
                pages: vec![]
            },
            temp.path().join("empty.pdf")
        )
        .is_err());
    assert!(engine
        .export_pdf(
            ExportRequest {
                flatten: true,
                pages: vec![valid_page.clone()]
            },
            &source
        )
        .is_err());

    let mut bad_rotation = valid_page.clone();
    bad_rotation.rotation = 45;
    assert!(engine
        .export_pdf(
            ExportRequest {
                flatten: true,
                pages: vec![bad_rotation]
            },
            temp.path().join("rotation.pdf")
        )
        .is_err());

    let mut bad_color = valid_page.clone();
    bad_color.overlays.push(Overlay::Text(TextOverlay {
        font_id: None,
        font_name: Default::default(),
        rotation: 0,
        id: "bad-color".into(),
        x: 10.0,
        y: 10.0,
        text: "x".into(),
        font_size: 12.0,
        color: "red".into(),
    }));
    assert!(engine
        .export_pdf(
            ExportRequest {
                flatten: true,
                pages: vec![bad_color]
            },
            temp.path().join("color.pdf")
        )
        .is_err());

    let mut bad_point = valid_page.clone();
    bad_point.overlays.push(Overlay::Ink(InkOverlay {
        id: "bad-point".into(),
        paths: vec![vec![Point {
            x: f32::NAN,
            y: 20.0,
        }]],
        color: "#000000".into(),
        stroke_width: 2.0,
    }));
    assert!(engine
        .export_pdf(
            ExportRequest {
                flatten: true,
                pages: vec![bad_point]
            },
            temp.path().join("point.pdf")
        )
        .is_err());

    let prior_output = b"previous export stays intact";
    for (name, text, codepoint) in [
        ("cjk", "A 中 B", "U+4E2D"),
        ("checkmark", "A ✓ B", "U+2713"),
        ("emoji", "A 😀 B", "U+1F600"),
        ("nbsp", "A\u{00A0}B", "U+00A0"),
        ("soft-hyphen", "A\u{00AD}B", "U+00AD"),
    ] {
        let unsupported_output = temp.path().join(format!("unsupported-{name}.pdf"));
        fs::write(&unsupported_output, prior_output).unwrap();
        let mut unsupported_page = valid_page.clone();
        unsupported_page.overlays.push(Overlay::Text(TextOverlay {
            font_id: None,
            font_name: Default::default(),
            rotation: 0,
            id: format!("unsupported-{name}"),
            x: 10.0,
            y: 10.0,
            text: text.into(),
            font_size: 12.0,
            color: "#111111".into(),
        }));
        let error = engine
            .export_pdf(
                ExportRequest {
                    flatten: true,
                    pages: vec![unsupported_page],
                },
                &unsupported_output,
            )
            .unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains("Helvetica does not support character") && message.contains(codepoint),
            "unexpected validation error: {message}"
        );
        assert_eq!(fs::read(&unsupported_output).unwrap(), prior_output);
    }

    let clipped_output = temp.path().join("clipped.pdf");
    let mut clipped_page = valid_page;
    clipped_page.overlays = vec![
        Overlay::Text(TextOverlay {
            font_id: None,
            font_name: Default::default(),
            rotation: 0,
            id: "cleared-text".into(),
            x: -40.0,
            y: 340.0,
            text: String::new(),
            font_size: 12.0,
            color: "#111111".into(),
        }),
        Overlay::Ink(InkOverlay {
            id: "clipped-ink".into(),
            paths: vec![vec![
                Point { x: -20.0, y: 30.0 },
                Point { x: 20.0, y: 30.0 },
            ]],
            color: "#111111".into(),
            stroke_width: 2.0,
        }),
    ];
    engine
        .export_pdf(
            ExportRequest {
                flatten: true,
                pages: vec![clipped_page],
            },
            &clipped_output,
        )
        .unwrap();
    let clipped = engine.open_document(&clipped_output).unwrap();
    assert!(engine
        .extract_text(&clipped.id, 0)
        .unwrap()
        .contains("SAFE"));
}

#[test]
fn duplicated_annotated_pages_may_reuse_overlay_ids() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("duplicate-source.pdf");
    let output = temp.path().join("duplicate-output.pdf");
    write_pdf(&source, &[("ORIGINAL", [0.0, 0.0, 200.0, 300.0], None, 0)]);
    let engine = PdfEngine::start(pdfium_path()).unwrap();
    let document = engine.open_document(&source).unwrap();
    fs::write(&output, b"previous export").unwrap();
    let overlay = Overlay::Text(TextOverlay {
        font_id: None,
        font_name: Default::default(),
        rotation: 0,
        id: "same-overlay-after-clone".into(),
        x: 20.0,
        y: 20.0,
        text: "CLONED Café – 45°".into(),
        font_size: 14.0,
        color: "#222222".into(),
    });
    let first = PagePlan {
        id: "first-copy".into(),
        source_id: document.id.clone(),
        page_index: 0,
        width: 200.0,
        height: 300.0,
        rotation: 0,
        overlays: vec![overlay.clone()],
    };
    let mut second = first.clone();
    second.id = "second-copy".into();

    engine
        .export_pdf(
            ExportRequest {
                flatten: true,
                pages: vec![first, second],
            },
            &output,
        )
        .unwrap();
    let reopened = engine.open_document(&output).unwrap();
    assert_eq!(reopened.pages.len(), 2);
    assert!(engine
        .extract_text(&reopened.id, 0)
        .unwrap()
        .contains("CLONED Café – 45°"));
    assert!(engine
        .extract_text(&reopened.id, 1)
        .unwrap()
        .contains("CLONED Café – 45°"));
}

#[test]
fn export_rejects_a_hardlink_alias_of_an_open_source_without_modifying_it() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("identity-source.pdf");
    let alias = temp.path().join("identity-alias.pdf");
    write_pdf(&source, &[("IDENTITY", [0.0, 0.0, 200.0, 300.0], None, 0)]);
    fs::hard_link(&source, &alias).unwrap();
    let original = fs::read(&source).unwrap();
    let engine = PdfEngine::start(pdfium_path()).unwrap();
    let document = engine.open_document(&source).unwrap();
    let plan = PagePlan {
        id: "identity-page".into(),
        source_id: document.id.clone(),
        page_index: 0,
        width: 200.0,
        height: 300.0,
        rotation: 0,
        overlays: vec![],
    };

    assert!(engine
        .export_pdf(
            ExportRequest {
                flatten: true,
                pages: vec![plan]
            },
            &alias
        )
        .is_err());
    assert_eq!(fs::read(&source).unwrap(), original);
}
