use folio_engine::PdfEngine;
use lopdf::{dictionary, Document, Object, Stream};
use serde_json::Value;
use std::{
    ffi::OsStr,
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
fn fixture(path: &Path, streams: &[&str], unit: f32, rotate: i64) {
    let mut doc = Document::with_version("1.7");
    let pages = doc.new_object_id();
    let font =
        doc.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let mut kids = Vec::<Object>::new();
    for stream in streams {
        let content = doc.add_object(Stream::new(dictionary! {}, stream.as_bytes().to_vec()));
        let page = doc.add_object(dictionary! {"Type"=>"Page","Parent"=>pages,
        "MediaBox"=>vec![0.into(),0.into(),400.into(),600.into()],"UserUnit"=>unit,"Rotate"=>rotate,
        "Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});
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
fn invoke(path: &Path, args: &[&str], password: Option<&[u8]>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_folio-cli"));
    command
        .arg("inspect-text")
        .arg(path)
        .arg("--engine")
        .arg(renderer())
        .args(args);
    if password.is_some() {
        command.arg("--password-stdin");
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(password) = password {
        child.stdin.as_mut().unwrap().write_all(password).unwrap();
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
    let value: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(value["command"], "inspect-text");
    value["result"].clone()
}
fn fail(out: Output, exit: i32, code: &str) {
    assert_eq!(
        out.status.code(),
        Some(exit),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stdout.is_empty());
    let value: Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(value["error"]["code"], code);
}
const WORDS: &str = "BT /F1 12 Tf 30 500 Td (Same) Tj 0 -30 Td (Same) Tj ET";

#[test]
fn selectors_survive_workers_and_rename_but_bind_source_page_and_run() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.pdf");
    fixture(&path, &[WORDS, WORDS], 1., 0);
    let bytes = fs::read(&path).unwrap();
    let first = ok(invoke(&path, &["--page", "1"], None));
    assert_eq!(first, ok(invoke(&path, &["--page", "1"], None)));
    let renamed = dir.path().join("different name.pdf");
    fs::write(&renamed, &bytes).unwrap();
    assert_eq!(first, ok(invoke(&renamed, &["--page", "1"], None)));
    let a = first["runs"][0]["selectionId"].as_str().unwrap();
    assert!(a.starts_with("text-v1-") && a.len() == 72);
    assert_ne!(
        first["runs"][0]["selectionId"],
        first["runs"][1]["selectionId"]
    );
    let second = ok(invoke(&path, &["--page", "2"], None));
    assert_ne!(
        first["runs"][0]["selectionId"],
        second["runs"][0]["selectionId"]
    );
    let mut changed = bytes.clone();
    changed.extend_from_slice(b"\n% byte change outside text\n");
    fs::write(&renamed, changed).unwrap();
    let changed = ok(invoke(&renamed, &["--page", "1"], None));
    assert_ne!(first["sourceFingerprint"], changed["sourceFingerprint"]);
    assert_ne!(
        first["runs"][0]["selectionId"],
        changed["runs"][0]["selectionId"]
    );
    assert_eq!(fs::read(path).unwrap(), bytes);
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
}

#[test]
fn exact_native_fields_and_physical_geometry_are_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("geometry.pdf");
    let engine = PdfEngine::start(renderer()).unwrap();
    for rotate in [0, 90, 180, 270] {
        fixture(
            &path,
            &["BT /F1 12 Tf 0.001 Tc 60 300 Td (Old) Tj 42.679 0 Td (ds Neighbor) Tj ET"],
            1.7,
            rotate,
        );
        let mut pdf = Document::load(&path).unwrap();
        let page_id = *pdf.get_pages().values().next().unwrap();
        pdf.get_dictionary_mut(page_id).unwrap().set(
            "CropBox",
            vec![
                Object::Integer(40),
                Object::Integer(50),
                Object::Integer(340),
                Object::Integer(550),
            ],
        );
        pdf.save(&path).unwrap();
        let doc = engine.open_document(&path).unwrap();
        let native = engine.list_occurrence_text_runs(&doc.id, 0).unwrap();
        if rotate == 0 {
            assert!(
                native.runs.iter().any(|run| run.source_text.is_some()),
                "fixture must exercise generated reading spaces: {native:?}"
            );
        }
        let result = ok(invoke(&path, &["--page", "1"], None));
        assert_eq!(result["coordinateSpace"], "displayed-page-points");
        assert_eq!(result["width"], serde_json::json!(doc.pages[0].width));
        assert_eq!(result["height"], serde_json::json!(doc.pages[0].height));
        assert_eq!(result["runs"].as_array().unwrap().len(), native.runs.len());
        for (value, run) in result["runs"].as_array().unwrap().iter().zip(native.runs) {
            let mut value = value.clone();
            value.as_object_mut().unwrap().remove("selectionId");
            // Compare the native f32 values, not two different JSON float spellings.
            assert_eq!(
                serde_json::from_value::<folio_engine::EditableTextRun>(value).unwrap(),
                run
            );
        }
        engine.close_document(&doc.id).unwrap();
    }
}

#[test]
fn oversized_run_fails_explicitly_without_partial_text_or_file_outputs() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("large.pdf");
    let stream = format!("BT /F1 0.01 Tf 20 100 Td ({}) Tj ET", "W".repeat(17000));
    fixture(&path, &[&stream], 1., 0);
    let bytes = fs::read(&path).unwrap();
    let out = invoke(&path, &["--page", "1"], None);
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("response byte limit"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    fail(out, 1, "invalid_request");
    assert_eq!(fs::read(path).unwrap(), bytes);
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn inspection_budget_includes_the_returned_selection_id() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("near-limit.pdf");
    let engine = PdfEngine::start(renderer()).unwrap();
    for (character, json_bytes) in [("W", 1), ("\"", 2)] {
        let mut length = 15000 / json_bytes;
        let mut found = false;
        for _ in 0..10 {
            let stream = format!(
                "BT /F1 0.01 Tf 20 100 Td ({}) Tj ET",
                character.repeat(length)
            );
            fixture(&path, &[&stream], 1., 0);
            let source = engine.open_document(&path).unwrap();
            let run = engine
                .list_occurrence_text_runs(&source.id, 0)
                .unwrap()
                .runs
                .remove(0);
            engine.close_document(&source.id).unwrap();
            let size = serde_json::to_vec(&run).unwrap().len();
            if (16330..16384).contains(&size) {
                found = true;
                break;
            }
            length = (length as isize + (16344 - size as isize) / json_bytes as isize) as usize;
        }
        assert!(
            found,
            "construct a run fitting by itself, but not with its selection ID"
        );
        let out = invoke(&path, &["--page", "1"], None);
        assert!(String::from_utf8_lossy(&out.stderr).contains("response byte limit"));
        fail(out, 1, "invalid_request");
    }
}

#[test]
fn pagination_is_complete_without_repeating_or_omitting_selections() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("many.pdf");
    let stream = (0..103)
        .map(|i| {
            format!(
                "BT /F1 8 Tf 1 0 0 1 {} {} Tm (Item {i}) Tj ET\n",
                20 + (i % 4) * 90,
                570 - (i / 4) * 20
            )
        })
        .collect::<String>();
    fixture(&path, &[&stream], 1., 0);
    let first = ok(invoke(&path, &["--page", "1"], None));
    assert_eq!(first["totalRuns"], 103);
    assert_eq!(first["runs"].as_array().unwrap().len(), 100);
    assert_eq!(first["nextOffset"], 100);
    let tail = ok(invoke(&path, &["--page", "1", "--offset", "100"], None));
    assert_eq!(tail["runs"].as_array().unwrap().len(), 3);
    assert!(tail["nextOffset"].is_null());
    let ids = first["runs"]
        .as_array()
        .unwrap()
        .iter()
        .chain(tail["runs"].as_array().unwrap())
        .map(|r| r["selectionId"].as_str().unwrap())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(ids.len(), 103);
    let end = ok(invoke(&path, &["--page", "1", "--offset", "103"], None));
    assert_eq!(end["runs"], serde_json::json!([]));
    assert!(end["nextOffset"].is_null());
    fail(
        invoke(&path, &["--page", "1", "--offset", "104"], None),
        1,
        "invalid_request",
    );
}

#[test]
fn blank_and_unsupported_runs_have_honest_reasons() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("unsupported.pdf");
    fixture(
        &path,
        &["", "BT /F1 12 Tf 1 Tr 30 500 Td (Stroke only) Tj ET"],
        1.,
        0,
    );
    let blank = ok(invoke(&path, &["--page", "1"], None));
    assert_eq!(blank["totalRuns"], 0);
    assert!(blank["reason"].is_string());
    let unsupported = ok(invoke(&path, &["--page", "2"], None));
    assert_eq!(unsupported["runs"][0]["supported"], false);
    assert!(unsupported["runs"][0]["reason"].is_string());
    let mut pdf = Document::load(&path).unwrap();
    pdf.catalog_mut()
        .unwrap()
        .set("OCProperties", dictionary! {});
    pdf.save(&path).unwrap();
    let restricted = ok(invoke(&path, &["--page", "2"], None));
    assert_eq!(restricted["totalRuns"], 0);
    assert!(restricted["reason"]
        .as_str()
        .unwrap()
        .to_lowercase()
        .contains("layer"));
}

#[test]
fn inspection_requires_copy_and_edit_rights_and_supports_owner_unlock() {
    use lopdf::encryption::Permissions;
    let dir = tempfile::tempdir().unwrap();
    let plain = dir.path().join("plain.pdf");
    let locked = dir.path().join("locked.pdf");
    fixture(&plain, &[WORDS], 1., 0);
    let bytes = fs::read(plain).unwrap();
    for rights in [
        Permissions::MODIFIABLE,
        Permissions::COPYABLE,
        Permissions::PRINTABLE,
    ] {
        fs::write(&locked, protected_fixture::encrypt(&bytes, rights)).unwrap();
        fail(
            invoke(&locked, &["--page", "1"], None),
            1,
            "password_required",
        );
        fail(
            invoke(&locked, &["--page", "1"], Some(b"reader\n")),
            1,
            "permission_denied",
        );
        let owned = ok(invoke(&locked, &["--page", "1"], Some(b"owner\n")));
        assert_eq!(owned["totalRuns"], 2);
    }
    fs::write(
        &locked,
        protected_fixture::encrypt(&bytes, Permissions::MODIFIABLE | Permissions::COPYABLE),
    )
    .unwrap();
    let allowed = ok(invoke(&locked, &["--page", "1"], Some(b"reader\n")));
    assert_eq!(allowed["totalRuns"], 2);
    assert_eq!(
        allowed,
        ok(invoke(&locked, &["--page", "1"], Some(b"reader\n")))
    );
}

#[test]
fn strict_inspection_arguments_fail_before_native_loading() {
    for args in [
        vec![],
        vec!["--page", "0"],
        vec!["--page", "10001"],
        vec!["--page", "1", "--offset", "-1"],
        vec!["--page", "1", "--offset", "100001"],
        vec!["--page", "1", "--dpi", "72"],
        vec!["--page", "1", "--out", "no.txt"],
        vec!["--page", "1", "--page", "2"],
    ] {
        fail(
            invoke(Path::new("does-not-exist.pdf"), &args, None),
            2,
            "invalid_arguments",
        );
    }
    let help = Command::new(env!("CARGO_BIN_EXE_folio-cli"))
        .args([OsStr::new("inspect-text"), OsStr::new("--help")])
        .output()
        .unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("--offset"));
}
