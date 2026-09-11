//! Isolated editing-API probe: this binary must not start the engine worker.
//! Exercises the bundled DLL, including setters and generated saved content.
//!
//! Measured with chromium/8044: SetPositions preserves one-axis advances as TJ;
//! different y offsets use separate text-object matrices. Independent ActualText
//! marks repeat replacement text; AddExistingMark emits one BDC/EMC span across
//! adjacent objects and extracts exact Unicode in PDFium. This does not establish
//! independent-reader, script shaping, rotation, or production editing support.
use pdfium_render::prelude::*;
use std::{
    ffi::c_void,
    os::raw::{c_int, c_ulong},
    path::PathBuf,
};

const FONT: &[u8] = include_bytes!("../../tests/fixtures/corpus/fonts/DejaVuSerif.ttf");
const LIMIT: usize = 16 * 1024 * 1024;

struct Library(Box<dyn PdfiumLibraryBindings>);
impl Drop for Library {
    fn drop(&mut self) {
        unsafe {
            self.0.FPDF_DestroyLibrary();
        }
    }
}

#[repr(C)]
struct Writer {
    interface: FPDF_FILEWRITE,
    bytes: Vec<u8>,
}
unsafe extern "C" fn write_block(
    writer: *mut FPDF_FILEWRITE,
    data: *const c_void,
    size: c_ulong,
) -> c_int {
    let writer = &mut *(writer as *mut Writer);
    let size = size as usize;
    if size > LIMIT.saturating_sub(writer.bytes.len()) || (data.is_null() && size != 0) {
        return 0;
    }
    if writer.bytes.try_reserve(size).is_err() {
        return 0;
    }
    if size != 0 {
        writer
            .bytes
            .extend_from_slice(std::slice::from_raw_parts(data.cast::<u8>(), size));
    }
    1
}

