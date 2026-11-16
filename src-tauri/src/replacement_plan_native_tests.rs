use super::*;
use crate::engine::*;
use lopdf::{dictionary,Document,Object,Stream};
use sha2::{Digest,Sha256};
use std::fs;
#[path="../tests/support/protected_fixture.rs"]
mod protected_fixture;

fn fixture(rotation:i64,unit:f32)->Vec<u8>{
    let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
    let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let leaf=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),200.into(),30.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}},b"BT /F1 12 Tf 1 0 0 1 5 10 Tm (Shared) Tj ET".to_vec()));
    let content=pdf.add_object(Stream::new(dictionary!{},b"BT /F1 12 Tf 1 0 0 1 30 330 Tm (Alpha) Tj ET BT /F1 12 Tf 1 0 0 1 30 280 Tm (Beta) Tj ET q 1 0 0 1 30 150 cm /Shared Do Q q 1 0 0 1 30 100 cm /Shared Do Q".to_vec()));
    let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),400.into(),500.into()],"Rotate"=>rotation,"UserUnit"=>Object::Real(unit),"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font},"XObject"=>dictionary!{"Shared"=>leaf}},"Contents"=>content});
    pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]}.into());
    let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",root);crate::pdf_output::save(&mut pdf).unwrap()
}
fn runtime()->WorkerRuntime{WorkerRuntime::new(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),Arc::new(FontRegistry::new())).unwrap()}
fn fingerprint(bytes:&[u8])->String{Sha256::digest(bytes).iter().map(|b|format!("{b:02x}")).collect()}
fn apply(rt:&mut WorkerRuntime,source:&str,edits:&[TextBatchEdit])->PageContentReplacement{
    let(reply,receive)=mpsc::channel();rt.handle_text_batch(crate::engine::text_batch::Request{source:source.into(),edits:edits.to_vec(),reply});receive.recv().unwrap().unwrap()
}
#[test]
fn native_replacement_planning_root_binds_exact_source_without_registering_a_candidate(){
    let mut rt=runtime();let bytes=fixture(0,1.);let info=rt.register_generated_document(bytes.clone(),"Plan.pdf").unwrap();let count=rt.documents.len();
    let plan=rt.plan_text_replacement_page_raw(&info.id,0,"Alpha","First",PageSearchOptions::default(),None).unwrap();
    assert_eq!(plan.source_fingerprint,fingerprint(&bytes));assert_eq!(plan.source_id,info.id);assert_eq!(plan.matches.len(),1);assert!(plan.can_apply);assert_eq!(plan.edits.len(),1);
    assert_eq!(plan.edits[0].expected_text,"Alpha");assert_eq!(plan.edits[0].replacement,"First");assert_eq!(rt.documents.len(),count);assert_eq!(rt.document(&info.id).unwrap().bytes().unwrap().as_ref(),bytes.as_slice());
    let result=apply(&mut rt,&info.id,&plan.edits);assert!(rt.extract_text(&result.document.id,0).unwrap().contains("First"));assert!(rt.extract_text(&info.id,0).unwrap().contains("Alpha"));rt.discard_unpublished_source(&result.document.id);
}
#[test]
fn native_replacement_planning_shared_occurrences_have_distinct_paths_and_selective_edits(){
    let mut rt=runtime();let info=rt.register_generated_document(fixture(0,1.),"Shared.pdf").unwrap();let count=rt.documents.len();
    let plan=rt.plan_text_replacement_page_raw(&info.id,0,"Shared","New",PageSearchOptions::default(),None).unwrap();assert_eq!(plan.matches.len(),2);assert_eq!(plan.edits.len(),2);assert!(plan.edits.iter().all(|e|e.object_path.len()==2));assert_ne!(plan.edits[0].object_path,plan.edits[1].object_path);
    let all=apply(&mut rt,&info.id,&plan.edits);assert_eq!(rt.extract_text(&all.document.id,0).unwrap().matches("New").count(),2);rt.discard_unpublished_source(&all.document.id);
    let one=rt.plan_text_replacement_page_raw(&info.id,0,"Shared","Only here",PageSearchOptions::default(),Some(&[0])).unwrap();assert_eq!(one.edits.len(),1);
    let changed=apply(&mut rt,&info.id,&one.edits);let text=rt.extract_text(&changed.document.id,0).unwrap();assert_eq!(text.matches("Only here").count(),1);assert_eq!(text.matches("Shared").count(),1);rt.discard_unpublished_source(&changed.document.id);assert_eq!(rt.documents.len(),count);
}
#[test]
fn native_replacement_planning_reports_cross_object_phrases_instead_of_omitting_them(){
    let mut rt=runtime();let info=rt.register_generated_document(fixture(0,1.),"Cross.pdf").unwrap();
    assert_eq!(rt.search_page(&info.id,0,"Alpha Beta",PageSearchOptions::default()).unwrap().matches.len(),1);
    let plan=rt.plan_text_replacement_page_raw(&info.id,0,"Alpha Beta","Both",PageSearchOptions::default(),None).unwrap();assert_eq!(plan.matches.len(),1);assert_eq!(plan.matches[0].blocker,Some(Blocker::CrossObject));assert!(plan.edits.is_empty());assert!(!plan.can_apply);
}
#[test]
fn native_replacement_planning_public_rotation_and_fractional_units_match_find_locations(){
    let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();let dir=tempfile::tempdir().unwrap();
    for rotation in [0,90,180,270]{
        let path=dir.path().join(format!("physical-{rotation}.pdf"));let bytes=fixture(rotation,1.7);fs::write(&path,&bytes).unwrap();let info=engine.open_document(&path).unwrap();
        let plan=engine.plan_text_replacement_page(&info.id,0,"Alpha","First",PageSearchOptions::default(),None).unwrap();assert_eq!(plan.width,info.pages[0].width);assert_eq!(plan.height,info.pages[0].height);assert!(plan.can_apply,"rotation{rotation}: {:?}",plan.matches);
        let found=engine.search_page(&info.id,0,"Alpha").unwrap();assert_eq!(plan.matches.len(),found.matches.len());assert_eq!(plan.matches[0].rects,found.matches[0]);assert_eq!(engine.source_bytes(&info.id).unwrap().as_ref(),bytes.as_slice());assert_eq!(fs::read(&path).unwrap(),bytes);engine.close_document(&info.id).unwrap();
    }
}
#[test]
fn native_replacement_planning_requires_copy_and_edit_even_for_noop(){
    use lopdf::encryption::Permissions;
    let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();let dir=tempfile::tempdir().unwrap();
    for(name,permissions)in[("allowed",Permissions::all()),("no-copy",Permissions::all()&!Permissions::COPYABLE),("read-only",Permissions::all()&!Permissions::MODIFIABLE)]{
        let bytes=protected_fixture::encrypt(&fixture(0,1.),permissions);let path=dir.path().join(format!("{name}.pdf"));fs::write(&path,&bytes).unwrap();let info=engine.open_document_with_password(&path,"reader").unwrap();
        let working_before=engine.source_bytes(&info.id).unwrap();
        let result=engine.plan_text_replacement_page(&info.id,0,"Alpha","Alpha",PageSearchOptions::default(),None);
        if name=="allowed"{let plan=result.unwrap();assert_eq!(plan.matches.len(),1);assert!(plan.edits.is_empty());assert_eq!(plan.source_fingerprint,fingerprint(&working_before));}else{assert!(matches!(result,Err(EngineError::PermissionDenied(_))),"{result:?}");}
        assert_eq!(fingerprint(&engine.source_bytes(&info.id).unwrap()),fingerprint(&working_before));assert_eq!(fingerprint(&fs::read(&path).unwrap()),fingerprint(&bytes));engine.close_document(&info.id).unwrap();
    }
}
#[test]
fn native_replacement_planning_invalid_selection_or_page_returns_no_partial_plan(){
    let mut rt=runtime();let info=rt.register_generated_document(fixture(0,1.),"Bounds.pdf").unwrap();let count=rt.documents.len();
    let valid=rt.plan_text_replacement_page_raw(&info.id,0,"Alpha","First",PageSearchOptions::default(),None).unwrap();assert!(valid.can_apply);
    for(page,selected)in[(1,None),(0,Some(&[1][..])),(0,Some(&[0,0][..]))]{assert!(rt.plan_text_replacement_page_raw(&info.id,page,"Alpha","First",PageSearchOptions::default(),selected).is_err());}
    assert_eq!(rt.documents.len(),count);
}

