use super::*;
use lopdf::{dictionary,Document,Stream,Object};
fn source(rt:&mut WorkerRuntime,guarded:bool)->DocumentInfo{
 let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
 let leaf=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),200.into(),50.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}},b"BT /F1 12 Tf 5 10 Td (Old) Tj ET".to_vec()));
 let content=pdf.add_object(Stream::new(dictionary!{},b"q 1 0 0 1 20 200 cm /F Do Q".to_vec()));
 let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),400.into(),400.into()],"Resources"=>dictionary!{"XObject"=>dictionary!{"F"=>leaf}},"Contents"=>content});
 if guarded{pdf.get_dictionary_mut(page).unwrap().set("PieceInfo",dictionary!{});}
 pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]}.into());let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",root);
 rt.register_generated_document(crate::pdf_output::save(&mut pdf).unwrap(),"Form paragraph.pdf").unwrap()
}
#[test]
fn form_paragraph_cleans_projections_and_candidates_on_every_reply_path(){
 let root=PathBuf::from(env!("CARGO_MANIFEST_DIR"));let mut rt=WorkerRuntime::new(&root.join("resources/pdfium/pdfium.dll"),Arc::new(FontRegistry::new())).unwrap();
 let original=source(&mut rt,false);let guarded=source(&mut rt,true);let baseline=rt.documents.len();
 for case in ["stale","scope","font","publication","abandoned","success"]{
  let (reply,receive)=mpsc::channel();let receive=if case=="abandoned"{drop(receive);None}else{Some(receive)};
  let mut request=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:vec![if case=="scope"{99}else{0}],expected_text:if case=="stale"{"stale"}else{"Old"}.into(),runs:vec![StyledParagraphRun{text:"New".into(),style_id:0}],style_changes:vec![],bounds:AnnotationRect{x:25.,y:160.,width:200.,height:50.},alignment:ParagraphAlignment::Left,line_spacing:1.2};
  if case=="font"{request.style_changes.push(StyledParagraphStyleChange{style_id:0,source_style_id:0,font_size:None,color:None,font_id:Some("missing-font".into())});}
  rt.handle_form_paragraph(Request::Preview{source:if case=="publication"{guarded.id.clone()}else{original.id.clone()},occurrences:vec![0],selected:0,path:vec![0,0],request,reply});
  if let Some(receive)=receive{let result=receive.recv().unwrap();if case=="success"{let result=result.unwrap();assert_eq!(rt.documents.len(),baseline+1);rt.discard_unpublished_source(&result.document.id);}else{let error=result.unwrap_err();if case=="publication"{assert!(error.to_string().contains("document-linked"),"{error}");}}}
  assert_eq!(rt.documents.len(),baseline,"{case}");
 }
}
