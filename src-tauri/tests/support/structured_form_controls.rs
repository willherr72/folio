//! Native later-fill oracles: model values alone cannot prove copied controls work.
use super::*;
use folio_engine::{FillFormFieldRequest,FormFieldValue};

fn button_fixture(path:&Path,radio:bool,indexed:bool,unison:bool)->Vec<u8>{
    tagged_form(path);let mut doc=Document::load(path).unwrap();let page=doc.get_pages()[&1];
    let widget=doc.catalog().unwrap().get(b"AcroForm").unwrap().as_dict().unwrap().get(b"Fields").unwrap().as_array().unwrap()[0].as_reference().unwrap();
    let off=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),18.into(),18.into()]},b"0 0 0 RG 1 w 1 1 16 16 re S".to_vec()));
    let on=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),18.into(),18.into()]},b"0 0 0 RG 1 w 1 1 16 16 re S 3 3 m 15 15 l 3 15 m 15 3 l S".to_vec()));
    let first=if indexed{"0"}else if radio{"A"}else{"Yes"};
    let w=doc.get_dictionary_mut(widget).unwrap();
    for key in [b"FT".as_slice(),b"T",b"V",b"DV"]{w.remove(key);}
    w.set("Rect",vec![40.into(),100.into(),58.into(),118.into()]);
    w.set("AP",dictionary!{"N"=>dictionary!{"Off"=>off,first=>on}});w.set("AS",if radio{first}else{"Off"});
    if radio{
        let parent=doc.new_object_id();let second=if indexed{"1"}else{"B"};
        let other=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","P"=>page,"Parent"=>parent,"Rect"=>vec![75.into(),100.into(),93.into(),118.into()],"F"=>4,"AS"=>"Off","AP"=>dictionary!{"N"=>dictionary!{"Off"=>off,second=>on}},"StructParent"=>4});
        let mut field=dictionary!{"FT"=>"Btn","T"=>Object::string_literal("answer"),"Ff"=>32768+if unison{33554432}else{0},"V"=>first,"DV"=>first,"Kids"=>vec![Object::Reference(widget),other.into()]};
        if indexed{field.set("Opt",vec![Object::string_literal("A"),Object::string_literal("B")]);}
        doc.objects.insert(parent,field.into());doc.get_dictionary_mut(widget).unwrap().set("Parent",parent);
        doc.catalog_mut().unwrap().get_mut(b"AcroForm").unwrap().as_dict_mut().unwrap().set("Fields",vec![Object::Reference(parent)]);
        doc.get_dictionary_mut(page).unwrap().get_mut(b"Annots").unwrap().as_array_mut().unwrap().push(other.into());
        let root=doc.catalog().unwrap().get(b"StructTreeRoot").unwrap().as_reference().unwrap();
        let element=doc.add_object(dictionary!{"Type"=>"StructElem","S"=>"Form","P"=>root,"Pg"=>page,"K"=>dictionary!{"Type"=>"OBJR","Obj"=>other}});
        let parents=doc.get_dictionary(root).unwrap().get(b"ParentTree").unwrap().as_reference().unwrap();
        doc.get_dictionary_mut(parents).unwrap().get_mut(b"Nums").unwrap().as_array_mut().unwrap().extend([4.into(),element.into()]);
        doc.get_dictionary_mut(root).unwrap().get_mut(b"K").unwrap().as_array_mut().unwrap().push(element.into());
    }else{
        let w=doc.get_dictionary_mut(widget).unwrap();w.set("FT","Btn");w.set("T",Object::string_literal("answer"));w.set("Ff",0);w.set("V","Off");w.set("DV",first);
        if indexed{w.set("Opt",vec![Object::string_literal("Accepted")]);}
    }
    doc.save(path).unwrap();fs::read(path).unwrap()
}
fn fill(e:&PdfEngine,source:&DocumentInfo,page:usize,control:usize,value:FormFieldValue)->DocumentInfo{
    let field=e.list_form_fields(&source.id,page).unwrap().remove(control);
    assert!(field.reason.is_none(),"source field eligibility: {:?}",field.reason);
    e.fill_form_field(&source.id,FillFormFieldRequest{page_index:page,annotation_index:field.annotation_index,expected_value:field.value,value}).expect("native fill must succeed")
}
fn duplicate(e:&PdfEngine,source:&DocumentInfo,path:&Path)->DocumentInfo{
    let mut plan=request(&[source]);let mut copy=plan.pages[0].clone();copy.id="copied-controls".into();plan.pages.insert(1,copy);
    e.export_pdf(plan,path).expect("duplicate Save must succeed");e.open_document(path).unwrap()
}
fn toggle_roundtrip(radio:bool,indexed:bool,unison:bool){
    let e=engine();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("source.pdf");let original=button_fixture(&path,radio,indexed,unison);let source=e.open_document(&path).unwrap();let selected=usize::from(radio);let wanted=FormFieldValue::Toggle{checked:true};
    let original_image=e.render_page(&source.id,0,500).unwrap();
    let baseline=fill(&e,&source,0,selected,wanted.clone());let baseline_fields=e.list_form_fields(&baseline.id,0).unwrap();let expected=if radio{vec![false,true]}else{vec![true]};
    for (field,checked) in baseline_fields.iter().zip(&expected){assert_eq!(field.value,FormFieldValue::Toggle{checked:*checked},"original source must support intended fill");}
    let baseline_image=e.render_page(&baseline.id,0,500).unwrap();assert!(baseline_image!=original_image,"source fill must visibly change control");
    let output=dir.path().join("duplicated.pdf");let copied=duplicate(&e,&source,&output);check_structure(&Document::load(&output).unwrap(),if radio{9}else{7});
    for page in 0..2{assert!(e.render_page(&copied.id,page,500).unwrap()==original_image,"initial copy {page} differs");}
    let changed=fill(&e,&copied,1,selected,wanted);
    let saved=dir.path().join("filled.pdf");e.export_pdf(request(&[&changed]),&saved).unwrap();let reopened=e.open_document(&saved).unwrap();
    for stage in [&changed,&reopened]{for page in 0..2{
        let fields=e.list_form_fields(&stage.id,page).unwrap();assert_eq!(fields.len(),expected.len());
        for (index,(field,checked)) in fields.iter().zip(&expected).enumerate(){assert_eq!(field.value,FormFieldValue::Toggle{checked:*checked},"page {page}, control {index} shared answer");}
        assert!(e.render_page(&stage.id,page,500).unwrap()==baseline_image,"page {page} must match filling original source");
    }}
    check_structure(&Document::load(&saved).unwrap(),if radio{9}else{7});assert_eq!(fs::read(path).unwrap(),original);
    // Refill in the opposite direction from the original page, then duplicate
    // again. Named converted states must remain ordinary editable PDF fields.
    let restored=fill(&e,&reopened,0,0,FormFieldValue::Toggle{checked:radio});
    for page in 0..2{assert!(e.render_page(&restored.id,page,500).unwrap()==original_image,"reverse fill page {page}");}
    let third_path=dir.path().join("copied-again.pdf");let third=duplicate(&e,&restored,&third_path);
    for page in 0..3{assert!(e.render_page(&third.id,page,500).unwrap()==original_image,"repeated copy page {page}");}
    if let Some(root)=std::env::var_os("FOLIO_STRUCTURED_CONTROL_OUTPUT"){
        let root=PathBuf::from(root).join(format!("radio-{radio}-indexed-{indexed}-unison-{unison}"));fs::create_dir_all(&root).unwrap();
        for name in ["source.pdf","duplicated.pdf","filled.pdf","copied-again.pdf"]{fs::copy(dir.path().join(name),root.join(name)).unwrap();}
        e.export_pdf(request(&[&baseline]),&root.join("baseline-filled.pdf")).unwrap();
    }
}
#[test]fn structured_controls_checkbox_plain_refill(){toggle_roundtrip(false,false,false)}
#[test]fn structured_controls_checkbox_indexed_refill(){toggle_roundtrip(false,true,false)}
#[test]fn structured_controls_radio_plain_refill(){toggle_roundtrip(true,false,false)}
#[test]fn structured_controls_radio_plain_unison_refill(){toggle_roundtrip(true,false,true)}
#[test]fn structured_controls_radio_indexed_refill(){toggle_roundtrip(true,true,false)}
#[test]fn structured_controls_radio_indexed_unison_refill(){toggle_roundtrip(true,true,true)}

