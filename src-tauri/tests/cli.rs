use serde_json::Value;

#[cfg(windows)]
#[test]
fn non_unicode_windows_outputs_have_lossless_json_after_publication() {
    use std::{
        ffi::OsString,
        fs,
        os::windows::ffi::{OsStrExt, OsStringExt},
    };
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("source.pdf");
    fixture(&input);
    let native_name = OsString::from_wide(&[0xd800, 0x002e, 0x0074, 0x0078, 0x0074]);
    let output = tmp.path().join(native_name);
    let result = success(invoke(
        "text",
        &input,
        &[OsStr::new("--out"), output.as_os_str()],
        None,
    ));
    assert_eq!(result["path"], Value::Null);
    assert_eq!(result["pathNative"]["encoding"], "windows-utf16");
    let units: Vec<u16> = serde_json::from_value(result["pathNative"]["units"].clone()).unwrap();
    let recovered = PathBuf::from(OsString::from_wide(&units));
    assert_eq!(
        recovered.as_os_str().encode_wide().collect::<Vec<_>>(),
        output
            .canonicalize()
            .unwrap()
            .as_os_str()
            .encode_wide()
            .collect::<Vec<_>>()
    );
    assert!(fs::read_to_string(&recovered)
        .unwrap()
        .contains("First page"));
    let parent = tmp.path().join(OsString::from_wide(&[0xd801]));
    fs::create_dir(&parent).unwrap();
    let result = success(invoke(
        "render",
        &input,
        &[
            OsStr::new("--out-dir"),
            parent.as_os_str(),
            OsStr::new("--page"),
            OsStr::new("1"),
            OsStr::new("--dpi"),
            OsStr::new("72"),
        ],
        None,
    ));
    assert_eq!(result["folder"], Value::Null);
    assert_eq!(result["folderNative"]["encoding"], "windows-utf16");
    let units: Vec<u16> = serde_json::from_value(result["folderNative"]["units"].clone()).unwrap();
    let folder = PathBuf::from(OsString::from_wide(&units));
    assert!(folder.join("page-0001.png").is_file());
}
use std::{
    ffi::OsStr,
    io::Write,
    path::PathBuf,
    process::{Command, Output, Stdio},
};

fn cli(args: &[&OsStr], stdin: Option<&[u8]>) -> Output {
    let binary =
        option_env!("CARGO_BIN_EXE_folio-cli").expect("Folio CLI executable is not implemented");
    let mut child = Command::new(binary)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(bytes) = stdin {
        child.stdin.as_mut().unwrap().write_all(bytes).unwrap();
    }
    drop(child.stdin.take());
    child.wait_with_output().unwrap()
}
fn words(args: &[&str]) -> Output {
    cli(&args.iter().map(OsStr::new).collect::<Vec<_>>(), None)
}
fn failure(out: Output, status: i32, code: &str) {
    assert_eq!(
        out.status.code(),
        Some(status),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stdout.is_empty());
    let json: Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(json["schemaVersion"], 1);
    assert_eq!(json["ok"], false);
    assert_eq!(json["error"]["code"], code);
}
fn renderer() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")
}

fn fixture(path: &std::path::Path) {
    use lopdf::{dictionary, Document, Stream};
    let mut pdf = Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let font =
        pdf.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let mut kids = vec![];
    for (i, text) in ["First page", "Second page"].iter().enumerate() {
        let stream = pdf.add_object(Stream::new(
            dictionary! {},
            format!("BT /F1 12 Tf 30 200 Td ({text}) Tj ET").into_bytes(),
        ));
        let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),300.into(),240.into()],"Rotate"=>if i==0{90}else{0},"UserUnit"=>1.7f32,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>stream});
        kids.push(page.into());
    }
    pdf.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Count"=>2,"Kids"=>kids}.into(),
    );
    let root = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    pdf.trailer.set("Root", root);
    pdf.save(path).unwrap();
}
fn invoke(
    command: &str,
    input: &std::path::Path,
    extra: &[&OsStr],
    password: Option<&[u8]>,
) -> Output {
    let engine = renderer();
    let mut args = vec![
        OsStr::new(command),
        input.as_os_str(),
        OsStr::new("--engine"),
        engine.as_os_str(),
    ];
    args.extend_from_slice(extra);
    cli(&args, password)
}
fn success(out: Output) -> Value {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.stderr.is_empty());
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["schemaVersion"], 1);
    v["result"].clone()
}
fn plans(info: &folio_engine::DocumentInfo) -> Vec<folio_engine::PagePlan> {
    info.pages
        .iter()
        .enumerate()
        .map(|(i, p)| folio_engine::PagePlan {
            id: format!("page-{i}"),
            source_id: info.id.clone(),
            page_index: i,
            width: p.width,
            height: p.height,
            rotation: 0,
            crop: None,
            overlays: p.overlays.clone(),
        })
        .collect()
}

