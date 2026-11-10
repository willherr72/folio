//! Source-alignment fidelity in the ordinary attributed paragraph workflow.
use folio_engine::{PdfEngine,ParagraphAlignment as Align,StyledParagraphLayoutRequest,StyledParagraphRun};
use lopdf::{dictionary,Document,Object,Stream};
use std::path::{Path,PathBuf};

fn engine()->PdfEngine{PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture(path:&Path,alignment:Align,mixed:bool,rotation:i32,unit:f32){
 fixture_with_leading(path,alignment,mixed,rotation,unit,19);
}
fn fixture_with_leading(path:&Path,alignment:Align,mixed:bool,rotation:i32,unit:f32,leading:usize){
 let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
 let plain=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
 let bold=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier-Bold"});
 let mut content=String::new();
 for (row,(a,b)) in [("Long source ","mixed text"),("Short ","row"),("Medium source ","line")].into_iter().enumerate(){
  let second_size=if mixed{16.}else{12.};let width=a.len() as f32*7.2+b.len() as f32*second_size*0.6;
  let x=match alignment{Align::Center=>300.-width*0.5,Align::Right=>500.-width,_=>100.};
  let second_x=x+a.len() as f32*7.2;let y=700-row*leading;
  content.push_str(&format!("BT /F1 12 Tf 0 0 0 rg 1 0 0 1 {x} {y} Tm ({a}) Tj ET\n"));
  let font=if mixed{"F2"}else{"F1"};let color=if mixed{"1 0 0"}else{"0 0 0"};
  content.push_str(&format!("BT /{font} {second_size} Tf {color} rg 1 0 0 1 {second_x} {y} Tm ({b}) Tj ET\n"));
 }
 content.push_str("BT /F1 12 Tf 0 0 0 rg 1 0 0 1 50 500 Tm (Neighbor) Tj ET");
 let content=pdf.add_object(Stream::new(dictionary!{},content.into_bytes()));
 let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),600.into(),800.into()],"CropBox"=>vec![10.into(),20.into(),590.into(),780.into()],"Rotate"=>rotation,"UserUnit"=>Object::Real(unit),"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>plain,"F2"=>bold}},"Contents"=>content});
 pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",root);pdf.save(path).unwrap();
}
fn request(e:&PdfEngine,id:&str)->StyledParagraphLayoutRequest{
 let s=e.inspect_styled_paragraph(id,0,&[0,1,2,3,4,5]).unwrap();
 StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:s.object_indices,expected_text:s.text,runs:s.runs,style_changes:vec![],bounds:s.bounds,alignment:s.alignment.unwrap_or(Align::Left),line_spacing:s.line_spacing.unwrap_or(1.2)}
}
fn expected_anchor(alignment:Align)->f32{match alignment{Align::Center=>300.,Align::Right=>500.,_=>100.}}
fn anchor(bounds:[f32;4],alignment:Align)->f32{match alignment{Align::Center=>(bounds[0]+bounds[2])*0.5,Align::Right=>bounds[2],_=>bounds[0]}}

#[test]fn fresh_mixed_rows_retain_source_line_spacing_without_unneeded_widening(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("source.pdf");
 for leading in [18,19]{
 fixture_with_leading(&path,Align::Center,true,0,1.,leading);let source=e.open_document(&path).unwrap();let draft=request(&e,&source.id);let layout=e.layout_styled_paragraph(&source.id,0,&draft).unwrap();
 assert_eq!(layout.lines.len(),3,"unchanged three-row source was rewrapped: spacing={}, frame={:?}, rows={:?}",draft.line_spacing,layout.pdf_bounds,layout.lines);
 assert_eq!(draft.line_spacing,leading as f32/16.);
 let gap=leading as f32;
 for (index,line) in layout.lines.iter().enumerate(){let baseline=700.-index as f32*gap;assert!((line.baseline-baseline).abs()<0.001,"source leading {leading} baseline {baseline} became {}",line.baseline);}
 assert!(layout.pdf_bounds[2]-layout.pdf_bounds[0]<185.,"initial frame widened unnecessarily");e.close_document(&source.id).unwrap();
 }
}