fn choice_roundtrip(list:bool){
    use folio_engine::{AnnotationRect,CreateFormFieldRequest,NewFormField};
    let e=engine();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("source.pdf");let original=fixture(&path,true,"Choice source");let opened=e.open_document(&path).unwrap();
    let options=vec!["First row".into(),"Second row".into()];
    let created=e.create_form_field_occurrence(&opened.id,&[0,1],0,CreateFormFieldRequest{bounds:AnnotationRect{x:150.,y:200.,width:180.,height:55.},rotation:0,name:"choice".into(),label:"Choice answer".into(),required:false,font_size:12.,field:if list{NewFormField::List{options,multi_select:true}}else{NewFormField::Dropdown{options}}}).unwrap();
    let value=FormFieldValue::Choice{indices:if list{vec![0,1]}else{vec![1]}};
    let baseline=fill(&e,&created.document,0,0,value.clone());let baseline_image=e.render_page(&baseline.id,0,500).unwrap();let image=e.render_page(&created.document.id,0,500).unwrap();assert!(baseline_image!=image);
    let output=dir.path().join("duplicated.pdf");let copied=duplicate(&e,&created.document,&output);
    for page in 0..2{assert!(e.render_page(&copied.id,page,500).unwrap()==image);}
    let changed=fill(&e,&copied,1,0,value.clone());let saved=dir.path().join("filled.pdf");e.export_pdf(request(&[&changed]),&saved).unwrap();let reopened=e.open_document(&saved).unwrap();
    for stage in [&changed,&reopened]{for page in 0..2{assert_eq!(e.list_form_fields(&stage.id,page).unwrap()[0].value,value);assert!(e.render_page(&stage.id,page,500).unwrap()==baseline_image,"choice copy {page} appearance");}}
    check_structure(&Document::load(&saved).unwrap(),5);assert_eq!(fs::read(path).unwrap(),original);
    if let Some(root)=std::env::var_os("FOLIO_STRUCTURED_CONTROL_OUTPUT"){
        let root=PathBuf::from(root).join(if list{"list"}else{"dropdown"});fs::create_dir_all(&root).unwrap();
        e.export_pdf(request(&[&created.document]),&root.join("source.pdf")).unwrap();
        e.export_pdf(request(&[&baseline]),&root.join("baseline-filled.pdf")).unwrap();
        for name in ["duplicated.pdf","filled.pdf"]{fs::copy(dir.path().join(name),root.join(name)).unwrap();}
    }
}
#[test]fn structured_controls_dropdown_refill(){choice_roundtrip(false)}
#[test]fn structured_controls_list_refill(){choice_roundtrip(true)}

