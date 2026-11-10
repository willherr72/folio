use super::*;
use super::structured_overlay_render::tests as fixtures;
use crate::composition_annotation_additions::{SourceLanguage,NewOwners};
use lopdf::Object;
fn tagged(rotation:u16,unit:f32)->Vec<u8>{
    let mut rt=fixtures::runtime();let info=fixtures::source(&mut rt,rotation,unit,b"q 0.2 0.4 0.8 rg 160 300 20 20 re f Q");let request=fixtures::request(&rt,&info);
    let stage=rt.prepare_structured_overlays(&request,&mut ||Ok(())).unwrap();let mut doc=crate::pdf_output::load(rt.document(&info.id).unwrap().bytes().unwrap().as_ref()).unwrap();doc.catalog_mut().unwrap().remove(b"SourceOnly");let page=doc.get_pages()[&1];let catalog=doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let language=SourceLanguage::capture(&doc,catalog).unwrap();let targets=[structured_overlay_publish::TargetPage{page_id:page,geometry:stage.pages[0].geometry,unit}];
    let attached=structured_overlay_publish::attach_generated(doc,&targets,stage,&mut ||Ok(())).unwrap();let ids=attached.pages[0].overlays.iter().map(|b|b.annotation_id).collect::<Vec<_>>();
    let mut result=crate::composition_annotation_additions::append(attached.document,&[NewOwners{page,annotations:&ids,language:&language}],&mut ||Ok(())).unwrap();crate::pdf_output::save(&mut result).unwrap()
}
fn open(rt:&mut WorkerRuntime,bytes:Vec<u8>)->DocumentInfo {
    let path=std::env::temp_dir().join(format!("folio-tagged-import-{}.pdf",Uuid::new_v4()));rt.open_bytes(bytes,path.clone(),path,true,true,false,OpenSecurity::Restored(None)).unwrap()
}
#[test]
fn tagged_overlay_import_reopens_renderer_text_ink_image_highlight_with_exact_source(){
    for rotation in [0,90,180,270]{for unit in [1.,1.7]{let bytes=tagged(rotation,unit);let mut rt=fixtures::runtime();let info=open(&mut rt,bytes.clone());
        assert_eq!(info.pages[0].overlays.len(),4,"tagged portable drawings and highlight must remain editable");
        let source=rt.document(&info.id).unwrap();assert_eq!(source.bytes().unwrap().as_ref(),bytes.as_slice());assert_eq!(source.imported_annotations[&0].len(),4);
        assert_eq!(source.pdf().unwrap().pages().get(0).unwrap().annotations().len(),1,"native comment stays native");
    }}
}
#[test]
fn tagged_overlay_import_does_not_promote_highlight_with_stale_tag_description(){
    let bytes=tagged(0,1.);let mut doc=crate::pdf_output::load(&bytes).unwrap();let page=doc.get_pages()[&1];let root=doc.catalog().unwrap().get(b"StructTreeRoot").unwrap().as_reference().unwrap();
    let ids=doc.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap().iter().map(|v|v.as_reference().unwrap()).collect::<Vec<_>>();
    let highlight=ids.into_iter().find(|id|doc.get_dictionary(*id).unwrap().get(b"Subtype").unwrap().as_name().unwrap()==b"Highlight").unwrap();
    let key=doc.get_dictionary(highlight).unwrap().get(b"StructParent").unwrap().as_i64().unwrap();let tree=doc.get_dictionary(root).unwrap().get(b"ParentTree").unwrap().as_reference().unwrap();
    let nums=doc.get_dictionary(tree).unwrap().get(b"Nums").unwrap().as_array().unwrap();let owner=nums.chunks_exact(2).find(|p|p[0].as_i64().unwrap()==key).unwrap()[1].as_reference().unwrap();
    doc.get_dictionary_mut(owner).unwrap().set("Alt",Object::string_literal("Authored highlight meaning"));
    let bytes=crate::pdf_output::save(&mut doc).unwrap();let mut rt=fixtures::runtime();let info=open(&mut rt,bytes);
    assert!(!info.pages[0].overlays.iter().any(|o|matches!(o,Overlay::Highlight(_))),"a tag description must not become stale through workspace editing");
}

