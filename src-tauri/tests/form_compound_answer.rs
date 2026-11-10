//! Regressions against the original, unrestructured PyMuPDF compound /Tx field.
use folio_engine::{DocumentInfo, ExportRequest, FillFormFieldRequest, FormFieldValue, PagePlan, PdfEngine};
use std::{collections::HashMap, fs, path::{Path, PathBuf}};

const ORIGINAL: &[u8] = include_bytes!("fixtures/form-compound-pymupdf.pdf");

#[test]
fn originally_empty_compound_field_with_background_fills_and_reopens() {
    use lopdf::{content::{Content, Operation}, Object};
    let mut pdf=lopdf::Document::load_mem(ORIGINAL).unwrap();
    let page=pdf.get_pages()[&2];
    let widget=pdf.get_object(page).unwrap().as_dict().unwrap().get(b"Annots").unwrap()
        .as_array().unwrap()[0].as_reference().unwrap();
    let appearance=pdf.get_object(widget).unwrap().as_dict().unwrap().get(b"AP").unwrap()
        .as_dict().unwrap().get(b"N").unwrap().as_reference().unwrap();
    pdf.get_object_mut(widget).unwrap().as_dict_mut().unwrap().set("V",Object::string_literal(""));
    let stream=pdf.get_object_mut(appearance).unwrap().as_stream_mut().unwrap();
    let bytes=stream.decompressed_content().unwrap_or_else(|_|stream.content.clone());
    let mut ops=Content::decode_strict(&bytes).unwrap().operations;
    for op in &mut ops {if op.operator=="Tj" {op.operands[0]=Object::string_literal("");}}
    // Preserve the original frame/clip; add a proved full-box pale background.
    ops.splice(2..2,[Operation::new("g",vec![Object::Real(0.9)]),
        Operation::new("re",vec![0.into(),0.into(),250.into(),30.into()]),Operation::new("f",vec![])]);
    stream.set_plain_content(Content{operations:ops}.encode().unwrap());
    let mut original=Vec::new();pdf.save_to(&mut original).unwrap();
    let dir=tempfile::tempdir().unwrap();let path=dir.path().join("empty.pdf");fs::write(&path,&original).unwrap();
    let e=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
    let source=e.open_document(&path).unwrap();
    let before:Vec<_>=(0..3).map(|i|image::load_from_memory(&e.render_page(&source.id,i,840).unwrap()).unwrap().to_rgb8()).collect();
    let field=e.list_form_fields(&source.id,1).unwrap().remove(0);
    assert_eq!(field.value,FormFieldValue::Text{text:String::new()});
    let wanted=FormFieldValue::Text{text:"Fresh answer".into()};
    let filled=e.fill_form_field(&source.id,FillFormFieldRequest{page_index:1,annotation_index:field.annotation_index,
        expected_value:field.value,value:wanted.clone()}).unwrap();
    unchanged_except_inner_clip(&e,&before,&filled);
    let destination=dir.path().join("filled.pdf");let reopened=save(&e,&filled,&destination);
    unchanged_except_inner_clip(&e,&before,&reopened);
    assert_eq!(e.list_form_fields(&reopened.id,1).unwrap()[0].value,wanted);
    assert_eq!(fs::read(&path).unwrap(),original);
    if let Some(root)=std::env::var_os("FOLIO_COMPOUND_OUTPUTS").map(PathBuf::from) {
        fs::create_dir_all(&root).unwrap();
        for (name,bytes) in [("empty-original.pdf",original),("empty-filled.pdf",fs::read(destination).unwrap())] {
            assert!(!root.join(name).exists());fs::write(root.join(name),bytes).unwrap();
        }
    }
    for doc in [&reopened,&filled,&source] {e.close_document(&doc.id).unwrap();}
}

