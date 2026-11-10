use lopdf::{dictionary, Document, Object, Stream};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn font() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")
}
fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_folio-cli"))
        .args(args)
        .output()
        .unwrap()
}
fn native(command: &str, source: &Path, args: &[&str]) -> Output {
    let engine = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll");
    let mut a = vec![
        command,
        source.to_str().unwrap(),
        "--engine",
        engine.to_str().unwrap(),
    ];
    a.extend(args);
    cli(&a)
}
fn ok(out: Output) -> Value {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stderr.is_empty());
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["ok"], true);
    value["result"].clone()
}
fn refused(out: Output) {
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    let value: Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(value["ok"], false);
}
fn fixture(path: &Path) {
    let mut d = Document::with_version("1.7");
    let pages = d.new_object_id();
    let f = d.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let mut kids = Vec::<Object>::new();
    for text in ["Old content", "Other page"] {
        let content = d.add_object(Stream::new(
            dictionary! {},
            format!("BT /F1 14 Tf 30 250 Td ({text}) Tj 0 -70 Td (Neighbor) Tj ET").into_bytes(),
        ));
        let p=d.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),400.into(),400.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>f}},"Contents"=>content});
        kids.push(p.into());
    }
    d.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Kids"=>kids,"Count"=>2}.into(),
    );
    let root = d.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    d.trailer.set("Root", root);
    d.save(path).unwrap();
}
fn inspect(path: &Path) -> Value {
    let text = ok(native("inspect-text", path, &["--page", "1"]));
    ok(native(
        "inspect-paragraph",
        path,
        &[
            "--page",
            "1",
            "--selection-id",
            text["runs"][0]["selectionId"].as_str().unwrap(),
        ],
    ))
}
#[test]
fn fonts_catalog_uses_exact_bytes_without_a_pdf_engine() {
    let f = font();
    let bytes = fs::read(&f).unwrap();
    let result = ok(cli(&[
        "fonts",
        "--font",
        f.to_str().unwrap(),
        "--font",
        f.to_str().unwrap(),
    ]));
    assert_eq!(result["fonts"].as_array().unwrap().len(), 1);
    let entry = &result["fonts"][0];
    assert_eq!(entry["fontId"], hash(&bytes));
    assert_eq!(entry["bytes"], bytes.len());
    assert!(entry["name"].as_str().unwrap().contains("DejaVu"));
    assert_eq!(
        entry.as_object().unwrap().len(),
        5,
        "No paths/raw bytes/coverage in compact catalog"
    );
}
#[test]
fn explicit_paragraph_font_changes_unchanged_text_and_survives_save_without_font_file() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pdf");
    fixture(&source);
    let original = fs::read(&source).unwrap();
    let selected = inspect(&source);
    let f = dir.path().join("selected.ttf");
    fs::copy(font(), &f).unwrap();
    let font_bytes = fs::read(&f).unwrap();
    let id = hash(&font_bytes);
    let style = selected["paragraph"]["styles"][0]["styleId"].clone();
    let mut edit = json!({"page":1,"seedSelectionId":selected["seedSelectionId"],"paragraphId":selected["paragraphId"],"runs":selected["paragraph"]["runs"],"fontOverrides":[{"styleId":style,"fontId":id}],"fit":true});
    let request = dir.path().join("edit.json");
    fs::write(&request, serde_json::to_vec(&edit).unwrap()).unwrap();
    let preview = ok(native(
        "preview-paragraph",
        &source,
        &[
            "--request",
            request.to_str().unwrap(),
            "--out-dir",
            dir.path().to_str().unwrap(),
            "--font",
            f.to_str().unwrap(),
        ],
    ));
    assert_eq!(preview["paragraph"]["baselineAnchor"], selected["paragraph"]["baselineAnchor"]);
    assert_eq!(preview["paragraph"]["fontOverrides"], edit["fontOverrides"]);
    assert_eq!(preview["paragraph"]["substituteFonts"][0]["fontId"], id);
    assert_ne!(preview["candidateSha256"], hash(&original));
    // A same-text explicit font request without Fit must still publish the font.
    edit["fit"] = json!(false);
    edit["bounds"] = preview["paragraph"]["bounds"].clone();
    fs::write(&request, serde_json::to_vec(&edit).unwrap()).unwrap();
    let fixed = ok(native(
        "preview-paragraph",
        &source,
        &[
            "--request",
            request.to_str().unwrap(),
            "--out-dir",
            dir.path().to_str().unwrap(),
            "--font",
            f.to_str().unwrap(),
        ],
    ));
    assert_ne!(fixed["candidateSha256"], hash(&original));
    // Area-only callers retain legacy placement. Callers requiring exact fitted
    // placement can echo the independent baseline as well as the rectangle.
    edit["baselineAnchor"] = preview["paragraph"]["baselineAnchor"].clone();
    fs::write(&request, serde_json::to_vec(&edit).unwrap()).unwrap();
    let anchored = ok(native("preview-paragraph", &source, &[
        "--request", request.to_str().unwrap(), "--out-dir", dir.path().to_str().unwrap(),
        "--font", f.to_str().unwrap(),
    ]));
    assert_eq!(image::open(preview["after"].as_str().unwrap()).unwrap().to_rgba8(),
        image::open(anchored["after"].as_str().unwrap()).unwrap().to_rgba8());
    fs::remove_file(&f).unwrap();
    let saved = dir.path().join("saved.pdf");
    ok(native(
        "save-text-preview",
        Path::new(fixed["folder"].as_str().unwrap()),
        &[
            "--manifest-sha256",
            fixed["manifestSha256"].as_str().unwrap(),
            "--out",
            saved.to_str().unwrap(),
        ],
    ));
    let after = inspect(&saved);
    assert_eq!(after["paragraph"]["text"], selected["paragraph"]["text"]);
    let text = ok(native("inspect-text", &saved, &["--page", "1"]));
    assert_eq!(text["runs"][0]["fontName"], format!("Folio{id}Paragraph"));
    assert!(text["runs"][0]["fontDisplayName"]
        .as_str()
        .unwrap()
        .contains("DejaVu"));
    let pdf = Document::load(&saved).unwrap();
    assert!(
        pdf.objects
            .values()
            .any(|object| object.as_stream().is_ok_and(|stream| stream
                .decompressed_content()
                .unwrap_or_else(|_| stream.content.clone())
                == font_bytes)),
        "The exact explicitly authorized font program must be embedded"
    );
    assert_eq!(fs::read(&source).unwrap(), original);
    let other_before = ok(native("inspect-text", &source, &["--page", "2"]));
    let other_after = ok(native("inspect-text", &saved, &["--page", "2"]));
    for (a, b) in other_before["runs"]
        .as_array()
        .unwrap()
        .iter()
        .zip(other_after["runs"].as_array().unwrap())
    {
        assert_eq!(a["text"], b["text"]);
        assert_eq!(a["bounds"], b["bounds"]);
        assert_eq!(a["fontName"], b["fontName"]);
    }
}
#[test]
fn fonts_reject_bad_inputs_and_options_without_creating_outputs() {
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad.ttf");
    fs::write(&bad, b"invalid").unwrap();
    let f = font();
    for args in [
        vec!["fonts"],
        vec!["fonts", "--font", bad.to_str().unwrap()],
        vec!["fonts", "--font", dir.path().to_str().unwrap()],
        vec![
            "fonts",
            "--font",
            f.to_str().unwrap(),
            "--out",
            "unrequested",
        ],
    ] {
        refused(cli(&args));
    }
}

