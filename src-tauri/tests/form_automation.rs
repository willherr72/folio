use folio_engine::PdfEngine;
use lopdf::Document;
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
#[path = "support/automation_forms.rs"]
mod forms;
fn inspect(path: &Path, page: &str, password: Option<&str>) -> Value {
    ok(cli("inspect-forms", path, &["--page", page], password))
}
fn preview(
    path: &Path,
    parent: &Path,
    page: u32,
    field: &Value,
    value: Value,
    password: Option<&str>,
) -> Output {
    let request = parent.join("edit.json");
    fs::write(
        &request,
        serde_json::to_vec(&json!({"page":page,"fieldId":field["fieldId"],"value":value})).unwrap(),
    )
    .unwrap();
    cli(
        "preview-form",
        path,
        &[
            "--request",
            request.to_str().unwrap(),
            "--out-dir",
            parent.to_str().unwrap(),
        ],
        password,
    )
}
fn save(bundle: &Value, out: &Path) -> Output {
    cli(
        "save-text-preview",
        Path::new(bundle["folder"].as_str().unwrap()),
        &[
            "--manifest-sha256",
            bundle["manifestSha256"].as_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ],
        None,
    )
}
#[test]
fn inspect_fill_save_reedit_preserves_shared_fields_and_source() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("form.pdf");
    let bytes = forms::fixture();
    fs::write(&input, &bytes).unwrap();
    let selected = inspect(&input, "1", None);
    let renamed = dir.path().join("renamed.pdf");
    fs::write(&renamed, &bytes).unwrap();
    assert_eq!(inspect(&renamed, "1", None), selected);
    let text = &selected["fields"][0];
    assert_eq!(text["name"], "person.name");
    assert!(text["fieldId"].as_str().unwrap().starts_with("field-v1-"));
    let bundle = ok(preview(
        &input,
        dir.path(),
        1,
        text,
        json!({"kind":"text","text":"Updated answer"}),
        None,
    ));
    let saved = dir.path().join("filled.pdf");
    ok(save(&bundle, &saved));
    assert_eq!(
        fs::read(&saved).unwrap(),
        fs::read(bundle["candidate"].as_str().unwrap()).unwrap()
    );
    let next = inspect(&saved, "1", None);
    assert_eq!(next["fields"][0]["value"]["text"], "Updated answer");
    assert_eq!(
        inspect(&saved, "2", None)["fields"][0]["value"]["text"],
        "Updated answer"
    );
    assert_eq!(next["fields"][1]["value"], selected["fields"][1]["value"]);
    assert_ne!(next["fields"][0]["fieldId"], text["fieldId"]);
    refused(preview(
        &saved,
        dir.path(),
        1,
        text,
        json!({"kind":"text","text":"Stale"}),
        None,
    ));
    let cleared = ok(preview(
        &saved,
        dir.path(),
        1,
        &next["fields"][0],
        json!({"kind":"text","text":""}),
        None,
    ));
    let empty = dir.path().join("empty.pdf");
    ok(save(&cleared, &empty));
    assert_eq!(inspect(&empty, "1", None)["fields"][0]["value"]["text"], "");
    let check = ok(preview(
        &input,
        dir.path(),
        1,
        &selected["fields"][1],
        json!({"kind":"toggle","checked":false}),
        None,
    ));
    let off = dir.path().join("off.pdf");
    ok(save(&check, &off));
    assert_eq!(
        inspect(&off, "1", None)["fields"][1]["value"]["checked"],
        false
    );
    let choice = inspect(&input, "3", None);
    let b = ok(preview(
        &input,
        dir.path(),
        3,
        &choice["fields"][0],
        json!({"kind":"choice","indices":[0]}),
        None,
    ));
    let chosen = dir.path().join("choice.pdf");
    ok(save(&b, &chosen));
    assert_eq!(
        inspect(&chosen, "3", None)["fields"][0]["value"]["indices"],
        json!([0])
    );
    assert_eq!(fs::read(&input).unwrap(), bytes);
    let e = PdfEngine::start(renderer()).unwrap();
    let a = e.open_document(&input).unwrap();
    let b = e.open_document(&saved).unwrap();
    assert_eq!(
        e.render_page(&a.id, 2, 600).unwrap(),
        e.render_page(&b.id, 2, 600).unwrap()
    );
}
#[test]
fn form_requests_reject_invalid_schema_values_and_stale_sources_without_bundles() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("form.pdf");
    fs::write(&input, forms::fixture()).unwrap();
    let field = inspect(&input, "1", None)["fields"][0].clone();
    for value in [
        Value::Null,
        json!({"kind":"text","text":null}),
        json!({"kind":"text","text":"hello","fontId":"unknown"}),
        json!({"kind":"toggle","checked":true}),
        json!({"kind":"text","text":"x".repeat(32769)}),
        json!({"kind":"text","text":"a\nb"}),
    ] {
        refused(preview(&input, dir.path(), 1, &field, value, None));
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }
    let mut bytes = fs::read(&input).unwrap();
    bytes.extend_from_slice(b"\n% different source\n");
    fs::write(&input, bytes).unwrap();
    refused(preview(
        &input,
        dir.path(),
        1,
        &field,
        json!({"kind":"text","text":"x"}),
        None,
    ));
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
}
#[test]
fn fill_only_permission_retains_protection_and_exact_candidate() {
    use lopdf::encryption::Permissions;
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("protected.pdf");
    let bytes = protected_fixture::encrypt(&forms::fixture(), Permissions::FILLABLE);
    fs::write(&input, &bytes).unwrap();
    let selected = inspect(&input, "1", Some("reader"));
    let b = ok(preview(
        &input,
        dir.path(),
        1,
        &selected["fields"][0],
        json!({"kind":"text","text":"Fill only"}),
        Some("reader"),
    ));
    let saved = dir.path().join("saved.pdf");
    ok(save(&b, &saved));
    assert_eq!(
        inspect(&saved, "1", Some("reader"))["fields"][0]["value"]["text"],
        "Fill only"
    );
    refused(cli("inspect-forms", &saved, &["--page", "1"], None));
    assert_eq!(fs::read(&input).unwrap(), bytes);
    let blocked = dir.path().join("blocked.pdf");
    fs::write(
        &blocked,
        protected_fixture::encrypt(&forms::fixture(), Permissions::COPYABLE),
    )
    .unwrap();
    let f = inspect(&blocked, "1", Some("reader"));
    refused(preview(
        &blocked,
        dir.path(),
        1,
        &f["fields"][0],
        json!({"kind":"text","text":"No"}),
        Some("reader"),
    ));
}
#[test]
fn v3_manifest_rejects_version_confusion_and_changed_metadata() {
    use sha2::{Digest, Sha256};
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("form.pdf");
    fs::write(&input, forms::fixture()).unwrap();
    let field = inspect(&input, "1", None)["fields"][0].clone();
    let bundle = ok(preview(
        &input,
        dir.path(),
        1,
        &field,
        json!({"kind":"text","text":"Answer"}),
        None,
    ));
    let path = Path::new(bundle["folder"].as_str().unwrap()).join("manifest.json");
    let original: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(original["version"], 3);
    for n in 0..4 {
        let mut bad = original.clone();
        match n {
            0 => bad["version"] = json!(1),
            1 => bad["form"] = Value::Null,
            2 => bad["form"]["value"]["extra"] = json!(true),
            _ => bad["replacement"] = json!("fake summary"),
        };
        let bytes = serde_json::to_vec(&bad).unwrap();
        let hash: String = Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        fs::write(&path, bytes).unwrap();
        let mut changed = bundle.clone();
        changed["manifestSha256"] = json!(hash);
        let output = dir.path().join(format!("bad{n}.pdf"));
        refused(save(&changed, &output));
        assert!(!output.exists());
    }
}

