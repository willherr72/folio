use super::*;
use lopdf::{dictionary, Object, Stream};

fn source()->Vec<u8> {
    let mut pdf=crate::pdf_output::load(&super::super::text_context_projection_tests::source(dictionary!{"Type"=>"Group","S"=>"Transparency","CS"=>"DeviceRGB"})).unwrap();
    let page=*pdf.get_pages().values().next().unwrap();
    let contents=pdf.add_object(Stream::new(dictionary!{},b"/Span << /MCID 0 /Lang (en-US) >> BDC q 0 0 300 400 re W n BT /F1 18 Tf 30 200 Td (Old content) Tj ET Q EMC".to_vec()));
    let dict=pdf.get_dictionary_mut(page).unwrap();dict.set("Contents",contents);dict.set("StructParents",0);
    let root=pdf.new_object_id();
    let owner=pdf.add_object(dictionary!{"Type"=>"StructElem","S"=>"P","P"=>root,"Pg"=>page,"K"=>0});
    let parents=pdf.add_object(dictionary!{"Nums"=>vec![0.into(),Object::Array(vec![owner.into()])]});
    pdf.objects.insert(root,dictionary!{"Type"=>"StructTreeRoot","K"=>owner,"ParentTree"=>parents}.into());pdf.catalog_mut().unwrap().set("StructTreeRoot",root);
    crate::pdf_output::save(&mut pdf).unwrap()
}
fn runtime()->WorkerRuntime {
    WorkerRuntime::new(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),Arc::new(FontRegistry::new())).unwrap()
}

#[test]
fn nonpainting_space_context_requires_source_native_bounds_and_admitted_origin() {
    let rt=runtime();let bytes=source();
    let document=rt.pdfium.load_pdf_from_byte_slice(&bytes,None).unwrap();
    let page=document.pages().get(0).unwrap();
    let bound=SourcePage::read(&bytes,0,&page).unwrap().unwrap();
    let context=&bound.context.shows[0].1;
    let mut blank=bound.runs[0].clone();blank.text.clear();
    blank.bounds=[blank.matrix[4],blank.matrix[5],blank.matrix[4],blank.matrix[5]];
    assert!(check_ink(context,&blank,&[],Some(" ")).is_ok());
    assert!(check_ink(context,&blank,&[],Some("   ")).is_ok());
    for source in [None,Some(""),Some("A"),Some("\t"),Some("\n"),Some("\u{a0}")] {
        assert!(check_ink(context,&blank,&[],source).is_err(),"{source:?}");
    }
    for case in ["ink","origin","nonfinite","invisible","unknown-text","displaced-bounds"] {
        let mut run=blank.clone();
        match case {
            "ink"=>{run.bounds[2]+=1.;run.bounds[3]+=1.;},
            "origin"=>{run.matrix[4]=-1.;run.bounds=[-1.,run.matrix[5],-1.,run.matrix[5]];},
            "nonfinite"=>run.matrix[0]=f32::NAN,
            "invisible"=>run.mode=PdfPageTextRenderMode::Invisible,
            "unknown-text"=>run.text="A".into(),
            _=>run.bounds=[0.;4],
        }
        assert!(check_ink(context,&run,&[],Some(" ")).is_err(),"{case}");
    }
    let outside=Glyph{text:"A".into(),position:[-2.,200.,-2.,200.,1.,210.]};
    assert!(check_ink(context,&blank,&[outside],Some(" ")).is_err());
}

#[test]
fn context_publication_refuses_pixel_equivalent_context_changes_and_cleans_candidates() {
    let mut rt=runtime();let bytes=source();let original=rt.register_generated_document(bytes.clone(),"Context.pdf").unwrap();
    let proof=rt.single_text_context(&original.id,0,0).unwrap().unwrap();
    let before=rt.documents.len();
    for case in ["clip","language","mcid","group"] {
        let mut changed=crate::pdf_output::load(&bytes).unwrap();let page=*changed.get_pages().values().next().unwrap();
        if case=="group" {changed.get_dictionary_mut(page).unwrap().set("Group",dictionary!{"S"=>"Transparency","CS"=>"DeviceRGB","I"=>true});}
        else {
            let mut ops=operations(&changed,page).unwrap();
            match case {
                "clip"=>ops.iter_mut().find(|o|o.operator=="re").unwrap().operands[2]=301.into(),
                "language"=>ops[0].operands[1].as_dict_mut().unwrap().set("Lang",Object::string_literal("fr-FR")),
                _=>ops[0].operands[1].as_dict_mut().unwrap().set("MCID",1),
            }
            let contents=changed.add_object(Stream::new(dictionary!{},Content{operations:ops}.encode().unwrap()));changed.get_dictionary_mut(page).unwrap().set("Contents",contents);
        }
        let bytes=crate::pdf_output::save(&mut changed).unwrap();
        let error=rt.publish_context_text_occurrence(&original.id,&[0],0,Some(&proof),|rt,_|rt.register_generated_document(bytes,"Changed context.pdf")).unwrap_err();
        assert!(error.to_string().contains("clipping or tag ownership"),"{case}: {error}");
        assert_eq!(rt.documents.len(),before,"{case}");
    }
}