#[test]fn fresh_styled_inspection_infers_original_alignment_across_pieces_and_formats(){
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for mixed in [false,true]{for alignment in [Align::Left,Align::Center,Align::Right]{
  let path=temp.path().join("source.pdf");fixture(&path,alignment,mixed,0,1.);let original=std::fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();
  let s=e.inspect_styled_paragraph(&source.id,0,&[0,1,2,3,4,5]).unwrap();
  assert_eq!(s.alignment,Some(alignment),"mixed={mixed}, expected={alignment:?}");
  let first=e.list_text_runs(&source.id,0).unwrap().runs[0].clone();let suggested=e.suggest_styled_paragraph(&source.id,0,first.object_index,&first.text).unwrap();
  assert_eq!(suggested.object_indices,s.object_indices);assert_eq!(suggested.alignment,s.alignment);
  assert_eq!(std::fs::read(path).unwrap(),original);e.close_document(&source.id).unwrap();
 }}
}

#[test]fn fresh_center_and_right_frames_retain_source_anchor_in_physical_pages(){
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for (rotation,unit) in [(0,1.),(90,1.7),(180,2.),(270,1.)]{for alignment in [Align::Center,Align::Right]{
  let path=temp.path().join("source.pdf");fixture(&path,alignment,true,rotation,unit);let source=e.open_document(&path).unwrap();
  let mut draft=request(&e,&source.id);draft.alignment=alignment;
  let layout=e.layout_styled_paragraph(&source.id,0,&draft).unwrap();
  assert!((anchor(layout.pdf_bounds,alignment)-expected_anchor(alignment)).abs()<0.001,"{alignment:?} rotation={rotation} unit={unit} frame={:?}",layout.pdf_bounds);
  assert!((layout.lines[0].baseline-700.).abs()<0.001);
  let preview=e.preview_styled_paragraph(&source.id,0,&draft).unwrap();let saved=temp.path().join("saved.pdf");std::fs::write(&saved,e.source_bytes(&preview.id).unwrap()).unwrap();let reopened=e.open_document(&saved).unwrap();
  let ids=e.list_text_runs(&reopened.id,0).unwrap().runs.into_iter().filter(|r|r.text.trim()!="Neighbor").map(|r|r.object_index).collect::<Vec<_>>();let remembered=e.inspect_styled_paragraph(&reopened.id,0,&ids).unwrap();assert_eq!(remembered.alignment,Some(alignment));assert_eq!(remembered.bounds,draft.bounds);
  let again=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:remembered.object_indices,expected_text:remembered.text,runs:remembered.runs,style_changes:vec![],bounds:remembered.bounds,alignment:remembered.alignment.unwrap(),line_spacing:remembered.line_spacing.unwrap()};
  let repeated=e.preview_styled_paragraph(&reopened.id,0,&again).unwrap();assert_eq!(e.render_page(&preview.id,0,800).unwrap(),e.render_page(&repeated.id,0,800).unwrap());
  if let Some(root)=std::env::var_os("FOLIO_SOURCE_ALIGNMENT_DIR"){
   let root=PathBuf::from(root);std::fs::create_dir_all(&root).unwrap();let key=format!("{alignment:?}-{rotation}-{unit}");
   for (name,bytes) in [("source",e.source_bytes(&source.id).unwrap()),("edited",e.source_bytes(&preview.id).unwrap()),("repeated",e.source_bytes(&repeated.id).unwrap())]{std::fs::write(root.join(format!("{key}-{name}.pdf")),bytes).unwrap();}
   std::fs::write(root.join(format!("{key}.json")),serde_json::to_vec(&serde_json::json!({"alignment":alignment,"rotation":rotation,"unit":unit,"frame":layout.pdf_bounds,"anchor":expected_anchor(alignment),"firstBaseline":700,"sourceLeading":19})).unwrap()).unwrap();
  }
  for id in [&source.id,&preview.id,&reopened.id,&repeated.id]{e.close_document(id).unwrap();}
 }}
}