#[test]
fn form_inspection_paginates_and_multiline_values_normalize() {
    use lopdf::Object;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("form.pdf");
    let mut d = Document::load_mem(&forms::fixture()).unwrap();
    let name = d
        .objects
        .iter()
        .find_map(|(id, o)| {
            o.as_dict()
                .ok()
                .filter(|x| x.get(b"T").ok().and_then(|o| o.as_str().ok()) == Some(b"name"))
                .map(|_| *id)
        })
        .unwrap();
    d.get_dictionary_mut(name).unwrap().set("Ff", 4096);
    let page = d.get_pages()[&1];
    let mut annots = d
        .get_dictionary(page)
        .unwrap()
        .get(b"Annots")
        .unwrap()
        .as_array()
        .unwrap()
        .clone();
    let template = d
        .get_dictionary(annots[1].as_reference().unwrap())
        .unwrap()
        .clone();
    let form = d
        .catalog()
        .unwrap()
        .get(b"AcroForm")
        .unwrap()
        .as_reference()
        .unwrap();
    let mut roots = d
        .get_dictionary(form)
        .unwrap()
        .get(b"Fields")
        .unwrap()
        .as_array()
        .unwrap()
        .clone();
    for i in 0..28 {
        let mut field = template.clone();
        field.set("T", Object::string_literal(format!("extra-{i}")));
        let id = d.add_object(field);
        annots.push(id.into());
        roots.push(id.into());
    }
    d.get_dictionary_mut(page).unwrap().set("Annots", annots);
    d.get_dictionary_mut(form).unwrap().set("Fields", roots);
    d.save(&path).unwrap();
    let first = inspect(&path, "1", None);
    assert_eq!(first["totalFields"], 30);
    assert_eq!(first["fields"].as_array().unwrap().len(), 25);
    assert_eq!(first["nextOffset"], 25);
    let second = ok(cli(
        "inspect-forms",
        &path,
        &["--page", "1", "--offset", "25"],
        None,
    ));
    assert_eq!(second["fields"].as_array().unwrap().len(), 5);
    assert!(second["nextOffset"].is_null());
    let ids = first["fields"]
        .as_array()
        .unwrap()
        .iter()
        .chain(second["fields"].as_array().unwrap())
        .map(|f| f["fieldId"].as_str().unwrap())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(ids.len(), 30);
    let b = ok(preview(
        &path,
        dir.path(),
        1,
        &first["fields"][0],
        json!({"kind":"text","text":"Line one\r\nLine two"}),
        None,
    ));
    let saved = dir.path().join("multiline.pdf");
    ok(save(&b, &saved));
    assert_eq!(
        inspect(&saved, "1", None)["fields"][0]["value"]["text"],
        "Line one\nLine two"
    );
}

