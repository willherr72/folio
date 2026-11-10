use folio_engine::{AnnotationRect,ParagraphAlignment,PdfEngine,StyledParagraphLayoutRequest,StyledParagraphRun,StyledParagraphStyleChange};
use lopdf::{dictionary,Document,Stream};
use std::path::{Path,PathBuf};
fn root()->PathBuf{PathBuf::from(env!("CARGO_MANIFEST_DIR"))}
fn engine()->PdfEngine{PdfEngine::start(root().join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture(path:&Path,rotation:i32,highlight:bool){
 let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
 let background=if highlight{"q 1 1 0 rg 70 496 18 16 re f Q "}else{""};
 let content=format!("{background}BT /F1 12 Tf 1 0 0 1 50 500 Tm (AA BB CC) Tj ET BT /F1 12 Tf 1 0 0 1 50 100 Tm (Neighbor) Tj ET");
 let stream=pdf.add_object(Stream::new(dictionary!{},content.into_bytes()));let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),500.into(),600.into()],"CropBox"=>vec![10.into(),20.into(),490.into(),580.into()],"Rotate"=>rotation,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>stream});
 pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());let catalog=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",catalog);pdf.save(path).unwrap();
}
fn request(style_id:usize,bounds:AnnotationRect)->StyledParagraphLayoutRequest{StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:vec![1],expected_text:"AA BB CC".into(),runs:vec![StyledParagraphRun{text:"BB".into(),style_id}],style_changes:vec![],bounds,alignment:ParagraphAlignment::Left,line_spacing:1.2}}
#[test]
fn inspection_and_layout_preserve_partial_highlight_identity_with_crop_and_rotation(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("highlight.pdf");let mut reference=None;
 for (rotation,b) in [(0,[40.,60.,100.,160.]),(90,[340.,40.,160.,100.]),(180,[340.,340.,100.,160.]),(270,[60.,340.,160.,100.])]{
  fixture(&path,rotation,true);let source=e.open_document(&path).unwrap();let original=e.source_bytes(&source.id).unwrap();let pixels=e.render_page(&source.id,0,600).unwrap();
  let selected=e.inspect_styled_paragraph(&source.id,0,&[1]).unwrap();assert_eq!(selected.styles.len(),2);
  assert_eq!(selected.styles[0].background_color,None);assert_eq!(selected.styles[1].background_color.as_deref(),Some("#FFFF00"));
  let dto=serde_json::to_value(&selected).unwrap();assert!(dto["styles"][0].get("backgroundColor").is_none());assert_eq!(dto["styles"][1]["backgroundColor"],"#FFFF00");
  assert_eq!(selected.runs.iter().map(|r|(r.text.as_str(),r.style_id)).collect::<Vec<_>>(),vec![("AA ",0),("BB",1),(" CC",0)]);
  assert_eq!(selected.styles[0].font_name,selected.styles[1].font_name);assert_eq!(selected.styles[0].font_size,selected.styles[1].font_size);
  let mut initial=request(0,selected.bounds.clone());initial.runs=selected.runs.clone();assert!(e.layout_styled_paragraph(&source.id,0,&initial).is_ok());
  let bounds=AnnotationRect{x:b[0],y:b[1],width:b[2],height:b[3]};let marked=request(1,bounds.clone());let plain=request(0,bounds);
  let layout=e.layout_styled_paragraph(&source.id,0,&marked).unwrap();let without=e.layout_styled_paragraph(&source.id,0,&plain).unwrap();
  // Mark reservation belongs to the frame, so changing a word's highlight
  // does not move its text anchor. Vertical mark clearance is still measured.
  assert!((layout.lines[0].runs[0].origin[0]-without.lines[0].runs[0].origin[0]).abs()<0.001);
  assert!(layout.lines[0].runs[0].origin[0]>layout.pdf_bounds[0]+0.5);assert!(layout.lines[0].baseline<without.lines[0].baseline-0.5);
  if let Some(expected)=&reference{assert_eq!(&layout,expected);}else{reference=Some(layout);}
  let preview=e.preview_styled_paragraph(&source.id,0,&marked).unwrap();e.close_document(&preview.id).unwrap();
  assert_eq!(e.source_bytes(&source.id).unwrap(),original);assert_eq!(std::fs::read(&path).unwrap(),original.as_ref());assert_eq!(e.render_page(&source.id,0,600).unwrap(),pixels);e.close_document(&source.id).unwrap();
 }
}

