use folio_engine::{AnnotationRect,ExportRequest,PagePlan,ParagraphAlignment,PdfEngine,StyledParagraphLayoutRequest,StyledParagraphRun};
use lopdf::{dictionary,Document,Object,Stream};
use std::path::{Path,PathBuf};
fn engine()->PdfEngine{PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture(path:&Path,rotation:i32,extra:&str){
 let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
 let plain=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
 let bold=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica-Bold"});
 let stream=pdf.add_object(Stream::new(dictionary!{},format!("BT /F1 12 Tf 50 500 Td (One ) Tj ET BT /F2 20 Tf 0 .5 0 rg 95 500 Td (Two) Tj ET BT /F1 12 Tf 0 0 0 rg 50 100 Td (Neighbor) Tj ET {extra}").into_bytes()));
 let note=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![300.into(),100.into(),318.into(),118.into()],"Contents"=>Object::string_literal("Keep note"),"NM"=>Object::string_literal("mixed-note"),"T"=>Object::string_literal("Reviewer"),"F"=>4});
 let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),500.into(),600.into()],"CropBox"=>vec![10.into(),20.into(),490.into(),580.into()],"Rotate"=>rotation,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>plain,"F2"=>bold}},"Contents"=>stream,"Annots"=>vec![note.into()]});
 pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",root);pdf.save(path).unwrap();
}
fn run(text:&str,style_id:usize)->StyledParagraphRun{StyledParagraphRun{text:text.into(),style_id}}
fn request(rotation:i32)->StyledParagraphLayoutRequest{
 let b=match rotation{0=>[40.,60.,180.,200.],90=>[300.,40.,200.,180.],180=>[260.,300.,180.,200.],270=>[60.,260.,200.,180.],_=>unreachable!()};
 StyledParagraphLayoutRequest{baseline_anchor: None,style_changes:vec![],object_indices:vec![0,1],expected_text:"One Two".into(),runs:vec![run("pre",0),run("fix ",1),run("word alpha beta gamma\n",0),run("bold",1)],bounds:AnnotationRect{x:b[0],y:b[1],width:b[2],height:b[3]},alignment:ParagraphAlignment::Justify,line_spacing:1.2}
}
fn export(e:&PdfEngine,source:&folio_engine::DocumentInfo,path:&Path){
 e.export_pdf(ExportRequest{flatten:false,pages:vec![PagePlan{crop: None,id:"page".into(),source_id:source.id.clone(),page_index:0,width:source.pages[0].width,height:source.pages[0].height,rotation:0,overlays:vec![]}]},path).unwrap();
}
#[test]
fn publishes_source_styles_and_preserves_neighbors_notes_and_original_through_save(){
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for rotation in [0,90,180,270]{
  let path=temp.path().join("input.pdf");fixture(&path,rotation,"");let bytes=std::fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();
  let original=e.list_text_runs(&source.id,0).unwrap().runs;let neighbor=original.iter().find(|r|r.text=="Neighbor").unwrap();
  let edited=e.preview_styled_paragraph(&source.id,0,&request(rotation)).unwrap();
  assert_eq!(e.list_native_comments(&edited.id).unwrap()[0].text,"Keep note");
  let output=temp.path().join("saved.pdf");export(&e,&edited,&output);let reopened=e.open_document(&output).unwrap();
  for id in [&edited.id,&reopened.id]{
   let runs=e.list_text_runs(id,0).unwrap().runs;let kept=runs.iter().find(|r|r.text=="Neighbor").unwrap();assert_eq!(kept.bounds,neighbor.bounds);assert_eq!(kept.font_name,neighbor.font_name);
   let bold=runs.iter().find(|r|r.text.trim()=="bold").unwrap();assert_eq!(bold.font_name,"Helvetica-Bold");assert_eq!(bold.font_size,20.);
   assert!(runs.iter().any(|r|r.text=="pre"&&r.font_name=="Courier"&&r.font_size==12.));assert!(!runs.iter().any(|r|r.text=="One"||r.text=="Two"));
   assert_eq!(e.list_native_comments(id).unwrap()[0].text,"Keep note");
  }
  assert_eq!(e.render_page(&edited.id,0,700).unwrap(),e.render_page(&reopened.id,0,700).unwrap());
  assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),bytes.as_slice());assert_eq!(std::fs::read(&path).unwrap(),bytes);
  for id in [&source.id,&edited.id,&reopened.id]{e.close_document(id).unwrap();}
 }
}
#[test]
fn refuses_graphics_text_collisions_and_invalid_requests_without_source_changes(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("refuse.pdf");
 for extra in ["q 1 0 0 rg 50 430 60 30 re f Q","BT /F1 12 Tf 50 430 Td (Blocking text) Tj ET"]{
  fixture(&path,0,extra);let source=e.open_document(&path).unwrap();let bytes=e.source_bytes(&source.id).unwrap();let error=e.preview_styled_paragraph(&source.id,0,&request(0)).unwrap_err().to_string();assert!(error.contains("overlap"),"{error}");assert_eq!(e.source_bytes(&source.id).unwrap(),bytes);e.close_document(&source.id).unwrap();
 }
 fixture(&path,0,"");let source=e.open_document(&path).unwrap();let bytes=e.source_bytes(&source.id).unwrap();
 let mut bad=request(0);bad.runs[0].style_id=77;assert!(e.preview_styled_paragraph(&source.id,0,&bad).is_err());
 let mut bad=request(0);bad.expected_text="stale".into();assert!(e.preview_styled_paragraph(&source.id,0,&bad).is_err());
 let mut bad=request(0);bad.bounds.height=5.;assert!(e.preview_styled_paragraph(&source.id,0,&bad).is_err());
 assert_eq!(e.source_bytes(&source.id).unwrap(),bytes);e.close_document(&source.id).unwrap();
}
#[test]
fn embedded_and_standard_fragments_publish_with_original_encoding_and_fonts(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("embedded.pdf");
 let mut pdf=Document::load(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/embedded-text/reportlab-subset.pdf")).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
 let stream=pdf.add_object(Stream::new(dictionary!{},b"BT /F2+0 16 Tf 1 0 0 1 30 300 Tm (Caf\x01) Tj ET BT /F1 12 Tf 1 0 0 1 90 300 Tm (mix) Tj ET BT /F1 12 Tf 30 60 Td (Neighbor) Tj ET".to_vec()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);pdf.save(&path).unwrap();
 let source=e.open_document(&path).unwrap();let before=e.source_bytes(&source.id).unwrap();let selected=e.inspect_styled_paragraph(&source.id,0,&[0,1]).unwrap();
 let mut value=StyledParagraphLayoutRequest{baseline_anchor: None,style_changes:vec![],object_indices:vec![0,1],expected_text:selected.text,runs:vec![run("Café ",0),run("mix\n\n",1),run("old",0)],bounds:AnnotationRect{x:20.,y:60.,width:200.,height:150.},alignment:ParagraphAlignment::Right,line_spacing:1.2};
 let edited=e.preview_styled_paragraph(&source.id,0,&value).unwrap();
 value.runs[0].text="Cafè".into();assert!(e.preview_styled_paragraph(&source.id,0,&value).is_err());assert_eq!(e.source_bytes(&source.id).unwrap(),before);
 e.close_document(&source.id).unwrap(); // Embedded output must own its font/source bytes.
 let output=temp.path().join("saved.pdf");export(&e,&edited,&output);let reopened=e.open_document(&output).unwrap();
 let runs=e.list_text_runs(&reopened.id,0).unwrap().runs;assert!(runs.iter().any(|r|r.text.trim()=="Café"&&r.is_embedded&&r.font_size==16.));assert!(runs.iter().any(|r|r.text=="old"&&r.is_embedded));assert!(runs.iter().any(|r|r.text=="Neighbor"));
 for id in [&edited.id,&reopened.id]{e.close_document(id).unwrap();}
}
#[test]
fn whitespace_fragments_and_hard_breaks_survive_publication(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("spaces.pdf");fixture(&path,0,"");let source=e.open_document(&path).unwrap();
 let mut value=request(0);value.runs=vec![run("alpha ",0),run(" ",1),run("beta gamma\n\n",0),run("bold  ",1)];
 let edited=e.preview_styled_paragraph(&source.id,0,&value).unwrap();let bytes=e.source_bytes(&edited.id).unwrap();let pdf=Document::load_mem(&bytes).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
 let content=lopdf::content::Content::decode(&pdf.get_page_content(page)).unwrap();
 assert!(content.operations.iter().any(|op|op.operator=="Tj"&&op.operands[0].as_str().ok()==Some(&b" "[..])));
 assert!(content.operations.iter().any(|op|op.operator=="Tj"&&op.operands[0].as_str().ok()==Some(&b"bold  "[..])));
 let output=temp.path().join("saved.pdf");export(&e,&edited,&output);let reopened=e.open_document(&output).unwrap();assert_eq!(e.render_page(&edited.id,0,600).unwrap(),e.render_page(&reopened.id,0,600).unwrap());
 for id in [&source.id,&edited.id,&reopened.id]{e.close_document(id).unwrap();}
}
#[path="support/protected_fixture.rs"]
mod protected_fixture;
#[test]
fn protected_publication_keeps_rights_credentials_and_original_path_guard(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("protected.pdf");
 for writable in [false,true]{
  fixture(&path,0,"");let rights=if writable{lopdf::encryption::Permissions::MODIFIABLE}else{lopdf::encryption::Permissions::PRINTABLE};let original=protected_fixture::encrypt(&std::fs::read(&path).unwrap(),rights);std::fs::write(&path,&original).unwrap();
  let source=e.open_document_with_password(&path,"reader").unwrap();
  if writable{
   let edited=e.preview_styled_paragraph(&source.id,0,&request(0)).unwrap();assert_eq!(edited.protection,source.protection);assert!(e.extract_text(&edited.id,0).is_err());
   let plan=PagePlan{crop: None,id:"p".into(),source_id:edited.id.clone(),page_index:0,width:edited.pages[0].width,height:edited.pages[0].height,rotation:0,overlays:vec![]};
   assert!(e.export_pdf(ExportRequest{pages:vec![plan],flatten:false},&path).is_err());let output=temp.path().join("saved.pdf");export(&e,&edited,&output);
   for password in ["reader","owner"]{let unlocked=folio_engine::protection::unlock_pdf(&std::fs::read(&output).unwrap(),Some(password)).unwrap();let pdf=Document::load_mem(&unlocked.bytes).unwrap();assert!(pdf.extract_text(&[1]).unwrap().contains("bold"));}
   e.close_document(&edited.id).unwrap();
  }else{assert!(e.preview_styled_paragraph(&source.id,0,&request(0)).is_err());}
  assert_eq!(std::fs::read(&path).unwrap(),original);e.close_document(&source.id).unwrap();
 }
}
#[test]
fn normalized_effective_sizes_and_admitted_repeat_edits_keep_source_fonts(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("scaled.pdf");fixture(&path,0,"");
 let mut pdf=Document::load(&path).unwrap();let page=*pdf.get_pages().get(&1).unwrap();let stream=pdf.add_object(Stream::new(dictionary!{},b"BT /F1 6 Tf 2 0 0 2 50 500 Tm (One ) Tj ET BT /F2 20 Tf 0 .5 0 rg 95 500 Td (Two) Tj ET BT /F1 12 Tf 50 100 Td (Neighbor) Tj ET".to_vec()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);pdf.save(&path).unwrap();
 let source=e.open_document(&path).unwrap();let mut current=source.clone();
 for text in ["Changed ","Short ","Again "]{
  let runs=e.list_text_runs(&current.id,0).unwrap().runs;let indices=runs.iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect::<Vec<_>>();let selection=e.inspect_styled_paragraph(&current.id,0,&indices).unwrap();let mut value=request(0);value.object_indices=indices;value.expected_text=selection.text;value.runs=vec![run(text,0),run("Bold",1)];value.alignment=ParagraphAlignment::Left;
  let next=e.preview_styled_paragraph(&current.id,0,&value).unwrap();assert!(e.list_text_runs(&next.id,0).unwrap().runs.iter().any(|r|r.text.trim()==text.trim()&&r.font_size==12.));
  if current.id!=source.id{e.close_document(&current.id).unwrap();}current=next;
 }
 e.close_document(&current.id).unwrap();e.close_document(&source.id).unwrap();
}
#[test]
fn publication_keeps_alias_and_distinct_same_name_resource_bindings(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("aliases.pdf");fixture(&path,0,"");let mut pdf=Document::load(&path).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
 let distinct=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
 let fonts=pdf.get_dictionary_mut(page).unwrap().get_mut(b"Resources").unwrap().as_dict_mut().unwrap().get_mut(b"Font").unwrap().as_dict_mut().unwrap();let original=fonts.get(b"F1").unwrap().clone();fonts.set("Alias",original);fonts.set("F3",distinct);
 let stream=pdf.add_object(Stream::new(dictionary!{},b"BT /Alias 12 Tf 50 500 Td (A ) Tj ET BT /F1 12 Tf 70 500 Td (B ) Tj ET BT /F3 12 Tf 90 500 Td (C) Tj ET".to_vec()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);pdf.save(&path).unwrap();
 let source=e.open_document(&path).unwrap();let selected=e.inspect_styled_paragraph(&source.id,0,&[0,1,2]).unwrap();assert_eq!(selected.styles.len(),2);assert_eq!(selected.styles[0].object_indices,[0,1]);
 let mut value=request(0);value.object_indices=vec![0,1,2];value.expected_text=selected.text;value.runs=vec![run("Alias ",0),run("Distinct",1)];
 let edited=e.preview_styled_paragraph(&source.id,0,&value).unwrap();let bytes=e.source_bytes(&edited.id).unwrap();let output=Document::load_mem(&bytes).unwrap();let page=*output.get_pages().get(&1).unwrap();let content=lopdf::content::Content::decode(&output.get_page_content(page)).unwrap();let mut font=vec![];let mut seen=Vec::new();
 for op in content.operations {if op.operator=="Tf"{font=op.operands[0].as_name().unwrap().to_vec();}if op.operator=="Tj"{seen.push((op.operands[0].as_str().unwrap().to_vec(),font.clone()));}}
 assert!(seen.contains(&(b"Alias ".to_vec(),b"Alias".to_vec())));assert!(seen.contains(&(b"Distinct".to_vec(),b"F3".to_vec())));
 let saved=temp.path().join("saved.pdf");export(&e,&edited,&saved);let reopened=e.open_document(&saved).unwrap();assert!(e.extract_text(&reopened.id,0).unwrap().contains("Distinct"));
 for id in [&source.id,&edited.id,&reopened.id]{e.close_document(id).unwrap();}
}
#[test]
fn mixed_cursor_removals_keep_a_relative_following_show_at_its_original_position(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("relative.pdf");fixture(&path,0,"");let mut pdf=Document::load(&path).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
 let stream=pdf.add_object(Stream::new(dictionary!{},b"BT /F1 12 Tf .25 Tc .5 Tw 50 500 Td [(One ) -30] TJ /F2 20 Tf (Two ) Tj /F1 12 Tf (Neighbor) Tj ET".to_vec()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);pdf.save(&path).unwrap();
 let source=e.open_document(&path).unwrap();let before=e.list_text_runs(&source.id,0).unwrap().runs;let neighbor=before.iter().find(|r|r.text=="Neighbor").unwrap();let selection=e.inspect_styled_paragraph(&source.id,0,&[0,1]).unwrap();
 let mut value=request(0);value.expected_text=selection.text;value.bounds=AnnotationRect{x:200.,y:200.,width:180.,height:150.};
 let edited=e.preview_styled_paragraph(&source.id,0,&value).unwrap();let after=e.list_text_runs(&edited.id,0).unwrap().runs;let actual=after.iter().find(|r|r.text=="Neighbor").unwrap();
 for (a,b) in [(actual.bounds.x,neighbor.bounds.x),(actual.bounds.y,neighbor.bounds.y),(actual.bounds.width,neighbor.bounds.width),(actual.bounds.height,neighbor.bounds.height)]{assert!((a-b).abs()<0.001,"{a} != {b}");}
 e.close_document(&edited.id).unwrap();e.close_document(&source.id).unwrap();
}
