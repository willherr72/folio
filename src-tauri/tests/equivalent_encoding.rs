use folio_engine::{DocumentInfo, ExportRequest, PagePlan, PdfEngine};
use lopdf::{dictionary, Document, Object, Stream};
use std::{fs, path::{Path, PathBuf}};

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()
}
fn fixture(path: &Path, base: &str, indirect: bool, rotation: i64, embedded: Option<bool>, spaced: bool) {
    let mut pdf = Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let names = vec![32.into(), Object::Name(b"space".to_vec()), 65.into(), Object::Name(b"A".to_vec()), Object::Name(b"B".to_vec())];
    let enc: Object = if indirect {
        let base = pdf.add_object(Object::Name(base.as_bytes().to_vec()));
        let values = pdf.add_object(names);
        pdf.add_object(dictionary! {"Type"=>"Encoding","BaseEncoding"=>base,"Differences"=>values}).into()
    } else { dictionary! {"BaseEncoding"=>base,"Differences"=>names}.into() };
    let mut font = dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica","Encoding"=>enc};
    if let Some(to_unicode) = embedded {
        let program = fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap();
        let face = ttf_parser::Face::parse(&program, 0).unwrap();
        let widths: Vec<Object> = (32..=126).map(|code| {
            let gid = face.glyph_index(char::from_u32(code).unwrap()).unwrap();
            Object::Real(face.glyph_hor_advance(gid).unwrap() as f32 * 1000. / face.units_per_em() as f32)
        }).collect();
        let file = pdf.add_object(Stream::new(dictionary! {"Length1"=>program.len() as i64}, program));
        let descriptor = pdf.add_object(dictionary! {"Type"=>"FontDescriptor","FontName"=>"DejaVuSerif","Flags"=>32,"FontBBox"=>vec![(-1000).into(),(-1000).into(),2000.into(),2000.into()],"ItalicAngle"=>0,"Ascent"=>1000,"Descent"=>-300,"CapHeight"=>800,"StemV"=>80,"FontFile2"=>file});
        font.set("Subtype", "TrueType"); font.set("BaseFont", "DejaVuSerif");
        font.set("FontDescriptor", descriptor); font.set("FirstChar", 32); font.set("LastChar", 126); font.set("Widths", widths);
        if to_unicode {
            let mut cmap = String::from("1 begincodespacerange <00> <FF> endcodespacerange 95 beginbfchar\n");
            for code in 32..=126 { cmap.push_str(&format!("<{code:02X}> <{code:04X}>\n")); }
            cmap.push_str("endbfchar");
            let cmap = pdf.add_object(Stream::new(dictionary! {}, cmap.into_bytes())); font.set("ToUnicode", cmap);
        }
    }
    let font = pdf.add_object(font);
    let mut content = String::from("q .2 .4 .6 rg BT /F1 16 Tf 1 0 0 1 40 300 Tm ");
    content.push_str(if spaced {".1 Tc [(Old) -12 ( words)] TJ"} else {"(Old words) Tj"});
    content.push_str(" ET Q BT /F1 16 Tf 40 200 Td (Neighbor safe) Tj ET");
    let content = pdf.add_object(Stream::new(dictionary! {}, content.into_bytes()));
    let page = pdf.add_object(dictionary! {"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),360.into(),400.into()],"Rotate"=>rotation,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});
    pdf.objects.insert(pages, dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());
    let root = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages}); pdf.trailer.set("Root", root);
    pdf.save(path).unwrap();
}
fn export(engine: &PdfEngine, source: &DocumentInfo, path: &Path) {
    engine.export_pdf(ExportRequest { flatten: false, pages: vec![PagePlan {
        id:"page".into(), source_id:source.id.clone(), page_index:0,
        width:source.pages[0].width, height:source.pages[0].height, rotation:0, crop:None, overlays:vec![],
    }] }, path).unwrap();
}
fn dump(name: &str, input: &Path, output: &Path) {
    if let Some(root) = std::env::var_os("FOLIO_ENCODING_DUMP") {
        let dir = PathBuf::from(root).join(name); fs::create_dir_all(dir.parent().unwrap()).unwrap(); fs::create_dir(&dir).unwrap();
        fs::copy(input, dir.join("original.pdf")).unwrap(); fs::copy(output, dir.join("edited.pdf")).unwrap();
    }
}
fn roundtrip(engine: &PdfEngine, source: &DocumentInfo, changed: &DocumentInfo, input: &Path, output: &Path, name: &str, expected: &str) {
    let before = engine.list_text_runs(&source.id,0).unwrap();
    export(engine, changed, output);
    let reopened = engine.open_document(output).unwrap();
    let after = engine.list_occurrence_text_runs(&reopened.id,0).unwrap();
    assert!(after.runs.iter().any(|run| run.text == expected), "{name}: {:?}", after.runs);
    let a = before.runs.iter().find(|r|r.text == "Neighbor safe").unwrap();
    let b = after.runs.iter().find(|r|r.text == a.text).unwrap();
    assert_eq!(a.bounds,b.bounds,"{name}"); assert_eq!(a.font_name,b.font_name,"{name}"); assert_eq!(a.font_size,b.font_size,"{name}");
    dump(name,input,output);
    engine.close_document(&reopened.id).unwrap();
}

