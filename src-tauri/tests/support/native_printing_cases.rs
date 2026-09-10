use super::*;
use crate::{ExportRequest, PdfEngine};
use std::path::Path;

fn engine() -> PdfEngine {
    let library = std::env::var_os("FOLIO_PDFIUM_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")
        });
    PdfEngine::start(library).unwrap()
}

fn fixture(path: &Path) {
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>",
        "<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 300] /Resources << >> /Contents 4 0 R >>",
        "<< /Length 0 >>\nstream\n\nendstream",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 200] /Resources << >> /Contents 6 0 R >>",
        "<< /Length 0 >>\nstream\n\nendstream",
    ];
    let mut pdf = b"%PDF-1.7\n".to_vec();
    let mut offsets = vec![0];
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", index + 1, object).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend_from_slice(b"xref\n0 7\n0000000000 65535 f \n");
    for offset in offsets.iter().skip(1) {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!("trailer\n<< /Size 7 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    std::fs::write(path, pdf).unwrap();
}

#[test]
fn prepared_job_exports_selected_order_rotation_and_edits_and_cleans_up() {
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("source.pdf");
    fixture(&source_path);
    let engine = engine();
    let source = engine.open_document(&source_path).unwrap();
    let request: ExportRequest = serde_json::from_value(serde_json::json!({"pages": [
        {"id": "selected-second", "sourceId": source.id, "pageIndex": 1, "width": 400, "height": 200, "rotation": 90,
         "overlays": [{"type": "text", "id": "label", "x": 20, "y": 30, "text": "PRINT EDIT", "fontSize": 16, "color": "#ff0000"}]},
        {"id": "selected-first", "sourceId": source.id, "pageIndex": 0, "width": 200, "height": 300, "rotation": 0, "overlays": []}
    ]})).unwrap();
    let prepared = PreparedPrint::new(engine.clone(), request).unwrap();
    assert_eq!(prepared.document.pages.len(), 2);
    assert_eq!(
        (
            prepared.document.pages[0].width,
            prepared.document.pages[0].height
        ),
        (200.0, 400.0)
    );
    assert_eq!(
        (
            prepared.document.pages[1].width,
            prepared.document.pages[1].height
        ),
        (200.0, 300.0)
    );
    let saved = lopdf::Document::load(prepared._directory.path().join("Folio print.pdf")).unwrap();
    let page = saved.get_dictionary(saved.get_pages()[&1]).unwrap();
    let annotations = page.get(b"Annots").unwrap().as_array().unwrap();
    assert!(annotations.iter().any(|annotation| saved
        .dereference(annotation)
        .unwrap()
        .1
        .as_dict()
        .unwrap()
        .get(b"Subtype")
        .unwrap()
        .as_name()
        .unwrap()
        == b"FreeText"));
    let (width, height, pixels) =
        png_to_bgra(&engine.render_page(&prepared.document.id, 0, 600).unwrap()).unwrap();
    assert_eq!((width, height), (600, 1200));
    assert!(
        pixels
            .chunks_exact(4)
            .any(|p| p[2] > 180 && p[0] < 100 && p[1] < 100),
        "edited text must reach print raster"
    );
    let directory = prepared._directory.path().to_owned();
    let document_id = prepared.document.id.clone();
    drop(prepared);
    assert!(!directory.exists(), "temporary print files must be removed");
    assert!(
        engine.render_page(&document_id, 0, 100).is_err(),
        "temporary PDF must be closed"
    );
    assert!(
        engine.render_page(&source.id, 0, 100).is_ok(),
        "original document stays usable"
    );
    engine.close_document(&source.id).unwrap();
}

#[test]
fn print_raster_preserves_channel_order_and_flattens_alpha_onto_white() {
    let rgba = image::RgbaImage::from_raw(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 0]).unwrap();
    let mut png = std::io::Cursor::new(Vec::new());
    rgba.write_to(&mut png, image::ImageFormat::Png).unwrap();
    assert_eq!(
        png_to_bgra(png.get_ref()).unwrap(),
        (2, 1, vec![0, 0, 255, 255, 255, 255, 255, 255])
    );
    assert!(png_to_bgra(b"not a PNG").is_err());
}

