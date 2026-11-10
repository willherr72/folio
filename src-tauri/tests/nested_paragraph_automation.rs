use folio_engine::PdfEngine;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, path::{Path, PathBuf}, process::{Command, Output}};
#[path = "support/nested_automation.rs"]
mod nested;

fn renderer() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")
}
fn cli(verb: &str, input: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_folio-cli"))
        .arg(verb).arg(input).arg("--engine").arg(renderer()).args(args).output().unwrap()
}
fn ok(out: Output) -> Value {
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(out.stderr.is_empty());
    let frame: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(frame["ok"], true);
    frame["result"].clone()
}
fn refused(out: Output) {
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    assert_eq!(serde_json::from_slice::<Value>(&out.stderr).unwrap()["ok"], false);
}
fn runs(source: &Path, nested: bool) -> Value {
    ok(cli(if nested {"inspect-nested-text"} else {"inspect-text"}, source, &["--page", "1"]))
}
fn inspect(source: &Path, seed: &Value) -> Value {
    ok(cli("inspect-paragraph", source, &["--page", "1", "--selection-id", seed["selectionId"].as_str().unwrap()]))
}
fn right_seed(listed: &Value) -> &Value {
    listed["runs"].as_array().unwrap().iter().find(|run| run["bounds"]["x"].as_f64().unwrap() > 300. && run["text"] == "Nested first line").unwrap()
}
fn request(selected: &Value, text: &str) -> Value {
    json!({"page":1,"seedSelectionId":selected["seedSelectionId"],"paragraphId":selected["paragraphId"],"runs":[{"text":text,"styleId":selected["paragraph"]["styles"][0]["styleId"]}],"fit":true})
}
fn preview(source: &Path, parent: &Path, request: &Value) -> Output {
    let file = parent.join("edit.json");
    fs::write(&file, serde_json::to_vec(request).unwrap()).unwrap();
    cli("preview-paragraph", source, &["--request", file.to_str().unwrap(), "--out-dir", parent.to_str().unwrap()])
}
fn save(bundle: &Value, destination: &Path) -> Output {
    cli("save-text-preview", Path::new(bundle["folder"].as_str().unwrap()), &["--manifest-sha256", bundle["manifestSha256"].as_str().unwrap(), "--out", destination.to_str().unwrap()])
}
fn manifest(bundle: &Value) -> Value {
    serde_json::from_slice(&fs::read(Path::new(bundle["folder"].as_str().unwrap()).join("manifest.json")).unwrap()).unwrap()
}
fn promoted(source: &Path, prefix: &str) -> Value {
    let listed = runs(source, false);
    let seed = listed["runs"].as_array().unwrap().iter().find(|run| run["bounds"]["x"].as_f64().unwrap() > 300. && run["text"].as_str().unwrap().starts_with(prefix)).expect("promoted page text");
    inspect(source, seed)
}
fn preserve_others(source: &Path, saved: &Path) {
    let engine = PdfEngine::start(renderer()).unwrap();
    let before = engine.open_document(source).unwrap();
    let after = engine.open_document(saved).unwrap();
    assert_eq!(before.pages.len(), after.pages.len());
    for page in [1, 2] {
        assert_eq!(engine.render_page(&before.id, page, 900).unwrap(), engine.render_page(&after.id, page, 900).unwrap());
    }
    engine.close_document(&before.id).unwrap();
    engine.close_document(&after.id).unwrap();
    let pdf = lopdf::Document::load(saved).unwrap();
    assert!(pdf.get_dictionary(pdf.get_pages()[&3]).unwrap().has(b"Annots"));
}

