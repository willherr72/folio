use folio_engine::PdfEngine;
use lopdf::{dictionary,Document,Stream};
use std::path::{Path,PathBuf};
fn engine()->PdfEngine{PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture(path:&Path,rotation:i32,content:&str){
 let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();let plain=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});let bold=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica-Bold"});
 let stream=pdf.add_object(Stream::new(dictionary!{},content.as_bytes().to_vec()));let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),600.into(),800.into()],"CropBox"=>vec![10.into(),20.into(),590.into(),780.into()],"Rotate"=>rotation,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>plain,"F2"=>bold}},"Contents"=>stream});pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",root);pdf.save(path).unwrap();
}
#[test]
fn every_piece_suggests_the_complete_mixed_highlighted_line_at_each_rotation(){
 let e=engine();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("line.pdf");
 for rotation in [0,90,180,270]{
  fixture(&path,rotation,"q 1 1 0 rg 70 696 21 16 re f Q BT /F1 12 Tf 50 700 Td (AA ) Tj ET BT /F2 12 Tf 72 700 Td (BB) Tj ET BT /F1 12 Tf 90 700 Td ( CC) Tj ET BT /F1 12 Tf 50 100 Td (Neighbor) Tj ET");let source=e.open_document(&path).unwrap();let bytes=e.source_bytes(&source.id).unwrap();let checked=e.inspect_styled_paragraph(&source.id,0,&[1,2,3]).unwrap();assert!(checked.styles.iter().any(|s|s.background_color.is_some()));
  for anchor in e.list_text_runs(&source.id,0).unwrap().runs.into_iter().filter(|r|r.object_index<4){assert_eq!(e.suggest_styled_paragraph(&source.id,0,anchor.object_index,&anchor.text).unwrap(),checked);}
  assert_eq!(e.source_bytes(&source.id).unwrap(),bytes);assert_eq!(std::fs::read(&path).unwrap(),bytes.as_ref());e.close_document(&source.id).unwrap();
 }
}
#[test]
fn mixed_wrapped_rows_share_body_size_without_absorbing_heading_or_next_list_item(){
 let e=engine();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("rows.pdf");
 fixture(&path,0,"BT /F2 20 Tf 50 740 Td (Heading) Tj ET BT /F1 12 Tf 50 716 Td (Body ) Tj ET BT /F2 18 Tf 86 716 Td (large) Tj ET BT /F1 12 Tf 50 692 Td (continued line) Tj ET BT /F1 12 Tf 50 678 Td (2. Next item) Tj ET");
 let source=e.open_document(&path).unwrap();let checked=e.inspect_styled_paragraph(&source.id,0,&[1,2,3]).unwrap();
 for anchor in e.list_text_runs(&source.id,0).unwrap().runs{let suggested=e.suggest_styled_paragraph(&source.id,0,anchor.object_index,&anchor.text).unwrap();if (1..=3).contains(&anchor.object_index){assert_eq!(suggested,checked);}else{assert_eq!(suggested.object_indices,vec![anchor.object_index]);}}
 e.close_document(&source.id).unwrap();
}
#[test]
fn separated_columns_and_shifted_baselines_keep_manual_selection(){
 let e=engine();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("boundaries.pdf");
 for content in ["BT /F1 12 Tf 50 700 Td (Left) Tj ET BT /F2 12 Tf 300 700 Td (Right) Tj ET","BT /F1 12 Tf 50 700 Td (x) Tj ET BT /F2 8 Tf 58 704 Td (2) Tj ET BT /F1 12 Tf 64 700 Td ( value) Tj ET"]{
  fixture(&path,0,content);let source=e.open_document(&path).unwrap();for anchor in e.list_text_runs(&source.id,0).unwrap().runs{assert_eq!(e.suggest_styled_paragraph(&source.id,0,anchor.object_index,&anchor.text).unwrap().object_indices,vec![anchor.object_index]);}e.close_document(&source.id).unwrap();
 }
}

#[test]
fn cumulative_baseline_drift_and_oversized_groups_fall_back_to_the_clicked_piece(){
 let e=engine();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("bounded.pdf");
 for content in ["BT /F1 12 Tf 50 700 Td (A) Tj ET BT /F2 12 Tf 58 699.985 Td (B) Tj ET BT /F1 12 Tf 67 699.97 Td (C) Tj ET".to_string(),(0..65).map(|i|format!("BT /F1 4 Tf {} 700 Td (A) Tj ET ",50+i*3)).collect::<String>()]{
  fixture(&path,0,&content);let source=e.open_document(&path).unwrap();let runs=e.list_text_runs(&source.id,0).unwrap().runs;
  for anchor in [&runs[0],&runs[runs.len()-1]]{assert_eq!(e.suggest_styled_paragraph(&source.id,0,anchor.object_index,&anchor.text).unwrap().object_indices,vec![anchor.object_index]);}e.close_document(&source.id).unwrap();
 }
}

#[test]
fn body_size_evidence_cannot_drift_through_pairwise_matching_rows(){
 let e=engine();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("sizes.pdf");
 fixture(&path,0,"BT /F1 12 Tf 50 740 Td (First) Tj ET BT /F1 12 Tf 50 716 Td (Body ) Tj ET BT /F2 18 Tf 86 716 Td (large) Tj ET BT /F1 18 Tf 50 688 Td (Other ) Tj ET BT /F2 24 Tf 115 688 Td (bigger) Tj ET BT /F1 24 Tf 50 660 Td (Last) Tj ET");
 let source=e.open_document(&path).unwrap();let checked=e.inspect_styled_paragraph(&source.id,0,&[0,1,2]).unwrap();let first=e.list_text_runs(&source.id,0).unwrap().runs.remove(0);assert_eq!(e.suggest_styled_paragraph(&source.id,0,first.object_index,&first.text).unwrap(),checked);e.close_document(&source.id).unwrap();
}

#[test]
fn earlier_large_emphasis_does_not_bridge_a_later_gap_between_small_pieces(){
 let e=engine();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("local-gap.pdf");
 fixture(&path,0,"BT /F1 36 Tf 50 700 Td (A) Tj ET BT /F1 12 Tf 75 700 Td (B) Tj ET BT /F1 12 Tf 108 700 Td (C) Tj ET");
 let source=e.open_document(&path).unwrap();for anchor in e.list_text_runs(&source.id,0).unwrap().runs{let expected=if anchor.object_index<2{vec![0,1]}else{vec![2]};assert_eq!(e.suggest_styled_paragraph(&source.id,0,anchor.object_index,&anchor.text).unwrap().object_indices,expected);}e.close_document(&source.id).unwrap();
}