fn save(e: &PdfEngine, doc: &DocumentInfo, path: &Path) -> DocumentInfo {
    let pages: Vec<PagePlan> = doc.pages.iter().enumerate().map(|(i,p)| {
        serde_json::from_value(serde_json::json!({"id":format!("page-{i}"),"sourceId":doc.id,
            "pageIndex":i,"width":p.width,"height":p.height,"rotation":0,"overlays":p.overlays})).unwrap()
    }).collect();
    e.export_pdf(ExportRequest {pages,flatten:false},path).unwrap();
    e.open_document(path).unwrap()
}

fn unchanged_except_inner_clip(e: &PdfEngine, before: &[image::RgbImage], doc: &DocumentInfo) {
    for (page,original) in before.iter().enumerate() {
        let current=image::load_from_memory(&e.render_page(&doc.id,page,840).unwrap()).unwrap().to_rgb8();
        assert_eq!(current.dimensions(),original.dimensions());
        if page!=1 {assert_eq!(current, *original,"unrelated page changed");continue;}
        assert_eq!(current.dimensions(),(840,600));
        for (x,y,pixel) in current.enumerate_pixels() {
            // Original widget is x100..350,y65..95; its text clip is inset1pt.
            // At2px/pt the actual border is OUTSIDE this exclusion and is tested.
            if !(202..698).contains(&x) || !(132..188).contains(&y) {
                assert_eq!(pixel,original.get_pixel(x,y),"frame/artwork changed at {x},{y}");
            }
        }
    }
}

#[test]
fn original_compound_field_fills_clears_refills_and_reopens_without_art_or_graph_growth() {
    let dir=tempfile::tempdir().unwrap();let original_path=dir.path().join("original.pdf");
    fs::write(&original_path,ORIGINAL).unwrap();
    let e=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
    let source=e.open_document(&original_path).unwrap();
    let before:Vec<_>=(0..3).map(|i|image::load_from_memory(&e.render_page(&source.id,i,840).unwrap()).unwrap().to_rgb8()).collect();
    let mut current=source.clone();let mut sizes=HashMap::new();
    let outputs=std::env::var_os("FOLIO_COMPOUND_OUTPUTS").map(PathBuf::from);
    if let Some(root)=&outputs {fs::create_dir_all(root).unwrap();assert!(!root.join("original.pdf").exists());fs::write(root.join("original.pdf"),ORIGINAL).unwrap();}
    for cycle in 0..3 {
        for (which,value) in ["Printed answer","","Refilled answer"].iter().enumerate() {
            let old=e.list_form_fields(&current.id,1).unwrap().remove(0);
            let wanted=FormFieldValue::Text {text:(*value).into()};
            let filled=e.fill_form_field(&current.id,FillFormFieldRequest {page_index:1,
                annotation_index:old.annotation_index,expected_value:old.value,value:wanted.clone()}).unwrap();
            unchanged_except_inner_clip(&e,&before,&filled);
            assert_eq!(e.list_form_fields(&filled.id,1).unwrap()[0].value,wanted);
            let path=dir.path().join(format!("saved-{cycle}-{which}.pdf"));
            let reopened=save(&e,&filled,&path);unchanged_except_inner_clip(&e,&before,&reopened);
            assert_eq!(e.list_form_fields(&reopened.id,1).unwrap()[0].value,wanted);
            let bytes=fs::read(&path).unwrap();let parsed=lopdf::Document::load_mem(&bytes).unwrap();
            // After one fill/clear/refill transition, repeated identical states
            // must have the same graph size rather than retaining obsolete answers.
            if cycle==1 {sizes.insert(which,parsed.objects.len());}
            if cycle==2 {assert_eq!(parsed.objects.len(),sizes[&which],"answer graph grew on repeated state");}
            if let Some(root)=&outputs {let dest=root.join(path.file_name().unwrap());assert!(!dest.exists());fs::write(dest,&bytes).unwrap();}
            assert_eq!(fs::read(&original_path).unwrap(),ORIGINAL);
            assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),ORIGINAL);
            e.close_document(&filled.id).unwrap();
            if current.id!=source.id {e.close_document(&current.id).unwrap();}
            current=reopened;
        }
    }
    e.close_document(&current.id).unwrap();e.close_document(&source.id).unwrap();
}
