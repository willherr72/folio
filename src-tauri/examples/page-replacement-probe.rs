//! Feasibility evidence only; opens public synthetic PDFs without changing inputs.
use folio_engine::{FillFormFieldRequest, FormFieldValue, PdfEngine};
use serde_json::{json, Value};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(std::env::args().nth(1).ok_or("Pass the probe directory")?);
    let run = std::env::args().nth(2).unwrap_or("native".into());
    let output = root.join(format!("{run}.json"));
    if output.exists() { return Err("Preserving existing probe evidence".into()); }
    let input: Value = serde_json::from_slice(&std::fs::read(root.join("independent.json"))?)?;
    let engine = PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))?;
    let mut cases = Vec::new();
    let mut passed = true;
    for case in input["cases"].as_array().ok_or("Missing cases")? {
        let label = case["label"].as_str().unwrap();
        let paths: Vec<_> = ["target", "actual", "reference"].iter().map(|key| root.join(case[key].as_str().unwrap())).collect();
        let originals: Vec<_> = paths.iter().map(std::fs::read).collect::<Result<_,_>>()?;
        let before = engine.open_document(&paths[0])?;
        let actual = engine.open_document(&paths[1])?;
        let reference = engine.open_document(&paths[2])?;
        let fields = engine.list_form_fields(&before.id, 0)?;
        let actual_fields = engine.list_form_fields(&actual.id, 0)?;
        let field_match = serde_json::to_value(&fields)? == serde_json::to_value(&actual_fields)?;
        let comment_match = serde_json::to_value(engine.list_native_comments(&before.id)?)?
            == serde_json::to_value(engine.list_native_comments(&actual.id)?)?;
        let bookmark_match = before.bookmarks == actual.bookmarks;
        let mut equal_pixels = true;
        let mut pixel_differences = Vec::new();
        for width in [600, 1000] {
            let a = engine.render_page(&actual.id,0,width)?;
            let b = engine.render_page(&reference.id,0,width)?;
            equal_pixels &= a == b;
            if a != b {
                let a_image = image::load_from_memory(&a)?.to_rgba8();
                let b_image = image::load_from_memory(&b)?.to_rgba8();
                let different = a_image.pixels().zip(b_image.pixels()).filter(|(a,b)| a != b).count();
                pixel_differences.push(json!({"width":width,"differentPixels":different,"dimensions":a_image.dimensions()}));
                std::fs::write(root.join(format!("{run}-{label}-{width}-actual.png")),a)?;
                std::fs::write(root.join(format!("{run}-{label}-{width}-reference.png")),b)?;
            }
            equal_pixels &= engine.render_page(&actual.id,1,width)? == engine.render_page(&before.id,1,width)?;
        }
        let raw_equal = engine.extract_text(&actual.id,0)? == engine.extract_text(&reference.id,0)?;
        let actual_top = engine.list_text_runs(&actual.id,0)?;
        let reference_top = engine.list_text_runs(&reference.id,0)?;
        let actual_forms = engine.list_form_text_runs(&actual.id,0)?;
        let filled = engine.fill_form_field(&actual.id, FillFormFieldRequest {
            page_index: 0, annotation_index: actual_fields[0].annotation_index,
            expected_value: actual_fields[0].value.clone(),
            value: FormFieldValue::Text {text:"Still editable".into()},
        })?;
        let fillable = engine.list_form_fields(&filled.id,0)?[0].value == FormFieldValue::Text {text:"Still editable".into()};
        engine.close_document(&filled.id)?;
        for ((source,path), bytes) in [&before,&actual,&reference].into_iter().zip(&paths).zip(&originals) {
            assert_eq!(engine.source_bytes(&source.id)?.as_ref(),bytes.as_slice());
            assert_eq!(&std::fs::read(path)?,bytes);
            engine.close_document(&source.id)?;
        }
        let ok = equal_pixels && raw_equal && field_match && comment_match && bookmark_match && fillable;
        passed &= ok;
        cases.push(json!({"label":label,"passed":ok,"pixelsEqual":equal_pixels,"rawTextEqual":raw_equal,
            "fieldsEqual":field_match,"commentsEqual":comment_match,"bookmarksEqual":bookmark_match,
            "fieldStillEditable":fillable,"formPlacementTopLevelText":actual_top.runs.len(),
            "directReferenceTopLevelText":reference_top.runs.len(),"formPlacementNestedText":actual_forms.runs.len(),
            "beforeBookmarks":before.bookmarks,"actualBookmarks":actual.bookmarks,
            "formInspectionReason":actual_forms.reason,"directRuns":reference_top,"pixelDifferences":pixel_differences}));
    }
    let result = json!({"passed":passed,"feasibilityOnly":true,"cases":cases});
    use std::io::Write;
    std::fs::OpenOptions::new().create_new(true).write(true).open(output)?
        .write_all(&serde_json::to_vec_pretty(&result)?)?;
    println!("{}",json!({"passed":passed,"cases":result["cases"].as_array().unwrap().len(),"feasibilityOnly":true}));
    if !passed { return Err("Probe differences recorded in native.json".into()); }
    Ok(())
}
