use folio_engine::PdfEngine;
use lopdf::{dictionary, Document, Object, Stream};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};
#[path = "support/protected_fixture.rs"]
mod protected_fixture;
fn renderer() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")
}
fn cli(command: &str, input: &Path, args: &[&str], password: Option<&str>) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_folio-cli"));
    cmd.arg(command)
        .arg(input)
        .arg("--engine")
        .arg(renderer())
        .args(args);
    if password.is_some() {
        cmd.arg("--password-stdin");
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(p) = password {
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(p.as_bytes())
            .unwrap();
    }
    drop(child.stdin.take());
    child.wait_with_output().unwrap()
}
fn ok(out: Output) -> Value {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stderr.is_empty());
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["ok"], true);
    v["result"].clone()
}
fn refused(out: Output) {
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    let v: Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(v["ok"], false);
}
fn fixture(path: &Path) {
    let mut doc = Document::with_version("1.7");
    let pages = doc.new_object_id();
    let font =
        doc.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let mut kids = Vec::<Object>::new();
    for text in ["Old content", "Other page"] {
        let stream = doc.add_object(Stream::new(
            dictionary! {},
            format!("BT /F1 14 Tf 30 250 Td ({text}) Tj 0 -60 Td (Neighbor) Tj ET").into_bytes(),
        ));
        let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),400.into(),400.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>stream});
        kids.push(page.into());
    }
    let note=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![300.into(),300.into(),320.into(),320.into()],"Contents"=>Object::string_literal("Keep note")});
    doc.get_dictionary_mut(kids[1].as_reference().unwrap())
        .unwrap()
        .set("Annots", vec![Object::Reference(note)]);
    let outlines = doc.new_object_id();
    let bookmark=doc.add_object(dictionary!{"Title"=>Object::string_literal("Other page"),"Parent"=>outlines,"Dest"=>vec![kids[1].clone(),Object::Name(b"Fit".to_vec())]});
    doc.objects.insert(
        outlines,
        dictionary! {"Type"=>"Outlines","First"=>bookmark,"Last"=>bookmark,"Count"=>1}.into(),
    );
    doc.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Kids"=>kids,"Count"=>2}.into(),
    );
    let root = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages,"Outlines"=>outlines});
    doc.trailer.set("Root", root);
    doc.save(path).unwrap();
}

