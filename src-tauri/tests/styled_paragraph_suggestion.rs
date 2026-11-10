use folio_engine::{PdfEngine,StyledParagraphLayoutRequest};
use lopdf::{dictionary,Document,Object,Stream,StringFormat};
use serde_json::{json,Value};
use std::path::{Path,PathBuf};
fn engine()->PdfEngine{PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture(path:&Path){
 let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();let plain=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});let bold=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica-Bold"});
 let content=pdf.add_object(Stream::new(dictionary!{},b"BT /F1 12 Tf 50 700 Td (One ) Tj ET BT /F2 20 Tf 95 700 Td (Two) Tj ET BT /F1 12 Tf 50 60 Td (Neighbor) Tj ET".to_vec()));let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),600.into(),800.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>plain,"F2"=>bold}},"Contents"=>content});pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",root);pdf.save(path).unwrap();
}
fn request()->StyledParagraphLayoutRequest{serde_json::from_value(json!({"objectIndices":[0,1],"expectedText":"One Two","runs":(0..200).map(|i|json!({"text":if i%2==0{"a"}else{" "},"styleId":i%2})).collect::<Vec<_>>(),"styleChanges":[{"styleId":0,"sourceStyleId":0,"fontSize":8},{"styleId":1,"sourceStyleId":1,"fontSize":8}],"bounds":{"x":50,"y":80,"width":400,"height":250},"alignment":"right","lineSpacing":1.4})).unwrap()}

#[test]
fn an_anchor_recovers_the_complete_saved_draft_including_whitespace_fragments(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("source.pdf");fixture(&path);let source=e.open_document(&path).unwrap();let request=request();let preview=e.preview_styled_paragraph(&source.id,0,&request).unwrap();let bytes=e.source_bytes(&preview.id).unwrap();let saved_path=temp.path().join("saved.pdf");std::fs::write(&saved_path,&bytes).unwrap();e.close_document(&preview.id).unwrap();e.close_document(&source.id).unwrap();let saved=e.open_document(&saved_path).unwrap();
 let runs=e.list_text_runs(&saved.id,0).unwrap().runs;let paragraph=runs.iter().filter(|r|r.text!="Neighbor").collect::<Vec<_>>();assert_eq!(paragraph.len(),200);let ids=paragraph.iter().map(|r|r.object_index).collect::<Vec<_>>();let checked=e.inspect_styled_paragraph(&saved.id,0,&ids).unwrap();
 assert!(paragraph.iter().any(|r|r.text.trim().is_empty()));
 for anchor in [paragraph[0],paragraph[99],paragraph[199]]{
  let suggested=e.suggest_styled_paragraph(&saved.id,0,anchor.object_index,&anchor.text).unwrap();assert_eq!(suggested,checked);assert_eq!(suggested.runs,request.runs);assert_eq!(suggested.bounds,request.bounds);
 }
 assert_eq!(e.source_bytes(&saved.id).unwrap(),bytes);assert_eq!(std::fs::read(&saved_path).unwrap(),bytes.as_ref());e.close_document(&saved.id).unwrap();
}

#[test]
fn ordinary_and_stale_anchors_never_authorize_an_unchecked_paragraph(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("source.pdf");fixture(&path);let source=e.open_document(&path).unwrap();let original=e.source_bytes(&source.id).unwrap();let runs=e.list_text_runs(&source.id,0).unwrap().runs;
 // This fixture's pieces have a gap wider than either font; keep that boundary.
 for anchor in &runs{let suggested=e.suggest_styled_paragraph(&source.id,0,anchor.object_index,&anchor.text).unwrap();assert_eq!(suggested.object_indices,vec![anchor.object_index]);assert_eq!(suggested.styles.len(),1);}
 assert!(e.suggest_styled_paragraph(&source.id,0,0,"stale").is_err());assert!(e.suggest_styled_paragraph(&source.id,0,9999,"One ").is_err());assert!(e.suggest_styled_paragraph(&source.id,1,0,"One ").is_err());
 let mixed=e.inspect_styled_paragraph(&source.id,0,&[0,1]).unwrap();assert_eq!(mixed.styles.len(),2);assert_eq!(e.source_bytes(&source.id).unwrap(),original);e.close_document(&source.id).unwrap();
}

#[test]
fn forged_ambiguous_and_malformed_saved_hints_fall_back_without_restoring_settings(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("source.pdf");fixture(&path);let source=e.open_document(&path).unwrap();let preview=e.preview_styled_paragraph(&source.id,0,&request()).unwrap();let original=Document::load_mem(&e.source_bytes(&preview.id).unwrap()).unwrap();let page=*original.get_pages().get(&1).unwrap();let hints:Value=serde_json::from_slice(original.get_dictionary(page).unwrap().get(b"FolioStyledParagraphs").unwrap().as_str().unwrap()).unwrap();
 for case in 0..6{
  let mut pdf=original.clone();let mut hints=hints.clone();match case{
   0=>hints["version"]=json!(999),1=>hints["paragraphs"][0]["runs"][0]["text"]=json!("b"),
   2|3|4=>{let mut other=hints["paragraphs"][0].clone();if case==3{other["fragments"].as_array_mut().unwrap().truncate(2);}if case==4{hints["paragraphs"].as_array_mut().unwrap().insert(0,other);}else{hints["paragraphs"].as_array_mut().unwrap().push(other);}},
   _=>hints=json!({"unexpected":"data"}),
  }
  pdf.get_dictionary_mut(page).unwrap().set("FolioStyledParagraphs",Object::String(serde_json::to_vec(&hints).unwrap(),StringFormat::Hexadecimal));let path=temp.path().join(format!("case{case}.pdf"));pdf.save(&path).unwrap();let changed=e.open_document(&path).unwrap();let bytes=e.source_bytes(&changed.id).unwrap();let anchor=e.list_text_runs(&changed.id,0).unwrap().runs.into_iter().find(|r|r.text=="a").unwrap();
  let suggestion=e.suggest_styled_paragraph(&changed.id,0,anchor.object_index,&anchor.text).unwrap();assert_eq!(suggestion.object_indices,vec![anchor.object_index],"case {case}");assert_eq!(suggestion.alignment,None);assert_eq!(suggestion.line_spacing,None);assert!(e.suggest_styled_paragraph(&changed.id,0,anchor.object_index,"b").is_err());assert_eq!(e.source_bytes(&changed.id).unwrap(),bytes);e.close_document(&changed.id).unwrap();
 }
 for id in [&source.id,&preview.id]{e.close_document(id).unwrap();}
}
