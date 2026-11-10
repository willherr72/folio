use folio_engine::{PdfEngine,StyledParagraphLayoutRequest};
use lopdf::{dictionary,Document,Stream};
use serde_json::json;
use std::path::{Path,PathBuf};
#[path="support/affine_forms.rs"]
#[allow(dead_code)]
mod forms;
fn engine()->PdfEngine{PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture(path:&Path){
 fixture_content(path,b"BT /F1 12 Tf 50 700 Td (one ) Tj (one ) Tj ET BT /F1 12 Tf 50 60 Td (Neighbor) Tj ET");
}
fn fixture_content(path:&Path,bytes:&[u8]){
 let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
 let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier","Encoding"=>"WinAnsiEncoding"});
 let content=pdf.add_object(Stream::new(dictionary!{},bytes.to_vec()));
 let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),600.into(),800.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});
 pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",root);pdf.save(path).unwrap();
}
#[test]
fn mapping_reports_utf16_endpoints_for_multibyte_source_text(){
 let e=engine();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("accent.pdf");
 fixture_content(&path,b"BT /F1 12 Tf 50 700 Td <636166e920> Tj <636166e920> Tj ET");
 let source=e.open_document(&path).unwrap();let selection=e.inspect_occurrence_styled_paragraph(&source.id,0,&[0,1]).unwrap();assert_eq!(selection.text,"café café");
 let map=e.map_paragraph_draft(&source.id,0,&[0,1],&selection.text,1,"café ",true).unwrap();
 assert_eq!((map.source_start,map.source_end,map.paragraph_start,map.paragraph_end),(0,4,5,9));e.close_document(&source.id).unwrap();
}
#[test]
fn saved_uniform_mapping_retains_blank_and_trailing_hard_lines(){
 let e=engine();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("source.pdf");fixture(&path);let source=e.open_document(&path).unwrap();let text="one two three\n\none two three\n";
 let edited=e.preview_text_paragraph_occurrence(&source.id,&[0],0,&[0,1],"one one",text,&folio_engine::AnnotationRect{x:50.,y:80.,width:160.,height:180.},&Default::default()).unwrap();
 let runs=e.list_occurrence_text_runs(&edited.document.id,0).unwrap().runs;let anchor=runs.iter().find(|r|r.text.contains("three")).unwrap();
 let selected=e.suggest_occurrence_text_paragraph(&edited.document.id,0,anchor.object_index,&anchor.text).unwrap();assert_eq!(selected.text,text);
 let raw=anchor.source_text.as_deref().unwrap_or(&anchor.text);let map=e.map_paragraph_draft(&edited.document.id,0,&selected.object_indices,text,anchor.object_index,raw,false).unwrap();
 let chars=text.encode_utf16().collect::<Vec<_>>();let original=raw.encode_utf16().collect::<Vec<_>>();assert_eq!(&chars[map.paragraph_start..map.paragraph_end],&original[map.source_start..map.source_end]);
 for id in [&source.id,&edited.document.id]{e.close_document(id).unwrap();}
}
#[test]
fn reusable_form_mapping_keeps_original_source_identity_and_scope(){
 let e=engine();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("form.pdf");forms::fixture(&path,"standard","plain",0,"Cafe old");let source=e.open_document(&path).unwrap();let before=e.source_bytes(&source.id).unwrap();
 let forms=e.list_occurrence_form_text_runs(&source.id,0).unwrap().runs;let anchor=&forms[0];let listing=e.list_form_paragraph_runs(&source.id,0,&anchor.object_path,&anchor.text).unwrap();
 let selected=e.suggest_form_styled_paragraph(&source.id,0,&anchor.object_path,listing.anchor_index,&anchor.text).unwrap();
 let mapped=e.map_form_paragraph_draft(&source.id,0,&anchor.object_path,&selected.object_indices,&selected.text,listing.anchor_index,&anchor.text).unwrap();
 assert_eq!(mapped.source_id,source.id);assert_eq!(mapped.page_index,0);assert_eq!(mapped.source_text,anchor.text);assert_eq!(mapped.paragraph_text,selected.text);
 assert!(e.map_form_paragraph_draft(&source.id,0,&anchor.object_path,&[usize::MAX],&selected.text,listing.anchor_index,&anchor.text).is_err());
 assert!(e.map_form_paragraph_draft(&source.id,0,&[0],&selected.object_indices,&selected.text,listing.anchor_index,&anchor.text).is_err());
 assert_eq!(e.source_bytes(&source.id).unwrap(),before);assert_eq!(std::fs::read(path).unwrap(),before.as_ref());e.close_document(&source.id).unwrap();
}
#[test]
fn fresh_mapping_binds_the_selected_repeated_piece_and_trimmed_source_padding(){
 let e=engine();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("source.pdf");fixture(&path);let source=e.open_document(&path).unwrap();let before=e.source_bytes(&source.id).unwrap();
 let selected=e.inspect_occurrence_styled_paragraph(&source.id,0,&[0,1]).unwrap();assert_eq!(selected.text,"one one");
 let mapped=e.map_paragraph_draft(&source.id,0,&[0,1],&selected.text,1,"one ",true).unwrap();
 assert_eq!((mapped.source_start,mapped.source_end,mapped.paragraph_start,mapped.paragraph_end),(0,3,4,7));
 assert_eq!(mapped.source_id,source.id);assert_eq!(mapped.object_indices,vec![0,1]);assert_eq!(mapped.anchor_index,1);
 for (text,index,raw) in [("stale",1,"one "),("one one",2,"Neighbor"),("one one",1,"one")]{assert!(e.map_paragraph_draft(&source.id,0,&[0,1],text,index,raw,true).is_err());}
 assert_eq!(e.source_bytes(&source.id).unwrap(),before);assert_eq!(std::fs::read(path).unwrap(),before.as_ref());
 e.close_document(&source.id).unwrap();
}
#[test]
fn saved_styled_mapping_keeps_hard_blank_lines_wrap_and_justified_fragments(){
 let e=engine();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("source.pdf");fixture(&path);let source=e.open_document(&path).unwrap();
 let text="one two three four five\n\none two three four five\n";
 let request:StyledParagraphLayoutRequest=serde_json::from_value(json!({"objectIndices":[0,1],"expectedText":"one one","runs":[{"text":text,"styleId":0}],"styleChanges":[],"bounds":{"x":50,"y":80,"width":130,"height":180},"alignment":"justify","lineSpacing":1.2})).unwrap();
 let edited=e.preview_styled_paragraph_occurrence(&source.id,&[0],0,&request).unwrap();let path=dir.path().join("saved.pdf");std::fs::write(&path,e.source_bytes(&edited.document.id).unwrap()).unwrap();let saved=e.open_document(&path).unwrap();
 let anchor=e.list_occurrence_text_runs(&saved.id,0).unwrap().runs.into_iter().find(|r|r.text.contains("three")).unwrap();
 let selection=e.suggest_occurrence_styled_paragraph(&saved.id,0,anchor.object_index,&anchor.text).unwrap();assert_eq!(selection.text,text);
 for index in &selection.object_indices{
  let run=e.list_occurrence_text_runs(&saved.id,0).unwrap().runs.into_iter().find(|r|r.object_index==*index).unwrap();let raw=run.source_text.as_deref().unwrap_or(&run.text);
  let map=e.map_paragraph_draft(&saved.id,0,&selection.object_indices,text,*index,raw,true).unwrap();
  let source=raw.encode_utf16().collect::<Vec<_>>();let logical=text.encode_utf16().collect::<Vec<_>>();assert_eq!(&source[map.source_start..map.source_end],&logical[map.paragraph_start..map.paragraph_end]);
 }
 for id in [&source.id,&edited.document.id,&saved.id]{e.close_document(id).unwrap();}
}
