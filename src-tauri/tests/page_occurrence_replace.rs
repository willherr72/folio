use folio_engine::{
    DocumentInfo, ExportRequest, FillFormFieldRequest, FormFieldValue, PagePlan, PdfEngine,
};
use lopdf::{dictionary, Document, Object, Stream};
use std::{fs, path::PathBuf};

// PdfEngine::start shares one registry across this test binary. Font-removing
// cases must not remove a registration another concurrent case still uses.
static FONT_CASE: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn nested_form_fixture(first:&str,rotation:i64,unit:f32)->Document {
    let mut pdf=fixture(false);
    let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica","Encoding"=>"WinAnsiEncoding"});
    let mut wrappers=vec![];
    for text in [first,"Old"] {
        let leaf=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),240.into(),40.into()]},format!("BT /F1 16 Tf 1 0 0 1 5 10 Tm ({text}) Tj ET").into_bytes()));
        wrappers.push(pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),250.into(),50.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font},"XObject"=>dictionary!{"Leaf"=>leaf}}},b"q 1 0 0 1 3 4 cm /Leaf Do Q".to_vec())));
    }
    for (index,id) in pdf.get_pages().values().enumerate() {
        let content=pdf.add_object(Stream::new(dictionary!{},if index==0 {b"q 1 0 0 1 20 260 cm /First Do Q q 0.8 0 0 0.8 20 200 cm /Shared Do Q".to_vec()}else{b"q 1 0 0 1 20 260 cm /Shared Do Q".to_vec()}));
        let page=pdf.get_dictionary_mut(*id).unwrap();page.set("Contents",content);
        page.set("Resources",dictionary!{"Font"=>dictionary!{"F1"=>font},"XObject"=>dictionary!{"First"=>if first=="Old"{wrappers[1]}else{wrappers[0]},"Shared"=>wrappers[1]}});
        page.set("Rotate",rotation);page.set("UserUnit",Object::Real(unit));
    }
    pdf
}

#[test]
fn occurrence_form_connected_baseline() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let source=open(&engine,&temp,"nested-connected.pdf",&mut nested_form_fixture("Old",0,1.));
    assert!(engine.list_form_text_runs(&source.id,0).unwrap().reason.unwrap().contains("annotation"));
    let listed=engine.list_occurrence_form_text_runs(&source.id,0).unwrap();
    assert_eq!(listed.runs.len(),2,"{listed:?}");
    engine.close_document(&source.id).unwrap();
}

#[test]
fn occurrence_form_preserves_appearance_sharing_the_edited_resource_chain(){
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let with_stamp=|first:&str|{
        let mut pdf=nested_form_fixture(first,0,1.);let page=*pdf.get_pages().values().next().unwrap();
        let shared=pdf.get_dictionary(page).unwrap().get(b"Resources").unwrap().as_dict().unwrap().get(b"XObject").unwrap().as_dict().unwrap().get(b"Shared").unwrap().clone();
        let stamp=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Stamp","Rect"=>vec![20.into(),320.into(),220.into(),360.into()],"F"=>4,"AP"=>dictionary!{"N"=>shared},"Contents"=>Object::string_literal("Shared appearance")});
        pdf.get_dictionary_mut(page).unwrap().get_mut(b"Annots").unwrap().as_array_mut().unwrap().push(stamp.into());pdf
    };
    let source=open(&engine,&temp,"shared-appearance.pdf",&mut with_stamp("Old"));
    let reference=open(&engine,&temp,"appearance-reference.pdf",&mut with_stamp("New"));
    let result=engine.replace_form_text_occurrence(&source.id,&[0,1,0],2,&[0,0,0],"Old","New",None).unwrap();
    for width in [300,600,1200]{assert_eq!(engine.render_page(&result.document.id,result.page_indices[2],width).unwrap(),engine.render_page(&reference.id,0,width).unwrap());}
    let file=temp.path().join("appearance-saved.pdf");engine.export_pdf(ExportRequest{pages:plans(&result.document,&result.page_indices),flatten:false},&file).unwrap();
    let saved=engine.open_document(&file).unwrap();
    assert_eq!(engine.render_page(&saved.id,2,600).unwrap(),engine.render_page(&reference.id,0,600).unwrap());
    assert_eq!(engine.render_page(&saved.id,0,600).unwrap(),engine.render_page(&source.id,0,600).unwrap());
    for id in [&source.id,&reference.id,&result.document.id,&saved.id]{engine.close_document(id).unwrap();}
}

#[test]
fn occurrence_form_retains_original_restrictions_and_other_page_annotations(){
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    for restriction in ["StructTreeRoot","OCProperties","StructParent"] {
        let mut pdf=nested_form_fixture("Old",0,1.);
        if restriction=="StructParent" {let page=*pdf.get_pages().values().nth(1).unwrap();pdf.get_dictionary_mut(page).unwrap().set(restriction,0);}
        else{pdf.catalog_mut().unwrap().set(restriction,dictionary!{});}
        let source=open(&engine,&temp,"restricted.pdf",&mut pdf);
        assert!(engine.list_occurrence_form_text_runs(&source.id,0).unwrap().reason.unwrap().contains("Tagged"));
        assert!(engine.replace_form_text_occurrence(&source.id,&[0,1],0,&[0,0,0],"Old","New",None).is_err());engine.close_document(&source.id).unwrap();
    }
    let mut pdf=nested_form_fixture("Old",0,1.);
    for id in pdf.get_pages().values(){pdf.get_dictionary_mut(*id).unwrap().remove(b"Annots");}
    pdf.catalog_mut().unwrap().remove(b"AcroForm");pdf.prune_objects();
    let other=*pdf.get_pages().values().nth(1).unwrap();
    let note=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![240.into(),50.into(),258.into(),68.into()],"Contents"=>Object::string_literal("Other page note")});
    pdf.get_dictionary_mut(other).unwrap().set("Annots",vec![Object::Reference(note)]);
    let source=open(&engine,&temp,"other-page-note.pdf",&mut pdf);let original=engine.source_bytes(&source.id).unwrap();
    let changed=engine.replace_form_text_occurrence(&source.id,&[0,1],0,&[0,0,0],"Old","New",None).unwrap();
    assert_eq!(engine.render_page(&source.id,1,600).unwrap(),engine.render_page(&changed.document.id,1,600).unwrap());
    for (order,selected,path,expected,text) in [(vec![],0,vec![0,0,0],"Old","New"),(vec![0,9],0,vec![0,0,0],"Old","New"),(vec![0,1],2,vec![0,0,0],"Old","New"),(vec![0,1],0,vec![9,0,0],"Old","New"),(vec![0,1],0,vec![0,0,0],"stale","New"),(vec![0,1],0,vec![0,0,0],"Old","")] {
        assert!(engine.replace_form_text_occurrence(&source.id,&order,selected,&path,expected,text,None).is_err());
    }
    assert_eq!(engine.source_bytes(&source.id).unwrap(),original);
    for id in [&source.id,&changed.document.id]{engine.close_document(id).unwrap();}
}

