use folio_engine::{PdfEngine,StyledParagraphLayoutRequest,StyledParagraphSelection,ParagraphAlignment};
use lopdf::{dictionary,Document,Object,Stream};
use std::path::{Path,PathBuf};

fn engine()->PdfEngine{PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture(path:&Path,rotation:i32,unit:f32){
 let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
 let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
 let stream=pdf.add_object(Stream::new(dictionary!{},b"BT /F1 12 Tf 1 0 0 1 60 600 Tm (AA BB) Tj ET BT /F1 12 Tf 1 0 0 1 250 180 Tm (Neighbor) Tj ET".to_vec()));
 let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),400.into(),700.into()],"CropBox"=>vec![10.into(),20.into(),390.into(),680.into()],"Rotate"=>rotation,"UserUnit"=>Object::Real(unit),"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>stream});
 pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",root);pdf.save(path).unwrap();
}
fn request(s:StyledParagraphSelection)->StyledParagraphLayoutRequest{
 StyledParagraphLayoutRequest{baseline_anchor:s.baseline_anchor,object_indices:s.object_indices,expected_text:s.text,runs:s.runs,style_changes:vec![],bounds:s.bounds,alignment:s.alignment.unwrap_or(ParagraphAlignment::Left),line_spacing:s.line_spacing.unwrap_or(1.2)}
}

#[test]
fn anchored_fit_recovers_from_excessive_soft_rows_without_relaxing_final_limits(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("narrow.pdf");fixture(&path,0,1.);
 let source=e.open_document(&path).unwrap();let mut r=request(e.inspect_styled_paragraph(&source.id,0,&[0]).unwrap());
 r.runs[0].text=vec!["AA";300].join(" ");r.bounds=folio_engine::AnnotationRect{x:50.,y:73.,width:300.,height:350.};
 let layout=e.layout_styled_paragraph(&source.id,0,&r).expect("wide positive control");assert!(layout.lines.len()<256);assert_eq!(layout.lines[0].baseline,600.);
 r.bounds.width=16.;r.bounds.height=20.;let mut legacy=r.clone();legacy.baseline_anchor=None;
 legacy.bounds=e.fit_styled_paragraph(&source.id,0,&legacy).expect("legacy Fit control");
 assert!(e.layout_styled_paragraph(&source.id,0,&legacy).unwrap().lines.len()<256);
 r.bounds=e.fit_styled_paragraph(&source.id,0,&r).expect("anchored Fit must try widening before rejecting wrapped row count");
 let fitted=e.layout_styled_paragraph(&source.id,0,&r).unwrap();assert!(fitted.lines.len()<256);assert_eq!(fitted.lines[0].baseline,600.);
 r.runs[0].text=vec!["AA";257].join("\n");assert!(e.fit_styled_paragraph(&source.id,0,&r).is_err());
 assert!(e.layout_styled_paragraph(&source.id,0,&r).is_err());e.close_document(&source.id).unwrap();
}

