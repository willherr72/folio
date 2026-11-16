use super::*;
use lopdf::{dictionary,Document,Object,Stream};
use super::super::replacement_plan::{TextReplacementMatch,TextReplacementBlocker};
#[path="../tests/support/protected_fixture.rs"]mod protected_fixture;

fn fixture(pages:&[Vec<&str>],shared:bool)->Vec<u8>{
    let mut pdf=Document::with_version("1.7");let parent=pdf.new_object_id();
    let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let leaf=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),150.into(),30.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}},b"BT /F1 12 Tf 5 10 Td (Shared) Tj ET".to_vec()));
    let mut kids=vec![];
    for(i,parts)in pages.iter().enumerate(){
        let mut data=String::new();for(j,text)in parts.iter().enumerate(){data.push_str(&format!("BT /F1 12 Tf 30 {} Td ({text}) Tj ET ",450-j*30));}
        if shared{data.push_str("q 1 0 0 1 30 100 cm /S Do Q q 1 0 0 1 30 50 cm /S Do Q");}
        let content=pdf.add_object(Stream::new(dictionary!{},data.into_bytes()));
        let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>parent,"MediaBox"=>vec![0.into(),0.into(),400.into(),500.into()],"UserUnit"=>Object::Real(if i%2==0{1.}else{1.7}),"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font},"XObject"=>dictionary!{"S"=>leaf}},"Contents"=>content});kids.push(Object::Reference(page));
    }
    pdf.objects.insert(parent,dictionary!{"Type"=>"Pages","Count"=>kids.len()as i64,"Kids"=>kids}.into());let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>parent});pdf.trailer.set("Root",root);crate::pdf_output::save(&mut pdf).unwrap()
}
fn runtime()->WorkerRuntime{WorkerRuntime::new(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),Arc::new(FontRegistry::new())).unwrap()}
fn plan(rt:&mut WorkerRuntime,id:&str,query:&str,replacement:&str,selected:Option<&[TextReplacementSelection]>)->EngineResult<TextReplacementDocumentPlan>{rt.plan_text_replacement_document_raw(id,query,replacement,Default::default(),selected,&Default::default())}