#[test]
fn occurrence_form_protected_credentials_and_duplicate_permissions(){
    use lopdf::encryption::Permissions as P;
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    for (name,rights,edit,duplicate) in [("edit",P::MODIFIABLE,true,false),("both",P::MODIFIABLE|P::ASSEMBLABLE,true,true),("assemble",P::ASSEMBLABLE,false,false)] {
        let mut pdf=nested_form_fixture("Old",0,1.);let mut bytes=vec![];pdf.save_to(&mut bytes).unwrap();
        let bytes=protected_fixture::encrypt(&bytes,rights);let path=temp.path().join(format!("{name}.pdf"));fs::write(&path,&bytes).unwrap();
        let source=engine.open_document_with_password(&path,"reader").unwrap();
        for (order,selected,allowed) in [(vec![0,1],0,edit),(vec![0,1,0],2,duplicate)] {
            let result=engine.replace_form_text_occurrence(&source.id,&order,selected,&[0,0,0],"Old","New",None);
            if !allowed{assert!(result.is_err());continue;}
            let result=result.unwrap();assert!(engine.extract_text(&result.document.id,result.page_indices[selected]).is_err());
            let saved=temp.path().join("protected-edited.pdf");engine.export_pdf(ExportRequest{pages:plans(&result.document,&result.page_indices),flatten:false},&saved).unwrap();
            for password in ["reader","owner"]{let opened=engine.open_document_with_password(&saved,password).unwrap();assert_eq!(engine.list_occurrence_form_text_runs(&opened.id,selected).unwrap().runs[0].text,"New");engine.close_document(&opened.id).unwrap();}
            engine.close_document(&result.document.id).unwrap();
        }
        assert_eq!(fs::read(path).unwrap(),bytes);engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn occurrence_form_substitution_saved_reedit_and_noop_preserve_other_invocations(){
    let _font_case=FONT_CASE.lock().unwrap();let engine=engine();let temp=tempfile::tempdir().unwrap();
    let font=engine.fonts().register(fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
    let source=open(&engine,&temp,"form-font.pdf",&mut nested_form_fixture("Old",0,1.7));
    let result=engine.replace_form_text_occurrence(&source.id,&[0,1,0],2,&[0,0,0],"Old","New",Some(&font.id)).unwrap();
    let file=temp.path().join("font-saved.pdf");engine.export_pdf(ExportRequest{pages:plans(&result.document,&result.page_indices),flatten:false},&file).unwrap();
    engine.fonts().remove(&font.id).unwrap();let saved=engine.open_document(&file).unwrap();
    let selected=engine.list_occurrence_form_text_runs(&saved.id,2).unwrap();assert_eq!(selected.runs[0].text,"New");assert!(selected.runs[0].is_embedded);
    let replay=engine.replace_form_text_occurrence(&saved.id,&[0,1,2],2,&selected.runs[0].object_path,"New","New",None).unwrap();
    for page in 0..3{assert_eq!(engine.render_page(&saved.id,page,600).unwrap(),engine.render_page(&replay.document.id,page,600).unwrap());}
    let edited=engine.replace_form_text_occurrence(&saved.id,&[0,1,2],2,&selected.runs[0].object_path,"New","New New",None).unwrap();
    for page in [0,1]{assert_eq!(engine.render_page(&saved.id,page,600).unwrap(),engine.render_page(&edited.document.id,page,600).unwrap());}
    assert_eq!(engine.list_occurrence_form_text_runs(&edited.document.id,2).unwrap().runs[1].text,"Old");
    for id in [&source.id,&result.document.id,&saved.id,&replay.document.id,&edited.document.id]{engine.close_document(id).unwrap();}
}

#[test]
fn occurrence_form_rejects_new_annotation_overlap() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();let mut pdf=nested_form_fixture("Old",0,1.);
    let first=*pdf.get_pages().values().next().unwrap();
    let annotation=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Link","Rect"=>vec![70.into(),270.into(),115.into(),295.into()],"Border"=>vec![0.into(),0.into(),0.into()],"Dest"=>vec![Object::Reference(first),Object::Name(b"Fit".to_vec())]});
    pdf.get_dictionary_mut(first).unwrap().get_mut(b"Annots").unwrap().as_array_mut().unwrap().push(annotation.into());
    let source=open(&engine,&temp,"nested-collision.pdf",&mut pdf);let bytes=engine.source_bytes(&source.id).unwrap();
    let runs=engine.list_occurrence_form_text_runs(&source.id,0).unwrap();let path=&runs.runs[0].object_path;
    let error=engine.replace_form_text_occurrence(&source.id,&[0,1],0,path,"Old","Longer words",None).unwrap_err();
    assert!(error.to_string().contains("annotation"),"{error}");
    let changed=engine.replace_form_text_occurrence(&source.id,&[0,1],0,path,"Old","New",None).unwrap();
    assert_eq!(engine.source_bytes(&source.id).unwrap(),bytes);
    for id in [&source.id,&changed.document.id]{engine.close_document(id).unwrap();}
}

#[test]
#[ignore = "diagnostic for the retained annotation resource reproduction"]
fn occurrence_form_flat_resource_probe() {
    let root=PathBuf::from(std::env::var_os("FOLIO_FORM_VALIDATION_PROBE").unwrap());let engine=engine();
    for name in ["original","flat","flat-restored","unannotated"] {
        if name=="flat-restored" {
            let mut flat=Document::load(root.join("flat.pdf")).unwrap();
            let page=*flat.get_pages().values().next().unwrap();
            let resources=flat.get_dictionary_mut(page).unwrap().get_mut(b"Resources").unwrap().as_dict_mut().unwrap();
            let fonts=resources.get_mut(b"Font").unwrap().as_dict_mut().unwrap();
            let font=fonts.get(b"FolioFlat0").unwrap().clone();fonts.set("F1",font);
            flat.save(root.join("flat-restored.pdf")).unwrap();
        }
        if name=="unannotated" {
            let mut pdf=Document::load(root.join("original.pdf")).unwrap();
            for id in pdf.get_pages().values(){pdf.get_dictionary_mut(*id).unwrap().remove(b"Annots");}
            pdf.catalog_mut().unwrap().remove(b"AcroForm");pdf.prune_objects();pdf.save(root.join("unannotated.pdf")).unwrap();
        }
        let source=engine.open_document(root.join(format!("{name}.pdf"))).unwrap();
        if name=="unannotated"{eprintln!("unannotated listing {:?}",engine.list_form_text_runs(&source.id,0).unwrap());}
        for width in [1000,1200,600] {let bytes=engine.render_page(&source.id,0,width).unwrap();fs::write(root.join(format!("native-{name}-{width}.png")),bytes).unwrap();}engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn occurrence_forms_preserve_shared_invocations_fields_and_links() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    for rotation in [0,90,180,270] {for unit in [1.,1.7,2.] {
        let source=open(&engine,&temp,"nested-source.pdf",&mut nested_form_fixture("Old",rotation,unit));
        let original=engine.source_bytes(&source.id).unwrap();
        let selected_path=engine.list_occurrence_form_text_runs(&source.id,0).unwrap().runs[0].object_path.clone();
        for (label,order,selected) in [("unique",vec![0,1],0),("first",vec![0,1,0],0),("later",vec![0,1,0],2)] {for replacement in ["Old","New"] {
            let result=engine.replace_form_text_occurrence(&source.id,&order,selected,&selected_path,"Old",replacement,None).unwrap();
            let reference=open(&engine,&temp,"nested-reference.pdf",&mut nested_form_fixture(replacement,rotation,unit));
            assert_eq!(result.document.bookmarks,source.bookmarks);
            for (i,old) in order.iter().enumerate() {for width in [300,600] {
                let expected=if i==selected {engine.render_page(&reference.id,0,width).unwrap()}else{engine.render_page(&source.id,*old,width).unwrap()};
                assert_eq!(engine.render_page(&result.document.id,result.page_indices[i],width).unwrap(),expected,"r{rotation} u{unit} {label} {replacement} page{i}");
            }}
            let path=temp.path().join("nested-saved.pdf");engine.export_pdf(ExportRequest{pages:plans(&result.document,&result.page_indices),flatten:false},&path).unwrap();
            verify_links(&Document::load(&path).unwrap(),&order);let saved=engine.open_document(&path).unwrap();
            let runs=engine.list_occurrence_form_text_runs(&saved.id,selected).unwrap();assert_eq!(runs.runs.len(),2,"{runs:?}");
            assert_eq!(runs.runs[0].text,replacement);assert_eq!(runs.runs[1].text,"Old");assert!((runs.runs[0].font_size-16.*unit).abs()<0.001);
            let field=engine.list_form_fields(&saved.id,selected).unwrap().remove(0);
            let filled=engine.fill_form_field(&saved.id,FillFormFieldRequest{page_index:selected,annotation_index:field.annotation_index,expected_value:field.value,value:FormFieldValue::Text{text:"Form linked".into()}}).unwrap();
            for page in 0..filled.pages.len(){assert_eq!(engine.list_form_fields(&filled.id,page).unwrap()[0].value,FormFieldValue::Text{text:"Form linked".into()});}
            let filled_path=temp.path().join("nested-filled.pdf");engine.export_pdf(ExportRequest{pages:plans(&filled,&(0..filled.pages.len()).collect::<Vec<_>>()),flatten:false},&filled_path).unwrap();
            capture(&format!("form-r{rotation}-u{unit}-{label}-{}",if replacement=="Old"{"noop"}else{"edit"}),&[("source",&original),("result",&fs::read(&path).unwrap()),("reference",&engine.source_bytes(&reference.id).unwrap()),("filled",&fs::read(&filled_path).unwrap())]);
            for id in [&result.document.id,&reference.id,&saved.id,&filled.id]{engine.close_document(id).unwrap();}
        }}
        assert_eq!(engine.source_bytes(&source.id).unwrap(),original);engine.close_document(&source.id).unwrap();
    }}
}

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}

fn group_fixture(rotation:i64,unit:f32)->Document {
    let mut pdf=fixture(false);let page=*pdf.get_pages().values().next().unwrap();
    let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
    let stream=pdf.add_object(Stream::new(dictionary!{},b"BT /F1 12 Tf 50 240 Td (Old) Tj ( words) Tj 0 110 Td (Neighbor) Tj ET".to_vec()));
    let dict=pdf.get_dictionary_mut(page).unwrap();dict.set("Contents",stream);dict.set("Resources",dictionary!{"Font"=>dictionary!{"F1"=>font}});dict.set("Rotate",rotation);dict.set("UserUnit",Object::Real(unit));pdf
}

#[test]
fn occurrence_group_shared_field_baseline() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();let source=open(&engine,&temp,"group-shared.pdf",&mut group_fixture(0,1.));
    assert!(engine.inspect_text_group(&source.id,0,&[0,1]).unwrap_err().to_string().contains("shared fields"));
    let selection=engine.inspect_occurrence_text_group(&source.id,0,&[0,1]).unwrap();
    assert_eq!(selection.text,"Old words");
}

#[test]
fn occurrence_groups_preserve_connected_documents_and_physical_pages() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    for rotation in [0,90,180,270] {for unit in [1.,1.7,2.] {
        let pdf=group_fixture(rotation,unit);let source=open(&engine,&temp,"group-source.pdf",&mut pdf.clone());
        let original=engine.source_bytes(&source.id).unwrap();let reference_source=open_paragraph_reference(&engine,&temp,"reference.pdf",pdf);
        let selection=engine.inspect_occurrence_text_group(&source.id,0,&[0,1]).unwrap();
        assert_eq!(selection.text,"Old words");assert!((selection.font_size-12.*unit).abs()<0.001);
        for (label,order,selected) in [("unique",vec![0,1],0),("first",vec![0,1,0],0),("later",vec![0,1,0],2)] {for replacement in ["Old words","New text"] {
            let result=engine.replace_text_group_occurrence(&source.id,&order,selected,&[0,1],"Old words",replacement).unwrap();
            let reference=engine.replace_text_group(&reference_source.id,0,&[0,1],"Old words",replacement).unwrap();
            assert_eq!(result.document.bookmarks,source.bookmarks);
            for (i,old) in order.iter().enumerate() {for width in [300,600] {
                let expected=if i==selected {engine.render_page(&reference.id,0,width).unwrap()}else{engine.render_page(&source.id,*old,width).unwrap()};
                assert_eq!(engine.render_page(&result.document.id,result.page_indices[i],width).unwrap(),expected,"r{rotation} u{unit} {label} {replacement} page{i}");
            }}
            let path=temp.path().join("group-saved.pdf");engine.export_pdf(ExportRequest{pages:plans(&result.document,&result.page_indices),flatten:false},&path).unwrap();
            verify_links(&Document::load(&path).unwrap(),&order);let saved=engine.open_document(&path).unwrap();
            let actual_text=engine.extract_text(&saved.id,selected).unwrap();
            assert_eq!(actual_text,engine.extract_text(&reference.id,0).unwrap(),"r{rotation} u{unit} {label} {replacement}");
            // Raw extraction of the unchanged 180-degree split line reverses its
            // two objects in both source and reference. The group inspector must
            // still reconstruct the exact logical selection without changing it.
            if replacement=="Old words"{assert_eq!(engine.inspect_occurrence_text_group(&saved.id,selected,&[0,1]).unwrap().text,replacement);}
            else{assert!(actual_text.contains(replacement));}
            let field=engine.list_form_fields(&saved.id,selected).unwrap().remove(0);
            let filled=engine.fill_form_field(&saved.id,FillFormFieldRequest{page_index:selected,annotation_index:field.annotation_index,expected_value:field.value,value:FormFieldValue::Text{text:"Group linked".into()}}).unwrap();
            for page in 0..filled.pages.len(){assert_eq!(engine.list_form_fields(&filled.id,page).unwrap()[0].value,FormFieldValue::Text{text:"Group linked".into()});}
            let filled_path=temp.path().join("group-filled.pdf");engine.export_pdf(ExportRequest{pages:plans(&filled,&(0..filled.pages.len()).collect::<Vec<_>>()),flatten:false},&filled_path).unwrap();
            capture(&format!("group-r{rotation}-u{unit}-{label}-{}",if replacement=="Old words"{"noop"}else{"edit"}),&[("source",&original),("result",&fs::read(&path).unwrap()),("reference",&engine.source_bytes(&reference.id).unwrap()),("filled",&fs::read(&filled_path).unwrap())]);
            for id in [&result.document.id,&reference.id,&saved.id,&filled.id]{engine.close_document(id).unwrap();}
        }}
        assert_eq!(engine.source_bytes(&source.id).unwrap(),original);engine.close_document(&source.id).unwrap();engine.close_document(&reference_source.id).unwrap();
    }}
}

#[test]
fn occurrence_groups_reject_stale_indices_and_original_document_restrictions() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();let source=open(&engine,&temp,"group-refusal.pdf",&mut group_fixture(0,1.));let original=engine.source_bytes(&source.id).unwrap();
    for (order,selected,indices,expected,replacement) in [(vec![],0,vec![0,1],"Old words","New"),(vec![0,1],2,vec![0,1],"Old words","New"),(vec![0,99],0,vec![0,1],"Old words","New"),(vec![0,1],0,vec![0,0],"Old words","New"),(vec![0,1],0,vec![0,1],"stale","New"),(vec![0,1],0,vec![0,1],"Old words","This replacement is much too long to fit within this page")]{
        assert!(engine.replace_text_group_occurrence(&source.id,&order,selected,&indices,expected,replacement).is_err());
    }
    assert_eq!(engine.source_bytes(&source.id).unwrap(),original);
    for tag in ["StructTreeRoot","OCProperties"] {
        let mut pdf=group_fixture(0,1.);pdf.catalog_mut().unwrap().set(tag,dictionary!{});let tagged=open(&engine,&temp,&format!("{tag}.pdf"),&mut pdf);
        assert!(engine.inspect_occurrence_text_group(&tagged.id,0,&[0,1]).is_err());assert!(engine.replace_text_group_occurrence(&tagged.id,&[0,1],0,&[0,1],"Old words","New").is_err());engine.close_document(&tagged.id).unwrap();
    }
    engine.close_document(&source.id).unwrap();
}