#[test]
fn paragraph_fonts_refuse_unknown_styles_ids_and_registry_only_authority() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pdf");
    fixture(&source);
    let original = fs::read(&source).unwrap();
    let selected = inspect(&source);
    let f = font();
    let bytes = fs::read(&f).unwrap();
    let id = hash(&bytes);
    let change = json!({"styleId":selected["paragraph"]["styles"][0]["styleId"],"fontId":id});
    let edit = json!({"page":1,"seedSelectionId":selected["seedSelectionId"],"paragraphId":selected["paragraphId"],"runs":selected["paragraph"]["runs"],"fontOverrides":[change],"fit":true});
    let file = dir.path().join("edit.json");
    let alternatives = vec![
        Value::Null,
        json!([change.clone(), change.clone()]),
        json!([{"styleId":63,"fontId":id}]),
        json!([{"styleId":0,"fontId":"a".repeat(64)}]),
        json!([{"styleId":0,"fontId":id,"path":"unrequested.ttf"}]),
    ];
    for overrides in alternatives {
        let mut bad = edit.clone();
        bad["fontOverrides"] = overrides;
        fs::write(&file, serde_json::to_vec(&bad).unwrap()).unwrap();
        refused(native(
            "preview-paragraph",
            &source,
            &[
                "--request",
                file.to_str().unwrap(),
                "--out-dir",
                dir.path().to_str().unwrap(),
                "--font",
                f.to_str().unwrap(),
            ],
        ));
    }
    fs::write(&file, serde_json::to_vec(&edit).unwrap()).unwrap();
    refused(native(
        "preview-paragraph",
        &source,
        &[
            "--request",
            file.to_str().unwrap(),
            "--out-dir",
            dir.path().to_str().unwrap(),
        ],
    ));
    let engine = folio_engine::PdfEngine::start(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )
    .unwrap();
    engine.fonts().register(bytes).unwrap();
    let result = folio_engine::automation::execute(
        &engine,
        folio_engine::automation::Request::PreviewParagraph {
            input: source.clone(),
            request_file: file,
            parent: dir.path().to_owned(),
        },
        None,
    );
    assert!(result.unwrap_err().to_string().contains("not authorized"));
    assert!(
        engine.fonts().get(&id).is_ok(),
        "Unauthorized request must not remove somebody else's registered font"
    );
    assert_eq!(fs::read(&source).unwrap(), original);
    assert!(
        fs::read_dir(dir.path()).unwrap().all(|entry| !entry
            .unwrap()
            .file_type()
            .unwrap()
            .is_dir()),
        "Refusals publish no bundles"
    );
}
