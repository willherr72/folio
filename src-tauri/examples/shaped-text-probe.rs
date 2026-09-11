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
    let semantic = first.as_deref() == Some(std::ffi::OsStr::new("--semantic"));
    let output = (if semantic { arguments.next() } else { first })
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            root.join(if semantic {
                "artifacts/shaped-text/semantic-native"
            } else {
                "artifacts/shaped-text/native"
            })
        });
    if arguments.next().is_some() {
        return Err("Usage: shaped-text-probe [--semantic] [new-output-directory]".into());
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
    let mut evidence = Vec::new();
    let mut refused = Vec::new();
    for (name, fixture, text, ligatures) in cases {
        let font =
            FontAsset::parse_for_shaping(fs::read(root.join("tests/fixtures").join(fixture))?)?;
        for rotation in [0, 90, 180, 270] {
            let stem = format!("{name}-{rotation}");
            let candidate = if semantic {
                create_semantic_pdf(&font, text, 24.0, TextDirection::Auto, ligatures, rotation)
            } else {
                create_shaped_pdf(&font, text, 24.0, TextDirection::Auto, ligatures, rotation)
            };
            if semantic && name == "mixed-bidi" {
                let error = candidate
                    .err()
                    .ok_or("Mixed-direction support changed; reassess this evidence gate.")?;
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
            let original_text = engine.extract_text(&source.id, 0)?;
            let original_geometry = engine.page_text(&source.id, 0)?;
            let png = engine.render_page(&source.id, 0, 1000)?;
            fs::write(output.join(format!("{stem}.png")), &png)?;
            let saved_path = output.join(format!("{stem}.resaved.pdf"));
            let page = &source.pages[0];
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
            let resaved_png = engine.render_page(&reopened.id, 0, 1000)?;
            let exact = |actual: &str| actual.trim_end_matches(['\r', '\n']) == text;
            evidence.push(json!({
                "case": name, "rotation": rotation, "source": text, "fontSha256": font.info.id,
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