#[test]
fn unicode_file_info_and_text_match_desktop_export_without_source_changes() {
    use folio_engine::{PdfEngine, TextExportControl, TextExportRequest};
    use std::{fs, sync::Arc};
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("résumé 日本語.pdf");
    fixture(&input);
    let original = fs::read(&input).unwrap();
    let engine = PdfEngine::start(renderer()).unwrap();
    let doc = engine.open_document(&input).unwrap();
    let info = success(invoke("info", &input, &[], None));
    assert_eq!(info["pageCount"], 2);
    assert_eq!(info["name"], "résumé 日本語.pdf");
    assert!(info.get("id").is_none());
    assert!(info["pages"][0].get("overlays").is_none());
    assert!(
        (info["pages"][0]["width"].as_f64().unwrap() - f64::from(doc.pages[0].width)).abs() < 0.001
    );
    for page in [None, Some(2)] {
        let output = tmp.path().join(format!("résultat-{page:?}.txt"));
        let expected = tmp.path().join(format!("expected-{page:?}.txt"));
        let mut args = vec![OsStr::new("--out"), output.as_os_str()];
        if page.is_some() {
            args.extend([OsStr::new("--page"), OsStr::new("2")]);
        }
        let result = success(invoke("text", &input, &args, None));
        assert_eq!(result["pageCount"], if page.is_some() { 1 } else { 2 });
        let mut pages = plans(&doc);
        if page.is_some() {
            pages = vec![pages.remove(1)];
        }
        engine
            .export_text(
                TextExportRequest {
                    pages,
                    include_form_values: false,
                },
                &expected,
                Arc::new(TextExportControl::new()),
                Arc::new(|_| {}),
            )
            .unwrap()
            .unwrap();
        assert_eq!(fs::read(&output).unwrap(), fs::read(&expected).unwrap());
    }
    assert_eq!(fs::read(&input).unwrap(), original);
    engine.close_document(&doc.id).unwrap();
}

#[test]
fn png_export_matches_desktop_geometry_pixels_and_unique_folder_publication() {
    use folio_engine::{
        PageImageControl, PageImageExportRequest, PageImageFormat, PageImageOptions, PdfEngine,
    };
    use std::{fs, sync::Arc};
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("source.pdf");
    fixture(&input);
    let original = fs::read(&input).unwrap();
    let result = success(invoke(
        "render",
        &input,
        &[
            OsStr::new("--out-dir"),
            tmp.path().as_os_str(),
            OsStr::new("--page"),
            OsStr::new("1"),
            OsStr::new("--dpi"),
            OsStr::new("72"),
        ],
        None,
    ));
    let folder = PathBuf::from(result["folder"].as_str().unwrap());
    assert_eq!(result["count"], 1);
    assert_eq!(folder.parent().unwrap(), tmp.path().canonicalize().unwrap());
    let engine = PdfEngine::start(renderer()).unwrap();
    let doc = engine.open_document(&input).unwrap();
    let expected = engine
        .export_page_images(
            PageImageExportRequest {
                pages: vec![plans(&doc).remove(0)],
                page_numbers: vec![1],
                workspace_page_count: 2,
                options: PageImageOptions {
                    format: PageImageFormat::Png,
                    dpi: 72,
                    jpeg_quality: 90,
                },
            },
            tmp.path(),
            Arc::new(PageImageControl::new()),
            Arc::new(|_| {}),
        )
        .unwrap()
        .unwrap();
    let a = image::open(folder.join("page-0001.png")).unwrap().to_rgb8();
    let b = image::open(expected.folder.join("page-0001.png"))
        .unwrap()
        .to_rgb8();
    assert_eq!(a, b);
    assert_eq!(fs::read(input).unwrap(), original);
    engine.close_document(&doc.id).unwrap();
}