#[test]
fn standard_equivalent_encodings_roundtrip_in_every_rotation_and_stream_route() {
    let engine = engine(); let temp = tempfile::tempdir().unwrap();
    for base in ["WinAnsiEncoding","MacRomanEncoding"] { for rotation in [0,90,180,270] { for indirect in [false,true] {
        let name = format!("standard-{base}-{rotation}-{indirect}");
        let input = temp.path().join(format!("{name}.pdf")); let output = temp.path().join(format!("{name}-saved.pdf"));
        fixture(&input,base,indirect,rotation,None,indirect);
        let original = fs::read(&input).unwrap(); let source = engine.open_document(&input).unwrap();
        let run = engine.list_occurrence_text_runs(&source.id,0).unwrap().runs.remove(0);
        assert!(run.supported,"{name}: {:?}",run.reason);
        let no_op = engine.replace_text_occurrence(&source.id,&[0],0,run.object_index,&run.text,run.source_text.as_deref().unwrap_or(&run.text),None).unwrap();
        assert_eq!(engine.render_page(&source.id,0,720).unwrap(),engine.render_page(&no_op.document.id,0,720).unwrap(),"{name}");
        engine.close_document(&no_op.document.id).unwrap();
        let changed = engine.replace_text_occurrence(&source.id,&[0],0,run.object_index,&run.text,"Add this",None).unwrap();
        roundtrip(&engine,&source,&changed.document,&input,&output,&name,"Add this");
        assert_eq!(fs::read(&input).unwrap(),original);
        engine.close_document(&changed.document.id).unwrap(); engine.close_document(&source.id).unwrap();
    } } }
}

#[test]
fn embedded_equivalent_winansi_keeps_cmap_and_width_proofs_with_or_without_tounicode() {
    let engine = engine(); let temp = tempfile::tempdir().unwrap();
    for unicode in [false,true] { for indirect in [false,true] {
        let name = format!("embedded-{unicode}-{indirect}");
        let input = temp.path().join(format!("{name}.pdf")); let output = temp.path().join(format!("{name}-saved.pdf"));
        fixture(&input,"WinAnsiEncoding",indirect,0,Some(unicode),false);
        let original = fs::read(&input).unwrap(); let source = engine.open_document(&input).unwrap();
        let run = engine.list_occurrence_text_runs(&source.id,0).unwrap().runs.remove(0);
        assert!(run.supported,"{name}: {:?}",run.reason);
        let changed = engine.replace_text_occurrence(&source.id,&[0],0,run.object_index,&run.text,"Add this",None).unwrap();
        roundtrip(&engine,&source,&changed.document,&input,&output,&name,"Add this");
        assert_eq!(fs::read(&input).unwrap(),original);
        engine.close_document(&changed.document.id).unwrap(); engine.close_document(&source.id).unwrap();
    } }
}

