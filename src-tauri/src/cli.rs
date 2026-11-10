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
folio-cli fonts --font FONT [--font FONT ...]\n\
folio-cli inspect-forms INPUT.pdf --page N [--offset N] [options]\n\
folio-cli preview-form INPUT.pdf --request EDIT.json --out-dir EXISTING_DIR [options]\n\
folio-cli inspect-text INPUT.pdf --page N [--offset N] [options]\n\
folio-cli inspect-nested-text INPUT.pdf --page N [--offset N] [options]\n\
folio-cli inspect-paragraph INPUT.pdf --page N --selection-id TEXT_SELECTION [options]\n\
folio-cli preview-text INPUT.pdf --request EDIT.json --out-dir EXISTING_DIR [--font FONT ...] [options]\n\
folio-cli preview-paragraph INPUT.pdf --request EDIT.json --out-dir EXISTING_DIR [--font FONT ...] [options]\n\
folio-cli save-text-preview BUNDLE_DIR --manifest-sha256 HASH --out NEW.pdf [--engine PATH]\n\
folio-cli text INPUT.pdf --out NEW.txt [--page N] [--include-form-values] [options]\n\
folio-cli render INPUT.pdf --out-dir EXISTING_DIR [--page N] [--dpi 72|150|300|600] [options]\n\n\
folio-cli mcp --input PDF [--input PDF ...] --out-dir EXISTING_DIR [--engine PATH] [--font FONT ...]\n\n\
Options: --engine PATH, --password-stdin\n\
--help   Show help without loading the PDF engine.\n\
--version   Show version.\n\n\
Pages are 1-based; text/render omit --page for all pages. PNG defaults to 150 DPI and a new child folder.\n\
Inspection requires --page; --offset defaults to 0 and returns up to 100 text runs.\n\
Text uses the desktop UTF-8 export, with a BOM. Existing files are never overwritten.\n\
Passwords are read as one UTF-8 line from stdin only when requested (maximum 1024 bytes).\n\
Success JSON goes to stdout; error JSON to stderr. Exit codes: 0 success, 2 arguments, 1 operation.\n\
Without --engine, PDFium is loaded from resources/pdfium beside this executable.\n";
struct Invocation {
    request: Request,
    engine: Option<PathBuf>,
    password: bool,
    fonts: Vec<PathBuf>,
}
enum Action {
    Help,
    Version,
    Fonts(Vec<PathBuf>),
    Execute(Invocation),
}
fn parse(args: Vec<OsString>) -> Result<Action, &'static str> {
    if args.len() == 1 && args[0] == "--version" {
        return Ok(Action::Version);
    }
    if (args.len() == 1 && args[0] == "--help")
        || (args.len() == 2
            && matches!(args[0].to_str(), Some("fonts" | "info" | "text" | "render" | "inspect-nested-text" | "inspect-text" | "inspect-paragraph" | "preview-text" | "preview-paragraph" | "preview-form" | "inspect-forms" | "save-text-preview"))
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
    if !matches!(command, "info" | "text" | "render" | "inspect-nested-text" | "inspect-text" | "inspect-paragraph" | "preview-text" | "preview-paragraph" | "preview-form" | "inspect-forms" | "save-text-preview") {
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
            || (flag=="--font" && matches!(command,"preview-text"|"preview-paragraph"))
            || (flag == "--out" && matches!(command,"text"|"save-text-preview"))
            || (flag == "--out-dir" && matches!(command,"render"|"preview-text"|"preview-paragraph"|"preview-form"))
            || (flag == "--request" && matches!(command,"preview-text"|"preview-paragraph"|"preview-form"))
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
        "info" => Request::Info { input },
        "inspect-forms"=>Request::InspectForms {input,page:page.ok_or("Form inspection requires --page N.")?,offset:offset as usize},
        "preview-form"=>Request::PreviewForm {input,request_file:request_file.ok_or("Form preview requires --request EDIT.json.")?,parent:parent.ok_or("Form preview requires --out-dir EXISTING_DIR.")?},
        "preview-paragraph"=>Request::PreviewParagraph {input,request_file:request_file.ok_or("Paragraph preview requires --request EDIT.json.")?,parent:parent.ok_or("Paragraph preview requires --out-dir EXISTING_DIR.")?},
        "preview-text"=>Request::PreviewText {input,request_file:request_file.ok_or("Text preview requires --request EDIT.json.")?,parent:parent.ok_or("Text preview requires --out-dir EXISTING_DIR.")?},
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
    let mut bytes = Sensitive(Vec::new());
    input
        .take(1027)
        .read_to_end(&mut bytes.0)
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
    if command=="inspect-paragraph" {
        automation::paragraph_inspection::check_response(&response)?;
    }
    if command=="inspect-nested-text" {automation::nested_inspection::check_response(&response)?;}
    if command=="inspect-forms" {automation::form_automation::check_response(&response)?;}
    Ok(response)
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