#[test]
fn empty_print_request_is_rejected_before_native_dialog() {
    assert!(PreparedPrint::new(
        engine(),
        ExportRequest {
            flatten: true,
            pages: vec![]
        }
    )
    .is_err());
}

#[test]
fn prepared_job_raster_contains_standard_highlights_and_comment_icons() {
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("annotations.pdf");
    fixture(&source_path);
    let engine = engine();
    let source = engine.open_document(&source_path).unwrap();
    let request: ExportRequest = serde_json::from_value(serde_json::json!({"pages":[{
        "id":"page","sourceId":source.id,"pageIndex":0,"width":200,"height":300,"rotation":0,
        "overlays":[
            {"type":"highlight","id":"highlight","rects":[{"x":20,"y":30,"width":80,"height":20}],"color":"#ffff00"},
            {"type":"comment","id":"comment","x":30,"y":90,"text":"Printed note","color":"#ff8000"}
        ]
    }]})).unwrap();
    let prepared = PreparedPrint::new(engine.clone(), request).unwrap();
    let image =
        image::load_from_memory(&engine.render_page(&prepared.document.id, 0, 400).unwrap())
            .unwrap()
            .into_rgb8();
    let highlight = image.get_pixel(80, 80);
    assert!(
        highlight[0] > 200 && highlight[1] > 180 && highlight[2] < 220,
        "highlight missing from print raster: {highlight:?}"
    );
    assert!(
        (180..220).any(|y| (60..100).any(|x| {
            let p = image.get_pixel(x, y);
            p[0] > 150 && p[1] < 200 && p[2] < 100
        })),
        "comment icon missing from print raster"
    );
}

#[test]
fn print_raster_honors_print_only_and_screen_only_annotation_flags() {
    use lopdf::{dictionary, Object};
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("flags.pdf");
    fixture(&path);
    let mut pdf = lopdf::Document::load(&path).unwrap();
    let print_only=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![20.into(),246.into(),44.into(),270.into()],"F"=>36,"C"=>vec![1.into(),Object::Real(0.5),0.into()],"Contents"=>Object::string_literal("Print only")});
    let screen_only=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![20.into(),196.into(),44.into(),220.into()],"F"=>0,"C"=>vec![1.into(),0.into(),0.into()],"Contents"=>Object::string_literal("Screen only")});
    pdf.get_dictionary_mut((3, 0)).unwrap().set(
        "Annots",
        vec![
            Object::Reference(print_only),
            Object::Reference(screen_only),
        ],
    );
    pdf.save(&path).unwrap();
    let engine = engine();
    let source = engine.open_document(&path).unwrap();
    assert!(
        source.pages[0].overlays.is_empty(),
        "annotations with unrepresented visibility flags must stay native"
    );
    let request=serde_json::from_value(serde_json::json!({"pages":[{"id":"page","sourceId":source.id,"pageIndex":0,"width":200,"height":300,"rotation":0,"overlays":[]}]})).unwrap();
    let prepared = PreparedPrint::new(engine.clone(), request).unwrap();
    let image =
        image::load_from_memory(&engine.render_page(&prepared.document.id, 0, 400).unwrap())
            .unwrap()
            .into_rgb8();
    let colored = |top: u32| {
        (top..top + 48).any(|y| {
            (40..88).any(|x| {
                let p = image.get_pixel(x, y);
                p[0] > 150 && p[1] < 220 && p[2] < 120
            })
        })
    };
    assert!(
        colored(60),
        "print-only annotation must reach the print raster"
    );
    assert!(
        !colored(160),
        "screen-only annotation must not reach the print raster"
    );
}

