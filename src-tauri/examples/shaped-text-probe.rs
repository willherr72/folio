//! Reproducible native serialization evidence; not an editor export option.
use folio_engine::{
    create_semantic_pdf, create_shaped_pdf, ExportRequest, FontAsset, PagePlan, PdfEngine,
    TextDirection,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{error::Error, fs, path::PathBuf};

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn main() -> Result<(), Box<dyn Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut arguments = std::env::args_os().skip(1);
    let first = arguments.next();
    let long_text = first.as_deref() == Some(std::ffi::OsStr::new("--semantic-long"));
    let semantic = long_text || first.as_deref() == Some(std::ffi::OsStr::new("--semantic"));
    let output = (if semantic { arguments.next() } else { first })
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            root.join(if long_text {
                "artifacts/shaped-text/semantic-long-native"
            } else if semantic {
                "artifacts/shaped-text/semantic-native"
            } else {
                "artifacts/shaped-text/native"
            })
        });
    if arguments.next().is_some() {
        return Err(
            "Usage: shaped-text-probe [--semantic|--semantic-long] [new-output-directory]".into(),
        );
    }
    // Evidence is immutable: use another directory for subsequent runs.
    if output.exists() {
        return Err("The evidence directory already exists; choose a new output path.".into());
    }
    fs::create_dir_all(&output)?;
    let dll = root.join("src-tauri/resources/pdfium/pdfium.dll");
    let engine = PdfEngine::start(&dll)?;
    let cases = [
        (
            "ligatures-on",
            "corpus/fonts/DejaVuSerif.ttf",
            "office",
            true,
        ),
        (
            "ligatures-off",
            "corpus/fonts/DejaVuSerif.ttf",
            "office",
            false,
        ),
        (
            "composed",
            "corpus/fonts/DejaVuSerif.ttf",
            "caf\u{e9}",
            true,
        ),
        (
            "decomposed",
            "corpus/fonts/DejaVuSerif.ttf",
            "cafe\u{301}",
            true,
        ),
        (
            "two-axis-marks",
            "shaped-text/DejaVuSans.ttf",
            "q\u{307}\u{323}",
            true,
        ),
        (
            "arabic",
            "shaped-text/NotoSansArabic-Regular.ttf",
            "سلام",
            true,
        ),
        (
            "mixed-bidi",
            "shaped-text/DejaVuSans.ttf",
            "ABC سلام 123 DEF",
            true,
        ),
        (
            "indic",
            "shaped-text/NotoSansDevanagari-Regular.ttf",
            "किताब",
            true,
        ),
        (
            "supplementary",
            "corpus/fonts/DejaVuSerif.ttf",
            "A \u{1d434} B",
            true,
        ),
    ];
    let cases: Vec<_> = if long_text {
        vec![
            (
                "long-ligatures-on",
                "corpus/fonts/DejaVuSerif.ttf",
                vec!["office"; 64].join(" "),
                true,
                24.,
            ),
            (
                "long-ligatures-off",
                "corpus/fonts/DejaVuSerif.ttf",
                vec!["office"; 64].join(" "),
                false,
                24.,
            ),
            (
                "long-marks",
                "shaped-text/DejaVuSans.ttf",
                vec!["q\u{307}\u{323}"; 80].join(" "),
                true,
                24.,
            ),
            (
                "long-arabic",
                "shaped-text/NotoSansArabic-Regular.ttf",
                "سلام".repeat(80),
                true,
                24.,
            ),
            (
                "long-indic",
                "shaped-text/NotoSansDevanagari-Regular.ttf",
                vec!["किताब"; 64].join(" "),
                true,
                24.,
            ),
            (
                "long-supplementary",
                "corpus/fonts/DejaVuSerif.ttf",
                "A\u{1d434}B".repeat(128),
                true,
                24.,
            ),
            (
                "maximum-scalars",
                "corpus/fonts/DejaVuSerif.ttf",
                "i".repeat(4096),
                false,
                4.,
            ),
            (
                "rtl-word-boundaries",
                "shaped-text/NotoSansArabic-Regular.ttf",
                vec!["سلام عالم"; 32].join(" "),
                true,
                24.,
            ),
            (
                "mixed-bidi",
                "shaped-text/DejaVuSans.ttf",
                "ABC سلام 123 DEF".into(),
                true,
                24.,
            ),
        ]
    } else {
        cases
            .into_iter()
            .map(|(name, fixture, text, ligatures)| {
                (name, fixture, text.to_owned(), ligatures, 24.)
            })
            .collect()
    };
    let mut evidence = Vec::new();
    let mut refused = Vec::new();
    for (name, fixture, text, ligatures, font_size) in cases {
        let font =
            FontAsset::parse_for_shaping(fs::read(root.join("tests/fixtures").join(fixture))?)?;
        for rotation in [0, 90, 180, 270] {
            let stem = format!("{name}-{rotation}");
            let candidate = if semantic {
                create_semantic_pdf(
                    &font,
                    &text,
                    font_size,
                    TextDirection::Auto,
                    ligatures,
                    rotation,
                )
            } else {
                create_shaped_pdf(
                    &font,
                    &text,
                    font_size,
                    TextDirection::Auto,
                    ligatures,
                    rotation,
                )
            };
            if semantic && matches!(name, "mixed-bidi" | "rtl-word-boundaries") {
                let error = candidate
                    .err()
                    .ok_or("Refused direction policy changed; reassess this evidence gate.")?;
                refused.push(json!({"case":name,"rotation":rotation,"source":text,"reason":error.to_string()}));
                continue;
            }
            let result = candidate?;
            let path = output.join(format!("{stem}.pdf"));
            fs::write(&path, &result.bytes)?;
            fs::write(
                output.join(format!("{stem}.layout.json")),
                serde_json::to_vec_pretty(&result.layout)?,
            )?;
            let source = engine.open_document(&path)?;
            let page = &source.pages[0];
            // The long-line matrix includes very narrow rotated pages. Bound
            // preview size instead of rendering a 1,000px-wide strip tens of
            // thousands of pixels tall. Target 2,000px on the longest side;
            // the engine's 64px minimum width can raise that on narrow pages.
            // PDF geometry stays native.
            let render_width = if long_text {
                (2000. * page.width / page.height).floor().clamp(64., 2000.) as u32
            } else {
                1000
            };
            let original_text = engine.extract_text(&source.id, 0)?;
            let original_geometry = engine.page_text(&source.id, 0)?;
            let png = engine.render_page(&source.id, 0, render_width)?;
            fs::write(output.join(format!("{stem}.png")), &png)?;
            let saved_path = output.join(format!("{stem}.resaved.pdf"));
            engine.export_pdf(
                ExportRequest {
                    flatten: false,
                    pages: vec![PagePlan {
                        id: "probe-page".into(),
                        source_id: source.id.clone(),
                        page_index: 0,
                        width: page.width,
                        height: page.height,
                        rotation: 0,
                        overlays: Vec::new(),
                    }],
                },
                &saved_path,
            )?;
            let reopened = engine.open_document(&saved_path)?;
            let resaved_text = engine.extract_text(&reopened.id, 0)?;
            let resaved_geometry = engine.page_text(&reopened.id, 0)?;
            let resaved_png = engine.render_page(&reopened.id, 0, render_width)?;
            let exact = |actual: &str| actual.trim_end_matches(['\r', '\n']) == text;
            evidence.push(json!({
                "case": name, "rotation": rotation, "source": text, "fontSha256": font.info.id,
                "renderWidth": render_width,
                "pdf": format!("{stem}.pdf"), "resavedPdf": format!("{stem}.resaved.pdf"),
                "originalText": original_text, "resavedText": resaved_text,
                "originalExact": exact(&original_text), "resavedExact": exact(&resaved_text),
                "originalGeometry": original_geometry, "resavedGeometry": resaved_geometry,
                "originalPngSha256": hash(&png), "resavedPngSha256": hash(&resaved_png),
                "renderUnchanged": png == resaved_png,
            }));
            engine.close_document(&reopened.id)?;
            engine.close_document(&source.id)?;
        }
    }
    fs::write(
        output.join("results.json"),
        serde_json::to_vec_pretty(&json!({
        "matrix": if long_text { "long-text" } else { "original" },
        "pdfiumSha256": hash(&fs::read(dll)?), "cases": evidence, "refused": refused,
        "representation": if semantic { "outlines-with-type3-semantic-text" } else { "allocated-cid-with-whole-line-actualtext" },
        "scope": "Experimental native representation. Reader Unicode, cluster geometry, rendering and resource limits require separate checks before editor support."
        }))?,
    )?;
    println!(
        "Wrote {} native cases to {}",
        evidence.len(),
        output.display()
    );
    Ok(())
}