#[test]
fn failures_preserve_destinations_and_source_aliases() {
    use std::fs;
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("source.pdf");
    fixture(&input);
    let original = fs::read(&input).unwrap();
    let output = tmp.path().join("keep.txt");
    fs::write(&output, b"keep this").unwrap();
    failure(
        invoke(
            "text",
            &input,
            &[OsStr::new("--out"), output.as_os_str()],
            None,
        ),
        1,
        "invalid_request",
    );
    assert_eq!(fs::read(&output).unwrap(), b"keep this");
    let alias = tmp.path().join("alias.txt");
    fs::hard_link(&input, &alias).unwrap();
    failure(
        invoke(
            "text",
            &input,
            &[OsStr::new("--out"), alias.as_os_str()],
            None,
        ),
        1,
        "invalid_request",
    );
    let absent = tmp.path().join("absent.txt");
    failure(
        invoke(
            "text",
            &input,
            &[
                OsStr::new("--out"),
                absent.as_os_str(),
                OsStr::new("--page"),
                OsStr::new("3"),
            ],
            None,
        ),
        1,
        "invalid_request",
    );
    assert!(!absent.exists());
    assert_eq!(fs::read(input).unwrap(), original);
    assert!(!fs::read_dir(tmp.path()).unwrap().any(|f| f
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".folio-cli-")));
}

#[path = "support/protected_fixture.rs"]
mod protected_fixture;
#[test]
fn password_stdin_preserves_copy_permissions_and_never_echoes_password() {
    use lopdf::encryption::Permissions;
    use std::fs;
    let tmp = tempfile::tempdir().unwrap();
    let plain = tmp.path().join("plain.pdf");
    fixture(&plain);
    let bytes = fs::read(&plain).unwrap();
    let input = tmp.path().join("locked.pdf");
    fs::write(
        &input,
        protected_fixture::encrypt(&bytes, Permissions::PRINTABLE),
    )
    .unwrap();
    failure(invoke("info", &input, &[], None), 1, "password_required");
    let result = success(invoke(
        "info",
        &input,
        &[OsStr::new("--password-stdin")],
        Some(b"reader\r\n"),
    ));
    assert_eq!(result["protection"]["permissions"]["copyContent"], false);
    let out = invoke(
        "info",
        &input,
        &[OsStr::new("--password-stdin")],
        Some(b"never-echo-me"),
    );
    assert!(!String::from_utf8_lossy(&out.stderr).contains("never-echo-me"));
    failure(out, 1, "incorrect_password");
    let output = tmp.path().join("denied.txt");
    failure(
        invoke(
            "text",
            &input,
            &[
                OsStr::new("--out"),
                output.as_os_str(),
                OsStr::new("--password-stdin"),
            ],
            Some(b"reader"),
        ),
        1,
        "permission_denied",
    );
    assert!(!output.exists());
    let result = invoke(
        "render",
        &input,
        &[
            OsStr::new("--out-dir"),
            tmp.path().as_os_str(),
            OsStr::new("--password-stdin"),
        ],
        Some(b"reader"),
    );
    failure(result, 1, "permission_denied");
}

