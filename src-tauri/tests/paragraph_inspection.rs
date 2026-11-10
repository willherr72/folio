use folio_engine::PdfEngine;
use lopdf::{dictionary, Document, Object, Stream};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

#[path = "support/protected_fixture.rs"]
mod protected_fixture;

fn renderer() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")
}
fn fixture(path: &Path, streams: &[&str], unit: f32, rotate: i64) {
    let mut doc = Document::with_version("1.7");
    let pages = doc.new_object_id();
    let font =
        doc.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
    let bold = doc
        .add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica-Bold"});
    let mut kids = Vec::<Object>::new();
    for stream in streams {
        let content = doc.add_object(Stream::new(dictionary! {}, stream.as_bytes().to_vec()));
        let page = doc.add_object(dictionary! {"Type"=>"Page","Parent"=>pages,
        "MediaBox"=>vec![0.into(),0.into(),400.into(),600.into()],"UserUnit"=>unit,"Rotate"=>rotate,
        "Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font,"F2"=>bold}},"Contents"=>content});
        kids.push(page.into());
    }
    doc.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Count"=>kids.len() as i64,"Kids"=>kids}.into(),
    );
    let root = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    doc.trailer.set("Root", root);
    doc.save(path).unwrap();
}

fn invoke(path: &Path, verb: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_folio-cli"))
        .arg(verb)
        .arg(path)
        .arg("--engine")
        .arg(renderer())
        .args(args)
        .output()
        .unwrap()
}
fn ok(out: Output) -> Value {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stderr.is_empty());
    serde_json::from_slice::<Value>(&out.stdout).unwrap()["result"].clone()
}
fn seed(path: &Path, page: &str) -> String {
    ok(invoke(path, "inspect-text", &["--page", page]))["runs"][0]["selectionId"]
        .as_str()
        .unwrap()
        .into()
}
fn paragraph(path: &Path, page: &str, id: &str) -> Value {
    ok(invoke(
        path,
        "inspect-paragraph",
        &["--page", page, "--selection-id", id],
    ))
}
const WORDS: &str =
    "BT /F1 12 Tf 30 500 Td (First paragraph line) Tj 0 -16 Td (Second paragraph line) Tj ET";
#[test]
fn paragraph_is_stable_source_bound_and_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.pdf");
    fixture(&path, &[WORDS, WORDS], 1., 0);
    let bytes = fs::read(&path).unwrap();
    let id = seed(&path, "1");
    let first = paragraph(&path, "1", &id);
    assert_eq!(first["seedSelectionId"], id);
    assert!(first["paragraphId"]
        .as_str()
        .unwrap()
        .starts_with("paragraph-v1-"));
    assert_eq!(
        first["paragraph"]["text"],
        "First paragraph line Second paragraph line"
    );
    assert_eq!(first, paragraph(&path, "1", &id));
    let copy = dir.path().join("renamed.pdf");
    fs::write(&copy, &bytes).unwrap();
    assert_eq!(first, paragraph(&copy, "1", &id));
    let other = seed(&path, "2");
    let second = paragraph(&path, "2", &other);
    assert_ne!(first["paragraphId"], second["paragraphId"]);
    let listed = ok(invoke(&path, "inspect-text", &["--page", "1"]));
    let other_seed = listed["runs"][1]["selectionId"].as_str().unwrap();
    assert_eq!(
        first["paragraphId"],
        paragraph(&path, "1", other_seed)["paragraphId"]
    );
    assert!(!invoke(
        &path,
        "inspect-paragraph",
        &["--page", "2", "--selection-id", &id]
    )
    .status
    .success());
    let mut changed = bytes.clone();
    changed.extend_from_slice(b"\n% new revision\n");
    fs::write(&copy, changed).unwrap();
    assert!(!invoke(
        &copy,
        "inspect-paragraph",
        &["--page", "1", "--selection-id", &id]
    )
    .status
    .success());
    assert_eq!(fs::read(path).unwrap(), bytes);
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
}
#[test]
fn mixed_styles_and_physical_geometry_match_desktop_native_inspection() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mixed.pdf");
    fixture(&path,&["q 1 1 0 rg 70 496 21 16 re f Q BT /F1 12 Tf 50 500 Td (AA ) Tj ET BT /F2 12 Tf 72 500 Td (BB) Tj ET BT /F1 12 Tf 90 500 Td ( CC) Tj ET BT /F1 12 Tf 50 100 Td (Neighbor) Tj ET"],1.7,0);
    let engine = PdfEngine::start(renderer()).unwrap();
    let doc = engine.open_document(&path).unwrap();
    let run = engine
        .list_occurrence_text_runs(&doc.id, 0)
        .unwrap()
        .runs
        .remove(0);
    let native = engine
        .suggest_occurrence_styled_paragraph(&doc.id, 0, run.object_index, &run.text)
        .unwrap();
    assert!(native.styles.len() > 1);
    assert!(native.runs.len() > 1);
    assert!(native.styles.iter().any(|s| s.background_color.is_some()));
    let got = paragraph(&path, "1", &seed(&path, "1"));
    assert_eq!(got["coordinateSpace"], "displayed-page-points");
    assert_eq!(
        serde_json::from_value::<folio_engine::StyledParagraphSelection>(got["paragraph"].clone())
            .unwrap(),
        native
    );
    assert_eq!(got["width"], serde_json::json!(doc.pages[0].width));
    engine.close_document(&doc.id).unwrap();
}
#[test]
fn paragraph_arguments_are_strict_and_unsupported_content_is_not_promised() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.pdf");
    fixture(&path, &[WORDS], 1., 0);
    let id = seed(&path, "1");
    for args in [
        vec![],
        vec!["--page", "1"],
        vec!["--selection-id", &id],
        vec!["--page", "0", "--selection-id", &id],
        vec!["--page", "1", "--selection-id", "text-v1-bad"],
        vec!["--page", "1", "--selection-id", &id, "--offset", "0"],
        vec!["--page", "1", "--selection-id", &id, "--selection-id", &id],
    ] {
        let out = invoke(&path, "inspect-paragraph", &args);
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
    }
    fixture(
        &path,
        &["BT /F1 12 Tf 1 Tr 30 500 Td (Stroke only) Tj ET"],
        1.,
        0,
    );
    let id = seed(&path, "1");
    let out = invoke(
        &path,
        "inspect-paragraph",
        &["--page", "1", "--selection-id", &id],
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
}