#[test]
fn nested_paragraph_inspect_fit_save_and_promoted_reedit() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pdf");
    nested::write_fixture(&source, nested::Mode::RepeatedOuter, 1.);
    let original = fs::read(&source).unwrap();
    let listed = runs(&source, true);
    let selected = inspect(&source, right_seed(&listed));
    assert!(selected["paragraphId"].as_str().unwrap().starts_with("paragraph-v2-"));
    assert_eq!(selected, inspect(&source, right_seed(&listed)));
    assert_eq!(selected["paragraph"]["objectIndices"].as_array().unwrap().len(), 2);
    let text = format!("{} this added text wraps into another line", selected["paragraph"]["text"].as_str().unwrap());
    let edit = request(&selected, &text);
    let first = ok(preview(&source, dir.path(), &edit));
    assert_eq!(manifest(&first)["version"], 5);
    assert_eq!(first["paragraph"]["occurrenceScope"], selected["occurrenceScope"]);
    let mut echoed=edit.clone(); echoed["fit"]=json!(false);
    echoed["bounds"]=first["paragraph"]["bounds"].clone();
    echoed["baselineAnchor"]=selected["paragraph"]["baselineAnchor"].clone();
    let echoed=ok(preview(&source,dir.path(),&echoed));
    assert_eq!(image::open(first["after"].as_str().unwrap()).unwrap().to_rgba8(),
        image::open(echoed["after"].as_str().unwrap()).unwrap().to_rgba8());
    let saved = dir.path().join("saved.pdf");
    ok(save(&first, &saved));
    assert_eq!(fs::read(&saved).unwrap(), fs::read(first["candidate"].as_str().unwrap()).unwrap());
    refused(save(&first, &saved));
    refused(preview(&saved, dir.path(), &edit));
    assert_eq!(runs(&saved, true)["runs"].as_array().unwrap().len(), 2);
    let next = promoted(&saved, "Nested");
    assert!(selected["paragraph"]["baselineAnchor"].is_object());
    assert_eq!(next["paragraph"]["baselineAnchor"], selected["paragraph"]["baselineAnchor"]);
    assert!(next["paragraphId"].as_str().unwrap().starts_with("paragraph-v1-"));
    assert!(next.get("occurrenceScope").is_none());
    assert_eq!(next["paragraph"]["text"], text);
    for key in ["fontName", "fontSize", "color"] {
        assert_eq!(next["paragraph"]["styles"][0][key], selected["paragraph"]["styles"][0][key]);
    }
    let revised = text.replacen("Nested", "Edited", 1);
    let second = ok(preview(&saved, dir.path(), &request(&next, &revised)));
    assert_eq!(manifest(&second)["version"], 2);
    let final_path = dir.path().join("final.pdf");
    ok(save(&second, &final_path));
    assert_eq!(promoted(&final_path, "Edited")["paragraph"]["text"], revised);
    assert_eq!(fs::read(&source).unwrap(), original);
    preserve_others(&source, &final_path);
}

#[test]
fn inner_form_membership_and_outer_scope_are_explicit() {
    for (mode, count) in [(nested::Mode::RepeatedInnerClose, 4), (nested::Mode::RepeatedInnerFar, 2)] {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.pdf");
        nested::write_fixture(&source, mode, 1.);
        let original = fs::read(&source).unwrap();
        let listed = runs(&source, true);
        let seed = right_seed(&listed);
        let selected = inspect(&source, seed);
        assert_eq!(selected["occurrenceScope"], json!({"kind":"outer-form","objectPath":[seed["objectPath"][0]]}));
        assert_eq!(selected["paragraph"]["objectIndices"].as_array().unwrap().len(), count);
        assert_eq!(selected["paragraph"]["text"].as_str().unwrap().matches("Nested first line").count(), count / 2);
        let text = selected["paragraph"]["text"].as_str().unwrap().replacen("Nested", "Edited", 1);
        let bundle = ok(preview(&source, dir.path(), &request(&selected, &text)));
        let before = image::open(bundle["before"].as_str().unwrap()).unwrap().to_rgba8();
        let after = image::open(bundle["after"].as_str().unwrap()).unwrap().to_rgba8();
        assert_eq!(before.dimensions(), after.dimensions());
        let scale = before.width() as f32 / 600.;
        let mut changed = 0;
        for (x, y, pixel) in before.enumerate_pixels() {
            if pixel != after.get_pixel(x, y) {
                changed += 1;
                // Generous selected paragraph region, disjoint from the left
                // outer invocation, heading and the distant inner paragraph.
                assert!((x as f32) > 325. * scale && (y as f32) > 112. * scale && (y as f32) < (if count == 4 {205.} else {175.}) * scale, "unselected pixel {x},{y}");
            }
        }
        assert!(changed > 0);
        let saved = dir.path().join("saved.pdf");
        ok(save(&bundle, &saved));
        let engine = PdfEngine::start(renderer()).unwrap();
        let document = engine.open_document(&saved).unwrap();
        let text: String = engine.page_text(&document.id, 0).unwrap().characters.into_iter().map(|c| c.text).collect();
        engine.close_document(&document.id).unwrap();
        assert_eq!(text.matches("Edited first line").count(), 1);
        assert_eq!(text.matches("Nested first line").count(), 3);
        preserve_others(&source, &saved);
        assert_eq!(fs::read(&source).unwrap(), original);
    }
}