#[test]
fn structured_controls_inherited_indexed_exports_refuse_without_publication(){
    let e=engine();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("inherited.pdf");button_fixture(&path,true,true,false);let mut doc=Document::load(&path).unwrap();
    let field=doc.catalog().unwrap().get(b"AcroForm").unwrap().as_dict().unwrap().get(b"Fields").unwrap().as_array().unwrap()[0].as_reference().unwrap();
    let options=doc.get_dictionary_mut(field).unwrap().remove(b"Opt").unwrap();let parent=doc.add_object(dictionary!{"T"=>Object::string_literal("group"),"Opt"=>options,"Kids"=>vec![Object::Reference(field)]});doc.get_dictionary_mut(field).unwrap().set("Parent",parent);
    doc.catalog_mut().unwrap().get_mut(b"AcroForm").unwrap().as_dict_mut().unwrap().set("Fields",vec![Object::Reference(parent)]);doc.save(&path).unwrap();let before=fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();
    let baseline=fill(&e,&source,0,1,FormFieldValue::Toggle{checked:true});assert_eq!(e.list_form_fields(&baseline.id,0).unwrap()[1].value,FormFieldValue::Toggle{checked:true});
    let mut plan=request(&[&source]);let mut copy=plan.pages[0].clone();copy.id="copy".into();plan.pages.insert(1,copy);let out=dir.path().join("destination.pdf");fs::write(&out,b"keep destination").unwrap();
    let result=e.export_pdf(plan,&out);assert!(result.is_err(),"inherited positional exports need a faithful ancestor adapter");assert_eq!(fs::read(out).unwrap(),b"keep destination");assert_eq!(fs::read(path).unwrap(),before);
}