#[test]
fn taller_and_shorter_letters_retain_the_first_baseline_through_fit_and_saved_reedit(){
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for (rotation,unit) in [(0,1.),(90,1.7),(180,1.),(270,2.)]{
  let input=temp.path().join(format!("source-{rotation}.pdf"));fixture(&input,rotation,unit);
  let original=std::fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();let mut current=source.clone();
  for (cycle,text) in ["AA BB CC","AA BB","aa bb"].into_iter().enumerate(){
   let runs=e.list_text_runs(&current.id,0).unwrap();let indices=runs.runs.iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect::<Vec<_>>();
   let mut r=request(e.inspect_styled_paragraph(&current.id,0,&indices).unwrap());r.runs[0].text=text.into();
   r.bounds=e.fit_styled_paragraph(&current.id,0,&r).unwrap();
   let layout=e.layout_styled_paragraph(&current.id,0,&r).unwrap();
   assert!((layout.lines[0].baseline-600.).abs()<0.0001,"rotation{rotation}, unit{unit}, cycle{cycle}: baseline600 -> {}",layout.lines[0].baseline);
   let preview=e.preview_styled_paragraph(&current.id,0,&r).unwrap();let saved=temp.path().join(format!("saved-{rotation}-{cycle}.pdf"));std::fs::write(&saved,e.source_bytes(&preview.id).unwrap()).unwrap();
   if let Some(root)=std::env::var_os("FOLIO_BASELINE_ANCHOR_OUTPUTS"){
    let dir=PathBuf::from(root).join(format!("rotate-{rotation}"));std::fs::create_dir_all(&dir).unwrap();
    let original_path=dir.join("source.pdf");if cycle==0{std::fs::write(&original_path,&original).unwrap();}
    let output=dir.join(format!("saved-{cycle}.pdf"));std::fs::copy(&saved,&output).unwrap();
    let previous=if cycle==0{original_path}else{dir.join(format!("saved-{}.pdf",cycle-1))};
    std::fs::write(dir.join(format!("edit-{cycle}.json")),serde_json::to_vec_pretty(&serde_json::json!({"rotation":rotation,"unit":unit,"cycle":cycle,"input":previous,"saved":output,"old":r.expected_text,"new":text,"firstBaseline":layout.lines[0].baseline,"bounds":layout.pdf_bounds})).unwrap()).unwrap();
   }
   let next=e.open_document(&saved).unwrap();
   let indices=e.list_text_runs(&next.id,0).unwrap().runs.iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect::<Vec<_>>();
   let unchanged=request(e.inspect_styled_paragraph(&next.id,0,&indices).unwrap());
   let replay=e.preview_styled_paragraph(&next.id,0,&unchanged).unwrap();
   assert_eq!(e.render_page(&preview.id,0,700).unwrap(),e.render_page(&replay.id,0,700).unwrap());
   e.close_document(&replay.id).unwrap();e.close_document(&preview.id).unwrap();
   if current.id!=source.id{e.close_document(&current.id).unwrap();}current=next;
  }
  assert_eq!(std::fs::read(&input).unwrap(),original);assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),original.as_slice());
  e.close_document(&current.id).unwrap();e.close_document(&source.id).unwrap();
 }
}

#[test]
fn movement_translates_the_anchor_and_legacy_requests_keep_their_old_policy(){
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for (rotation,delta) in [(0,(0.,10.)),(90,(-10.,0.)),(180,(0.,-10.)),(270,(10.,0.))]{
  let path=temp.path().join(format!("move-{rotation}.pdf"));fixture(&path,rotation,1.);let source=e.open_document(&path).unwrap();
  let mut r=request(e.inspect_styled_paragraph(&source.id,0,&[0]).unwrap());
  let anchor=r.baseline_anchor.as_mut().unwrap();anchor.x+=delta.0;anchor.y+=delta.1;r.bounds.x+=delta.0;r.bounds.y+=delta.1;
  r.runs[0].text="AA BB CC".into();r.bounds=e.fit_styled_paragraph(&source.id,0,&r).unwrap();
  assert!((e.layout_styled_paragraph(&source.id,0,&r).unwrap().lines[0].baseline-590.).abs()<0.0001);
  let edited=e.preview_styled_paragraph(&source.id,0,&r).unwrap();e.close_document(&edited.id).unwrap();
  let mut legacy=request(e.inspect_styled_paragraph(&source.id,0,&[0]).unwrap());legacy.baseline_anchor=None;legacy.runs[0].text="AA BB CC".into();legacy.bounds=e.fit_styled_paragraph(&source.id,0,&legacy).unwrap();
  assert!((e.layout_styled_paragraph(&source.id,0,&legacy).unwrap().lines[0].baseline-599.844).abs()<0.0001);
  if rotation==0{if let Some(root)=std::env::var_os("FOLIO_BASELINE_ANCHOR_OUTPUTS"){
   let dir=PathBuf::from(root).join("negative-control");std::fs::create_dir_all(&dir).unwrap();std::fs::copy(&path,dir.join("source.pdf")).unwrap();
   let shifted=e.preview_styled_paragraph(&source.id,0,&legacy).unwrap();std::fs::write(dir.join("shifted.pdf"),e.source_bytes(&shifted.id).unwrap()).unwrap();e.close_document(&shifted.id).unwrap();
  }}
  e.close_document(&source.id).unwrap();
 }
}