fn inspect(source: &Path, password: Option<&str>) -> Value {
    let runs = ok(cli("inspect-text", source, &["--page", "1"], password));
    let id = runs["runs"][0]["selectionId"].as_str().unwrap();
    ok(cli(
        "inspect-paragraph",
        source,
        &["--page", "1", "--selection-id", id],
        password,
    ))
}
fn edit(selected: &Value, text: &str) -> Value {
    json!({"page":1,"seedSelectionId":selected["seedSelectionId"],"paragraphId":selected["paragraphId"],"runs":[{"text":text,"styleId":selected["paragraph"]["styles"][0]["styleId"]}]})
}
fn preview(source: &Path, parent: &Path, request: &Value, password: Option<&str>) -> Output {
    let file = parent.join("edit.json");
    fs::write(&file, serde_json::to_vec(request).unwrap()).unwrap();
    cli(
        "preview-paragraph",
        source,
        &[
            "--request",
            file.to_str().unwrap(),
            "--out-dir",
            parent.to_str().unwrap(),
        ],
        password,
    )
}
fn save(bundle: &Value, path: &Path) -> Output {
    cli(
        "save-text-preview",
        Path::new(bundle["folder"].as_str().unwrap()),
        &[
            "--manifest-sha256",
            bundle["manifestSha256"].as_str().unwrap(),
            "--out",
            path.to_str().unwrap(),
        ],
        None,
    )
}
#[test]
fn cli_changed_ascent_fit_and_explicit_move_preserve_baseline_intent() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pdf");
    fixture(&source);
    let mut pdf = Document::load(&source).unwrap();
    for object in pdf.objects.values_mut() {
        if let Ok(dict) = object.as_dict_mut() {
            if dict.get(b"BaseFont").is_ok() { dict.set("BaseFont", Object::Name(b"Courier".to_vec())); }
        }
        if let Ok(stream) = object.as_stream_mut() {
            let bytes = stream.get_plain_content().unwrap();
            if let Ok(text) = String::from_utf8(bytes) {
                if text.contains("Old content") { stream.set_plain_content(text.replace("Old content", "AA BB").into_bytes()); }
            }
        }
    }
    pdf.save(&source).unwrap();
    let original = fs::read(&source).unwrap();
    let selected = inspect(&source, None);
    let anchor = selected["paragraph"]["baselineAnchor"].clone();
    assert_eq!(anchor["y"], json!(150.));
    let mut request = edit(&selected, "AA BB CC");
    request["fit"] = json!(true);
    let bundle = ok(preview(&source, dir.path(), &request, None));
    let saved = dir.path().join("saved.pdf"); ok(save(&bundle, &saved));
    let next = inspect(&saved, None);
    assert_eq!(next["paragraph"]["baselineAnchor"], anchor);
    let mut moved = edit(&next, "AA BB");
    let mut bounds = next["paragraph"]["bounds"].clone();
    bounds["y"] = json!(bounds["y"].as_f64().unwrap() + 10.);
    moved["bounds"] = bounds; moved["fit"] = json!(true);
    let bundle = ok(preview(&saved, dir.path(), &moved, None));
    let final_path = dir.path().join("moved.pdf"); ok(save(&bundle, &final_path));
    let final_selection = inspect(&final_path, None);
    assert_eq!(final_selection["paragraph"]["baselineAnchor"]["y"], json!(160.));
    assert_eq!(fs::read(&source).unwrap(), original);
}
#[test]
fn explicit_font_override_does_not_bypass_reader_edit_restrictions() {
    use lopdf::encryption::Permissions;
    let dir=tempfile::tempdir().unwrap();let plain=dir.path().join("plain.pdf");fixture(&plain);
    let source=dir.path().join("protected.pdf");
    let bytes=protected_fixture::encrypt(&fs::read(&plain).unwrap(),Permissions::COPYABLE);
    fs::write(&source,&bytes).unwrap();let selected=inspect(&source,Some("owner"));
    let font=Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf");
    use sha2::{Digest,Sha256};
    let id:String=Sha256::digest(fs::read(&font).unwrap()).iter().map(|b|format!("{b:02x}")).collect();
    let mut request=edit(&selected,selected["paragraph"]["text"].as_str().unwrap());
    request["fontOverrides"]=json!([{"styleId":selected["paragraph"]["styles"][0]["styleId"],"fontId":id}]);
    request["fit"]=json!(true);let path=dir.path().join("edit.json");fs::write(&path,serde_json::to_vec(&request).unwrap()).unwrap();
    let out=cli("preview-paragraph",&source,&["--request",path.to_str().unwrap(),"--out-dir",dir.path().to_str().unwrap(),"--font",font.to_str().unwrap()],Some("reader"));
    assert!(!out.status.success());assert!(out.stdout.is_empty());
    let error:Value=serde_json::from_slice(&out.stderr).unwrap();assert_eq!(error["error"]["code"],"permission_denied");
    assert_eq!(fs::read(source).unwrap(),bytes);
}
#[test]
fn paragraph_wrap_save_and_reedit_preserve_original_and_complete_source() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pdf");
    fixture(&source);
    let original = fs::read(&source).unwrap();
    let selected = inspect(&source, None);
    let text = "Old content with additional words that wrap inside the chosen paragraph area.";
    let mut request = edit(&selected, text);
    let mut bounds = selected["paragraph"]["bounds"].clone();
    bounds["width"] = json!(170.);
    bounds["height"] = json!(50.);
    request["bounds"] = bounds.clone();
    let bundle = ok(preview(&source, dir.path(), &request, None));
    assert_eq!(bundle["paragraph"]["bounds"], bounds);
    let manifest: Value = serde_json::from_slice(
        &fs::read(Path::new(bundle["folder"].as_str().unwrap()).join("manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["version"], 2);
    assert_eq!(manifest["selectionId"], selected["paragraphId"]);
    assert_eq!(manifest["paragraph"]["runs"], request["runs"]);
    let saved = dir.path().join("saved.pdf");
    ok(save(&bundle, &saved));
    assert_eq!(
        fs::read(&saved).unwrap(),
        fs::read(bundle["candidate"].as_str().unwrap()).unwrap()
    );
    let next = inspect(&saved, None);
    assert_eq!(next["paragraph"]["text"], text);
    assert_ne!(next["paragraphId"], selected["paragraphId"]);
    let after = ok(cli("inspect-text", &saved, &["--page", "1"], None));
    assert!(
        after["runs"].as_array().unwrap().len() >= 4,
        "Text must really wrap plus neighbor: {after}"
    );
    let style = &next["paragraph"]["styles"][0];
    let before_style = &selected["paragraph"]["styles"][0];
    for key in ["fontName", "fontSize", "color"] {
        assert_eq!(style[key], before_style[key]);
    }
    let unchanged = ok(preview(&saved, dir.path(), &edit(&next, text), None));
    assert_eq!(
        fs::read(unchanged["before"].as_str().unwrap()).unwrap(),
        fs::read(unchanged["after"].as_str().unwrap()).unwrap(),
        "No-op re-edit must retain saved pixels"
    );
    let second_text = text.replace("Old", "New");
    let mut second_request = edit(&next, &second_text);
    second_request["fit"] = json!(true);
    let second = ok(preview(&saved, dir.path(), &second_request, None));
    let final_path = dir.path().join("final.pdf");
    ok(save(&second, &final_path));
    assert_eq!(inspect(&final_path, None)["paragraph"]["text"], second_text);
    assert_eq!(fs::read(&source).unwrap(), original);
    let engine = PdfEngine::start(renderer()).unwrap();
    let a = engine.open_document(&source).unwrap();
    let b = engine.open_document(&final_path).unwrap();
    assert_eq!(a.pages.len(), b.pages.len());
    assert_eq!(
        engine.render_page(&a.id, 1, 800).unwrap(),
        engine.render_page(&b.id, 1, 800).unwrap()
    );
    engine.close_document(&a.id).unwrap();
    engine.close_document(&b.id).unwrap();
    let pdf = Document::load(&final_path).unwrap();
    assert!(pdf.catalog().unwrap().has(b"Outlines"));
    let page = pdf.get_pages()[&2];
    assert!(pdf.get_dictionary(page).unwrap().has(b"Annots"));
}

#[test]
fn invalid_or_stale_paragraph_requests_leave_no_preview_folders() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pdf");
    fixture(&source);
    let original = fs::read(&source).unwrap();
    let selected = inspect(&source, None);
    let valid = edit(&selected, "Updated content");
    let mut cases = vec![];
    for field in ["bounds", "alignment", "lineSpacing", "fit"] {
        let mut bad = valid.clone();
        bad[field] = Value::Null;
        cases.push(bad);
    }
    let mut bad = valid.clone();
    bad["paragraphId"] = json!(format!("paragraph-v1-{}", "0".repeat(64)));
    cases.push(bad);
    let mut bad = valid.clone();
    bad["runs"][0]["styleId"] = json!(63);
    cases.push(bad);
    let mut bad = valid.clone();
    bad["runs"][0]["fontId"] = json!("caller-font");
    cases.push(bad);
    let mut bad = valid.clone();
    bad["bounds"] = json!({"x":0,"y":0,"width":100,"height":100,"extra":true});
    cases.push(bad);
    let mut bad = valid.clone();
    bad["runs"][0]["text"] = json!("x".repeat(4097));
    cases.push(bad);
    let mut bad = valid.clone();
    bad["runs"] = json!([{"text":"a\rb","styleId":0}]);
    cases.push(bad);
    let mut bad = valid.clone();
    bad["lineSpacing"] = json!(0.59);
    cases.push(bad);
    let mut bad = valid.clone();
    bad["runs"] = json!(vec![json!({"text":"","styleId":0}); 1025]);
    cases.push(bad);
    let mut bad = valid.clone();
    bad["runs"] = json!(vec![json!({"text":"\u{0001}".repeat(4),"styleId":0}); 1024]);
    cases.push(bad);
    for bad in cases {
        refused(preview(&source, dir.path(), &bad, None));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }
    let mut changed = original.clone();
    changed.extend_from_slice(b"\n% changed source\n");
    fs::write(&source, &changed).unwrap();
    refused(preview(&source, dir.path(), &valid, None));
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    assert_eq!(fs::read(source).unwrap(), changed);
}

#[test]
fn fitting_is_explicit_and_reports_the_actual_native_area() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pdf");
    fixture(&source);
    let selected = inspect(&source, None);
    let mut request = edit(&selected, "Old content with additional words");
    refused(preview(&source, dir.path(), &request, None));
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    request["fit"] = json!(true);
    let bundle = ok(preview(&source, dir.path(), &request, None));
    assert_ne!(
        bundle["paragraph"]["bounds"],
        selected["paragraph"]["bounds"]
    );
    let saved = dir.path().join("fitted.pdf");
    ok(save(&bundle, &saved));
    assert_eq!(
        inspect(&saved, None)["paragraph"]["text"],
        "Old content with additional words"
    );
}