#[test]fn trailing_spaces_do_not_authorize_alignment_on_single_or_equal_width_rows(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("source.pdf");fixture(&path,Align::Center,true,0,1.);
 let mut pdf=Document::load(&path).unwrap();let page=*pdf.get_pages().get(&1).unwrap();let mut content=lopdf::content::Content::decode(&pdf.get_page_content(page)).unwrap();let mut show=0;
 for op in &mut content.operations{if op.operator=="Tj"{if [1,3,5].contains(&show){let mut bytes=op.operands[0].as_str().unwrap().to_vec();bytes.extend_from_slice(b"   ");op.operands[0]=Object::String(bytes,lopdf::StringFormat::Hexadecimal);}show+=1;}}
 let stream=pdf.add_object(Stream::new(dictionary!{},content.encode().unwrap()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);pdf.save(&path).unwrap();let source=e.open_document(&path).unwrap();
 assert_eq!(e.inspect_styled_paragraph(&source.id,0,&[0,1,2,3,4,5]).unwrap().alignment,Some(Align::Center));assert_eq!(e.inspect_styled_paragraph(&source.id,0,&[0,1]).unwrap().alignment,None);e.close_document(&source.id).unwrap();
 let stream=pdf.add_object(Stream::new(dictionary!{},b"BT /F1 12 Tf 100 700 Td (Same text) Tj 0 -18 Td (Same text) Tj 0 -18 Td (Same text) Tj ET".to_vec()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);pdf.save(&path).unwrap();let source=e.open_document(&path).unwrap();
 assert_eq!(e.inspect_styled_paragraph(&source.id,0,&[0,1,2]).unwrap().alignment,None);e.close_document(&source.id).unwrap();
}

#[test]fn centered_highlighted_style_keeps_native_preview_and_saved_replay(){
 let e=engine();let temp=tempfile::tempdir().unwrap();for leading in [18,19]{let path=temp.path().join("marked.pdf");fixture_with_leading(&path,Align::Center,true,0,1.,leading);
 let mut pdf=Document::load(&path).unwrap();let page=*pdf.get_pages().get(&1).unwrap();let mut bytes=format!("q 1 1 0 rg 306.2 {} 30.8 18 re f Q\n",697-leading).into_bytes();bytes.extend(pdf.get_page_content(page));let stream=pdf.add_object(Stream::new(dictionary!{},bytes));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);pdf.save(&path).unwrap();
 let original=std::fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();let ids=e.list_text_runs(&source.id,0).unwrap().runs.into_iter().filter(|r|r.text.trim()!="Neighbor").map(|r|r.object_index).collect::<Vec<_>>();let s=e.inspect_styled_paragraph(&source.id,0,&ids).unwrap();assert_eq!(s.alignment,Some(Align::Center));assert!(s.styles.iter().any(|style|style.background_color.as_deref()==Some("#FFFF00")));
 let draft=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:s.object_indices,expected_text:s.text,runs:s.runs,style_changes:vec![],bounds:s.bounds,alignment:s.alignment.unwrap(),line_spacing:s.line_spacing.unwrap()};let layout=e.layout_styled_paragraph(&source.id,0,&draft).unwrap();assert_eq!(layout.lines.len(),3);for (row,line) in layout.lines.iter().enumerate(){assert!((line.baseline-(700.-row as f32*leading as f32)).abs()<0.001);}
 let preview=e.preview_styled_paragraph(&source.id,0,&draft).unwrap();
 let saved=temp.path().join("saved.pdf");std::fs::write(&saved,e.source_bytes(&preview.id).unwrap()).unwrap();let reopened=e.open_document(&saved).unwrap();let ids=e.list_text_runs(&reopened.id,0).unwrap().runs.into_iter().filter(|r|r.text.trim()!="Neighbor").map(|r|r.object_index).collect::<Vec<_>>();let remembered=e.inspect_styled_paragraph(&reopened.id,0,&ids).unwrap();assert_eq!(remembered.alignment,Some(Align::Center));assert!(remembered.styles.iter().any(|style|style.background_color.as_deref()==Some("#FFFF00")));
 let again=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:remembered.object_indices,expected_text:remembered.text,runs:remembered.runs,style_changes:vec![],bounds:remembered.bounds,alignment:remembered.alignment.unwrap(),line_spacing:remembered.line_spacing.unwrap()};let repeated=e.preview_styled_paragraph(&reopened.id,0,&again).unwrap();assert_eq!(e.render_page(&preview.id,0,800).unwrap(),e.render_page(&repeated.id,0,800).unwrap());assert_eq!(std::fs::read(&path).unwrap(),original);
 for id in [&source.id,&preview.id,&reopened.id,&repeated.id]{e.close_document(id).unwrap();}
 }
}