#[test]
fn context_transaction_abandoned_reply_discards_complete_source_candidate() {
    let mut rt=runtime();let original=rt.register_generated_document(source(),"Context.pdf").unwrap();
    let before=rt.documents.len();
    let (reply,receive)=mpsc::channel();drop(receive);
    let (send,requests)=mpsc::channel();
    send.send(WorkerRequest::ReplaceTextOccurrence{source_id:original.id,occurrences:vec![0],selected_occurrence:0,object_index:0,expected_text:"Old content".into(),replacement:"New content".into(),font_id:None,reply}).unwrap();drop(send);
    rt.run(requests);
    assert_eq!(rt.documents.len(),before);
}

#[test]
fn substituted_context_rejects_changed_clips_marks_and_font_state() {
    let mut rt=runtime();let original=rt.register_generated_document(source(),"Font context.pdf").unwrap();
    let bytes=std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap();
    let font=rt.fonts.register(bytes).unwrap();
    let proof=rt.single_font_text_context(&original.id,0,0,"Old content","New content",Some(&font.id)).unwrap().unwrap();
    let isolated=rt.replace_transaction_text(&original.id,0,0,"Old content","New content",Some(&font.id),true).unwrap();
    let candidate=rt.editing_document(&isolated.id).unwrap().bytes().unwrap();
    let before=rt.documents.len();
    for case in ["clip","language","mcid","restore"] {
        let mut pdf=crate::pdf_output::load(&candidate).unwrap();let page=*pdf.get_pages().values().next().unwrap();let mut ops=operations(&pdf,page).unwrap();
        match case {
            "clip"=>ops.iter_mut().find(|op|op.operator=="re").unwrap().operands[2]=301.into(),
            "language"=>ops.iter_mut().find(|op|op.operator=="BDC").unwrap().operands[1].as_dict_mut().unwrap().set("Lang",Object::string_literal("fr-FR")),
            "mcid"=>ops.iter_mut().find(|op|op.operator=="BDC").unwrap().operands[1].as_dict_mut().unwrap().set("MCID",1),
            _=>ops.iter_mut().rev().find(|op|op.operator=="Tf").unwrap().operands[1]=19.into(),
        }
        let content=pdf.add_object(Stream::new(dictionary!{},Content{operations:ops}.encode().unwrap()));pdf.get_dictionary_mut(page).unwrap().set("Contents",content);
        let changed=crate::pdf_output::save(&mut pdf).unwrap();
        assert!(rt.publish_context_text_occurrence(&original.id,&[0],0,Some(&proof),|rt,_|rt.register_generated_document(changed,"Forged font context.pdf")).is_err(),"{case}");
        assert_eq!(rt.documents.len(),before,"{case}");
    }
}

#[test]
fn explicit_single_run_font_context_plan_requires_exact_source_operations_and_selection() {
    let mut rt=runtime();let original=rt.register_generated_document(source(),"Bound font.pdf").unwrap();
    let font=rt.fonts.register(std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
    let copy=rt.text_transaction_page_copy(&original.id,0,true).unwrap();let bytes=copy.save_to_bytes().unwrap();
    let fonts=FontProofs::read(&bytes).unwrap();let runs=snapshot(&copy.pages().get(0).unwrap(),Some(&fonts)).unwrap();
    let asset=rt.fonts.get(&font.id).unwrap();
    let plan=text_substitution::Plan::prepare(&bytes,&runs[0],&runs,"New content",&asset).unwrap();
    let mut proof=rt.single_text_context(&original.id,0,0).unwrap().unwrap();assert!(proof.bind_substitution(&plan).is_ok());
    for case in ["selection","clip","text"] {
        let mut proof=rt.single_text_context(&original.id,0,0).unwrap().unwrap();
        if case=="selection"{proof.ordinal+=1;}else{
            let mut operations=(*proof.operations).clone();
            if case=="clip"{operations.iter_mut().find(|op|op.operator=="re").unwrap().operands[2]=301.into();}
            else{operations.iter_mut().find(|op|op.operator=="Tj").unwrap().operands[0]=Object::string_literal("Different source");}
            proof.operations=Arc::new(operations);
        }
        assert!(proof.bind_substitution(&plan).is_err(),"{case}");
    }
}

#[test]
fn original_font_plan_bounds_replacement_before_encoding_embedded_text() {
    let mut rt=runtime();let original=rt.register_generated_document(source(),"Bound original font.pdf").unwrap();
    let font=rt.fonts.register(std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
    let edited=rt.replace_transaction_text(&original.id,0,0,"Old content","New content",Some(&font.id),true).unwrap();
    let source=rt.editing_document(&edited.id).unwrap();
    let lease=source.bytes().unwrap();
    let bytes=lease.as_ref();
    let fonts=FontProofs::read(bytes).unwrap();
    let runs=snapshot(&source.pdf().unwrap().pages().get(0).unwrap(),Some(&fonts)).unwrap();
    assert!(runs[0].proof.is_some());
    assert!(text_substitution::Plan::prepare_original(bytes,&runs[0],&runs,"New contents").is_ok());
    for text in ["N".repeat(4097),"é".repeat(2049),String::new()," ".into()] {
        assert!(text_substitution::Plan::prepare_original(bytes,&runs[0],&runs,&text).is_err(),"must reject {} UTF-8 bytes",text.len());
    }
}