#[test]
fn mixed_fonts_and_highlights_survive_reflow_and_reinspection() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("mixed.pdf");
    fixture(&source);
    let mut pdf = Document::load(&source).unwrap();
    let page = pdf.get_pages()[&1];
    let regular =
        pdf.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
    let bold = pdf
        .add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica-Bold"});
    let stream=pdf.add_object(Stream::new(dictionary!{},b"q 1 1 0 rg 70 246 21 16 re f Q BT /F1 12 Tf 50 250 Td (AA ) Tj ET BT /F2 12 Tf 72 250 Td (BB) Tj ET BT /F1 12 Tf 90 250 Td ( CC) Tj ET BT /F1 12 Tf 50 80 Td (Neighbor) Tj ET".to_vec()));
    let dict = pdf.get_dictionary_mut(page).unwrap();
    dict.set(
        "Resources",
        dictionary! {"Font"=>dictionary!{"F1"=>regular,"F2"=>bold}},
    );
    dict.set("Contents", stream);
    pdf.save(&source).unwrap();
    let original = fs::read(&source).unwrap();
    let selected = inspect(&source, None);
    let styles = selected["paragraph"]["styles"].as_array().unwrap();
    assert!(styles.len() > 1);
    assert!(styles.iter().any(|s| s["backgroundColor"] == "#FFFF00"));
    let mut runs = selected["paragraph"]["runs"].clone();
    for run in runs.as_array_mut().unwrap() {
        if run["text"].as_str().unwrap().contains("BB") {
            run["text"] = json!("BB highlighted words that wrap");
        }
    }
    let mut request = edit(&selected, "");
    request["runs"] = runs.clone();
    let mut bounds = selected["paragraph"]["bounds"].clone();
    bounds["width"] = json!(110.);
    bounds["height"] = json!(100.);
    request["bounds"] = bounds;
    let bundle = ok(preview(&source, dir.path(), &request, None));
    let saved = dir.path().join("saved.pdf");
    ok(save(&bundle, &saved));
    let after = inspect(&saved, None);
    let expected: String = runs
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["text"].as_str().unwrap())
        .collect();
    assert_eq!(after["paragraph"]["text"], expected);
    let after_styles = after["paragraph"]["styles"].as_array().unwrap();
    for style in styles {
        assert!(after_styles
            .iter()
            .any(|s| ["fontName", "fontSize", "color", "backgroundColor"]
                .into_iter()
                .all(|key| s[key] == style[key])));
    }
    assert_eq!(fs::read(source).unwrap(), original);
}

