//! Console transport for the shared one-shot automation API.
use crate::{
    automation::{self, Request},
    private_storage::Sensitive,
    protection::ProtectionError,
    EngineError, PdfEngine,
};
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    ffi::OsString,
    io::{Read, Write},
    path::PathBuf,
};

const HELP:&str="Folio CLI — local PDF automation\n\n\
folio-cli info INPUT.pdf [options]\n\
folio-cli plan-replacement INPUT.pdf --request INTENT.json [options]\n\
folio-cli preview-replacement INPUT.pdf --request INTENT.json --out-dir EXISTING_DIR [options]\n\
folio-cli export-pages INPUT.pdf --out NEW.pdf [--request PAGES.json] [options]\n\
folio-cli compose-pages --input PDF [--input PDF ...] --out NEW.pdf [--request PLAN.json] [--passwords-stdin] [--engine PATH]\n\
folio-cli fonts --font FONT [--font FONT ...]\n\
folio-cli inspect-forms INPUT.pdf --page N [--offset N] [options]\n\
folio-cli preview-form INPUT.pdf --request EDIT.json --out-dir EXISTING_DIR [options]\n\
folio-cli inspect-text INPUT.pdf --page N [--offset N] [options]\n\
folio-cli inspect-nested-text INPUT.pdf --page N [--offset N] [options]\n\
folio-cli inspect-paragraph INPUT.pdf --page N --selection-id TEXT_SELECTION [options]\n\
folio-cli preview-text INPUT.pdf --request EDIT.json --out-dir EXISTING_DIR [--font FONT ...] [options]\n\
folio-cli preview-text-batch INPUT.pdf --request EDITS.json --out-dir EXISTING_DIR [--font FONT ...] [options]\n\
folio-cli preview-paragraph INPUT.pdf --request EDIT.json --out-dir EXISTING_DIR [--font FONT ...] [options]\n\
folio-cli save-text-preview BUNDLE_DIR --manifest-sha256 HASH --out NEW.pdf [--engine PATH]\n\
folio-cli text INPUT.pdf --out NEW.txt [--page N] [--include-form-values] [options]\n\
folio-cli render INPUT.pdf --out-dir EXISTING_DIR [--page N] [--dpi 72|150|300|600] [options]\n\n\
folio-cli mcp --input PDF [--input PDF ...] --out-dir EXISTING_DIR [--engine PATH] [--font FONT ...] [--credentials FILE]\n\n\
Options: --engine PATH, --password-stdin\n\
--help   Show help without loading the PDF engine.\n\
--version   Show version.\n\n\
Pages are 1-based; text/render omit --page for all pages. PNG defaults to 150 DPI and a new child folder.\n\
Inspection requires --page; --offset defaults to 0 and returns up to 100 text runs.\n\
Text uses the desktop UTF-8 export, with a BOM. Existing files are never overwritten.\n\
export-pages omits --request for an exact copy. Request example: {\"version\":1,\"pages\":[{\"page\":1,\"rotation\":90}]}.\n\
Passwords are read as one UTF-8 line from stdin only when requested (maximum 1024 bytes).\n\
Success JSON goes to stdout; error JSON to stderr. Exit codes: 0 success, 2 arguments, 1 operation.\n\
Without --engine, PDFium is loaded from resources/pdfium beside this executable.\n";
struct Invocation {
    request: Request,
    engine: Option<PathBuf>,
    password: bool,
    fonts: Vec<PathBuf>,
}
struct ComposeInvocation{request:automation::composition::ComposeRequest,engine:Option<PathBuf>,passwords:bool}
enum Action {
    Help,
    Version,
    Fonts(Vec<PathBuf>),
    Execute(Invocation),
    Compose(ComposeInvocation),
}
#[cfg(test)]
mod batch_preview_cli_tests{
    use super::*;
    #[test]
    fn batch_preview_cli_accepts_request_directory_fonts_and_password(){
        let args=["preview-text-batch","Input.pdf","--request","edits.json","--out-dir","Output","--font","Font.ttf","--password-stdin"];
        let Action::Execute(invocation)=parse(args.into_iter().map(OsString::from).collect()).unwrap()else{panic!("Expected batch invocation")};
        assert_eq!(invocation.request.command(),"preview-text-batch");assert_eq!(invocation.fonts,vec![PathBuf::from("Font.ttf")]);assert!(invocation.password);
        for args in [vec!["preview-text-batch","Input.pdf","--out-dir","Output"],vec!["preview-text-batch","Input.pdf","--request","edit.json"],vec!["preview-text-batch","Input.pdf","--request","edit.json","--out-dir","Output","--page","1"]]{assert!(parse(args.into_iter().map(OsString::from).collect()).is_err());}
    }
}
fn parse_compose(args:&[OsString])->Result<Action,&'static str>{
    let(mut inputs,mut engine,mut request_file,mut output,mut passwords)=(Vec::new(),None,None,None,false);
    let mut seen=HashSet::new();let mut i=1;
    while i<args.len(){
        let flag=args[i].to_str().ok_or("Composition options must be valid UTF-8.")?;
        if flag!="--input"&&!seen.insert(flag){return Err("An option was repeated.");}
        if flag=="--passwords-stdin"{passwords=true;i+=1;continue;}
        if !matches!(flag,"--input"|"--out"|"--request"|"--engine"){return Err("Unknown composition option. See --help.");}
        let value=args.get(i+1).filter(|s|!s.is_empty()&&!s.to_string_lossy().starts_with("--")).ok_or("An option requires a value.")?;
        match flag{
            "--input"=>{if inputs.len()>=64{return Err("Supply 1 to 64 composition inputs.");}inputs.push(PathBuf::from(value));},
            "--out"=>output=Some(PathBuf::from(value)),"--request"=>request_file=Some(PathBuf::from(value)),"--engine"=>engine=Some(PathBuf::from(value)),_=>unreachable!(),
        }i+=2;
    }
    if inputs.is_empty(){return Err("Supply 1 to 64 composition inputs with --input.");}
    Ok(Action::Compose(ComposeInvocation{request:automation::composition::ComposeRequest{inputs,request_file,output:output.ok_or("Composition requires --out NEW.pdf.")?},engine,passwords}))
}
fn parse(args: Vec<OsString>) -> Result<Action, &'static str> {
    if args.len() == 1 && args[0] == "--version" {
        return Ok(Action::Version);
    }
    if (args.len() == 1 && args[0] == "--help")
        || (args.len() == 2
            && matches!(args[0].to_str(), Some("plan-replacement" | "preview-replacement" | "fonts" | "info" | "export-pages" | "compose-pages" | "text" | "render" | "inspect-nested-text" | "inspect-text" | "inspect-paragraph" | "preview-text" | "preview-text-batch" | "preview-paragraph" | "preview-form" | "inspect-forms" | "save-text-preview"))
            && args[1] == "--help")
    {
        return Ok(Action::Help);
    }
    let command = args
        .first()
        .and_then(|s| s.to_str())
        .ok_or("Choose info, inspect-text, text or render. See --help.")?;
    if command=="fonts" {
        if args.len()<3 || (args.len()-1)%2!=0 {return Err("Supply fonts with --font PATH. See fonts --help.");}
        let mut paths=Vec::new();
        for pair in args[1..].chunks_exact(2) {
            if pair[0]!="--font" || pair[1].is_empty() || paths.len()>=automation::font_inputs::MAX_FILES {
                return Err("Supply 1 to16 font files with repeated --font PATH.");
            }
            paths.push(PathBuf::from(&pair[1]));
        }
        return Ok(Action::Fonts(paths));
    }
    if command=="compose-pages"{return parse_compose(&args);}
    if !matches!(command, "plan-replacement" | "preview-replacement" | "info" | "export-pages" | "text" | "render" | "inspect-nested-text" | "inspect-text" | "inspect-paragraph" | "preview-text" | "preview-text-batch" | "preview-paragraph" | "preview-form" | "inspect-forms" | "save-text-preview") {
        return Err("Unknown command. See --help.");
    }
    let input = args
        .get(1)
        .filter(|s| !s.is_empty() && !s.to_string_lossy().starts_with("--"))
        .ok_or("Provide an input PDF path.")?;
    let (mut engine, mut out, mut parent, mut page) = (None, None, None, None);
    let (mut password, mut include_form_values, mut dpi) = (false, false, 150);
    let mut offset = 0;
    let mut selection_id = None;
    let (mut request_file,mut manifest_hash)=(None,None);
    let mut seen = HashSet::new();
    let mut fonts=Vec::new();
    let mut i = 2;
    while i < args.len() {
        let flag = args[i].to_str().ok_or("Options must be valid UTF-8.")?;
        if flag!="--font" && !seen.insert(flag) {
            return Err("An option was repeated.");
        }
        if flag == "--password-stdin" && command!="save-text-preview" {
            password = true;
            i += 1;
            continue;
        }
        if flag == "--include-form-values" && command == "text" {
            include_form_values = true;
            i += 1;
            continue;
        }
        if !(flag == "--engine"
            || (flag=="--font" && matches!(command,"preview-text"|"preview-text-batch"|"preview-paragraph"))
            || (flag == "--out" && matches!(command,"text"|"save-text-preview"|"export-pages"))
            || (flag == "--out-dir" && matches!(command,"preview-replacement"|"render"|"preview-text"|"preview-text-batch"|"preview-paragraph"|"preview-form"))
            || (flag == "--request" && matches!(command,"plan-replacement"|"preview-replacement"|"preview-text"|"preview-text-batch"|"preview-paragraph"|"preview-form"|"export-pages"))
            || (flag == "--manifest-sha256" && command=="save-text-preview")
            || (flag == "--page" && matches!(command,"text"|"render"|"inspect-nested-text"|"inspect-text"|"inspect-paragraph"|"inspect-forms"))
            || (flag == "--selection-id" && command == "inspect-paragraph")
            || (flag == "--offset" && matches!(command,"inspect-text"|"inspect-nested-text"|"inspect-forms"))
            || (flag == "--dpi" && command == "render"))
        {
            return Err("Unknown option or option not valid for this command.");
        }
        let value = args
            .get(i + 1)
            .filter(|s| !s.is_empty() && !s.to_string_lossy().starts_with("--"))
            .ok_or("An option requires a value.")?;
        match flag {
            "--font" => {
                if fonts.len()>=automation::font_inputs::MAX_FILES {return Err("Supply at most16 font files.");}
                fonts.push(PathBuf::from(value));
            }
            "--engine" => engine = Some(PathBuf::from(value)),
            "--out" => out = Some(PathBuf::from(value)),
            "--out-dir" => parent = Some(PathBuf::from(value)),
            "--request"=>request_file=Some(PathBuf::from(value)),
            "--manifest-sha256"=>{
                let value=value.to_str().filter(|v|automation::text_preview::valid_hash(v)).ok_or("Provide a lowercase SHA256 preview manifest fingerprint.")?;
                manifest_hash=Some(value.to_owned());
            }
            "--selection-id" => {
                selection_id=Some(value.to_str().filter(|v|automation::paragraph_inspection::valid_seed(v)).ok_or("Provide a text selectionId from inspect-text.")?.to_owned());
            }
            "--offset" => {
                offset = number(value)?;
                if offset > 100_000 { return Err("Offset must be between 0 and 100,000."); }
            }
            "--page" => {
                let n = number(value)?;
                if !(1..=10_000).contains(&n) {
                    return Err("Page must be between 1 and 10,000.");
                }
                page = Some(n);
            }
            "--dpi" => {
                dpi = number(value)?;
                if ![72, 150, 300, 600].contains(&dpi) {
                    return Err("DPI must be 72, 150, 300 or 600.");
                }
            }
            _ => unreachable!(),
        }
        i += 2;
    }
    let input = PathBuf::from(input);
    let request = match command {
        "plan-replacement"=>Request::PlanReplacement{input,request_file:request_file.ok_or("Replacement planning requires --request INTENT.json.")?},
        "preview-replacement"=>Request::PreviewReplacement{input,request_file:request_file.ok_or("Replacement preview requires --request INTENT.json.")?,parent:parent.ok_or("Replacement preview requires --out-dir EXISTING_DIR.")?},
        "info" => Request::Info { input },
        "export-pages"=>Request::ExportPages {input,request_file,output:out.ok_or("Page export requires --out NEW.pdf.")?},
        "inspect-forms"=>Request::InspectForms {input,page:page.ok_or("Form inspection requires --page N.")?,offset:offset as usize},
        "preview-form"=>Request::PreviewForm {input,request_file:request_file.ok_or("Form preview requires --request EDIT.json.")?,parent:parent.ok_or("Form preview requires --out-dir EXISTING_DIR.")?},
        "preview-paragraph"=>Request::PreviewParagraph {input,request_file:request_file.ok_or("Paragraph preview requires --request EDIT.json.")?,parent:parent.ok_or("Paragraph preview requires --out-dir EXISTING_DIR.")?},
        "preview-text"=>Request::PreviewText {input,request_file:request_file.ok_or("Text preview requires --request EDIT.json.")?,parent:parent.ok_or("Text preview requires --out-dir EXISTING_DIR.")?},
        "preview-text-batch"=>Request::PreviewTextBatch {input,request_file:request_file.ok_or("Batch preview requires --request EDITS.json.")?,parent:parent.ok_or("Batch preview requires --out-dir EXISTING_DIR.")?},
        "save-text-preview"=>Request::SaveTextPreview {input,manifest_hash:manifest_hash.ok_or("Saving requires --manifest-sha256 from the preview result.")?,output:out.ok_or("Saving requires --out NEW.pdf.")?},
        "inspect-paragraph" => Request::InspectParagraph {input,page:page.ok_or("Paragraph inspection requires --page N.")?,selection_id:selection_id.ok_or("Paragraph inspection requires --selection-id.")?},
        "inspect-nested-text" => Request::InspectNestedText {input,page:page.ok_or("Nested text inspection requires --page N.")?,offset:offset as usize},
        "inspect-text" => Request::InspectText {
            input, page: page.ok_or("Text inspection requires --page N.")?, offset: offset as usize,
        },
        "text" => Request::Text {
            input,
            output: out.ok_or("Text export requires --out NEW.txt.")?,
            page,
            include_form_values,
        },
        _ => Request::Render {
            input,
            parent: parent.ok_or("Rendering requires --out-dir EXISTING_DIR.")?,
            page,
            dpi,
        },
    };
    Ok(Action::Execute(Invocation {
        request,
        engine,
        password,
        fonts,
    }))
}
fn number(value: &OsString) -> Result<u32, &'static str> {
    let s = value
        .to_str()
        .filter(|s| s.bytes().all(|b| b.is_ascii_digit()))
        .ok_or("Expected a positive integer.")?;
    s.parse().map_err(|_| "Expected a positive integer.")
}
fn password(input: &mut impl Read) -> Result<Sensitive, &'static str> {
    let mut bytes = automation::credentials::read_sensitive(input,1026)
        .map_err(|_| "Cannot read password stdin.")?;
    if bytes.0.last() == Some(&b'\n') {
        bytes.0.pop();
        if bytes.0.last() == Some(&b'\r') {
            bytes.0.pop();
        }
    }
    if bytes.0.len() > 1024
        || bytes.0.iter().any(|b| matches!(*b, 0 | b'\r' | b'\n'))
        || std::str::from_utf8(&bytes.0).is_err()
    {
        return Err("Password must be one UTF-8 line of at most 1024 bytes.");
    }
    Ok(bytes)
}
pub(crate) fn engine_path() -> Result<PathBuf, EngineError> {
    let exe = std::env::current_exe().map_err(|e| EngineError::Initialization(e.to_string()))?;
    let parent = exe
        .parent()
        .ok_or_else(|| EngineError::Initialization("Executable directory unavailable.".into()))?;
    let name = if cfg!(windows) {
        "pdfium.dll"
    } else if cfg!(target_os = "macos") {
        "libpdfium.dylib"
    } else {
        "libpdfium.so"
    };
    Ok(parent.join("resources/pdfium").join(name))
}
fn error_code(error: &EngineError) -> &'static str {
    match error {
        EngineError::Initialization(_) => "engine_unavailable",
        EngineError::WorkerStopped => "engine_stopped",
        EngineError::InvalidRequest(_) => "invalid_request",
        EngineError::PermissionDenied(_) => "permission_denied",
        EngineError::Pdfium(_) => "invalid_document",
        EngineError::Io(_) => "io_error",
        EngineError::Protection(p) => match p {
            ProtectionError::PasswordRequired => "password_required",
            ProtectionError::IncorrectPassword => "incorrect_password",
            ProtectionError::Unsupported(_) => "unsupported_protection",
            ProtectionError::Limit(_) => "limit_exceeded",
            ProtectionError::Invalid(_) => "invalid_document",
        },
    }
}
fn report(error: &mut impl Write, code: &str, message: &str, status: i32) -> i32 {
    let _ = writeln!(
        error,
        "{}",
        json!({"schemaVersion":1,"ok":false,"error":{"code":code,"message":message}})
    );
    status
}
fn success_response(command: &str, result: Value) -> Result<Value, EngineError> {
    let response=json!({"schemaVersion":1,"ok":true,"command":command,"result":result});
    if matches!(command,"plan-replacement"|"preview-replacement"){automation::replacement::check_response(&response)?;}
    if command=="inspect-paragraph" {
        automation::paragraph_inspection::check_response(&response)?;
    }
    if command=="inspect-nested-text" {automation::nested_inspection::check_response(&response)?;}
    if command=="inspect-forms" {automation::form_automation::check_response(&response)?;}
    if matches!(command,"export-pages"|"compose-pages") {automation::page_organization::check_cli_response(&response)?;}
    Ok(response)
}
#[cfg(test)]
mod replacement_automation_cli_tests{
    use super::*;
    #[test]
    fn replacement_automation_cli_parses_plan_and_preview_without_manual_selections(){
        for command in ["plan-replacement","preview-replacement"]{
            let mut args=vec![command,"Input.pdf","--request","intent.json","--password-stdin"];
            if command=="preview-replacement"{args.extend(["--out-dir","Output"]);}
            let Action::Execute(parsed)=parse(args.into_iter().map(OsString::from).collect()).unwrap()else{panic!("Expected replacement invocation")};
            assert_eq!(parsed.request.command(),command);assert!(parsed.password);
        }
        for args in [vec!["plan-replacement","Input.pdf"],vec!["plan-replacement","Input.pdf","--request","i.json","--out-dir","Output"],vec!["preview-replacement","Input.pdf","--request","i.json"],vec!["plan-replacement","Input.pdf","--request","i.json","--page","1"]]{assert!(parse(args.into_iter().map(OsString::from).collect()).is_err());}
    }
}
/// Console entry point; argument values and stdin are never echoed on parse failures.
pub fn run(
    args: impl IntoIterator<Item = OsString>,
    mut input: impl Read,
    mut output: impl Write,
    mut error: impl Write,
) -> i32 {
    let action = match parse(args.into_iter().collect()) {
        Ok(a) => a,
        Err(e) => return report(&mut error, "invalid_arguments", e, 2),
    };
    let result: Result<Value, EngineError> = match action {
        Action::Compose(invocation)=>{
            let credentials=if invocation.passwords{
                match automation::credentials::Credentials::read(&mut input,invocation.request.inputs.len()){
                    Ok(value)=>value,Err(e)=>return report(&mut error,"invalid_arguments",e,2),
                }
            }else{automation::credentials::Credentials::none(invocation.request.inputs.len())};
            invocation.engine.map(Ok).unwrap_or_else(engine_path).and_then(PdfEngine::start)
                .and_then(|engine|automation::composition::execute(&engine,invocation.request,&credentials))
                .and_then(|result|success_response("compose-pages",result))
        }
        Action::Fonts(paths) => automation::font_inputs::FontInputs::load(&paths)
            .and_then(|fonts|success_response("fonts",fonts.catalog())),
        Action::Help | Action::Version => {
            let text = if matches!(action, Action::Help) {
                HELP.to_string()
            } else {
                format!("folio-cli {}\n", env!("CARGO_PKG_VERSION"))
            };
            return if output.write_all(text.as_bytes()).is_ok() {
                0
            } else {
                report(&mut error, "io_error", "Cannot write stdout.", 1)
            };
        }
        Action::Execute(invocation) => {
            let password = if invocation.password {
                match password(&mut input) {
                    Ok(p) => Some(p),
                    Err(e) => return report(&mut error, "invalid_arguments", e, 2),
                }
            } else {
                None
            };
            let command = invocation.request.command();
            let path = invocation.engine.map(Ok).unwrap_or_else(engine_path);
            automation::font_inputs::FontInputs::load(&invocation.fonts).and_then(|fonts|path.and_then(PdfEngine::start)
                .and_then(|engine| {
                    automation::execute_with_fonts(
                        &engine,
                        invocation.request,
                        password
                            .as_ref()
                            .map(|p| std::str::from_utf8(&p.0).expect("password validated")),
                        &fonts,
                    )
                }))
                .and_then(|result| success_response(command, result))
        }
    };
    match result {
        Ok(value) => {
            if writeln!(output, "{value}").is_ok() {
                0
            } else {
                report(&mut error, "io_error", "Cannot write stdout.", 1)
            }
        }
        Err(e) => {
            let message=match &e {EngineError::Initialization(_)=>"Cannot load PDFium. Supply --engine or package resources/pdfium beside the CLI.".to_string(),EngineError::Pdfium(_)=>"The PDF could not be processed.".to_string(),EngineError::Io(_)=>"The file operation failed. Check the paths and access permissions.".to_string(),_=>e.to_string()};
            report(&mut error, error_code(&e), &message, 1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paragraph_wire_limit_includes_cli_wrapper_and_newline() {
        const CAP:usize=256*1024;
        let base=success_response("inspect-paragraph",json!({"text":""})).unwrap();
        let overhead=serde_json::to_vec(&base).unwrap().len();
        let exact=json!({"text":"x".repeat(CAP-overhead-1)});
        let response=success_response("inspect-paragraph",exact.clone()).unwrap();
        assert_eq!(serde_json::to_vec(&response).unwrap().len()+1,CAP);
        assert_eq!(response["result"],exact);
        for value in [json!({"text":"x".repeat(CAP-overhead)}),json!({"text":"\"\\".repeat(CAP/2)}),json!({"text":"é".repeat(CAP/2)})] {
            assert!(success_response("inspect-paragraph",value).is_err(),"Reject final oversized frame before writing any success bytes");
        }
    }
    #[test]
    fn form_wire_limit_includes_cli_wrapper_and_newline() {
        const CAP:usize=2*1024*1024;
        let base=success_response("inspect-forms",json!({"text":""})).unwrap();
        let overhead=serde_json::to_vec(&base).unwrap().len();
        let exact=json!({"text":"x".repeat(CAP-overhead-1)});
        let response=success_response("inspect-forms",exact.clone()).unwrap();
        assert_eq!(serde_json::to_vec(&response).unwrap().len()+1,CAP);
        assert_eq!(response["result"],exact);
        for value in [json!({"text":"x".repeat(CAP-overhead)}),json!({"text":"\"\\".repeat(CAP/2)}),json!({"text":"é".repeat(CAP/2)})] {
            assert!(success_response("inspect-forms",value).is_err(),"Reject final oversized frame before writing any success bytes");
        }
    }
    #[test]
    fn password_boundary_and_crlf_are_exact() {
        for ending in [b"".as_slice(), b"\n", b"\r\n"] {
            let mut bytes = vec![b'x'; 1024];
            bytes.extend_from_slice(ending);
            assert_eq!(password(&mut bytes.as_slice()).unwrap().0.len(), 1024);
        }
        assert!(password(&mut vec![b'x'; 1025].as_slice()).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn native_paths_do_not_require_utf8() {
        use std::os::unix::ffi::OsStringExt;
        let input = OsString::from_vec(vec![b'p', 0xff]);
        let Action::Execute(invocation) = parse(vec!["info".into(), input.clone()]).unwrap() else {
            panic!()
        };
        let Request::Info { input: path } = invocation.request else {
            panic!()
        };
        assert_eq!(path.as_os_str(), input);
    }
}
