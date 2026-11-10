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

#[test]
fn selecting_second_identical_run_preserves_first_run_and_other_page() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("duplicates.pdf");
    fixture(&source);
    let mut doc = Document::load(&source).unwrap();
    let page = doc.get_pages()[&1];
    let content = doc
        .get_dictionary(page)
        .unwrap()
        .get(b"Contents")
        .unwrap()
        .as_reference()
        .unwrap();
    doc.objects.insert(
        content,
        Stream::new(
            dictionary! {},
            b"BT /F1 14 Tf 30 250 Td (Same words) Tj 0 -60 Td (Same words) Tj ET".to_vec(),
        )
        .into(),
    );
    doc.save(&source).unwrap();
    let original = fs::read(&source).unwrap();
    let runs = ok(cli("inspect-text", &source, &["--page", "1"], None));
    let runs = runs["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0]["text"], runs[1]["text"]);
    assert_ne!(runs[0]["selectionId"], runs[1]["selectionId"]);
    let bundle = ok(preview(
        &source,
        dir.path(),
        runs[1]["selectionId"].as_str().unwrap(),
        "Second changed",
        None,
    ));
    let output = dir.path().join("saved.pdf");
    ok(save(&bundle, &output));
    let after = ok(cli("inspect-text", &output, &["--page", "1"], None));
    assert_eq!(after["runs"][0]["text"], "Same words");
    assert_eq!(after["runs"][0]["bounds"], runs[0]["bounds"]);
    assert_eq!(after["runs"][1]["text"], "Second changed");
    assert_eq!(fs::read(&source).unwrap(), original);
    let engine = PdfEngine::start(renderer()).unwrap();
    let a = engine.open_document(&source).unwrap();
    let b = engine.open_document(&output).unwrap();
    assert_eq!(
        engine.render_page(&a.id, 1, 800).unwrap(),
        engine.render_page(&b.id, 1, 800).unwrap()
    );
    engine.close_document(&a.id).unwrap();
    engine.close_document(&b.id).unwrap();
}
fn selection(source: &Path, password: Option<&str>) -> String {
    ok(cli("inspect-text", source, &["--page", "1"], password))["runs"][0]["selectionId"]
        .as_str()
        .unwrap()
        .into()
}
fn preview(source: &Path, dir: &Path, id: &str, text: &str, password: Option<&str>) -> Output {
    let request = dir.join("edit.json");
    fs::write(
        &request,
        serde_json::to_vec(&json!({"page":1,"selectionId":id,"replacement":text})).unwrap(),
    )
    .unwrap();
    cli(
        "preview-text",
        source,
        &[
            "--request",
            request.to_str().unwrap(),
            "--out-dir",
            dir.to_str().unwrap(),
        ],
        password,
    )
}
fn save(bundle: &Value, dest: &Path) -> Output {
    cli(
        "save-text-preview",
        Path::new(bundle["folder"].as_str().unwrap()),
        &[
            "--manifest-sha256",
            bundle["manifestSha256"].as_str().unwrap(),
            "--out",
            dest.to_str().unwrap(),
        ],
        None,
    )
}