#[test]
fn paragraph_owner_preview_retains_encryption_and_save_rejects_invalid_v2_metadata() {
    use lopdf::encryption::Permissions;
    use sha2::{Digest, Sha256};
    let dir = tempfile::tempdir().unwrap();
    let plain = dir.path().join("plain.pdf");
    fixture(&plain);
    let source = dir.path().join("locked.pdf");
    fs::write(
        &source,
        protected_fixture::encrypt(&fs::read(&plain).unwrap(), Permissions::PRINTABLE),
    )
    .unwrap();
    let original = fs::read(&source).unwrap();
    let selected = inspect(&source, Some("owner\n"));
    let mut request = edit(&selected, "Old content expanded through paragraph editing");
    request["fit"] = json!(true);
    let bundle = ok(preview(&source, dir.path(), &request, Some("owner\n")));
    let saved = dir.path().join("saved.pdf");
    ok(save(&bundle, &saved));
    refused(cli("inspect-text", &saved, &["--page", "1"], None));
    refused(cli(
        "inspect-text",
        &saved,
        &["--page", "1"],
        Some("reader\n"),
    ));
    assert_eq!(
        inspect(&saved, Some("owner\n"))["paragraph"]["text"],
        "Old content expanded through paragraph editing"
    );
    assert_eq!(fs::read(&source).unwrap(), original);
    let path = Path::new(bundle["folder"].as_str().unwrap()).join("manifest.json");
    let manifest: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    for case in 0..4 {
        let mut bad = manifest.clone();
        match case {
            0 => bad["version"] = json!(1),
            1 => bad["paragraph"] = Value::Null,
            2 => bad["replacement"] = json!("different summary"),
            _ => bad["paragraph"]["bounds"]["extra"] = json!(true),
        }
        let bytes = serde_json::to_vec(&bad).unwrap();
        let hash: String = Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        fs::write(&path, bytes).unwrap();
        let mut changed = bundle.clone();
        changed["manifestSha256"] = json!(hash);
        let destination = dir.path().join(format!("refused-{case}.pdf"));
        refused(save(&changed, &destination));
        assert!(!destination.exists());
    }
}


