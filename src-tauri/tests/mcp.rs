use serde_json::{json, Value};
#[path="support/automation_forms.rs"]
mod automation_forms;

#[test]
fn form_previews_apply_and_fill_consecutively_without_restart() {
    let dir=tempfile::tempdir().unwrap();let source=dir.path().join("forms.pdf");let original=automation_forms::fixture();std::fs::write(&source,&original).unwrap();
    let output=dir.path().join("out");std::fs::create_dir(&output).unwrap();let mut s=Session::start(&source,&output);
    let first=success(call(&mut s,1,"folio_inspect_forms",json!({"documentId":"document-1","page":1})));
    let field=&first["fields"][0];
    let invalid=call(&mut s,2,"folio_preview_form",json!({"documentId":"document-1","page":1,"fieldId":field["fieldId"],"value":{"kind":"text","text":"Answer","fontId":"arbitrary"}}));assert_eq!(invalid["result"]["isError"],true);
    let preview=success(call(&mut s,3,"folio_preview_form",json!({"documentId":"document-1","page":1,"fieldId":field["fieldId"],"value":{"kind":"text","text":"MCP answer"}})));
    assert_eq!(preview["fieldId"],field["fieldId"]);assert_eq!(preview["form"]["oldValue"],field["value"]);let before=preview["before"].as_str().unwrap().to_string();
    let applied=success(call(&mut s,4,"folio_apply_preview",json!({"previewId":preview["previewId"]})));
    let next=success(call(&mut s,5,"folio_inspect_forms",json!({"documentId":applied["documentId"],"page":2})));assert_eq!(next["fields"][0]["value"]["text"],"MCP answer");
    let stale=call(&mut s,6,"folio_preview_form",json!({"documentId":applied["documentId"],"page":1,"fieldId":field["fieldId"],"value":{"kind":"text","text":"Stale"}}));assert_eq!(stale["result"]["isError"],true);
    let revised=success(call(&mut s,7,"folio_preview_form",json!({"documentId":applied["documentId"],"page":2,"fieldId":next["fields"][0]["fieldId"],"value":{"kind":"text","text":"Second answer"}})));
    let saved=success(call(&mut s,8,"folio_save_preview",json!({"previewId":revised["previewId"]})));assert_eq!(saved["sha256"],revised["candidateSha256"]);
    let final_doc=success(call(&mut s,9,"folio_apply_preview",json!({"previewId":revised["previewId"]})));
    let final_fields=success(call(&mut s,10,"folio_inspect_forms",json!({"documentId":final_doc["documentId"],"page":1})));assert_eq!(final_fields["fields"][0]["value"]["text"],"Second answer");
    assert_eq!(success(call(&mut s,11,"folio_inspect_forms",json!({"documentId":"document-1","page":1}))),first);
    success(call(&mut s,12,"folio_discard_preview",json!({"previewId":preview["previewId"]})));assert!(!Path::new(&before).exists());
    success(call(&mut s,13,"folio_close_document",json!({"documentId":applied["documentId"]})));let after=revised["after"].as_str().unwrap().to_string();s.stop();assert!(!Path::new(&after).exists());assert!(Path::new(saved["path"].as_str().unwrap()).exists());assert_eq!(std::fs::read(&source).unwrap(),original);
}
use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    time::Duration,
};

struct Session {
    child: Child,
    input: Option<ChildStdin>,
    replies: Receiver<Value>,
}
impl Session {
    fn start(source: &Path, output: &Path) -> Self {
        Self::start_many(&[source], output)
    }
    fn start_many(sources: &[&Path], output: &Path) -> Self {
        Self::start_with_fonts(sources,output,&[])
    }
    fn start_with_fonts(sources: &[&Path], output: &Path, fonts: &[&Path]) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_folio-cli"))
            .arg("mcp")
            .args(
                sources
                    .iter()
                    .flat_map(|source| [std::ffi::OsStr::new("--input"), source.as_os_str()]),
            )
            .args(fonts.iter().flat_map(|font|[std::ffi::OsStr::new("--font"),font.as_os_str()]))
            .arg("--out-dir")
            .arg(output)
            .arg("--engine")
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let input = child.stdin.take();
        let stdout = child.stdout.take().unwrap();
        let (tx, replies) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                let parsed = serde_json::from_str(&line).unwrap_or(json!({"nonProtocol":line}));
                if tx.send(parsed).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            input,
            replies,
        }
    }
    fn send(&mut self, v: Value) {
        self.raw(&format!("{v}\n"));
    }
    fn raw(&mut self, s: &str) {
        self.input
            .as_mut()
            .unwrap()
            .write_all(s.as_bytes())
            .unwrap();
    }
    fn receive(&self) -> Value {
        self.replies
            .recv_timeout(Duration::from_secs(30))
            .expect("MCP must respond with JSON-RPC")
    }
    fn request(&mut self, id: u32, method: &str, mut params: Value) -> Value {
        params["_meta"] = meta();
        self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
        let v = self.receive();
        assert_eq!(v["id"], id, "{v}");
        assert_eq!(v["jsonrpc"], "2.0");
        v
    }
    fn stop(&mut self) {
        drop(self.input.take());
        for _ in 0..200 {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(status.success());
                return;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        panic!("MCP failed to exit on EOF");
    }
}