#[test]
fn equal_metadata_different_seed_paths_have_distinct_identity() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pdf");
    nested::write_fixture(&source, nested::Mode::RepeatedInnerClose, 1.);
    let listed = runs(&source, true);
    let right: Vec<_> = listed["runs"].as_array().unwrap().iter().filter(|run| run["bounds"]["x"].as_f64().unwrap() > 300.).collect();
    let a = inspect(&source, right[0]);
    let b = inspect(&source, right[2]);
    assert_ne!(right[0]["objectPath"], right[2]["objectPath"]);
    assert_eq!(a["paragraph"], b["paragraph"]);
    assert_eq!(a["occurrenceScope"], b["occurrenceScope"]);
    assert_ne!(a["paragraphId"], b["paragraphId"]);
    let copy = dir.path().join("copy.pdf");
    fs::copy(&source, &copy).unwrap();
    assert_eq!(a, inspect(&copy, right[0]));
    let mut mixed = request(&a, "Edited paragraph");
    mixed["seedSelectionId"] = b["seedSelectionId"].clone();
    refused(preview(&source, dir.path(), &mixed));
}

#[test]
fn nested_paragraph_stale_palette_scope_and_version_pairs_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pdf");
    nested::write_fixture(&source, nested::Mode::RepeatedOuter, 1.);
    let listed = runs(&source, true);
    let selected = inspect(&source, right_seed(&listed));
    let valid = request(&selected, "Edited first line\nNested second line");
    let mut bads = Vec::new();
    for key in ["objectPath", "occurrenceScope", "fontId"] {
        let mut bad = valid.clone(); bad[key] = json!([0]); bads.push(bad);
    }
    let mut bad = valid.clone(); bad["page"] = json!(2); bads.push(bad);
    let mut bad = valid.clone(); bad["runs"][0]["styleId"] = json!(63); bads.push(bad);
    let mut bad = valid.clone(); bad["paragraphId"] = json!(selected["paragraphId"].as_str().unwrap().replace("paragraph-v2-", "paragraph-v1-")); bads.push(bad);
    for bad in bads { refused(preview(&source, dir.path(), &bad)); }
    assert_eq!(fs::read_dir(dir.path()).unwrap().filter(|p| p.as_ref().unwrap().path().is_dir()).count(), 0);
    let bundle = ok(preview(&source, dir.path(), &valid));
    let path = Path::new(bundle["folder"].as_str().unwrap()).join("manifest.json");
    let original = fs::read(&path).unwrap();
    let good: Value = serde_json::from_slice(&original).unwrap();
    let mut bads = Vec::new();
    for version in [1, 2, 3, 4] { let mut bad = good.clone(); bad["version"] = json!(version); bads.push(bad); }
    for scope in [Value::Null,json!({"kind":"inner-form","objectPath":[2]}),json!({"kind":"outer-form","objectPath":[2,0]}),json!({"kind":"outer-form","objectPath":[]}),json!({"kind":"outer-form","objectPath":[2],"extra":1})] {
        let mut bad = good.clone(); bad["paragraph"]["occurrenceScope"] = scope; bads.push(bad);
    }
    let mut bad = good.clone(); bad["paragraph"].as_object_mut().unwrap().remove("occurrenceScope"); bads.push(bad);
    for (i, bad) in bads.into_iter().enumerate() {
        let bytes = serde_json::to_vec(&bad).unwrap();
        fs::write(&path, &bytes).unwrap();
        let hash: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
        let mut forged = bundle.clone(); forged["manifestSha256"] = json!(hash);
        let destination = dir.path().join(format!("invalid-{i}.pdf"));
        refused(save(&forged, &destination));
        assert!(!destination.exists());
    }
    fs::write(&path, original).unwrap();
    ok(save(&bundle, &dir.path().join("valid.pdf")));
}