fn ids(doc:&lopdf::Document)->(lopdf::ObjectId,lopdf::ObjectId,lopdf::ObjectId){
    let page=doc.get_pages()[&1];let list=doc.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap();
    let text=list.iter().map(|v|doc.dereference(v).unwrap().0.unwrap()).find(|id|doc.get_dictionary(*id).unwrap().get(b"Subtype").unwrap().as_name().unwrap()==b"FreeText").unwrap();
    let root=doc.catalog().unwrap().get(b"StructTreeRoot").unwrap().as_reference().unwrap();let tree=doc.get_dictionary(root).unwrap().get(b"ParentTree").unwrap();let nums=doc.dereference(tree).unwrap().1.as_dict().unwrap().get(b"Nums").unwrap().as_array().unwrap();
    let key=doc.get_dictionary(text).unwrap().get(b"StructParent").unwrap().as_i64().unwrap();let owner=nums.chunks_exact(2).find(|p|p[0].as_i64().unwrap()==key).unwrap()[1].as_reference().unwrap();(page,text,owner)
}
#[test]
fn tagged_overlay_import_refuses_stale_ownership_ancestors_and_changed_appearance(){
    let original=tagged(0,1.);
    for mode in ["missing-key","unknown-key","wrong-objr","owner-page","ancestor-alt","actual-text","attributes","wrong-p","repeat-slot","broken-ap"]{
        let mut doc=crate::pdf_output::load(&original).unwrap();let (page,text,owner)=ids(&doc);
        match mode {
            "missing-key"=>{doc.get_dictionary_mut(text).unwrap().remove(b"StructParent");},
            "unknown-key"=>doc.get_dictionary_mut(text).unwrap().set("StructParent",999),
            "wrong-objr"=>doc.get_dictionary_mut(owner).unwrap().get_mut(b"K").unwrap().as_dict_mut().unwrap().set("Obj",page),
            "owner-page"=>{let foreign=doc.add_object(lopdf::dictionary!{"Type"=>"Page"});doc.get_dictionary_mut(owner).unwrap().get_mut(b"K").unwrap().as_dict_mut().unwrap().set("Pg",foreign);},
            "ancestor-alt"=>{let root=doc.catalog().unwrap().get(b"StructTreeRoot").unwrap().as_reference().unwrap();doc.get_dictionary_mut(root).unwrap().set("Alt",Object::string_literal("Original tree description"));},
            "actual-text"=>doc.get_dictionary_mut(owner).unwrap().set("ActualText",Object::string_literal("Authored meaning")),
            "attributes"=>doc.get_dictionary_mut(owner).unwrap().set("A",lopdf::dictionary!{"O"=>"Layout"}),
            "wrong-p"=>doc.get_dictionary_mut(text).unwrap().set("P",owner),
            "repeat-slot"=>doc.get_dictionary_mut(page).unwrap().get_mut(b"Annots").unwrap().as_array_mut().unwrap().push(text.into()),
            "broken-ap"=>{let ap=doc.get_dictionary(text).unwrap().get(b"AP").unwrap().as_dict().unwrap().get(b"N").unwrap().as_reference().unwrap();let stream=doc.get_object_mut(ap).unwrap().as_stream_mut().unwrap();stream.decompress_with_limit(1024*1024).unwrap();let mut program=stream.content.clone();program.extend_from_slice(b" q 1 0 0 rg 0 0 30 30 re f Q");stream.set_content(program);},_=>unreachable!(),
        }
        let bytes=crate::pdf_output::save(&mut doc).unwrap();let mut rt=fixtures::runtime();let info=open(&mut rt,bytes.clone());
        assert!(!info.pages[0].overlays.iter().any(|o|matches!(o,Overlay::Text(_))),"{mode}");
        assert_eq!(rt.document(&info.id).unwrap().bytes().unwrap().as_ref(),bytes.as_slice());
        if matches!(mode,"actual-text"|"attributes"|"wrong-p"|"repeat-slot"|"broken-ap"){assert!(info.pages[0].overlays.iter().any(|o|matches!(o,Overlay::Image(_))),"unrelated valid drawing in {mode}");}
    }
}
#[test]
fn tagged_overlay_import_explicit_annotation_language_respects_pdf_version(){
    let original=tagged(0,1.);
    for (version,override_version,lang,expected) in [("1.7",false,"fr",false),("2.0",false,"fr",true),("1.7",true,"fr",true),("2.0",false,"en--US",false)]{
        let mut doc=crate::pdf_output::load(&original).unwrap();let (_,text,_)=ids(&doc);doc.version=version.into();if override_version{doc.catalog_mut().unwrap().set("Version",Object::Name(b"2.0".to_vec()));}
        doc.get_dictionary_mut(text).unwrap().set("Lang",Object::string_literal(lang));let bytes=crate::pdf_output::save(&mut doc).unwrap();let mut rt=fixtures::runtime();let info=open(&mut rt,bytes);
        assert_eq!(info.pages[0].overlays.iter().any(|o|matches!(o,Overlay::Text(_))),expected,"{version} {override_version} {lang}");
    }
}
#[test]
fn tagged_overlay_import_private_edit_save_reopen_keeps_tag_graph_and_editability(){
    use imported_overlay_sources::publication::OwnedChange;
    for rotation in [0,90,180,270]{for unit in [1.,1.7]{
        let mut bytes=tagged(rotation,unit);let mut rt=fixtures::runtime();
        for round in 0..2 {
            let info=open(&mut rt,bytes.clone());assert_eq!(info.pages[0].overlays.len(),4);
            let mut request=fixtures::request(&rt,&info);request.pages[0].overlays=info.pages[0].overlays.clone();
            let text=request.pages[0].overlays.iter_mut().find_map(|o|if let Overlay::Text(t)=o{Some(t)}else{None}).unwrap();text.text.push_str(" x");let expected=text.text.clone();
            let source=rt.document(&info.id).unwrap();let mut publication=source.imported_overlay_sources.publication(source.source_bytes.identity(),1,&mut ||Ok(())).unwrap();let changes=publication.page(0,&request.pages[0].overlays,&mut ||Ok(())).unwrap();
            let groups=changes.owned.iter().map(|c|match c{OwnedChange::Keep(s,_)|OwnedChange::Replace(s,_)|OwnedChange::Delete(s)=>s.source_group()}).collect::<Vec<_>>();
            let mut graph=crate::composition_graph::CompositionGraph::new();graph.import_bytes(0,source.bytes().unwrap().as_ref(),Arc::new(||false)).unwrap();let groups=graph.source_annotation_groups(0,&groups,&mut ||Ok(())).unwrap();let anchors=groups.iter().flat_map(|g|std::iter::once(g.annotation).chain(g.popups.iter().copied())).collect::<Vec<_>>();let origin=anchors[0].page;
            let languages=graph.source_languages(&mut ||Ok(())).unwrap();let doc=graph.assemble(&[0],&mut ||Ok(())).unwrap();let (mut doc,pages,receipts)=crate::composition_occurrences::project_forms_tracked(doc,&[origin],&anchors,&mut ||Ok(())).unwrap();
            let catalog=doc.trailer.get(b"Root").unwrap().as_reference().unwrap();let structure=crate::composition_structure::inspect(&doc,catalog,&pages,&mut ||Ok(())).unwrap();let before=crate::pdf_output::save(&mut doc.clone()).unwrap();
            let prepared=rt.prepare_structured_overlay_changes(&request,&[&changes],&mut ||Ok(())).unwrap();let targets=[structured_overlay_publish::TargetPage{page_id:pages[0],geometry:prepared.pages[0].geometry,unit}];
            let plans=[structured_overlay_batch::PagePlan{source_page:origin,source_page_index:0,source_language:&languages[&0],changes:&changes,groups:&groups}];
            doc=structured_overlay_batch::publish(doc,&targets,&plans,&receipts,prepared,&mut ||Ok(())).unwrap().document;
            assert_eq!(crate::composition_structure::inspect(&doc,catalog,&pages,&mut ||Ok(())).unwrap(),structure);
            let saved=crate::pdf_output::save(&mut doc).unwrap();assert_eq!(source.bytes().unwrap().as_ref(),bytes.as_slice());
            if let Some(root)=std::env::var_os("FOLIO_STRUCTURED_OVERLAY_CAPTURE_DIR"){
                use std::io::Write;let directory=PathBuf::from(root);std::fs::create_dir_all(&directory).unwrap();
                for (kind,data) in [("before",before.as_slice()),("after",saved.as_slice())]{std::fs::OpenOptions::new().write(true).create_new(true).open(directory.join(format!("tagged-reedit-{rotation}-{unit}-{round}-{kind}.pdf"))).unwrap().write_all(data).unwrap();}
            }
            let reopened=open(&mut rt,saved.clone());assert_eq!(reopened.pages[0].overlays.len(),4);assert!(reopened.pages[0].overlays.iter().any(|o|matches!(o,Overlay::Text(t) if t.text==expected)));bytes=saved;
        }
    }}
}

