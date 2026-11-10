use folio_engine::PdfEngine;
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};
#[path = "support/nested_automation.rs"]
mod nested;
#[path = "support/protected_fixture.rs"]
mod protected_fixture;

fn renderer() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")
}
fn cli(verb: &str, input: &Path, args: &[&str], password: Option<&str>) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_folio-cli"));
    cmd.arg(verb)
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
    if let Some(password) = password {
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(password.as_bytes())
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
    let frame: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(frame["ok"], true);
    frame["result"].clone()
}
fn refused(out: Output) {
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    let frame: Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(frame["ok"], false);
}
fn inspect(source: &Path, page: &str) -> Value {
    ok(cli("inspect-nested-text", source, &["--page", page], None))
}
fn preview(source: &Path, parent: &Path, page: u32, id: &str, replacement: &str) -> Output {
    let request = parent.join("edit.json");
    fs::write(
        &request,
        serde_json::to_vec(&json!({"page":page,"selectionId":id,"replacement":replacement}))
            .unwrap(),
    )
    .unwrap();
    cli(
        "preview-text",
        source,
        &[
            "--request",
            request.to_str().unwrap(),
            "--out-dir",
            parent.to_str().unwrap(),
        ],
        None,
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
fn nested_discovery_is_stable_and_page_discovery_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pdf");
    nested::write_fixture(&source, nested::Mode::RepeatedOuter, 1.);
    let bytes = fs::read(&source).unwrap();
    let page = ok(cli("inspect-text", &source, &["--page", "1"], None));
    assert_eq!(page["totalRuns"], 1);
    assert_eq!(page["runs"][0]["text"], "Page heading");
    let first = inspect(&source, "1");
    assert_eq!(first, inspect(&source, "1"));
    assert_eq!(first["totalRuns"], 4);
    assert_eq!(first["selectionKind"], "nested");
    let runs = first["runs"].as_array().unwrap();
    let ids = runs
        .iter()
        .map(|r| r["selectionId"].as_str().unwrap())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(ids.len(), 4);
    for id in &ids {
        assert!(id.starts_with("nested-text-v1-"));
        assert_eq!(id.len(), 79);
    }
    for (i, run) in runs.iter().enumerate() {
        assert_eq!(
            run["text"],
            if i % 2 == 0 {
                "Nested first line"
            } else {
                "Nested second line"
            }
        );
        assert!(run["objectPath"].as_array().unwrap().len() >= 2);
    }
    for run in inspect(&source, "2")["runs"].as_array().unwrap() {
        assert!(!ids.contains(run["selectionId"].as_str().unwrap()));
    }
    let copy = dir.path().join("same bytes.pdf");
    fs::write(&copy, &bytes).unwrap();
    assert_eq!(first, inspect(&copy, "1"));
    let mut altered = bytes.clone();
    altered.extend_from_slice(b"\n% different source bytes\n");
    fs::write(&copy, altered).unwrap();
    assert_ne!(
        first["runs"][0]["selectionId"],
        inspect(&copy, "1")["runs"][0]["selectionId"]
    );
    assert_eq!(
        ok(cli(
            "inspect-nested-text",
            &source,
            &["--page", "1", "--offset", "4"],
            None
        ))["runs"],
        json!([])
    );
    refused(cli(
        "inspect-nested-text",
        &source,
        &["--page", "1", "--offset", "5"],
        None,
    ));
    assert_eq!(
        page,
        ok(cli("inspect-text", &source, &["--page", "1"], None))
    );
    assert_eq!(fs::read(&source).unwrap(), bytes);
}

#[test]
fn nested_text_save_reedit_isolates_reused_occurrences() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pdf");
    nested::write_fixture(&source, nested::Mode::RepeatedOuter, 1.);
    let bytes = fs::read(&source).unwrap();
    let original = inspect(&source, "1");
    let selected = &original["runs"][2];
    assert!(selected["bounds"]["x"].as_f64().unwrap() > 300.);
    let first = ok(preview(
        &source,
        dir.path(),
        1,
        selected["selectionId"].as_str().unwrap(),
        "Edited first line",
    ));
    let manifest: Value = serde_json::from_slice(
        &fs::read(Path::new(first["folder"].as_str().unwrap()).join("manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["version"], 4);
    let saved = dir.path().join("saved.pdf");
    let receipt = ok(save(&first, &saved));
    assert_eq!(receipt["sha256"], first["candidateSha256"]);
    let after = inspect(&saved, "1");
    assert_eq!(after["runs"][2]["text"], "Edited first line");
    for i in [0, 1, 3] {
        assert_eq!(after["runs"][i]["text"], original["runs"][i]["text"]);
        assert_eq!(after["runs"][i]["bounds"], original["runs"][i]["bounds"]);
    }
    refused(preview(
        &saved,
        dir.path(),
        1,
        selected["selectionId"].as_str().unwrap(),
        "Stale",
    ));
    let second = ok(preview(
        &saved,
        dir.path(),
        1,
        after["runs"][2]["selectionId"].as_str().unwrap(),
        "Third first line",
    ));
    let revised = dir.path().join("revised.pdf");
    ok(save(&second, &revised));
    assert_eq!(
        inspect(&revised, "1")["runs"][2]["text"],
        "Third first line"
    );
    let engine = PdfEngine::start(renderer()).unwrap();
    let a = engine.open_document(&source).unwrap();
    let b = engine.open_document(&revised).unwrap();
    for page in [1, 2] {
        assert_eq!(
            engine.render_page(&a.id, page, 1200).unwrap(),
            engine.render_page(&b.id, page, 1200).unwrap()
        );
    }
    engine.close_document(&a.id).unwrap();
    engine.close_document(&b.id).unwrap();
    refused(save(&second, &revised));
    assert_eq!(fs::read(&source).unwrap(), bytes);
}

#[test]
fn nested_selection_stale_cross_page_and_permission_failures() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pdf");
    nested::write_fixture(&source, nested::Mode::RepeatedOuter, 1.);
    let original = fs::read(&source).unwrap();
    let listed = inspect(&source, "1");
    let id = listed["runs"][0]["selectionId"].as_str().unwrap();
    refused(preview(&source, dir.path(), 2, id, "Wrong page"));
    refused(preview(
        &source,
        dir.path(),
        1,
        &id.replace("nested-text-v1-", "text-v1-"),
        "Wrong kind",
    ));
    let request = dir.path().join("forged.json");
    fs::write(
        &request,
        serde_json::to_vec(
            &json!({"page":1,"selectionId":id,"replacement":"Changed","objectPath":[0,0]}),
        )
        .unwrap(),
    )
    .unwrap();
    refused(cli(
        "preview-text",
        &source,
        &[
            "--request",
            request.to_str().unwrap(),
            "--out-dir",
            dir.path().to_str().unwrap(),
        ],
        None,
    ));
    assert_eq!(fs::read(&source).unwrap(), original);
    let protected = dir.path().join("restricted.pdf");
    for rights in [
        lopdf::encryption::Permissions::COPYABLE,
        lopdf::encryption::Permissions::MODIFIABLE,
    ] {
        fs::write(&protected, protected_fixture::encrypt(&original, rights)).unwrap();
        refused(cli(
            "inspect-nested-text",
            &protected,
            &["--page", "1"],
            Some("reader"),
        ));
    }
}

#[path = "support/affine_forms.rs"]
mod affine_corpus;

#[test]
fn nested_automation_matches_native_affine_subset_and_cid_edits() {
    use folio_engine::{ParagraphAlignment, StyledParagraphLayoutRequest, StyledParagraphRun};
    let dir = tempfile::tempdir().unwrap();
    let engine = PdfEngine::start(renderer()).unwrap();
    let mut successes = 0;
    let mut paragraph_successes = 0;
    let mut records = Vec::new();
    for (kind, case, rotation) in [
        ("standard", "plain", 0), ("simple", "plain", 0), ("cid", "plain", 0),
        ("standard", "generated-space", 0), ("cid", "generated-space", 0),
        ("standard", "page-quarter", 0), ("standard", "form-shear", 90),
        ("standard", "mirror", 180), ("standard", "composed", 270),
        ("simple", "page-quarter", 0), ("simple", "form-rotation", 90),
        ("cid", "page-quarter", 0), ("cid", "composed", 90),
        ("cid", "tight-rotation", 0),
    ] {
        let name = format!("{kind}-{case}-{rotation}");
        let source = dir.path().join(format!("{name}.pdf"));
        affine_corpus::fixture(&source, kind, case, rotation, "Cafe old");
        if case == "generated-space" {
            use lopdf::{content::{Content, Operation}, Document, Object};
            let mut pdf = Document::load(&source).unwrap();
            for object in pdf.objects.values_mut() {
                let Ok(stream) = object.as_stream_mut() else {continue};
                if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Form") {continue;}
                let mut content = Content::decode(&stream.get_plain_content().unwrap()).unwrap();
                for op in &mut content.operations {
                    if op.operator != "Tj" {continue;}
                    let bytes = op.operands[0].as_str().unwrap();
                    let width = if kind == "cid" {2} else {1};
                    let split = bytes.chunks_exact(width).position(|c| c == (if width == 2 {&[0,32][..]} else {&[32][..]}));
                    if let Some(split) = split {
                        let at = split * width;
                        *op = Operation::new("TJ", vec![Object::Array(vec![Object::string_literal(bytes[..at].to_vec()),Object::Integer(-250),Object::string_literal(bytes[at+width..].to_vec())])]);
                    }
                }
                stream.set_plain_content(content.encode().unwrap());
            }
            pdf.save(&source).unwrap();
        }
        let original = fs::read(&source).unwrap();
        let info = engine.open_document(&source).unwrap();
        let native_runs = engine.list_occurrence_form_text_runs(&info.id, 0).unwrap();
        let inspected = inspect(&source, "1");
        assert_eq!(inspected["totalRuns"], native_runs.runs.len(), "{name}");
        let Some(seed) = native_runs.runs.first() else {
            assert!(inspected["runs"].as_array().unwrap().is_empty());
            assert_eq!(fs::read(&source).unwrap(), original);
            engine.close_document(&info.id).unwrap();
            if let Some(root) = std::env::var_os("FOLIO_NESTED_AUTOMATION_CORPUS") {
                let root = PathBuf::from(root);fs::create_dir_all(&root).unwrap();
                let target = root.join(source.file_name().unwrap());assert!(!target.exists());fs::copy(&source,target).unwrap();
            }
            records.push(json!({"case":name,"textAccepted":false,"paragraphAccepted":false,"discoveredRuns":0,"nativeReason":native_runs.reason}));
            continue;
        };
        let selected = inspected["runs"].as_array().unwrap().iter().find(|run| run["objectPath"] == json!(seed.object_path)).unwrap();
        let native = engine.replace_form_text_occurrence(&info.id, &(0..info.pages.len()).collect::<Vec<_>>(), 0, &seed.object_path, &seed.text, "Edit", None);
        let automated = preview(&source, dir.path(), 1, selected["selectionId"].as_str().unwrap(), "Edit");
        assert_eq!(automated.status.success(), native.is_ok(), "{name}: {}", String::from_utf8_lossy(&automated.stderr));
        let mut saved_path = None;
        if let Ok(changed) = native {
            let bundle = ok(automated);
            let saved = dir.path().join(format!("{name}-saved.pdf"));
            ok(save(&bundle, &saved));
            assert_eq!(fs::read(&saved).unwrap(), fs::read(bundle["candidate"].as_str().unwrap()).unwrap());
            let reopened = engine.open_document(&saved).unwrap();
            for page in 0..info.pages.len() {
                assert_eq!(engine.render_page(&changed.document.id, page, 900).unwrap(), engine.render_page(&reopened.id, page, 900).unwrap(), "native/automation {name}/{page}");
                if page > 0 { assert_eq!(engine.render_page(&info.id, page, 900).unwrap(), engine.render_page(&reopened.id, page, 900).unwrap()); }
            }
            let after = engine.list_occurrence_form_text_runs(&reopened.id, 0).unwrap();
            assert_eq!(after.runs[0].text, "Edit");
            assert_eq!(after.runs[1], native_runs.runs[1]);
            engine.close_document(&reopened.id).unwrap();
            engine.close_document(&changed.document.id).unwrap();
            saved_path = Some(saved);
            successes += 1;
        } else { refused(automated); }
        let native_paragraph = engine.list_form_paragraph_runs(&info.id, 0, &seed.object_path, &seed.text).and_then(|listing| {
            let anchor = listing.runs.iter().find(|r| r.object_index == listing.anchor_index).unwrap();
            engine.suggest_form_styled_paragraph(&info.id, 0, &seed.object_path, anchor.object_index, &anchor.text)
        });
        let paragraph_refusal = native_paragraph.as_ref().err().map(ToString::to_string);
        let automated = cli("inspect-paragraph", &source, &["--page", "1", "--selection-id", selected["selectionId"].as_str().unwrap()], None);
        assert_eq!(automated.status.success(), native_paragraph.is_ok(), "paragraph admission {name}");
        let mut paragraph_saved = None;
        if let Ok(paragraph) = native_paragraph {
            let inspected = ok(automated);
            assert_eq!(serde_json::from_value::<folio_engine::StyledParagraphSelection>(inspected["paragraph"].clone()).unwrap(), paragraph);
            let request = StyledParagraphLayoutRequest {baseline_anchor:paragraph.baseline_anchor,object_indices: paragraph.object_indices, expected_text: paragraph.text, runs: vec![StyledParagraphRun {text:"Edit".into(),style_id:paragraph.styles[0].style_id}], style_changes:vec![], bounds:paragraph.bounds, alignment:paragraph.alignment.unwrap_or(ParagraphAlignment::Left), line_spacing:paragraph.line_spacing.unwrap_or(1.2)};
            let native = engine.preview_form_styled_paragraph_occurrence(&info.id, &(0..info.pages.len()).collect::<Vec<_>>(), 0, &seed.object_path, &request);
            let edit = dir.path().join("paragraph.json");
            fs::write(&edit, serde_json::to_vec(&json!({"page":1,"seedSelectionId":inspected["seedSelectionId"],"paragraphId":inspected["paragraphId"],"runs":request.runs})).unwrap()).unwrap();
            let automated = cli("preview-paragraph", &source, &["--request",edit.to_str().unwrap(),"--out-dir",dir.path().to_str().unwrap()], None);
            assert_eq!(automated.status.success(), native.is_ok(), "paragraph preview {name}: {}", String::from_utf8_lossy(&automated.stderr));
            if let Ok(changed) = native {
                let bundle = ok(automated);
                let saved = dir.path().join(format!("{name}-paragraph.pdf"));
                ok(save(&bundle, &saved));
                let reopened = engine.open_document(&saved).unwrap();
                for page in 0..info.pages.len() {assert!(engine.render_page(&changed.document.id, page, 900).unwrap() == engine.render_page(&reopened.id, page, 900).unwrap(), "native/automation paragraph {name}/{page}");}
                engine.close_document(&reopened.id).unwrap();
                engine.close_document(&changed.document.id).unwrap();
                paragraph_saved = Some(saved);
                paragraph_successes += 1;
            } else { refused(automated); }
        } else { refused(automated); }
        engine.close_document(&info.id).unwrap();
        assert_eq!(fs::read(&source).unwrap(), original);
        if let Some(root) = std::env::var_os("FOLIO_NESTED_AUTOMATION_CORPUS") {
            let root = PathBuf::from(root);
            fs::create_dir_all(&root).unwrap();
            for path in [Some(source.clone()), saved_path.clone(), paragraph_saved.clone()].into_iter().flatten() {
                let target = root.join(path.file_name().unwrap());
                assert!(!target.exists(), "Preserve prior corpus evidence");
                fs::copy(path, target).unwrap();
            }
        }
        records.push(json!({"case":name,"textAccepted":saved_path.is_some(),"paragraphAccepted":paragraph_saved.is_some(),"paragraphRefusal":paragraph_refusal}));
    }
    assert!(successes >= 8, "Require meaningful positive corpus: {records:?}");
    assert_eq!(records.iter().filter(|r| r["paragraphAccepted"] == true).count(), paragraph_successes);
    // Affine promotion can be refused by the native complete-appearance proof.
    // Positive paragraph coverage is required by the separate geometry/growth suites.
    if let Some(root) = std::env::var_os("FOLIO_NESTED_AUTOMATION_CORPUS") {
        let path = PathBuf::from(root).join("native-equivalence.json");
        assert!(!path.exists());
        fs::write(path, serde_json::to_vec_pretty(&records).unwrap()).unwrap();
    }
}

#[test]
fn nested_discovery_and_edits_keep_native_clip_overlap_and_depth_refusals() {
    use lopdf::{dictionary, Document, Object, Stream};
    let dir = tempfile::tempdir().unwrap();
    let engine = PdfEngine::start(renderer()).unwrap();
    for mode in ["clip", "overlap", "deep", "malformed"] {
        let source = dir.path().join(format!("{mode}.pdf"));
        nested::write_fixture(&source, nested::Mode::RepeatedOuter, 1.);
        let mut pdf = Document::load(&source).unwrap();
        let leaf = pdf.objects.iter().find_map(|(id,o)| o.as_stream().ok().filter(|s| s.content.starts_with(b"BT /F1 12 Tf 5 40")).map(|_|*id)).unwrap();
        if mode == "deep" {
            let mut target = leaf;
            for _ in 0..14 {
                target = pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),220.into(),70.into()],"Resources"=>dictionary!{"XObject"=>dictionary!{"Next"=>target}}}, b"/Next Do".to_vec()));
            }
            for o in pdf.objects.values_mut() {
                if let Ok(s) = o.as_stream_mut() {
                    if let Ok(resources) = s.dict.get_mut(b"Resources").and_then(Object::as_dict_mut) {
                        if let Ok(objects) = resources.get_mut(b"XObject").and_then(Object::as_dict_mut) {
                            if objects.has(b"Inner") { objects.set("Inner", target); }
                        }
                    }
                }
            }
        } else {
            let stream = pdf.get_object_mut(leaf).unwrap().as_stream_mut().unwrap();
            if mode == "clip" {
                stream.set_plain_content([b"q 0 0 1 1 re W n ".as_slice(), &stream.content, b" Q"].concat());
            } else if mode == "overlap" {
                stream.set_plain_content([stream.content.as_slice(), b" BT /F1 12 Tf 5 40 Td (Neighbor overlapping) Tj ET"].concat());
            } else {
                stream.dict.set("Matrix", vec![1.into(), 0.into()]);
            }
        }
        pdf.save(&source).unwrap();
        let original = fs::read(&source).unwrap();
        let info = engine.open_document(&source).unwrap();
        let native = engine.list_occurrence_form_text_runs(&info.id,0);
        let response = cli("inspect-nested-text", &source, &["--page","1"], None);
        assert_eq!(response.status.success(), native.is_ok(), "{mode} listing disagreement");
        if let Ok(native) = native {
            let listed = ok(response);
            assert_eq!(listed["totalRuns"], native.runs.len());
            for run in &native.runs {
                let direct = engine.replace_form_text_occurrence(&info.id,&[0,1,2],0,&run.object_path,&run.text,"A replacement that is intentionally much longer than the original overlapping or clipped text",None);
                assert!(direct.is_err(), "{mode} negative must be a native refusal");
                let seed = listed["runs"].as_array().unwrap().iter().find(|r| r["objectPath"] == json!(run.object_path)).unwrap();
                refused(preview(&source,dir.path(),1,seed["selectionId"].as_str().unwrap(),"A replacement that is intentionally much longer than the original overlapping or clipped text"));
            }
        } else { refused(response); }
        assert!(!fs::read_dir(dir.path()).unwrap().any(|p| p.unwrap().path().is_dir()));
        assert_eq!(fs::read(&source).unwrap(),original);
        engine.close_document(&info.id).unwrap();
    }
}
