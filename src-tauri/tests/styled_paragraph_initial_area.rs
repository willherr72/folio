use folio_engine::{PdfEngine,StyledParagraphLayoutRequest,ParagraphAlignment};
use lopdf::{dictionary,Document,Stream};
use std::path::{Path,PathBuf};
fn engine()->PdfEngine{PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture(path:&Path,rotation:i32){
 let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
 let plain=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
 let bold=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica-Bold"});
 let stream=pdf.add_object(Stream::new(dictionary!{},b"BT /F1 12 Tf 1 0 0 1 30 440 Tm (One ) Tj ET BT /F2 18 Tf 1 0 0 1 54 440 Tm (two) Tj ET BT /F1 12 Tf 1 0 0 1 280 180 Tm (Neighbor) Tj ET".to_vec()));
 let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),400.into(),500.into()],"CropBox"=>vec![10.into(),20.into(),390.into(),480.into()],"Rotate"=>rotation,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>plain,"F2"=>bold}},"Contents"=>stream});
 pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());let catalog=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",catalog);pdf.save(path).unwrap();
}
fn add_obstacles(path:&Path,right:bool,below:bool){
 let mut pdf=Document::load(path).unwrap();let id=*pdf.get_pages().get(&1).unwrap();let mut content=pdf.get_page_content(id);
 if right{content.extend_from_slice(b" q 0.2 0.4 0.6 rg 86.21 420 8 50 re f Q");}
 if below{content.extend_from_slice(b" q 0.2 0.4 0.6 rg 20 427.49 80 8 re f Q");}
 let stream=pdf.add_object(Stream::new(dictionary!{},content));pdf.get_dictionary_mut(id).unwrap().set("Contents",stream);pdf.save(path).unwrap();
}
#[test]
fn initial_area_respects_neighboring_graphics_and_retains_unfittable_selection(){
 let e=engine();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("obstacles.pdf");
 for rotation in [0,90,180,270]{for (right,below) in [(true,false),(false,true),(true,true)]{
  fixture(&path,rotation);add_obstacles(&path,right,below);let source=e.open_document(&path).unwrap();let original=e.source_bytes(&source.id).unwrap();
  let selected=e.inspect_styled_paragraph(&source.id,0,&[0,1]).unwrap();let area=selected.bounds.clone();
  let request=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:selected.object_indices,expected_text:selected.text,runs:selected.runs,style_changes:vec![],bounds:area.clone(),alignment:ParagraphAlignment::Left,line_spacing:1.2};
  let (width,height)=if rotation%180==0{(area.width,area.height)}else{(area.height,area.width)};
  if right{assert!(width<=56.21,"must not expand through right-hand graphic: {width}");}
  if below{assert!(height<=22.51,"must not expand through lower graphic: {height}");}
  if right&&below{assert!(e.layout_styled_paragraph(&source.id,0,&request).is_err());assert!(e.preview_styled_paragraph(&source.id,0,&request).is_err());}
  else{let preview=e.preview_styled_paragraph(&source.id,0,&request).unwrap();e.close_document(&preview.id).unwrap();}
  assert_eq!(e.source_bytes(&source.id).unwrap(),original);e.close_document(&source.id).unwrap();
 }}
}
#[test]
fn inspected_mixed_paragraph_previews_unchanged_with_its_initial_area(){
 let e=engine();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("initial.pdf");
 for rotation in [0,90,180,270]{
  fixture(&path,rotation);let source=e.open_document(&path).unwrap();let original=e.source_bytes(&source.id).unwrap();
  let selected=e.inspect_styled_paragraph(&source.id,0,&[0,1]).unwrap();
  let request=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:selected.object_indices,expected_text:selected.text,runs:selected.runs,style_changes:vec![],bounds:selected.bounds,alignment:ParagraphAlignment::Left,line_spacing:1.2};
  if rotation==0{
   for (label,width,height) in [("original",request.bounds.width,request.bounds.height),("wider",request.bounds.width+2.,request.bounds.height),("taller",request.bounds.width,100.)]{
    let mut probe=request.clone();probe.bounds.width=width;probe.bounds.height=height;
    match e.layout_styled_paragraph(&source.id,0,&probe){Ok(layout)=>println!("{label}: width={width} height={height} lines={}",layout.lines.len()),Err(error)=>println!("{label}: width={width} height={height} error={error}")}
   }
  }
  let layout=e.layout_styled_paragraph(&source.id,0,&request).expect("unchanged initial area must fit");assert_eq!(layout.lines.len(),1);
  let preview=e.preview_styled_paragraph(&source.id,0,&request).unwrap();e.close_document(&preview.id).unwrap();
  assert_eq!(e.source_bytes(&source.id).unwrap(),original);assert_eq!(std::fs::read(&path).unwrap(),original.as_ref());e.close_document(&source.id).unwrap();
 }
}