#[test]
fn structured_review_null_options_refuse_without_publication(){
 for indirect in [false,true]{
  let e=engine();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("inherited-null.pdf");button_fixture(&path,true,true,true);let mut doc=Document::load(&path).unwrap();
  let field=doc.catalog().unwrap().get(b"AcroForm").unwrap().as_dict().unwrap().get(b"Fields").unwrap().as_array().unwrap()[0].as_reference().unwrap();
  let options=doc.get_dictionary_mut(field).unwrap().remove(b"Opt").unwrap();let parent=doc.add_object(dictionary!{"T"=>Object::string_literal("group"),"Opt"=>options,"Kids"=>vec![Object::Reference(field)]});
  let null=if indirect{Object::Reference(doc.add_object(Object::Null))}else{Object::Null};let d=doc.get_dictionary_mut(field).unwrap();d.set("Parent",parent);d.set("Opt",null);
  doc.catalog_mut().unwrap().get_mut(b"AcroForm").unwrap().as_dict_mut().unwrap().set("Fields",vec![Object::Reference(parent)]);doc.save(&path).unwrap();let before=fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();
  let mut plan=request(&[&source]);let mut copy=plan.pages[0].clone();copy.id="copy".into();plan.pages.insert(1,copy);let out=dir.path().join("destination.pdf");fs::write(&out,b"keep destination").unwrap();
  let error=e.export_pdf(plan,&out).err().expect("null Opt must not authorize an inherited indexed copy").to_string();assert!(error.contains("ancestor adapter"),"{error}");assert_eq!(fs::read(out).unwrap(),b"keep destination");assert_eq!(fs::read(path).unwrap(),before);
 }
}

#[test]
fn structured_review_inherited_signature_refuses_without_publication(){
 for indirect_null in [false,true]{
  let e=engine();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("inherited-signature.pdf");tagged_form(&path);let mut doc=Document::load(&path).unwrap();
  let field=doc.catalog().unwrap().get(b"AcroForm").unwrap().as_dict().unwrap().get(b"Fields").unwrap().as_array().unwrap()[0].as_reference().unwrap();
  let signature=doc.add_object(dictionary!{"Type"=>"Sig","Contents"=>Object::string_literal("signed bytes")});let parent=doc.add_object(dictionary!{"T"=>Object::string_literal("group"),"V"=>signature,"Kids"=>vec![Object::Reference(field)]});let alias=doc.add_object(Object::Reference(parent));let null=doc.add_object(Object::Null);
  let d=doc.get_dictionary_mut(field).unwrap();d.set("FT","Sig");d.set("Parent",alias);d.remove(b"V");d.remove(b"DV");if indirect_null{d.set("V",null);}
  doc.catalog_mut().unwrap().get_mut(b"AcroForm").unwrap().as_dict_mut().unwrap().set("Fields",vec![Object::Reference(parent)]);doc.save(&path).unwrap();let before=fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();
  let out=dir.path().join("destination.pdf");fs::write(&out,b"keep destination").unwrap();let mut plan=request(&[&source]);let mut copy=plan.pages[0].clone();copy.id="copy".into();plan.pages.insert(1,copy);
  let error=e.export_pdf(plan,&out).err().expect("signed inherited value must block composition").to_string();assert!(error.contains("signed form"),"{error}");assert_eq!(fs::read(out).unwrap(),b"keep destination");assert_eq!(fs::read(path).unwrap(),before);
 }
}