fn explicit_font_session(legacy:bool) {
    use sha2::{Digest,Sha256};
    for nested in [false,true] {
        let dir=tempfile::tempdir().unwrap();let source=dir.path().join("source.pdf");
        if nested {nested_automation::write_fixture(&source,nested_automation::Mode::RepeatedInnerFar,1.);}else{pdf(&source);}
        let original=std::fs::read(&source).unwrap();let output=dir.path().join("exports");std::fs::create_dir(&output).unwrap();
        let font=dir.path().join("selected.ttf");std::fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf"),&font).unwrap();
        let font_bytes=std::fs::read(&font).unwrap();let font_id:String=Sha256::digest(&font_bytes).iter().map(|b|format!("{b:02x}")).collect();
        let mut s=Session::start_with_fonts(&[&source],&output,&[&font]);
        if legacy {
            s.send(json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"font-test","version":"1"}}}));
            assert_eq!(s.receive()["result"]["protocolVersion"],"2025-11-25");s.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        }
        let invoke=|s:&mut Session,id:u32,name:&str,args:Value| {
            if legacy {s.send(json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}}));let r=s.receive();assert_eq!(r["id"],id);r}else{call(s,id,name,args)}
        };
        let fonts=success(invoke(&mut s,1,"folio_list_fonts",json!({})));
        assert_eq!(fonts["fonts"].as_array().unwrap().len(),1);assert_eq!(fonts["fonts"][0]["fontId"],font_id);
        assert_eq!(fonts["fonts"][0].as_object().unwrap().len(),5);
        std::fs::write(&font,b"mutated after authorization").unwrap();std::fs::remove_file(&font).unwrap();
        let route=if nested {"folio_inspect_nested_text"}else{"folio_inspect_text"};
        let text=success(invoke(&mut s,2,route,json!({"documentId":"document-1","page":1})));
        let seed=if nested {text["runs"].as_array().unwrap().iter().find(|r|r["bounds"]["x"].as_f64().unwrap()>300. && r["text"]=="Nested first line").unwrap()}else{&text["runs"][0]};
        let selected=success(invoke(&mut s,3,"folio_inspect_paragraph",json!({"documentId":"document-1","page":1,"selectionId":seed["selectionId"]})));
        let replacement=format!("{} this",selected["paragraph"]["text"].as_str().unwrap());
        let args=json!({"documentId":"document-1","page":1,"seedSelectionId":selected["seedSelectionId"],"paragraphId":selected["paragraphId"],"runs":[{"text":replacement,"styleId":0}],"fontOverrides":[{"styleId":0,"fontId":font_id}],"fit":true});
        let mut bad=args.clone();bad["fontOverrides"][0]["fontId"]=json!("a".repeat(64));assert_eq!(invoke(&mut s,4,"folio_preview_paragraph",bad)["result"]["isError"],true);
        let mut bad=args.clone();bad["fontPath"]=json!("arbitrary.ttf");assert_eq!(invoke(&mut s,5,"folio_preview_paragraph",bad)["result"]["isError"],true);
        let preview=success(invoke(&mut s,6,"folio_preview_paragraph",args.clone()));
        assert_eq!(preview["paragraph"]["fontOverrides"],args["fontOverrides"]);assert_eq!(preview["paragraph"]["substituteFonts"],fonts["fonts"]);
        let applied=success(invoke(&mut s,7,"folio_apply_preview",json!({"previewId":preview["previewId"]})));
        let text=success(invoke(&mut s,8,"folio_inspect_text",json!({"documentId":applied["documentId"],"page":1})));
        let prefix=if nested {"Nested first line"}else{"Hello from Folio MCP"};
        let seed=text["runs"].as_array().unwrap().iter().find(|r|r["text"].as_str().unwrap().starts_with(prefix)).unwrap();
        assert_eq!(seed["fontName"],format!("Folio{font_id}Paragraph"));
        let next=success(invoke(&mut s,9,"folio_inspect_paragraph",json!({"documentId":applied["documentId"],"page":1,"selectionId":seed["selectionId"]})));
        assert_eq!(next["paragraph"]["text"],replacement);
        let second=success(invoke(&mut s,10,"folio_preview_paragraph",json!({"documentId":applied["documentId"],"page":1,"seedSelectionId":next["seedSelectionId"],"paragraphId":next["paragraphId"],"runs":[{"text":format!("{replacement} this"),"styleId":0}],"fit":true})));
        let saved=success(invoke(&mut s,11,"folio_save_preview",json!({"previewId":second["previewId"]})));
        let path=Path::new(saved["path"].as_str().unwrap());let bytes=std::fs::read(path).unwrap();
        assert_eq!(success(invoke(&mut s,12,"folio_list_fonts",json!({}))),fonts);
        success(invoke(&mut s,13,"folio_close_document",json!({"documentId":applied["documentId"]})));
        s.stop();assert_eq!(std::fs::read(path).unwrap(),bytes);assert_eq!(std::fs::read(&source).unwrap(),original);
        let document=lopdf::Document::load_mem(&bytes).unwrap();
        assert!(document.objects.values().any(|v|v.as_stream().is_ok_and(|stream|stream.decompressed_content().unwrap_or_else(|_|stream.content.clone())==font_bytes)));
    }
}
#[test]
fn explicit_font_inputs_modern_snapshot_apply_reedit_save() {explicit_font_session(false);}
#[test]
fn explicit_font_inputs_legacy_snapshot_apply_reedit_save() {explicit_font_session(true);}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn meta() -> Value {
    json!({"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}})
}
fn setup() -> (tempfile::TempDir, Session) {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("Sample.pdf");
    std::fs::write(&source, b"not parsed until requested").unwrap();
    let session = Session::start(&source, dir.path());
    (dir, session)
}

#[test]
fn mcp_help_is_available_without_renderer() {
    let out = Command::new(env!("CARGO_BIN_EXE_folio-cli"))
        .args(["mcp", "--help"])
        .output()
        .unwrap();
    assert!(out.status.success(), "MCP help must exist");
    let help = String::from_utf8(out.stdout).unwrap();
    assert!(help.contains("--input") && help.contains("--out-dir"));
}

#[test]
fn removing_private_session_state_is_not_advertised_as_additive() {
    let (_dir, mut s) = setup();
    let listed = s.request(1, "tools/list", json!({}));
    let tools = listed["result"]["tools"].as_array().unwrap();
    for name in ["folio_close_document", "folio_discard_preview"] {
        let tool = tools.iter().find(|t| t["name"] == name).unwrap();
        assert_eq!(tool["annotations"]["readOnlyHint"], false);
        assert_eq!(
            tool["annotations"]["destructiveHint"], true,
            "{name} removes retained session state"
        );
    }
    for name in ["folio_apply_preview", "folio_save_preview"] {
        let tool = tools.iter().find(|t| t["name"] == name).unwrap();
        assert_eq!(tool["annotations"]["destructiveHint"], false);
    }
    s.stop();
}

#[test]
fn current_discovery_tools_and_metadata_validation() {
    let (_dir, mut s) = setup();
    let v = s.request(1, "server/discover", json!({}));
    assert_eq!(v["result"]["resultType"], "complete");
    assert!(v["result"]["supportedVersions"]
        .as_array()
        .unwrap()
        .contains(&json!("2026-07-28")));
    let v = s.request(2, "tools/list", json!({}));
    let tools = v["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 16);
    let fonts=tools.iter().find(|tool|tool["name"]=="folio_list_fonts").unwrap();
    assert_eq!(fonts["annotations"]["readOnlyHint"],true);
    assert_eq!(fonts["inputSchema"]["properties"],json!({}));
    let paragraph=tools.iter().find(|tool|tool["name"]=="folio_preview_paragraph").unwrap();
    assert_eq!(paragraph["inputSchema"]["properties"]["fontOverrides"]["maxItems"],64);
    assert!(tools
        .iter()
        .all(|t| t["inputSchema"]["additionalProperties"] == false));
    s.send(json!({"jsonrpc":"2.0","id":3,"method":"tools/list","params":{}}));
    assert_eq!(s.receive()["error"]["code"], -32602);
    let mut metadata = meta();
    metadata["io.modelcontextprotocol/protocolVersion"] = json!("2099-01-01");
    s.send(json!({"jsonrpc":"2.0","id":4,"method":"tools/list","params":{"_meta":metadata}}));
    let v = s.receive();
    assert_eq!(v["error"]["code"], -32022);
    assert_eq!(v["error"]["data"]["requested"], "2099-01-01");
    let v = s.request(
        5,
        "tools/call",
        json!({"name":"folio_list_documents","arguments":{}}),
    );
    assert_eq!(
        v["result"]["structuredContent"]["documents"][0]["id"],
        "document-1"
    );
    s.stop();
}

#[test]
fn legacy_handshake_and_modern_requests_do_not_share_version_state() {
    let (_dir, mut s) = setup();
    s.send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}));
    let v = s.receive();
    assert_eq!(v["result"]["protocolVersion"], "2025-11-25");
    assert!(v["result"].get("resultType").is_none());
    s.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    s.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}));
    let v = s.receive();
    assert_eq!(v["result"]["tools"].as_array().unwrap().len(), 16);
    assert!(v["result"].get("resultType").is_none());
    let v = s.request(3, "tools/list", json!({}));
    assert_eq!(v["result"]["resultType"], "complete");
    s.send(json!({"jsonrpc":"2.0","id":4,"method":"tools/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"invalid"}}}));
    assert_eq!(s.receive()["error"]["code"], -32022);
    s.stop();
}

#[test]
fn malformed_frames_recover_and_notifications_never_execute_tools() {
    let (_dir, mut s) = setup();
    s.raw("{\n");
    assert_eq!(s.receive()["error"]["code"], -32700);
    s.raw(&format!("{}\n", "x".repeat(512*1024+1)));
    assert_eq!(s.receive()["error"]["code"], -32600);
    s.raw("[]\n");
    assert_eq!(s.receive()["error"]["code"], -32600);
    s.send(json!({"jsonrpc":"2.0","method":"tools/call","params":{"_meta":meta(),"name":"folio_export_text","arguments":{"documentId":"document-1"}}}));
    let v = s.request(8, "tools/list", json!({}));
    assert!(v.get("result").is_some(), "{v}");
    s.stop();
}

fn pdf(path: &Path) {
    use lopdf::{dictionary, Document, Stream};
    let mut pdf = Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let font =
        pdf.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let stream = pdf.add_object(Stream::new(
        dictionary! {},
        b"BT /F1 12 Tf 30 180 Td (Hello from Folio MCP) Tj ET".to_vec(),
    ));
    let page = pdf.add_object(dictionary! {"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),300.into(),240.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>stream});
    pdf.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Count"=>1,"Kids"=>vec![page.into()]}.into(),
    );
    let root = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    pdf.trailer.set("Root", root);
    pdf.save(path).unwrap();
}
fn call(s: &mut Session, id: u32, name: &str, arguments: Value) -> Value {
    s.request(id, "tools/call", json!({"name":name,"arguments":arguments}))
}
fn success(v: Value) -> Value {
    assert_eq!(v["result"]["isError"], false, "{v}");
    v["result"]["structuredContent"].clone()
}

#[test]
fn applied_previews_support_consecutive_edits_and_stable_document_ids() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pdf");
    pdf(&source);
    let original = std::fs::read(&source).unwrap();
    let out = dir.path().join("exports");
    std::fs::create_dir(&out).unwrap();
    let mut s = Session::start(&source, &out);
    let first = success(call(
        &mut s,
        1,
        "folio_inspect_text",
        json!({"documentId":"document-1","page":1}),
    ));
    let preview = success(call(
        &mut s,
        2,
        "folio_preview_text",
        json!({"documentId":"document-1","page":1,"selectionId":first["runs"][0]["selectionId"],"replacement":"Hello edited"}),
    ));
    let applied = success(call(
        &mut s,
        3,
        "folio_apply_preview",
        json!({"previewId":preview["previewId"]}),
    ));
    assert_eq!(applied["documentId"], "document-2");
    assert_eq!(applied["sha256"], preview["candidateSha256"]);
    assert!(applied.get("path").is_none());
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 0);
    let second = success(call(
        &mut s,
        4,
        "folio_inspect_text",
        json!({"documentId":applied["documentId"],"page":1}),
    ));
    assert_eq!(second["runs"][0]["text"], "Hello edited");
    assert_eq!(second["sourceFingerprint"], applied["sha256"]);
    let duplicate = success(call(
        &mut s,
        5,
        "folio_apply_preview",
        json!({"previewId":preview["previewId"]}),
    ));
    assert_eq!(duplicate["documentId"], "document-3");
    success(call(
        &mut s,
        6,
        "folio_discard_preview",
        json!({"previewId":preview["previewId"]}),
    ));
    let stale = call(
        &mut s,
        7,
        "folio_preview_text",
        json!({"documentId":"document-2","page":1,"selectionId":first["runs"][0]["selectionId"],"replacement":"Wrong source"}),
    );
    assert_eq!(stale["result"]["isError"], true);
    let next = success(call(
        &mut s,
        8,
        "folio_preview_text",
        json!({"documentId":"document-2","page":1,"selectionId":second["runs"][0]["selectionId"],"replacement":"Hello edited twice"}),
    ));
    let saved = success(call(
        &mut s,
        9,
        "folio_save_preview",
        json!({"previewId":next["previewId"]}),
    ));
    let final_doc = success(call(
        &mut s,
        10,
        "folio_apply_preview",
        json!({"previewId":next["previewId"]}),
    ));
    assert_eq!(final_doc["documentId"], "document-4");
    assert_eq!(final_doc["sha256"], saved["sha256"]);
    std::fs::write(
        saved["path"].as_str().unwrap(),
        b"public saved file changed",
    )
    .unwrap();
    success(call(
        &mut s,
        11,
        "folio_close_document",
        json!({"documentId":"document-1"}),
    ));
    success(call(
        &mut s,
        12,
        "folio_close_document",
        json!({"documentId":"document-2"}),
    ));
    let still_third = success(call(
        &mut s,
        13,
        "folio_inspect_text",
        json!({"documentId":"document-3","page":1}),
    ));
    assert_eq!(still_third["runs"][0]["text"], "Hello edited");
    let still_fourth = success(call(
        &mut s,
        14,
        "folio_inspect_text",
        json!({"documentId":"document-4","page":1}),
    ));
    assert_eq!(still_fourth["runs"][0]["text"], "Hello edited twice");
    let closed = call(&mut s, 15, "folio_info", json!({"documentId":"document-1"}));
    assert_eq!(closed["result"]["isError"], true);
    let newer = success(call(
        &mut s,
        16,
        "folio_apply_preview",
        json!({"previewId":next["previewId"]}),
    ));
    assert_eq!(newer["documentId"], "document-5");
    for (n, id) in ["document-3", "document-4", "document-5"]
        .iter()
        .enumerate()
    {
        success(call(
            &mut s,
            20 + n as u32,
            "folio_close_document",
            json!({"documentId":id}),
        ));
    }
    let listed = success(call(&mut s, 30, "folio_list_documents", json!({})));
    assert_eq!(listed["documents"].as_array().unwrap().len(), 0);
    // An independently owned preview remains applicable after every input closes.
    let reopened = success(call(
        &mut s,
        31,
        "folio_apply_preview",
        json!({"previewId":next["previewId"]}),
    ));
    assert_eq!(reopened["documentId"], "document-6");
    s.stop();
    assert_eq!(std::fs::read(source).unwrap(), original);
    assert_eq!(
        std::fs::read(saved["path"].as_str().unwrap()).unwrap(),
        b"public saved file changed"
    );
}

#[test]
fn invalid_or_modified_previews_do_not_register_new_documents() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pdf");
    pdf(&source);
    let out = dir.path().join("out");
    std::fs::create_dir(&out).unwrap();
    let mut s = Session::start(&source, &out);
    let inspected = success(call(
        &mut s,
        1,
        "folio_inspect_text",
        json!({"documentId":"document-1","page":1}),
    ));
    let preview = success(call(
        &mut s,
        2,
        "folio_preview_text",
        json!({"documentId":"document-1","page":1,"selectionId":inspected["runs"][0]["selectionId"],"replacement":"Hello edited"}),
    ));
    std::fs::write(preview["after"].as_str().unwrap(), b"tampered").unwrap();
    let invalid = call(
        &mut s,
        3,
        "folio_apply_preview",
        json!({"previewId":preview["previewId"]}),
    );
    assert_eq!(invalid["result"]["isError"], true);
    for (i, (name, args)) in [
        (
            "folio_apply_preview",
            json!({"previewId":preview["previewId"],"input":"other.pdf"}),
        ),
        (
            "folio_apply_preview",
            json!({"previewId":"preview-unknown"}),
        ),
        (
            "folio_close_document",
            json!({"documentId":"document-1","path":"other.pdf"}),
        ),
        ("folio_close_document", json!({"documentId":"document-2"})),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            call(&mut s, 10 + i as u32, name, args)["result"]["isError"],
            true
        );
    }
    let list = success(call(&mut s, 20, "folio_list_documents", json!({})));
    assert_eq!(list["documents"].as_array().unwrap().len(), 1);
    s.stop();
    assert_eq!(std::fs::read_dir(out).unwrap().count(), 0);
}

