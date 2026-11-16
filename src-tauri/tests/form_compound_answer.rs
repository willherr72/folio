//! Regressions against the original, unrestructured PyMuPDF compound /Tx field.
use folio_engine::{DocumentInfo, ExportRequest, FillFormFieldRequest, FormFieldValue, PagePlan, PdfEngine};
use std::{collections::HashMap, fs, path::{Path, PathBuf}};

const ORIGINAL: &[u8] = include_bytes!("fixtures/form-compound-pymupdf.pdf");

fn unchanged_combo_frame(e:&PdfEngine,original:&image::RgbImage,doc:&DocumentInfo,expect_changed:bool) {
    let current=image::load_from_memory(&e.render_page(&doc.id,0,840).unwrap()).unwrap().to_rgb8();
    assert_eq!(current.dimensions(),(840,600));
    assert_eq!(original.dimensions(),current.dimensions());
    let mut changed=false;
    for (x,y,pixel) in current.enumerate_pixels() {
        // Exact source-owned1pt inset. The complete border stays outside it.
        if (202..698).contains(&x) && (132..268).contains(&y) {
            changed|=pixel!=original.get_pixel(x,y);
        } else {
            assert_eq!(pixel,original.get_pixel(x,y),"combo frame/artwork changed at{x},{y}");
        }
    }
    if expect_changed {assert!(changed,"new answer must visibly change the original appearance");}
}

fn compound_combo_case(bytes:&[u8],label:&str,editable:bool,empty:bool) {
    let dir=tempfile::tempdir().unwrap();let path=dir.path().join("original.pdf");fs::write(&path,bytes).unwrap();
    let e=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
    let source=e.open_document(&path).unwrap();
    let original=image::load_from_memory(&e.render_page(&source.id,0,840).unwrap()).unwrap().to_rgb8();
    let first=e.list_form_fields(&source.id,0).unwrap().remove(0);
    assert!(!first.read_only);assert!(first.reason.is_none());assert_eq!(first.editable_choice,editable);
    assert_eq!(first.options,vec!["Red","Green","Blue"]);
    assert_eq!(first.value,if editable {FormFieldValue::Text{text:(if empty {""}else{"Red"}).into()}}
        else {FormFieldValue::Choice{indices:if empty {vec![]}else{vec![0]}}});
    let mut metadata=serde_json::to_value(&first).unwrap();
    for name in ["value","selectedIndices"] {metadata.as_object_mut().unwrap().remove(name);}
    let outputs=std::env::var_os("FOLIO_COMPOUND_OUTPUTS").map(PathBuf::from);
    if let Some(root)=&outputs {fs::create_dir_all(root).unwrap();let p=root.join(format!("{label}-original.pdf"));assert!(!p.exists());fs::write(p,bytes).unwrap();}
    let mut current=source.clone();let mut sizes=HashMap::new();
    for cycle in 0..3 {
        for (which,text) in ["Custom answer","Another answer","Red"].iter().enumerate() {
            let old=e.list_form_fields(&current.id,0).unwrap().remove(0);
            let wanted=if editable {FormFieldValue::Text{text:(*text).into()}}
                else {FormFieldValue::Choice{indices:vec![[1,2,0][which]]}};
            let filled=e.fill_form_field(&current.id,FillFormFieldRequest{page_index:0,
                annotation_index:old.annotation_index,expected_value:old.value,value:wanted.clone()}).unwrap();
            unchanged_combo_frame(&e,&original,&filled,wanted!=first.value);
            let destination=dir.path().join(format!("{label}-{cycle}-{which}.pdf"));
            let reopened=save(&e,&filled,&destination);
            unchanged_combo_frame(&e,&original,&reopened,wanted!=first.value);
            let field=e.list_form_fields(&reopened.id,0).unwrap().remove(0);assert_eq!(field.value,wanted);
            let mut other=serde_json::to_value(field).unwrap();
            for name in ["value","selectedIndices"] {other.as_object_mut().unwrap().remove(name);}
            assert_eq!(other,metadata,"choice metadata changed");
            let saved=fs::read(&destination).unwrap();let parsed=lopdf::Document::load_mem(&saved).unwrap();
            if cycle==1 {sizes.insert(which,parsed.objects.len());}
            if cycle==2 {assert_eq!(parsed.objects.len(),sizes[&which],"obsolete answer graphs accumulated");}
            if let Some(root)=&outputs {let p=root.join(destination.file_name().unwrap());assert!(!p.exists());fs::write(p,&saved).unwrap();}
            assert_eq!(fs::read(&path).unwrap(),bytes);assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),bytes);
            e.close_document(&filled.id).unwrap();if current.id!=source.id {e.close_document(&current.id).unwrap();}current=reopened;
        }
    }
    e.close_document(&current.id).unwrap();e.close_document(&source.id).unwrap();
}