#[test]
fn occurrence_groups_preserve_original_rights_and_credentials() {
    use lopdf::encryption::Permissions as P;
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    for (name,rights,edit,duplicate) in [("edit",P::MODIFIABLE,true,false),("both",P::MODIFIABLE|P::ASSEMBLABLE,true,true),("assemble",P::ASSEMBLABLE,false,false)] {
        let mut plain=Vec::new();group_fixture(0,1.7).save_to(&mut plain).unwrap();let encrypted=protected_fixture::encrypt(&plain,rights);let path=temp.path().join(format!("{name}.pdf"));fs::write(&path,&encrypted).unwrap();
        let source=engine.open_document_with_password(&path,"reader").unwrap();
        assert_eq!(engine.inspect_occurrence_text_group(&source.id,0,&[0,1]).is_ok(),edit);
        for (order,selected,allowed) in [(vec![0,1],0,edit),(vec![0,1,0],2,duplicate)] {
            let result=engine.replace_text_group_occurrence(&source.id,&order,selected,&[0,1],"Old words","New text");
            if !allowed {assert!(matches!(result,Err(folio_engine::EngineError::PermissionDenied(_))));continue;}
            let result=result.unwrap();assert_eq!(result.document.protection,source.protection);
            assert!(matches!(engine.extract_text(&result.document.id,result.page_indices[selected]),Err(folio_engine::EngineError::PermissionDenied(_))));
            let output=temp.path().join(format!("{name}-{}.pdf",order.len()));engine.export_pdf(ExportRequest{pages:plans(&result.document,&result.page_indices),flatten:false},&output).unwrap();
            let saved=engine.open_document_with_password(&output,"reader").unwrap();assert_eq!(saved.protection.unwrap().permissions,source.protection.as_ref().unwrap().permissions);
            let owner=engine.open_document_with_password(&output,"owner").unwrap();assert!(engine.extract_text(&owner.id,selected).unwrap().contains("New text"));
            for id in [&result.document.id,&saved.id,&owner.id]{engine.close_document(id).unwrap();}
        }
        assert_eq!(fs::read(path).unwrap(),encrypted);engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn occurrence_paragraph_reflows_shared_fields_and_replays_only_the_selected_copy() {
    use folio_engine::{AnnotationRect,ParagraphFormatting,ParagraphAlignment,StyledParagraphLayoutRequest};
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let source=open(&engine,&temp,"paragraph-shared.pdf",&mut fixture(false));
    let reference_source=open_paragraph_reference(&engine,&temp,"paragraph-reference.pdf",fixture(false));
    let original=engine.source_bytes(&source.id).unwrap();
    let area=AnnotationRect{x:40.,y:140.,width:200.,height:90.};
    assert!(engine.inspect_text_paragraph(&source.id,0,&[0]).unwrap_err().to_string().contains("shared fields"));
    for styled in [false,true] {for (label,order,selected) in [("unique",vec![0,1],0),("first",vec![0,1,0],0),("later",vec![0,1,0],2)] {
        let replacement="One paragraph\nSecond line";
        let (result,reference)=if styled {
            let selection=engine.inspect_occurrence_styled_paragraph(&source.id,0,&[0]).unwrap();
            let suggested=engine.suggest_occurrence_styled_paragraph(&source.id,0,0,"Old content").unwrap();
            assert_eq!(selection.object_indices,suggested.object_indices);
            let request:StyledParagraphLayoutRequest=serde_json::from_value(serde_json::json!({"objectIndices":[0],"expectedText":"Old content","runs":[{"text":replacement,"styleId":0}],"bounds":area,"alignment":"right","lineSpacing":1.4})).unwrap();
            engine.layout_occurrence_styled_paragraph(&source.id,0,&request).unwrap();
            engine.fit_occurrence_styled_paragraph(&source.id,0,&request).unwrap();
            (engine.preview_styled_paragraph_occurrence(&source.id,&order,selected,&request).unwrap(),engine.preview_styled_paragraph(&reference_source.id,0,&request).unwrap())
        }else{
            let selection=engine.inspect_occurrence_text_paragraph(&source.id,0,&[0]).unwrap();
            let suggested=engine.suggest_occurrence_text_paragraph(&source.id,0,0,"Old content").unwrap();
            assert_eq!(selection.object_indices,suggested.object_indices);
            let formatting=ParagraphFormatting{alignment:Some(ParagraphAlignment::Right),line_spacing:Some(1.4),..Default::default()};
            (engine.preview_text_paragraph_occurrence(&source.id,&order,selected,&[0],"Old content",replacement,&area,&formatting).unwrap(),engine.preview_text_paragraph_with_format(&reference_source.id,0,&[0],"Old content",replacement,&area,&formatting).unwrap())
        };
        assert_eq!(result.document.bookmarks,source.bookmarks);
        let target=result.page_indices[selected];
        let runs=engine.list_occurrence_text_runs(&result.document.id,target).unwrap().runs;
        let indices=runs.iter().map(|r|r.object_index).collect::<Vec<_>>();
        let replay=if styled {
            let s=engine.inspect_occurrence_styled_paragraph(&result.document.id,target,&indices).unwrap();
            assert_eq!(s.text,replacement);assert_eq!(s.alignment,Some(ParagraphAlignment::Right));
            let request:StyledParagraphLayoutRequest=serde_json::from_value(serde_json::json!({"objectIndices":indices,"expectedText":s.text,"runs":s.runs,"bounds":s.bounds,"alignment":s.alignment,"lineSpacing":s.line_spacing})).unwrap();
            engine.preview_styled_paragraph_occurrence(&result.document.id,&result.page_indices,selected,&request).unwrap()
        }else{
            let s=engine.inspect_occurrence_text_paragraph(&result.document.id,target,&indices).unwrap();
            assert_eq!(s.text,replacement);assert_eq!(s.alignment,Some(ParagraphAlignment::Right));
            engine.preview_text_paragraph_occurrence(&result.document.id,&result.page_indices,selected,&indices,&s.text,&s.text,&s.bounds,&ParagraphFormatting::default()).unwrap()
        };
        assert_eq!(replay.document.pages.len(),result.document.pages.len());
        for (i,old) in order.iter().enumerate() {for width in [300,600] {
            let expected=if i==selected {engine.render_page(&result.document.id,target,width).unwrap()}else{engine.render_page(&source.id,*old,width).unwrap()};
            assert_eq!(engine.render_page(&replay.document.id,replay.page_indices[i],width).unwrap(),expected);
        }}
        let path=temp.path().join(format!("paragraph-{styled}-{label}.pdf"));
        engine.export_pdf(ExportRequest{pages:plans(&replay.document,&replay.page_indices),flatten:false},&path).unwrap();
        verify_links(&Document::load(&path).unwrap(),&order);
        let reopened=engine.open_document(&path).unwrap();
        assert!(engine.extract_text(&reopened.id,selected).unwrap().contains("One paragraph"));
        let field=engine.list_form_fields(&reopened.id,selected).unwrap().remove(0);
        let filled=engine.fill_form_field(&reopened.id,FillFormFieldRequest{page_index:selected,annotation_index:field.annotation_index,expected_value:field.value,value:FormFieldValue::Text{text:"Paragraph linked".into()}}).unwrap();
        for page in 0..filled.pages.len(){assert_eq!(engine.list_form_fields(&filled.id,page).unwrap()[0].value,FormFieldValue::Text{text:"Paragraph linked".into()});}
        let filled_path=temp.path().join(format!("paragraph-{styled}-{label}-filled.pdf"));
        engine.export_pdf(ExportRequest{pages:plans(&filled,&(0..filled.pages.len()).collect::<Vec<_>>()),flatten:false},&filled_path).unwrap();
        let filled_saved=engine.open_document(&filled_path).unwrap();
        for page in 0..filled_saved.pages.len(){assert_eq!(engine.list_form_fields(&filled_saved.id,page).unwrap()[0].value,FormFieldValue::Text{text:"Paragraph linked".into()});}
        capture(&format!("paragraph-{styled}-{label}"),&[("source",&original),("result",&fs::read(&path).unwrap()),("reference",&engine.source_bytes(&reference.id).unwrap()),("filled",&fs::read(&filled_path).unwrap())]);
        for id in [&result.document.id,&replay.document.id,&reopened.id,&filled.id,&filled_saved.id,&reference.id]{engine.close_document(id).unwrap();}
    }}
    assert_eq!(engine.source_bytes(&source.id).unwrap(),original);
    engine.close_document(&source.id).unwrap();
    engine.close_document(&reference_source.id).unwrap();
}

#[test]
fn occurrence_paragraph_keeps_physical_frames_highlights_fonts_and_blank_lines() {
    let _font_case=FONT_CASE.lock().unwrap();
    use folio_engine::{AnnotationRect,ParagraphFormatting,ParagraphAlignment,StyledParagraphLayoutRequest};
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let font=engine.fonts().register(fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
    for styled in [false,true] {for rotation in [0,90,180,270] {for unit in [1.,1.7,2.] {
        let mut pdf=fixture(false);let page=*pdf.get_pages().values().next().unwrap();
        let data=if styled {"q 1 1 0 rg 70 236 21 16 re f Q BT /F1 12 Tf 50 240 Td (AA ) Tj ET BT /F2 12 Tf 72 240 Td (BB) Tj ET q 0 .5 0 rg BT /F1 12 Tf 90 240 Td ( CC) Tj ET Q BT /F1 12 Tf 50 350 Td (Neighbor) Tj ET"}else{"BT /F1 12 Tf 50 240 Td (Old content) Tj ET BT /F1 12 Tf 50 350 Td (Neighbor) Tj ET"};
        let content=pdf.add_object(Stream::new(dictionary!{},data.as_bytes().to_vec()));
        let f1=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
        let f2=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier-Bold"});
        let dict=pdf.get_dictionary_mut(page).unwrap();dict.set("Contents",content);
        dict.set("Resources",dictionary!{"Font"=>dictionary!{"F1"=>f1,"F2"=>f2}});
        dict.set("MediaBox",vec![(-20).into(),(-10).into(),300.into(),400.into()]);
        dict.set("CropBox",vec![10.into(),20.into(),290.into(),380.into()]);dict.set("Rotate",rotation);dict.set("UserUnit",Object::Real(unit));
        let name=format!("paragraph-physical-{styled}-r{rotation}-u{unit}");
        let source=open(&engine,&temp,&format!("{name}.pdf"),&mut pdf);
        let reference_source=open_paragraph_reference(&engine,&temp,&format!("{name}-reference.pdf"),pdf.clone());
        let original=engine.source_bytes(&source.id).unwrap();
        let b=match rotation {0=>[40.,120.,180.,110.],90=>[130.,40.,110.,180.],180=>[60.,130.,180.,110.],_=>[120.,60.,110.,180.]};
        let area=AnnotationRect{x:b[0]*unit,y:b[1]*unit,width:b[2]*unit,height:b[3]*unit};
        let (result,reference)=if styled {
            let s=engine.inspect_occurrence_styled_paragraph(&source.id,0,&[1,2,3]).unwrap();
            assert_eq!(s.styles.len(),3);assert_eq!(s.styles[1].background_color.as_deref(),Some("#FFFF00"));
            let request:StyledParagraphLayoutRequest=serde_json::from_value(serde_json::json!({"objectIndices":[1,2,3],"expectedText":s.text,"runs":[{"text":"AA ","styleId":0},{"text":"BB expanded","styleId":1},{"text":" CC\n\nLast line","styleId":2}],"bounds":area,"alignment":"center","lineSpacing":1.5,"styleChanges":[{"styleId":1,"sourceStyleId":1,"fontId":font.id,"fontSize":16.*unit,"color":"#993366"}]})).unwrap();
            (engine.preview_styled_paragraph_occurrence(&source.id,&[0,1,0],2,&request).unwrap(),engine.preview_styled_paragraph(&reference_source.id,0,&request).unwrap())
        }else{
            let formatting=ParagraphFormatting{font_size:Some(12.*unit),alignment:Some(ParagraphAlignment::Center),line_spacing:Some(1.5),font_id:Some(font.id.clone()),..Default::default()};
            (engine.preview_text_paragraph_occurrence(&source.id,&[0,1,0],2,&[0],"Old content","First line\n\nLast line",&area,&formatting).unwrap(),engine.preview_text_paragraph_with_format(&reference_source.id,0,&[0],"Old content","First line\n\nLast line",&area,&formatting).unwrap())
        };
        assert_eq!(result.document.pages[2],source.pages[0]);
        let path=temp.path().join(format!("{name}-saved.pdf"));
        engine.export_pdf(ExportRequest{pages:plans(&result.document,&result.page_indices),flatten:false},&path).unwrap();
        verify_links(&Document::load(&path).unwrap(),&[0,1,0]);
        let reopened=engine.open_document(&path).unwrap();
        let indices=engine.list_occurrence_text_runs(&reopened.id,2).unwrap().runs.into_iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect::<Vec<_>>();
        let replay=if styled {
            let s=engine.inspect_occurrence_styled_paragraph(&reopened.id,2,&indices).unwrap();
            assert_eq!(s.text,"AA BB expanded CC\n\nLast line");assert_physical_rect(&s.bounds,&area);
            assert!(s.styles.iter().any(|style|style.background_color.as_deref()==Some("#FFFF00")&&style.color=="#993366"&&(style.font_size-16.*unit).abs()<0.001));
            let changes=s.styles.iter().map(|s|serde_json::json!({"styleId":s.style_id,"sourceStyleId":s.style_id,"fontSize":s.font_size})).collect::<Vec<_>>();
            let request=serde_json::from_value(serde_json::json!({"objectIndices":indices,"expectedText":s.text,"runs":s.runs,"bounds":s.bounds,"alignment":s.alignment,"lineSpacing":s.line_spacing,"styleChanges":changes})).unwrap();
            engine.preview_styled_paragraph_occurrence(&reopened.id,&[0,1,2],2,&request).unwrap()
        }else{
            let s=engine.inspect_occurrence_text_paragraph(&reopened.id,2,&indices).unwrap();
            assert_eq!(s.text,"First line\n\nLast line");assert_physical_rect(&s.bounds,&area);
            engine.preview_text_paragraph_occurrence(&reopened.id,&[0,1,2],2,&indices,&s.text,&s.text,&s.bounds,&ParagraphFormatting{font_size:Some(s.font_size),..Default::default()}).unwrap()
        };
        for width in [300,600] {
            assert_eq!(engine.render_page(&replay.document.id,2,width).unwrap(),engine.render_page(&result.document.id,2,width).unwrap(),"{name} replay");
            assert_eq!(engine.render_page(&replay.document.id,0,width).unwrap(),engine.render_page(&source.id,0,width).unwrap(),"{name} untouched");
        }
        capture(&name,&[("source",&original),("result",&fs::read(&path).unwrap()),("reference",&engine.source_bytes(&reference.id).unwrap())]);
        assert_eq!(engine.source_bytes(&source.id).unwrap(),original);
        for id in [&result.document.id,&reopened.id,&replay.document.id,&source.id,&reference.id,&reference_source.id]{engine.close_document(id).unwrap();}
    }}}
    engine.fonts().remove(&font.id).unwrap();
}

fn assert_physical_rect(actual:&folio_engine::AnnotationRect,expected:&folio_engine::AnnotationRect){
    for (a,b) in [actual.x,actual.y,actual.width,actual.height].into_iter().zip([expected.x,expected.y,expected.width,expected.height]){assert!((a-b).abs()<0.001,"Physical rectangle {actual:?} differs from {expected:?}");}
}

fn open_paragraph_reference(engine:&PdfEngine,temp:&tempfile::TempDir,name:&str,mut pdf:Document)->DocumentInfo{
    // The first page is visually identical but its field is no longer shared,
    // allowing the established isolated editor to serve as a publication oracle.
    let form=pdf.catalog().unwrap().get(b"AcroForm").unwrap().as_reference().unwrap();
    let field=pdf.get_dictionary(form).unwrap().get(b"Fields").unwrap().as_array().unwrap()[0].as_reference().unwrap();
    let mut kids=pdf.get_dictionary(field).unwrap().get(b"Kids").unwrap().as_array().unwrap().clone();kids.truncate(1);
    pdf.get_dictionary_mut(field).unwrap().set("Kids",kids);
    for page in pdf.get_pages().values().skip(1){
        let annotations=pdf.get_dictionary(*page).unwrap().get(b"Annots").unwrap().as_array().unwrap().iter().filter(|a|pdf.get_dictionary(a.as_reference().unwrap()).unwrap().get(b"Subtype").unwrap().as_name().unwrap()!=b"Widget").cloned().collect::<Vec<_>>();
        pdf.get_dictionary_mut(*page).unwrap().set("Annots",annotations);
    }
    open(engine,temp,name,&mut pdf)
}

#[test]
fn occurrence_paragraph_keeps_other_paragraph_memory_across_duplicate_save_reopen() {
    let _font_case=FONT_CASE.lock().unwrap();
    use folio_engine::{AnnotationRect,ParagraphFormatting,ParagraphAlignment};
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let font=engine.fonts().register(fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
    let mut pdf=fixture(false);let page=*pdf.get_pages().values().next().unwrap();
    let content=pdf.add_object(Stream::new(dictionary!{},b"BT /F1 12 Tf 40 240 Td (Old content) Tj ET BT /F1 12 Tf 40 350 Td (Neighbor) Tj ET".to_vec()));
    pdf.get_dictionary_mut(page).unwrap().set("Contents",content);
    let source=open(&engine,&temp,"two-paragraphs.pdf",&mut pdf);
    let a=AnnotationRect{x:40.,y:30.,width:220.,height:90.};let b=AnnotationRect{x:40.,y:140.,width:200.,height:80.};
    for saved_styled in [false,true] {for edited_styled in [false,true] {
        let saved_text="Saved paragraph\n\nOther line";
        let first=if saved_styled {
            let request=serde_json::from_value(serde_json::json!({"objectIndices":[1],"expectedText":"Neighbor","runs":[{"text":saved_text,"styleId":0}],"bounds":a,"alignment":"right","lineSpacing":1.4,"styleChanges":[{"styleId":0,"sourceStyleId":0,"fontId":font.id,"fontSize":12.,"color":"#993366"}]})).unwrap();
            engine.preview_styled_paragraph_occurrence(&source.id,&[0,1],0,&request).unwrap()
        }else{engine.preview_text_paragraph_occurrence(&source.id,&[0,1],0,&[1],"Neighbor",saved_text,&a,&ParagraphFormatting{font_id:Some(font.id.clone()),font_size:Some(12.),color:Some("#993366".into()),alignment:Some(ParagraphAlignment::Right),line_spacing:Some(1.4)}).unwrap()};
        let first_ids=engine.list_occurrence_text_runs(&first.document.id,0).unwrap().runs.into_iter().filter(|r|r.text!="Old content").map(|r|r.object_index).collect::<Vec<_>>();
        let normalize=|mut value:serde_json::Value| {value["objectIndices"]=serde_json::json!([]);if let Some(styles)=value["styles"].as_array_mut(){for style in styles{style["objectIndices"]=serde_json::json!([]);}}value};
        let expected=normalize(if saved_styled {serde_json::to_value(engine.inspect_occurrence_styled_paragraph(&first.document.id,0,&first_ids).unwrap()).unwrap()}else{serde_json::to_value(engine.inspect_occurrence_text_paragraph(&first.document.id,0,&first_ids).unwrap()).unwrap()});
        let second=if edited_styled {
            let request=serde_json::from_value(serde_json::json!({"objectIndices":[0],"expectedText":"Old content","runs":[{"text":"Edited main","styleId":0}],"bounds":b,"alignment":"center","lineSpacing":1.2})).unwrap();
            engine.preview_styled_paragraph_occurrence(&first.document.id,&[0,1,0],2,&request).unwrap()
        }else{engine.preview_text_paragraph_occurrence(&first.document.id,&[0,1,0],2,&[0],"Old content","Edited main",&b,&ParagraphFormatting::default()).unwrap()};
        let path=temp.path().join(format!("two-{saved_styled}-{edited_styled}.pdf"));
        engine.export_pdf(ExportRequest{pages:plans(&second.document,&second.page_indices),flatten:false},&path).unwrap();
        let reopened=engine.open_document(&path).unwrap();
        let ids=engine.list_occurrence_text_runs(&reopened.id,2).unwrap().runs.into_iter().filter(|r|r.text!="Edited main").map(|r|r.object_index).collect::<Vec<_>>();
        let replay=if saved_styled {
            let s=engine.inspect_occurrence_styled_paragraph(&reopened.id,2,&ids).unwrap();
            assert_eq!(normalize(serde_json::to_value(&s).unwrap()),expected);
            let request=serde_json::from_value(serde_json::json!({"objectIndices":ids,"expectedText":s.text,"runs":s.runs,"bounds":s.bounds,"alignment":s.alignment,"lineSpacing":s.line_spacing})).unwrap();
            engine.preview_styled_paragraph_occurrence(&reopened.id,&[0,1,2],2,&request).unwrap()
        }else{
            let s=engine.inspect_occurrence_text_paragraph(&reopened.id,2,&ids).unwrap();
            assert_eq!(normalize(serde_json::to_value(&s).unwrap()),expected);
            engine.preview_text_paragraph_occurrence(&reopened.id,&[0,1,2],2,&ids,&s.text,&s.text,&s.bounds,&ParagraphFormatting::default()).unwrap()
        };
        for width in [300,600]{assert_eq!(engine.render_page(&replay.document.id,2,width).unwrap(),engine.render_page(&reopened.id,2,width).unwrap());}
        for id in [&first.document.id,&second.document.id,&reopened.id,&replay.document.id]{engine.close_document(id).unwrap();}
    }}
    engine.close_document(&source.id).unwrap();engine.fonts().remove(&font.id).unwrap();
}

#[test]
fn occurrence_paragraph_preserves_protected_rights_and_rejects_stale_or_invalid_requests() {
    use folio_engine::{AnnotationRect,ParagraphFormatting};use lopdf::encryption::Permissions;
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let area=AnnotationRect{x:40.,y:140.,width:200.,height:90.};
    for (name,rights,edit,duplicate) in [("edit",Permissions::MODIFIABLE,true,false),("assemble",Permissions::ASSEMBLABLE,false,false),("both",Permissions::MODIFIABLE|Permissions::ASSEMBLABLE,true,true)] {
        let mut plain=Vec::new();fixture(false).save_to(&mut plain).unwrap();let encrypted=protected_fixture::encrypt(&plain,rights);
        let path=temp.path().join(format!("paragraph-{name}.pdf"));fs::write(&path,&encrypted).unwrap();
        let source=engine.open_document_with_password(&path,"reader").unwrap();
        for styled in [false,true] {for (order,selected,allowed) in [(vec![0,1],0,edit),(vec![0,1,0],2,duplicate)] {
            let result=if styled {
                let request=serde_json::from_value(serde_json::json!({"objectIndices":[0],"expectedText":"Old content","runs":[{"text":"Protected paragraph","styleId":0}],"bounds":area,"alignment":"left","lineSpacing":1.2})).unwrap();
                engine.preview_styled_paragraph_occurrence(&source.id,&order,selected,&request)
            }else{engine.preview_text_paragraph_occurrence(&source.id,&order,selected,&[0],"Old content","Protected paragraph",&area,&ParagraphFormatting::default())};
            if !allowed {assert!(matches!(result,Err(folio_engine::EngineError::PermissionDenied(_))));continue;}
            let result=result.unwrap();assert_eq!(result.document.protection,source.protection);
            assert!(matches!(engine.extract_text(&result.document.id,result.page_indices[selected]),Err(folio_engine::EngineError::PermissionDenied(_))));
            let saved=temp.path().join(format!("{name}-{styled}-{}.pdf",order.len()));
            engine.export_pdf(ExportRequest{pages:plans(&result.document,&result.page_indices),flatten:false},&saved).unwrap();
            let reader=engine.open_document_with_password(&saved,"reader").unwrap();assert_eq!(reader.protection.as_ref().unwrap().permissions,source.protection.as_ref().unwrap().permissions);
            let owner=engine.open_document_with_password(&saved,"owner").unwrap();assert!(engine.extract_text(&owner.id,selected).unwrap().contains("Protected paragraph"));
            for id in [&result.document.id,&reader.id,&owner.id]{engine.close_document(id).unwrap();}
        }}
        if edit {
            assert!(engine.preview_text_paragraph_occurrence(&source.id,&[0,1],0,&[0],"stale","New",&area,&ParagraphFormatting::default()).is_err());
            for (order,selected) in [(vec![],0),(vec![0],1),(vec![0,2],0),(vec![0;100_001],0)] {assert!(engine.preview_text_paragraph_occurrence(&source.id,&order,selected,&[0],"Old content","New",&area,&ParagraphFormatting::default()).is_err());}
        }
        assert_eq!(fs::read(path).unwrap(),encrypted);engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn occurrence_text_edit_keeps_shared_fields_and_reedits_the_selected_copy() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let source=open(&engine,&temp,"text-source.pdf",&mut fixture(false));
    let original=engine.source_bytes(&source.id).unwrap();
    // The legacy single-page publication must retain its ownership guard.
    assert!(engine.list_text_runs(&source.id,0).unwrap_err().to_string().contains("shared fields"));
    let runs=engine.list_occurrence_text_runs(&source.id,0).unwrap();
    let run=runs.runs.iter().find(|run|run.text=="Old content").unwrap();
    assert!(run.supported,"{:?}",run.reason);
    for (case,order,selected) in [("unique",vec![0,1],0),("first",vec![0,1,0],0),("later",vec![0,1,0],2)] {
        let result=engine.replace_text_occurrence(&source.id,&order,selected,run.object_index,&run.text,"New content",None).unwrap();
        assert_eq!(result.document.pages.len(),if case=="unique"{2}else{3});
        assert_eq!(result.document.bookmarks,source.bookmarks);
        for (i,old) in order.iter().enumerate() {
            let page=result.page_indices[i];
            if i==selected {assert!(engine.extract_text(&result.document.id,page).unwrap().contains("New content"));}
            else {for width in [300,600] {assert_eq!(engine.render_page(&result.document.id,page,width).unwrap(),engine.render_page(&source.id,*old,width).unwrap());}}
        }
        let index=result.page_indices[selected];
        let again=engine.list_occurrence_text_runs(&result.document.id,index).unwrap();
        let run=again.runs.iter().find(|r|r.text=="New content").unwrap();
        let edited=engine.replace_text_occurrence(&result.document.id,&result.page_indices,selected,run.object_index,&run.text,"Next content",None).unwrap();
        assert_eq!(edited.document.pages.len(),result.document.pages.len());
        let field=engine.list_form_fields(&edited.document.id,edited.page_indices[selected]).unwrap().remove(0);
        let filled=engine.fill_form_field(&edited.document.id,FillFormFieldRequest{page_index:edited.page_indices[selected],annotation_index:field.annotation_index,expected_value:field.value,value:FormFieldValue::Text{text:"Linked after editing".into()}}).unwrap();
        for page in 0..filled.pages.len() {
            assert_eq!(engine.list_form_fields(&filled.id,page).unwrap()[0].value,FormFieldValue::Text{text:"Linked after editing".into()});
        }
        let filled_path=temp.path().join(format!("filled-{case}.pdf"));
        engine.export_pdf(ExportRequest{pages:plans(&filled,&edited.page_indices),flatten:false},&filled_path).unwrap();
        let filled_saved=engine.open_document(&filled_path).unwrap();
        for page in 0..filled_saved.pages.len() {
            assert_eq!(engine.list_form_fields(&filled_saved.id,page).unwrap()[0].value,FormFieldValue::Text{text:"Linked after editing".into()});
        }
        let other=engine.list_form_fields(&filled_saved.id,1).unwrap().remove(0);
        let filled_again=engine.fill_form_field(&filled_saved.id,FillFormFieldRequest{page_index:1,annotation_index:other.annotation_index,expected_value:other.value,value:FormFieldValue::Text{text:"Changed again".into()}}).unwrap();
        for page in 0..filled_again.pages.len() {
            assert_eq!(engine.list_form_fields(&filled_again.id,page).unwrap()[0].value,FormFieldValue::Text{text:"Changed again".into()});
        }
        let path=temp.path().join(format!("text-{case}.pdf"));
        engine.export_pdf(ExportRequest{pages:plans(&edited.document,&edited.page_indices),flatten:false},&path).unwrap();
        verify_links(&Document::load(&path).unwrap(),&order);
        let reopened=engine.open_document(&path).unwrap();
        assert!(engine.extract_text(&reopened.id,selected).unwrap().contains("Next content"));
        capture(&format!("text-{case}"),&[("source",&original),("result",&fs::read(&path).unwrap()),("filled",&fs::read(&filled_path).unwrap())]);
        for id in [&result.document.id,&edited.document.id,&filled.id,&reopened.id,&filled_saved.id,&filled_again.id] {engine.close_document(id).unwrap();}
    }
    assert_eq!(engine.source_bytes(&source.id).unwrap(),original);
    assert_eq!(fs::read(temp.path().join("text-source.pdf")).unwrap(),original.as_ref());
    engine.close_document(&source.id).unwrap();
}

#[test]
fn occurrence_text_edit_keeps_physical_frames_and_rejects_stale_requests() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    for rotation in [0,90,180,270] {for unit in [1.,1.7,2.] {
        let mut pdf=fixture(false);let page=*pdf.get_pages().values().next().unwrap();
        let dict=pdf.get_dictionary_mut(page).unwrap();
        dict.set("MediaBox",vec![(-20).into(),(-10).into(),300.into(),400.into()]);
        dict.set("CropBox",vec![10.into(),20.into(),290.into(),380.into()]);
        dict.set("Rotate",rotation);dict.set("UserUnit",Object::Real(unit));
        let name=format!("text-r{rotation}-u{unit}");
        let source=open(&engine,&temp,&format!("{name}.pdf"),&mut pdf);
        let original=engine.source_bytes(&source.id).unwrap();
        let run=engine.list_occurrence_text_runs(&source.id,0).unwrap().runs.remove(0);
        assert!(run.supported,"{:?}",run.reason);
        assert!((run.font_size-18.*unit).abs()<0.001);
        for (order,selected) in [(vec![],0),(vec![0],1),(vec![0,2],0),(vec![0;100_001],0)] {
            assert!(engine.replace_text_occurrence(&source.id,&order,selected,run.object_index,&run.text,"New content",None).is_err());
        }
        assert!(engine.replace_text_occurrence(&source.id,&[0,1],0,run.object_index,"stale","New content",None).unwrap_err().to_string().contains("changed since"));
        let result=engine.replace_text_occurrence(&source.id,&[0,1,0],2,run.object_index,&run.text,"New content",None).unwrap();
        assert_eq!(result.page_indices,vec![0,1,2]);
        assert_eq!(result.document.pages[2],source.pages[0]);
        let saved=engine.source_bytes(&result.document.id).unwrap();let actual=Document::load_mem(&saved).unwrap();
        let copied=actual.get_dictionary(*actual.get_pages().values().nth(2).unwrap()).unwrap();
        for key in [b"MediaBox".as_slice(),b"CropBox",b"Rotate"] {assert_eq!(copied.get(key).unwrap(),pdf.get_dictionary(page).unwrap().get(key).unwrap());}
        assert_eq!(copied.get(b"UserUnit").unwrap().as_float().unwrap(),unit);
        assert_eq!(engine.render_page(&result.document.id,0,600).unwrap(),engine.render_page(&source.id,0,600).unwrap());
        let edited=engine.list_occurrence_text_runs(&result.document.id,2).unwrap().runs.remove(0);
        assert_eq!(edited.text,"New content");assert!((edited.font_size-run.font_size).abs()<0.001);
        capture(&name,&[("source",&original),("result",&saved)]);
        assert_eq!(engine.source_bytes(&source.id).unwrap(),original);
        engine.close_document(&result.document.id).unwrap();engine.close_document(&source.id).unwrap();
    }}
}

#[test]
fn occurrence_text_edit_preserves_protected_origin_and_rights() {
    use lopdf::encryption::Permissions;
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    for (name,rights,edit,duplicate) in [
        ("edit",Permissions::MODIFIABLE,true,false),
        ("assemble",Permissions::ASSEMBLABLE,false,false),
        ("both",Permissions::MODIFIABLE|Permissions::ASSEMBLABLE,true,true),
    ] {
        let mut plain=Vec::new();fixture(false).save_to(&mut plain).unwrap();
        let encrypted=protected_fixture::encrypt(&plain,rights);let path=temp.path().join(format!("{name}.pdf"));fs::write(&path,&encrypted).unwrap();
        let source=engine.open_document_with_password(&path,"reader").unwrap();
        let runs=engine.list_occurrence_text_runs(&source.id,0);
        if !edit {assert!(matches!(runs,Err(folio_engine::EngineError::PermissionDenied(_))));}
        for (order,selected,allowed) in [(vec![0,1],0,edit),(vec![0,1,0],2,duplicate)] {
            let result=engine.replace_text_occurrence(&source.id,&order,selected,0,"Old content","New content",None);
            if !allowed {assert!(matches!(result,Err(folio_engine::EngineError::PermissionDenied(_))));continue;}
            let result=result.unwrap();assert_eq!(result.document.protection,source.protection);
            assert!(matches!(engine.extract_text(&result.document.id,result.page_indices[selected]),Err(folio_engine::EngineError::PermissionDenied(_))));
            let store=folio_engine::RecoveryStore::new(temp.path().join(format!("text-recovery-{name}-{}",order.len()))).unwrap();
            let workspace=serde_json::json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","dirty":true,"savedDigest":"old","zoom":100,"scrollPosition":{"top":0,"left":0},"document":{"name":"Protected.pdf","pages":plans(&result.document,&result.page_indices),"selectedPageId":"p0","selectedOverlayId":null}}]});
            store.save(&engine,workspace).unwrap();
            let recovered=store.load(&engine).unwrap().unwrap();
            let recovered_pages:Vec<PagePlan>=serde_json::from_value(recovered["tabs"][0]["document"]["pages"].clone()).unwrap();
            let recovered_id=recovered_pages[0].source_id.clone();
            assert!(recovered_pages.iter().all(|p|p.source_id==recovered_id));
            assert!(matches!(engine.extract_text(&recovered_id,recovered_pages[selected].page_index),Err(folio_engine::EngineError::PermissionDenied(_))));
            let saved=temp.path().join(format!("{name}-{}.pdf",order.len()));
            engine.export_pdf(ExportRequest{pages:recovered_pages,flatten:false},&saved).unwrap();
            let reader=engine.open_document_with_password(&saved,"reader").unwrap();
            assert_eq!(reader.protection.as_ref().unwrap().permissions,source.protection.as_ref().unwrap().permissions);
            let owner=engine.open_document_with_password(&saved,"owner").unwrap();
            assert!(engine.extract_text(&owner.id,selected).unwrap().contains("New content"));
            for id in [&result.document.id,&reader.id,&owner.id,&recovered_id] {engine.close_document(id).unwrap();}
        }
        assert_eq!(fs::read(&path).unwrap(),encrypted);engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn occurrence_text_edit_retains_unaffected_paragraph_replay() {
    let _font_case=FONT_CASE.lock().unwrap();
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let mut pdf=fixture(false);pdf.catalog_mut().unwrap().remove(b"AcroForm");
    for page in pdf.get_pages().values() {pdf.get_dictionary_mut(*page).unwrap().remove(b"Annots");}
    let page=*pdf.get_pages().values().next().unwrap();
    let stream=pdf.add_object(Stream::new(dictionary!{},b"BT /F1 12 Tf 40 240 Td (Old content) Tj ET BT /F1 12 Tf 40 350 Td (Neighbor) Tj ET".to_vec()));
    pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);
    let source=open(&engine,&temp,"paragraph-source.pdf",&mut pdf);
    let font=engine.fonts().register(fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
    for styled in [false,true] {for substitute in [false,true] {
        let area=folio_engine::AnnotationRect{x:40.,y:140.,width:200.,height:80.};
        let draft=if styled {
            let request=serde_json::from_value(serde_json::json!({"objectIndices":[0],"expectedText":"Old content","runs":[{"text":"One paragraph\nSecond line","styleId":0}],"bounds":area,"alignment":"right","lineSpacing":1.4})).unwrap();
            engine.preview_styled_paragraph(&source.id,0,&request).unwrap()
        } else {
            engine.preview_text_paragraph_with_format(&source.id,0,&[0],"Old content","One paragraph\nSecond line",&area,&folio_engine::ParagraphFormatting{alignment:Some(folio_engine::ParagraphAlignment::Right),line_spacing:Some(1.4),..Default::default()}).unwrap()
        };
        let runs=engine.list_text_runs(&draft.id,0).unwrap().runs;
        let neighbor=runs.iter().find(|r|r.text=="Neighbor").unwrap();
        let indices=runs.iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect::<Vec<_>>();
        let before=if styled {serde_json::to_value(engine.inspect_styled_paragraph(&draft.id,0,&indices).unwrap()).unwrap()}else{serde_json::to_value(engine.inspect_text_paragraph(&draft.id,0,&indices).unwrap()).unwrap()};
        let result=engine.replace_text_occurrence(&draft.id,&[0],0,neighbor.object_index,"Neighbor","Changed",substitute.then_some(font.id.as_str())).unwrap();
        let after=if styled {serde_json::to_value(engine.inspect_styled_paragraph(&result.document.id,0,&indices).unwrap()).unwrap()}else{serde_json::to_value(engine.inspect_text_paragraph(&result.document.id,0,&indices).unwrap()).unwrap()};
        assert_eq!(before,after,"styled={styled}");
        assert_eq!(after["alignment"],"right");assert_eq!(after["text"],"One paragraph\nSecond line");
        engine.close_document(&result.document.id).unwrap();engine.close_document(&draft.id).unwrap();
    }}
    engine.close_document(&source.id).unwrap();
}

#[test]
fn review_custom_appearance_resources_survive_duplicate_copy() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    for key in ["First","Prev","Parent"] {
        let mut doc=fixture(false);
        // Move all fixture objects up so the skipped reference collides with
        // the importer's low-number catalog rather than another source object.
        doc.renumber_objects_with(20);
        let color=(1,0);
        doc.objects.insert(color,Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),20.into(),20.into()],"Resources"=>dictionary!{}},b"0.8 0.1 0.3 rg 0 0 20 20 re f".to_vec()).into());
        let ap=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),20.into(),20.into()],"Resources"=>dictionary!{"XObject"=>dictionary!{key=>color}}},format!("/{key} Do").into_bytes()));
        let page=*doc.get_pages().values().next().unwrap();
        let note=doc.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap()[1].as_reference().unwrap();
        doc.get_dictionary_mut(note).unwrap().set("AP",dictionary!{"N"=>ap});
        let source=open(&engine,&temp,&format!("ap-{key}.pdf"),&mut doc);
        let incoming=open(&engine,&temp,&format!("incoming-{key}.pdf"),&mut fixture(true));
        for selected in [0,2] {
            let result=engine.replace_page_content_occurrence(&source.id,&[0,1,0],selected,&incoming.id,0).unwrap();
            let raw=engine.source_bytes(&result.document.id).unwrap();let pdf=Document::load_mem(&raw).unwrap();
            for index in [0,2] {
                let p=*pdf.get_pages().values().nth(result.page_indices[index]).unwrap();
                let annots=pdf.get_dictionary(p).unwrap().get(b"Annots").unwrap().as_array().unwrap();
                let a=pdf.get_dictionary(annots[1].as_reference().unwrap()).unwrap();
                let appearance=pdf.dereference(a.get(b"AP").unwrap()).unwrap().1.as_dict().unwrap();
                let stream=pdf.dereference(appearance.get(b"N").unwrap()).unwrap().1.as_stream().unwrap();
                let resources=pdf.dereference(stream.dict.get(b"Resources").unwrap()).unwrap().1.as_dict().unwrap();
                let objects=pdf.dereference(resources.get(b"XObject").unwrap()).unwrap().1.as_dict().unwrap();
                let child=pdf.dereference(objects.get(key.as_bytes()).unwrap()).unwrap().1;
                assert!(child.as_stream().is_ok(),"{key}: annotation resource must still be a Form stream, got {child:?}");
                if index!=selected {for width in [300,600] {assert_eq!(engine.render_page(&result.document.id,result.page_indices[index],width).unwrap(),engine.render_page(&source.id,0,width).unwrap(),"{key} unselected appearance changed");}}
            }
            capture(&format!("ap-{key}-s{selected}"),&[("source",&engine.source_bytes(&source.id).unwrap()),("incoming",&engine.source_bytes(&incoming.id).unwrap()),("result",&raw)]);
        }
    }
}