#[cfg(windows)]
#[test]
#[ignore = "spools only to Microsoft Print to PDF; run explicitly for virtual-printer verification"]
fn virtual_printer_spools_edited_pages_to_pdf_without_a_physical_printer() {
    use windows_sys::Win32::Graphics::Gdi::{CreateDCW, DeleteDC};
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("source.pdf");
    let output_path = temp.path().join("printed.pdf");
    fixture(&source_path);
    let engine = engine();
    let source = engine.open_document(&source_path).unwrap();
    let request = serde_json::from_value(serde_json::json!({"pages": [
        {"id": "printed", "sourceId": source.id, "pageIndex": 0, "width": 200, "height": 300, "rotation": 0,
         "overlays": [{"type": "text", "id": "label", "x": 20, "y": 30, "text": "PRINT EDIT", "fontSize": 20, "color": "#ff0000"}]}
    ]})).unwrap();
    let prepared = PreparedPrint::new(engine.clone(), request).unwrap();
    let driver: Vec<u16> = "WINSPOOL\0".encode_utf16().collect();
    // Hard-coded virtual printer: this test can never choose the default/physical printer.
    let printer: Vec<u16> = "Microsoft Print to PDF\0".encode_utf16().collect();
    let dc = unsafe {
        CreateDCW(
            driver.as_ptr(),
            printer.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    assert!(!dc.is_null(), "Microsoft Print to PDF must be installed");
    let output: Vec<u16> = output_path
        .to_string_lossy()
        .encode_utf16()
        .chain([0])
        .collect();
    let result = windows::spool_document(&prepared, dc, PrintScale::Fit, Some(&output));
    unsafe {
        DeleteDC(dc);
    }
    assert!(result.unwrap());
    let printed = engine.open_document(&output_path).unwrap();
    assert_eq!(printed.pages.len(), 1);
    let (_, _, pixels) = png_to_bgra(&engine.render_page(&printed.id, 0, 600).unwrap()).unwrap();
    assert!(
        pixels
            .chunks_exact(4)
            .any(|p| p[2] > 180 && p[0] < 100 && p[1] < 100),
        "edited red text must survive GDI spooling"
    );
    engine.close_document(&printed.id).unwrap();
    engine.close_document(&source.id).unwrap();
}

#[cfg(windows)]
#[test]
#[ignore = "opens and automatically cancels the native dialog; run explicitly for desktop verification"]
fn native_dialog_cancellation_returns_false_without_spooling() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumThreadWindows, GetClassNameW, PostMessageW, IDCANCEL, WM_COMMAND,
    };
    unsafe extern "system" fn cancel_dialog(window: HWND, _: LPARAM) -> i32 {
        let mut class = [0_u16; 32];
        let length = GetClassNameW(window, class.as_mut_ptr(), class.len() as i32);
        if length > 0 && String::from_utf16_lossy(&class[..length as usize]) == "#32770" {
            // Only Cancel is ever posted, and only to the test's own thread windows.
            PostMessageW(window, WM_COMMAND, IDCANCEL as usize, 0);
        }
        1
    }
    let temp = tempfile::tempdir().unwrap();
    let source_path = temp.path().join("source.pdf");
    fixture(&source_path);
    let engine = engine();
    let source = engine.open_document(&source_path).unwrap();
    let request = serde_json::from_value(serde_json::json!({"pages": [
        {"id": "cancelled", "sourceId": source.id, "pageIndex": 0, "width": 200, "height": 300, "rotation": 0, "overlays": []}
    ]})).unwrap();
    let thread_id = unsafe { GetCurrentThreadId() };
    let finished = Arc::new(AtomicBool::new(false));
    let stop = finished.clone();
    let canceller = std::thread::spawn(move || {
        for _ in 0..200 {
            if stop.load(Ordering::SeqCst) {
                return;
            }
            unsafe {
                EnumThreadWindows(thread_id, Some(cancel_dialog), 0);
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    });
    let result = print_document(
        engine.clone(),
        request,
        PrintOptions {
            scale: PrintScale::Fit,
            orientation: PrintOrientation::Landscape,
        },
    );
    finished.store(true, Ordering::SeqCst);
    canceller.join().unwrap();
    assert!(
        !result.unwrap(),
        "cancelling must not report a submitted job"
    );
    assert!(engine.render_page(&source.id, 0, 100).is_ok());
    engine.close_document(&source.id).unwrap();
}