#[test]
fn structured_controls_duplicate_cancellation_keeps_destination_and_source(){
    let e=engine();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("source.pdf");let before=button_fixture(&path,true,true,false);let source=e.open_document(&path).unwrap();
    let mut plan=request(&[&source]);let mut copy=plan.pages[0].clone();copy.id="copy".into();plan.pages.insert(1,copy);
    for stage in ["checking","copying","preserving","applying","preparingFile"]{
        let out=dir.path().join(format!("cancel-{stage}.pdf"));fs::write(&out,b"keep destination").unwrap();let control=Arc::new(PdfSaveControl::new());let cancel=control.clone();let observed=Arc::new(AtomicBool::new(false));let seen=observed.clone();
        let report=Arc::new(move|event:PdfSaveProgress|{if event.stage==stage{seen.store(true,Ordering::SeqCst);cancel.cancel();}});
        assert!(!e.export_pdf_controlled(plan.clone(),None,&out,control,report).unwrap());assert!(observed.load(Ordering::SeqCst),"missing stage {stage}");assert_eq!(fs::read(out).unwrap(),b"keep destination");
    }
    assert_eq!(fs::read(path).unwrap(),before);
}

#[test]
fn structured_controls_duplicate_preserves_protection_and_requires_assembly(){
    use lopdf::encryption::Permissions;
    let e=engine();let dir=tempfile::tempdir().unwrap();let plain=dir.path().join("plain.pdf");let bytes=button_fixture(&plain,true,true,false);let encrypted=protected_fixture::encrypt(&bytes,Permissions::FILLABLE|Permissions::PRINTABLE);let path=dir.path().join("protected.pdf");fs::write(&path,&encrypted).unwrap();
    let reader=e.open_document_with_password(&path,"reader").unwrap();let mut plan=request(&[&reader]);let mut copy=plan.pages[0].clone();copy.id="copy".into();plan.pages.insert(1,copy);let out=dir.path().join("saved.pdf");fs::write(&out,b"keep destination").unwrap();assert!(e.export_pdf(plan,&out).is_err());assert_eq!(fs::read(&out).unwrap(),b"keep destination");
    let owner=e.open_document_with_password(&path,"owner").unwrap();let mut plan=request(&[&owner]);let mut copy=plan.pages[0].clone();copy.id="owner-copy".into();plan.pages.insert(1,copy);e.export_pdf(plan,&out).unwrap();assert!(e.open_document(&out).is_err());let saved=e.open_document_with_password(&out,"reader").unwrap();assert!(!saved.protection.as_ref().unwrap().permissions.assemble);
    let filled=fill(&e,&saved,1,1,FormFieldValue::Toggle{checked:true});for page in 0..2{let fields=e.list_form_fields(&filled.id,page).unwrap();assert_eq!(fields[0].value,FormFieldValue::Toggle{checked:false});assert_eq!(fields[1].value,FormFieldValue::Toggle{checked:true});}assert_eq!(fs::read(path).unwrap(),encrypted);
}