#[test]
fn equivalent_encodings_support_paragraph_and_explicit_font_substitution() {
    let engine = engine(); let temp = tempfile::tempdir().unwrap();
    let font = engine.fonts().register(fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
    for embedded in [false,true] {
        let input = temp.path().join(format!("paragraph-{embedded}.pdf"));
        fixture(&input,"WinAnsiEncoding",true,0,embedded.then_some(true),false);
        let original = fs::read(&input).unwrap(); let source = engine.open_document(&input).unwrap();
        let selection = engine.inspect_text_paragraph(&source.id,0,&[0]).unwrap();
        let mut area = selection.bounds.clone(); area.width = 250.; area.height = 50.;
        let changed = engine.preview_text_paragraph(&source.id,0,&[0],&selection.text,"New words",&area).unwrap();
        let output = temp.path().join(format!("paragraph-{embedded}-saved.pdf"));
        roundtrip(&engine,&source,&changed,&input,&output,&format!("paragraph-{embedded}"),"New words");
        engine.close_document(&changed.id).unwrap();
        let run = engine.list_occurrence_text_runs(&source.id,0).unwrap().runs.remove(0);
        let changed = engine.replace_text_occurrence(&source.id,&[0],0,run.object_index,&run.text,"Café",Some(&font.id)).unwrap();
        let output = temp.path().join(format!("substitute-{embedded}-saved.pdf"));
        roundtrip(&engine,&source,&changed.document,&input,&output,&format!("substitute-{embedded}"),"Café");
        engine.close_document(&changed.document.id).unwrap();
        assert_eq!(fs::read(&input).unwrap(),original); engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn encoding_equivalence_does_not_bypass_remapping_or_embedded_width_validation() {
    let engine = engine(); let temp = tempfile::tempdir().unwrap();
    for bad in ["remap","alias","wrong-width","malformed-unicode","symbolic"] {
        let input = temp.path().join(format!("bad-{bad}.pdf"));
        fixture(&input,"WinAnsiEncoding",false,0,Some(bad != "symbolic"),false);
        let mut pdf = Document::load(&input).unwrap();
        let font_id = *pdf.objects.iter().find(|(_,v)|v.as_dict().is_ok_and(|d|d.get(b"Subtype").and_then(Object::as_name).ok()==Some(b"TrueType"))).unwrap().0;
        match bad {
            "remap" | "alias" => {
                let glyph = if bad == "remap" {b"Z".as_slice()} else {b"uni0041"};
                pdf.get_dictionary_mut(font_id).unwrap().set("Encoding",dictionary!{"BaseEncoding"=>"WinAnsiEncoding","Differences"=>vec![65.into(),Object::Name(glyph.to_vec())]});
            }
            "wrong-width" => {
                let widths=pdf.get_dictionary_mut(font_id).unwrap().get_mut(b"Widths").unwrap().as_array_mut().unwrap();
                // A is absent from Old words: reject a bad replacement-glyph proof too.
                widths[65-32]=Object::Real(2000.);
            }
            "malformed-unicode" => {pdf.get_dictionary_mut(font_id).unwrap().set("ToUnicode",Object::Null);}
            "symbolic" => {
                let descriptor=pdf.get_dictionary(font_id).unwrap().get(b"FontDescriptor").unwrap().as_reference().unwrap();
                pdf.get_dictionary_mut(descriptor).unwrap().set("Flags",4);
            }
            _ => unreachable!(),
        }
        pdf.save(&input).unwrap(); let original=fs::read(&input).unwrap();
        let source=engine.open_document(&input).unwrap();
        let run=engine.list_occurrence_text_runs(&source.id,0).unwrap().runs.remove(0);
        assert!(engine.replace_text_occurrence(&source.id,&[0],0,run.object_index,&run.text,"Add this",None).is_err(),"{bad}");
        assert_eq!(fs::read(&input).unwrap(),original); assert_eq!(engine.source_bytes(&source.id).unwrap().as_ref(),original);
        engine.close_document(&source.id).unwrap();
    }
}