#[test]
fn tagged_overlay_import_valid_unowned_direct_drawing_keeps_existing_admission(){
    let mut doc=crate::pdf_output::load(&tagged(0,1.)).unwrap();let (page,text,owner)=ids(&doc);let root=doc.catalog().unwrap().get(b"StructTreeRoot").unwrap().as_reference().unwrap();
    let key=doc.get_dictionary(text).unwrap().get(b"StructParent").unwrap().as_i64().unwrap();
    doc.get_dictionary_mut(root).unwrap().get_mut(b"K").unwrap().as_array_mut().unwrap().retain(|v|v.as_reference().unwrap()!=owner);
    let tree=doc.get_dictionary(root).unwrap().get(b"ParentTree").unwrap().as_reference().unwrap();let nums=doc.get_dictionary(tree).unwrap().get(b"Nums").unwrap().as_array().unwrap().chunks_exact(2).filter(|pair|pair[0].as_i64().unwrap()!=key).flat_map(|pair|pair.iter().cloned()).collect::<Vec<_>>();doc.get_dictionary_mut(tree).unwrap().set("Nums",nums);
    let mut direct=doc.get_dictionary(text).unwrap().clone();direct.remove(b"StructParent");
    let list=doc.get_dictionary_mut(page).unwrap().get_mut(b"Annots").unwrap().as_array_mut().unwrap();*list.iter_mut().find(|v|v.as_reference().unwrap()==text).unwrap()=direct.into();
    let bytes=crate::pdf_output::save(&mut doc).unwrap();let mut rt=fixtures::runtime();let info=open(&mut rt,bytes);assert_eq!(info.pages[0].overlays.len(),4);assert!(info.pages[0].overlays.iter().any(|o|matches!(o,Overlay::Text(_))));
}
#[test]
fn tagged_overlay_import_valid_foreign_page_objr_does_not_authorize_source_annotation(){
    let mut doc=crate::pdf_output::load(&tagged(0,1.)).unwrap();let (page,_,owner)=ids(&doc);let tree=doc.get_dictionary(page).unwrap().get(b"Parent").unwrap().as_reference().unwrap();
    let foreign=doc.add_object(lopdf::dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),200.into(),200.into()],"Resources"=>lopdf::dictionary!{}});
    let pages=doc.get_dictionary_mut(tree).unwrap();pages.get_mut(b"Kids").unwrap().as_array_mut().unwrap().push(foreign.into());pages.set("Count",2);
    doc.get_dictionary_mut(owner).unwrap().get_mut(b"K").unwrap().as_dict_mut().unwrap().set("Pg",foreign);
    let catalog=doc.trailer.get(b"Root").unwrap().as_reference().unwrap();assert!(crate::composition_structure::inspect(&doc,catalog,&[page,foreign],&mut ||Ok(())).is_ok(),"fixture must pass global reverse ownership validation");
    let bytes=crate::pdf_output::save(&mut doc).unwrap();let mut rt=fixtures::runtime();let info=open(&mut rt,bytes);assert!(!info.pages[0].overlays.iter().any(|o|matches!(o,Overlay::Text(_))));assert!(info.pages[0].overlays.iter().any(|o|matches!(o,Overlay::Image(_))));
}
