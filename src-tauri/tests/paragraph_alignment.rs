use folio_engine::{AnnotationRect,ParagraphFormatting,PdfEngine,ParagraphAlignment as Align};
use lopdf::{dictionary,Document,Stream};
use std::path::{Path,PathBuf};
fn engine()->PdfEngine {PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture(path:&Path,program:&str,rotation:i32){
 let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
 let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
 let content=pdf.add_object(Stream::new(dictionary!{},program.as_bytes().to_vec()));
 let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),400.into(),500.into()],"Rotate"=>rotation,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});
 pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",root);pdf.save(path).unwrap();
}
fn area(rotation:i32)->AnnotationRect {match rotation {
 90=>AnnotationRect{x:340.0,y:60.0,width:80.0,height:180.0},
 180=>AnnotationRect{x:160.0,y:340.0,width:180.0,height:80.0},
 270=>AnnotationRect{x:80.0,y:160.0,width:80.0,height:180.0},
 _=>AnnotationRect{x:60.0,y:80.0,width:180.0,height:80.0},
}}
#[test]
fn infers_edges_and_centers_from_rows_but_not_ambiguous_or_single_lines(){
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for rotation in [0,90,180,270] {for (first,second,expected) in [(60.0,60.0,Some(Align::Left)),(162.0,144.0,Some(Align::Center)),(164.0,128.0,Some(Align::Right)),(60.0,70.0,None)] {
  let path=temp.path().join("rows.pdf");fixture(&path,&format!("BT /F1 12 Tf 1 0 0 1 {first} 400 Tm (XX) Tj (XXX) Tj 1 0 0 1 {second} 382 Tm (XXXXXXXXXX) Tj ET"),rotation);
  let source=e.open_document(&path).unwrap();assert_eq!(e.inspect_text_paragraph(&source.id,0,&[0,1,2]).unwrap().alignment,expected);assert_eq!(e.inspect_text_paragraph(&source.id,0,&[0,1]).unwrap().alignment,None);e.close_document(&source.id).unwrap();
 }}
 let path=temp.path().join("equal.pdf");fixture(&path,"BT /F1 12 Tf 60 400 Td (XXXXX) Tj 0 -18 Td (XXXXX) Tj ET",0);let source=e.open_document(&path).unwrap();assert_eq!(e.inspect_text_paragraph(&source.id,0,&[0,1]).unwrap().alignment,None);e.close_document(&source.id).unwrap();
}
#[test]
fn chosen_alignment_composes_with_font_size_rotation_and_preserves_neighbor(){
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for rotation in [0,90,180,270] {for alignment in [Align::Left,Align::Center,Align::Right] {
  let path=temp.path().join("source.pdf");fixture(&path,"BT /F1 12 Tf 60 400 Td (Original) Tj 0 -18 Td (words) Tj 0 -182 Td (Neighbor) Tj ET",rotation);let original=std::fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();let before=e.list_text_runs(&source.id,0).unwrap().runs;
  let selection=e.inspect_text_paragraph(&source.id,0,&[0,1]).unwrap();let edited=e.preview_text_paragraph_with_format(&source.id,0,&[0,1],&selection.text,"XXXXXXXXXX\nXX   ",&area(rotation),&ParagraphFormatting{font_size:Some(16.0),alignment:Some(alignment),..Default::default()}).unwrap_or_else(|error|panic!("rotation={rotation} alignment={alignment:?}: {error}"));
  let output=temp.path().join("saved.pdf");e.export_pdf(folio_engine::ExportRequest{flatten:false,pages:vec![folio_engine::PagePlan{crop: None,id:"p".into(),source_id:edited.id.clone(),page_index:0,width:edited.pages[0].width,height:edited.pages[0].height,rotation:0,overlays:vec![]}]},&output).unwrap();
  let reopened=e.open_document(&output).unwrap();let after=e.list_text_runs(&reopened.id,0).unwrap().runs;let neighbor=after.iter().find(|r|r.text=="Neighbor").unwrap();assert_eq!(neighbor.bounds,before[2].bounds);
  // Compare raw Tm positions: trailing spaces do not shift the shorter line.
  let pdf=Document::load(&output).unwrap();let page=*pdf.get_pages().get(&1).unwrap();let content=lopdf::content::Content::decode(&pdf.get_page_content(page)).unwrap();
  let positions:Vec<_>=content.operations.iter().filter(|op|op.operator=="Tm").map(|op|op.operands[4].as_float().unwrap()).collect();let x=&positions[positions.len()-2..];
  let delta=match alignment {Align::Left=>0.0,Align::Center=>38.4,Align::Right=>76.8,Align::Justify=>unreachable!("this test enumerates the three fixed alignments")};assert!((x[1]-x[0]-delta).abs()<0.03,"{rotation:?} {alignment:?}: {x:?}");
  for run in after.iter().filter(|r|r.text!="Neighbor") {assert!((run.font_size-16.0).abs()<0.01);}
  for run in after.iter().filter(|r|r.text!="Neighbor") {
   let suggested=e.suggest_text_paragraph(&reopened.id,0,run.object_index,&run.text).unwrap();assert_eq!(suggested.object_indices.len(),2,"{rotation} {alignment:?}");assert_eq!(suggested.alignment,Some(alignment));
   let extent=match rotation {0=>suggested.bounds.x+suggested.bounds.width,90=>suggested.bounds.y+suggested.bounds.height,180=>400.0-suggested.bounds.x,_=>400.0-suggested.bounds.y};
   assert!(extent<=241.7,"Trailing cursor positions enlarged the area: {extent}");
  }
  assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),original.as_slice());assert_eq!(std::fs::read(&path).unwrap(),original);
  for id in [&source.id,&edited.id,&reopened.id]{e.close_document(id).unwrap();}
 }}
}
#[test]
fn omitted_alignment_uses_inferred_center(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("center.pdf");fixture(&path,"BT /F1 12 Tf 162 400 Td (XXXXX) Tj -18 -18 Td (XXXXXXXXXX) Tj ET",0);
 let source=e.open_document(&path).unwrap();let selection=e.inspect_text_paragraph(&source.id,0,&[0,1]).unwrap();assert_eq!(selection.alignment,Some(Align::Center));
 let edited=e.preview_text_paragraph(&source.id,0,&[0,1],&selection.text,"XXXXXXXXXX\nXX",&area(0)).unwrap();let runs=e.list_text_runs(&edited.id,0).unwrap().runs;let centers:Vec<_>=runs.iter().map(|r|r.bounds.x+r.bounds.width/2.0).collect();assert!((centers[0]-centers[1]).abs()<0.1,"{centers:?}");for id in [&source.id,&edited.id]{e.close_document(id).unwrap();}
}