#[test]
fn review_untouched_page_action_targets_the_live_shared_field() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    for named in [false,true] {
        let mut doc=fixture(false);
        let form=doc.get_dictionary(doc.catalog().unwrap().get(b"AcroForm").unwrap().as_reference().unwrap()).unwrap();
        let field=form.get(b"Fields").unwrap().as_array().unwrap()[0].as_reference().unwrap();
        doc.get_dictionary_mut(field).unwrap().set("DV",Object::string_literal("Default"));
        let other=*doc.get_pages().values().nth(1).unwrap();
        let action=doc.add_object(dictionary!{"S"=>"ResetForm","Fields"=>vec![if named {Object::string_literal("shared")} else {Object::Reference(field)}]});
        doc.get_dictionary_mut(other).unwrap().set("AA",dictionary!{"O"=>action});
        let source=open(&engine,&temp,&format!("actions-{named}.pdf"),&mut doc);
        let incoming=open(&engine,&temp,&format!("action-incoming-{named}.pdf"),&mut fixture(true));
        for selected in [0,2] {
            let result=engine.replace_page_content_occurrence(&source.id,&[0,1,0],selected,&incoming.id,0).unwrap();
            let pdf=Document::load_mem(&engine.source_bytes(&result.document.id).unwrap()).unwrap();
            let form=pdf.dereference(pdf.catalog().unwrap().get(b"AcroForm").unwrap()).unwrap().1.as_dict().unwrap();
            let live=form.get(b"Fields").unwrap().as_array().unwrap()[0].as_reference().unwrap();
            let aa=pdf.dereference(pdf.get_dictionary(other).unwrap().get(b"AA").unwrap()).unwrap().1.as_dict().unwrap();
            let action=pdf.dereference(aa.get(b"O").unwrap()).unwrap().1.as_dict().unwrap();
            let target=&action.get(b"Fields").unwrap().as_array().unwrap()[0];
            if named {assert_eq!(target.as_str().unwrap(),b"shared");} else {assert_eq!(target.as_reference().unwrap(),live,"untouched page action targets obsolete field");assert!(!pdf.objects.contains_key(&field),"obsolete field graph must not remain reachable");}
        }
    }
}