#[path = "support/protected_fixture.rs"]
mod protected_fixture;

fn cli_password(verb: &str, input: &Path, args: &[&str], password: &str) -> Output {
    use std::{io::Write, process::Stdio};
    let mut process = Command::new(env!("CARGO_BIN_EXE_folio-cli"))
        .arg(verb).arg(input).arg("--engine").arg(renderer()).args(args).arg("--password-stdin")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    process.stdin.take().unwrap().write_all(password.as_bytes()).unwrap();
    process.wait_with_output().unwrap()
}

#[test]
fn nested_paragraph_owner_edits_retain_encryption_and_reader_restrictions() {
    let dir = tempfile::tempdir().unwrap();
    let plain = dir.path().join("plain.pdf");
    nested::write_fixture(&plain, nested::Mode::RepeatedOuter, 1.7);
    let source = dir.path().join("protected.pdf");
    fs::write(&source, protected_fixture::encrypt(&fs::read(&plain).unwrap(), lopdf::encryption::Permissions::PRINTABLE)).unwrap();
    let original = fs::read(&source).unwrap();
    refused(cli("inspect-nested-text", &source, &["--page","1"]));
    refused(cli_password("inspect-nested-text", &source, &["--page","1"], "reader"));
    let listed = ok(cli_password("inspect-nested-text", &source, &["--page","1"], "owner"));
    let seed = &listed["runs"][2];
    let selected = ok(cli_password("inspect-paragraph", &source, &["--page","1","--selection-id",seed["selectionId"].as_str().unwrap()], "owner"));
    let text = selected["paragraph"]["text"].as_str().unwrap().replacen("Nested", "Edited", 1);
    let request_path = dir.path().join("edit.json");
    fs::write(&request_path, serde_json::to_vec(&request(&selected, &text)).unwrap()).unwrap();
    let options = ["--request",request_path.to_str().unwrap(),"--out-dir",dir.path().to_str().unwrap()];
    refused(cli_password("preview-paragraph", &source, &options, "reader"));
    let bundle = ok(cli_password("preview-paragraph", &source, &options, "owner"));
    let saved = dir.path().join("saved.pdf");
    ok(save(&bundle, &saved));
    assert_eq!(fs::read(&saved).unwrap(), fs::read(bundle["candidate"].as_str().unwrap()).unwrap());
    refused(cli("inspect-text", &saved, &["--page","1"]));
    refused(cli_password("inspect-text", &saved, &["--page","1"], "reader"));
    let page = ok(cli_password("inspect-text", &saved, &["--page","1"], "owner"));
    let seed = page["runs"].as_array().unwrap().iter().find(|r| r["text"].as_str().unwrap().starts_with("Edited first")).unwrap();
    let after = ok(cli_password("inspect-paragraph", &saved, &["--page","1","--selection-id",seed["selectionId"].as_str().unwrap()], "owner"));
    assert_eq!(after["paragraph"]["text"], text);
    assert_eq!(after["paragraph"]["styles"][0]["fontSize"], selected["paragraph"]["styles"][0]["fontSize"]);
    let engine = PdfEngine::start(renderer()).unwrap();
    let a = engine.open_document_with_password(&source, "owner").unwrap();
    let b = engine.open_document_with_password(&saved, "owner").unwrap();
    for page in [1,2] {assert_eq!(engine.render_page(&a.id,page,900).unwrap(), engine.render_page(&b.id,page,900).unwrap());}
    engine.close_document(&a.id).unwrap(); engine.close_document(&b.id).unwrap();
    assert_eq!(fs::read(&source).unwrap(), original);
    protected_fixture::record("nested-paragraph-userunit-owner", &original, &fs::read(&saved).unwrap());
}