#[test]
fn leading_blank_rows_keep_the_logical_baseline_and_forged_hints_do_not_restore(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("source.pdf");fixture(&path,0,1.);
 let source=e.open_document(&path).unwrap();let mut r=request(e.inspect_styled_paragraph(&source.id,0,&[0]).unwrap());
 r.runs[0].text="\nAA BB CC".into();r.bounds=e.fit_styled_paragraph(&source.id,0,&r).unwrap();
 let layout=e.layout_styled_paragraph(&source.id,0,&r).unwrap();assert_eq!(layout.lines[0].baseline,600.);assert!(layout.lines[1].baseline<600.);
 let preview=e.preview_styled_paragraph(&source.id,0,&r).unwrap();let bytes=e.source_bytes(&preview.id).unwrap();
 let indices=e.list_text_runs(&preview.id,0).unwrap().runs.into_iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect::<Vec<_>>();
 let selected=e.inspect_styled_paragraph(&preview.id,0,&indices).unwrap();assert_eq!(selected.text,"\nAA BB CC");
 assert_eq!(e.layout_styled_paragraph(&preview.id,0,&request(selected)).unwrap().lines[0].baseline,600.);
 let pdf=Document::load_mem(&bytes).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
 let clean:serde_json::Value=serde_json::from_slice(pdf.get_dictionary(page).unwrap().get(b"FolioStyledParagraphs").unwrap().as_str().unwrap()).unwrap();
 assert_eq!(clean["version"],7);assert_eq!(clean["paragraphs"][0]["first_baseline"],600.);
 for case in ["changed","legacy","null","outside"]{
  let mut altered=pdf.clone();let mut hint=clean.clone();match case{
   "changed"=>hint["paragraphs"][0]["first_baseline"]=serde_json::json!(601.),
   "legacy"=>hint["version"]=serde_json::json!(6),
   "null"=>hint["paragraphs"][0]["first_baseline"]=serde_json::Value::Null,
   _=>hint["paragraphs"][0]["first_baseline"]=serde_json::json!(10_000.),
  }
  altered.get_dictionary_mut(page).unwrap().set("FolioStyledParagraphs",Object::string_literal(serde_json::to_vec(&hint).unwrap()));
  let path=temp.path().join(format!("bad-{case}.pdf"));altered.save(&path).unwrap();let opened=e.open_document(&path).unwrap();
  let selected=e.inspect_styled_paragraph(&opened.id,0,&indices).unwrap();assert!(!selected.text.starts_with('\n'),"unproved first blank line restored for{case}");
  e.close_document(&opened.id).unwrap();
 }
 e.close_document(&preview.id).unwrap();e.close_document(&source.id).unwrap();
}

#[test]
fn insufficient_clip_headroom_refuses_instead_of_moving_or_clipping_the_baseline(){
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for height in [11.9,25.]{
  let path=temp.path().join(format!("clip-{height}.pdf"));fixture(&path,0,1.);
  let mut pdf=Document::load(&path).unwrap();let page=*pdf.get_pages().get(&1).unwrap();let content=pdf.get_dictionary(page).unwrap().get(b"Contents").unwrap().as_reference().unwrap();
  pdf.get_object_mut(content).unwrap().as_stream_mut().unwrap().set_content(format!("q 40 595 170 {height} re W n BT /F1 12 Tf 1 0 0 1 60 600 Tm (AA BB) Tj ET Q BT /F1 12 Tf 1 0 0 1 250 180 Tm (Neighbor) Tj ET").into_bytes());pdf.save(&path).unwrap();
  let original=std::fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();let mut r=request(e.inspect_occurrence_styled_paragraph(&source.id,0,&[0]).unwrap());r.runs[0].text="AA BB CC".into();
  let fitted=e.fit_occurrence_styled_paragraph(&source.id,0,&r);
  if height<20.{assert!(fitted.is_err());assert!(e.preview_styled_paragraph_occurrence(&source.id,&[0],0,&r).is_err());}
  else{r.bounds=fitted.unwrap();let preview=e.preview_styled_paragraph_occurrence(&source.id,&[0],0,&r).unwrap().document;let s=e.inspect_occurrence_styled_paragraph(&preview.id,0,&[0]).unwrap();assert!((s.baseline_anchor.unwrap().y-80.).abs()<0.0001);e.close_document(&preview.id).unwrap();}
  assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),original.as_slice());assert_eq!(std::fs::read(path).unwrap(),original);e.close_document(&source.id).unwrap();
 }
}