#[test]
fn compound_combo_populated_producer_fills_and_reopens() {
    compound_combo_case(include_bytes!("fixtures/form-compound-pymupdf-combo.pdf"),"combo",false,false);
}
#[test]
fn compound_combo_empty_producer_fills_and_reopens() {
    compound_combo_case(include_bytes!("fixtures/form-compound-pymupdf-empty-combo.pdf"),"empty-combo",false,true);
}
#[test]
fn compound_combo_editable_populated_producer_fills_and_reopens() {
    compound_combo_case(include_bytes!("fixtures/form-compound-pymupdf-editable-combo.pdf"),"editable-combo",true,false);
}
#[test]
fn compound_combo_editable_empty_producer_fills_and_reopens() {
    compound_combo_case(include_bytes!("fixtures/form-compound-pymupdf-empty-editable-combo.pdf"),"empty-editable-combo",true,true);
}

fn compound_list_case(bytes:&[u8],label:&str,multi:bool,initial:&[usize]) {
    compound_list_case_options(bytes,label,multi,initial,vec!["Red".into(),"Green".into(),"Blue".into()]);
}

fn compound_list_case_options(bytes:&[u8],label:&str,multi:bool,initial:&[usize],options:Vec<String>) {
    let dir=tempfile::tempdir().unwrap();let path=dir.path().join("original.pdf");fs::write(&path,bytes).unwrap();
    let e=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
    let source=e.open_document(&path).unwrap();
    let original=image::load_from_memory(&e.render_page(&source.id,0,840).unwrap()).unwrap().to_rgb8();
    let first=e.list_form_fields(&source.id,0).unwrap().remove(0);
    assert_eq!(first.kind,"list");assert_eq!(first.multi_select,multi);assert!(first.reason.is_none());
    assert_eq!(first.options,options);
    assert_eq!(first.value,FormFieldValue::Choice{indices:initial.to_vec()});
    let mut metadata=serde_json::to_value(&first).unwrap();
    for name in ["value","selectedIndices"] {metadata.as_object_mut().unwrap().remove(name);}
    let outputs=std::env::var_os("FOLIO_COMPOUND_OUTPUTS").map(PathBuf::from);
    if let Some(root)=&outputs {fs::create_dir_all(root).unwrap();let p=root.join(format!("{label}-original.pdf"));assert!(!p.exists());fs::write(p,bytes).unwrap();}
    let values=if multi {vec![vec![1,2],vec![],vec![0,2]]}else{vec![vec![1],vec![],vec![2]]};
    let mut current=source.clone();let mut sizes=HashMap::new();
    for cycle in 0..3 {for (which,indices) in values.iter().enumerate() {
        let old=e.list_form_fields(&current.id,0).unwrap().remove(0);
        let wanted=FormFieldValue::Choice{indices:indices.clone()};
        let filled=e.fill_form_field(&current.id,FillFormFieldRequest{page_index:0,
            annotation_index:old.annotation_index,expected_value:old.value,value:wanted.clone()}).unwrap();
        unchanged_combo_frame(&e,&original,&filled,wanted!=first.value);
        let destination=dir.path().join(format!("{label}-{cycle}-{which}.pdf"));
        let reopened=save(&e,&filled,&destination);unchanged_combo_frame(&e,&original,&reopened,wanted!=first.value);
        let field=e.list_form_fields(&reopened.id,0).unwrap().remove(0);
        assert_eq!(field.value,wanted);assert_eq!(field.selected_indices,*indices);
        let mut other=serde_json::to_value(field).unwrap();
        for name in ["value","selectedIndices"] {other.as_object_mut().unwrap().remove(name);}
        assert_eq!(other,metadata,"list metadata changed");
        let saved=fs::read(&destination).unwrap();let parsed=lopdf::Document::load_mem(&saved).unwrap();
        if cycle==1 {sizes.insert(which,parsed.objects.len());}
        if cycle==2 {assert_eq!(parsed.objects.len(),sizes[&which],"list answer graph grew");}
        if let Some(root)=&outputs {let p=root.join(destination.file_name().unwrap());assert!(!p.exists());fs::write(p,&saved).unwrap();}
        assert_eq!(fs::read(&path).unwrap(),bytes);assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),bytes);
        e.close_document(&filled.id).unwrap();if current.id!=source.id {e.close_document(&current.id).unwrap();}current=reopened;
    }}
    e.close_document(&current.id).unwrap();e.close_document(&source.id).unwrap();
}
#[test]
fn compound_list_original_single_fills_clears_refills_and_reopens() {
    compound_list_case(include_bytes!("fixtures/form-compound-list-original-single.pdf"),"list-original-single",false,&[0]);
}
#[test]
fn compound_list_empty_single_fills_clears_refills_and_reopens() {
    compound_list_case(include_bytes!("fixtures/form-compound-list-empty-single.pdf"),"list-empty-single",false,&[]);
}
#[test]
fn compound_list_multi_one_fills_clears_refills_and_reopens() {
    compound_list_case(include_bytes!("fixtures/form-compound-list-multi-one.pdf"),"list-multi-one",true,&[0]);
}
#[test]
fn compound_list_multi_two_fills_clears_refills_and_reopens() {
    compound_list_case(include_bytes!("fixtures/form-compound-list-multi-two.pdf"),"list-multi-two",true,&[0,2]);
}
#[test]
fn compound_list_source_single_selection_fills_clears_refills_and_reopens() {
    compound_list_case(include_bytes!("fixtures/form-compound-list-single-selection-paint.pdf"),
        "list-source-single-selection",false,&[0]);
}

