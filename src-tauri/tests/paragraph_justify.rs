use folio_engine::{AnnotationRect,ParagraphFormatting,PdfEngine};
use lopdf::{dictionary,Document,Object,Stream};
use std::path::{Path,PathBuf};
#[path = "support/protected_fixture.rs"]
mod protected_fixture;
fn root()->PathBuf {PathBuf::from(env!("CARGO_MANIFEST_DIR"))}
fn engine()->PdfEngine {PdfEngine::start(root().join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture(path:&Path,rotation:i32){
 let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
 let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
 let content=pdf.add_object(Stream::new(dictionary!{},b"BT /F1 10 Tf 40 330 Td (Original words) Tj 0 -150 Td (Neighbor) Tj ET".to_vec()));
 let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Rotate"=>rotation,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});
 pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());let catalog=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",catalog);pdf.save(path).unwrap();
}
fn area(rotation:i32)->AnnotationRect {let (x,y,width,height)=match rotation {90=>(250.,40.,90.,100.),180=>(160.,250.,100.,90.),270=>(60.,160.,90.,100.),_=>(40.,60.,100.,90.)};AnnotationRect{x,y,width,height}}
fn formatting(font:Option<&str>)->ParagraphFormatting {serde_json::from_value(serde_json::json!({"alignment":"justify","fontSize":10,"lineSpacing":1.5,"fontId":font})).unwrap()}
fn alignment(e:&PdfEngine,id:&str,indices:&[usize])->String {serde_json::to_value(e.inspect_text_paragraph(id,0,indices).unwrap()).unwrap()["alignment"].as_str().unwrap().into()}
fn lines(bytes:&[u8])->Vec<(String,f32)> {
 let pdf=Document::load_mem(bytes).unwrap();let page=*pdf.get_pages().get(&1).unwrap();let content=lopdf::content::Content::decode(&pdf.get_page_content(page)).unwrap();
 content.operations.into_iter().filter_map(|op|{
  let values=match (op.operator.as_str(),op.operands.as_slice()){("Tj",[value])=>vec![value.clone()],("TJ",[Object::Array(values)])=>values.clone(),_=>return None};
  let mut text=String::new();let mut adjustment=0.;for v in values {if let Ok(s)=v.as_str(){text.push_str(std::str::from_utf8(s).unwrap());}else{adjustment+=v.as_float().unwrap();}}
  (!text.is_empty()).then_some((text,adjustment))
 }).collect()
}
#[test]
fn justifies_soft_wrapped_lines_and_reinfers_after_save_at_all_rotations(){
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for rotation in [0,90,180,270] {
  let path=temp.path().join("source.pdf");fixture(&path,rotation);let original=std::fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();let selected=e.inspect_text_paragraph(&source.id,0,&[0]).unwrap();
  let neighbor=e.list_text_runs(&source.id,0).unwrap().runs[1].clone();
  let edit=e.preview_text_paragraph_with_format(&source.id,0,&[0],&selected.text,"Alpha beta gamma delta epsilon zeta",&area(rotation),&formatting(None)).unwrap();
  let rows=lines(&e.source_bytes(&edit.id).unwrap());let changed=&rows[1..];assert_eq!(changed.len(),3);assert_eq!(changed[0].0,"Alpha beta gamma ");assert_eq!(changed[1].0,"delta epsilon ");assert_eq!(changed[2].0,"zeta");
  for (text,delta) in &changed[..2]{assert!(*delta<0.);assert!((text.trim_end().len() as f32*6.-delta*0.01-100.).abs()<0.02);}
  assert_eq!(changed[2].1,0.);
  let saved=temp.path().join("saved.pdf");e.export_pdf(folio_engine::ExportRequest{flatten:false,pages:vec![folio_engine::PagePlan{crop: None,id:"p".into(),source_id:edit.id.clone(),page_index:0,width:edit.pages[0].width,height:edit.pages[0].height,rotation:0,overlays:vec![]}]},&saved).unwrap();
  let reopened=e.open_document(&saved).unwrap();let runs=e.list_text_runs(&reopened.id,0).unwrap().runs;assert_eq!(runs.iter().find(|r|r.text=="Neighbor").unwrap().bounds,neighbor.bounds);
  let indices:Vec<_>=runs.iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect();assert_eq!(alignment(&e,&reopened.id,&indices),"justify");
  let selected=e.inspect_text_paragraph(&reopened.id,0,&indices).unwrap();let again=e.preview_text_paragraph(&reopened.id,0,&indices,&selected.text,"Alpha beta gamma delta epsilon end",&area(rotation)).unwrap();
  assert!(lines(&e.source_bytes(&again.id).unwrap()).iter().any(|(_,gap)|*gap<0.));assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),original.as_slice());
  for id in [&source.id,&edit.id,&reopened.id,&again.id]{e.close_document(id).unwrap();}
 }
}
#[test]
fn explicit_breaks_blank_lines_and_single_words_are_not_stretched(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("source.pdf");fixture(&path,0);let source=e.open_document(&path).unwrap();let selected=e.inspect_text_paragraph(&source.id,0,&[0]).unwrap();
 let edit=e.preview_text_paragraph_with_format(&source.id,0,&[0],&selected.text,"Alpha beta\n\nSingle\nBeta gamma delta",&area(0),&formatting(None)).unwrap();
 let rows=lines(&e.source_bytes(&edit.id).unwrap());assert_eq!(rows[1..].iter().map(|r|r.0.as_str()).collect::<Vec<_>>(),vec!["Alpha beta","Single","Beta gamma delta"]);assert!(rows.iter().all(|r|r.1==0.));
 for id in [&source.id,&edit.id]{e.close_document(id).unwrap();}
}
#[test]
fn repeated_and_trailing_spaces_and_substitute_unicode_remain_editable(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("source.pdf");fixture(&path,0);let source=e.open_document(&path).unwrap();let selected=e.inspect_text_paragraph(&source.id,0,&[0]).unwrap();
 let edit=e.preview_text_paragraph_with_format(&source.id,0,&[0],&selected.text,"Alpha  beta gamma delta  ",&area(0),&formatting(None)).unwrap();let rows=lines(&e.source_bytes(&edit.id).unwrap());assert!(rows[1].0.contains("  "));assert!(rows.last().unwrap().0.ends_with("  "));
 let indices:Vec<_>=e.list_text_runs(&edit.id,0).unwrap().runs.iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect();assert!(e.inspect_text_paragraph(&edit.id,0,&indices).unwrap().text.contains("Alpha  beta"));assert_eq!(alignment(&e,&edit.id,&indices),"justify");
 let font=e.fonts().register(std::fs::read(root().join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
 let sub=e.preview_text_paragraph_with_format(&source.id,0,&[0],&selected.text,"Cafè words again after more words finish",&area(0),&formatting(Some(&font.id))).unwrap();
 let runs=e.list_text_runs(&sub.id,0).unwrap().runs;let indices:Vec<_>=runs.iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect();let inspected=e.inspect_text_paragraph(&sub.id,0,&indices).unwrap();assert!(inspected.text.contains("Cafè"));assert_eq!(alignment(&e,&sub.id,&indices),"justify");
 for id in [&source.id,&edit.id,&sub.id]{e.close_document(id).unwrap();}
}
#[test]
fn leading_spaces_single_word_wraps_bearings_embedded_fonts_and_overflow(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("source.pdf");fixture(&path,0);let source=e.open_document(&path).unwrap();let selected=e.inspect_text_paragraph(&source.id,0,&[0]).unwrap();let original=e.source_bytes(&source.id).unwrap();
 for value in ["  Alpha beta gamma delta","XXXXXXXXXXXXXXXX delta"] {
  let edit=e.preview_text_paragraph_with_format(&source.id,0,&[0],&selected.text,value,&area(0),&formatting(None)).unwrap();let rows=lines(&e.source_bytes(&edit.id).unwrap());
  if value.starts_with(' '){assert!(rows[1].0.starts_with("  "));}else{assert_eq!(rows[1].1,0.);}
  e.close_document(&edit.id).unwrap();
 }
 let font=e.fonts().register(std::fs::read(root().join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
 let edit=e.preview_text_paragraph_with_format(&source.id,0,&[0],&selected.text,"jj beta gamma delta more words",&area(0),&formatting(Some(&font.id))).unwrap();e.close_document(&edit.id).unwrap();
 let too_short=AnnotationRect{height:10.,..area(0)};assert!(e.preview_text_paragraph_with_format(&source.id,0,&[0],&selected.text,"Alpha beta gamma delta",&too_short,&formatting(None)).unwrap_err().to_string().contains("fit"));assert_eq!(e.source_bytes(&source.id).unwrap(),original);e.close_document(&source.id).unwrap();
 let subset=e.open_document(root().join("../tests/fixtures/embedded-text/reportlab-subset.pdf")).unwrap();let selected=e.inspect_text_paragraph(&subset.id,0,&[0]).unwrap();let bounds=AnnotationRect{x:30.,y:84.,width:130.,height:55.};
 let edit=e.preview_text_paragraph_with_format(&subset.id,0,&[0],&selected.text,"edit Café edit Café edit Café edit",&bounds,&formatting(None)).unwrap();
 let runs=e.list_text_runs(&edit.id,0).unwrap().runs;assert!(runs.iter().filter(|r|r.text!="Neighbor").all(|r|r.font_name==selected.font_name&&r.is_embedded));
 e.close_document(&edit.id).unwrap();e.close_document(&subset.id).unwrap();
}
#[test]
fn inference_requires_uniform_positive_expansion_at_every_word_gap(){
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for (first,second,expected) in [(-200,0,"left"),(-200,-200,"justify"),(0,0,"left"),(-200,-250,"left")] {
  let path=temp.path().join("source.pdf");fixture(&path,0);let mut pdf=Document::load(&path).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
  let program=format!("BT /F1 10 Tf 1 0 0 1 40 330 Tm [(Alpha ) {first} (beta ) {second} (gamma)] TJ 1 0 0 1 40 315 Tm (end) Tj 1 0 0 1 40 180 Tm (Neighbor) Tj ET");
  let content=pdf.add_object(Stream::new(dictionary!{},program.into_bytes()));pdf.get_dictionary_mut(page).unwrap().set("Contents",content);pdf.save(&path).unwrap();
  let source=e.open_document(&path).unwrap();assert_eq!(alignment(&e,&source.id,&[0,1]),expected,"gaps={first},{second}");
  let selected=e.inspect_text_paragraph(&source.id,0,&[0,1]).unwrap();let edit=e.preview_text_paragraph(&source.id,0,&[0,1],&selected.text,"Alpha beta gamma delta",&area(0)).unwrap();
  assert_eq!(lines(&e.source_bytes(&edit.id).unwrap()).iter().any(|(_,gap)|*gap<0.),expected=="justify");
  e.close_document(&edit.id).unwrap();e.close_document(&source.id).unwrap();
 }
}

#[test]
fn reediting_preserves_explicit_breaks_edge_spaces_and_edit_area(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("source.pdf");fixture(&path,0);
 let source=e.open_document(&path).unwrap();let selected=e.inspect_text_paragraph(&source.id,0,&[0]).unwrap();
 let replacement="  Alpha beta\nGamma  delta  ";
 let edit=e.preview_text_paragraph_with_format(&source.id,0,&[0],&selected.text,replacement,&area(0),&formatting(None)).unwrap();
 let saved=temp.path().join("saved.pdf");
 e.export_pdf(folio_engine::ExportRequest{flatten:false,pages:vec![folio_engine::PagePlan{crop: None,id:"p".into(),source_id:edit.id.clone(),page_index:0,width:edit.pages[0].width,height:edit.pages[0].height,rotation:0,overlays:vec![]}]},&saved).unwrap();
 let reopened=e.open_document(&saved).unwrap();
 let indices:Vec<_>=e.list_text_runs(&reopened.id,0).unwrap().runs.iter().filter(|run|run.text!="Neighbor").map(|run|run.object_index).collect();
 let selection=e.inspect_text_paragraph(&reopened.id,0,&indices).unwrap();
 assert_eq!(selection.text,replacement,"Re-edit must preserve intentional breaks and edge spaces");
 let actual=selection.bounds;let expected=area(0);
 for (a,b) in [(actual.x,expected.x),(actual.y,expected.y),(actual.width,expected.width),(actual.height,expected.height)]{assert!((a-b).abs()<0.01,"Re-edit must retain the chosen area");}
 assert_eq!(alignment(&e,&reopened.id,&indices),"justify");
 for id in [&source.id,&edit.id,&reopened.id]{e.close_document(id).unwrap();}
}


#[test]
fn reediting_keeps_blank_lines_soft_wraps_and_suggestion_at_all_rotations(){
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for rotation in [0,90,180,270]{
  let path=temp.path().join("source.pdf");fixture(&path,rotation);let source=e.open_document(&path).unwrap();let initial=e.inspect_text_paragraph(&source.id,0,&[0]).unwrap();
  let text="\nAlpha beta gamma delta\n\nZeta  \n";
  let edit=e.preview_text_paragraph_with_format(&source.id,0,&[0],&initial.text,text,&area(rotation),&formatting(None)).unwrap();
  let runs=e.list_text_runs(&edit.id,0).unwrap().runs;let indices:Vec<_>=runs.iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect();
  let selected=e.inspect_text_paragraph(&edit.id,0,&indices).unwrap();assert_eq!(selected.text,text);
  let anchor=runs.iter().find(|r|r.object_index==*indices.last().unwrap()).unwrap();
  let suggested=e.suggest_text_paragraph(&edit.id,0,anchor.object_index,&anchor.text).unwrap();assert_eq!(suggested.object_indices,indices);assert_eq!(suggested.text,text);
  let before=e.source_bytes(&edit.id).unwrap();
  let again=e.preview_text_paragraph(&edit.id,0,&indices,&selected.text,&selected.text,&selected.bounds).unwrap();
  assert_eq!(lines(&before),lines(&e.source_bytes(&again.id).unwrap()));
  for id in [&source.id,&edit.id,&again.id]{e.close_document(id).unwrap();}
 }
}

#[test]
fn paragraph_hints_are_bounded_untrusted_and_do_not_duplicate_deleted_words(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("source.pdf");fixture(&path,0);let source=e.open_document(&path).unwrap();let selected=e.inspect_text_paragraph(&source.id,0,&[0]).unwrap();
 let edit=e.preview_text_paragraph_with_format(&source.id,0,&[0],&selected.text,"Alpha beta\nGamma delta",&area(0),&formatting(None)).unwrap();
 let bytes=e.source_bytes(&edit.id).unwrap();let pdf=Document::load_mem(&bytes).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
 let metadata=pdf.get_dictionary(page).unwrap().get(b"FolioParagraphs").unwrap().as_str().unwrap();
 assert!(!String::from_utf8_lossy(metadata).contains("Alpha"));assert!(!String::from_utf8_lossy(metadata).contains("Original"));
 for bad in [b"{bad".to_vec(),vec![b' ';512*1024+1],br#"{"version":999,"paragraphs":[]}"#.to_vec()] {
  let mut tampered=pdf.clone();tampered.get_dictionary_mut(page).unwrap().set("FolioParagraphs",Object::String(bad,lopdf::StringFormat::Hexadecimal));let path=temp.path().join("tampered.pdf");tampered.save(&path).unwrap();let opened=e.open_document(&path).unwrap();
  let indices:Vec<_>=e.list_text_runs(&opened.id,0).unwrap().runs.iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect();
  assert_eq!(e.inspect_text_paragraph(&opened.id,0,&indices).unwrap().text,"Alpha beta Gamma delta");e.close_document(&opened.id).unwrap();
 }
 for id in [&source.id,&edit.id]{e.close_document(id).unwrap();}
}

#[test]
fn paragraph_hints_ignore_changed_character_spacing(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("source.pdf");fixture(&path,0);let source=e.open_document(&path).unwrap();let selected=e.inspect_text_paragraph(&source.id,0,&[0]).unwrap();
 let edit=e.preview_text_paragraph_with_format(&source.id,0,&[0],&selected.text,"Alpha beta\nGamma delta",&area(0),&formatting(None)).unwrap();
 let mut pdf=Document::load_mem(&e.source_bytes(&edit.id).unwrap()).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
 let mut content=lopdf::content::Content::decode(&pdf.get_page_content(page)).unwrap();
 content.operations.iter_mut().rev().find(|op|op.operator=="Tc").unwrap().operands=vec![Object::Real(0.2)];
 let stream=pdf.add_object(Stream::new(dictionary!{},content.encode().unwrap()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);
 let path=temp.path().join("changed-spacing.pdf");pdf.save(&path).unwrap();let opened=e.open_document(&path).unwrap();
 let indices:Vec<_>=e.list_text_runs(&opened.id,0).unwrap().runs.iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect();
 assert_eq!(e.inspect_text_paragraph(&opened.id,0,&indices).unwrap().text,"Alpha beta Gamma delta","Changed glyph spacing must invalidate the saved paragraph hint");
 for id in [&source.id,&edit.id,&opened.id]{e.close_document(id).unwrap();}
}

#[test]
fn paragraph_hints_preserve_other_paragraphs_and_reject_partial_or_forged_settings(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("source.pdf");fixture(&path,0);let source=e.open_document(&path).unwrap();let initial=e.inspect_text_paragraph(&source.id,0,&[0]).unwrap();
 let text="Alpha\n\nBeta";let mut format=formatting(None);format.line_spacing=Some(3.);
 let edit=e.preview_text_paragraph_with_format(&source.id,0,&[0],&initial.text,text,&area(0),&format).unwrap();
 let runs=e.list_text_runs(&edit.id,0).unwrap().runs;let indices:Vec<_>=runs.iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect();
 assert_eq!(e.inspect_text_paragraph(&edit.id,0,&indices).unwrap().text,text);
 let partial=e.inspect_text_paragraph(&edit.id,0,&indices[..1]).unwrap();assert_eq!(partial.text,"Alpha");assert_ne!(partial.bounds.height,area(0).height);
 let neighbor=runs.iter().find(|r|r.text=="Neighbor").unwrap();let select=e.inspect_text_paragraph(&edit.id,0,&[neighbor.object_index]).unwrap();
 let other_area=AnnotationRect{x:40.,y:210.,width:180.,height:65.};
 let second=e.preview_text_paragraph_with_format(&edit.id,0,&[neighbor.object_index],&select.text,"Other words\nSecond row",&other_area,&formatting(None)).unwrap();
 let runs=e.list_text_runs(&second.id,0).unwrap().runs;let anchor=runs.iter().find(|r|r.text=="Beta").unwrap();
 let remembered=e.suggest_text_paragraph(&second.id,0,anchor.object_index,&anchor.text).unwrap();assert_eq!(remembered.text,text);assert_eq!(remembered.line_spacing,Some(3.));
 let again=e.preview_text_paragraph(&second.id,0,&remembered.object_indices,&remembered.text,&remembered.text,&remembered.bounds).unwrap();
 let pdf=Document::load_mem(&e.source_bytes(&again.id).unwrap()).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
 let records:serde_json::Value=serde_json::from_slice(pdf.get_dictionary(page).unwrap().get(b"FolioParagraphs").unwrap().as_str().unwrap()).unwrap();assert_eq!(records["paragraphs"].as_array().unwrap().len(),2);
 // A forged spacing claim cannot authorize an otherwise unsupported 60pt gap.
 let mut bad=records.clone();bad["paragraphs"][1]["spacing"]=serde_json::json!(1.2);
 let mut altered=pdf.clone();altered.get_dictionary_mut(page).unwrap().set("FolioParagraphs",Object::String(serde_json::to_vec(&bad).unwrap(),lopdf::StringFormat::Hexadecimal));let path=temp.path().join("forged.pdf");altered.save(&path).unwrap();let opened=e.open_document(&path).unwrap();
 let runs=e.list_text_runs(&opened.id,0).unwrap().runs;let selected:Vec<_>=runs.iter().filter(|r|r.text=="Alpha"||r.text=="Beta").map(|r|r.object_index).collect();
 assert!(e.inspect_text_paragraph(&opened.id,0,&selected).unwrap_err().to_string().contains("consecutive"));
 for id in [&source.id,&edit.id,&second.id,&again.id,&opened.id]{e.close_document(id).unwrap();}
}

#[test]
fn paragraph_hints_survive_substitute_fonts_rotation_duplication_and_protection(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("source.pdf");fixture(&path,0);
 let protected=protected_fixture::encrypt(&std::fs::read(&path).unwrap(),lopdf::encryption::Permissions::all());std::fs::write(&path,&protected).unwrap();
 let source=e.open_document_with_password(&path,"owner").unwrap();let initial=e.inspect_text_paragraph(&source.id,0,&[0]).unwrap();
 let font=e.fonts().register(std::fs::read(root().join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
 let value="  jj Café words\n\nMore words  ";
 let edit=e.preview_text_paragraph_with_format(&source.id,0,&[0],&initial.text,value,&area(0),&formatting(Some(&font.id))).unwrap();
 let saved=temp.path().join("rotated-duplicates.pdf");let plans=[90,0].into_iter().enumerate().map(|(i,rotation)|folio_engine::PagePlan{crop: None,id:format!("p{i}"),source_id:edit.id.clone(),page_index:0,width:edit.pages[0].width,height:edit.pages[0].height,rotation,overlays:vec![]}).collect();
 e.export_pdf(folio_engine::ExportRequest{flatten:false,pages:plans},&saved).unwrap();assert!(e.open_document(&saved).is_err());
 let opened=e.open_document_with_password(&saved,"owner").unwrap();assert_eq!(opened.pages.len(),2);
 for page in 0..2{
  let runs=e.list_text_runs(&opened.id,page).unwrap().runs;let indices:Vec<_>=runs.iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect();
  let selected=e.inspect_text_paragraph(&opened.id,page,&indices).unwrap();assert_eq!(selected.text,value);assert_eq!(selected.bounds,area(if page==0{90}else{0}));
  let again=e.preview_text_paragraph(&opened.id,page,&indices,&selected.text,&selected.text,&selected.bounds).unwrap();
  let runs=e.list_text_runs(&again.id,0).unwrap().runs;let indices:Vec<_>=runs.iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect();assert_eq!(e.inspect_text_paragraph(&again.id,0,&indices).unwrap().text,value);e.close_document(&again.id).unwrap();
 }
 let reverse=temp.path().join("reversed.pdf");
 let plans=[1,0].into_iter().map(|index|folio_engine::PagePlan{crop: None,id:format!("reverse-{index}"),source_id:opened.id.clone(),page_index:index,width:opened.pages[index].width,height:opened.pages[index].height,rotation:0,overlays:vec![]}).collect();
 e.export_pdf(folio_engine::ExportRequest{flatten:false,pages:plans},&reverse).unwrap();
 let reversed=e.open_document_with_password(&reverse,"owner").unwrap();
 for page in 0..2{
  let indices:Vec<_>=e.list_text_runs(&reversed.id,page).unwrap().runs.iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect();
  let selected=e.inspect_text_paragraph(&reversed.id,page,&indices).unwrap();assert_eq!(selected.text,value);assert_eq!(selected.bounds,area(if page==0{0}else{90}));
 }
 for id in [&source.id,&edit.id,&opened.id,&reversed.id]{e.close_document(id).unwrap();}
 assert_eq!(std::fs::read(&path).unwrap(),protected);
}

#[test]
fn review_forged_glyphs_do_not_authorize_nonzero_character_spacing(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("source.pdf");fixture(&path,0);let source=e.open_document(&path).unwrap();let selected=e.inspect_text_paragraph(&source.id,0,&[0]).unwrap();
 let edit=e.preview_text_paragraph_with_format(&source.id,0,&[0],&selected.text,"Alpha\nGamma",&area(0),&formatting(None)).unwrap();
 let mut pdf=Document::load_mem(&e.source_bytes(&edit.id).unwrap()).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
 let mut content=lopdf::content::Content::decode(&pdf.get_page_content(page)).unwrap();content.operations.iter_mut().rev().find(|op|op.operator=="Tc").unwrap().operands=vec![Object::Real(0.2)];
 let stream=pdf.add_object(Stream::new(dictionary!{},content.encode().unwrap()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);
 let mut hints:serde_json::Value=serde_json::from_slice(pdf.get_dictionary(page).unwrap().get(b"FolioParagraphs").unwrap().as_str().unwrap()).unwrap();
 for row in hints["paragraphs"][0]["rows"].as_array_mut().unwrap(){
  for (i,glyph) in row["glyphs"].as_array_mut().unwrap().iter_mut().enumerate(){for coordinate in [0,2,4]{glyph[coordinate]=serde_json::json!(glyph[coordinate].as_f64().unwrap()+i as f64*0.2);}}
 }
 pdf.get_dictionary_mut(page).unwrap().set("FolioParagraphs",Object::String(serde_json::to_vec(&hints).unwrap(),lopdf::StringFormat::Hexadecimal));let path=temp.path().join("forged-glyphs.pdf");pdf.save(&path).unwrap();let opened=e.open_document(&path).unwrap();
 let indices:Vec<_>=e.list_text_runs(&opened.id,0).unwrap().runs.iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect();
 assert_eq!(e.inspect_text_paragraph(&opened.id,0,&indices).unwrap().text,"Alpha Gamma");
 for id in [&source.id,&edit.id,&opened.id]{e.close_document(id).unwrap();}
}

#[test]
fn review_overlapping_hints_do_not_choose_a_paragraph_by_json_order(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("source.pdf");fixture(&path,0);let source=e.open_document(&path).unwrap();let initial=e.inspect_text_paragraph(&source.id,0,&[0]).unwrap();
 let edit=e.preview_text_paragraph_with_format(&source.id,0,&[0],&initial.text,"Alpha\nBeta",&area(0),&formatting(None)).unwrap();
 let pdf=Document::load_mem(&e.source_bytes(&edit.id).unwrap()).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
 let hints:serde_json::Value=serde_json::from_slice(pdf.get_dictionary(page).unwrap().get(b"FolioParagraphs").unwrap().as_str().unwrap()).unwrap();
 let full=hints["paragraphs"][0].clone();let mut partial=full.clone();partial["rows"].as_array_mut().unwrap().truncate(1);partial["trailing"]=serde_json::json!(0);
 for records in [vec![partial.clone(),full.clone()],vec![full.clone(),partial.clone()]]{
  let mut altered=pdf.clone();altered.get_dictionary_mut(page).unwrap().set("FolioParagraphs",Object::String(serde_json::to_vec(&serde_json::json!({"version":1,"paragraphs":records})).unwrap(),lopdf::StringFormat::Hexadecimal));let path=temp.path().join("overlap.pdf");altered.save(&path).unwrap();let opened=e.open_document(&path).unwrap();let anchor=e.list_text_runs(&opened.id,0).unwrap().runs.into_iter().find(|r|r.text=="Alpha").unwrap();
  let selected=e.suggest_text_paragraph(&opened.id,0,anchor.object_index,&anchor.text).unwrap();assert_eq!(selected.text,"Alpha Beta");assert_eq!(selected.object_indices.len(),2);e.close_document(&opened.id).unwrap();
 }
 for id in [&source.id,&edit.id]{e.close_document(id).unwrap();}
}

#[test]
fn review_whitespace_only_hard_lines_roundtrip_original_and_substitute_fonts(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("source.pdf");fixture(&path,0);let source=e.open_document(&path).unwrap();let initial=e.inspect_text_paragraph(&source.id,0,&[0]).unwrap();
 let font=e.fonts().register(std::fs::read(root().join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
 for substitute in [None,Some(font.id.as_str())]{
  let value="Alpha\n  \nBeta";let edit=e.preview_text_paragraph_with_format(&source.id,0,&[0],&initial.text,value,&area(0),&formatting(substitute)).unwrap();
  let indices:Vec<_>=e.list_text_runs(&edit.id,0).unwrap().runs.iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect();
  assert_eq!(e.inspect_text_paragraph(&edit.id,0,&indices).unwrap().text,value);
  let anchor=e.list_text_runs(&edit.id,0).unwrap().runs.into_iter().find(|r|r.text=="Beta").unwrap();assert_eq!(e.suggest_text_paragraph(&edit.id,0,anchor.object_index,&anchor.text).unwrap().text,value);e.close_document(&edit.id).unwrap();
 }
 e.close_document(&source.id).unwrap();
}