#[test]fn fitting_an_unbreakable_word_preserves_center_or_right_anchor(){
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for alignment in [Align::Center,Align::Right]{
  let path=temp.path().join("source.pdf");fixture(&path,alignment,false,0,1.);let source=e.open_document(&path).unwrap();
  let mut draft=request(&e,&source.id);draft.alignment=alignment;draft.runs=vec![StyledParagraphRun{text:"Unbreakableword".into(),style_id:0}];
  // Display coordinates include the10pt left/20pt bottom CropBox offset.
  draft.bounds.x=match alignment{Align::Center=>260.,_=>430.};draft.bounds.width=60.;draft.bounds.y=80.;draft.bounds.height=30.;
  let original=draft.bounds.clone();let old_anchor=match alignment{Align::Center=>original.x+original.width*0.5,_=>original.x+original.width};
  draft.bounds=e.fit_styled_paragraph(&source.id,0,&draft).unwrap();assert!(draft.bounds.width>original.width);
  let fitted_anchor=match alignment{Align::Center=>draft.bounds.x+draft.bounds.width*0.5,_=>draft.bounds.x+draft.bounds.width};
  assert!((fitted_anchor-old_anchor).abs()<0.001,"{alignment:?}: {original:?} -> {:?}",draft.bounds);
  let preview=e.preview_styled_paragraph(&source.id,0,&draft).unwrap();e.close_document(&preview.id).unwrap();e.close_document(&source.id).unwrap();
 }
}

#[test]fn edge_highlights_preserve_text_anchors_and_saved_replay(){
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for alignment in [Align::Center,Align::Right]{for side in ["left","right"]{for marked_row in [0,1]{
  let path=temp.path().join("edge.pdf");fixture(&path,alignment,false,0,1.);
  let mut pdf=Document::load(&path).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
  let target=expected_anchor(alignment);let marked_width=if marked_row==0{72.}else{43.2};let x=if alignment==Align::Center{target-marked_width*0.5}else{target-marked_width};
  let left=if side=="left"{x-4.}else{x};let width=marked_width+4.;let bottom=698-marked_row*19;
  let mut bytes=format!("q 1 1 0 rg {left} {bottom} {width} 12 re f Q\n");
  for (row,count) in [10,6,9].into_iter().enumerate(){let width=count as f32*7.2;let x=if alignment==Align::Center{target-width*0.5}else{target-width};let y=700-row*19;
   bytes.push_str(&format!("BT /F1 12 Tf 1 0 0 1 {x} {y} Tm ({}) Tj ET\n","M".repeat(count)));
  }
  bytes.push_str("BT /F1 12 Tf 50 500 Td (Neighbor) Tj ET");
  let stream=pdf.add_object(Stream::new(dictionary!{},bytes.into_bytes()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);pdf.save(&path).unwrap();
  let source=e.open_document(&path).unwrap();let ids=e.list_text_runs(&source.id,0).unwrap().runs.into_iter().filter(|r|r.text.trim()!="Neighbor").map(|r|r.object_index).collect::<Vec<_>>();
  let selection=e.inspect_styled_paragraph(&source.id,0,&ids).unwrap();assert_eq!(selection.alignment,Some(alignment));assert!(selection.styles.iter().any(|s|s.background_color.is_some()));
  let draft=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:selection.object_indices,expected_text:selection.text,runs:selection.runs,style_changes:vec![],bounds:selection.bounds,alignment,line_spacing:selection.line_spacing.unwrap()};
  let layout=e.layout_styled_paragraph(&source.id,0,&draft).unwrap();assert_eq!(layout.lines.len(),3);
  for (row,line) in layout.lines.iter().enumerate(){let width=[72.,43.2,64.8][row];let observed=line.runs[0].origin[0]+if alignment==Align::Center{width*0.5}else{width};assert!((observed-target).abs()<0.001,"{alignment:?} {side} row{row}: text anchor {observed}, expected {target}");}
  let preview=e.preview_styled_paragraph(&source.id,0,&draft).unwrap();let saved=temp.path().join("saved.pdf");std::fs::write(&saved,e.source_bytes(&preview.id).unwrap()).unwrap();let reopened=e.open_document(&saved).unwrap();
  let ids=e.list_text_runs(&reopened.id,0).unwrap().runs.into_iter().filter(|r|r.text.trim()!="Neighbor").map(|r|r.object_index).collect::<Vec<_>>();let remembered=e.inspect_styled_paragraph(&reopened.id,0,&ids).unwrap();assert_eq!(remembered.alignment,Some(alignment));assert_eq!(remembered.bounds,draft.bounds);
  let again=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:remembered.object_indices,expected_text:remembered.text,runs:remembered.runs,style_changes:vec![],bounds:remembered.bounds,alignment,line_spacing:remembered.line_spacing.unwrap()};
  let repeated=e.preview_styled_paragraph(&reopened.id,0,&again).unwrap();assert_eq!(e.render_page(&preview.id,0,800).unwrap(),e.render_page(&repeated.id,0,800).unwrap());
  if let Some(root)=std::env::var_os("FOLIO_EDGE_ALIGNMENT_DIR"){let root=PathBuf::from(root);std::fs::create_dir_all(&root).unwrap();let key=format!("{alignment:?}-{side}-{marked_row}");for (name,id) in [("source",&source.id),("edited",&preview.id),("repeated",&repeated.id)]{std::fs::write(root.join(format!("{key}-{name}.pdf")),e.source_bytes(id).unwrap()).unwrap();}}
  let mut grown=draft.clone();grown.runs[0].text.insert_str(0,"MMMM");grown.bounds=e.fit_styled_paragraph(&source.id,0,&grown).unwrap();let enlarged=e.layout_styled_paragraph(&source.id,0,&grown).unwrap();
  assert!(grown.bounds.width>draft.bounds.width);
  for line in &enlarged.lines{let width=line.runs.iter().map(|r|r.text.chars().filter(|c|*c=='M').count()).sum::<usize>() as f32*7.2;let observed=line.runs[0].origin[0]+if alignment==Align::Center{width*0.5}else{width};assert!((observed-target).abs()<0.002,"Fitted highlighted text moved its anchor: {observed}");}
  let expanded=e.preview_styled_paragraph(&source.id,0,&grown).unwrap();e.close_document(&expanded.id).unwrap();
  for id in [&source.id,&preview.id,&reopened.id,&repeated.id]{e.close_document(id).unwrap();}
 }}}
}

