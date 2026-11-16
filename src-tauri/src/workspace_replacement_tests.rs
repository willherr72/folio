use super::*;
use lopdf::{dictionary,Document,Object,Stream};
#[path="../tests/support/protected_fixture.rs"]mod protected_fixture;

fn fixture(text:&str,unit:f32)->Vec<u8>{
    let mut pdf=Document::with_version("1.7");let parent=pdf.new_object_id();
    let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let content=pdf.add_object(Stream::new(dictionary!{},format!("BT /F1 12 Tf 30 330 Td ({text}) Tj ET BT /F1 12 Tf 30 130 Td (Alpha hidden) Tj ET").into_bytes()));
    let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>parent,"UserUnit"=>Object::Real(unit),"MediaBox"=>vec![0.into(),0.into(),400.into(),500.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});
    pdf.objects.insert(parent,dictionary!{"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]}.into());let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>parent});pdf.trailer.set("Root",root);crate::pdf_output::save(&mut pdf).unwrap()
}
fn runtime()->WorkerRuntime{WorkerRuntime::new(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),Arc::new(FontRegistry::new())).unwrap()}
fn source(rt:&mut WorkerRuntime,unit:f32)->DocumentInfo{rt.register_generated_document(fixture("Alpha",unit),"Authored.pdf").unwrap()}
fn page(source:&str,id:&str,unit:f32)->PagePlan{PagePlan{id:id.into(),source_id:source.into(),page_index:0,width:400.*unit,height:500.*unit,rotation:90,crop:Some(AnnotationRect{x:10.*unit,y:100.*unit,width:380.*unit,height:150.*unit}),overlays:vec![]}}
fn added(text:&str)->Overlay{Overlay::Text(TextOverlay{id:"added".into(),x:40.,y:200.,text:text.into(),font_size:12.,font_name:TextFont::Helvetica,font_id:None,shaping:None,area:None,color:"#123456".into(),rotation:0})}
fn intent(query:&str,replacement:&str)->WorkspaceReplacementIntent{WorkspaceReplacementIntent{query:query.into(),replacement:replacement.into(),options:Default::default(),selected:None}}
fn plan(rt:&mut WorkerRuntime,pages:&[PagePlan],intent:&WorkspaceReplacementIntent)->EngineResult<WorkspaceReplacementPlan>{rt.plan_workspace_replacement(pages,intent,&Default::default())}