#[test]
fn review_unknown_inbound_field_references_are_refused_without_changing_sources() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();let mut pdf=fixture(false);
    let form=pdf.get_dictionary(pdf.catalog().unwrap().get(b"AcroForm").unwrap().as_reference().unwrap()).unwrap();
    let field=form.get(b"Fields").unwrap().as_array().unwrap()[0].clone();
    pdf.catalog_mut().unwrap().set("PrivateFieldTarget",field);
    let source=open(&engine,&temp,"private-target.pdf",&mut pdf);
    let incoming=open(&engine,&temp,"incoming.pdf",&mut fixture(true));
    let before=engine.source_bytes(&source.id).unwrap();
    let error=engine.replace_page_content_occurrence(&source.id,&[0,1,0],0,&incoming.id,0).unwrap_err().to_string();
    assert!(error.contains("unsupported retained reference"),"{error}");
    assert_eq!(engine.source_bytes(&source.id).unwrap(),before);
    // Unique occurrence does not rebuild the form graph and retains identity.
    let result=engine.replace_page_content_occurrence(&source.id,&[0,1],0,&incoming.id,0).unwrap();
    assert_eq!(result.document.pages.len(),2);
}

#[test]
fn review_annotation_actions_and_each_copied_page_self_action_remain_bound() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();let mut pdf=fixture(false);
    let first=*pdf.get_pages().values().next().unwrap();
    let form=pdf.get_dictionary(pdf.catalog().unwrap().get(b"AcroForm").unwrap().as_reference().unwrap()).unwrap();
    let field=form.get(b"Fields").unwrap().as_array().unwrap()[0].as_reference().unwrap();
    let annots=pdf.get_dictionary(first).unwrap().get(b"Annots").unwrap().as_array().unwrap();
    let widget=annots[0].as_reference().unwrap();let note=annots[1].as_reference().unwrap();
    pdf.get_dictionary_mut(note).unwrap().set("AA",dictionary!{"E"=>dictionary!{"S"=>"ResetForm","Fields"=>vec![Object::Reference(field)]},"X"=>dictionary!{"S"=>"Hide","T"=>widget}});
    let action=pdf.add_object(dictionary!{"S"=>"GoTo","D"=>vec![Object::Reference(first),Object::Name(b"Fit".to_vec())]});
    pdf.get_dictionary_mut(first).unwrap().set("AA",dictionary!{"O"=>action});
    let source=open(&engine,&temp,"annotation-actions.pdf",&mut pdf);
    let incoming=open(&engine,&temp,"incoming.pdf",&mut fixture(true));
    for selected in [0,2] {
        let result=engine.replace_page_content_occurrence(&source.id,&[0,1,0],selected,&incoming.id,0).unwrap();
        let pdf=Document::load_mem(&engine.source_bytes(&result.document.id).unwrap()).unwrap();
        let form=pdf.dereference(pdf.catalog().unwrap().get(b"AcroForm").unwrap()).unwrap().1.as_dict().unwrap();
        let field=form.get(b"Fields").unwrap().as_array().unwrap()[0].as_reference().unwrap();
        let widgets=pdf.get_dictionary(field).unwrap().get(b"Kids").unwrap().as_array().unwrap();
        for index in [0,2] {
            let p=*pdf.get_pages().values().nth(index).unwrap();let page=pdf.get_dictionary(p).unwrap();
            let aa=pdf.dereference(page.get(b"AA").unwrap()).unwrap().1.as_dict().unwrap();
            let action=pdf.dereference(aa.get(b"O").unwrap()).unwrap().1.as_dict().unwrap();
            assert_eq!(action.get(b"D").unwrap().as_array().unwrap()[0].as_reference().unwrap(),p);
            let note=pdf.get_dictionary(page.get(b"Annots").unwrap().as_array().unwrap()[1].as_reference().unwrap()).unwrap();
            let aa=pdf.dereference(note.get(b"AA").unwrap()).unwrap().1.as_dict().unwrap();
            let reset=pdf.dereference(aa.get(b"E").unwrap()).unwrap().1.as_dict().unwrap();
            assert_eq!(reset.get(b"Fields").unwrap().as_array().unwrap()[0].as_reference().unwrap(),field);
            let hide=pdf.dereference(aa.get(b"X").unwrap()).unwrap().1.as_dict().unwrap();
            let targets=hide.get(b"T").unwrap().as_array().unwrap();
            assert_eq!(targets.len(),2);
            assert!(targets.iter().all(|target|widgets.contains(target)));
        }
    }
}