#[test]fn legacy_native_ink_highlights_retain_pixels_and_policy_on_scaled_rotated_pages(){
 let e=engine();let path=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/paragraphs/legacy-highlight-v3-physical.pdf");let original=std::fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();
 let ids=e.list_text_runs(&source.id,0).unwrap().runs.into_iter().filter(|r|r.text.trim()!="Neighbor").map(|r|r.object_index).collect::<Vec<_>>();let s=e.inspect_styled_paragraph(&source.id,0,&ids).unwrap();assert_eq!(s.line_spacing,Some(1.5));
 let draft=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:s.object_indices,expected_text:s.text,runs:s.runs,style_changes:vec![],bounds:s.bounds,alignment:s.alignment.unwrap(),line_spacing:s.line_spacing.unwrap()};let preview=e.preview_styled_paragraph(&source.id,0,&draft).unwrap();
 assert_eq!(e.render_page(&source.id,0,900).unwrap(),e.render_page(&preview.id,0,900).unwrap());
 let pdf=Document::load_mem(&e.source_bytes(&preview.id).unwrap()).unwrap();let page=*pdf.get_pages().get(&1).unwrap();let metadata:serde_json::Value=serde_json::from_slice(pdf.get_dictionary(page).unwrap().get(b"FolioStyledParagraphs").unwrap().as_str().unwrap()).unwrap();assert_eq!(metadata["version"],3);assert_eq!(metadata["paragraphs"][0]["native_ink"],true);assert!(metadata["paragraphs"][0].get("text_alignment").is_none());
 assert_eq!(std::fs::read(path).unwrap(),original);for id in [&source.id,&preview.id]{e.close_document(id).unwrap();}
}


