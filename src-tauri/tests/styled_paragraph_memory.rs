use folio_engine::{PdfEngine,StyledParagraphLayoutRequest,StyledParagraphSelection,ExportRequest,PagePlan};
use lopdf::{dictionary,Document,Object,Stream};
use serde_json::{json,Value};
use std::path::{Path,PathBuf};
fn engine()->PdfEngine{PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture(path:&Path){
 let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();let f=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});let g=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica-Bold"});
 let stream=pdf.add_object(Stream::new(dictionary!{},b"BT /F1 12 Tf 50 700 Td (One ) Tj ET BT /F2 20 Tf 95 700 Td (Two) Tj ET BT /F1 12 Tf 50 60 Td (Neighbor) Tj ET".to_vec()));let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),600.into(),800.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>f,"F2"=>g}},"Contents"=>stream});pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",root);pdf.save(path).unwrap();
}
fn draft()->StyledParagraphLayoutRequest{serde_json::from_value(json!({"objectIndices":[0,1],"expectedText":"One Two","runs":[{"text":"pre","styleId":0},{"text":"fix ","styleId":1},{"text":"word alpha beta gamma delta\n","styleId":0},{"text":"\n","styleId":2},{"text":"last  \n","styleId":1}],"styleChanges":[{"styleId":2,"sourceStyleId":0,"fontSize":30,"color":"#993366"}],"bounds":{"x":50,"y":80,"width":160,"height":350},"alignment":"justify","lineSpacing":1.6})).unwrap()}
fn selected(e:&PdfEngine,id:&str)->StyledParagraphSelection{let indices=e.list_text_runs(id,0).unwrap().runs.iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect::<Vec<_>>();e.inspect_styled_paragraph(id,0,&indices).unwrap()}
fn again(s:&StyledParagraphSelection)->StyledParagraphLayoutRequest{serde_json::from_value(json!({"objectIndices":s.object_indices,"expectedText":s.text,"runs":s.runs,"bounds":s.bounds,"alignment":serde_json::to_value(s).unwrap()["alignment"],"lineSpacing":serde_json::to_value(s).unwrap()["lineSpacing"]})).unwrap()}
fn save(e:&PdfEngine,doc:&folio_engine::DocumentInfo,path:&Path){e.export_pdf(ExportRequest{flatten:false,pages:vec![PagePlan{crop: None,id:"p".into(),source_id:doc.id.clone(),page_index:0,width:doc.pages[0].width,height:doc.pages[0].height,rotation:0,overlays:vec![]}]},path).unwrap();}
#[test]
fn hard_soft_breaks_blank_styles_and_noop_pixels_survive_repeated_save_and_reedit(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("input.pdf");fixture(&input);let original=std::fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();let request=draft();let text=request.runs.iter().map(|r|r.text.as_str()).collect::<String>();let mut edited=e.preview_styled_paragraph(&source.id,0,&request).unwrap();let pixels=e.render_page(&edited.id,0,900).unwrap();
 let mut operation_count=None;
 for n in 0..3{
  let path=temp.path().join(format!("saved{n}.pdf"));save(&e,&edited,&path);e.close_document(&edited.id).unwrap();let saved=e.open_document(&path).unwrap();let s=selected(&e,&saved.id);assert_eq!(s.text,text);assert_eq!(s.bounds,request.bounds);let v=serde_json::to_value(&s).unwrap();assert_eq!(v["alignment"],"justify");assert_eq!(s.line_spacing,Some(1.6));assert!(s.styles.iter().any(|style|style.font_size==30.&&style.color=="#993366"));
  edited=e.preview_styled_paragraph(&saved.id,0,&again(&s)).unwrap();assert_eq!(e.render_page(&edited.id,0,900).unwrap(),pixels);e.close_document(&saved.id).unwrap();
  let pdf=Document::load_mem(&e.source_bytes(&edited.id).unwrap()).unwrap();let page=*pdf.get_pages().get(&1).unwrap();let count=lopdf::content::Content::decode(&pdf.get_page_content(page)).unwrap().operations.len();if let Some(previous)=operation_count{assert_eq!(count,previous,"Obsolete fragment blocks accumulated");}operation_count=Some(count);
 }
 assert_eq!(std::fs::read(&input).unwrap(),original);e.close_document(&source.id).unwrap();e.close_document(&edited.id).unwrap();
}
#[test]
fn a_font_used_only_for_blank_lines_and_a_space_only_fragment_is_restored(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("input.pdf");fixture(&input);let source=e.open_document(&input).unwrap();let mut request=draft();request.runs=serde_json::from_value(json!([{"text":"alpha ","styleId":0},{"text":" ","styleId":2},{"text":"beta\n","styleId":0},{"text":"\n","styleId":1},{"text":"last\n","styleId":0}])).unwrap();let edited=e.preview_styled_paragraph(&source.id,0,&request).unwrap();let s=selected(&e,&edited.id);assert_eq!(s.text,"alpha  beta\n\nlast\n");assert!(s.styles.iter().any(|style|style.font_name=="Helvetica-Bold"));let noop=e.preview_styled_paragraph(&edited.id,0,&again(&s)).unwrap();assert_eq!(e.render_page(&edited.id,0,800).unwrap(),e.render_page(&noop.id,0,800).unwrap());for id in [&source.id,&edited.id,&noop.id]{e.close_document(id).unwrap();}
}
#[test]
fn more_than_64_fragments_remain_reeditable_only_with_verified_memory(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("input.pdf");fixture(&input);let source=e.open_document(&input).unwrap();let mut value=serde_json::to_value(draft()).unwrap();value["runs"]=Value::Array((0..200).map(|i|json!({"text":"a ","styleId":i%2})).collect());value["styleChanges"]=json!([{"styleId":0,"sourceStyleId":0,"fontSize":8},{"styleId":1,"sourceStyleId":1,"fontSize":8}]);value["bounds"]=json!({"x":50,"y":80,"width":400,"height":600});let request=serde_json::from_value(value).unwrap();let edited=e.preview_styled_paragraph(&source.id,0,&request).unwrap();let s=selected(&e,&edited.id);assert!(s.object_indices.len()>64);let noop=e.preview_styled_paragraph(&edited.id,0,&again(&s)).unwrap();assert_eq!(e.render_page(&edited.id,0,800).unwrap(),e.render_page(&noop.id,0,800).unwrap());
 let bytes=e.source_bytes(&edited.id).unwrap();let mut pdf=Document::load_mem(&bytes).unwrap();let page=*pdf.get_pages().get(&1).unwrap();pdf.get_dictionary_mut(page).unwrap().remove(b"FolioStyledParagraphs");let path=temp.path().join("without.pdf");pdf.save(&path).unwrap();let without=e.open_document(&path).unwrap();assert!(e.inspect_styled_paragraph(&without.id,0,&s.object_indices).is_err());for id in [&source.id,&edited.id,&noop.id,&without.id]{e.close_document(id).unwrap();}
}