#[test]
fn inspect_preview_save_reopen_keeps_original_graph_and_exact_candidate() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pdf");
    fixture(&source);
    let original = fs::read(&source).unwrap();
    let id = selection(&source, None);
    let bundle = ok(preview(&source, dir.path(), &id, "Old this content", None));
    assert_eq!(fs::read(&source).unwrap(), original);
    let candidate = fs::read(bundle["candidate"].as_str().unwrap()).unwrap();
    let before = image::open(bundle["before"].as_str().unwrap())
        .unwrap()
        .to_rgb8();
    let after = image::open(bundle["after"].as_str().unwrap())
        .unwrap()
        .to_rgb8();
    assert_eq!(before.dimensions(), after.dimensions());
    assert_ne!(before, after);
    // Outside a generous selected-line region, the actual saved page image is identical.
    for (x, y, pixel) in before.enumerate_pixels() {
        if y < 390 || y > 480 || x < 60 || x > 650 {
            assert_eq!(pixel, after.get_pixel(x, y));
        }
    }
    let saved = dir.path().join("edited.pdf");
    ok(save(&bundle, &saved));
    assert_eq!(fs::read(&saved).unwrap(), candidate);
    let engine = PdfEngine::start(renderer()).unwrap();
    let old = engine.open_document(&source).unwrap();
    let new = engine.open_document(&saved).unwrap();
    assert!(engine
        .extract_text(&new.id, 0)
        .unwrap()
        .contains("Old this content"));
    assert!(engine
        .extract_text(&new.id, 0)
        .unwrap()
        .contains("Neighbor"));
    let normalize = |bookmarks: &[folio_engine::DocumentBookmark]| {
        let mut value = serde_json::to_value(bookmarks).unwrap();
        for bookmark in value.as_array_mut().unwrap() {
            bookmark.as_object_mut().unwrap().remove("id");
        }
        value
    };
    assert_eq!(normalize(&old.bookmarks), normalize(&new.bookmarks));
    assert_eq!(
        engine.render_page(&old.id, 1, 600).unwrap(),
        engine.render_page(&new.id, 1, 600).unwrap()
    );
    for id in [&old.id, &new.id] {
        engine.close_document(id).unwrap();
    }
    let pdf = Document::load(&saved).unwrap();
    let second = *pdf.get_pages().values().nth(1).unwrap();
    assert_eq!(
        pdf.get_dictionary(second)
            .unwrap()
            .get(b"Annots")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    // Originals are no longer needed to save the exact proved preview a second time.
    fs::rename(&source, dir.path().join("moved-original.pdf")).unwrap();
    let again = dir.path().join("second.pdf");
    ok(save(&bundle, &again));
    assert_eq!(fs::read(again).unwrap(), candidate);
}

#[test]
fn stale_unsupported_and_overlapping_edits_leave_no_preview_bundle() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pdf");
    fixture(&source);
    let id = selection(&source, None);
    let original = fs::read(&source).unwrap();
    for replacement in [
        "",
        "not a line\nsecond",
        "💥",
        &"W".repeat(4097),
        &"W".repeat(120),
    ] {
        refused(preview(&source, dir.path(), &id, replacement, None));
    }
    let mut changed = original.clone();
    changed.extend_from_slice(b"\n% changed bytes\n");
    fs::write(&source, &changed).unwrap();
    refused(preview(&source, dir.path(), &id, "New content", None));
    assert_eq!(fs::read(&source).unwrap(), changed);
    assert!(!fs::read_dir(dir.path())
        .unwrap()
        .any(|e| e.unwrap().file_type().unwrap().is_dir()));
}

#[test]
fn save_refuses_tampered_bundles_and_existing_destination_aliases() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pdf");
    fixture(&source);
    let original = fs::read(&source).unwrap();
    let bundle = ok(preview(
        &source,
        dir.path(),
        &selection(&source, None),
        "New content",
        None,
    ));
    let folder = Path::new(bundle["folder"].as_str().unwrap());
    let dest = dir.path().join("saved.pdf");
    for name in ["manifest.json", "candidate.pdf", "before.png", "after.png"] {
        let path = folder.join(name);
        let bytes = fs::read(&path).unwrap();
        fs::write(&path, b"tampered").unwrap();
        refused(save(&bundle, &dest));
        assert!(!dest.exists());
        fs::write(path, bytes).unwrap();
    }
    fs::write(&dest, b"keep destination").unwrap();
    refused(save(&bundle, &dest));
    assert_eq!(fs::read(&dest).unwrap(), b"keep destination");
    let alias = dir.path().join("alias.pdf");
    fs::hard_link(&source, &alias).unwrap();
    refused(save(&bundle, &alias));
    assert_eq!(fs::read(source).unwrap(), original);
}