fn single_stream(stream:&str)->Vec<u8>{
    let mut pdf=Document::load_mem(&fixture(0,1.)).unwrap();let page=*pdf.get_pages().values().next().unwrap();
    let content=pdf.add_object(Stream::new(dictionary!{},stream.as_bytes().to_vec()));pdf.get_dictionary_mut(page).unwrap().set("Contents",content);crate::pdf_output::save(&mut pdf).unwrap()
}
#[test]
fn native_replacement_planning_literal_space_keeps_source_and_selection_identity(){
    let mut rt=runtime();let bytes=single_stream("BT /F1 12 Tf 30 330 Td (Alpha words) Tj ET");let info=rt.register_generated_document(bytes.clone(),"Spaces.pdf").unwrap();
    let plan=rt.plan_text_replacement_page_raw(&info.id,0,"words","text",PageSearchOptions::default(),None).unwrap();assert!(plan.can_apply,"{:?}",plan.matches);assert_eq!(plan.edits[0].expected_text,"Alpha words");assert_eq!(plan.edits[0].replacement,"Alpha text");
    let changed=apply(&mut rt,&info.id,&plan.edits);assert!(rt.extract_text(&changed.document.id,0).unwrap().contains("Alpha text"));rt.discard_unpublished_source(&changed.document.id);assert_eq!(fingerprint(&rt.document(&info.id).unwrap().bytes().unwrap()),fingerprint(&bytes));
}
#[test]
fn native_replacement_planning_generated_separator_is_not_encoded_as_source_text(){
    let mut rt=runtime();let bytes=single_stream("BT /F1 12 Tf 30 330 Td [(Alpha) -700 (Beta)] TJ ET BT /F1 12 Tf 30 280 Td (Neighbor) Tj ET");let info=rt.register_generated_document(bytes.clone(),"Generated.pdf").unwrap();
    let listed=rt.list_transaction_text_runs(&info.id,0,true).unwrap();let first=listed.runs.iter().find(|r|r.text.starts_with("Alpha")).unwrap();assert!(first.text.contains("Alpha Beta"),"{first:?}");assert_eq!(first.source_text.as_deref(),Some("AlphaBeta"));
    let whole=rt.plan_text_replacement_page_raw(&info.id,0,"Alpha Beta","Both",PageSearchOptions::default(),None).unwrap();assert_eq!(whole.matches.len(),1);assert_eq!(whole.matches[0].blocker,Some(Blocker::Unmapped));assert!(whole.edits.is_empty());
    let part=rt.plan_text_replacement_page_raw(&info.id,0,"Beta","New",PageSearchOptions::default(),None).unwrap();assert!(part.can_apply,"{:?}",part.matches);assert_eq!(part.edits[0].expected_text,first.text);assert_eq!(part.edits[0].replacement,"AlphaNew");
    let changed=apply(&mut rt,&info.id,&part.edits);let text=rt.extract_text(&changed.document.id,0).unwrap();assert!(text.contains("New")&&text.contains("Neighbor")&&!text.contains("Beta"),"{text}");rt.discard_unpublished_source(&changed.document.id);assert_eq!(fingerprint(&rt.document(&info.id).unwrap().bytes().unwrap()),fingerprint(&bytes));
}

