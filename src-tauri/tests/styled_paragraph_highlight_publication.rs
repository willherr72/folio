//! End-to-end highlighted publication and independently verified saved memory.
use folio_engine::{AnnotationRect,ExportRequest,PagePlan,ParagraphAlignment,PdfEngine,StyledParagraphLayoutRequest,StyledParagraphRun,StyledParagraphSelection};
use lopdf::{dictionary,Document,Object,Stream};
use serde_json::json;
use std::path::{Path,PathBuf};
fn engine()->PdfEngine{PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture(path:&Path,rotation:i32,partial:bool){
 let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
 let (rectangle,text)=if partial{("70 496 18 16","AA BB CC")}else{("48 496 39 16","AA BB")};
 let stream=pdf.add_object(Stream::new(dictionary!{},format!("q 1 1 0 rg {rectangle} re f Q BT /F1 12 Tf 50 500 Td ({text}) Tj ET BT /F1 12 Tf 50 100 Td (Neighbor) Tj ET").into_bytes()));
 let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),500.into(),600.into()],"CropBox"=>vec![10.into(),20.into(),490.into(),580.into()],"Rotate"=>rotation,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>stream});
 pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());let catalog=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",catalog);pdf.save(path).unwrap();
}
fn save(e:&PdfEngine,doc:&folio_engine::DocumentInfo,path:&Path){e.export_pdf(ExportRequest{flatten:false,pages:vec![PagePlan{crop: None,id:"p".into(),source_id:doc.id.clone(),page_index:0,width:doc.pages[0].width,height:doc.pages[0].height,rotation:0,overlays:vec![]}]},path).unwrap();}
fn selected(e:&PdfEngine,id:&str)->StyledParagraphSelection{let ids=e.list_text_runs(id,0).unwrap().runs.into_iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect::<Vec<_>>();e.inspect_styled_paragraph(id,0,&ids).unwrap()}
fn again(s:&StyledParagraphSelection)->StyledParagraphLayoutRequest{StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:s.object_indices.clone(),expected_text:s.text.clone(),runs:s.runs.clone(),style_changes:vec![],bounds:s.bounds.clone(),alignment:s.alignment.unwrap(),line_spacing:s.line_spacing.unwrap()}}
fn counts(bytes:&[u8])->(usize,usize){let pdf=Document::load_mem(bytes).unwrap();let id=*pdf.get_pages().get(&1).unwrap();let content=lopdf::content::Content::decode(&pdf.get_page_content(id)).unwrap();(content.operations.len(),content.operations.iter().filter(|op|op.operator=="f").count())}
fn capture(path:&Path){if let Some(root)=std::env::var_os("FOLIO_HIGHLIGHT_ACCEPTANCE_DIR"){let root=PathBuf::from(root);std::fs::create_dir_all(&root).unwrap();let target=root.join(path.file_name().unwrap());assert!(!target.exists(),"Preserve prior acceptance outputs");std::fs::copy(path,target).unwrap();}}