fn split_radio(path:&Path,current:&str,default:&str)->Vec<u8>{
    button_fixture(path,true,false,false);let mut doc=Document::load(path).unwrap();let pages=doc.get_pages();
    let other=doc.get_dictionary_mut(pages[&1]).unwrap().get_mut(b"Annots").unwrap().as_array_mut().unwrap().pop().unwrap().as_reference().unwrap();
    doc.get_dictionary_mut(pages[&2]).unwrap().set("Annots",vec![Object::Reference(other)]);doc.get_dictionary_mut(other).unwrap().set("P",pages[&2]);
    let field=doc.get_dictionary(other).unwrap().get(b"Parent").unwrap().as_reference().unwrap();let controls=doc.get_dictionary(field).unwrap().get(b"Kids").unwrap().as_array().unwrap().clone();
    for (index,control) in controls.iter().enumerate(){let on=if index==0{"A"}else{"B"};doc.get_dictionary_mut(control.as_reference().unwrap()).unwrap().set("AS",if current==on{on}else{"Off"});}
    doc.get_dictionary_mut(field).unwrap().set("V",current);doc.get_dictionary_mut(field).unwrap().set("DV",default);
    for value in doc.objects.values_mut(){if let Object::Dictionary(dict)=value{if dict.get(b"Type").and_then(Object::as_name).ok()==Some(b"StructElem") && dict.get(b"K").and_then(Object::as_dict).and_then(|k|k.get(b"Obj")).and_then(Object::as_reference).ok()==Some(other){dict.set("Pg",pages[&2]);}}}
    doc.save(path).unwrap();fs::read(path).unwrap()
}
fn removed_named_answer(current:&str,default:&str){
    let e=engine();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("split.pdf");let before=split_radio(&path,current,default);let source=e.open_document(&path).unwrap();
    assert_eq!(e.list_form_fields(&source.id,0).unwrap()[0].value,FormFieldValue::Toggle{checked:current=="A"});assert_eq!(e.list_form_fields(&source.id,1).unwrap()[0].value,FormFieldValue::Toggle{checked:current=="B"});
    let mut plan=request(&[&source]);plan.pages.remove(0);let out=dir.path().join("selected.pdf");fs::write(&out,b"keep destination").unwrap();
    assert!(e.export_pdf(plan,&out).is_err(),"partial radio selection must not strand the saved/default answer");assert_eq!(fs::read(out).unwrap(),b"keep destination");assert_eq!(fs::read(path).unwrap(),before);
}
#[test]fn structured_controls_selection_refuses_removed_current_named_answer(){removed_named_answer("A","B")}
#[test]fn structured_controls_selection_refuses_removed_default_named_answer(){removed_named_answer("B","A")}
#[test]
fn structured_controls_selection_keeps_surviving_named_default_and_later_fill(){
    let e=engine();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("split.pdf");let before=split_radio(&path,"Off","B");let source=e.open_document(&path).unwrap();let image=e.render_page(&source.id,1,500).unwrap();
    let baseline=fill(&e,&source,1,0,FormFieldValue::Toggle{checked:true});let baseline_image=e.render_page(&baseline.id,1,500).unwrap();let mut plan=request(&[&source]);plan.pages.remove(0);let out=dir.path().join("selected.pdf");e.export_pdf(plan,&out).unwrap();let selected=e.open_document(&out).unwrap();assert!(e.render_page(&selected.id,0,500).unwrap()==image);check_structure(&Document::load(&out).unwrap(),2);
    let filled=fill(&e,&selected,0,0,FormFieldValue::Toggle{checked:true});assert!(e.render_page(&filled.id,0,500).unwrap()==baseline_image);let saved=dir.path().join("selected-filled.pdf");e.export_pdf(request(&[&filled]),&saved).unwrap();let reopened=e.open_document(&saved).unwrap();assert!(e.render_page(&reopened.id,0,500).unwrap()==baseline_image);assert_eq!(e.list_form_fields(&reopened.id,0).unwrap()[0].value,FormFieldValue::Toggle{checked:true});assert_eq!(fs::read(path).unwrap(),before);
}
