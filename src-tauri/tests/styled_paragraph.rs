use folio_engine::PdfEngine;
use lopdf::{dictionary,Document,Stream};
use std::path::{Path,PathBuf};
fn engine()->PdfEngine{PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture(path:&Path,rotation:i32,content:&str){
 let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
 let plain=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
 let bold=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica-Bold"});
 let stream=pdf.add_object(Stream::new(dictionary!{},content.as_bytes().to_vec()));
 let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),500.into(),600.into()],"CropBox"=>vec![10.into(),20.into(),490.into(),580.into()],"Rotate"=>rotation,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>plain,"Alias"=>plain,"F2"=>bold}},"Contents"=>stream});
 pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",root);pdf.save(path).unwrap();
}
#[test]
fn inspects_verified_mixed_styles_in_each_page_rotation_without_mutation(){
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for rotation in [0,90,180,270]{
  let path=temp.path().join("mixed.pdf");fixture(&path,rotation,"BT /F1 12 Tf 50 500 Td (Plain ) Tj ET BT /F2 16 Tf 100 500 Td (Bold ) Tj ET BT /F1 12 Tf 0 0.5 0 rg 155 500 Td (green) Tj ET");
  let original=std::fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();let image=e.render_page(&source.id,0,700).unwrap();
  assert!(e.inspect_text_paragraph(&source.id,0,&[0,1,2]).unwrap_err().to_string().contains("same font"));
  let selected=e.inspect_styled_paragraph(&source.id,0,&[0,1,2]).unwrap();assert_eq!(selected.text,"Plain Bold green");assert_eq!(selected.runs.iter().map(|r|r.text.as_str()).collect::<String>(),selected.text);assert_eq!(selected.styles.len(),3);
  assert_eq!(selected.styles[0].font_name,"Courier");assert_eq!(selected.styles[1].font_name,"Helvetica-Bold");assert_eq!(selected.styles[1].font_size,16.);assert_eq!(selected.styles[2].color,"#008000");
  assert_eq!(selected.runs.iter().map(|r|r.style_id).collect::<Vec<_>>(),vec![0,1,2]);assert_eq!(selected.styles[1].object_indices,vec![1]);
  let page=&source.pages[0];assert!(selected.bounds.x>=0.&&selected.bounds.y>=0.&&selected.bounds.x+selected.bounds.width<=page.width&&selected.bounds.y+selected.bounds.height<=page.height);
  assert_eq!(e.render_page(&source.id,0,700).unwrap(),image);assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),original);assert_eq!(std::fs::read(&path).unwrap(),original);e.close_document(&source.id).unwrap();
 }
}
#[test]
fn preserves_separator_styles_and_deduplicates_only_verified_resource_styles(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("joined.pdf");
 fixture(&path,0,"BT /F1 12 Tf 50 500 Td (  Alpha) Tj ET BT /F2 12 Tf 120 500 Td (Beta  ) Tj ET BT /Alias 12 Tf 50 482 Td (Gamma  ) Tj ET");
 let source=e.open_document(&path).unwrap();let s=e.inspect_styled_paragraph(&source.id,0,&[0,1,2]).unwrap();assert_eq!(s.text,"Alpha Beta Gamma");assert_eq!(s.styles.len(),2);assert_eq!(s.styles[0].object_indices,vec![0,2]);
 assert_eq!(s.runs.iter().map(|r|(r.text.as_str(),r.style_id)).collect::<Vec<_>>(),vec![("Alpha ",0),("Beta ",1),("Gamma",0)]);e.close_document(&source.id).unwrap();
}
#[test]
fn rejects_missing_intervening_reversed_and_nonconsecutive_pieces(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("refuse.pdf");fixture(&path,0,"BT /F1 12 Tf 50 500 Td (One) Tj ET BT /F2 12 Tf 100 500 Td (Two) Tj ET BT /F1 12 Tf 50 300 Td (Far) Tj ET");let source=e.open_document(&path).unwrap();let original=e.source_bytes(&source.id).unwrap();
 for indices in [&[][..],&[0,2],&[1,0],&[99],&[0,1,2]]{assert!(e.inspect_styled_paragraph(&source.id,0,indices).is_err(),"{indices:?}");}
 assert_eq!(e.source_bytes(&source.id).unwrap(),original);e.close_document(&source.id).unwrap();
}
#[test]
fn embedded_and_standard_fonts_keep_distinct_verified_bindings(){
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for rotation in [0,90,180,270]{
  let mut pdf=Document::load(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/embedded-text/reportlab-subset.pdf")).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
  let stream=pdf.add_object(Stream::new(dictionary!{},b"q .2 .4 .6 rg BT /F2+0 16 Tf 1 0 0 1 30 300 Tm (Caf\x01) Tj ET Q BT /F1 12 Tf 1 0 0 1 90 300 Tm (mix) Tj ET q .2 .4 .6 rg BT /F2+0 16 Tf 1 0 0 1 30 276 Tm (old) Tj ET Q".to_vec()));
  let page_dict=pdf.get_dictionary_mut(page).unwrap();page_dict.set("Contents",stream);page_dict.set("Rotate",rotation);let path=temp.path().join("embedded.pdf");pdf.save(&path).unwrap();
  let original=std::fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();let s=e.inspect_styled_paragraph(&source.id,0,&[0,1,2]).unwrap();
  assert_eq!(s.text,"Café mix old");assert_eq!(s.styles.len(),2);assert!(s.styles[0].is_embedded);assert!(!s.styles[1].is_embedded);assert_eq!(s.styles[0].object_indices,vec![0,2]);assert_eq!(s.styles[0].color,"#336699");assert!(s.styles[0].font_display_name.as_deref().unwrap().contains("DejaVu"));
  assert_eq!(s.runs.iter().map(|r|(r.text.as_str(),r.style_id)).collect::<Vec<_>>(),vec![("Café ",0),("mix ",1),("old",0)]);
  assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),original);assert_eq!(std::fs::read(&path).unwrap(),original);e.close_document(&source.id).unwrap();
 }
}
#[test]
fn distinct_resource_objects_are_not_merged_by_display_name(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("resources.pdf");fixture(&path,0,"BT /F1 12 Tf 50 500 Td (One ) Tj ET BT /F3 12 Tf 90 500 Td (Two) Tj ET");
 let mut pdf=Document::load(&path).unwrap();let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});let page=*pdf.get_pages().get(&1).unwrap();pdf.get_dictionary_mut(page).unwrap().get_mut(b"Resources").unwrap().as_dict_mut().unwrap().get_mut(b"Font").unwrap().as_dict_mut().unwrap().set("F3",font);pdf.save(&path).unwrap();
 let source=e.open_document(&path).unwrap();let s=e.inspect_styled_paragraph(&source.id,0,&[0,1]).unwrap();assert_eq!(s.text,"One Two");assert_eq!(s.styles.len(),2);assert_eq!(s.styles[0].font_name,s.styles[1].font_name);assert_eq!(s.styles[0].font_size,s.styles[1].font_size);e.close_document(&source.id).unwrap();
}
#[test]
fn effective_size_is_derived_per_piece_and_equivalent_scaling_can_share_style(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("scale.pdf");fixture(&path,0,"BT /F1 12 Tf 50 500 Td (One ) Tj ET q 2 0 0 2 0 0 cm BT /F1 6 Tf 45 250 Td (Two) Tj ET Q");
 let source=e.open_document(&path).unwrap();let s=e.inspect_styled_paragraph(&source.id,0,&[0,1]).unwrap();assert_eq!(s.text,"One Two");assert_eq!(s.styles.len(),1);assert_eq!(s.styles[0].font_size,12.);assert_eq!(s.styles[0].object_indices,vec![0,1]);assert!(e.inspect_text_paragraph(&source.id,0,&[0,1]).is_err());e.close_document(&source.id).unwrap();
}
#[test]
fn mixed_inspection_keeps_existing_text_state_and_content_refusals(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("unsupported.pdf");
 for content in ["BT /F1 12 Tf 3 Tr 50 500 Td (Hidden) Tj ET","BT /F1 12 Tf 50 500 Td (Implicit) ' ET","/Span BMC BT /F1 12 Tf 50 500 Td (Marked) Tj ET EMC","BT /F1 12 Tf 1 .2 0 1 50 500 Tm (Skewed) Tj ET"]{
  fixture(&path,0,content);let source=e.open_document(&path).unwrap();let before=e.source_bytes(&source.id).unwrap();assert!(e.inspect_styled_paragraph(&source.id,0,&[0]).is_err(),"{content}");assert_eq!(e.source_bytes(&source.id).unwrap(),before);e.close_document(&source.id).unwrap();
 }
}
#[test]
fn small_text_word_gaps_do_not_depend_on_a_large_first_piece(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("gaps.pdf");fixture(&path,0,"BT /F1 30 Tf 50 500 Td (Heading) Tj ET BT /F1 10 Tf 50 460 Td (one) Tj ET BT /F2 10 Tf 72 460 Td (two) Tj ET");
 let source=e.open_document(&path).unwrap();let s=e.inspect_styled_paragraph(&source.id,0,&[0,1,2]).unwrap();assert_eq!(s.text,"Heading one two");assert_eq!(s.runs.iter().map(|r|(r.text.as_str(),r.style_id)).collect::<Vec<_>>(),vec![("Heading ",0),("one ",1),("two",2)]);e.close_document(&source.id).unwrap();
}
#[test]
fn line_admission_considers_larger_pieces_later_in_the_incoming_row(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("row-size.pdf");
 for (gap,accepted) in [(40,true),(100,false),(12,false)]{
  fixture(&path,0,&format!("BT /F1 10 Tf 50 500 Td (Top) Tj ET BT /F1 10 Tf 50 {} Td (small ) Tj ET BT /F1 30 Tf 100 {} Td (Big) Tj ET",500-gap,500-gap));
  let source=e.open_document(&path).unwrap();let before=e.source_bytes(&source.id).unwrap();let result=e.inspect_styled_paragraph(&source.id,0,&[0,1,2]);
  if accepted{assert_eq!(result.unwrap().text,"Top small Big");}else{assert!(result.unwrap_err().to_string().contains("consecutive"));}
  assert_eq!(e.source_bytes(&source.id).unwrap(),before);e.close_document(&source.id).unwrap();
 }
}
#[test]
fn inspection_enforces_piece_and_logical_byte_limits_without_truncation(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("limits.pdf");
 for (count,bytes,accepted) in [(64,4096,true),(64,4097,false),(65,129,false)]{
  // Inter-row separators contribute count-1 bytes to the complete logical text.
  let characters=bytes-(count-1);let mut content=String::new();
  for i in 0..count{
   let length=characters/count+usize::from(i<characters%count);
   let color=if i%2==0{"0 0 0"}else{"0 .5 0"};
   content.push_str(&format!("BT /F1 1 Tf {color} rg 50 {} Td ({}) Tj ET ",500-3*i,"A".repeat(length)));
  }
  fixture(&path,0,&content);let source=e.open_document(&path).unwrap();let before=e.source_bytes(&source.id).unwrap();
  let result=e.inspect_styled_paragraph(&source.id,0,&(0..count).collect::<Vec<_>>());
  if accepted{let selection=result.unwrap();assert_eq!(selection.text.len(),bytes);assert_eq!(selection.object_indices.len(),count);assert_eq!(selection.styles.len(),2);assert_eq!(selection.runs.iter().map(|r|r.text.as_str()).collect::<String>(),selection.text);}
  else{let error=result.unwrap_err().to_string();assert!(error.contains(if count>64{"1 to 64"}else{"4096 UTF-8 bytes"}),"{error}");}
  assert_eq!(e.source_bytes(&source.id).unwrap(),before);e.close_document(&source.id).unwrap();
 }
}