#[test]
fn italic_cell_highlights_wrap_reformat_and_replay_without_growing(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("cell-source.pdf");fixture(&input,0,false);
 let mut pdf=Document::load(&input).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
 let font=pdf.get_dictionary(page).unwrap().get(b"Resources").unwrap().as_dict().unwrap().get(b"Font").unwrap().as_dict().unwrap().get(b"F1").unwrap().as_reference().unwrap();
 pdf.get_dictionary_mut(font).unwrap().set("BaseFont",Object::Name(b"Helvetica-Oblique".to_vec()));
 let stream=pdf.add_object(Stream::new(dictionary!{},b"q 1 1 0 rg 50 499 34.668 11 re f Q BT /F1 12 Tf 50 500 Td (review) Tj ET BT /F1 12 Tf 50 100 Td (Neighbor) Tj ET".to_vec()));
 pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);pdf.save(&input).unwrap();capture(&input);
 let original=std::fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();let s=e.inspect_styled_paragraph(&source.id,0,&[1]).unwrap();
 assert_eq!(s.styles[0].background_color.as_deref(),Some("#FFFF00"));
 let mut request=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:s.object_indices,expected_text:s.text,runs:vec![StyledParagraphRun{text:"review ".into(),style_id:0},StyledParagraphRun{text:"review review review".into(),style_id:1}],style_changes:vec![],bounds:s.bounds,alignment:ParagraphAlignment::Left,line_spacing:1.3};
 request.style_changes=vec![folio_engine::StyledParagraphStyleChange{style_id:1,source_style_id:0,font_size:Some(20.),color:Some("#0033A6".into()),font_id:None}];
 request.bounds=AnnotationRect{x:40.,y:60.,width:100.,height:160.};request.line_spacing=1.3;
 let mut edited=e.preview_styled_paragraph(&source.id,0,&request).unwrap();let pixels=e.render_page(&edited.id,0,900).unwrap();let mut previous=None;
 for iteration in 0..3{
  let output=temp.path().join(format!("cell-saved-{iteration}.pdf"));save(&e,&edited,&output);capture(&output);
  let bytes=std::fs::read(&output).unwrap();let pdf=Document::load_mem(&bytes).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
  let memory:serde_json::Value=serde_json::from_slice(pdf.get_dictionary(page).unwrap().get(b"FolioStyledParagraphs").unwrap().as_str().unwrap()).unwrap();
  assert_eq!(memory["version"],8);assert!(memory["paragraphs"][0]["backgrounds"][0]["cell"].is_object());
  if iteration==0{
   // Spacing can be inferred from native baselines without any memory. Compare
   // to a metadata-free control, rather than treating Some(spacing) as replay.
   let mut control=pdf.clone();control.get_dictionary_mut(page).unwrap().remove(b"FolioStyledParagraphs");
   let control_path=temp.path().join("cell-no-memory.pdf");control.save(&control_path).unwrap();let clean=e.open_document(&control_path).unwrap();
   let clean_selection=selected(&e,&clean.id);assert_ne!(clean_selection.bounds,request.bounds);
   let expected=serde_json::to_value(clean_selection).unwrap();e.close_document(&clean.id).unwrap();
   for case in 0..4{
   let mut forged=pdf.clone();let mut value=memory.clone();
   match case{
    0=>value["paragraphs"][0]["backgrounds"][0]["cell"]["offsets"][0]=json!(0.004),
    1=>value["paragraphs"][0]["backgrounds"][0]["cell"]["offsets"][1]=json!(-0.004),
    2=>value["version"]=json!(7),
    _=>value["paragraphs"][0]["backgrounds"][0]["padding"]=json!([1.,0.,0.,0.]),
   }
   forged.get_dictionary_mut(page).unwrap().set("FolioStyledParagraphs",Object::string_literal(serde_json::to_vec(&value).unwrap()));
   let path=temp.path().join(format!("cell-forged-{case}.pdf"));forged.save(&path).unwrap();let bad=e.open_document(&path).unwrap();let before=e.source_bytes(&bad.id).unwrap();
   let ids=e.list_text_runs(&bad.id,0).unwrap().runs.into_iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect::<Vec<_>>();
   let inspected=e.inspect_styled_paragraph(&bad.id,0,&ids).unwrap();
   assert_ne!(inspected.bounds,request.bounds,"forged cell policy restored the remembered frame: {case}");
   assert_eq!(serde_json::to_value(inspected).unwrap(),expected,"forged cell policy differs from metadata-free inspection: {case}");
   assert_eq!(e.source_bytes(&bad.id).unwrap(),before);e.close_document(&bad.id).unwrap();
  }}
  e.close_document(&edited.id).unwrap();let reopened=e.open_document(&output).unwrap();let s=selected(&e,&reopened.id);
  assert_eq!(s.text,"review review review review");assert_eq!(s.line_spacing,Some(1.3));assert_eq!(s.bounds,request.bounds);
  assert!(s.styles.iter().all(|s|s.background_color.as_deref()==Some("#FFFF00")));
  edited=e.preview_styled_paragraph(&reopened.id,0,&again(&s)).unwrap();assert_eq!(e.render_page(&edited.id,0,900).unwrap(),pixels);
  let actual=counts(&e.source_bytes(&edited.id).unwrap());assert!(actual.1>=2);if let Some(before)=previous{assert_eq!(actual,before);}previous=Some(actual);
  e.close_document(&reopened.id).unwrap();
 }
 assert_eq!(std::fs::read(input).unwrap(),original);e.close_document(&source.id).unwrap();e.close_document(&edited.id).unwrap();
}