#[test]
fn saved_text_annotations_and_opt_in_form_values_follow_desktop_behavior() {
    use folio_engine::{ExportRequest, PdfEngine};
    use lopdf::{dictionary, Document, Object};
    use std::fs;
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("source.pdf");
    fixture(&source);
    let engine = PdfEngine::start(renderer()).unwrap();
    let info = engine.open_document(&source).unwrap();
    let mut pages = plans(&info);
    pages[1].overlays=serde_json::from_value(serde_json::json!([{"type":"text","id":"note","x":40,"y":100,"text":"Saved Folio note","fontSize":12,"color":"#ff0000"}])).unwrap();
    let saved = tmp.path().join("saved.pdf");
    engine
        .export_pdf(
            ExportRequest {
                pages,
                flatten: false,
            },
            &saved,
        )
        .unwrap();
    engine.close_document(&info.id).unwrap();
    let mut pdf = Document::load(&saved).unwrap();
    let page = *pdf.get_pages().get(&2).unwrap();
    let field=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Tx","T"=>Object::string_literal("Name"),"V"=>Object::string_literal("Alice"),"F"=>4,"Rect"=>vec![30.into(),20.into(),200.into(),40.into()],"P"=>page});
    let dict = pdf.get_dictionary_mut(page).unwrap();
    let mut annots = dict
        .get(b"Annots")
        .ok()
        .and_then(|a| a.as_array().ok())
        .cloned()
        .unwrap_or_default();
    annots.push(field.into());
    dict.set("Annots", annots);
    let root = pdf.trailer.get(b"Root").unwrap().as_reference().unwrap();
    pdf.get_dictionary_mut(root).unwrap().set(
        "AcroForm",
        dictionary! {"Fields"=>vec![Object::Reference(field)]},
    );
    let input = tmp.path().join("forms.pdf");
    pdf.save(&input).unwrap();
    let original = fs::read(&input).unwrap();
    for include in [false, true] {
        let output = tmp.path().join(format!("forms-{include}.txt"));
        let mut args = vec![OsStr::new("--out"), output.as_os_str()];
        if include {
            args.push(OsStr::new("--include-form-values"));
        }
        success(invoke("text", &input, &args, None));
        let text = fs::read_to_string(output).unwrap();
        assert!(text.contains("Saved Folio note"), "{text}");
        assert_eq!(text.contains("Name: Alice"), include, "{text}");
    }
    assert_eq!(fs::read(&input).unwrap(), original);
}

#[test]
fn malformed_pdf_never_publishes_a_text_output() {
    use std::fs;
    let tmp = tempfile::tempdir().unwrap();
    let input = tmp.path().join("invalid.pdf");
    fs::write(&input, b"Not a PDF with private contents").unwrap();
    let output = tmp.path().join("out.txt");
    let result = invoke(
        "text",
        &input,
        &[OsStr::new("--out"), output.as_os_str()],
        None,
    );
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&result.stderr).contains("private contents"));
    assert!(!output.exists());
    assert_eq!(
        fs::read(&input).unwrap(),
        b"Not a PDF with private contents"
    );
}

#[test]
fn help_and_version_work_without_loading_renderer() {
    let out = words(&["--help"]);
    assert!(out.status.success());
    assert!(out.stderr.is_empty());
    let help = String::from_utf8(out.stdout).unwrap();
    for word in ["info", "text", "render", "--password-stdin", "--engine"] {
        assert!(help.contains(word));
    }
    let out = words(&["--version"]);
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap().trim(),
        concat!("folio-cli ", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn invalid_arguments_are_machine_readable_and_never_load_a_document() {
    for args in [
        vec![],
        vec!["unknown"],
        vec!["info"],
        vec!["text", "missing.pdf"],
        vec!["info", "missing.pdf", "--page", "1"],
        vec!["render", "missing.pdf", "--out-dir", ".", "--dpi", "73"],
        vec!["text", "missing.pdf", "--out", "x.txt", "--page", "0"],
        vec![
            "text",
            "missing.pdf",
            "--out",
            "x.txt",
            "--page",
            "1",
            "--page",
            "2",
        ],
        vec!["info", "missing.pdf", "--password", "do-not-echo-this"],
    ] {
        let out = words(&args);
        assert!(!String::from_utf8_lossy(&out.stderr).contains("do-not-echo-this"));
        failure(out, 2, "invalid_arguments");
    }
}

#[test]
fn missing_renderer_is_an_operation_error_and_stdin_is_bounded() {
    failure(
        words(&["info", "missing.pdf", "--engine", "missing-renderer.dll"]),
        1,
        "engine_unavailable",
    );
    let args = [
        "info",
        "missing.pdf",
        "--engine",
        "missing-renderer.dll",
        "--password-stdin",
    ]
    .map(OsStr::new);
    for bytes in [
        vec![b'x'; 1027],
        b"secret\nsecond-line".to_vec(),
        vec![0xff],
        b"secret\0".to_vec(),
    ] {
        let out = cli(&args, Some(&bytes));
        assert!(!String::from_utf8_lossy(&out.stderr).contains("secret"));
        failure(out, 2, "invalid_arguments");
    }
    let _ = renderer();
}