#[test]
fn document_capacity_refusal_preserves_preview_and_close_allows_retry() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pdf");
    pdf(&source);
    let output = dir.path().join("exports");
    std::fs::create_dir(&output).unwrap();
    let mut s = Session::start_many(&vec![source.as_path(); 64], &output);
    let inspected = success(call(
        &mut s,
        1,
        "folio_inspect_text",
        json!({"documentId":"document-1","page":1}),
    ));
    let preview = success(call(
        &mut s,
        2,
        "folio_preview_text",
        json!({"documentId":"document-1","page":1,"selectionId":inspected["runs"][0]["selectionId"],"replacement":"Hello edited"}),
    ));
    let full = call(
        &mut s,
        3,
        "folio_apply_preview",
        json!({"previewId":preview["previewId"]}),
    );
    assert_eq!(
        full["result"]["structuredContent"]["error"]["code"],
        "document_limit"
    );
    assert!(Path::new(preview["after"].as_str().unwrap()).exists());
    assert_eq!(std::fs::read_dir(&output).unwrap().count(), 0);
    success(call(
        &mut s,
        4,
        "folio_close_document",
        json!({"documentId":"document-2"}),
    ));
    let added = success(call(
        &mut s,
        5,
        "folio_apply_preview",
        json!({"previewId":preview["previewId"]}),
    ));
    assert_eq!(added["documentId"], "document-65");
    let old = success(call(
        &mut s,
        6,
        "folio_inspect_text",
        json!({"documentId":"document-3","page":1}),
    ));
    assert_eq!(old, inspected);
    let edited = success(call(
        &mut s,
        7,
        "folio_inspect_text",
        json!({"documentId":"document-65","page":1}),
    ));
    assert_eq!(edited["runs"][0]["text"], "Hello edited");
    let list = success(call(&mut s, 8, "folio_list_documents", json!({})));
    assert_eq!(list["documents"].as_array().unwrap().len(), 64);
    s.stop();
    assert!(!Path::new(preview["after"].as_str().unwrap()).exists());
}

#[test]
fn text_preview_save_discard_uses_owned_ids_and_preserves_exact_candidates() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pdf");
    pdf(&source);
    let original = std::fs::read(&source).unwrap();
    let out = dir.path().join("exports");
    std::fs::create_dir(&out).unwrap();
    let mut s = Session::start(&source, &out);
    let inspected = success(call(
        &mut s,
        1,
        "folio_inspect_text",
        json!({"documentId":"document-1","page":1}),
    ));
    let args = json!({"documentId":"document-1","page":1,"selectionId":inspected["runs"][0]["selectionId"],"replacement":"Hello from edited Folio"});
    let preview = success(call(&mut s, 2, "folio_preview_text", args));
    let id = preview["previewId"].as_str().unwrap();
    assert!(id.starts_with("preview-"));
    assert!(preview.get("candidate").is_none() && preview.get("folder").is_none());
    let before = std::path::PathBuf::from(preview["before"].as_str().unwrap());
    let after = std::path::PathBuf::from(preview["after"].as_str().unwrap());
    assert!(before.is_file() && after.is_file());
    assert_ne!(
        std::fs::read(&before).unwrap(),
        std::fs::read(&after).unwrap()
    );
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 0);
    let listed = success(call(&mut s, 3, "folio_list_documents", json!({})));
    assert_eq!(listed["previews"][0]["previewId"], id);
    let saved = success(call(
        &mut s,
        4,
        "folio_save_preview",
        json!({"previewId":id}),
    ));
    let saved_path = std::path::PathBuf::from(saved["path"].as_str().unwrap());
    assert!(saved_path.starts_with(out.canonicalize().unwrap()));
    assert_eq!(saved["sha256"], preview["candidateSha256"]);
    let bytes = std::fs::read(&saved_path).unwrap();
    let again = success(call(
        &mut s,
        5,
        "folio_save_preview",
        json!({"previewId":id}),
    ));
    assert_ne!(again["path"], saved["path"]);
    assert_eq!(
        std::fs::read(again["path"].as_str().unwrap()).unwrap(),
        bytes
    );
    let dropped = success(call(
        &mut s,
        6,
        "folio_discard_preview",
        json!({"previewId":id}),
    ));
    assert_eq!(dropped["discarded"], true);
    assert!(!before.exists() && !after.exists());
    let stale = call(&mut s, 7, "folio_save_preview", json!({"previewId":id}));
    assert_eq!(stale["result"]["isError"], true);
    for (i, args) in [
        json!({"previewId":saved_path}),
        json!({"previewId":"preview-unknown","input":source}),
        json!({"previewId":"preview-unknown","output":"arbitrary.pdf"}),
    ]
    .into_iter()
    .enumerate()
    {
        let value = call(&mut s, 10 + i as u32, "folio_save_preview", args);
        assert_eq!(value["result"]["isError"], true);
    }
    s.stop();
    assert_eq!(std::fs::read(source).unwrap(), original);
    assert_eq!(std::fs::read(saved_path).unwrap(), bytes);
}