struct Document<'a> {
    b: &'a dyn PdfiumLibraryBindings,
    doc: FPDF_DOCUMENT,
    pages: Vec<FPDF_PAGE>,
    fonts: Vec<FPDF_FONT>,
}
impl Drop for Document<'_> {
    fn drop(&mut self) {
        unsafe {
            for page in self.pages.drain(..) {
                self.b.FPDF_ClosePage(page);
            }
            for font in self.fonts.drain(..) {
                self.b.FPDFFont_Close(font);
            }
            self.b.FPDF_CloseDocument(self.doc);
        }
    }
}
impl Document<'_> {
    unsafe fn page(&mut self) -> FPDF_PAGE {
        let page = self
            .b
            .FPDFPage_New(self.doc, self.pages.len() as i32, 180., 120.);
        assert!(!page.is_null());
        self.pages.push(page);
        page
    }
    unsafe fn font(&mut self, chars: &[char], mappings: &[&str]) -> FPDF_FONT {
        let face = ttf_parser::Face::parse(FONT, 0).unwrap();
        let mut gids = vec![0u8; (chars.len() + 1) * 2];
        let mut cmap = String::from("/CIDInit /ProcSet findresource begin 12 dict begin begincmap /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def /CMapName /FolioProbe def /CMapType 2 def 1 begincodespacerange <0000> <FFFF> endcodespacerange\n");
        cmap.push_str(&format!("{} beginbfchar\n", chars.len()));
        for (i, ch) in chars.iter().enumerate() {
            gids[(i + 1) * 2..(i + 2) * 2]
                .copy_from_slice(&face.glyph_index(*ch).unwrap().0.to_be_bytes());
            let unicode: String = mappings[i]
                .encode_utf16()
                .map(|v| format!("{v:04X}"))
                .collect();
            cmap.push_str(&format!("<{:04X}> <{unicode}>\n", i + 1));
        }
        cmap.push_str("endbfchar endcmap CMapName currentdict /CMap defineresource pop end end");
        let font = self.b.FPDFText_LoadCidType2Font(
            self.doc,
            FONT.as_ptr(),
            FONT.len() as u32,
            &cmap,
            gids.as_ptr(),
            gids.len() as u32,
        );
        assert!(!font.is_null());
        self.fonts.push(font);
        font
    }
    unsafe fn object(&self, font: FPDF_FONT, codes: &[u32], x: f32, y: f32) -> FPDF_PAGEOBJECT {
        let object = self.b.FPDFPageObj_CreateTextObj(self.doc, font, 20.);
        assert!(!object.is_null());
        assert_ne!(
            self.b
                .FPDFText_SetCharcodes(object, codes.as_ptr(), codes.len()),
            0
        );
        assert_ne!(
            self.b.FPDFPageObj_SetMatrix(
                object,
                &FS_MATRIX {
                    a: 1.,
                    b: 0.,
                    c: 0.,
                    d: 1.,
                    e: x,
                    f: y
                }
            ),
            0
        );
        object
    }
    unsafe fn save(&self) -> Vec<u8> {
        let mut writer = Writer {
            interface: FPDF_FILEWRITE {
                version: 1,
                WriteBlock: Some(write_block),
            },
            bytes: vec![],
        };
        assert_ne!(
            self.b.FPDF_SaveAsCopy(self.doc, &mut writer.interface, 0),
            0
        );
        writer.bytes
    }
}
unsafe fn text(b: &dyn PdfiumLibraryBindings, page: FPDF_PAGE) -> (String, Vec<(u32, f64, f64)>) {
    let text = b.FPDFText_LoadPage(page);
    assert!(!text.is_null());
    let count = b.FPDFText_CountChars(text);
    assert!((0..100).contains(&count));
    let mut chars = vec![0u16; count as usize + 1];
    let length = b.FPDFText_GetText(text, 0, count, chars.as_mut_ptr());
    let value = String::from_utf16(&chars[..length.saturating_sub(1) as usize]).unwrap();
    let mut origins = vec![];
    for i in 0..count {
        let (mut x, mut y) = (0., 0.);
        assert_ne!(b.FPDFText_GetCharOrigin(text, i, &mut x, &mut y), 0);
        origins.push((b.FPDFText_GetUnicode(text, i), x, y));
    }
    b.FPDFText_ClosePage(text);
    (value, origins)
}
unsafe fn raster(b: &dyn PdfiumLibraryBindings, page: FPDF_PAGE) -> Vec<u8> {
    let bitmap = b.FPDFBitmap_Create(360, 240, 1);
    assert!(!bitmap.is_null());
    b.FPDFBitmap_FillRect(bitmap, 0, 0, 360, 240, 0xffffffff);
    b.FPDF_RenderPageBitmap(bitmap, page, 0, 0, 360, 240, 0, 0);
    let pixels = b.FPDFBitmap_GetBuffer_as_vec(bitmap);
    b.FPDFBitmap_Destroy(bitmap);
    pixels
}
unsafe fn actual_text(d: &Document<'_>, object: FPDF_PAGEOBJECT) -> FPDF_PAGEOBJECTMARK {
    let mark = d.b.FPDFPageObj_AddMark(object, "Span");
    assert!(!mark.is_null());
    let bytes: Vec<u8> = [0xfe, 0xff]
        .into_iter()
        .chain(
            "q\u{0307}\u{0323}"
                .encode_utf16()
                .flat_map(u16::to_be_bytes),
        )
        .collect();
    assert_ne!(
        d.b.FPDFPageObjMark_SetBlobParam(
            d.doc,
            object,
            mark,
            "ActualText",
            bytes.as_ptr(),
            bytes.len() as c_ulong
        ),
        0
    );
    mark
}