#[test]
fn document_replacement_planning_last_page_shared_and_physical_units_preserve_source(){
    let mut rt=runtime();let bytes=fixture(&[vec!["Alpha"],vec!["Alpha"]],true);let info=rt.register_generated_document(bytes.clone(),"Document.pdf").unwrap();let count=rt.documents.len();
    let roots=plan(&mut rt,&info.id,"Alpha","First",None).unwrap();assert_eq!((roots.total_pages,roots.scanned_pages,roots.match_count,roots.edits.len()),(2,2,2,2));assert!(roots.can_apply);assert_eq!(roots.pages[0].width,400.);assert_eq!(roots.pages[1].width,680.);assert_eq!(roots.edits[1].page_index,1);
    let shared=plan(&mut rt,&info.id,"Shared","New",None).unwrap();assert_eq!(shared.match_count,4);assert_eq!(shared.edits.len(),4);assert!(shared.edits.iter().all(|edit|edit.object_path.len()==2));
    let selected=[TextReplacementSelection{page_index:1,match_index:1}];let one=plan(&mut rt,&info.id,"Shared","New",Some(&selected)).unwrap();assert_eq!((one.match_count,one.selected_count,one.edits.len()),(4,1,1));assert_eq!(one.edits[0].page_index,1);
    assert_eq!(rt.documents.len(),count);assert_eq!(rt.document(&info.id).unwrap().bytes().unwrap().as_ref(),bytes.as_slice());
}
#[test]
fn document_replacement_planning_late_blocker_clears_all_child_edits_but_subset_is_allowed(){
    let mut rt=runtime();let info=rt.register_generated_document(fixture(&[vec!["Alpha tail"],vec!["Alpha"]],false),"Blocked.pdf").unwrap();
    let all=plan(&mut rt,&info.id,"Alpha","",None).unwrap();assert_eq!((all.match_count,all.blocked_count),(2,1));assert!(!all.can_apply);assert_eq!(all.batch_blocker,Some(DocumentReplacementBlocker::SelectedMatchesBlocked));assert!(all.edits.is_empty()&&all.pages.iter().all(|page|page.edits.is_empty()&&!page.can_apply));
    let one=plan(&mut rt,&info.id,"Alpha","",Some(&[TextReplacementSelection{page_index:0,match_index:0}])).unwrap();assert!(one.can_apply);assert_eq!(one.edits.len(),1);assert!(!one.pages[1].matches[0].selected);
}
#[test]
fn document_replacement_planning_selected_ordinals_noop_and_empty_selection(){
    let mut rt=runtime();let info=rt.register_generated_document(fixture(&[vec!["Alpha"],vec!["Gamma"]],false),"Ordinals.pdf").unwrap();
    for selected in [vec![TextReplacementSelection{page_index:2,match_index:0}],vec![TextReplacementSelection{page_index:1,match_index:0}],vec![TextReplacementSelection{page_index:0,match_index:1}],vec![TextReplacementSelection{page_index:0,match_index:0};2]]{assert!(plan(&mut rt,&info.id,"Alpha","New",Some(&selected)).is_err());}
    for selected in [None,Some(&[][..])]{let p=plan(&mut rt,&info.id,"Alpha","Alpha",selected).unwrap();assert_eq!(p.match_count,1);assert!(!p.can_apply);assert!(p.edits.is_empty());}
    let absent=plan(&mut rt,&info.id,"Absent","New",None).unwrap();assert_eq!(absent.scanned_pages,2);assert!(absent.pages.is_empty());assert!(!absent.can_apply);
}
#[test]
fn document_replacement_planning_scanned_pages_are_not_changed_page_limit(){
    let mut rt=runtime();let mut pages=vec![vec!["Other"];34];pages[33]=vec!["Alpha"];
    let info=rt.register_generated_document(fixture(&pages,false),"Sparse.pdf").unwrap();let p=plan(&mut rt,&info.id,"Alpha","New",None).unwrap();assert_eq!(p.scanned_pages,34);assert!(p.can_apply);assert_eq!(p.edits[0].page_index,33);
    for count in [32,33]{let info=rt.register_generated_document(fixture(&vec![vec!["Alpha"];count],false),"Pages.pdf").unwrap();let p=plan(&mut rt,&info.id,"Alpha","New",None).unwrap();assert_eq!(p.match_count,count);assert_eq!(p.can_apply,count==32);if count==33{assert_eq!(p.batch_blocker,Some(DocumentReplacementBlocker::TooManyPages));assert!(p.pages.iter().all(|page|page.edits.is_empty()));}}
}
#[test]
fn document_replacement_planning_global_edit_limit(){
    let mut rt=runtime();
    for count in [64,65]{let mut pages=vec![vec!["Alpha","Alpha","Alpha"];count/3];if count%3>0{pages.push(vec!["Alpha";count%3]);}let info=rt.register_generated_document(fixture(&pages,false),"Edits.pdf").unwrap();let p=plan(&mut rt,&info.id,"Alpha","New",None).unwrap();assert_eq!(p.match_count,count);assert_eq!(p.can_apply,count==64);if count==65{assert_eq!(p.batch_blocker,Some(DocumentReplacementBlocker::TooManyEdits));assert!(p.edits.is_empty());}}
}
fn synthetic_page(index:usize,count:usize,reason:usize)->TextReplacementPagePlan{
    TextReplacementPagePlan{source_id:"source".into(),source_fingerprint:"a".repeat(64),page_index:index,width:400.,height:500.,query:"a".into(),replacement:"b".into(),options:Default::default(),matches:(0..count).map(|i|TextReplacementMatch{match_index:i,character_start:i,character_end:i+1,rects:vec![],source_range:None,selected:false,blocker:Some(TextReplacementBlocker::Unmapped),source_reason:Some("x".repeat(reason))}).collect(),edits:vec![],selected_count:0,blocked_count:0,batch_blocker:None,can_apply:false}
}
#[test]
fn document_replacement_planning_global_match_and_serialized_limits(){
    let mut exact=Accumulator::new("source",&"a".repeat(64),11,"a","b",Default::default());for i in 0..10{exact.push(synthetic_page(i,1000,0)).unwrap();}assert_eq!(exact.plan.match_count,10000);assert!(exact.push(synthetic_page(10,1,0)).is_err());
    let mut large=Accumulator::new("source",&"a".repeat(64),100,"a","b",Default::default());let mut rejected=false;
    for i in 0..100{match large.push(synthetic_page(i,100,1024)){Ok(())=>assert!(i<99),Err(_)=>{rejected=true;break;}}}assert!(rejected);
}
#[test]
fn document_replacement_planning_global_native_byte_budget_across_pages(){
    for overflow in [false,true]{
        let mut result=Accumulator::new("source",&"a".repeat(64),32,"a","b",Default::default());
        for index in 0..32{
            let mut page=synthetic_page(index,2,0);page.selected_count=2;page.can_apply=true;
            for found in &mut page.matches{found.selected=true;found.blocker=None;}
            page.edits=(0..2).map(|object|crate::TextBatchEdit{page_index:index,object_path:vec![object],expected_text:"a".repeat(4096+usize::from(overflow)),replacement:"b".repeat(4096),font_id:None}).collect();
            result.push(page).unwrap();
        }
        let result=result.finish().unwrap();assert_eq!(result.can_apply,!overflow);assert_eq!(result.selected_count,64);
        if overflow{assert_eq!(result.batch_blocker,Some(DocumentReplacementBlocker::BatchBytes));assert!(result.edits.is_empty()&&result.pages.iter().all(|p|p.edits.is_empty()));}else{assert_eq!(result.edits.len(),64);}
    }
}
#[test]
fn document_replacement_planning_cross_object_late_match_and_success_progress(){
    let mut rt=runtime();let info=rt.register_generated_document(fixture(&[vec!["Alpha Beta"],vec!["Alpha","Beta"]],false),"Cross.pdf").unwrap();
    let(tx,rx)=mpsc::channel();let control=ReplacementPlanningControl::new(Some(tx));
    let all=rt.plan_text_replacement_document_raw(&info.id,"Alpha Beta","Both",Default::default(),None,&control).unwrap();assert_eq!((all.match_count,all.blocked_count),(2,1));assert_eq!(all.pages[1].matches[0].blocker,Some(TextReplacementBlocker::CrossObject));assert!(all.edits.is_empty()&&all.pages.iter().all(|p|p.edits.is_empty()));
    assert_eq!(rx.try_iter().map(|p|(p.completed,p.total)).collect::<Vec<_>>(),vec![(0,2),(1,2),(2,2)]);
    let subset=plan(&mut rt,&info.id,"Alpha Beta","Both",Some(&[TextReplacementSelection{page_index:0,match_index:0}])).unwrap();assert!(subset.can_apply);assert_eq!(subset.edits.len(),1);
}
#[test]
fn document_replacement_planning_cancel_and_progress_are_between_complete_pages(){
    let dir=tempfile::tempdir().unwrap();let path=dir.path().join("Many.pdf");std::fs::write(&path,fixture(&vec![vec!["Alpha"];100],false)).unwrap();let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();let info=engine.open_document(&path).unwrap();
    let control=ReplacementPlanningControl::default();control.cancel();assert!(engine.plan_text_replacement_document(&info.id,"Alpha","New",Default::default(),None,control).unwrap_err().to_string().to_lowercase().contains("cancel"));
    let(tx,rx)=mpsc::channel();let control=ReplacementPlanningControl::new(Some(tx));let owned=control.clone();let e=engine.clone();let id=info.id.clone();let task=std::thread::spawn(move||e.plan_text_replacement_document(&id,"Alpha","New",Default::default(),None,owned));
    let start=rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();assert_eq!((start.completed,start.total),(0,100));let first=rx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();assert_eq!(first.completed,1);control.cancel();assert!(task.join().unwrap().unwrap_err().to_string().to_lowercase().contains("cancel"));assert_eq!(engine.source_bytes(&info.id).unwrap().as_ref(),std::fs::read(&path).unwrap().as_slice());engine.close_document(&info.id).unwrap();
}
#[test]
fn document_replacement_planning_permissions_apply_even_to_noops(){
    let dir=tempfile::tempdir().unwrap();let original=fixture(&[vec!["Alpha"]],false);let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
    for rights in [lopdf::encryption::Permissions::all(),lopdf::encryption::Permissions::all()&!lopdf::encryption::Permissions::COPYABLE,lopdf::encryption::Permissions::all()&!lopdf::encryption::Permissions::MODIFIABLE]{
        let path=dir.path().join(format!("{}.pdf",rights.bits()));let bytes=protected_fixture::encrypt(&original,rights);std::fs::write(&path,&bytes).unwrap();let info=engine.open_document_with_password(&path,"reader").unwrap();let p=engine.plan_text_replacement_document(&info.id,"Alpha","Alpha",Default::default(),Some(&[]),Default::default());assert_eq!(p.is_ok(),rights==lopdf::encryption::Permissions::all());assert_eq!(std::fs::read(&path).unwrap(),bytes);engine.close_document(&info.id).unwrap();
    }
}