#[test]
fn retained_previews_are_bounded_recoverable_and_cleaned_on_eof() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pdf");
    pdf(&source);
    let out = dir.path().join("exports");
    std::fs::create_dir(&out).unwrap();
    let mut s = Session::start(&source, &out);
    let inspected = success(call(
        &mut s,
        1,
        "folio_inspect_text",
        json!({"documentId":"document-1","page":1}),
    ));
    let args = json!({"documentId":"document-1","page":1,"selectionId":inspected["runs"][0]["selectionId"],"replacement":"Hello from Folio"});
    let mut paths = vec![];
    for id in 2..10 {
        let preview = success(call(&mut s, id, "folio_preview_text", args.clone()));
        paths.push(std::path::PathBuf::from(preview["after"].as_str().unwrap()));
    }
    let full = call(&mut s, 10, "folio_preview_text", args.clone());
    assert_eq!(
        full["result"]["structuredContent"]["error"]["code"],
        "preview_limit"
    );
    let list = success(call(&mut s, 11, "folio_list_documents", json!({})));
    assert_eq!(list["previews"].as_array().unwrap().len(), 8);
    success(call(
        &mut s,
        12,
        "folio_discard_preview",
        json!({"previewId":list["previews"][0]["previewId"]}),
    ));
    let preview = success(call(&mut s, 13, "folio_preview_text", args));
    paths.push(std::path::PathBuf::from(preview["after"].as_str().unwrap()));
    s.stop();
    assert!(paths.iter().all(|p| !p.exists()));
    assert_eq!(std::fs::read_dir(out).unwrap().count(), 0);
}

#[test]
fn preview_arguments_and_modified_images_cannot_publish_a_copy() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pdf");
    pdf(&source);
    let out = dir.path().join("exports");
    std::fs::create_dir(&out).unwrap();
    let mut s = Session::start(&source, &out);
    let inspected = success(call(
        &mut s,
        1,
        "folio_inspect_text",
        json!({"documentId":"document-1","page":1}),
    ));
    let args = json!({"documentId":"document-1","page":1,"selectionId":inspected["runs"][0]["selectionId"],"replacement":"Hello from Folio"});
    for (i, (key, value)) in [
        ("page", json!(0)),
        ("selectionId", json!("text-v1-invalid")),
        ("replacement", json!("")),
        ("replacement", json!("é".repeat(2049))),
        ("input", json!("private.pdf")),
        ("password", json!("private")),
        ("output", json!("arbitrary.pdf")),
        ("documentId", json!("document-2")),
    ]
    .into_iter()
    .enumerate()
    {
        let mut bad = args.clone();
        bad[key] = value;
        let result = call(&mut s, 10 + i as u32, "folio_preview_text", bad);
        assert_eq!(
            result["result"]["structuredContent"]["error"]["code"], "invalid_arguments",
            "{result}"
        );
    }
    let preview = success(call(&mut s, 30, "folio_preview_text", args));
    let after = Path::new(preview["after"].as_str().unwrap());
    let bytes = std::fs::read(after).unwrap();
    let mut altered = bytes.clone();
    altered[20] ^= 1;
    std::fs::write(after, altered).unwrap();
    let result = call(
        &mut s,
        31,
        "folio_save_preview",
        json!({"previewId":preview["previewId"]}),
    );
    assert_eq!(result["result"]["isError"], true, "{result}");
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 0);
    std::fs::write(after, bytes).unwrap();
    success(call(
        &mut s,
        32,
        "folio_save_preview",
        json!({"previewId":preview["previewId"]}),
    ));
    s.stop();
    assert!(!after.exists());
    assert_eq!(std::fs::read_dir(&out).unwrap().count(), 1);
}

#[test]
fn text_inspection_uses_stable_snapshot_selections_without_export_folders() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pdf");
    pdf(&source);
    let output = dir.path().join("exports");
    std::fs::create_dir(&output).unwrap();
    let mut s = Session::start(&source, &output);
    let listed = s.request(1, "tools/list", json!({}));
    let tool = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "folio_inspect_text")
        .expect("inspection tool");
    assert_eq!(tool["annotations"]["readOnlyHint"], true);
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    let first = success(call(
        &mut s,
        2,
        "folio_inspect_text",
        json!({"documentId":"document-1","page":1}),
    ));
    let direct = Command::new(env!("CARGO_BIN_EXE_folio-cli"))
        .arg("inspect-text")
        .arg(&source)
        .args(["--page", "1", "--engine"])
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .output()
        .unwrap();
    assert!(
        direct.status.success(),
        "{}",
        String::from_utf8_lossy(&direct.stderr)
    );
    let direct: Value = serde_json::from_slice(&direct.stdout).unwrap();
    assert_eq!(first, direct["result"]);
    std::fs::write(&source, b"changed after snapshot").unwrap();
    assert_eq!(
        first,
        success(call(
            &mut s,
            3,
            "folio_inspect_text",
            json!({"documentId":"document-1","page":1,"offset":0})
        ))
    );
    assert_eq!(first["runs"][0]["text"], "Hello from Folio MCP");
    assert!(first["runs"][0]["selectionId"]
        .as_str()
        .unwrap()
        .starts_with("text-v1-"));
    for (i, args) in [
        json!({"documentId":"document-1"}),
        json!({"documentId":"document-1","page":null}),
        json!({"documentId":"document-1","page":1,"offset":null}),
        json!({"documentId":"document-1","page":1,"offset":100001}),
        json!({"documentId":"document-1","page":1,"offset":-1}),
        json!({"documentId":"document-1","page":1,"input":"secret.pdf"}),
        json!({"documentId":"document-1","page":1,"password":"secret"}),
    ]
    .into_iter()
    .enumerate()
    {
        let v = call(&mut s, 10 + i as u32, "folio_inspect_text", args);
        assert_eq!(v["result"]["isError"], true, "{v}");
        assert_eq!(
            v["result"]["structuredContent"]["error"]["code"],
            "invalid_arguments"
        );
    }
    let bad_page = call(
        &mut s,
        30,
        "folio_inspect_text",
        json!({"documentId":"document-1","page":2}),
    );
    assert_eq!(bad_page["result"]["isError"], true);
    s.stop();
    assert_eq!(std::fs::read_dir(output).unwrap().count(), 0);
    assert_eq!(std::fs::read(source).unwrap(), b"changed after snapshot");
}

#[test]
fn native_operations_use_snapshots_and_preserve_originals_and_exports() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("Notes café.pdf");
    pdf(&source);
    let original = std::fs::read(&source).unwrap();
    let output = dir.path().join("exports");
    std::fs::create_dir(&output).unwrap();
    let mut s = Session::start(&source, &output);
    success(call(&mut s, 1, "folio_list_documents", json!({})));
    // The live original is no longer a valid PDF. Only the launch snapshot is readable.
    std::fs::write(&source, b"original changed after launch").unwrap();
    for id in 2..5 {
        let info = success(call(
            &mut s,
            id,
            "folio_info",
            json!({"documentId":"document-1"}),
        ));
        assert_eq!(info["name"], "Notes café.pdf");
        assert_eq!(info["pageCount"], 1);
    }
    let text = success(call(
        &mut s,
        5,
        "folio_export_text",
        json!({"documentId":"document-1","page":1}),
    ));
    assert!(text["text"]
        .as_str()
        .unwrap()
        .contains("Hello from Folio MCP"));
    assert_eq!(text["truncated"], false);
    let text_path = std::path::PathBuf::from(text["path"].as_str().unwrap());
    assert!(text_path.starts_with(output.canonicalize().unwrap()));
    let text_bytes = std::fs::read(&text_path).unwrap();
    assert!(text_bytes.starts_with(&[0xef, 0xbb, 0xbf]));
    let rendered = success(call(
        &mut s,
        6,
        "folio_render_page",
        json!({"documentId":"document-1","page":1,"dpi":72}),
    ));
    let png = std::path::PathBuf::from(rendered["folder"].as_str().unwrap()).join("page-0001.png");
    assert_eq!(image::image_dimensions(&png).unwrap(), (300, 240));
    let failed = call(
        &mut s,
        7,
        "folio_export_text",
        json!({"documentId":"document-1","page":2}),
    );
    assert_eq!(failed["result"]["isError"], true);
    let second = success(call(
        &mut s,
        8,
        "folio_export_text",
        json!({"documentId":"document-1"}),
    ));
    assert_ne!(second["path"], text["path"]);
    s.stop();
    assert_eq!(
        std::fs::read(&source).unwrap(),
        b"original changed after launch"
    );
    assert_eq!(std::fs::read(text_path).unwrap(), text_bytes);
    assert!(png.is_file());
    assert_eq!(
        std::fs::read_dir(&output).unwrap().count(),
        3,
        "Only successful exports persist"
    );
    assert!(original.starts_with(b"%PDF"));
}