#[test]
fn highlighted_wraps_and_blank_lines_save_reopen_and_reedit_without_accumulating_marks(){
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for (rotation,b) in [(0,[40.,60.,50.,200.]),(90,[300.,40.,200.,50.]),(180,[390.,300.,50.,200.]),(270,[60.,390.,200.,50.])]{
  let input=temp.path().join(format!("source-{rotation}.pdf"));fixture(&input,rotation,false);let original=std::fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();let s=e.inspect_styled_paragraph(&source.id,0,&[1]).unwrap();
  capture(&input);
  let request=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:vec![1],expected_text:s.text,runs:vec![StyledParagraphRun{text:"AA BB CC DD\n\nEE FF".into(),style_id:0}],style_changes:vec![],bounds:AnnotationRect{x:b[0],y:b[1],width:b[2],height:b[3]},alignment:ParagraphAlignment::Justify,line_spacing:1.5};
  let mut edited=e.preview_styled_paragraph(&source.id,0,&request).unwrap();let pixels=e.render_page(&edited.id,0,900).unwrap();let mut previous=None;
  for iteration in 0..3 {
   let output=temp.path().join(format!("edited-{rotation}-{iteration}.pdf"));save(&e,&edited,&output);capture(&output);e.close_document(&edited.id).unwrap();let reopened=e.open_document(&output).unwrap();let s=selected(&e,&reopened.id);
   assert_eq!(s.text,"AA BB CC DD\n\nEE FF");assert_eq!(s.bounds,request.bounds);assert_eq!(s.alignment,Some(ParagraphAlignment::Justify));assert_eq!(s.line_spacing,Some(1.5));
   assert!(s.styles.iter().all(|style|style.background_color.as_deref()==Some("#FFFF00")));
   edited=e.preview_styled_paragraph(&reopened.id,0,&again(&s)).unwrap();assert_eq!(e.render_page(&edited.id,0,900).unwrap(),pixels);let actual=counts(&e.source_bytes(&edited.id).unwrap());assert_eq!(actual.1,3,"One rectangle per visible wrapped/hard line; none for the blank row");if let Some(expected)=previous{assert_eq!(actual,expected,"Saved highlight/text blocks accumulated");}previous=Some(actual);e.close_document(&reopened.id).unwrap();
  }
  assert_eq!(std::fs::read(&input).unwrap(),original);e.close_document(&source.id).unwrap();e.close_document(&edited.id).unwrap();
 }
}