#[test]
fn radio_choice_and_readonly_semantics_follow_native_filling() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("controls.pdf");
    std::fs::write(&path, forms::controls()).unwrap();
    let inspected = inspect(&path, "1", None);
    for (i, (name, value)) in [
        ("combo", json!({"kind":"choice","indices":[1]})),
        ("list", json!({"kind":"choice","indices":[0,1]})),
        ("free", json!({"kind":"text","text":"Custom answer"})),
        ("side", json!({"kind":"toggle","checked":true})),
    ]
    .into_iter()
    .enumerate()
    {
        let field = inspected["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["name"] == name && (name != "side" || f["value"]["checked"] == false))
            .unwrap();
        let b = ok(preview(&path, dir.path(), 1, field, value.clone(), None));
        let saved = dir.path().join(format!("case{i}.pdf"));
        ok(save(&b, &saved));
        let after = inspect(&saved, "1", None);
        let current = after["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["annotationIndex"] == field["annotationIndex"])
            .unwrap();
        assert_eq!(current["value"], value);
        if name == "side" {
            assert_eq!(
                after["fields"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|f| f["name"] == "side" && f["value"]["checked"] == true)
                    .count(),
                1
            );
            refused(preview(
                &saved,
                dir.path(),
                1,
                current,
                json!({"kind":"toggle","checked":false}),
                None,
            ));
        }
    }
    let mut d = Document::load_mem(&forms::fixture()).unwrap();
    let name = d
        .objects
        .iter()
        .find_map(|(id, o)| {
            o.as_dict()
                .ok()
                .filter(|x| x.get(b"T").ok().and_then(|o| o.as_str().ok()) == Some(b"name"))
                .map(|_| *id)
        })
        .unwrap();
    d.get_dictionary_mut(name).unwrap().set("Ff", 1);
    let locked = dir.path().join("readonly.pdf");
    d.save(&locked).unwrap();
    let f = inspect(&locked, "1", None)["fields"][0].clone();
    assert_eq!(f["readOnly"], true);
    refused(preview(
        &locked,
        dir.path(),
        1,
        &f,
        json!({"kind":"text","text":"No"}),
        None,
    ));
}