#[test]
fn tool_arguments_cannot_authorize_new_paths_or_options() {
    let (_dir, mut s) = setup();
    for (id, args) in [
        json!({"documentId":"../secret.pdf"}),
        json!({"documentId":"document-1","input":"C:/secret.pdf"}),
        json!({"documentId":"document-1","output":"C:/secret.txt"}),
        json!({"documentId":"document-01"}),
        json!({"documentId":"document-1","page":0}),
        json!({"documentId":"document-1","page":1.5}),
        json!({"documentId":"document-1","page":null}),
        json!({"documentId":"document-1","includeFormValues":"yes"}),
    ]
    .into_iter()
    .enumerate()
    {
        let v = call(&mut s, id as u32, "folio_export_text", args);
        assert_eq!(v["result"]["isError"], true, "{v}");
        assert_eq!(
            v["result"]["structuredContent"]["error"]["code"],
            "invalid_arguments"
        );
    }
    let v = call(
        &mut s,
        20,
        "folio_render_page",
        json!({"documentId":"document-1","page":1,"dpi":999}),
    );
    assert_eq!(v["result"]["isError"], true);
    s.stop();
}

#[test]
fn native_failure_does_not_poison_protocol_or_publish_artifacts() {
    let (dir, mut s) = setup();
    let before = std::fs::read_dir(dir.path()).unwrap().count();
    let v = call(
        &mut s,
        1,
        "folio_export_text",
        json!({"documentId":"document-1"}),
    );
    assert_eq!(v["result"]["isError"], true);
    assert_ne!(
        v["result"]["structuredContent"]["error"]["code"],
        "unavailable"
    );
    let v = s.request(2, "ping", json!({}));
    assert_eq!(v["result"]["resultType"], "complete");
    s.stop();
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), before);
}

#[test]
fn cancellation_and_eof_remove_only_unfinished_exports() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pdf");
    pdf(&source);
    let before = std::fs::read(&source).unwrap();
    let output = dir.path().join("out");
    std::fs::create_dir(&output).unwrap();
    std::fs::write(output.join("keep.txt"), b"keep").unwrap();
    let mut s = Session::start(&source, &output);
    success(call(&mut s, 1, "folio_list_documents", json!({})));
    // One write delivers call + cancellation together, before the child can load PDFium.
    let call = json!({"jsonrpc":"2.0","id":"cancel-me","method":"tools/call","params":{"_meta":meta(),"name":"folio_render_page","arguments":{"documentId":"document-1","page":1,"dpi":600}}});
    let cancel = json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":"cancel-me"}});
    let ping = json!({"jsonrpc":"2.0","id":2,"method":"ping","params":{"_meta":meta()}});
    s.raw(&format!("{call}\n{cancel}\n{ping}\n"));
    assert_eq!(
        s.receive()["id"],
        2,
        "Cancelled call must not publish a response"
    );
    // Repeat with EOF; session must kill/reap its owned worker and clean the stage.
    s.send(call);
    s.stop();
    assert_eq!(std::fs::read(&source).unwrap(), before);
    assert_eq!(std::fs::read(output.join("keep.txt")).unwrap(), b"keep");
    assert_eq!(std::fs::read_dir(output).unwrap().count(), 1);
}

#[test]
fn launch_rejects_invalid_configuration_without_protocol_output() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("oversized.pdf");
    let file = std::fs::File::create(&source).unwrap();
    file.set_len(512 * 1024 * 1024 + 1).unwrap();
    drop(file);
    for args in [
        vec!["mcp".into()],
        vec![
            "mcp".into(),
            "--input".into(),
            source.as_os_str().to_owned(),
            "--out-dir".into(),
            dir.path().as_os_str().to_owned(),
        ],
        vec![
            "mcp".into(),
            "--input".into(),
            dir.path().as_os_str().to_owned(),
            "--out-dir".into(),
            dir.path().as_os_str().to_owned(),
        ],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_folio-cli"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&result.stderr).contains("oversized.pdf"));
    }
}

#[path = "support/protected_fixture.rs"]
mod protected_fixture;
#[test]
fn protected_inputs_report_password_requirement_without_outputs() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("protected.pdf");
    pdf(&source);
    let encrypted = protected_fixture::encrypt(
        &std::fs::read(&source).unwrap(),
        lopdf::encryption::Permissions::PRINTABLE,
    );
    std::fs::write(&source, &encrypted).unwrap();
    let output = dir.path().join("out");
    std::fs::create_dir(&output).unwrap();
    let mut s = Session::start(&source, &output);
    let v = call(
        &mut s,
        1,
        "folio_export_text",
        json!({"documentId":"document-1"}),
    );
    assert_eq!(
        v["result"]["structuredContent"]["error"]["code"],
        "password_required"
    );
    assert!(!v.to_string().contains("folio-mcp-inputs"));
    s.stop();
    assert_eq!(std::fs::read(source).unwrap(), encrypted);
    assert_eq!(std::fs::read_dir(output).unwrap().count(), 0);
}

#[test]
fn explicit_modern_initialize_cannot_switch_to_legacy() {
    for (version, capabilities, expected) in [
        ("2099-01-01", json!({}), -32022),
        ("2026-07-28", json!("invalid"), -32602),
        ("2026-07-28", json!({}), -32601),
    ] {
        let (_dir, mut s) = setup();
        let params = json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"},
            "_meta":{"io.modelcontextprotocol/protocolVersion":version,"io.modelcontextprotocol/clientCapabilities":capabilities}});
        s.send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":params}));
        assert_eq!(s.receive()["error"]["code"], expected);
        s.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        s.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}));
        assert_eq!(
            s.receive()["error"]["code"],
            -32602,
            "Rejected initialize cannot enable legacy tools"
        );
        s.stop();
    }
}

#[test]
fn modern_cacheable_results_declare_private_cache_policy() {
    let (_dir, mut s) = setup();
    for (id, method) in [(1, "server/discover"), (2, "tools/list")] {
        let v = s.request(id, method, json!({}));
        assert_eq!(v["result"]["ttlMs"], 0, "{method}");
        assert_eq!(v["result"]["cacheScope"], "private", "{method}");
    }
    s.stop();
}

#[test]
fn legacy_pings_work_throughout_handshake_without_enabling_tools() {
    for version in ["2025-11-25", "2025-06-18", "2024-11-05"] {
        let (_dir, mut s) = setup();
        s.send(json!({"jsonrpc":"2.0","id":1,"method":"ping"}));
        assert_eq!(s.receive()["result"], json!({}));
        s.send(json!({"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":version,"capabilities":{},"clientInfo":{"name":"test","version":"1"}}}));
        assert_eq!(s.receive()["result"]["protocolVersion"], version);
        s.send(json!({"jsonrpc":"2.0","id":3,"method":"ping"}));
        assert_eq!(s.receive()["result"], json!({}));
        s.send(json!({"jsonrpc":"2.0","id":4,"method":"tools/list"}));
        assert_eq!(s.receive()["error"]["code"], -32602);
        s.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        s.send(json!({"jsonrpc":"2.0","id":5,"method":"ping"}));
        assert_eq!(s.receive()["result"], json!({}));
        s.stop();
    }
}