#[test]
fn deleting_the_last_highlighted_word_removes_its_mark_and_keeps_plain_text_reeditable(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("partial.pdf");fixture(&input,0,true);let source=e.open_document(&input).unwrap();let s=e.inspect_styled_paragraph(&source.id,0,&[1]).unwrap();assert_eq!(s.styles.len(),2);
 let request=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:vec![1],expected_text:s.text,runs:vec![StyledParagraphRun{text:"AA CC".into(),style_id:0}],style_changes:vec![],bounds:AnnotationRect{x:40.,y:60.,width:100.,height:100.},alignment:ParagraphAlignment::Left,line_spacing:1.2};
 let edited=e.preview_styled_paragraph(&source.id,0,&request).unwrap();assert_eq!(counts(&e.source_bytes(&edited.id).unwrap()).1,0);
 let pdf=Document::load_mem(&e.source_bytes(&edited.id).unwrap()).unwrap();let page=*pdf.get_pages().get(&1).unwrap();let metadata:serde_json::Value=serde_json::from_slice(pdf.get_dictionary(page).unwrap().get(b"FolioStyledParagraphs").unwrap().as_str().unwrap()).unwrap();assert_eq!(metadata["version"],5);assert!(metadata["paragraphs"][0].get("backgrounds").is_none());assert!(metadata["paragraphs"][0]["text_insets"][0].as_f64().unwrap()>0.);
 let output=temp.path().join("plain.pdf");save(&e,&edited,&output);let reopened=e.open_document(&output).unwrap();let s=selected(&e,&reopened.id);assert_eq!(s.text,"AA CC");let replay=e.preview_styled_paragraph(&reopened.id,0,&again(&s)).unwrap();assert_eq!(counts(&e.source_bytes(&replay.id).unwrap()).1,0);assert_eq!(e.render_page(&replay.id,0,900).unwrap(),e.render_page(&edited.id,0,900).unwrap());
 for id in [&source.id,&edited.id,&reopened.id,&replay.id]{e.close_document(id).unwrap();}
}

#[test]
fn altered_background_hints_and_paint_blocks_cannot_restore_a_saved_draft(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("input.pdf");fixture(&input,0,false);let source=e.open_document(&input).unwrap();
 let request=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:vec![1],expected_text:"AA BB".into(),runs:vec![StyledParagraphRun{text:"AA BB CC DD\n\nEE FF".into(),style_id:0}],style_changes:vec![],bounds:AnnotationRect{x:40.,y:60.,width:50.,height:200.},alignment:ParagraphAlignment::Justify,line_spacing:1.5};
 let edited=e.preview_styled_paragraph(&source.id,0,&request).unwrap();let bytes=e.source_bytes(&edited.id).unwrap();let original=Document::load_mem(&bytes).unwrap();let page=*original.get_pages().get(&1).unwrap();let metadata:serde_json::Value=serde_json::from_slice(original.get_dictionary(page).unwrap().get(b"FolioStyledParagraphs").unwrap().as_str().unwrap()).unwrap();assert_eq!(metadata["version"],5);assert_eq!(metadata["paragraphs"][0]["text_alignment"],true);
 for case in 0..15 {
  let mut pdf=original.clone();let mut value=metadata.clone();
  match case {
   0=>value["paragraphs"][0]["backgrounds"][0]["padding"][0]=json!(value["paragraphs"][0]["backgrounds"][0]["padding"][0].as_f64().unwrap()+0.015),
   1=>value["paragraphs"][0]["backgrounds"][0]["color"]=json!([0,255,0]),
   2=>{let extra=value["paragraphs"][0]["backgrounds"][0].clone();value["paragraphs"][0]["backgrounds"].as_array_mut().unwrap().push(extra);},
   3=>value["paragraphs"][0]["styles"][0]["background_id"]=json!(63),
   4=>value["version"]=json!(1),
   8=>value["paragraphs"][0]["text_alignment"]=json!(false),
   9=>value["version"]=json!(3),
   10=>value["paragraphs"][0]["text_insets"][0]=json!(value["paragraphs"][0]["text_insets"][0].as_f64().unwrap()+0.015),
   11=>value["paragraphs"][0]["text_insets"]=json!([-1,0]),
   13=>value["version"]=json!(4),
   14=>value["paragraphs"][0]["ink_vertical"]=json!("invalid"),
   12=>value["paragraphs"][0]["text_insets"]=json!([100001,0]),
   _=>{
    let mut content=lopdf::content::Content::decode(&pdf.get_page_content(page)).unwrap();let paint=content.operations.iter().position(|op|op.operator=="f").unwrap();
    if case==5{content.operations[paint]=lopdf::content::Operation::new("n",vec![]);}
    else if case==6{let copy=content.operations[paint-3..paint+2].to_vec();content.operations.splice(paint-3..paint-3,copy);}
    else{let block=content.operations.drain(paint-3..paint+2).collect::<Vec<_>>();content.operations.extend(block);}
    let stream=pdf.add_object(Stream::new(dictionary!{},content.encode().unwrap()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);
   }
  }
  pdf.get_dictionary_mut(page).unwrap().set("FolioStyledParagraphs",Object::string_literal(serde_json::to_vec(&value).unwrap()));let path=temp.path().join(format!("forged-{case}.pdf"));pdf.save(&path).unwrap();let bad=e.open_document(&path).unwrap();let before=e.source_bytes(&bad.id).unwrap();let ids=e.list_text_runs(&bad.id,0).unwrap().runs.into_iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect::<Vec<_>>();
  if let Ok(s)=e.inspect_styled_paragraph(&bad.id,0,&ids){assert_eq!(s.line_spacing,None,"forged case {case} restored saved settings");}
  assert_eq!(e.source_bytes(&bad.id).unwrap(),before);e.close_document(&bad.id).unwrap();
 }
 e.close_document(&source.id).unwrap();e.close_document(&edited.id).unwrap();
}