#[test]
fn review_nested_inherited_page_geometry_survives_append() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();let mut pdf=fixture(false);
    let pages:Vec<_>=pdf.get_pages().values().copied().collect();
    let root=pdf.catalog().unwrap().get(b"Pages").unwrap().as_reference().unwrap();
    let page=pdf.get_dictionary(pages[0]).unwrap();let resources=page.get(b"Resources").unwrap().clone();
    let kids=pdf.add_object(Object::Array(vec![Object::Reference(pages[0])]));
    let branch=pdf.add_object(dictionary!{"Type"=>"Pages","Parent"=>root,"Kids"=>kids,"Count"=>1,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"CropBox"=>vec![10.into(),20.into(),280.into(),370.into()],"Rotate"=>90,"Resources"=>resources});
    let page=pdf.get_dictionary_mut(pages[0]).unwrap();page.remove(b"MediaBox");page.remove(b"Resources");page.set("Parent",branch);
    let root_kids=pdf.add_object(Object::Array(vec![Object::Reference(branch),Object::Reference(pages[1])]));
    pdf.get_dictionary_mut(root).unwrap().set("Kids",root_kids);
    let source=open(&engine,&temp,"inherited.pdf",&mut pdf);let incoming=open(&engine,&temp,"incoming.pdf",&mut fixture(true));
    for selected in [0,2] {
        let result=engine.replace_page_content_occurrence(&source.id,&[0,1,0],selected,&incoming.id,0).unwrap();
        for index in [0,1,2] {
            let old=if index==1 {1}else{0};assert_eq!(result.document.pages[result.page_indices[index]],source.pages[old]);
            if index!=selected {assert_eq!(engine.render_page(&result.document.id,result.page_indices[index],600).unwrap(),engine.render_page(&source.id,old,600).unwrap());}
        }
    }
}