#[test]
fn stalled_stdout_cannot_prevent_eof_or_worker_cleanup() {
    for requests in [4, 40] {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.pdf");
        pdf(&source);
        let output = dir.path().join("out");
        std::fs::create_dir(&output).unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_folio-cli"))
            .args(["mcp", "--input"])
            .arg(&source)
            .arg("--out-dir")
            .arg(&output)
            .arg("--engine")
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut input = child.stdin.take().unwrap();
        // Intentionally leave stdout undrained. This thread must not keep the test
        // itself blocked if the old dispatcher stops reading its bounded channel.
        let writer = std::thread::spawn(move || {
            let operation = json!({"jsonrpc":"2.0","id":"native","method":"tools/call","params":{"_meta":meta(),"name":"folio_render_page","arguments":{"documentId":"document-1","page":1,"dpi":600}}});
            let _ = writeln!(input, "{operation}");
            for id in 0..requests {
                let request = json!({"jsonrpc":"2.0","id":id,"method":"tools/list","params":{"_meta":meta()}});
                if writeln!(input, "{request}").is_err() {
                    break;
                }
            }
        });
        let mut exited = false;
        for _ in 0..160 {
            if child.try_wait().unwrap().is_some() {
                exited = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        if !exited {
            let _ = child.kill();
        }
        let _ = child.wait();
        let _ = writer.join();
        assert!(
            exited,
            "MCP must supervise EOF even when stdout is blocked (requests={requests})"
        );
        assert_eq!(
            std::fs::read_dir(&output).unwrap().count(),
            0,
            "Unfinished operation must be cleaned up"
        );
    }
}

#[cfg(windows)]
#[test]
fn drive_relative_engine_is_resolved_at_launch() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pdf");
    pdf(&source);
    let cwd = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cwd_text = cwd.to_str().unwrap();
    let relative = format!("{}resources\\pdfium\\pdfium.dll", &cwd_text[..2]);
    let mut child = Command::new(env!("CARGO_BIN_EXE_folio-cli"))
        .args(["mcp", "--input"])
        .arg(source)
        .arg("--out-dir")
        .arg(dir.path())
        .arg("--engine")
        .arg(relative)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let request = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"_meta":meta(),"name":"folio_info","arguments":{"documentId":"document-1"}}});
    writeln!(child.stdin.as_mut().unwrap(), "{request}").unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    drop(child.stdin.take());
    let status = child.wait().unwrap();
    assert!(status.success());
    let response: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(response["result"]["isError"], false, "{response}");
}

#[cfg(windows)]
#[test]
fn terminated_discovery_process_leaves_no_snapshot_payload() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.pdf");
    pdf(&source);
    let before = std::fs::read(&source).unwrap();
    let tmp = dir.path().join("private-temp");
    std::fs::create_dir(&tmp).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_folio-cli"))
        .args(["mcp", "--input"])
        .arg(&source)
        .arg("--out-dir")
        .arg(dir.path())
        .env("TEMP", &tmp)
        .env("TMP", &tmp)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let request =
        json!({"jsonrpc":"2.0","id":1,"method":"server/discover","params":{"_meta":meta()}});
    writeln!(child.stdin.as_mut().unwrap(), "{request}").unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let discovery: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(discovery["result"]["resultType"], "complete");
    assert!(
        std::fs::read_dir(&tmp).unwrap().count() > 0,
        "Snapshot must exist during discovery"
    );
    child.kill().unwrap();
    child.wait().unwrap();
    assert_eq!(
        std::fs::read_dir(&tmp).unwrap().count(),
        0,
        "OS must remove snapshot after abrupt probe termination"
    );
    assert_eq!(std::fs::read(source).unwrap(), before);
}


#[test]
fn paragraph_inspection_is_read_only_stable_and_strict() {
    let dir=tempfile::tempdir().unwrap();
    let source=dir.path().join("source.pdf");pdf(&source);
    let output=dir.path().join("exports");std::fs::create_dir(&output).unwrap();
    let mut s=Session::start(&source,&output);
    let tools=s.request(1,"tools/list",json!({}));
    let tool=tools["result"]["tools"].as_array().unwrap().iter().find(|t|t["name"]=="folio_inspect_paragraph").expect("paragraph inspection tool");
    assert_eq!(tool["annotations"]["readOnlyHint"],true);
    assert_eq!(tool["inputSchema"]["additionalProperties"],false);
    let text=success(call(&mut s,2,"folio_inspect_text",json!({"documentId":"document-1","page":1})));
    let seed=text["runs"][0]["selectionId"].as_str().unwrap();
    let args=json!({"documentId":"document-1","page":1,"selectionId":seed});
    let result=success(call(&mut s,3,"folio_inspect_paragraph",args.clone()));
    assert_eq!(result["paragraph"]["text"],"Hello from Folio MCP");
    let direct=Command::new(env!("CARGO_BIN_EXE_folio-cli")).arg("inspect-paragraph").arg(&source).args(["--page","1","--selection-id",seed,"--engine"]).arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).output().unwrap();
    assert!(direct.status.success(),"{}",String::from_utf8_lossy(&direct.stderr));
    assert_eq!(result,serde_json::from_slice::<Value>(&direct.stdout).unwrap()["result"]);
    std::fs::write(&source,b"source changed after launch snapshot").unwrap();
    assert_eq!(result,success(call(&mut s,4,"folio_inspect_paragraph",args.clone())));
    for (i,bad) in [json!({"documentId":"document-1","page":1}),json!({"documentId":"document-1","page":null,"selectionId":seed}),json!({"documentId":"document-1","page":1,"selectionId":"wrong"}),json!({"documentId":"document-1","page":1,"selectionId":seed,"input":"secret.pdf"}),json!({"documentId":"document-1","page":1,"selectionId":seed,"offset":0}),json!({"documentId":"document-9","page":1,"selectionId":seed})].into_iter().enumerate() {
        let value=call(&mut s,10+i as u32,"folio_inspect_paragraph",bad);
        assert_eq!(value["result"]["structuredContent"]["error"]["code"],"invalid_arguments","{value}");
    }
    let bad=call(&mut s,20,"folio_inspect_paragraph",json!({"documentId":"document-1","page":2,"selectionId":seed}));
    assert_eq!(bad["result"]["isError"],true);
    s.stop();assert_eq!(std::fs::read_dir(output).unwrap().count(),0);
    assert_eq!(std::fs::read(source).unwrap(),b"source changed after launch snapshot");
}


#[test]
fn paragraph_previews_apply_reedit_and_save_without_restarting() {
 let dir=tempfile::tempdir().unwrap();let source=dir.path().join("source.pdf");pdf(&source);let original=std::fs::read(&source).unwrap();
 let output=dir.path().join("exports");std::fs::create_dir(&output).unwrap();let mut s=Session::start(&source,&output);
 let tools=s.request(1,"tools/list",json!({}));
 let tool=tools["result"]["tools"].as_array().unwrap().iter().find(|t|t["name"]=="folio_preview_paragraph").expect("paragraph preview tool");
 assert_eq!(tool["annotations"]["readOnlyHint"],false);assert_eq!(tool["inputSchema"]["additionalProperties"],false);
 let text=success(call(&mut s,2,"folio_inspect_text",json!({"documentId":"document-1","page":1})));
 let selected=success(call(&mut s,3,"folio_inspect_paragraph",json!({"documentId":"document-1","page":1,"selectionId":text["runs"][0]["selectionId"]})));
 let replacement="Hello from Folio MCP with several extra words in a wrapping paragraph.";
 let mut bounds=selected["paragraph"]["bounds"].clone();bounds["width"]=json!(110.);bounds["height"]=json!(100.);
 let args=json!({"documentId":"document-1","page":1,"seedSelectionId":selected["seedSelectionId"],"paragraphId":selected["paragraphId"],"runs":[{"text":replacement,"styleId":0}],"bounds":bounds});
 for (i,field) in ["bounds","alignment","lineSpacing","fit"].into_iter().enumerate() {
  let mut bad=args.clone();bad[field]=Value::Null;
  assert_eq!(call(&mut s,10+i as u32,"folio_preview_paragraph",bad)["result"]["isError"],true);
 }
 let mut bad=args.clone();bad["runs"][0]["fontId"]=json!("untrusted");assert_eq!(call(&mut s,15,"folio_preview_paragraph",bad)["result"]["isError"],true);
 let first=success(call(&mut s,20,"folio_preview_paragraph",args.clone()));assert_eq!(first["paragraphId"],selected["paragraphId"]);assert_eq!(first["paragraph"]["bounds"],bounds);
 assert_eq!(std::fs::read_dir(&output).unwrap().count(),0);
 let applied=success(call(&mut s,21,"folio_apply_preview",json!({"previewId":first["previewId"]})));
 let text=success(call(&mut s,22,"folio_inspect_text",json!({"documentId":applied["documentId"],"page":1})));assert!(text["runs"].as_array().unwrap().len()>=3);
 let next=success(call(&mut s,23,"folio_inspect_paragraph",json!({"documentId":applied["documentId"],"page":1,"selectionId":text["runs"][0]["selectionId"]})));
 assert_eq!(next["paragraph"]["text"],replacement);
 let mut stale=args;stale["documentId"]=applied["documentId"].clone();assert_eq!(call(&mut s,24,"folio_preview_paragraph",stale)["result"]["isError"],true);
 let revised=replacement.replace("Hello","Great");
 let second=success(call(&mut s,25,"folio_preview_paragraph",json!({"documentId":applied["documentId"],"page":1,"seedSelectionId":next["seedSelectionId"],"paragraphId":next["paragraphId"],"runs":[{"text":revised,"styleId":0}]})));
 let saved=success(call(&mut s,26,"folio_save_preview",json!({"previewId":second["previewId"]})));assert_eq!(saved["sha256"],second["candidateSha256"]);
 let final_doc=success(call(&mut s,27,"folio_apply_preview",json!({"previewId":second["previewId"]})));
 success(call(&mut s,28,"folio_discard_preview",json!({"previewId":first["previewId"]})));
 success(call(&mut s,29,"folio_close_document",json!({"documentId":applied["documentId"]})));
 let text=success(call(&mut s,30,"folio_inspect_text",json!({"documentId":final_doc["documentId"],"page":1})));
 assert!(text["runs"][0]["text"].as_str().unwrap().starts_with("Great"));
 s.stop();assert!(Path::new(saved["path"].as_str().unwrap()).exists());assert!(!Path::new(second["before"].as_str().unwrap()).exists());
 assert_eq!(std::fs::read(source).unwrap(),original);
}