#[test]
fn compound_list_source_multi_selection_fills_clears_refills_and_reopens() {
    compound_list_case(include_bytes!("fixtures/form-compound-list-multi-selection-paint.pdf"),
        "list-source-multi-selection",true,&[0,2]);
}

#[test]
fn compound_list_source_scrolled_selection_fills_clears_refills_and_reopens() {
    compound_list_case_options(include_bytes!("fixtures/form-compound-list-scrolled-selection-paint.pdf"),
        "list-source-scrolled-selection",false,&[5],
        (1..=15).map(|index|format!("Option {index:02}")).collect());
}

#[test]
fn compound_list_source_custom_art_remains_refused() {
    for bytes in [include_bytes!("fixtures/form-compound-list-ambiguous-artwork.pdf").as_slice()] {
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("original.pdf");fs::write(&path,bytes).unwrap();
        let e=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
        let source=e.open_document(&path).unwrap();let old=e.list_form_fields(&source.id,0).unwrap().remove(0);
        assert!(e.fill_form_field(&source.id,FillFormFieldRequest{page_index:0,annotation_index:old.annotation_index,
            expected_value:old.value,value:FormFieldValue::Choice{indices:vec![1]}}).is_err());
        assert_eq!(fs::read(&path).unwrap(),bytes);assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),bytes);
        e.close_document(&source.id).unwrap();
    }
}