fn unicode_fixture(character:char)->Vec<u8>{
    let program=include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf");let face=ttf_parser::Face::parse(program,0).unwrap();let scale=1000./face.units_per_em()as f32;
    let glyphs=['A',character,'B','C'].map(|c|(c,face.glyph_index(c).expect("Public font must contain the authored scalar").0));
    let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();let file=pdf.add_object(Stream::new(dictionary!{"Length1"=>program.len()as i64},program.to_vec()));let bbox=face.global_bounding_box();
    let descriptor=pdf.add_object(dictionary!{"Type"=>"FontDescriptor","FontName"=>"DejaVuSans","Flags"=>32,"FontBBox"=>vec![Object::Real(bbox.x_min as f32*scale),Object::Real(bbox.y_min as f32*scale),Object::Real(bbox.x_max as f32*scale),Object::Real(bbox.y_max as f32*scale)],"ItalicAngle"=>0,"Ascent"=>Object::Real(face.ascender()as f32*scale),"Descent"=>Object::Real(face.descender()as f32*scale),"CapHeight"=>Object::Real(face.capital_height().unwrap_or(face.ascender())as f32*scale),"StemV"=>80,"FontFile2"=>file});
    let mut widths=Vec::new();let mut cmap=String::from("/CIDInit /ProcSet findresource begin 12 dict begin begincmap /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> def /CMapName /Authored def /CMapType 2 def 1 begincodespacerange <0000> <FFFF> endcodespacerange 4 beginbfchar\n");
    for(c,gid)in glyphs {widths.push(Object::Integer(gid as i64));widths.push(Object::Array(vec![Object::Real(face.glyph_hor_advance(ttf_parser::GlyphId(gid)).unwrap()as f32*scale)]));let mut encoded=[0u16;2];let unicode=c.encode_utf16(&mut encoded).iter().map(|u|format!("{u:04X}")).collect::<String>();cmap.push_str(&format!("<{gid:04X}> <{unicode}>\n"));}
    cmap.push_str("endbfchar endcmap CMapName currentdict /CMap defineresource pop end end");
    let map=pdf.add_object(Stream::new(dictionary!{},cmap.into_bytes()));let child=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"CIDFontType2","BaseFont"=>"DejaVuSans","CIDSystemInfo"=>dictionary!{"Registry"=>Object::string_literal("Adobe"),"Ordering"=>Object::string_literal("Identity"),"Supplement"=>0},"FontDescriptor"=>descriptor,"DW"=>1000,"W"=>widths,"CIDToGIDMap"=>"Identity"});
    let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type0","BaseFont"=>"DejaVuSans","Encoding"=>"Identity-H","DescendantFonts"=>vec![Object::Reference(child)],"ToUnicode"=>map});
    let encoded=glyphs[..3].iter().map(|(_,g)|format!("{g:04X}")).collect::<String>();let content=pdf.add_object(Stream::new(dictionary!{},format!("BT /F1 12 Tf 30 330 Td <{encoded}> Tj ET").into_bytes()));
    let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),400.into(),500.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]}.into());let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",root);crate::pdf_output::save(&mut pdf).unwrap()
}
#[test]
fn native_replacement_planning_unicode_preserves_ownership_and_existing_eligibility(){
    let mut rt=runtime();
    for(character,eligible)in [('😀',false),('Σ',true)]{
        let bytes=unicode_fixture(character);let info=rt.register_generated_document(bytes.clone(),"Unicode.pdf").unwrap();let query=character.to_string();let text=rt.read_page_text(&info.id,0,Some(250_000)).unwrap();assert!(text.characters.iter().any(|c|c.text==query));
        let listed=rt.list_transaction_text_runs(&info.id,0,true).unwrap();assert_eq!(listed.runs[0].supported,eligible,"{:?}",listed.runs[0]);
        let plan=rt.plan_text_replacement_page_raw(&info.id,0,&query,"C",PageSearchOptions::default(),None).unwrap();assert_eq!(plan.matches.len(),1);let range=plan.matches[0].source_range.as_ref().unwrap();assert_eq!((range.start,range.end),(1,1+character.len_utf8()));
        if eligible{
            assert!(plan.can_apply);assert!(plan.matches[0].source_reason.is_none());assert_eq!(plan.edits[0].expected_text,format!("A{character}B"));assert_eq!(plan.edits[0].replacement,"ACB");
            let changed=apply(&mut rt,&info.id,&plan.edits);assert!(rt.extract_text(&changed.document.id,0).unwrap().contains("ACB"));rt.discard_unpublished_source(&changed.document.id);
        }else{
            assert!(!plan.can_apply);assert!(plan.edits.is_empty());assert_eq!(plan.matches[0].blocker,Some(Blocker::Ineligible));assert_eq!(plan.matches[0].source_reason,listed.runs[0].reason);
        }
        assert_eq!(fingerprint(&rt.document(&info.id).unwrap().bytes().unwrap()),fingerprint(&bytes));
    }
}