#[test]
fn review_shared_stamp_state_appearances_keep_aliases_and_pixels() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();let mut pdf=fixture(false);
    let first=*pdf.get_pages().values().next().unwrap();
    let ap=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),20.into(),20.into()],"Resources"=>dictionary!{}},b"0.2 0.7 0.4 rg 0 0 20 20 re f".to_vec()));
    let appearances=pdf.add_object(dictionary!{"N"=>dictionary!{"On"=>ap},"R"=>ap,"D"=>ap});
    let mut annots=pdf.get_dictionary(first).unwrap().get(b"Annots").unwrap().as_array().unwrap().clone();
    for left in [30,80] {
        let stamp=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Stamp","P"=>first,"Rect"=>vec![left.into(),300.into(),(left+20).into(),320.into()],"F"=>4,"AS"=>"On","AP"=>appearances});
        annots.push(Object::Reference(stamp));
    }
    pdf.get_dictionary_mut(first).unwrap().set("Annots",annots);
    let source=open(&engine,&temp,"stamps.pdf",&mut pdf);let incoming=open(&engine,&temp,"incoming.pdf",&mut fixture(true));
    for selected in [0,2] {
        let result=engine.replace_page_content_occurrence(&source.id,&[0,1,0],selected,&incoming.id,0).unwrap();
        let pdf=Document::load_mem(&engine.source_bytes(&result.document.id).unwrap()).unwrap();
        let unchanged=if selected==0 {2}else{0};
        for width in [300,600] {assert_eq!(engine.render_page(&result.document.id,unchanged,width).unwrap(),engine.render_page(&source.id,0,width).unwrap());}
        for index in [0,2] {
            let p=*pdf.get_pages().values().nth(index).unwrap();
            let stamps:Vec<_>=pdf.get_dictionary(p).unwrap().get(b"Annots").unwrap().as_array().unwrap().iter().filter_map(|v|{
                let d=pdf.get_dictionary(v.as_reference().unwrap()).unwrap();
                (d.get(b"Subtype").unwrap().as_name().unwrap()==b"Stamp").then_some(d)
            }).collect();
            assert_eq!(stamps.len(),2);
            let mut ids=Vec::new();
            for stamp in stamps {
                assert_eq!(stamp.get(b"AS").unwrap().as_name().unwrap(),b"On");
                let ap=pdf.dereference(stamp.get(b"AP").unwrap()).unwrap().1.as_dict().unwrap();
                let states=pdf.dereference(ap.get(b"N").unwrap()).unwrap().1.as_dict().unwrap();
                for value in [states.get(b"On").unwrap(),ap.get(b"R").unwrap(),ap.get(b"D").unwrap()] {ids.push(value.as_reference().unwrap());}
            }
            assert!(ids.iter().all(|id|*id==ids[0]),"AP states and stamps must share one appearance per page");
        }
    }
}
fn fixture(incoming: bool) -> Document {
    let mut doc = Document::with_version("1.7");
    let tree = doc.new_object_id();
    let font=doc.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>if incoming {"Courier"} else {"Helvetica"}});
    let resources = dictionary! {"Font"=>dictionary! {"F1"=>font}};
    let mut pages = Vec::new();
    for text in [
        if incoming {
            "New content"
        } else {
            "Old content"
        },
        "Other page",
    ] {
        let stream = doc.add_object(Stream::new(
            dictionary! {},
            format!("BT /F1 18 Tf 40 240 Td ({text}) Tj ET").into_bytes(),
        ));
        pages.push(doc.add_object(dictionary! {"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Resources"=>resources.clone(),"Contents"=>stream}));
    }
    let field = doc.new_object_id();
    let mut widgets = Vec::new();
    for (index, page) in pages.iter().enumerate() {
        let ap=doc.add_object(Stream::new(dictionary! {"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),180.into(),26.into()],"Resources"=>resources.clone()},b"BT /F1 12 Tf 4 8 Td (Keep field) Tj ET".to_vec()));
        let widget=doc.add_object(dictionary! {"Type"=>"Annot","Subtype"=>"Widget","Parent"=>field,"P"=>*page,"Rect"=>vec![30.into(),45.into(),210.into(),71.into()],"F"=>4,"AP"=>dictionary! {"N"=>ap}});
        widgets.push(Object::Reference(widget));
        let note=doc.add_object(dictionary! {"Type"=>"Annot","Subtype"=>"Text","P"=>*page,"Rect"=>vec![240.into(),50.into(),258.into(),68.into()],"Contents"=>Object::string_literal("Keep review"),"F"=>4});
        let mut annotations = vec![Object::Reference(widget), Object::Reference(note)];
        for (slot, kind) in ["direct", "named", "self"].iter().enumerate() {
            let mut link = dictionary! {"Type"=>"Annot","Subtype"=>"Link","P"=>*page,"NM"=>Object::string_literal(*kind),"Rect"=>vec![30.into(),(90+slot as i64*25).into(),180.into(),(110+slot as i64*25).into()],"Border"=>vec![0.into(),0.into(),0.into()]};
            if *kind == "named" {
                link.set(
                    "A",
                    dictionary! {"S"=>"GoTo","D"=>Object::string_literal(format!("p{}",1-index))},
                );
            } else {
                link.set(
                    "Dest",
                    vec![
                        Object::Reference(if *kind == "self" {
                            *page
                        } else {
                            pages[1 - index]
                        }),
                        Object::Name(b"Fit".to_vec()),
                    ],
                );
            }
            annotations.push(Object::Reference(doc.add_object(link)));
        }
        doc.get_dictionary_mut(*page)
            .unwrap()
            .set("Annots", annotations);
    }
    doc.objects.insert(field,dictionary! {"FT"=>"Tx","T"=>Object::string_literal("shared"),"V"=>Object::string_literal("Keep field"),"DA"=>Object::string_literal("/F1 12 Tf 0 g"),"Kids"=>widgets}.into());
    let form=doc.add_object(dictionary! {"Fields"=>vec![Object::Reference(field)],"DR"=>resources,"DA"=>Object::string_literal("/F1 12 Tf 0 g")});
    doc.objects.insert(tree,dictionary! {"Type"=>"Pages","Kids"=>pages.iter().map(|p|Object::Reference(*p)).collect::<Vec<_>>(),"Count"=>2}.into());
    let outlines = doc.new_object_id();
    let bookmark=doc.add_object(dictionary! {"Title"=>Object::string_literal("Retained bookmark"),"Parent"=>outlines,"Dest"=>vec![Object::Reference(pages[0]),Object::Name(b"Fit".to_vec())]});
    doc.objects.insert(
        outlines,
        dictionary! {"Type"=>"Outlines","First"=>bookmark,"Last"=>bookmark,"Count"=>1}.into(),
    );
    let metadata = doc.add_object(Stream::new(
        dictionary! {"Type"=>"Metadata","Subtype"=>"XML"},
        b"<retained>original catalog</retained>".to_vec(),
    ));
    let root=doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>tree,"AcroForm"=>form,"Outlines"=>outlines,"Metadata"=>metadata,"Lang"=>Object::string_literal("en-US"),"PageLayout"=>"TwoColumnLeft","Names"=>dictionary! {"Dests"=>dictionary! {"Names"=>vec![Object::string_literal("p0"),Object::Array(vec![Object::Reference(pages[0]),Object::Name(b"Fit".to_vec())]),Object::string_literal("p1"),Object::Array(vec![Object::Reference(pages[1]),Object::Name(b"Fit".to_vec())])]}}});
    doc.trailer.set("Root", root);
    doc
}
fn open(
    engine: &PdfEngine,
    temp: &tempfile::TempDir,
    name: &str,
    doc: &mut Document,
) -> DocumentInfo {
    let path = temp.path().join(name);
    doc.save(&path).unwrap();
    engine.open_document(&path).unwrap()
}
fn plans(source: &DocumentInfo, indices: &[usize]) -> Vec<PagePlan> {
    indices.iter().enumerate().map(|(i,p)|serde_json::from_value(serde_json::json!({"id":format!("p{i}"),"sourceId":source.id,"pageIndex":p,"width":source.pages[*p].width,"height":source.pages[*p].height,"rotation":0,"overlays":source.pages[*p].overlays})).unwrap()).collect()
}
fn target(pdf: &Document, value: &Object) -> usize {
    let mut value = pdf.dereference(value).unwrap().1;
    if let Object::String(name,_) | Object::Name(name) = value {
        // This fixture has a flat destination name tree. A whole-source save
        // correctly retains it; page-import saves may instead emit direct arrays.
        let names=pdf.dereference(pdf.catalog().unwrap().get(b"Names").unwrap()).unwrap().1.as_dict().unwrap();
        let destinations=pdf.dereference(names.get(b"Dests").unwrap()).unwrap().1.as_dict().unwrap();
        let entries=pdf.dereference(destinations.get(b"Names").unwrap()).unwrap().1.as_array().unwrap();
        assert_eq!(entries.len()%2,0);
        let matches:Vec<_>=entries.chunks_exact(2).filter(|pair|pair[0].as_str().is_ok_and(|key|key==name)).collect();
        assert_eq!(matches.len(),1,"Named destination must resolve exactly once");
        value=pdf.dereference(&matches[0][1]).unwrap().1;
    }
    let a = value.as_array().unwrap();
    let id = a[0].as_reference().unwrap();
    pdf.get_pages().values().position(|p| *p == id).unwrap()
}

#[test]
fn original_named_destinations_are_valid_before_and_after_whole_source_save() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();let mut pdf=fixture(false);
    // The oracle must accept the fixture's original named destinations, not
    // require page-import normalization into an explicit destination array.
    verify_links(&pdf,&[0,1]);
    let source=open(&engine,&temp,"named-source.pdf",&mut pdf);
    let before=engine.source_bytes(&source.id).unwrap();let path=temp.path().join("named-saved.pdf");
    engine.export_pdf(ExportRequest{pages:plans(&source,&[0,1]),flatten:false},&path).unwrap();
    assert!(fs::read(&path).unwrap().as_slice()==before.as_ref());
    verify_links(&Document::load(&path).unwrap(),&[0,1]);
}
fn verify_links(pdf: &Document, order: &[usize]) {
    for (index, id) in pdf.get_pages().values().enumerate() {
        let annotations = pdf
            .get_dictionary(*id)
            .unwrap()
            .get(b"Annots")
            .unwrap()
            .as_array()
            .unwrap();
        let mut count = 0;
        for annotation in annotations {
            let annotation = pdf
                .get_dictionary(annotation.as_reference().unwrap())
                .unwrap();
            if !annotation
                .get(b"Subtype")
                .and_then(Object::as_name)
                .is_ok_and(|s| s == b"Link")
            {
                continue;
            }
            count += 1;
            let name = annotation.get(b"NM").unwrap().as_str().unwrap();
            let dest = if name == b"named" {
                annotation
                    .get(b"A")
                    .unwrap()
                    .as_dict()
                    .unwrap()
                    .get(b"D")
                    .unwrap()
            } else {
                annotation.get(b"Dest").unwrap()
            };
            assert_eq!(
                target(pdf, dest),
                if name == b"self" {
                    index
                } else {
                    order.iter().position(|p| *p == 1 - order[index]).unwrap()
                }
            );
        }
        assert_eq!(count, 3);
    }
}

#[test]
fn replaces_one_duplicate_retaining_shared_fields_and_links() {
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    let mut original_pdf = fixture(false);
    // Composed Save preserves supported XMP. Keep the shared non-XMP fixture
    // unchanged for byte-preserving routes and the explicit refusal test below.
    let metadata = original_pdf.catalog().unwrap().get(b"Metadata").unwrap().as_reference().unwrap();
    original_pdf.get_object_mut(metadata).unwrap().as_stream_mut().unwrap().set_content(
        br#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:v="urn:folio-occurrence"><v:Retained>original catalog</v:Retained></rdf:Description></rdf:RDF></x:xmpmeta>"#.to_vec(),
    );
    let source = open(&engine, &temp, "original.pdf", &mut original_pdf);
    let incoming = open(&engine, &temp, "incoming.pdf", &mut fixture(true));
    let original = fs::read(temp.path().join("original.pdf")).unwrap();
    let order = [0, 1, 0, 0];
    for selected in [0, 2, 3] {
        let result = engine
            .replace_page_content_occurrence(&source.id, &order, selected, &incoming.id, 0)
            .unwrap();
        assert_eq!(result.document.pages.len(), 3);
        assert_eq!(result.page_indices.len(), 4);
        for (occurrence, old) in order.iter().enumerate() {
            let index = result.page_indices[occurrence];
            assert_eq!(result.document.pages[index], source.pages[*old]);
            if occurrence == selected {
                assert!(engine
                    .extract_text(&result.document.id, index)
                    .unwrap()
                    .contains("New content"));
            } else {
                assert_eq!(
                    engine.render_page(&result.document.id, index, 600).unwrap(),
                    engine.render_page(&source.id, *old, 600).unwrap()
                );
            }
        }
        let raw = engine.source_bytes(&result.document.id).unwrap();
        let pdf = Document::load_mem(&raw).unwrap();
        assert_eq!(
            pdf.get_pages()
                .values()
                .take(2)
                .copied()
                .collect::<Vec<_>>(),
            original_pdf
                .get_pages()
                .values()
                .copied()
                .collect::<Vec<_>>()
        );
        assert_eq!(
            pdf.catalog().unwrap().get(b"Metadata").unwrap(),
            original_pdf.catalog().unwrap().get(b"Metadata").unwrap()
        );
        assert_eq!(result.document.bookmarks, source.bookmarks);
        let path = temp.path().join(format!("selected-{selected}.pdf"));
        engine
            .export_pdf(
                ExportRequest {
                    pages: plans(&result.document, &result.page_indices),
                    flatten: false,
                },
                &path,
            )
            .unwrap();
        let saved_pdf = Document::load(&path).unwrap();
        verify_links(&saved_pdf, &order);
        let metadata = saved_pdf.catalog().unwrap().get(b"Metadata").unwrap().as_reference().unwrap();
        let xml = String::from_utf8(saved_pdf.get_object(metadata).unwrap().as_stream().unwrap()
            .decompressed_content_strict_with_limit(1024 * 1024).unwrap()).unwrap();
        let tree = roxmltree::Document::parse(&xml).unwrap();
        assert_eq!(tree.descendants().filter(|node| node.has_tag_name(("urn:folio-occurrence", "Retained")))
            .map(|node| node.text().unwrap()).collect::<Vec<_>>(), vec!["original catalog"]);
        let field = engine
            .list_form_fields(&result.document.id, result.page_indices[selected])
            .unwrap()
            .remove(0);
        let filled = engine
            .fill_form_field(
                &result.document.id,
                FillFormFieldRequest {
                    page_index: result.page_indices[selected],
                    annotation_index: field.annotation_index,
                    expected_value: field.value,
                    value: FormFieldValue::Text {
                        text: "Updated shared".into(),
                    },
                },
            )
            .unwrap();
        for page in 0..3 {
            assert_eq!(
                engine.list_form_fields(&filled.id, page).unwrap()[0].value,
                FormFieldValue::Text {
                    text: "Updated shared".into()
                }
            );
        }
        engine.close_document(&filled.id).unwrap();
        engine.close_document(&result.document.id).unwrap();
    }
    assert_eq!(
        fs::read(temp.path().join("original.pdf")).unwrap(),
        original
    );
    engine.close_document(&source.id).unwrap();
    engine.close_document(&incoming.id).unwrap();
}