#[test]
fn mixed_substitute_fonts_and_space_only_fragments_keep_their_highlight_after_reopen(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("mixed-source.pdf");fixture(&input,0,true);let source=e.open_document(&input).unwrap();let font=e.fonts().register(std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
 let request=serde_json::from_value(json!({"objectIndices":[1],"expectedText":"AA BB CC","runs":[{"text":"AA  ","styleId":0},{"text":"Café word","styleId":2},{"text":" ","styleId":3},{"text":"again","styleId":1},{"text":"  CC","styleId":0}],"styleChanges":[{"styleId":2,"sourceStyleId":1,"fontId":font.id,"fontSize":18,"color":"#993366"},{"styleId":3,"sourceStyleId":1,"fontSize":24}],"bounds":{"x":40,"y":60,"width":300,"height":200},"alignment":"left","lineSpacing":1.2})).unwrap();
 let edited=e.preview_styled_paragraph(&source.id,0,&request).unwrap();e.fonts().remove(&font.id).unwrap();let output=temp.path().join("mixed-highlight.pdf");save(&e,&edited,&output);capture(&input);capture(&output);let reopened=e.open_document(&output).unwrap();let s=selected(&e,&reopened.id);assert_eq!(s.text,"AA  Café word again  CC");let replay=e.preview_styled_paragraph(&reopened.id,0,&again(&s)).unwrap();let replay_path=temp.path().join("mixed-highlight-replay.pdf");save(&e,&replay,&replay_path);capture(&replay_path);
 assert_eq!(counts(&e.source_bytes(&replay.id).unwrap()).1,1);assert_eq!(e.render_page(&replay.id,0,900).unwrap(),e.render_page(&edited.id,0,900).unwrap());
 for id in [&source.id,&edited.id,&reopened.id,&replay.id]{e.close_document(id).unwrap();}
}

#[test]
fn owned_highlights_do_not_exempt_other_artwork_from_collision_checks(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("obstacle.pdf");fixture(&input,0,false);let mut pdf=Document::load(&input).unwrap();let page=*pdf.get_pages().get(&1).unwrap();let mut data=pdf.get_page_content(page);data.extend_from_slice(b" q 0 .5 .5 rg 150 350 20 20 re f Q");let stream=pdf.add_object(Stream::new(dictionary!{},data));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);pdf.save(&input).unwrap();let source=e.open_document(&input).unwrap();let before=e.source_bytes(&source.id).unwrap();
 let request=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:vec![1],expected_text:"AA BB".into(),runs:vec![StyledParagraphRun{text:"AA BB".into(),style_id:0}],style_changes:vec![],bounds:AnnotationRect{x:40.,y:60.,width:200.,height:200.},alignment:ParagraphAlignment::Left,line_spacing:1.2};
 assert!(e.preview_styled_paragraph(&source.id,0,&request).unwrap_err().to_string().contains("graphics"));assert_eq!(e.source_bytes(&source.id).unwrap(),before);e.close_document(&source.id).unwrap();
}