#[test]fn tight_source_leading_survives_physical_save_reopen_and_exact_repeat(){
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for (rotation,unit) in [(0,1.),(90,1.7),(180,2.),(270,1.3)]{for alignment in [Align::Center,Align::Right]{
  let path=temp.path().join("source.pdf");fixture_with_leading(&path,alignment,true,rotation,unit,18);
  let original=std::fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();let draft=request(&e,&source.id);
  let layout=e.layout_styled_paragraph(&source.id,0,&draft).unwrap();assert_eq!(layout.lines.len(),3);
  for (row,line) in layout.lines.iter().enumerate(){assert!((line.baseline-(700.-row as f32*18.)).abs()<0.001);}
  let preview=e.preview_styled_paragraph(&source.id,0,&draft).unwrap();let saved=temp.path().join("saved.pdf");std::fs::write(&saved,e.source_bytes(&preview.id).unwrap()).unwrap();let reopened=e.open_document(&saved).unwrap();
  let ids=e.list_text_runs(&reopened.id,0).unwrap().runs.into_iter().filter(|r|r.text.trim()!="Neighbor").map(|r|r.object_index).collect::<Vec<_>>();
  let s=e.inspect_styled_paragraph(&reopened.id,0,&ids).unwrap();assert_eq!(s.bounds,draft.bounds);assert_eq!(s.runs,draft.runs);assert_eq!(s.line_spacing,Some(18./16.));
  let again=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:s.object_indices,expected_text:s.text,runs:s.runs,style_changes:vec![],bounds:s.bounds,alignment:s.alignment.unwrap(),line_spacing:s.line_spacing.unwrap()};
  assert_eq!(e.layout_styled_paragraph(&reopened.id,0,&again).unwrap(),layout);
  let repeated=e.preview_styled_paragraph(&reopened.id,0,&again).unwrap();assert_eq!(e.render_page(&preview.id,0,800).unwrap(),e.render_page(&repeated.id,0,800).unwrap());
  let pdf=Document::load_mem(&e.source_bytes(&repeated.id).unwrap()).unwrap();let page=*pdf.get_pages().get(&1).unwrap();let meta:serde_json::Value=serde_json::from_slice(pdf.get_dictionary(page).unwrap().get(b"FolioStyledParagraphs").unwrap().as_str().unwrap()).unwrap();assert_eq!(meta["version"],5);assert_eq!(meta["paragraphs"][0]["ink_vertical"],true);
  if let Some(root)=std::env::var_os("FOLIO_TIGHT_LEADING_DIR"){
   let root=PathBuf::from(root);std::fs::create_dir_all(&root).unwrap();let key=format!("{alignment:?}-{rotation}-{unit}");
   for (name,id) in [("source",&source.id),("edited",&preview.id),("repeated",&repeated.id)]{std::fs::write(root.join(format!("{key}-{name}.pdf")),e.source_bytes(id).unwrap()).unwrap();}
   std::fs::write(root.join(format!("{key}.json")),serde_json::to_vec(&serde_json::json!({"alignment":alignment,"rotation":rotation,"unit":unit,"frame":layout.pdf_bounds,"anchor":expected_anchor(alignment),"firstBaseline":700,"sourceLeading":18})).unwrap()).unwrap();
  }
  assert_eq!(std::fs::read(&path).unwrap(),original);for id in [&source.id,&preview.id,&reopened.id,&repeated.id]{e.close_document(id).unwrap();}
 }}
}

#[test]fn legacy_v4_tight_leading_keeps_font_minimum_and_exact_pixels(){
 let e=engine();let path=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/paragraphs/legacy-tight-v4.pdf");let original=std::fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();
 let draft=request(&e,&source.id);assert_eq!(draft.line_spacing,18./16.);let layout=e.layout_styled_paragraph(&source.id,0,&draft).unwrap();
 for (row,line) in layout.lines.iter().enumerate(){assert!((line.baseline-(700.-row as f32*18.112)).abs()<0.001);}
 assert_eq!(layout.lines.len(),3);let preview=e.preview_styled_paragraph(&source.id,0,&draft).unwrap();
 assert_eq!(e.render_page(&source.id,0,1000).unwrap(),e.render_page(&preview.id,0,1000).unwrap());
 let pdf=Document::load_mem(&e.source_bytes(&preview.id).unwrap()).unwrap();let page=*pdf.get_pages().get(&1).unwrap();let meta:serde_json::Value=serde_json::from_slice(pdf.get_dictionary(page).unwrap().get(b"FolioStyledParagraphs").unwrap().as_str().unwrap()).unwrap();assert_eq!(meta["version"],4);assert!(meta["paragraphs"][0].get("ink_vertical").is_none());
 let mut changed=draft.clone();changed.style_changes=vec![folio_engine::StyledParagraphStyleChange{style_id:0,source_style_id:0,font_size:None,color:Some("#003399".into()),font_id:None}];
 assert_eq!(e.layout_styled_paragraph(&source.id,0,&changed).unwrap(),layout);
 let colored=e.preview_styled_paragraph(&source.id,0,&changed).unwrap();let pdf=Document::load_mem(&e.source_bytes(&colored.id).unwrap()).unwrap();let page=*pdf.get_pages().get(&1).unwrap();let meta:serde_json::Value=serde_json::from_slice(pdf.get_dictionary(page).unwrap().get(b"FolioStyledParagraphs").unwrap().as_str().unwrap()).unwrap();assert_eq!(meta["version"],4);assert!(meta["paragraphs"][0].get("ink_vertical").is_none());
 assert_eq!(std::fs::read(&path).unwrap(),original);for id in [&source.id,&preview.id,&colored.id]{e.close_document(id).unwrap();}
}