#[test]
fn composed_save_refuses_non_xmp_metadata_without_changing_inputs_or_destination() {
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    let source = open(&engine, &temp, "original.pdf", &mut fixture(false));
    let incoming = open(&engine, &temp, "incoming.pdf", &mut fixture(true));
    let original = engine.source_bytes(&source.id).unwrap();
    let incoming_bytes = engine.source_bytes(&incoming.id).unwrap();
    let result = engine.replace_page_content_occurrence(&source.id, &[0, 1, 0, 0], 0, &incoming.id, 0).unwrap();
    let candidate = engine.source_bytes(&result.document.id).unwrap();
    let output = temp.path().join("existing.pdf");
    fs::write(&output, b"preserve existing destination").unwrap();
    let error = engine.export_pdf(ExportRequest {
        pages: plans(&result.document, &result.page_indices), flatten: false,
    }, &output).unwrap_err();
    assert!(error.to_string().contains("Document metadata is malformed"), "{error}");
    assert_eq!(fs::read(&output).unwrap(), b"preserve existing destination");
    assert_eq!(engine.source_bytes(&source.id).unwrap(), original);
    assert_eq!(engine.source_bytes(&incoming.id).unwrap(), incoming_bytes);
    assert_eq!(engine.source_bytes(&result.document.id).unwrap(), candidate);
    assert_eq!(fs::read(temp.path().join("original.pdf")).unwrap(), original.as_ref());
    assert_eq!(fs::read(temp.path().join("incoming.pdf")).unwrap(), incoming_bytes.as_ref());
    for id in [&result.document.id, &source.id, &incoming.id] { engine.close_document(id).unwrap(); }
}

#[test]
fn nonduplicate_selection_and_invalid_occurrences_leave_sources_intact() {
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    let source = open(&engine, &temp, "source.pdf", &mut fixture(false));
    let incoming = open(&engine, &temp, "incoming.pdf", &mut fixture(true));
    for (order, selected) in [
        (vec![], 0),
        (vec![0], 1),
        (vec![2], 0),
        (vec![0; 100_001], 0),
    ] {
        assert!(engine
            .replace_page_content_occurrence(&source.id, &order, selected, &incoming.id, 0)
            .is_err());
    }
    assert!(engine
        .replace_page_content_occurrence(&source.id, &[0, 0], 0, &incoming.id, 2)
        .is_err());
    let result = engine
        .replace_page_content_occurrence(&source.id, &[0, 1, 0], 1, &incoming.id, 0)
        .unwrap();
    assert_eq!(result.page_indices, vec![0, 1, 0]);
    assert_eq!(result.document.pages.len(), 2);
    assert_eq!(
        engine.render_page(&result.document.id, 0, 600).unwrap(),
        engine.render_page(&source.id, 0, 600).unwrap()
    );
    let error = engine.list_text_runs(&result.document.id, 1).unwrap_err();
    // Keep the existing isolated-text admission rule: shared fields must not
    // silently unlink. A separate ordinary-text control exercises editability.
    assert!(error.to_string().contains("shared fields"));
    for id in [&result.document.id, &source.id, &incoming.id] {
        engine.close_document(id).unwrap();
    }
}

fn capture(label: &str, files: &[(&str, &[u8])]) {
    if let Some(root) = std::env::var_os("FOLIO_OCCURRENCE_OUTPUT") {
        use std::io::Write;
        let root = PathBuf::from(root);
        fs::create_dir_all(&root).unwrap();
        for (suffix, bytes) in files {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(root.join(format!("{label}-{suffix}.pdf")))
                .unwrap()
                .write_all(bytes)
                .unwrap();
        }
    }
}

#[test]
fn native_frames_remain_independent_of_workspace_crop_and_rotation() {
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    let incoming = open(&engine, &temp, "incoming.pdf", &mut fixture(true));
    for rotation in [0, 90, 180, 270] {
        for unit in [1., 1.7] {
            for selected in [0, 2] {
                let label = format!("r{rotation}-u{unit}-s{selected}");
                let mut pdf = fixture(false);
                let id = *pdf.get_pages().values().next().unwrap();
                let page = pdf.get_dictionary_mut(id).unwrap();
                page.set(
                    "MediaBox",
                    vec![(-20).into(), (-10).into(), 300.into(), 400.into()],
                );
                page.set(
                    "CropBox",
                    vec![10.into(), 20.into(), 290.into(), 380.into()],
                );
                page.set("Rotate", rotation);
                page.set("UserUnit", unit);
                let source = open(&engine, &temp, &format!("{label}.pdf"), &mut pdf);
                let result = engine
                    .replace_page_content_occurrence(
                        &source.id,
                        &[0, 1, 0],
                        selected,
                        &incoming.id,
                        0,
                    )
                    .unwrap();
                assert_eq!(result.page_indices, vec![0, 1, 2]);
                let raw = engine.source_bytes(&result.document.id).unwrap();
                let output = Document::load_mem(&raw).unwrap();
                let old = pdf.get_dictionary(id).unwrap();
                for index in [0, 2] {
                    assert_eq!(result.document.pages[index], source.pages[0]);
                    let id = *output.get_pages().values().nth(index).unwrap();
                    let page = output.get_dictionary(id).unwrap();
                    for key in [b"MediaBox".as_slice(), b"CropBox", b"Rotate", b"UserUnit"] {
                        let numeric = |value: &Object| match value {
                            Object::Array(values) => values
                                .iter()
                                .map(|v| v.as_float().unwrap())
                                .collect::<Vec<_>>(),
                            value => vec![value.as_float().unwrap()],
                        };
                        assert_eq!(
                            numeric(page.get(key).unwrap()),
                            numeric(old.get(key).unwrap()),
                            "{label} {key:?}"
                        );
                    }
                }
                let other = 2 - selected;
                let mut before = plans(&source, &[0]).remove(0);
                let mut after = plans(&result.document, &[other]).remove(0);
                let crop = folio_engine::AnnotationRect {
                    x: 20.,
                    y: 20.,
                    width: 100.,
                    height: 120.,
                };
                before.crop = Some(crop.clone());
                after.crop = Some(crop);
                before.rotation = 90;
                after.rotation = 90;
                assert_eq!(
                    engine.render_page_plan(before, 500).unwrap(),
                    engine.render_page_plan(after, 500).unwrap(),
                    "{label}"
                );
                capture(
                    &label,
                    &[
                        ("source", &engine.source_bytes(&source.id).unwrap()),
                        ("incoming", &engine.source_bytes(&incoming.id).unwrap()),
                        ("result", &raw),
                    ],
                );
                engine.close_document(&result.document.id).unwrap();
                engine.close_document(&source.id).unwrap();
            }
        }
    }
    engine.close_document(&incoming.id).unwrap();
}

#[path = "support/protected_fixture.rs"]
mod protected_fixture;

#[test]
fn protected_occurrences_keep_credentials_origin_and_recovery_permissions() {
    use folio_engine::{EngineError, RecoveryStore};
    use lopdf::encryption::Permissions;
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    let incoming = open(&engine, &temp, "incoming.pdf", &mut fixture(true));
    for (name, rights, allowed) in [
        ("edit", Permissions::MODIFIABLE, false),
        ("assemble", Permissions::ASSEMBLABLE, false),
        (
            "both",
            Permissions::MODIFIABLE | Permissions::ASSEMBLABLE,
            true,
        ),
    ] {
        let mut plain = Vec::new();
        fixture(false).save_to(&mut plain).unwrap();
        let encrypted = protected_fixture::encrypt(&plain, rights);
        let path = temp.path().join(format!("{name}.pdf"));
        fs::write(&path, &encrypted).unwrap();
        let source = engine.open_document_with_password(&path, "reader").unwrap();
        let operation =
            engine.replace_page_content_occurrence(&source.id, &[0, 1, 0], 2, &incoming.id, 0);
        if allowed {
            let result = operation.unwrap();
            assert_eq!(result.document.protection, source.protection);
            let pages = plans(&result.document, &result.page_indices);
            let store = RecoveryStore::new(temp.path().join(format!("{name}-recovery"))).unwrap();
            let workspace = serde_json::json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","dirty":true,"savedDigest":"old","zoom":100,"scrollPosition":{"top":0,"left":0},"document":{"name":"Protected.pdf","pages":pages,"selectedPageId":"p0","selectedOverlayId":null}}]});
            store.save(&engine, workspace).unwrap();
            engine.close_document(&result.document.id).unwrap();
            let recovered = store.load(&engine).unwrap().unwrap();
            let pages: Vec<PagePlan> =
                serde_json::from_value(recovered["tabs"][0]["document"]["pages"].clone()).unwrap();
            assert!(matches!(
                engine.extract_text(&pages[0].source_id, 0),
                Err(EngineError::PermissionDenied(_))
            ));
            let out = temp.path().join("recovered.pdf");
            engine
                .export_pdf(
                    ExportRequest {
                        pages: pages.clone(),
                        flatten: false,
                    },
                    &out,
                )
                .unwrap();
            for password in ["reader", "owner"] {
                let reopened = engine.open_document_with_password(&out, password).unwrap();
                if password == "reader" {
                    assert_eq!(
                        reopened.protection.as_ref().unwrap().permissions,
                        source.protection.as_ref().unwrap().permissions
                    );
                } else {
                    assert!(engine
                        .extract_text(&reopened.id, 2)
                        .unwrap()
                        .contains("New content"));
                    assert!(engine
                        .extract_text(&reopened.id, 0)
                        .unwrap()
                        .contains("Old content"));
                }
                engine.close_document(&reopened.id).unwrap();
            }
            engine.close_document(&pages[0].source_id).unwrap();
        } else {
            assert!(
                matches!(operation, Err(EngineError::PermissionDenied(_))),
                "{name}"
            );
        }
        assert_eq!(fs::read(&path).unwrap(), encrypted);
        engine.close_document(&source.id).unwrap();
    }
    engine.close_document(&incoming.id).unwrap();
}

#[test]
fn repeated_replacements_stop_adding_pages_once_occurrences_are_distinct() {
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    let mut source = open(&engine, &temp, "source.pdf", &mut fixture(false));
    let incoming = open(&engine, &temp, "incoming.pdf", &mut fixture(true));
    let mut order = vec![0, 1, 0, 0];
    let mut stable_count = None;
    for round in 0..12 {
        let selected = [0, 2, 3][round % 3];
        let result = engine
            .replace_page_content_occurrence(&source.id, &order, selected, &incoming.id, 0)
            .unwrap();
        assert!(result.document.pages.len() <= 4);
        let pdf = Document::load_mem(&engine.source_bytes(&result.document.id).unwrap()).unwrap();
        if round >= 4 {
            if let Some(count) = stable_count {
                assert_eq!(pdf.objects.len(), count);
            } else {
                stable_count = Some(pdf.objects.len());
            }
        }
        engine.close_document(&source.id).unwrap();
        source = result.document;
        order = result.page_indices;
    }
    assert_eq!(source.pages.len(), 4);
    engine.close_document(&source.id).unwrap();
    engine.close_document(&incoming.id).unwrap();
}

#[test]
fn replaced_occurrence_keeps_standard_text_editable() {
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    let mut pdf = fixture(false);
    pdf.catalog_mut().unwrap().remove(b"AcroForm");
    let ids: Vec<_> = pdf.get_pages().values().copied().collect();
    for id in ids {
        pdf.get_dictionary_mut(id).unwrap().remove(b"Annots");
    }
    let source = open(&engine, &temp, "source.pdf", &mut pdf);
    let incoming = open(&engine, &temp, "incoming.pdf", &mut fixture(true));
    let result = engine
        .replace_page_content_occurrence(&source.id, &[0, 1, 0], 2, &incoming.id, 0)
        .unwrap();
    let runs = engine.list_text_runs(&result.document.id, 2).unwrap();
    let run = runs.runs.iter().find(|r| r.text == "New content").unwrap();
    assert!(run.supported);
    let edited = engine
        .replace_text(
            &result.document.id,
            2,
            run.object_index,
            "New content",
            "Changed wording",
        )
        .unwrap();
    assert!(engine
        .extract_text(&edited.id, 0)
        .unwrap()
        .contains("Changed wording"));
    for id in [&edited.id, &result.document.id, &source.id, &incoming.id] {
        engine.close_document(id).unwrap();
    }
}