#[test]
fn different_terminal_bearings_keep_alignment_after_save_and_inspection(){
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for rotation in [0,90,180,270] {for alignment in [Align::Center,Align::Right] {
  let path=temp.path().join("bearings.pdf");fixture(&path,"BT /F1 12 Tf 60 400 Td (Original) Tj ET",rotation);let source=e.open_document(&path).unwrap();let selection=e.inspect_text_paragraph(&source.id,0,&[0]).unwrap();
  let edited=e.preview_text_paragraph_with_format(&source.id,0,&[0],&selection.text,"XXXXXXXXXX\njj",&area(rotation),&ParagraphFormatting{font_size:Some(24.0),alignment:Some(alignment),..Default::default()}).unwrap();
  assert_eq!(e.inspect_text_paragraph(&edited.id,0,&[0,1]).unwrap().alignment,Some(alignment));
  let path=temp.path().join("saved.pdf");e.export_pdf(folio_engine::ExportRequest{flatten:false,pages:vec![folio_engine::PagePlan{crop: None,id:"p".into(),source_id:edited.id.clone(),page_index:0,width:edited.pages[0].width,height:edited.pages[0].height,rotation:0,overlays:vec![]}]},&path).unwrap();let reopened=e.open_document(&path).unwrap();assert_eq!(e.inspect_text_paragraph(&reopened.id,0,&[0,1]).unwrap().alignment,Some(alignment));
  for id in [&source.id,&edited.id,&reopened.id]{e.close_document(id).unwrap();}
 }}
}
#[test]
fn centered_line_near_area_width_retains_spaces_without_overflow(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("spaces.pdf");fixture(&path,"BT /F1 12 Tf 60 400 Td (Original) Tj ET",0);let source=e.open_document(&path).unwrap();let selection=e.inspect_text_paragraph(&source.id,0,&[0]).unwrap();
 let edited=e.preview_text_paragraph_with_format(&source.id,0,&[0],&selection.text,"XXXXXXXXXX   \nXX",&AnnotationRect{x:60.0,y:80.0,width:100.0,height:80.0},&ParagraphFormatting{font_size:Some(16.0),alignment:Some(Align::Center),..Default::default()}).unwrap();
 let bytes=e.source_bytes(&edited.id).unwrap();let pdf=Document::load_mem(&bytes).unwrap();let page=*pdf.get_pages().get(&1).unwrap();let content=lopdf::content::Content::decode(&pdf.get_page_content(page)).unwrap();assert!(content.operations.iter().any(|op|op.operator=="Tj"&&op.operands[0].as_str().ok()==Some(b"XXXXXXXXXX   ")));
 assert_eq!(e.inspect_text_paragraph(&edited.id,0,&[0,1]).unwrap().alignment,Some(Align::Center));for id in [&source.id,&edited.id]{e.close_document(id).unwrap();}
}