#[test]
fn protected_preview_and_saved_copy_preserve_credentials_and_permissions() {
    use lopdf::encryption::Permissions;
    let dir = tempfile::tempdir().unwrap();
    let plain = dir.path().join("plain.pdf");
    fixture(&plain);
    let source = dir.path().join("locked.pdf");
    fs::write(
        &source,
        protected_fixture::encrypt(
            &fs::read(plain).unwrap(),
            Permissions::MODIFIABLE | Permissions::COPYABLE,
        ),
    )
    .unwrap();
    let original = fs::read(&source).unwrap();
    let id = selection(&source, Some("reader\n"));
    let bundle = ok(preview(
        &source,
        dir.path(),
        &id,
        "New protected content",
        Some("reader\n"),
    ));
    let dest = dir.path().join("saved.pdf");
    ok(save(&bundle, &dest));
    let mut raw = Document::load(&dest).unwrap();
    assert!(raw.is_encrypted());
    assert!(raw.decrypt("reader").is_ok());
    let engine = PdfEngine::start(renderer()).unwrap();
    let old = engine
        .open_document_with_password(&source, "reader")
        .unwrap();
    let new = engine.open_document_with_password(&dest, "reader").unwrap();
    assert_eq!(
        old.protection.as_ref().unwrap().permissions,
        new.protection.as_ref().unwrap().permissions
    );
    assert!(engine
        .extract_text(&new.id, 0)
        .unwrap()
        .contains("New protected content"));
    for id in [&old.id, &new.id] {
        engine.close_document(id).unwrap();
    }
    assert_eq!(fs::read(source).unwrap(), original);
}

#[test]
fn request_schema_and_limits_are_strict_and_errors_do_not_echo_contents() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pdf");
    fixture(&source);
    let request = dir.path().join("edit.json");
    for text in ["private replacement broken json".into()," ".repeat(32769),json!({"page":1,"selectionId":selection(&source,None),"replacement":"private replacement","input":"other.pdf"}).to_string()] {
        fs::write(&request,text).unwrap();let out=cli("preview-text",&source,&["--request",request.to_str().unwrap(),"--out-dir",dir.path().to_str().unwrap()],None);
        assert!(!String::from_utf8_lossy(&out.stderr).contains("private replacement"));refused(out);
    }
    let help = Command::new(env!("CARGO_BIN_EXE_folio-cli"))
        .args(["preview-text", "--help"])
        .output()
        .unwrap();
    assert!(help.status.success());
}

#[test]
fn preview_enforces_user_edit_and_copy_permissions_before_resolving_selection() {
    use lopdf::encryption::Permissions;
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("restricted.pdf");
    let plain = dir.path().join("plain.pdf");
    fixture(&plain);
    for permissions in [Permissions::COPYABLE, Permissions::MODIFIABLE] {
        fs::write(
            &source,
            protected_fixture::encrypt(&fs::read(&plain).unwrap(), permissions),
        )
        .unwrap();
        let original = fs::read(&source).unwrap();
        let out = preview(
            &source,
            dir.path(),
            &format!("text-v1-{}", "a".repeat(64)),
            "New content",
            Some("reader\n"),
        );
        assert!(!out.status.success());
        let response: Value = serde_json::from_slice(&out.stderr).unwrap();
        assert_eq!(response["error"]["code"], "permission_denied", "{response}");
        assert_eq!(fs::read(&source).unwrap(), original);
    }
    assert!(!fs::read_dir(dir.path()).unwrap().any(|entry| entry
        .unwrap()
        .file_type()
        .unwrap()
        .is_dir()));
}