#[test]
fn native_cid_setters_positions_and_shared_actual_text_survive_save_reopen() {
    let path = std::env::var_os("FOLIO_PDFIUM_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")
        });
    let library = Library(Pdfium::bind_to_library(path).unwrap());
    let b = library.0.as_ref();
    unsafe {
        b.FPDF_InitLibrary();
        let handle = b.FPDF_CreateNewDocument();
        assert!(!handle.is_null());
        let mut d = Document {
            b,
            doc: handle,
            pages: vec![],
            fonts: vec![],
        };
        let font = d.font(&['A', 'B', 'C'], &["A", "B", "C"]);
        let page = d.page();
        let object = d.object(font, &[1, 2, 3], 20., 70.);
        assert_eq!(
            b.FPDFText_SetPositions(object, [15.].as_ptr(), 1),
            0,
            "N glyphs require N-1 positions"
        );
        assert_ne!(b.FPDFText_SetPositions(object, [15., 30.].as_ptr(), 2), 0);
        assert_ne!(b.FPDFPage_InsertObject(page, object), 0);
        assert_ne!(b.FPDFPage_GenerateContent(page), 0);
        let mark_font = d.font(
            &['q', '\u{0307}', '\u{0323}'],
            &["q", "\u{0307}\u{0323}", "\u{0307}\u{0323}"],
        );
        // Zero-advance marks need distinct y origins: SetPositions only accepts one axis.
        // Pages 1/2/3 compare no marks, independent ActualText, and a shared mark.
        for mode in 0..3 {
            let page = d.page();
            let mut shared = std::ptr::null_mut();
            for (index, (x, y)) in [(20., 70.), (32., 70.), (32., 65.)].into_iter().enumerate() {
                let object = d.object(mark_font, &[index as u32 + 1], x, y);
                assert_eq!(
                    b.FPDFText_SetPositions(object, [0.].as_ptr(), 1),
                    0,
                    "single glyph positioning uses its matrix"
                );
                if mode == 1 {
                    actual_text(&d, object);
                }
                if mode == 2 {
                    if index == 0 {
                        shared = actual_text(&d, object);
                    } else {
                        assert_ne!(b.FPDFPageObj_AddExistingMark(object, shared), 0);
                    }
                }
                assert_ne!(b.FPDFPage_InsertObject(page, object), 0);
            }
            assert_ne!(b.FPDFPage_GenerateContent(page), 0);
        }
        let before: Vec<_> = d
            .pages
            .iter()
            .map(|&page| (text(b, page), raster(b, page)))
            .collect();
        let bytes = d.save();
        let output = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../artifacts/shaped-text");
        std::fs::create_dir_all(&output).unwrap();
        std::fs::write(output.join("pdfium-api.pdf"), &bytes).unwrap();
        let handle = b.FPDF_LoadMemDocument64(&bytes, None);
        assert!(!handle.is_null());
        let mut reopened = Document {
            b,
            doc: handle,
            pages: vec![],
            fonts: vec![],
        };
        let parsed = lopdf::Document::load_mem(&bytes).unwrap();
        for i in 0..4 {
            let page = b.FPDF_LoadPage(handle, i as i32);
            assert!(!page.is_null());
            reopened.pages.push(page);
            let after = text(b, page);
            println!(
                "page {i}: before={:?}, reopened={:?}, origins={:?}",
                before[i].0 .0, after.0, after.1
            );
            assert_eq!(before[i].0 .0, after.0, "saved extraction changed");
            assert_eq!(before[i].1, raster(b, page), "saved raster changed");
            let content = parsed.get_page_content(parsed.get_pages()[&(i as u32 + 1)]);
            std::fs::write(output.join(format!("pdfium-api-page-{i}.txt")), &content).unwrap();
            let operators = lopdf::content::Content::decode(&content).unwrap();
            assert_eq!(
                operators
                    .operations
                    .iter()
                    .filter(|op| op.operator == "BDC")
                    .count(),
                [0, 0, 3, 1][i]
            );
            if i == 0 {
                let letters: Vec<_> = after
                    .1
                    .iter()
                    .filter(|(ch, _, _)| (65..=67).contains(ch))
                    .collect();
                assert_eq!(letters.len(), 3);
                for (letter, expected_x) in letters.iter().zip([20., 35., 50.]) {
                    assert!((letter.1 - expected_x).abs() < 0.01);
                    assert!((letter.2 - 70.).abs() < 0.01);
                }
            }
            if i == 3 {
                assert_eq!(
                    after.0, "q\u{0307}\u{0323}",
                    "one shared replacement span should preserve logical text"
                );
            }
            if i == 1 {
                assert_eq!(
                    after.0, "q\u{0307}\u{0323}\r\n\u{0307}\u{0323}",
                    "negative control must expose duplicated cluster mapping"
                );
                for (_, x, y) in after.1.iter().filter(|(ch, _, _)| *ch == 775) {
                    assert!((*x - 32.).abs() < 0.01);
                    assert!((*y - 70.).abs() < 0.01 || (*y - 65.).abs() < 0.01);
                }
            }
            if i == 2 {
                assert_eq!(
                    after.0, "q\u{0307}\u{0323}q\u{0307}\u{0323}\r\nq\u{0307}\u{0323}",
                    "independent marks each replace their own object"
                );
            }
        }
        assert_eq!(before[1].1, before[2].1);
        assert_eq!(before[1].1, before[3].1);
        std::fs::write(output.join("pdfium-api.json"), serde_json::to_vec_pretty(&serde_json::json!({
            "font": "tests/fixtures/corpus/fonts/DejaVuSerif.ttf (LICENSE_DEJAVU.txt)",
            "page_text": before.iter().map(|page| &page.0.0).collect::<Vec<_>>(),
            "page_origins": before.iter().map(|page| &page.0.1).collect::<Vec<_>>(),
            "before_after_rasters_equal": true,
            "mark_variants_rasters_equal": true,
            "generated_bdc_counts": [0, 0, 3, 1],
            "scope": "Actual native setter/create/save/reopen test; PDFium extraction only, rotation 0, manually selected glyphs and offsets."
        })).unwrap()).unwrap();
    }
}