#[path = "support/nested_automation.rs"]
mod nested_automation;

#[test]
fn nested_paragraph_apply_promotion_reedit_preserves_prior_document_in_both_protocols() {
    for legacy in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.pdf");
        nested_automation::write_fixture(&source, nested_automation::Mode::RepeatedInnerFar, 1.);
        let original = std::fs::read(&source).unwrap();
        let output = dir.path().join("out");
        std::fs::create_dir(&output).unwrap();
        let mut s = Session::start(&source, &output);
        if legacy {
            s.send(json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}));
            assert_eq!(s.receive()["result"]["protocolVersion"], "2025-11-25");
            s.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        }
        let invoke = |s: &mut Session, id: u32, name: &str, args: Value| {
            if legacy {
                s.send(json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}}));
                let response = s.receive();
                assert_eq!(response["id"], id);
                assert!(response["result"].get("resultType").is_none());
                response
            } else { call(s, id, name, args) }
        };
        let listed = success(invoke(&mut s, 1, "folio_inspect_nested_text", json!({"documentId":"document-1","page":1})));
        let seed = listed["runs"].as_array().unwrap().iter().find(|run| run["bounds"]["x"].as_f64().unwrap() > 300. && run["text"] == "Nested first line").unwrap();
        let selected = success(invoke(&mut s, 2, "folio_inspect_paragraph", json!({"documentId":"document-1","page":1,"selectionId":seed["selectionId"]})));
        let replacement = format!("{} this added text wraps into another line", selected["paragraph"]["text"].as_str().unwrap());
        let args = json!({"documentId":"document-1","page":1,"seedSelectionId":selected["seedSelectionId"],"paragraphId":selected["paragraphId"],"runs":[{"text":replacement,"styleId":0}],"fit":true});
        let mut bad = args.clone(); bad["occurrenceScope"] = selected["occurrenceScope"].clone();
        assert_eq!(invoke(&mut s, 3, "folio_preview_paragraph", bad)["result"]["isError"], true);
        let first = success(invoke(&mut s, 4, "folio_preview_paragraph", args.clone()));
        assert_eq!(first["paragraph"]["occurrenceScope"], selected["occurrenceScope"]);
        let applied = success(invoke(&mut s, 5, "folio_apply_preview", json!({"previewId":first["previewId"]})));
        let nested = success(invoke(&mut s, 6, "folio_inspect_nested_text", json!({"documentId":applied["documentId"],"page":1})));
        assert_eq!(nested["runs"].as_array().unwrap().len(), 4);
        let page = success(invoke(&mut s, 7, "folio_inspect_text", json!({"documentId":applied["documentId"],"page":1})));
        let seed = page["runs"].as_array().unwrap().iter().find(|r| r["bounds"]["x"].as_f64().unwrap() > 300. && r["text"].as_str().unwrap().starts_with("Nested first")).unwrap();
        let next = success(invoke(&mut s, 8, "folio_inspect_paragraph", json!({"documentId":applied["documentId"],"page":1,"selectionId":seed["selectionId"]})));
        assert_eq!(next["paragraph"]["text"], replacement);
        assert!(next.get("occurrenceScope").is_none());
        let mut stale = args; stale["documentId"] = applied["documentId"].clone();
        assert_eq!(invoke(&mut s, 9, "folio_preview_paragraph", stale)["result"]["isError"], true);
        let revised = replacement.replacen("Nested", "Edited", 1);
        let second = success(invoke(&mut s, 10, "folio_preview_paragraph", json!({"documentId":applied["documentId"],"page":1,"seedSelectionId":next["seedSelectionId"],"paragraphId":next["paragraphId"],"runs":[{"text":revised,"styleId":0}],"fit":true})));
        let saved = success(invoke(&mut s, 11, "folio_save_preview", json!({"previewId":second["previewId"]})));
        assert_eq!(saved["sha256"], second["candidateSha256"]);
        use sha2::{Digest, Sha256};
        let bytes = std::fs::read(saved["path"].as_str().unwrap()).unwrap();
        let digest: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(saved["sha256"], digest);
        let engine = folio_engine::PdfEngine::start(Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
        let document = engine.open_document(saved["path"].as_str().unwrap()).unwrap();
        let text: String = engine.page_text(&document.id, 0).unwrap().characters.into_iter().map(|c| c.text).collect();
        engine.close_document(&document.id).unwrap();
        assert_eq!(text.matches("Edited first").count(), 1);
        assert!(text.contains("this added text"));
        assert_eq!(text.matches("Nested first line").count(), 3);
        assert_eq!(success(invoke(&mut s, 12, "folio_inspect_nested_text", json!({"documentId":"document-1","page":1}))), listed);
        success(invoke(&mut s, 13, "folio_discard_preview", json!({"previewId":first["previewId"]})));
        success(invoke(&mut s, 14, "folio_close_document", json!({"documentId":applied["documentId"]})));
        s.stop();
        assert!(!Path::new(first["before"].as_str().unwrap()).exists());
        assert!(!Path::new(second["before"].as_str().unwrap()).exists());
        assert_eq!(std::fs::read(saved["path"].as_str().unwrap()).unwrap(), bytes);
        assert_eq!(std::fs::read(&source).unwrap(), original);
    }
}

#[test]
fn nested_text_apply_reinspect_save_keeps_prior_document() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("nested.pdf");
    nested_automation::write_fixture(&source, nested_automation::Mode::RepeatedOuter, 1.);
    let original = std::fs::read(&source).unwrap();
    let output = dir.path().join("out");
    std::fs::create_dir(&output).unwrap();
    let mut s = Session::start(&source, &output);
    let first = success(call(
        &mut s,
        1,
        "folio_inspect_nested_text",
        json!({"documentId":"document-1","page":1}),
    ));
    assert_eq!(first["totalRuns"], 4);
    assert_eq!(
        call(
            &mut s,
            2,
            "folio_inspect_nested_text",
            json!({"documentId":"document-1","page":1,"objectPath":[0,0]})
        )["result"]["isError"],
        true
    );
    let preview = success(call(
        &mut s,
        3,
        "folio_preview_text",
        json!({"documentId":"document-1","page":1,"selectionId":first["runs"][2]["selectionId"],"replacement":"Edited first line"}),
    ));
    let before = preview["before"].as_str().unwrap().to_string();
    let applied = success(call(
        &mut s,
        4,
        "folio_apply_preview",
        json!({"previewId":preview["previewId"]}),
    ));
    let next = success(call(
        &mut s,
        5,
        "folio_inspect_nested_text",
        json!({"documentId":applied["documentId"],"page":1}),
    ));
    assert_eq!(next["runs"][2]["text"], "Edited first line");
    for i in [0, 1, 3] {
        assert_eq!(next["runs"][i]["text"], first["runs"][i]["text"]);
    }
    let stale = call(
        &mut s,
        6,
        "folio_preview_text",
        json!({"documentId":applied["documentId"],"page":1,"selectionId":first["runs"][2]["selectionId"],"replacement":"Stale"}),
    );
    assert_eq!(stale["result"]["isError"], true);
    let revised = success(call(
        &mut s,
        7,
        "folio_preview_text",
        json!({"documentId":applied["documentId"],"page":1,"selectionId":next["runs"][2]["selectionId"],"replacement":"Third first line"}),
    ));
    let saved = success(call(
        &mut s,
        8,
        "folio_save_preview",
        json!({"previewId":revised["previewId"]}),
    ));
    assert_eq!(saved["sha256"], revised["candidateSha256"]);
    let saved_path = Path::new(saved["path"].as_str().unwrap());
    use sha2::{Digest, Sha256};
    assert_eq!(
        Sha256::digest(std::fs::read(saved_path).unwrap())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        saved["sha256"].as_str().unwrap()
    );
    assert_eq!(
        success(call(
            &mut s,
            9,
            "folio_inspect_nested_text",
            json!({"documentId":"document-1","page":1})
        )),
        first
    );
    for page in [2, 3] {
        let prior = success(call(
            &mut s,
            10 + page * 2,
            "folio_inspect_nested_text",
            json!({"documentId":"document-1","page":page}),
        ));
        let current = success(call(
            &mut s,
            11 + page * 2,
            "folio_inspect_nested_text",
            json!({"documentId":applied["documentId"],"page":page}),
        ));
        for i in 0..2 {
            assert_eq!(prior["runs"][i]["text"], current["runs"][i]["text"]);
            assert_eq!(prior["runs"][i]["bounds"], current["runs"][i]["bounds"]);
        }
    }
    success(call(
        &mut s,
        20,
        "folio_discard_preview",
        json!({"previewId":preview["previewId"]}),
    ));
    assert!(!Path::new(&before).exists());
    success(call(
        &mut s,
        21,
        "folio_close_document",
        json!({"documentId":applied["documentId"]}),
    ));
    assert_eq!(
        call(
            &mut s,
            22,
            "folio_inspect_nested_text",
            json!({"documentId":applied["documentId"],"page":1})
        )["result"]["isError"],
        true
    );
    let after = revised["after"].as_str().unwrap().to_string();
    s.stop();
    assert!(!Path::new(&after).exists());
    assert!(saved_path.exists());
    assert_eq!(std::fs::read(&source).unwrap(), original);
}