#[test]
fn original_background_identity_survives_dense_style_remapping_and_font_substitution(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("highlight.pdf");fixture(&path,0,true);let source=e.open_document(&path).unwrap();
 let font=e.fonts().register(std::fs::read(root().join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
 for font_id in [None,Some(font.id.clone())]{
  let bounds=AnnotationRect{x:40.,y:60.,width:200.,height:160.};let mut marked=request(0,bounds.clone());
  marked.style_changes=vec![StyledParagraphStyleChange{style_id:0,source_style_id:1,font_size:Some(18.),color:Some("#993366".into()),font_id:font_id.clone()}];
  let mut plain=marked.clone();plain.style_changes[0].source_style_id=0;
  let a=e.layout_styled_paragraph(&source.id,0,&marked).unwrap();let b=e.layout_styled_paragraph(&source.id,0,&plain).unwrap();
  assert!((a.lines[0].runs[0].origin[0]-b.lines[0].runs[0].origin[0]).abs()<0.001);
  assert!(a.lines[0].runs[0].origin[0]>a.pdf_bounds[0]+0.5);
  // Font ascent can already reserve part or all of the upper inset. Background
  // reservation cannot move the baseline upward, but need not add a fixed delta.
  assert!(a.lines[0].baseline<=b.lines[0].baseline);
 }
 e.fonts().remove(&font.id).unwrap();e.close_document(&source.id).unwrap();
}

#[test]
fn incomplete_ownership_is_atomic_and_unrelated_plain_paragraphs_stay_editable(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("shared.pdf");fixture(&path,0,true);
 let mut pdf=Document::load(&path).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
 let stream=pdf.add_object(Stream::new(dictionary!{},b"q 1 1 0 rg 45 496 55 16 re f Q BT /F1 12 Tf 50 500 Td (AA) Tj ET BT /F1 12 Tf 80 500 Td (BB) Tj ET BT /F1 12 Tf 50 100 Td (Neighbor) Tj ET".to_vec()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);pdf.save(&path).unwrap();
 let source=e.open_document(&path).unwrap();let original=e.source_bytes(&source.id).unwrap();
 let complete=e.inspect_styled_paragraph(&source.id,0,&[1,2]).unwrap();assert_eq!(complete.styles.len(),1);assert_eq!(complete.text,"AA BB");
 let partial=e.inspect_styled_paragraph(&source.id,0,&[1]).unwrap();assert_eq!(partial.styles.len(),1);assert_eq!(partial.text,"AA");
 let mut edit=request(0,AnnotationRect{x:40.,y:60.,width:25.,height:100.});edit.expected_text=partial.text;edit.runs=partial.runs;
 assert!(e.preview_styled_paragraph(&source.id,0,&edit).unwrap_err().to_string().contains("highlight"));
 let plain=e.inspect_styled_paragraph(&source.id,0,&[3]).unwrap();assert_eq!(plain.styles.len(),1);assert_eq!(plain.styles[0].object_indices,vec![3]);
 edit.object_indices=vec![3];edit.expected_text=plain.text;edit.runs=vec![StyledParagraphRun{text:"Neighbor edited".into(),style_id:0}];edit.bounds=AnnotationRect{x:40.,y:450.,width:200.,height:60.};
 let preview=e.preview_styled_paragraph(&source.id,0,&edit).unwrap();assert_eq!(e.source_bytes(&source.id).unwrap(),original);assert_eq!(std::fs::read(&path).unwrap(),original.as_ref());
 e.close_document(&preview.id).unwrap();e.close_document(&source.id).unwrap();
}

#[test]
fn unrelated_artwork_does_not_reorder_an_existing_plain_palette(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("plain.pdf");fixture(&path,0,false);
 let mut pdf=Document::load(&path).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
 let stream=pdf.add_object(Stream::new(dictionary!{},b"q 1 0 0 rg 300 100 20 20 re f Q BT /F1 12 Tf 50 500 Td (Plain) Tj ET BT /F1 18 Tf 50 475 Td (large) Tj ET".to_vec()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);pdf.save(&path).unwrap();
 let source=e.open_document(&path).unwrap();let selected=e.inspect_styled_paragraph(&source.id,0,&[1,2]).unwrap();assert_eq!(selected.styles.len(),2);assert_eq!(selected.styles[0].font_size,12.);assert_eq!(selected.styles[1].font_size,18.);assert_eq!(selected.runs.iter().map(|r|r.style_id).collect::<Vec<_>>(),vec![0,1]);e.close_document(&source.id).unwrap();
}