#[test]
fn unchanged_paragraph_runs_and_echoed_bounds_retain_exact_input_identity() {
    use lopdf::encryption::Permissions;
    for protected in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.pdf");
        fixture(&source);
        if protected {
            let bytes = protected_fixture::encrypt(&fs::read(&source).unwrap(), Permissions::MODIFIABLE | Permissions::COPYABLE);
            fs::write(&source, bytes).unwrap();
        }
        let password = protected.then_some("reader\n");
        let original = fs::read(&source).unwrap();
        let selected = inspect(&source, password);
        for variant in 0..4 {
            let mut request = json!({"page":1,"seedSelectionId":selected["seedSelectionId"],
                "paragraphId":selected["paragraphId"],"runs":selected["paragraph"]["runs"]});
            match variant {
                1 => {
                    let runs: Vec<Value> = selected["paragraph"]["runs"].as_array().unwrap().iter().flat_map(|run| {
                        run["text"].as_str().unwrap().chars().map(move |ch| json!({"text":ch.to_string(),"styleId":run["styleId"]}))
                    }).collect();
                    request["runs"] = json!(runs);
                }
                2 => request["runs"].as_array_mut().unwrap().push(json!({"text":"","styleId":selected["paragraph"]["styles"][0]["styleId"]})),
                3 => request["bounds"] = selected["paragraph"]["bounds"].clone(),
                _ => {},
            }
            let bundle = ok(preview(&source, dir.path(), &request, password));
            let saved = dir.path().join(format!("saved-{variant}.pdf"));
            ok(save(&bundle, &saved));
            assert_eq!(fs::read(&saved).unwrap(), original, "No-op variant {variant}, protected={protected}");
            assert_eq!(inspect(&saved, password), selected);
            assert_eq!(fs::read(bundle["before"].as_str().unwrap()).unwrap(), fs::read(bundle["after"].as_str().unwrap()).unwrap());
        }
        assert_eq!(fs::read(source).unwrap(), original);
    }
}

#[test]
fn unchanged_paragraph_does_not_bypass_reader_permissions_with_owner_metadata() {
    use lopdf::encryption::Permissions;
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("restricted.pdf");
    fixture(&source);
    let bytes = protected_fixture::encrypt(&fs::read(&source).unwrap(), Permissions::COPYABLE);
    fs::write(&source, &bytes).unwrap();
    let selected = inspect(&source, Some("owner\n"));
    let request = edit(&selected, selected["paragraph"]["text"].as_str().unwrap());
    let denied = preview(&source, dir.path(), &request, Some("reader\n"));
    assert!(!denied.status.success());
    assert_eq!(serde_json::from_slice::<Value>(&denied.stderr).unwrap()["error"]["code"], "permission_denied");
    assert_eq!(fs::read(source).unwrap(), bytes);
}