#[test]
fn compound_list_source_invalid_top_hidden_text_partial_art_and_empty_ambiguity_refuse_atomically() {
    use lopdf::{dictionary,Object,Stream};
    for case in ["negative-top","wrong-top","hidden-text","partial-paint","wrong-font","empty-painted",
        "mixed-subpaths","late-paint","raw-step","raw-step-tall"] {
        let raw=if case.ends_with("top"){include_bytes!("fixtures/form-compound-list-scrolled-selection-paint.pdf").as_slice()}
            else{include_bytes!("fixtures/form-compound-list-single-selection-paint.pdf").as_slice()};
        let mut doc=lopdf::Document::load_mem(raw).unwrap();let page=doc.get_pages()[&1];
        let id=doc.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap()[0].as_reference().unwrap();
        let ap=doc.get_dictionary(id).unwrap().get(b"AP").unwrap().as_dict().unwrap();
        let mut normal=doc.dereference(ap.get(b"N").unwrap()).unwrap().1.as_stream().unwrap().clone();
        let body=String::from_utf8(normal.get_plain_content_with_limit(4*1024*1024).unwrap()).unwrap();
        match case {
            "negative-top"=>{doc.get_dictionary_mut(id).unwrap().set("TI",-1);},
            "wrong-top"=>{doc.get_dictionary_mut(id).unwrap().set("TI",0);},
            "hidden-text"=>normal.set_plain_content(body.replace("/Helv 14 Tf","/Helv 14 Tf 3 Tr").into_bytes()),
            "partial-paint"=>normal.set_plain_content(body.replace("1 49.000000 248 16 re","1 49.000000 100 16 re").into_bytes()),
            "wrong-font"=>normal.dict.set("Resources",dictionary!{"Font"=>dictionary!{"Helv"=>dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Symbol"}}}),
            "empty-painted"=>{let owner=doc.get_dictionary_mut(id).unwrap();owner.set("V",Object::string_literal(""));owner.set("I",Vec::<Object>::new());owner.remove(b"DV");},
            "mixed-subpaths"=>normal.set_plain_content(body.replace("q 0.7 0.8 1 rg 1 49.000000 248 16 re f Q",
                "q 0.7 0.8 1 rg 180 15 m 195 15 l 195 30 l h 1 49 248 16 re f n Q").into_bytes()),
            "late-paint"=>{let paint="q 0.7 0.8 1 rg 1 49.000000 248 16 re f Q";
                normal.set_plain_content(body.replace(paint,"").replace("ET\nQ",&format!("ET\n{paint}\nQ")).into_bytes());},
            "raw-step"|"raw-step-tall"=>{
                let mut changed=body.replace("2 68 Td\n0 -15.624001 Td","2 1052.3759765625 Td\n0 -1000 Td")
                    .replace("1 49.000000 248 16 re","1 48 248 21 re");
                if case=="raw-step-tall"{changed=changed.replace(".5 .5 249 69 re",".5 .5 249 249 re")
                    .replace("1 1 248 68 re","1 1 248 248 re").replace("1 48 248 21 re","1 48 248 201 re");
                    normal.dict.set("BBox",vec![0.into(),0.into(),250.into(),250.into()]);}
                normal.set_plain_content(changed.into_bytes());
            },
            _=>unreachable!(),
        }
        let normal=doc.add_object(Stream::new(normal.dict,normal.content));
        doc.get_dictionary_mut(id).unwrap().set("AP",dictionary!{"N"=>normal});
        let mut bytes=Vec::new();doc.save_to(&mut bytes).unwrap();
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("source.pdf");fs::write(&path,&bytes).unwrap();
        let e=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
        let source=e.open_document(&path).unwrap();let old=e.list_form_fields(&source.id,0).unwrap().remove(0);
        let result=e.fill_form_field(&source.id,FillFormFieldRequest{page_index:0,annotation_index:old.annotation_index,
            expected_value:old.value.clone(),value:FormFieldValue::Choice{indices:vec![1]}});
        assert!(result.is_err(),"{case} must not authorize removing source paint");
        assert_eq!(e.list_form_fields(&source.id,0).unwrap()[0].value,old.value,"{case} changed original state");
        assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),bytes);assert_eq!(fs::read(&path).unwrap(),bytes);
        e.close_document(&source.id).unwrap();
    }
}

#[test]
fn compound_list_source_linked_widgets_keep_their_distinct_frames_through_save_and_reedit() {
    use lopdf::dictionary;
    let raw=include_bytes!("fixtures/form-compound-list-scrolled-selection-paint.pdf");
    let mut doc=lopdf::Document::load_mem(raw).unwrap();let page=doc.get_pages()[&1];
    let first=doc.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap()[0].as_reference().unwrap();
    let mut owner=lopdf::Dictionary::new();
    for key in [b"FT".as_slice(),b"Ff",b"T",b"TU",b"Opt",b"V",b"I",b"TI",b"DV",b"DA"]{
        if let Some(value)=doc.get_dictionary_mut(first).unwrap().remove(key){owner.set(key.to_vec(),value);}
    }
    let parent=doc.add_object(owner);doc.get_dictionary_mut(first).unwrap().set("Parent",parent);
    let mut sibling=doc.get_dictionary(first).unwrap().clone();
    sibling.set("Rect",vec![100.into(),40.into(),350.into(),110.into()]);
    let normal=sibling.get(b"AP").unwrap().as_dict().unwrap().get(b"N").unwrap();
    let mut appearance=doc.dereference(normal).unwrap().1.as_stream().unwrap().clone();
    let body=String::from_utf8(appearance.get_plain_content_with_limit(4*1024*1024).unwrap()).unwrap();
    appearance.set_plain_content(body.replace("0 0 0 RG","0.15 0.35 0.6 RG").into_bytes());
    let painted=doc.add_object(appearance);sibling.set("AP",dictionary!{"N"=>painted});
    let second=doc.add_object(sibling);doc.get_dictionary_mut(parent).unwrap().set("Kids",vec![first.into(),second.into()]);
    doc.get_dictionary_mut(page).unwrap().set("Annots",vec![first.into(),second.into()]);
    let form=doc.catalog().unwrap().get(b"AcroForm").unwrap().as_reference().ok();
    if let Some(id)=form{doc.get_dictionary_mut(id).unwrap().set("Fields",vec![parent.into()]);}
    else{doc.catalog_mut().unwrap().get_mut(b"AcroForm").unwrap().as_dict_mut().unwrap().set("Fields",vec![parent.into()]);}
    let mut bytes=Vec::new();doc.save_to(&mut bytes).unwrap();
    let dir=tempfile::tempdir().unwrap();let path=dir.path().join("linked.pdf");fs::write(&path,&bytes).unwrap();
    let e=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
    let source=e.open_document(&path).unwrap();let original=image::load_from_memory(&e.render_page(&source.id,0,840).unwrap()).unwrap().to_rgb8();
    let mut current=source.clone();let values=[vec![1],vec![],vec![2]];let mut sizes=HashMap::new();
    for step in 0..9 {
        let old=e.list_form_fields(&current.id,0).unwrap();assert_eq!(old.len(),2);
        let wanted=FormFieldValue::Choice{indices:values[step%3].clone()};
        let filled=e.fill_form_field(&current.id,FillFormFieldRequest{page_index:0,annotation_index:old[0].annotation_index,
            expected_value:old[0].value.clone(),value:wanted.clone()}).unwrap();
        let destination=dir.path().join(format!("linked-{step}.pdf"));let reopened=save(&e,&filled,&destination);
        for field in e.list_form_fields(&reopened.id,0).unwrap(){assert_eq!(field.value,wanted);}
        let pixels=image::load_from_memory(&e.render_page(&reopened.id,0,840).unwrap()).unwrap().to_rgb8();
        for(x,y,pixel)in pixels.enumerate_pixels(){
            if !((202..698).contains(&x)&&((132..268).contains(&y)||(382..518).contains(&y))){
                assert_eq!(pixel,original.get_pixel(x,y),"linked source frame changed at{x},{y}");
            }
        }
        let output=fs::read(&destination).unwrap();let parsed=lopdf::Document::load_mem(&output).unwrap();
        if step/3==1{sizes.insert(step%3,parsed.objects.len());}
        if step/3==2{assert_eq!(parsed.objects.len(),sizes[&(step%3)],"linked graph grew");}
        if let Some(root)=std::env::var_os("FOLIO_COMPOUND_OUTPUTS"){
            let root=PathBuf::from(root);fs::create_dir_all(&root).unwrap();let target=root.join(format!("list-source-linked-{step}.pdf"));
            assert!(!target.exists());fs::write(target,&output).unwrap();
            let original=root.join("list-source-linked-original.pdf");if step==0{assert!(!original.exists());fs::write(original,&bytes).unwrap();}
        }
        assert_eq!(fs::read(&path).unwrap(),bytes);assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),bytes);
        e.close_document(&filled.id).unwrap();if current.id!=source.id{e.close_document(&current.id).unwrap();}current=reopened;
    }
    e.close_document(&current.id).unwrap();e.close_document(&source.id).unwrap();
}

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