#[test]
fn malformed_overlapping_and_forged_layout_hints_do_not_restore(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("input.pdf");fixture(&input);let source=e.open_document(&input).unwrap();let mut request=draft();request.runs=serde_json::from_value(json!([{"text":"alpha ","styleId":0},{"text":"beta","styleId":1}])).unwrap();request.alignment=folio_engine::ParagraphAlignment::Right;
 let edited=e.preview_styled_paragraph(&source.id,0,&request).unwrap();let selected=selected(&e,&edited.id);let bytes=e.source_bytes(&edited.id).unwrap();let original=Document::load_mem(&bytes).unwrap();let page=*original.get_pages().get(&1).unwrap();let metadata:Value=serde_json::from_slice(original.get_dictionary(page).unwrap().get(b"FolioStyledParagraphs").unwrap().as_str().unwrap()).unwrap();
 for case in 0..12{
  let mut pdf=original.clone();let mut value=metadata.clone();match case{
   0=>value["version"]=json!(99),
   1=>{let duplicate=value["paragraphs"][0].clone();value["paragraphs"].as_array_mut().unwrap().push(duplicate);},
   2=>value["paragraphs"][0]["runs"][0]["text"]=json!("forged "),
   3=>value["paragraphs"][0]["styles"][0]["resource"]=json!([70,50]),
   4=>{let b=value["paragraphs"][0]["bounds"].as_array_mut().unwrap();b[0]=json!(b[0].as_f64().unwrap()+0.015);b[2]=json!(b[2].as_f64().unwrap()+0.015);},
   5=>{let mut content=lopdf::content::Content::decode(&pdf.get_page_content(page)).unwrap();for op in &mut content.operations{if op.operator=="Tc"{op.operands=vec![Object::Real(0.01)];}}let stream=pdf.add_object(Stream::new(dictionary!{},content.encode().unwrap()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);},
   6=>{value["paragraphs"][0]["fragments"][0]["digest"]=json!("0".repeat(64));},
   7=>{let mut partial=value["paragraphs"][0].clone();partial["fragments"].as_array_mut().unwrap().truncate(1);value["paragraphs"].as_array_mut().unwrap().push(partial);},
   8=>{let duplicate=value["paragraphs"][0].clone();value["paragraphs"].as_array_mut().unwrap().insert(0,duplicate);},
   9=>{let mut content=lopdf::content::Content::decode(&pdf.get_page_content(page)).unwrap();for op in &mut content.operations{if op.operator=="Tw"{op.operands=vec![Object::Real(0.01)];}}let stream=pdf.add_object(Stream::new(dictionary!{},content.encode().unwrap()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);},
   _=>{}
  }
  let mut data=serde_json::to_vec(&value).unwrap();if case>=10{data.resize(4*1024*1024+usize::from(case==10),b' ');}
  pdf.get_dictionary_mut(page).unwrap().set("FolioStyledParagraphs",Object::String(data,lopdf::StringFormat::Hexadecimal));let path=temp.path().join(format!("tampered{case}.pdf"));pdf.save(&path).unwrap();let changed=e.open_document(&path).unwrap();let result=e.inspect_styled_paragraph(&changed.id,0,&selected.object_indices);
  if case==11{assert_eq!(result.unwrap().alignment,Some(folio_engine::ParagraphAlignment::Right));}
  else if let Ok(selection)=result{assert_eq!(selection.alignment,None,"case {case} accepted forged hints");}e.close_document(&changed.id).unwrap();
 }
 assert_eq!(e.source_bytes(&edited.id).unwrap(),bytes);for id in [&source.id,&edited.id]{e.close_document(id).unwrap();}
}

#[test]
fn editing_another_paragraph_keeps_the_first_saved_draft(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("input.pdf");fixture(&input);let source=e.open_document(&input).unwrap();let request=draft();let first=e.preview_styled_paragraph(&source.id,0,&request).unwrap();let before=selected(&e,&first.id);
 let neighbor=e.list_text_runs(&first.id,0).unwrap().runs.into_iter().find(|r|r.text=="Neighbor").unwrap();let second:StyledParagraphLayoutRequest=serde_json::from_value(json!({"objectIndices":[neighbor.object_index],"expectedText":"Neighbor","runs":[{"text":"Second paragraph","styleId":0}],"bounds":{"x":50,"y":600,"width":200,"height":80},"alignment":"left","lineSpacing":1.2})).unwrap();let edited=e.preview_styled_paragraph(&first.id,0,&second).unwrap();let indices=e.list_text_runs(&edited.id,0).unwrap().runs.into_iter().filter(|r|r.text!="Second paragraph").map(|r|r.object_index).collect::<Vec<_>>();let restored=e.inspect_styled_paragraph(&edited.id,0,&indices).unwrap();assert_eq!(restored.text,before.text);assert_eq!(restored.runs,before.runs);assert_eq!(restored.bounds,before.bounds);let noop=e.preview_styled_paragraph(&edited.id,0,&again(&restored)).unwrap();assert_eq!(e.render_page(&edited.id,0,800).unwrap(),e.render_page(&noop.id,0,800).unwrap());for id in [&source.id,&first.id,&edited.id,&noop.id]{e.close_document(id).unwrap();}
}

#[test]
fn glyph_owner_key_collisions_preserve_distinct_overlapping_neighbors(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("input.pdf");fixture(&input);let mut pdf=Document::load(&input).unwrap();let page=*pdf.get_pages().get(&1).unwrap();let mut data=pdf.get_page_content(page);data.extend_from_slice(b" BT /F1 12 Tf 1 0 0 rg 50 60 Td (Overlay) Tj ET");let stream=pdf.add_object(Stream::new(dictionary!{},data));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);pdf.save(&input).unwrap();let source=e.open_document(&input).unwrap();let neighbors=e.list_text_runs(&source.id,0).unwrap().runs.into_iter().skip(2).collect::<Vec<_>>();assert_eq!(neighbors.len(),2);let edited=e.preview_styled_paragraph(&source.id,0,&draft()).unwrap();let runs=e.list_text_runs(&edited.id,0).unwrap().runs;for before in &neighbors{let matches=runs.iter().filter(|r|r.text==before.text).collect::<Vec<_>>();assert_eq!(matches.len(),1);let after=matches[0];assert_eq!(before.font_name,after.font_name);assert_eq!(before.bounds,after.bounds);}
 assert_eq!(runs[runs.len()-2..].iter().map(|r|r.text.as_str()).collect::<Vec<_>>(),neighbors.iter().map(|r|r.text.as_str()).collect::<Vec<_>>());
 let indices=runs.iter().filter(|r|!neighbors.iter().any(|n|n.text==r.text)).map(|r|r.object_index).collect::<Vec<_>>();let selected=e.inspect_styled_paragraph(&edited.id,0,&indices).unwrap();let noop=e.preview_styled_paragraph(&edited.id,0,&again(&selected)).unwrap();assert_eq!(e.render_page(&edited.id,0,900).unwrap(),e.render_page(&noop.id,0,900).unwrap());for id in [&source.id,&edited.id,&noop.id]{e.close_document(id).unwrap();}
}

#[test]
fn imported_font_and_newline_only_variant_restore_after_registry_release(){
 let e=engine();let font=e.fonts().register(std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("input.pdf");fixture(&input);let source=e.open_document(&input).unwrap();let mut value=serde_json::to_value(draft()).unwrap();
 value["styleChanges"]=json!([{"styleId":0,"sourceStyleId":0,"fontId":font.id,"fontSize":17},{"styleId":2,"sourceStyleId":1,"fontId":font.id,"fontSize":30,"color":"#993366"}]);let request:StyledParagraphLayoutRequest=serde_json::from_value(value).unwrap();let edited=e.preview_styled_paragraph(&source.id,0,&request).unwrap();e.fonts().remove(&font.id).unwrap();e.close_document(&source.id).unwrap();let path=temp.path().join("saved.pdf");save(&e,&edited,&path);let saved=e.open_document(&path).unwrap();let s=selected(&e,&saved.id);assert_eq!(s.text,request.runs.iter().map(|r|r.text.as_str()).collect::<String>());let noop=e.preview_styled_paragraph(&saved.id,0,&again(&s)).unwrap();assert_eq!(e.render_page(&edited.id,0,900).unwrap(),e.render_page(&noop.id,0,900).unwrap());for id in [&edited.id,&saved.id,&noop.id]{e.close_document(id).unwrap();}
}

#[test]
fn subpixel_font_size_and_linear_transform_changes_do_not_restore(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("input.pdf");fixture(&input);let source=e.open_document(&input).unwrap();let edited=e.preview_styled_paragraph(&source.id,0,&draft()).unwrap();let selection=selected(&e,&edited.id);let original=Document::load_mem(&e.source_bytes(&edited.id).unwrap()).unwrap();let page=*original.get_pages().get(&1).unwrap();
 for kind in ["size","matrix"]{let mut pdf=original.clone();let mut content=lopdf::content::Content::decode(&pdf.get_page_content(page)).unwrap();let tm=content.operations.iter().position(|op|op.operator=="Tm").unwrap();
  if kind=="matrix"{content.operations[tm].operands[0]=Object::Real(1.000001);content.operations[tm].operands[3]=Object::Real(1.000001);}
  else{let tf=content.operations[..tm].iter().rposition(|op|op.operator=="Tf").unwrap();content.operations[tf].operands[1]=Object::Real(12.000001);}
  let stream=pdf.add_object(Stream::new(dictionary!{},content.encode().unwrap()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);let path=temp.path().join(format!("{kind}.pdf"));pdf.save(&path).unwrap();let modified=e.open_document(&path).unwrap();if let Ok(s)=e.inspect_styled_paragraph(&modified.id,0,&selection.object_indices){assert_eq!(s.alignment,None,"accepted modified {kind}");}e.close_document(&modified.id).unwrap();
 }
 for id in [&source.id,&edited.id]{e.close_document(id).unwrap();}
}

#[test]
fn invalid_overlapping_hints_elsewhere_do_not_exhaust_the_new_edit_budget(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("input.pdf");fixture(&input);let source=e.open_document(&input).unwrap();let first=e.preview_styled_paragraph(&source.id,0,&draft()).unwrap();let mut pdf=Document::load_mem(&e.source_bytes(&first.id).unwrap()).unwrap();let page=*pdf.get_pages().get(&1).unwrap();let mut hints:Value=serde_json::from_slice(pdf.get_dictionary(page).unwrap().get(b"FolioStyledParagraphs").unwrap().as_str().unwrap()).unwrap();let record=hints["paragraphs"][0].clone();hints["paragraphs"]=Value::Array(vec![record;64]);pdf.get_dictionary_mut(page).unwrap().set("FolioStyledParagraphs",Object::String(serde_json::to_vec(&hints).unwrap(),lopdf::StringFormat::Hexadecimal));let path=temp.path().join("ambiguous.pdf");pdf.save(&path).unwrap();let ambiguous=e.open_document(&path).unwrap();let bytes=e.source_bytes(&ambiguous.id).unwrap();let neighbor=e.list_text_runs(&ambiguous.id,0).unwrap().runs.into_iter().find(|r|r.text=="Neighbor").unwrap();
 let request:StyledParagraphLayoutRequest=serde_json::from_value(json!({"objectIndices":[neighbor.object_index],"expectedText":"Neighbor","runs":[{"text":"Second paragraph","styleId":0}],"bounds":{"x":50,"y":600,"width":200,"height":80},"alignment":"left","lineSpacing":1.2})).unwrap();let edited=e.preview_styled_paragraph(&ambiguous.id,0,&request).unwrap();let output=Document::load_mem(&e.source_bytes(&edited.id).unwrap()).unwrap();let page=*output.get_pages().get(&1).unwrap();let hints:Value=serde_json::from_slice(output.get_dictionary(page).unwrap().get(b"FolioStyledParagraphs").unwrap().as_str().unwrap()).unwrap();assert_eq!(hints["paragraphs"].as_array().unwrap().len(),1);assert_eq!(e.source_bytes(&ambiguous.id).unwrap(),bytes);for id in [&source.id,&first.id,&ambiguous.id,&edited.id]{e.close_document(id).unwrap();}
}

#[test]
fn only_replayable_unique_hints_count_toward_the_record_budget(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("input.pdf");fixture(&input);let source=e.open_document(&input).unwrap();
 let mut request:StyledParagraphLayoutRequest=serde_json::from_value(json!({"objectIndices":[0,1],"expectedText":"One Two","runs":[{"text":"A","styleId":0}],"bounds":{"x":50,"y":80,"width":100,"height":20},"alignment":"left","lineSpacing":1.2})).unwrap();
 let first=e.preview_styled_paragraph(&source.id,0,&request).unwrap();let mut pdf=Document::load_mem(&e.source_bytes(&first.id).unwrap()).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
 let mut hints:Value=serde_json::from_slice(pdf.get_dictionary(page).unwrap().get(b"FolioStyledParagraphs").unwrap().as_str().unwrap()).unwrap();let record=hints["paragraphs"][0].clone();
 let mut content=lopdf::content::Content::decode(&pdf.get_page_content(page)).unwrap();let starts=content.operations.windows(13).enumerate().filter(|(_,ops)|ops.iter().zip(["q","BT","Tf","Tc","Tw","Tz","Ts","Tr","rg","Tm","Tj","ET","Q"]).all(|(op,name)|op.operator==name)&&ops[10].operands[0].as_str().ok()==Some(b"A".as_slice())).map(|(i,_)|i).collect::<Vec<_>>();assert_eq!(starts.len(),1);let slot=starts[0];
 let block=content.operations.drain(slot..slot+13).collect::<Vec<_>>();let origin=record["fragments"][0]["origin"].as_array().unwrap();assert_eq!(block[9].operands[4].as_float().unwrap(),origin[0].as_f64().unwrap() as f32);assert_eq!(block[9].operands[5].as_float().unwrap(),origin[1].as_f64().unwrap() as f32);let mut expanded=vec![];let mut records=vec![];
 for index in 0..64{
  request.bounds.y=80.+index as f32*10.;let layout=e.layout_styled_paragraph(&source.id,0,&request).unwrap();let origin=layout.lines[0].runs[0].origin;
  let mut current=record.clone();current["bounds"]=json!(layout.pdf_bounds);current["fragments"][0]["origin"]=json!(origin);records.push(current);
  let mut ops=block.clone();ops[9].operands[4]=origin[0].into();ops[9].operands[5]=origin[1].into();expanded.extend(ops);
 }
 content.operations.splice(slot..slot,expanded);let stream=pdf.add_object(Stream::new(dictionary!{},content.encode().unwrap()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);
 for forged in [false,true]{
  hints["paragraphs"]=json!(records);if forged{hints["paragraphs"][31]["runs"][0]["text"]=json!("B");}
  pdf.get_dictionary_mut(page).unwrap().set("FolioStyledParagraphs",Object::String(serde_json::to_vec(&hints).unwrap(),lopdf::StringFormat::Hexadecimal));let path=temp.path().join(format!("capacity-{forged}.pdf"));pdf.save(&path).unwrap();let source=e.open_document(&path).unwrap();let original=e.source_bytes(&source.id).unwrap();let runs=e.list_text_runs(&source.id,0).unwrap().runs;
  // Prove the synthetic records really restore, not merely that they bind.
  assert_eq!(runs.len(),65);assert!(runs[..64].iter().all(|r|r.text=="A"));assert_eq!(runs[64].text,"Neighbor");
  for (index,run) in runs.iter().filter(|r|r.text=="A").enumerate(){
   let selection=e.inspect_styled_paragraph(&source.id,0,&[run.object_index]).unwrap();let bad=forged&&index==31;assert_eq!(selection.alignment.is_some(),!bad);
  }
  let neighbor=runs.iter().find(|r|r.text=="Neighbor").unwrap();let edit:StyledParagraphLayoutRequest=serde_json::from_value(json!({"objectIndices":[neighbor.object_index],"expectedText":"Neighbor","runs":[{"text":"X","styleId":0}],"bounds":{"x":400,"y":50,"width":150,"height":60},"alignment":"left","lineSpacing":1.2})).unwrap();
  let result=e.preview_styled_paragraph(&source.id,0,&edit);
  if forged{
   let edited=result.expect("An unreplayable hint must not block an unrelated edit");let output=Document::load_mem(&e.source_bytes(&edited.id).unwrap()).unwrap();let page=*output.get_pages().get(&1).unwrap();let hints:Value=serde_json::from_slice(output.get_dictionary(page).unwrap().get(b"FolioStyledParagraphs").unwrap().as_str().unwrap()).unwrap();assert_eq!(hints["paragraphs"].as_array().unwrap().len(),64);assert_eq!(e.list_text_runs(&edited.id,0).unwrap().runs.iter().filter(|r|r.text=="A").count(),64);e.close_document(&edited.id).unwrap();
  }else{assert!(result.unwrap_err().to_string().contains("fragment limit"));}
  assert_eq!(e.source_bytes(&source.id).unwrap(),original);assert_eq!(std::fs::read(&path).unwrap(),original.as_ref());e.close_document(&source.id).unwrap();
 }
 for id in [&source.id,&first.id]{e.close_document(id).unwrap();}
}