#[test]
fn workspace_replacement_plans_visible_imported_duplicate_and_added_matches_without_mutation(){
    let mut rt=runtime();let a=source(&mut rt,1.);let b=source(&mut rt,1.7);let before=rt.documents.len();
    let mut pages=vec![page(&a.id,"first",1.),page(&b.id,"imported",1.7),page(&a.id,"duplicate",1.)];pages[0].overlays.push(added("Alpha added"));
    let p=plan(&mut rt,&pages,&intent("Alpha","New")).unwrap();assert!(p.can_apply);assert_eq!(p.pages.len(),3);assert_eq!(p.selected_count,4);assert_eq!(p.blocked_count,0);assert_eq!(p.native_edits.len(),3);assert_eq!(p.overlay_edits.len(),1);
    assert_eq!(p.pages[0].matches.len(),2);assert_eq!(p.pages[0].matches[1].overlay_id.as_deref(),Some("added"));assert!((p.pages[1].matches[0].rects[0].x-51.).abs()<1.);
    assert_eq!(rt.documents.len(),before);let json=serde_json::to_value(&p).unwrap();assert!(json.get("nativeEdits").is_none()&&json.get("binding").is_none()&&json.get("overlayEdits").is_none());
}
#[test]
fn workspace_replacement_blocks_all_selected_failures_and_allows_explicit_subset(){
    let mut rt=runtime();let info=source(&mut rt,1.);let mut pages=vec![page(&info.id,"one",1.)];pages[0].overlays.push(added("Alpha"));
    let p=plan(&mut rt,&pages,&intent("Alpha","")).unwrap();assert!(!p.can_apply);assert_eq!(p.blocked_count,1);assert!(p.native_edits.is_empty()&&p.overlay_edits.is_empty());
    let mut chosen=intent("Alpha","");chosen.selected=Some(vec![WorkspaceReplacementSelection{page_id:"one".into(),match_index:1}]);
    let p=plan(&mut rt,&pages,&chosen).unwrap();assert!(p.can_apply);assert_eq!(p.overlay_edits[0].text,None);assert!(!p.pages[0].matches[0].selected);
    chosen.selected=Some(vec![WorkspaceReplacementSelection{page_id:"one".into(),match_index:2}]);assert!(plan(&mut rt,&pages,&chosen).is_err());
    chosen.selected=Some(vec![WorkspaceReplacementSelection{page_id:"foreign".into(),match_index:0}]);assert!(plan(&mut rt,&pages,&chosen).is_err());
}
#[test]
fn workspace_replacement_overlay_unicode_literal_multiline_and_page_qualified_ids(){
    let mut rt=runtime();let info=source(&mut rt,1.);let mut pages=vec![page(&info.id,"one",1.),page(&info.id,"two",1.)];
    for p in &mut pages{p.overlays.push(added("Before CAFÉ\nAfter"));}
    let mut chosen=intent("café","$&\\1");chosen.options.whole_words=true;chosen.selected=Some(vec![WorkspaceReplacementSelection{page_id:"two".into(),match_index:0}]);
    let p=plan(&mut rt,&pages,&chosen).unwrap();assert!(p.can_apply);assert_eq!(p.overlay_edits.len(),1);assert_eq!(p.overlay_edits[0].page_id,"two");assert_eq!(p.overlay_edits[0].text.as_deref(),Some("Before $&\\1\nAfter"));
    let original=pages[1].overlays[0].clone();let result=rt.preview_workspace_replacement(&pages,&p,&Default::default()).unwrap();assert!(result.sources.is_empty());assert_eq!(result.overlays.len(),1);assert_eq!(pages[1].overlays[0],original);
}
#[test]
fn workspace_replacement_rebinds_preview_to_model_and_preserves_original_bytes(){
    let mut rt=runtime();let info=source(&mut rt,1.);let pages=vec![page(&info.id,"one",1.),page(&info.id,"duplicate",1.)];let before=rt.documents.len();let bytes=rt.document(&info.id).unwrap().bytes().unwrap().as_ref().to_vec();
    let mut chosen=intent("Alpha","New");chosen.selected=Some(vec![WorkspaceReplacementSelection{page_id:"duplicate".into(),match_index:0}]);let p=plan(&mut rt,&pages,&chosen).unwrap();
    let mut stale=pages.clone();stale[0].rotation=180;assert!(rt.preview_workspace_replacement(&stale,&p,&Default::default()).is_err());
    let result=rt.preview_workspace_replacement(&pages,&p,&Default::default()).unwrap();assert_eq!(result.sources.len(),1);let candidate=&result.sources[0];assert_eq!(candidate.page_indices.len(),2);assert_ne!(candidate.page_indices[0],candidate.page_indices[1]);
    let text=|rt:&mut WorkerRuntime,index|rt.list_transaction_text_runs(&candidate.document.id,index,true).unwrap().runs.into_iter().map(|r|r.text).collect::<Vec<_>>().join("|");
    assert!(text(&mut rt,candidate.page_indices[0]).starts_with("Alpha"));assert!(text(&mut rt,candidate.page_indices[1]).starts_with("New"));
    assert_eq!(rt.document(&info.id).unwrap().bytes().unwrap().as_ref(),bytes.as_slice());rt.discard_unpublished_source(&candidate.document.id);assert_eq!(rt.documents.len(),before);
}
#[test]
fn workspace_replacement_refuses_overlay_growth_outside_page_and_keeps_blocked_rows(){
    let mut rt=runtime();let info=source(&mut rt,1.);let mut pages=vec![page(&info.id,"one",1.)];pages[0].overlays.push(added("Find"));
    let p=plan(&mut rt,&pages,&intent("Find",&"Long ".repeat(100))).unwrap();assert!(!p.can_apply);assert_eq!(p.blocked_count,1);assert!(p.pages[0].matches[0].blocker.is_some());assert!(p.overlay_edits.is_empty());
}
#[test]
fn workspace_replacement_noop_empty_selection_cancel_and_ambiguous_model_are_safe(){
    let mut rt=runtime();let info=source(&mut rt,1.);let pages=vec![page(&info.id,"one",1.)];
    assert!(!plan(&mut rt,&pages,&intent("Alpha","Alpha")).unwrap().can_apply);
    let mut chosen=intent("Alpha","New");chosen.selected=Some(vec![]);assert!(!plan(&mut rt,&pages,&chosen).unwrap().can_apply);
    let control=crate::ReplacementPlanningControl::default();control.cancel();assert!(rt.plan_workspace_replacement(&pages,&intent("Alpha","New"),&control).is_err());
    assert!(plan(&mut rt,&[pages[0].clone(),pages[0].clone()],&intent("Alpha","New")).is_err());
    let mut invalid=pages.clone();invalid[0].width=401.;assert!(plan(&mut rt,&invalid,&intent("Alpha","New")).is_err());
}
#[test]
fn workspace_replacement_aggregate_limits_clear_every_executable_change(){
    let mut rt=runtime();let info=source(&mut rt,1.);let pages=(0..33).map(|i|page(&info.id,&format!("p{i}"),1.)).collect::<Vec<_>>();
    let p=plan(&mut rt,&pages,&intent("Alpha","New")).unwrap();assert!(!p.can_apply);assert!(p.batch_blocker.is_some());assert!(p.native_edits.is_empty()&&p.overlay_edits.is_empty());
}
#[test]
fn workspace_replacement_public_preview_keeps_fractional_physical_dimensions(){
    let dir=tempfile::tempdir().unwrap();let path=dir.path().join("Physical.pdf");std::fs::write(&path,fixture("Alpha",1.7)).unwrap();
    let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();let info=engine.open_document(&path).unwrap();let pages=vec![page(&info.id,"one",1.7)];
    let p=engine.plan_workspace_replacement(&pages,&intent("Alpha","New"),Default::default()).unwrap();let result=engine.preview_workspace_replacement(&pages,&p,Default::default()).unwrap();
    assert_eq!(result.sources.len(),1);let candidate=&result.sources[0];assert!((candidate.document.pages[0].width-680.).abs()<0.01);assert!((candidate.document.pages[0].height-850.).abs()<0.01);
    engine.close_document(&candidate.document.id).unwrap();engine.close_document(&info.id).unwrap();
}
#[test]
fn workspace_replacement_added_text_uses_annotation_rights_independently_of_native_edit_rights(){
    use lopdf::encryption::Permissions;
    let dir=tempfile::tempdir().unwrap();let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
    for rights in [Permissions::all()&!Permissions::MODIFIABLE,Permissions::all()&!Permissions::ANNOTABLE,Permissions::all()&!Permissions::COPYABLE]{
        let path=dir.path().join(format!("{}.pdf",rights.bits()));std::fs::write(&path,protected_fixture::encrypt(&fixture("Alpha",1.),rights)).unwrap();let info=engine.open_document_with_password(&path,"reader").unwrap();
        let mut pages=vec![page(&info.id,"one",1.)];pages[0].crop=None;pages[0].overlays.push(added("Alpha"));
        let mut chosen=intent("Alpha","New");chosen.selected=Some(vec![WorkspaceReplacementSelection{page_id:"one".into(),match_index:2}]);
        let p=engine.plan_workspace_replacement(&pages,&chosen,Default::default());
        if !rights.contains(Permissions::COPYABLE){assert!(p.is_err());}else{let p=p.unwrap();assert_eq!(p.can_apply,rights.contains(Permissions::ANNOTABLE));if p.can_apply{let result=engine.preview_workspace_replacement(&pages,&p,Default::default()).unwrap();assert!(result.sources.is_empty());assert_eq!(result.overlays.len(),1);}}
        engine.close_document(&info.id).unwrap();
    }
}
#[cfg(feature="shaped-text")]
#[test]
fn workspace_replacement_shaped_unicode_and_saved_area_overflow_use_native_layout(){
    let mut rt=runtime();let info=source(&mut rt,1.);let font=rt.fonts.register(std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/shaped-text/DejaVuSans.ttf")).unwrap()).unwrap();
    let mut pages=vec![page(&info.id,"one",1.)];let Overlay::Text(mut text)=added("😀 Straße ος")else{unreachable!()};text.font_id=Some(font.id);text.shaping=Some(TextShaping{version:1,direction:ShapingDirection::Ltr,ligatures:true});pages[0].overlays.push(Overlay::Text(text.clone()));
    let p=plan(&mut rt,&pages,&intent("STRASSE","Road")).unwrap();assert!(p.can_apply);assert_eq!(p.overlay_edits[0].text.as_deref(),Some("😀 Road ος"));
    text.text="Fit".into();text.area=Some(TextAreaSettings{version:1,width:100.,height:40.,inset:2.,line_spacing:1.2,alignment:TextAreaAlignment::Left});pages[0].overlays[0]=Overlay::Text(text);
    let p=plan(&mut rt,&pages,&intent("Fit",&"Words ".repeat(50))).unwrap();assert!(!p.can_apply);assert_eq!(p.blocked_count,1);assert!(p.pages[0].matches[0].blocker.is_some());
}
#[test]
fn workspace_replacement_counts_added_objects_and_rejects_invalid_selected_pairs(){
    let mut rt=runtime();let info=source(&mut rt,1.);let mut pages=vec![page(&info.id,"one",1.)];
    pages[0].overlays=(0..65).map(|i|{let Overlay::Text(mut text)=added("Find")else{unreachable!()};text.id=format!("added{i}");Overlay::Text(text)}).collect();
    let p=plan(&mut rt,&pages,&intent("Find","New")).unwrap();assert!(!p.can_apply);assert!(p.overlay_edits.is_empty());assert!(p.batch_blocker.is_some());
    let selection=WorkspaceReplacementSelection{page_id:"one".into(),match_index:0};let mut chosen=intent("Find","New");chosen.selected=Some(vec![selection.clone(),selection]);assert!(plan(&mut rt,&pages,&chosen).is_err());
}
