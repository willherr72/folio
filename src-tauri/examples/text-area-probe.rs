//! Development-only text-area checkpoint. Each accepted line uses ordinary shaped overlays.
use folio_engine::{
    layout_text_area, ExportRequest, Overlay, PagePlan, PdfEngine, TextAreaRequest, TextOverlay,
};
use lopdf::{dictionary, Document, Object};
use pdfium_render::prelude::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const PAGE: f32 = 480.;
const ORIGIN: f32 = 32.;
fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

// Run raw bindings in another process: the engine owns PDFium on its worker thread.
// No Folio extraction, newline trimming, normalization, or replacement is applied.
fn raw_pdfium(dll: &Path, path: &Path) -> Result<Value, Box<dyn Error>> {
    let bytes = fs::read(path)?;
    let bindings = Pdfium::bind_to_library(dll)?;
    unsafe {
        bindings.FPDF_InitLibrary();
        let doc = bindings.FPDF_LoadMemDocument64(&bytes, None);
        assert!(!doc.is_null(), "PDFium cannot open evidence PDF");
        let page = bindings.FPDF_LoadPage(doc, 0);
        assert!(!page.is_null());
        let text = bindings.FPDFText_LoadPage(page);
        assert!(!text.is_null());
        let count = bindings.FPDFText_CountChars(text);
        assert!((0..=32768).contains(&count));
        let mut utf16 = vec![0u16; count as usize + 1];
        let length = bindings.FPDFText_GetText(text, 0, count, utf16.as_mut_ptr());
        assert!(length >= 0 && length as usize <= utf16.len());
        let value = String::from_utf16(&utf16[..length.saturating_sub(1) as usize])?;
        let mut geometry = Vec::new();
        for index in 0..count {
            let (mut x, mut y) = (0., 0.);
            let has_origin = bindings.FPDFText_GetCharOrigin(text, index, &mut x, &mut y) != 0;
            geometry.push(json!({"index":index,"unicode":bindings.FPDFText_GetUnicode(text,index),"x":x,"y":y,"hasOrigin":has_origin}));
        }
        bindings.FPDFText_ClosePage(text);
        bindings.FPDF_ClosePage(page);
        bindings.FPDF_CloseDocument(doc);
        bindings.FPDF_DestroyLibrary();
        Ok(json!({"text":value,"geometry":geometry}))
    }
}
fn extract(dll: &Path, path: &Path) -> Result<Value, Box<dyn Error>> {
    let result = Command::new(std::env::current_exe()?)
        .arg("--raw-pdfium")
        .arg(dll)
        .arg(path)
        .output()?;
    if !result.status.success() {
        return Err(format!(
            "Raw PDFium child failed: {}",
            String::from_utf8_lossy(&result.stderr)
        )
        .into());
    }
    Ok(serde_json::from_slice(&result.stdout)?)
}
fn plan(source: &str, overlays: Vec<Overlay>) -> ExportRequest {
    ExportRequest {
        flatten: true,
        pages: vec![PagePlan {
            id: "area-page".into(),
            source_id: source.into(),
            page_index: 0,
            width: PAGE,
            height: PAGE,
            rotation: 0,
            overlays,
        }],
    }
}
fn blank(path: &Path) -> Result<(), Box<dyn Error>> {
    let mut pdf = Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let page = pdf.add_object(dictionary! {"Type"=>"Page","Parent"=>pages,
        "MediaBox"=>vec![0.into(),0.into(),Object::Real(PAGE),Object::Real(PAGE)],"Resources"=>dictionary!{}});
    pdf.objects.insert(
        pages,
        Object::Dictionary(
            dictionary! {"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1},
        ),
    );
    let catalog = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    pdf.trailer.set("Root", catalog);
    pdf.save(path)?;
    Ok(())
}
fn main() -> Result<(), Box<dyn Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments.first().is_some_and(|v| v == "--raw-pdfium") {
        if arguments.len() != 3 {
            return Err("Expected --raw-pdfium dll pdf".into());
        }
        println!(
            "{}",
            raw_pdfium(Path::new(&arguments[1]), Path::new(&arguments[2]))?
        );
        return Ok(());
    }
    if arguments.len() > 1 {
        return Err("Usage: text-area-probe [new-output-directory]".into());
    }
    let out = arguments
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("artifacts/text-area-native"));
    if out.exists() {
        return Err("Evidence directory already exists; choose a new output path.".into());
    }
    fs::create_dir_all(&out)?;
    let source_path = out.join("source.pdf");
    blank(&source_path)?;
    let source_bytes = fs::read(&source_path)?;
    let dll = root
        .join("src-tauri/resources/pdfium/pdfium.dll")
        .canonicalize()?;
    let engine = PdfEngine::start(&dll)?;
    let source = engine.open_document(&source_path)?;
    let fixture = "tests/fixtures/corpus/fonts/DejaVuSerif.ttf";
    let info = engine.fonts().register(fs::read(root.join(fixture))?)?;
    let font = engine.fonts().get(&info.id)?;
    let paragraph = "office café cafe\u{301} with fine lines and clear words";
    let samples = [
        ("narrow-left", paragraph, 190., 350., "left", true, false),
        ("wide-left", paragraph, 416., 350., "left", true, false),
        (
            "narrow-center",
            paragraph,
            190.,
            350.,
            "center",
            true,
            false,
        ),
        ("narrow-right", paragraph, 190., 350., "right", true, false),
        ("ligatures-off", paragraph, 190., 350., "left", false, false),
        (
            "hard-breaks",
            "office\r\n\ncafé\ncafe\u{301}\n",
            250.,
            350.,
            "left",
            true,
            false,
        ),
        (
            "spaces-nbsp",
            "one  two three\u{a0}four five",
            170.,
            350.,
            "left",
            true,
            false,
        ),
        (
            "horizontal-overflow",
            "unbreakableword",
            48.,
            350.,
            "left",
            true,
            true,
        ),
        (
            "vertical-overflow",
            paragraph,
            190.,
            20.,
            "left",
            true,
            true,
        ),
    ];
    let mut cases = Vec::new();
    for (name, text, width, height, alignment, ligatures, expected_refusal) in samples {
        let request: TextAreaRequest = serde_json::from_value(json!({
            "version":1,"text":text,"fontSize":20,"width":width,"height":height,
            "inset":6,"lineSpacing":1.2,"alignment":alignment,"direction":"ltr","ligatures":ligatures
        }))?;
        let layout = layout_text_area(&font, &request)?;
        let roundtrip: TextAreaRequest = serde_json::from_slice(&serde_json::to_vec(&request)?)?;
        assert_eq!(
            serde_json::to_value(&layout)?,
            serde_json::to_value(layout_text_area(&font, &roundtrip)?)?
        );
        assert_eq!(
            !layout.can_export, expected_refusal,
            "Unexpected specimen overflow: {name}"
        );
        let painted_lines: Vec<_> = layout
            .lines
            .iter()
            .map(|line| &text[line.source.utf8_start..line.source.utf8_end])
            .collect();
        let reconstructed: String = layout
            .lines
            .iter()
            .map(|line| &text[line.source.utf8_start..line.delimiter.utf8_end])
            .collect();
        assert_eq!(reconstructed, text);
        let mut item = json!({"name":name,"request":request,"layout":layout,"logicalText":text,
            "paintedLines":painted_lines,"fontFixture":fixture,"fontSha256":info.id,
            "pageWidth":PAGE,"pageHeight":PAGE,"exportRefused":!layout.can_export,
            "pdf":null,"resavedPdf":null,"pdfiumText":null,"pdfiumResavedText":null,
            "pdfiumGeometry":null,"pdfiumResavedGeometry":null,"overlays":[],"rasters":[]});
        let path = out.join(format!("{name}.pdf"));
        let saved = out.join(format!("{name}.resaved.pdf"));
        if layout.can_export {
            let mut overlays = Vec::new();
            let mut prepared = Vec::new();
            for (index, line) in layout.lines.iter().enumerate() {
                if line.shaped.is_none() {
                    continue;
                }
                // Existing overlays store the top of the font-size box, not the baseline.
                let overlay: TextOverlay = serde_json::from_value(json!({
                    "id":format!("{name}-{index}"),"x":ORIGIN+line.baseline_x,"y":ORIGIN+line.baseline_y-request.font_size,
                    "text":painted_lines[index],"fontSize":request.font_size,"fontId":info.id,
                    "color":"#222222","rotation":0,"shaping":{"version":1,"direction":"ltr","ligatures":ligatures}
                }))?;
                let result = engine.prepare_text_overlay(&overlay)?;
                let shaped = line.shaped.as_ref().expect("painted line has shaping");
                assert_eq!(result.preview.text, shaped.text);
                assert_eq!(result.preview.font_id, shaped.font_id);
                assert_eq!(shaped.width, line.advance_width);
                assert_eq!(
                    shaped,
                    &folio_engine::shape_text(
                        &font,
                        &overlay.text,
                        request.font_size,
                        request.direction,
                        request.ligatures
                    )?
                );
                prepared.push(json!({"lineIndex":index,"overlay":overlay,"result":result}));
                overlays.push(Overlay::Text(overlay));
            }
            engine.export_pdf(plan(&source.id, overlays), &path)?;
            let opened = engine.open_document(&path)?;
            engine.export_pdf(plan(&opened.id, Vec::new()), &saved)?;
            let reopened = engine.open_document(&saved)?;
            let mut rasters = Vec::new();
            for zoom in [150, 300] {
                let pixels = (PAGE * zoom as f32 / 100.).round() as u32;
                let original = engine.render_page(&opened.id, 0, pixels)?;
                let resaved = engine.render_page(&reopened.id, 0, pixels)?;
                let file = format!("{name}-{zoom}.pdfium.png");
                let saved_file = format!("{name}-{zoom}.resaved.pdfium.png");
                fs::write(out.join(&file), &original)?;
                fs::write(out.join(&saved_file), &resaved)?;
                rasters.push(json!({"zoom":zoom,"file":file,"resavedFile":saved_file,"unchanged":original==resaved}));
            }
            engine.close_document(&reopened.id)?;
            engine.close_document(&opened.id)?;
            let original = extract(&dll, &path)?;
            let resaved = extract(&dll, &saved)?;
            item["pdf"] = json!(format!("{name}.pdf"));
            item["resavedPdf"] = json!(format!("{name}.resaved.pdf"));
            item["pdfiumText"] = original["text"].clone();
            item["pdfiumResavedText"] = resaved["text"].clone();
            item["pdfiumGeometry"] = original["geometry"].clone();
            item["pdfiumResavedGeometry"] = resaved["geometry"].clone();
            item["overlays"] = json!(prepared);
            item["rasters"] = json!(rasters);
        } else {
            assert!(!path.exists() && !saved.exists(), "Overflow produced a PDF");
        }
        fs::write(
            out.join(format!("{name}.json")),
            serde_json::to_vec_pretty(&item)?,
        )?;
        cases.push(item);
    }
    engine.close_document(&source.id)?;
    assert_eq!(
        source_bytes,
        fs::read(&source_path)?,
        "Original source was modified"
    );
    fs::write(
        out.join("results.json"),
        serde_json::to_vec_pretty(&json!({
            "version":1,"pageWidth":PAGE,"pageHeight":PAGE,"areaOrigin":{"x":ORIGIN,"y":ORIGIN},
            "sourcePdf":"source.pdf","sourceSha256":hash(&source_bytes),"sourceUnchanged":true,
            "pdfiumSha256":hash(&fs::read(&dll)?),"cases":cases,
            "scope":"Development layout only. External reader logical text and painted lines are separate gates; no normalization establishes exact copy."
        }))?,
    )?;
    println!(
        "Wrote {} text-area specimens to {}",
        cases.len(),
        out.display()
    );
    Ok(())
}