fn ligature_fixture()->Vec<u8>{
    let mut pdf=Document::load_mem(&unicode_fixture('\u{fb03}')).unwrap();
    let map=pdf.objects.values().find_map(|value|value.as_dict().ok().filter(|dict|dict.get(b"Subtype").and_then(Object::as_name).ok()==Some(b"Type0")).map(|dict|dict.get(b"ToUnicode").unwrap().as_reference().unwrap())).unwrap();
    let stream=pdf.get_object_mut(map).unwrap().as_stream_mut().unwrap();let data=String::from_utf8(stream.get_plain_content_with_limit(16*1024*1024).unwrap()).unwrap();assert_eq!(data.matches("<FB03>").count(),1);stream.set_content(data.replace("<FB03>","<006600660069>").into_bytes());
    crate::pdf_output::save(&mut pdf).unwrap()
}
#[test]
fn native_replacement_planning_source_ligature_keeps_the_full_logical_match(){
    let bytes=ligature_fixture();let mut rt=runtime();let info=rt.register_generated_document(bytes.clone(),"Ligature.pdf").unwrap();
    let listed=rt.list_transaction_text_runs(&info.id,0,true).unwrap();assert_eq!(listed.runs[0].text,"AffiB");assert!(listed.runs[0].supported,"{:?}",listed.runs[0]);
    let plan=rt.plan_text_replacement_page_raw(&info.id,0,"ffi","C",PageSearchOptions::default(),None).unwrap();assert_eq!(plan.matches.len(),1);
    if !plan.can_apply {
        let page=rt.document(&info.id).unwrap().pdf().unwrap().pages().get(0).unwrap();let text=page.text().unwrap();
        println!("public authored ligature native chars: {:?}",text.chars().iter().map(|c|(c.index(),c.unicode_value(),c.is_generated(),c.tight_bounds())).collect::<Vec<_>>());
    }
    assert!(plan.can_apply,"{:?}",plan.matches);let range=plan.matches[0].source_range.as_ref().unwrap();assert_eq!((range.start,range.end),(1,4));assert_eq!(plan.edits[0].expected_text,"AffiB");assert_eq!(plan.edits[0].replacement,"ACB");
    let changed=apply(&mut rt,&info.id,&plan.edits);assert!(rt.extract_text(&changed.document.id,0).unwrap().contains("ACB"));rt.discard_unpublished_source(&changed.document.id);assert_eq!(fingerprint(&rt.document(&info.id).unwrap().bytes().unwrap()),fingerprint(&bytes));
}