fn explicit_single_run_font_session(legacy:bool) {
    for nested in [false,true] {
        let dir=tempfile::tempdir().unwrap();let source=dir.path().join("source.pdf");
        if nested{nested_automation::write_fixture(&source,nested_automation::Mode::RepeatedInnerFar,1.);}else{pdf(&source);}
        let original=std::fs::read(&source).unwrap();let output=dir.path().join("out");std::fs::create_dir(&output).unwrap();
        let font=dir.path().join("selected.ttf");std::fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf"),&font).unwrap();
        let mut s=Session::start_with_fonts(&[&source],&output,&[&font]);
        if legacy{s.send(json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"single-font-test","version":"1"}}}));assert_eq!(s.receive()["result"]["protocolVersion"],"2025-11-25");s.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));}
        let invoke=|s:&mut Session,id:u32,name:&str,args:Value|{if legacy{s.send(json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":args}}));let r=s.receive();assert_eq!(r["id"],id);r}else{call(s,id,name,args)}};
        let catalog=success(invoke(&mut s,1,"folio_list_fonts",json!({})));let choice=&catalog["fonts"][0];
        std::fs::write(&font,b"changed external font").unwrap();std::fs::remove_file(&font).unwrap();
        let route=if nested{"folio_inspect_nested_text"}else{"folio_inspect_text"};
        let listed=success(invoke(&mut s,2,route,json!({"documentId":"document-1","page":1})));
        let index=if nested{listed["runs"].as_array().unwrap().iter().position(|r|r["bounds"]["x"].as_f64().unwrap()>300. && r["text"]=="Nested first line").unwrap()}else{0};let selected=&listed["runs"][index];
        let args=json!({"documentId":"document-1","page":1,"selectionId":selected["selectionId"],"replacement":selected["text"],"fontId":choice["fontId"]});
        for (i,bad) in [("fontId",json!("a".repeat(64))),("fontPath",json!("arbitrary.ttf")),("fontId",Value::Null)]{
            let mut invalid=args.clone();invalid[i]=bad;assert_eq!(invoke(&mut s,3,"folio_preview_text",invalid)["result"]["isError"],true);
        }
        let preview=success(invoke(&mut s,4,"folio_preview_text",args));assert_eq!(preview["substituteFont"],*choice);
        let applied=success(invoke(&mut s,5,"folio_apply_preview",json!({"previewId":preview["previewId"]})));
        let next=success(invoke(&mut s,6,route,json!({"documentId":applied["documentId"],"page":1})));
        assert_eq!(next["runs"][index]["text"],selected["text"]);assert_eq!(next["runs"][index]["fontDisplayName"],"DejaVu Serif");assert_ne!(next["runs"][index]["fontName"],selected["fontName"]);
        for (i,run) in listed["runs"].as_array().unwrap().iter().enumerate(){if i!=index{assert_eq!(next["runs"][i]["text"],run["text"]);assert_eq!(next["runs"][i]["bounds"],run["bounds"]);}}
        let revised=success(invoke(&mut s,7,"folio_preview_text",json!({"documentId":applied["documentId"],"page":1,"selectionId":next["runs"][index]["selectionId"],"replacement":"Changed text"})));
        assert!(revised.get("substituteFont").is_none());
        let saved=success(invoke(&mut s,8,"folio_save_preview",json!({"previewId":revised["previewId"]})));assert_eq!(saved["sha256"],revised["candidateSha256"]);
        assert_eq!(success(invoke(&mut s,9,route,json!({"documentId":"document-1","page":1}))),listed);
        success(invoke(&mut s,10,"folio_close_document",json!({"documentId":applied["documentId"]})));
        s.stop();assert_eq!(std::fs::read(&source).unwrap(),original);assert!(Path::new(saved["path"].as_str().unwrap()).exists());
        let engine=folio_engine::PdfEngine::start(Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
        let opened=engine.open_document(saved["path"].as_str().unwrap()).unwrap();assert!(engine.extract_text(&opened.id,0).unwrap().contains("Changed text"));
        engine.close_document(&opened.id).unwrap();
    }
}
#[test]
fn explicit_single_run_font_modern_snapshot_apply_reedit_save(){explicit_single_run_font_session(false);}
#[test]
fn explicit_single_run_font_legacy_snapshot_apply_reedit_save(){explicit_single_run_font_session(true);}


#[test]
fn paragraph_baseline_controls_reach_saved_mcp_content_and_reject_malformed_points() {
    let dir=tempfile::tempdir().unwrap(); let source=dir.path().join("source.pdf");pdf(&source);
    let original=std::fs::read(&source).unwrap(); let output=dir.path().join("exports");
    std::fs::create_dir(&output).unwrap(); let mut s=Session::start(&source,&output);
    let tools=s.request(1,"tools/list",json!({}));
    let tool=tools["result"]["tools"].as_array().unwrap().iter().find(|t|t["name"]=="folio_preview_paragraph").unwrap();
    assert_eq!(tool["inputSchema"]["properties"]["baselineAnchor"]["additionalProperties"],false);
    let text=success(call(&mut s,2,"folio_inspect_text",json!({"documentId":"document-1","page":1})));
    let selected=success(call(&mut s,3,"folio_inspect_paragraph",json!({"documentId":"document-1","page":1,"selectionId":text["runs"][0]["selectionId"]})));
    let mut args=json!({"documentId":"document-1","page":1,"seedSelectionId":selected["seedSelectionId"],"paragraphId":selected["paragraphId"],"runs":selected["paragraph"]["runs"],"bounds":selected["paragraph"]["bounds"]});
    for (i,bad) in [Value::Null,json!({"x":0}),json!({"x":0,"y":0,"extra":true}),json!({"x":0,"y":1_000_001})].into_iter().enumerate() {
        args["baselineAnchor"]=bad;
        assert_eq!(call(&mut s,10+i as u32,"folio_preview_paragraph",args.clone())["result"]["isError"],true);
    }
    assert_eq!(std::fs::read_dir(&output).unwrap().count(),0);
    args["baselineAnchor"]=selected["paragraph"]["baselineAnchor"].clone();
    let baseline=selected["paragraph"]["baselineAnchor"]["y"].as_f64().unwrap()+10.;
    args["baselineAnchor"]["y"]=json!(baseline);
    args["bounds"]["y"]=json!(args["bounds"]["y"].as_f64().unwrap()+10.);
    let first=success(call(&mut s,20,"folio_preview_paragraph",args));
    let applied=success(call(&mut s,21,"folio_apply_preview",json!({"previewId":first["previewId"]})));
    let text=success(call(&mut s,22,"folio_inspect_text",json!({"documentId":applied["documentId"],"page":1})));
    let after=success(call(&mut s,23,"folio_inspect_paragraph",json!({"documentId":applied["documentId"],"page":1,"selectionId":text["runs"][0]["selectionId"]})));
    assert!((after["paragraph"]["baselineAnchor"]["y"].as_f64().unwrap()-baseline).abs()<0.0001);
    let saved=success(call(&mut s,24,"folio_save_preview",json!({"previewId":first["previewId"]})));
    assert_eq!(saved["sha256"],first["candidateSha256"]);
    s.stop(); assert_eq!(std::fs::read(source).unwrap(),original);
}