#[cfg(windows)]
#[test]
fn preview_and_save_preserve_non_unicode_native_parent_paths() {
    use std::{
        ffi::OsString,
        os::windows::ffi::{OsStrExt, OsStringExt},
    };
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pdf");
    fixture(&source);
    let parent = dir.path().join(OsString::from_wide(&[b'p' as u16, 0xd800]));
    fs::create_dir(&parent).unwrap();
    let request = dir.path().join("edit.json");
    fs::write(
        &request,
        json!({"page":1,"selectionId":selection(&source,None),"replacement":"New content"})
            .to_string(),
    )
    .unwrap();
    let preview = ok(Command::new(env!("CARGO_BIN_EXE_folio-cli"))
        .arg("preview-text")
        .arg(&source)
        .arg("--request")
        .arg(&request)
        .arg("--out-dir")
        .arg(&parent)
        .arg("--engine")
        .arg(renderer())
        .output()
        .unwrap());
    assert!(preview["folder"].is_null());
    let units = preview["folderNative"]["units"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u16)
        .collect::<Vec<_>>();
    let bundle = PathBuf::from(OsString::from_wide(&units));
    assert!(bundle.starts_with(parent.canonicalize().unwrap()));
    let dest = parent.join("saved.pdf");
    let saved = ok(Command::new(env!("CARGO_BIN_EXE_folio-cli"))
        .arg("save-text-preview")
        .arg(&bundle)
        .arg("--manifest-sha256")
        .arg(preview["manifestSha256"].as_str().unwrap())
        .arg("--out")
        .arg(&dest)
        .arg("--engine")
        .arg(renderer())
        .output()
        .unwrap());
    assert!(saved["path"].is_null());
    let saved_units = saved["pathNative"]["units"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u16)
        .collect::<Vec<_>>();
    assert_eq!(
        saved_units,
        dest.canonicalize()
            .unwrap()
            .as_os_str()
            .encode_wide()
            .collect::<Vec<_>>()
    );
    assert_eq!(
        fs::read(dest).unwrap(),
        fs::read(bundle.join("candidate.pdf")).unwrap()
    );
}


#[test]
fn unchanged_text_preview_retains_exact_source_bytes_and_selection_identity() {
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
        let id = selection(&source, password);
        let bundle = ok(preview(&source, dir.path(), &id, "Old content", password));
        let saved = dir.path().join("saved.pdf");
        ok(save(&bundle, &saved));
        assert_eq!(fs::read(&saved).unwrap(), original, "No-op must retain original ciphertext as well as plain PDF bytes");
        assert_eq!(selection(&saved, password), id);
        assert_eq!(fs::read(bundle["before"].as_str().unwrap()).unwrap(), fs::read(bundle["after"].as_str().unwrap()).unwrap());
        let changed = ok(preview(&saved, dir.path(), &id, "New content", password));
        assert_ne!(fs::read(changed["candidate"].as_str().unwrap()).unwrap(), original);
        assert_eq!(fs::read(&source).unwrap(), original);
    }
}

#[test]
fn unchanged_text_does_not_bypass_reader_permissions_with_an_owner_selector() {
    use lopdf::encryption::Permissions;
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("restricted.pdf");
    fixture(&source);
    let bytes = protected_fixture::encrypt(&fs::read(&source).unwrap(), Permissions::COPYABLE);
    fs::write(&source, &bytes).unwrap();
    let id = selection(&source, Some("owner\n"));
    let denied = preview(&source, dir.path(), &id, "Old content", Some("reader\n"));
    assert!(!denied.status.success());
    assert_eq!(serde_json::from_slice::<Value>(&denied.stderr).unwrap()["error"]["code"], "permission_denied");
    assert_eq!(fs::read(source).unwrap(), bytes);
}

#[path = "support/nested_automation.rs"]
mod nested_automation;