#[test]
fn nested_paragraph_crop_rotation_and_physical_units_match_native_publication() {
    use folio_engine::{ParagraphAlignment, StyledParagraphLayoutRequest, StyledParagraphRun};
    let dir = tempfile::tempdir().unwrap();
    let engine = PdfEngine::start(renderer()).unwrap();
    let mut outputs = Vec::new();
    for rotation in [0, 90, 180, 270] {
        let source = dir.path().join(format!("geometry-{rotation}.pdf"));
        nested::write_fixture(&source, nested::Mode::RepeatedInnerFar, 1.7);
        let mut pdf = lopdf::Document::load(&source).unwrap();
        let page = pdf.get_pages()[&1];
        let dictionary = pdf.get_dictionary_mut(page).unwrap();
        dictionary.set("Rotate", rotation);
        dictionary.set("CropBox", vec![lopdf::Object::Integer(10), 10.into(), 590.into(), 390.into()]);
        pdf.save(&source).unwrap();
        let original = fs::read(&source).unwrap();
        let info = engine.open_document(&source).unwrap();
        let listed = engine.list_occurrence_form_text_runs(&info.id,0).unwrap();
        let seed = &listed.runs[4];
        let projected = engine.list_form_paragraph_runs(&info.id,0,&seed.object_path,&seed.text).unwrap();
        let anchor = projected.runs.iter().find(|r| r.object_index == projected.anchor_index).unwrap();
        let native = engine.suggest_form_styled_paragraph(&info.id,0,&seed.object_path,anchor.object_index,&anchor.text).unwrap();
        let discovered = runs(&source,true);
        let selected = inspect(&source, discovered["runs"].as_array().unwrap().iter().find(|r| r["objectPath"] == json!(seed.object_path)).unwrap());
        assert_eq!(serde_json::from_value::<folio_engine::StyledParagraphSelection>(selected["paragraph"].clone()).unwrap(), native);
        let text = native.text.replacen("Nested", "Edited", 1);
        let mut direct = StyledParagraphLayoutRequest {baseline_anchor:native.baseline_anchor,object_indices:native.object_indices,expected_text:native.text,runs:vec![StyledParagraphRun{text:text.clone(),style_id:native.styles[0].style_id}],style_changes:vec![],bounds:native.bounds,alignment:native.alignment.unwrap_or(ParagraphAlignment::Left),line_spacing:native.line_spacing.unwrap_or(1.2)};
        direct.bounds = engine.fit_form_styled_paragraph(&info.id,0,&seed.object_path,&direct).unwrap();
        let changed = engine.preview_form_styled_paragraph_occurrence(&info.id,&[0,1,2],0,&seed.object_path,&direct).unwrap();
        let bundle = ok(preview(&source,dir.path(),&request(&selected,&text)));
        assert_eq!(serde_json::from_value::<folio_engine::AnnotationRect>(bundle["paragraph"]["bounds"].clone()).unwrap(), direct.bounds);
        let saved = dir.path().join(format!("geometry-{rotation}-saved.pdf"));
        ok(save(&bundle,&saved));
        let reopened = engine.open_document(&saved).unwrap();
        for page in 0..3 {assert_eq!(engine.render_page(&changed.document.id,page,900).unwrap(),engine.render_page(&reopened.id,page,900).unwrap());}
        for page in [1,2] {assert_eq!(engine.render_page(&info.id,page,900).unwrap(),engine.render_page(&reopened.id,page,900).unwrap());}
        for id in [&reopened.id,&changed.document.id,&info.id] {engine.close_document(id).unwrap();}
        assert_eq!(fs::read(&source).unwrap(),original);
        if let Some(root) = std::env::var_os("FOLIO_NESTED_AUTOMATION_CORPUS") {
            let root = PathBuf::from(root); fs::create_dir_all(&root).unwrap();
            for path in [&source,&saved] {let target=root.join(path.file_name().unwrap());assert!(!target.exists());fs::copy(path,target).unwrap();}
        }
        outputs.push(rotation);
    }
    assert_eq!(outputs.len(),4);
}

