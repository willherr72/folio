//! Actual editor placement and export raster evidence.
use folio_engine::{ExportRequest, Overlay, PagePlan, PdfEngine, TextOverlay};
use lopdf::{dictionary, Document, Object};
use std::{fs, path::PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let out = root.join("artifacts/shaped-text/editor-native");
    if out.exists() {
        return Err("Evidence directory already exists".into());
    }
    fs::create_dir_all(&out)?;
    let mut pdf = Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),400.into(),400.into()],"Resources"=>dictionary!{}});
    pdf.objects.insert(
        pages,
        Object::Dictionary(
            dictionary! {"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1},
        ),
    );
    let catalog = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    pdf.trailer.set("Root", catalog);
    let blank = out.join("blank.pdf");
    pdf.save(&blank)?;
    let engine = PdfEngine::start(root.join("src-tauri/resources/pdfium/pdfium.dll"))?;
    let source = engine.open_document(&blank)?;
    let samples = [
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
        ("composed", "corpus/fonts/DejaVuSerif.ttf", "café", true),
        (
            "decomposed",
            "corpus/fonts/DejaVuSerif.ttf",
            "cafe\u{301}",
            true,
        ),
        (
            "marks",
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
            "indic",
            "shaped-text/NotoSansDevanagari-Regular.ttf",
            "किताब",
            true,
        ),
        (
            "supplementary",
            "corpus/fonts/DejaVuSerif.ttf",
            "A\u{1d434}B",
            true,
        ),
    ];
    for (name, fixture, text, ligatures) in samples {
        let font = engine
            .fonts()
            .register(fs::read(root.join("tests/fixtures").join(fixture))?)?;
        for rotation in [0, 90, 180, 270] {
            let stem = format!("{name}-{rotation}");
            let overlay: TextOverlay = serde_json::from_value(
                serde_json::json!({"id":"sample","x":150,"y":150,"text":text,"fontSize":24,"fontId":font.id,"color":"#CC4422","rotation":rotation,"shaping":{"version":1,"direction":"auto","ligatures":ligatures}}),
            )?;
            let prepared = engine.prepare_text_overlay(&overlay)?;
            fs::write(
                out.join(format!("{stem}.prepared.json")),
                serde_json::to_vec_pretty(
                    &serde_json::json!({"overlay":overlay,"result":prepared}),
                )?,
            )?;
            let output = out.join(format!("{stem}.pdf"));
            engine.export_pdf(
                ExportRequest {
                    pages: vec![PagePlan {
                        id: "page".into(),
                        source_id: source.id.clone(),
                        page_index: 0,
                        width: 400.,
                        height: 400.,
                        rotation: 0,
                        overlays: vec![Overlay::Text(overlay)],
                    }],
                    flatten: true,
                },
                &output,
            )?;
            let opened = engine.open_document(&output)?;
            assert_eq!(
                engine
                    .extract_text(&opened.id, 0)?
                    .trim_end_matches(['\r', '\n']),
                text
            );
            for zoom in [150, 300] {
                fs::write(
                    out.join(format!("{stem}.preview-{zoom}.png")),
                    engine.render_page(&opened.id, 0, 400 * zoom / 100)?,
                )?;
            }
            engine.close_document(&opened.id)?;
        }
    }
    engine.close_document(&source.id)?;
    println!("Wrote 32 editor-native pairs");
    Ok(())
}