#[test]
fn paragraph_permission_checks_precede_selection_resolution() {
    use lopdf::encryption::Permissions;
    use std::io::Write;
    use std::process::Stdio;
    let dir = tempfile::tempdir().unwrap();
    let plain = dir.path().join("plain.pdf");
    let locked = dir.path().join("locked.pdf");
    fixture(&plain, &[WORDS], 1., 0);
    let bytes = fs::read(&plain).unwrap();
    for rights in [
        Permissions::MODIFIABLE,
        Permissions::COPYABLE,
        Permissions::PRINTABLE,
    ] {
        fs::write(&locked, protected_fixture::encrypt(&bytes, rights)).unwrap();
        let original = fs::read(&locked).unwrap();
        for (verb, args) in [
            ("inspect-text", vec!["--page", "1"]),
            (
                "inspect-paragraph",
                vec![
                    "--page",
                    "1",
                    "--selection-id",
                    "text-v1-0000000000000000000000000000000000000000000000000000000000000000",
                ],
            ),
        ] {
            let mut child = Command::new(env!("CARGO_BIN_EXE_folio-cli"))
                .arg(verb)
                .arg(&locked)
                .args(args)
                .arg("--engine")
                .arg(renderer())
                .arg("--password-stdin")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child.stdin.take().unwrap().write_all(b"reader\n").unwrap();
            let out = child.wait_with_output().unwrap();
            assert_eq!(out.status.code(), Some(1));
            assert!(out.stdout.is_empty());
            assert_eq!(
                serde_json::from_slice::<Value>(&out.stderr).unwrap()["error"]["code"],
                "permission_denied"
            );
        }
        assert_eq!(fs::read(&locked).unwrap(), original);
    }
}

#[test]
fn paragraph_inspection_supports_owner_password_over_stdin() {
    use lopdf::encryption::Permissions;
    use std::{io::Write, process::Stdio};
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pdf");
    fixture(&source, &[WORDS], 1., 0);
    let locked = dir.path().join("locked.pdf");
    fs::write(
        &locked,
        protected_fixture::encrypt(&fs::read(&source).unwrap(), Permissions::PRINTABLE),
    )
    .unwrap();
    let invoke = |verb: &str, extra: &[&str]| {
        let mut child = Command::new(env!("CARGO_BIN_EXE_folio-cli"))
            .arg(verb)
            .arg(&locked)
            .args(["--page", "1", "--password-stdin", "--engine"])
            .arg(renderer())
            .args(extra)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(b"owner\n").unwrap();
        ok(child.wait_with_output().unwrap())
    };
    let listed = invoke("inspect-text", &[]);
    let id = listed["runs"][0]["selectionId"].as_str().unwrap();
    let paragraph = invoke("inspect-paragraph", &["--selection-id", id]);
    assert_eq!(paragraph["sourceFingerprint"], listed["sourceFingerprint"]);
    assert_eq!(
        paragraph["paragraph"]["text"],
        "First paragraph line Second paragraph line"
    );
}