#[test]
fn review_replacement_planning_normalized_root_reader_is_actually_reconciled(){
    let base=single_stream("BT /F1 12 Tf 30 330 Td (AB) Tj (CD) Tj (EF) Tj 0 -40 Td (GH) Tj (IJ) Tj ET");
    for rotation in [90,180,270]{
        let mut pdf=Document::load_mem(&base).unwrap();let page=*pdf.get_pages().values().next().unwrap();pdf.get_dictionary_mut(page).unwrap().set("Rotate",rotation);pdf.get_dictionary_mut(page).unwrap().set("UserUnit",Object::Real(1.7));let bytes=crate::pdf_output::save(&mut pdf).unwrap();let mut rt=runtime();let info=rt.register_generated_document(bytes.clone(),"Copied.pdf").unwrap();
        {let page=rt.document(&info.id).unwrap().pdf().unwrap().pages().get(0).unwrap();assert!(crate::engine::simple_forward_text(&page,&page.text().unwrap()));}
        let plan=rt.plan_text_replacement_page_raw(&info.id,0,"CD","XY",Default::default(),None).unwrap();assert!(!source::LAST_DIRECT.with(|v|v.get()));assert_eq!(plan.matches.len(),1);assert!(plan.can_apply,"rotation{rotation}: {:?}",plan.matches);
        let native=rt.list_transaction_text_runs(&info.id,0,true).unwrap().runs.into_iter().find(|run|vec![run.object_index]==plan.edits[0].object_path).unwrap();assert_eq!(plan.edits[0].expected_text,native.text);assert_eq!(native.source_text.as_deref().unwrap_or(&native.text),"CD");assert_eq!(plan.edits[0].replacement,"XY");assert_eq!(fingerprint(&rt.document(&info.id).unwrap().bytes().unwrap()),fingerprint(&bytes));
    }
}
#[test]
fn review_replacement_planning_copied_ligature_remains_visibly_unmappable(){
    let mut pdf=Document::load_mem(&ligature_fixture()).unwrap();let page=*pdf.get_pages().values().next().unwrap();let previous=pdf.get_dictionary(page).unwrap().get(b"Contents").unwrap().clone();let face=ttf_parser::Face::parse(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf"),0).unwrap();let c=face.glyph_index('C').unwrap().0;
    let extra=pdf.add_object(Stream::new(dictionary!{},format!("BT /F1 12 Tf 30 280 Td <{c:04X}{c:04X}> Tj ET").into_bytes()));let p=pdf.get_dictionary_mut(page).unwrap();p.set("Contents",vec![previous,Object::Reference(extra)]);p.set("Rotate",180);let bytes=crate::pdf_output::save(&mut pdf).unwrap();let mut rt=runtime();let info=rt.register_generated_document(bytes.clone(),"Copied ligature.pdf").unwrap();
    {let page=rt.document(&info.id).unwrap().pdf().unwrap().pages().get(0).unwrap();assert!(crate::engine::simple_forward_text(&page,&page.text().unwrap()));}
    let plan=rt.plan_text_replacement_page_raw(&info.id,0,"ffi","C",Default::default(),None).unwrap();assert!(!source::LAST_DIRECT.with(|v|v.get()));assert_eq!(plan.matches.len(),1);assert_eq!(plan.matches[0].blocker,Some(Blocker::Unmapped));assert!(plan.edits.is_empty());assert!(!plan.can_apply);assert_eq!(fingerprint(&rt.document(&info.id).unwrap().bytes().unwrap()),fingerprint(&bytes));
}
#[test]
fn review_replacement_planning_coincident_shared_invocations_do_not_emit_an_edit(){
    let mut pdf=Document::load_mem(&fixture(0,1.)).unwrap();let page=*pdf.get_pages().values().next().unwrap();let data=String::from_utf8(pdf.get_page_content_with_limit(page,16*1024*1024).unwrap()).unwrap();assert!(data.contains("30 100 cm"));let stream=pdf.add_object(Stream::new(dictionary!{},data.replace("30 100 cm","30 150 cm").into_bytes()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);let bytes=crate::pdf_output::save(&mut pdf).unwrap();let mut rt=runtime();let info=rt.register_generated_document(bytes.clone(),"Coincident.pdf").unwrap();let count=rt.documents.len();
    let plan=rt.plan_text_replacement_page_raw(&info.id,0,"Shared","New",Default::default(),None).unwrap();assert!(!plan.matches.is_empty());assert!(plan.edits.is_empty(),"{:?}",plan.matches);assert!(!plan.can_apply);assert_eq!(rt.documents.len(),count);assert_eq!(fingerprint(&rt.document(&info.id).unwrap().bytes().unwrap()),fingerprint(&bytes));
}