#[test]
fn valid_legacy_memory_migrates_without_pixel_changes_and_invalid_anchors_refuse(){
 let e=engine();let path=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/paragraphs/legacy-tight-v4.pdf");let original=std::fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();
 let indices=e.list_text_runs(&source.id,0).unwrap().runs.into_iter().filter(|r|r.text!="Neighbor"&&!r.text.trim().is_empty()).map(|r|r.object_index).collect::<Vec<_>>();
 let r=request(e.inspect_styled_paragraph(&source.id,0,&indices).unwrap());assert!(r.baseline_anchor.is_some());
 let preview=e.preview_styled_paragraph(&source.id,0,&r).unwrap();assert_eq!(e.render_page(&source.id,0,700).unwrap(),e.render_page(&preview.id,0,700).unwrap());
 for y in [f32::NAN,f32::INFINITY,10_000.]{let mut bad=r.clone();bad.baseline_anchor.as_mut().unwrap().y=y;assert!(e.preview_styled_paragraph(&source.id,0,&bad).is_err());}
 assert_eq!(std::fs::read(path).unwrap(),original);e.close_document(&preview.id).unwrap();e.close_document(&source.id).unwrap();
}

#[test]
fn changed_sizes_in_mixed_and_highlighted_paragraphs_keep_the_verified_baseline(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let mixed=temp.path().join("mixed.pdf");fixture(&mixed,0,1.7);
 let mut pdf=Document::load(&mixed).unwrap();let page=*pdf.get_pages().get(&1).unwrap();let bold=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica-Bold"});
 pdf.get_dictionary_mut(page).unwrap().get_mut(b"Resources").unwrap().as_dict_mut().unwrap().get_mut(b"Font").unwrap().as_dict_mut().unwrap().set("F2",bold);
 let content=pdf.get_dictionary(page).unwrap().get(b"Contents").unwrap().as_reference().unwrap();
 pdf.get_object_mut(content).unwrap().as_stream_mut().unwrap().set_content(b"BT /F1 12 Tf 1 0 0 1 60 600 Tm (AA ) Tj ET BT /F2 18 Tf 1 0 0 1 81.6 600 Tm (BB) Tj ET BT /F1 12 Tf 1 0 0 1 250 180 Tm (Neighbor) Tj ET".to_vec());pdf.save(&mixed).unwrap();
 let highlight=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/paragraphs/native-ink-v3.pdf");
 for path in [mixed,highlight]{
  let original=std::fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();let indices=e.list_text_runs(&source.id,0).unwrap().runs.into_iter().filter(|r|r.text!="Neighbor"&&!r.text.trim().is_empty()).map(|r|r.object_index).collect::<Vec<_>>();
  let selected=e.inspect_styled_paragraph(&source.id,0,&indices).unwrap();
  assert!(selected.styles.len()>1||selected.styles.iter().any(|s|s.background_color.is_some()));
  let size=selected.styles[0].font_size;let mut r=request(selected);let before=e.layout_styled_paragraph(&source.id,0,&r).unwrap().lines[0].baseline;
  r.style_changes.push(folio_engine::StyledParagraphStyleChange{style_id:0,source_style_id:0,font_size:Some(size+4.),color:None,font_id:None});
  r.bounds=e.fit_styled_paragraph(&source.id,0,&r).unwrap();assert_eq!(e.layout_styled_paragraph(&source.id,0,&r).unwrap().lines[0].baseline,before);
  let preview=e.preview_styled_paragraph(&source.id,0,&r).unwrap();let saved=temp.path().join("formatted.pdf");std::fs::write(&saved,e.source_bytes(&preview.id).unwrap()).unwrap();let reopened=e.open_document(&saved).unwrap();
  let indices=e.list_text_runs(&reopened.id,0).unwrap().runs.into_iter().filter(|r|r.text!="Neighbor"&&!r.text.trim().is_empty()).map(|r|r.object_index).collect::<Vec<_>>();let again=request(e.inspect_styled_paragraph(&reopened.id,0,&indices).unwrap());
  assert_eq!(e.layout_styled_paragraph(&reopened.id,0,&again).unwrap().lines[0].baseline,before);let replay=e.preview_styled_paragraph(&reopened.id,0,&again).unwrap();assert_eq!(e.render_page(&preview.id,0,700).unwrap(),e.render_page(&replay.id,0,700).unwrap());
  assert_eq!(std::fs::read(&path).unwrap(),original);for id in [&source.id,&preview.id,&reopened.id,&replay.id]{e.close_document(id).unwrap();}
 }
}