#[test]
fn explicit_automation_baselines_survive_fit_echo_rotation_and_movement() {
    for rotation in [0, 90, 180, 270] {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.pdf"); fixture(&source);
        let mut pdf = Document::load(&source).unwrap();
        for id in pdf.get_pages().values() {
            let page = pdf.get_dictionary_mut(*id).unwrap();
            page.set("Rotate", rotation); page.set("UserUnit", 1.7_f32);
        }
        pdf.save(&source).unwrap(); let original = fs::read(&source).unwrap();
        let selected = inspect(&source, None);
        let axis = if rotation == 0 || rotation == 180 { "y" } else { "x" };
        let expected = match rotation { 0 | 270 => 255., _ => 425. };
        assert!((selected["paragraph"]["baselineAnchor"][axis].as_f64().unwrap()-expected).abs()<0.0001);
        let mut request = edit(&selected, "gg pq");
        request["fit"] = json!(true);
        request["baselineAnchor"] = selected["paragraph"]["baselineAnchor"].clone();
        let first = ok(preview(&source, dir.path(), &request, None));
        request["fit"] = json!(false);
        request["bounds"] = first["paragraph"]["bounds"].clone();
        let echoed = ok(preview(&source, dir.path(), &request, None));
        assert_eq!(image::open(first["after"].as_str().unwrap()).unwrap().to_rgba8(),
            image::open(echoed["after"].as_str().unwrap()).unwrap().to_rgba8());
        request["bounds"][axis] = json!(request["bounds"][axis].as_f64().unwrap()+10.);
        request["baselineAnchor"][axis] = json!(expected+10.);
        request["fit"] = json!(true);
        request["runs"][0]["text"] = json!("AA BB");
        let moved = ok(preview(&source, dir.path(), &request, None));
        let saved = dir.path().join("moved.pdf"); ok(save(&moved, &saved));
        let after = inspect(&saved, None);
        assert!((after["paragraph"]["baselineAnchor"][axis].as_f64().unwrap()-(expected+10.)).abs()<0.0001);
        assert_eq!(fs::read(&source).unwrap(), original);
    }
}

#[test]
fn automation_baseline_validation_and_unchanged_identity() {
    let dir = tempfile::tempdir().unwrap(); let source=dir.path().join("source.pdf");
    fixture(&source); let original=fs::read(&source).unwrap(); let selected=inspect(&source,None);
    let mut request=edit(&selected,selected["paragraph"]["text"].as_str().unwrap());
    for bad in [Value::Null,json!({"x":30}),json!({"x":30,"y":150,"extra":true}),
        json!({"x":30,"y":1_000_001}),json!({"x":30,"y":-1_000_001}),json!({"x":30,"y":0})] {
        request["baselineAnchor"]=bad; refused(preview(&source,dir.path(),&request,None));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(),2);
    }
    request["baselineAnchor"]=selected["paragraph"]["baselineAnchor"].clone();
    request["bounds"]=selected["paragraph"]["bounds"].clone();
    let unchanged=ok(preview(&source,dir.path(),&request,None));
    assert_eq!(fs::read(unchanged["candidate"].as_str().unwrap()).unwrap(),original);
    request["baselineAnchor"]["y"]=json!(160.);
    request["bounds"]["y"]=json!(request["bounds"]["y"].as_f64().unwrap()+10.);
    let moved=ok(preview(&source,dir.path(),&request,None));
    assert_ne!(fs::read(moved["candidate"].as_str().unwrap()).unwrap(),original);
    assert_ne!(image::open(moved["before"].as_str().unwrap()).unwrap().to_rgba8(),
        image::open(moved["after"].as_str().unwrap()).unwrap().to_rgba8());
    assert_eq!(fs::read(source).unwrap(),original);
}


#[test]
fn area_resize_retains_source_baseline_when_replacement_ascent_changes() {
    let dir=tempfile::tempdir().unwrap(); let source=dir.path().join("source.pdf");
    fixture(&source); let original=fs::read(&source).unwrap(); let selected=inspect(&source,None);
    let mut request=edit(&selected,"gg pq");
    let mut bounds=selected["paragraph"]["bounds"].clone();
    bounds["width"]=json!(bounds["width"].as_f64().unwrap()+20.);
    bounds["height"]=json!(bounds["height"].as_f64().unwrap()+10.);
    request["bounds"]=bounds; request["fit"]=json!(true);
    let bundle=ok(preview(&source,dir.path(),&request,None));
    let saved=dir.path().join("resized.pdf"); ok(save(&bundle,&saved));
    assert_eq!(inspect(&saved,None)["paragraph"]["baselineAnchor"]["y"],json!(150.));
    assert_eq!(fs::read(source).unwrap(),original);
}