#[test]
fn nested_paragraph_preserves_internal_link_field_answer_default_and_appearance() {
    use lopdf::{dictionary, Document, Object, Stream};
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("graph.pdf");
    nested::write_fixture(&source, nested::Mode::RepeatedOuter, 1.);
    let mut pdf = Document::load(&source).unwrap();
    let pages = pdf.get_pages();
    let font = pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let appearance = pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),180.into(),24.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"Helv"=>font}}}, b"q 0.8 g 0 0 180 24 re f Q BT /Helv 12 Tf 0 g 3 8 Td (Retained answer) Tj ET".to_vec()));
    let widget = pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Tx","T"=>Object::string_literal("Retained field"),"V"=>Object::string_literal("Retained answer"),"DV"=>Object::string_literal("Retained default"),"DA"=>Object::string_literal("/Helv 12 Tf 0 g"),"Rect"=>vec![40.into(),40.into(),220.into(),64.into()],"P"=>pages[&1],"F"=>4,"AP"=>dictionary!{"N"=>appearance}});
    let link = pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Link","Rect"=>vec![240.into(),40.into(),290.into(),64.into()],"Dest"=>vec![Object::Reference(pages[&2]),Object::Name(b"Fit".to_vec())]});
    pdf.get_dictionary_mut(pages[&1]).unwrap().set("Annots",vec![Object::Reference(widget),Object::Reference(link)]);
    let form = pdf.add_object(dictionary!{"Fields"=>vec![Object::Reference(widget)],"DR"=>dictionary!{"Font"=>dictionary!{"Helv"=>font}},"DA"=>Object::string_literal("/Helv 12 Tf 0 g")});
    pdf.catalog_mut().unwrap().set("AcroForm",form);
    pdf.save(&source).unwrap();
    let original = fs::read(&source).unwrap();
    let listed = runs(&source,true);
    let selected = inspect(&source,right_seed(&listed));
    let text = selected["paragraph"]["text"].as_str().unwrap().replacen("Nested","Edited",1);
    let bundle = ok(preview(&source,dir.path(),&request(&selected,&text)));
    let saved = dir.path().join("graph-saved.pdf");
    ok(save(&bundle,&saved));
    let pdf = Document::load(&saved).unwrap();
    let pages = pdf.get_pages();
    let annotations = pdf.get_dictionary(pages[&1]).unwrap().get(b"Annots").unwrap().as_array().unwrap();
    assert_eq!(annotations.len(),2);
    let fields: Vec<_> = annotations.iter().map(|v|pdf.get_dictionary(v.as_reference().unwrap()).unwrap()).collect();
    let widget = fields.iter().find(|v|v.get(b"Subtype").unwrap().as_name().unwrap()==b"Widget").unwrap();
    assert_eq!(widget.get(b"V").unwrap().as_str().unwrap(),b"Retained answer");
    assert_eq!(widget.get(b"DV").unwrap().as_str().unwrap(),b"Retained default");
    let link = fields.iter().find(|v|v.get(b"Subtype").unwrap().as_name().unwrap()==b"Link").unwrap();
    assert_eq!(link.get(b"Dest").unwrap().as_array().unwrap()[0].as_reference().unwrap(),pages[&2]);
    let before = image::open(bundle["before"].as_str().unwrap()).unwrap().to_rgba8();
    let after = image::open(bundle["after"].as_str().unwrap()).unwrap().to_rgba8();
    let scale = before.width() as f32/600.;
    let mut witness = 0;
    for (x,y,pixel) in before.enumerate_pixels() {
        if (x as f32) < 300.*scale || (y as f32) > 250.*scale {assert_eq!(pixel,after.get_pixel(x,y));}
        else if pixel != after.get_pixel(x,y) {witness+=1;}
    }
    assert!(witness>0);
    preserve_others(&source,&saved);
    assert_eq!(fs::read(&source).unwrap(),original);
    if let Some(root) = std::env::var_os("FOLIO_NESTED_AUTOMATION_CORPUS") {
        let root = PathBuf::from(root);fs::create_dir_all(&root).unwrap();
        for path in [&source,&saved] {let target=root.join(path.file_name().unwrap());assert!(!target.exists());fs::copy(path,target).unwrap();}
    }
}