#[test]fn old_and_new_vertical_policies_coexist_through_neighbor_edit_and_saved_replay(){
 let e=engine();let dir=tempfile::tempdir().unwrap();let path=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/paragraphs/legacy-tight-v4.pdf");let original=std::fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();
 let legacy=request(&e,&source.id);let legacy_layout=e.layout_styled_paragraph(&source.id,0,&legacy).unwrap();
 let neighbor=e.list_text_runs(&source.id,0).unwrap().runs.into_iter().find(|r|r.text=="Neighbor").unwrap();
 let selected=e.inspect_styled_paragraph(&source.id,0,&[neighbor.object_index]).unwrap();
 let mut new=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:selected.object_indices,expected_text:selected.text,runs:vec![StyledParagraphRun{text:"Neighbor edited".into(),style_id:0}],style_changes:vec![],bounds:selected.bounds,alignment:Align::Left,line_spacing:1.2};
 new.bounds=e.fit_styled_paragraph(&source.id,0,&new).unwrap();let combined=e.preview_styled_paragraph(&source.id,0,&new).unwrap();
 let saved=dir.path().join("combined.pdf");std::fs::write(&saved,e.source_bytes(&combined.id).unwrap()).unwrap();let reopened=e.open_document(&saved).unwrap();
 let mut old=request(&e,&reopened.id);assert_eq!(e.layout_styled_paragraph(&reopened.id,0,&old).unwrap(),legacy_layout);
 old.style_changes=vec![folio_engine::StyledParagraphStyleChange{style_id:0,source_style_id:0,font_size:None,color:Some("#003399".into()),font_id:None}];
 let colored=e.preview_styled_paragraph(&reopened.id,0,&old).unwrap();
 let pdf=Document::load_mem(&e.source_bytes(&colored.id).unwrap()).unwrap();let page=*pdf.get_pages().get(&1).unwrap();let meta:serde_json::Value=serde_json::from_slice(pdf.get_dictionary(page).unwrap().get(b"FolioStyledParagraphs").unwrap().as_str().unwrap()).unwrap();assert_eq!(meta["version"],5);let records=meta["paragraphs"].as_array().unwrap();assert_eq!(records.len(),2);assert_eq!(records.iter().filter(|r|r.get("ink_vertical").is_none()).count(),1);assert_eq!(records.iter().filter(|r|r["ink_vertical"]==true).count(),1);
 let selected=request(&e,&colored.id);assert_eq!(e.layout_styled_paragraph(&colored.id,0,&selected).unwrap(),legacy_layout);
 let repeated=e.preview_styled_paragraph(&colored.id,0,&selected).unwrap();assert_eq!(e.render_page(&colored.id,0,1000).unwrap(),e.render_page(&repeated.id,0,1000).unwrap());
 let neighbor=e.list_text_runs(&repeated.id,0).unwrap().runs.into_iter().find(|r|r.text=="Neighbor edited").unwrap();let current=e.inspect_styled_paragraph(&repeated.id,0,&[neighbor.object_index]).unwrap();assert_eq!(current.line_spacing,Some(1.2));assert_eq!(current.bounds,new.bounds);
 assert_eq!(std::fs::read(&path).unwrap(),original);for id in [&source.id,&combined.id,&reopened.id,&colored.id,&repeated.id]{e.close_document(id).unwrap();}
}