#[test]
fn explicit_single_run_font_cli_is_authorized_self_contained_and_not_a_text_noop() {
    for nested in [false,true] {
        let dir=tempfile::tempdir().unwrap();let source=dir.path().join("source.pdf");
        if nested{nested_automation::write_fixture(&source,nested_automation::Mode::RepeatedInnerFar,1.);}else{fixture(&source);}
        let original=fs::read(&source).unwrap();let font=dir.path().join("font.ttf");
        fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf"),&font).unwrap();
        let font_bytes=fs::read(&font).unwrap();
        let catalog=ok(Command::new(env!("CARGO_BIN_EXE_folio-cli")).args(["fonts","--font"]).arg(&font).output().unwrap());
        let choice=&catalog["fonts"][0];let route=if nested{"inspect-nested-text"}else{"inspect-text"};
        let listed=ok(cli(route,&source,&["--page","1"],None));
        let selected=if nested{listed["runs"].as_array().unwrap().iter().find(|r|r["bounds"]["x"].as_f64().unwrap()>300. && r["text"]=="Nested first line").unwrap()}else{&listed["runs"][0]};
        let index=listed["runs"].as_array().unwrap().iter().position(|r|r["selectionId"]==selected["selectionId"]).unwrap();
        let request=dir.path().join("font-edit.json");
        let edit=json!({"page":1,"selectionId":selected["selectionId"],"replacement":selected["text"],"fontId":choice["fontId"]});
        fs::write(&request,serde_json::to_vec(&edit).unwrap()).unwrap();
        let args=["--request",request.to_str().unwrap(),"--out-dir",dir.path().to_str().unwrap(),"--font",font.to_str().unwrap()];
        let bundle=ok(cli("preview-text",&source,&args,None));
        assert_eq!(bundle["substituteFont"],*choice);
        let manifest:Value=serde_json::from_slice(&fs::read(Path::new(bundle["folder"].as_str().unwrap()).join("manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest["substituteFont"],*choice);assert_eq!(manifest["version"],if nested{4}else{1});
        assert_ne!(fs::read(bundle["candidate"].as_str().unwrap()).unwrap(),original,"explicit font must bypass byte-noop shortcut");
        let mut no_override=edit.clone();no_override.as_object_mut().unwrap().remove("fontId");
        fs::write(&request,serde_json::to_vec(&no_override).unwrap()).unwrap();
        let noop=ok(cli("preview-text",&source,&args,None));
        assert!(noop.get("substituteFont").is_none());
        assert_eq!(fs::read(noop["candidate"].as_str().unwrap()).unwrap(),original,"merely authorizing a font must not change the source");
        let count=||fs::read_dir(dir.path()).unwrap().filter_map(Result::ok).filter(|e|e.file_name().to_string_lossy().starts_with("Folio preview-")).count();
        let before=count();
        for invalid in [json!("a".repeat(64)),Value::Null,json!("font.ttf")] {
            let mut bad=edit.clone();bad["fontId"]=invalid;fs::write(&request,serde_json::to_vec(&bad).unwrap()).unwrap();
            refused(cli("preview-text",&source,&args,None));assert_eq!(count(),before);
        }
        fs::write(&request,serde_json::to_vec(&edit).unwrap()).unwrap();
        refused(cli("preview-text",&source,&args[..4],None));assert_eq!(count(),before);
        fs::remove_file(&font).unwrap();let saved=dir.path().join("saved.pdf");ok(save(&bundle,&saved));
        let pdf=Document::load(&saved).unwrap();assert!(pdf.objects.values().any(|v|v.as_stream().is_ok_and(|s|s.decompressed_content().unwrap_or_else(|_|s.content.clone())==font_bytes)));
        let next=ok(cli(route,&saved,&["--page","1"],None));
        let next_run=&next["runs"][index];
        assert_eq!(next_run["text"],selected["text"]);assert_eq!(next_run["fontDisplayName"],"DejaVu Serif");assert_ne!(next_run["fontName"],selected["fontName"]);
        for (i,run) in listed["runs"].as_array().unwrap().iter().enumerate(){if i!=index{assert_eq!(next["runs"][i]["text"],run["text"]);assert_eq!(next["runs"][i]["bounds"],run["bounds"]);}}
        let revised=ok(preview(&saved,dir.path(),next_run["selectionId"].as_str().unwrap(),"Changed text",None));
        assert!(revised.get("substituteFont").is_none());let final_path=dir.path().join("reedited.pdf");ok(save(&revised,&final_path));
        assert_eq!(ok(cli(route,&final_path,&["--page","1"],None))["runs"][index]["text"],"Changed text");
        assert_eq!(fs::read(&source).unwrap(),original);
    }
}
