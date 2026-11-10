use folio_engine::{AnnotationRect,ExportRequest,PagePlan,ParagraphAlignment,PdfEngine,StyledParagraphLayoutRequest,StyledParagraphRun};
use lopdf::{dictionary,Document,Object,Stream};
use std::{fs,path::{Path,PathBuf}};
fn engine()->PdfEngine{PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture(path:&Path,unit:f32){
 let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
 let plain=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});let bold=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica-Bold"});
 let resources=dictionary!{"Font"=>dictionary!{"F1"=>plain,"F2"=>bold}};
 let leaf=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),230.into(),70.into()],"Resources"=>resources.clone()},b"BT /F2 14 Tf 1 0 0 1 5 45 Tm (First paragraph line) Tj ET BT /F1 12 Tf 1 0 0 1 5 25 Tm (Second paragraph line) Tj ET".to_vec()));
 let wrapper=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),230.into(),70.into()],"Resources"=>dictionary!{"XObject"=>dictionary!{"Leaf"=>leaf}}},b"/Leaf Do".to_vec()));
 let stream=pdf.add_object(Stream::new(dictionary!{},b"q 1 0 0 1 40 220 cm /Shared Do Q q 1 0 0 1 350 220 cm /Shared Do Q".to_vec()));
 let note=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![30.into(),50.into(),48.into(),68.into()],"Contents"=>Object::string_literal("Keep note"),"NM"=>Object::string_literal("form-paragraph-note")});
 let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),800.into(),500.into()],"UserUnit"=>unit,"Resources"=>dictionary!{"XObject"=>dictionary!{"Shared"=>wrapper}},"Contents"=>stream,"Annots"=>vec![Object::Reference(note)]});
 let other=pdf.add_object(Stream::new(dictionary!{},b"BT /F1 12 Tf 30 100 Td (Other page unchanged) Tj ET".to_vec()));
 let other_page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),800.into(),500.into()],"Resources"=>resources,"Contents"=>other});
 pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![Object::Reference(page),Object::Reference(other_page)],"Count"=>2}.into());let catalog=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",catalog);pdf.save(path).unwrap();
}
fn save(e:&PdfEngine,doc:&folio_engine::DocumentInfo,path:&Path){e.export_pdf(ExportRequest{flatten:false,pages:doc.pages.iter().enumerate().map(|(i,p)|PagePlan{id:format!("p{i}"),source_id:doc.id.clone(),page_index:i,width:p.width,height:p.height,rotation:0,crop:None,overlays:vec![]}).collect()},path).unwrap();}

#[path="support/affine_forms.rs"]
#[allow(dead_code)]
mod inherited_separator_fixture;

#[test]
fn form_paragraph_edits_inherited_fonts_without_changing_other_occurrences(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let mut failures=vec![];
 for kind in ["standard","simple","cid"] {
  let path=temp.path().join(format!("{kind}.pdf"));inherited_separator_fixture::fixture(&path,kind,"plain",0,"Cafe old");
  let original=fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();
  let result=(||->Result<(),Box<dyn std::error::Error>>{
   let nested=e.list_occurrence_form_text_runs(&source.id,0)?.runs;assert_eq!(nested.len(),2);
   let anchor=&nested[0];let listing=e.list_form_paragraph_runs(&source.id,0,&anchor.object_path,&anchor.text)?;
   let selected=e.suggest_form_styled_paragraph(&source.id,0,&anchor.object_path,listing.anchor_index,&anchor.text)?;
   assert_eq!(selected.text,anchor.text);
   let mut request=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:selected.object_indices,expected_text:selected.text,runs:selected.runs,style_changes:vec![],bounds:selected.bounds,alignment:selected.alignment.unwrap_or(ParagraphAlignment::Left),line_spacing:selected.line_spacing.unwrap_or(1.2)};
   let order=(0..source.pages.len()).collect::<Vec<_>>();
   let noop=e.preview_form_styled_paragraph_occurrence(&source.id,&order,0,&anchor.object_path,&request)?;
   for page in 0..source.pages.len(){assert_eq!(e.render_page(&source.id,page,1000)?,e.render_page(&noop.document.id,page,1000)?,"no-op {kind} page{page}");}
   e.close_document(&noop.document.id)?;
   request.runs.last_mut().unwrap().text.push_str(" old");
   request.bounds=e.fit_form_styled_paragraph(&source.id,0,&anchor.object_path,&request)?;
   let edited=e.preview_form_styled_paragraph_occurrence(&source.id,&order,0,&anchor.object_path,&request)?;
   let remaining=e.list_occurrence_form_text_runs(&edited.document.id,0)?.runs;
   assert_eq!(remaining.len(),1);assert_eq!(remaining[0].text,nested[1].text);assert_eq!(remaining[0].bounds,nested[1].bounds);
   for page in 1..source.pages.len(){assert_eq!(e.render_page(&source.id,page,1000)?,e.render_page(&edited.document.id,page,1000)?);}
   let output=temp.path().join(format!("{kind}-saved.pdf"));save(&e,&edited.document,&output);let reopened=e.open_document(&output)?;
   assert_eq!(e.render_page(&edited.document.id,0,1000)?,e.render_page(&reopened.id,0,1000)?);
   let runs=e.list_occurrence_text_runs(&reopened.id,0)?.runs;
   let changed=runs.iter().find(|r|r.text==format!("{} old",anchor.text)).expect("saved grown paragraph");
   let again=e.suggest_occurrence_styled_paragraph(&reopened.id,0,changed.object_index,&changed.text)?;
   assert_eq!(again.text,format!("{} old",anchor.text));
   e.close_document(&reopened.id)?;e.close_document(&edited.document.id)?;
   Ok(())
  })();
  if let Err(error)=result{failures.push(format!("{kind}: {error}"));}
  assert_eq!(fs::read(&path).unwrap(),original);e.close_document(&source.id).unwrap();
 }
 assert!(failures.is_empty(),"{}",failures.join("\n"));
}

#[test]
fn form_paragraph_edits_one_shared_invocation_and_replays_after_save(){
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for unit in [1.,1.7]{
  let path=temp.path().join(format!("nested-{unit}.pdf"));fixture(&path,unit);let original=fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();let before=e.source_bytes(&source.id).unwrap();
  let path_ids=[0,0,0];let listing=e.list_form_paragraph_runs(&source.id,0,&path_ids,"First paragraph line").unwrap();assert_eq!(listing.runs.len(),2);assert_eq!(listing.anchor_index,0);
  let suggested=e.suggest_form_styled_paragraph(&source.id,0,&path_ids,listing.anchor_index,&listing.runs[0].text).unwrap();
  // A larger heading and smaller body have no shared body size. Preserve the
  // existing conservative suggestion; explicit adjustment can select both.
  assert_eq!(suggested.object_indices,vec![0]);
  let selected=e.inspect_form_styled_paragraph(&source.id,0,&path_ids,&[0,1]).unwrap();assert_eq!(selected.styles.len(),2);
  let request=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:selected.object_indices,expected_text:selected.text,runs:vec![StyledParagraphRun{text:"Changed heading ".into(),style_id:0},StyledParagraphRun{text:"and the rest of this paragraph now wraps into more lines.".into(),style_id:1}],style_changes:vec![],bounds:AnnotationRect{x:45.*unit,y:220.*unit,width:240.*unit,height:95.*unit},alignment:ParagraphAlignment::Left,line_spacing:1.3};
  let edited=e.preview_form_styled_paragraph_occurrence(&source.id,&[0,0,1],0,&path_ids,&request).unwrap();assert_eq!(edited.document.pages.len(),3);assert_eq!(edited.page_indices,vec![0,2,1]);
  assert_eq!(e.render_page(&source.id,0,800).unwrap(),e.render_page(&edited.document.id,2,800).unwrap());assert_eq!(e.render_page(&source.id,1,800).unwrap(),e.render_page(&edited.document.id,1,800).unwrap());
  let remaining=e.list_occurrence_form_text_runs(&edited.document.id,0).unwrap();assert_eq!(remaining.runs.len(),2);assert_eq!(remaining.runs[0].text,"First paragraph line");assert!(remaining.runs[0].bounds.x>300.*unit);
  let output=temp.path().join(format!("saved-{unit}.pdf"));save(&e,&edited.document,&output);let reopened=e.open_document(&output).unwrap();assert_eq!(e.render_page(&edited.document.id,0,800).unwrap(),e.render_page(&reopened.id,0,800).unwrap());
  let runs=e.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;let anchor=runs.iter().find(|run|run.text.starts_with("Changed")).unwrap();let saved=e.suggest_occurrence_styled_paragraph(&reopened.id,0,anchor.object_index,&anchor.text).unwrap();
  let same=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:saved.object_indices,expected_text:saved.text,runs:saved.runs,style_changes:vec![],bounds:saved.bounds,alignment:saved.alignment.unwrap_or(ParagraphAlignment::Left),line_spacing:saved.line_spacing.unwrap_or(1.3)};
  let noop=e.preview_styled_paragraph_occurrence(&reopened.id,&[0,1,2],0,&same).unwrap();assert_eq!(e.render_page(&reopened.id,0,800).unwrap(),e.render_page(&noop.document.id,0,800).unwrap());
  assert_eq!(e.source_bytes(&source.id).unwrap(),before);assert_eq!(fs::read(path).unwrap(),original);
  for id in [&noop.document.id,&reopened.id,&edited.document.id,&source.id]{e.close_document(id).unwrap();}
 }
}

#[test]
fn form_paragraph_inspection_and_fit_enforce_the_original_invocation_scope(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("scope.pdf");fixture(&path,1.);let source=e.open_document(&path).unwrap();let path_ids=[0,0,0];
 assert!(e.list_form_paragraph_runs(&source.id,0,&path_ids,"stale").is_err());
 assert!(e.inspect_form_styled_paragraph(&source.id,0,&path_ids,&[2]).is_err());
 let selected=e.inspect_form_styled_paragraph(&source.id,0,&path_ids,&[0,1]).unwrap();
 let mut request=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:selected.object_indices,expected_text:selected.text,runs:selected.runs,style_changes:vec![],bounds:selected.bounds,alignment:ParagraphAlignment::Left,line_spacing:1.3};
 let area=e.fit_form_styled_paragraph(&source.id,0,&path_ids,&request).unwrap();assert!(area.width>0.&&area.height>0.);
 request.object_indices=vec![2];assert!(e.fit_form_styled_paragraph(&source.id,0,&path_ids,&request).is_err());assert!(e.preview_form_styled_paragraph_occurrence(&source.id,&[0,1],0,&path_ids,&request).is_err());
 e.close_document(&source.id).unwrap();
}

#[path="support/protected_fixture.rs"]
mod protected_fixture;
#[test]
fn oversized_form_paragraph_draft_is_rejected_before_source_projection(){
 let e=engine();let request=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:vec![0],expected_text:"Old".into(),runs:vec![StyledParagraphRun{text:"x".repeat(4097),style_id:0}],style_changes:vec![],bounds:AnnotationRect{x:0.,y:0.,width:100.,height:100.},alignment:ParagraphAlignment::Left,line_spacing:1.2};
 let error=e.fit_form_styled_paragraph("no-such-source",0,&[0,0],&request).unwrap_err().to_string();assert!(error.contains("4096"),"{error}");
}
#[test]
fn form_paragraph_preserves_protected_source_rights_and_save_provenance(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("protected.pdf");
 for writable in [false,true]{
  fixture(&path,1.);let rights=if writable{lopdf::encryption::Permissions::MODIFIABLE}else{lopdf::encryption::Permissions::PRINTABLE};
  let original=protected_fixture::encrypt(&fs::read(&path).unwrap(),rights);fs::write(&path,&original).unwrap();let source=e.open_document_with_password(&path,"reader").unwrap();let ids=[0,0,0];
  if !writable{assert!(e.list_form_paragraph_runs(&source.id,0,&ids,"First paragraph line").is_err());}
  else{
   let selected=e.inspect_form_styled_paragraph(&source.id,0,&ids,&[0,1]).unwrap();let mut runs=selected.runs;runs[0].text="New ".into();
   let request=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:selected.object_indices,expected_text:selected.text,runs,style_changes:vec![],bounds:AnnotationRect{x:45.,y:220.,width:240.,height:95.},alignment:ParagraphAlignment::Left,line_spacing:1.3};
   let changed=e.preview_form_styled_paragraph_occurrence(&source.id,&[0,1],0,&ids,&request).unwrap();assert_eq!(changed.document.protection,source.protection);assert!(e.extract_text(&changed.document.id,0).is_err());
   let plan=PagePlan{id:"p".into(),source_id:changed.document.id.clone(),page_index:0,width:changed.document.pages[0].width,height:changed.document.pages[0].height,rotation:0,crop:None,overlays:vec![]};assert!(e.export_pdf(ExportRequest{pages:vec![plan],flatten:false},&path).is_err());
   let output=temp.path().join("protected-saved.pdf");save(&e,&changed.document,&output);
   for password in ["reader","owner"]{assert!(folio_engine::protection::unlock_pdf(&fs::read(&output).unwrap(),Some(password)).is_ok());}
   e.close_document(&changed.document.id).unwrap();
  }
  assert_eq!(fs::read(&path).unwrap(),original);e.close_document(&source.id).unwrap();
 }
}

#[test]
fn form_paragraph_second_invocation_substitutes_font_and_preserves_fillable_widget(){
 use folio_engine::{StyledParagraphStyleChange,FillFormFieldRequest,FormFieldValue};
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("widget.pdf");fixture(&path,1.);
 let mut pdf=Document::load(&path).unwrap();
 for object in pdf.objects.values_mut(){if let Ok(stream)=object.as_stream_mut(){if stream.content.starts_with(b"BT /F2 14"){
  stream.set_plain_content(String::from_utf8(stream.content.clone()).unwrap().replace("/F2 14","/F1 12").replace("5 25 Tm","5 27 Tm").into_bytes());
 }}}
 let page=*pdf.get_pages().values().next().unwrap();let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
 let widget=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Tx","T"=>Object::string_literal("Keep field"),"V"=>Object::string_literal("Original"),"DA"=>Object::string_literal("/Helv 12 Tf 0 g"),"Rect"=>vec![30.into(),100.into(),180.into(),125.into()],"P"=>page,"F"=>4});
 pdf.get_dictionary_mut(page).unwrap().get_mut(b"Annots").unwrap().as_array_mut().unwrap().push(widget.into());
 let form=pdf.add_object(dictionary!{"Fields"=>vec![Object::Reference(widget)],"DR"=>dictionary!{"Font"=>dictionary!{"Helv"=>font}},"DA"=>Object::string_literal("/Helv 12 Tf 0 g")});pdf.catalog_mut().unwrap().set("AcroForm",form);pdf.save(&path).unwrap();
 let original=fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();let ids=[1,0,0];
 let listing=e.list_form_paragraph_runs(&source.id,0,&ids,"First paragraph line").unwrap();assert_eq!(listing.anchor_index,1);assert_eq!(listing.runs.len(),2);
 let selected=e.suggest_form_styled_paragraph(&source.id,0,&ids,listing.anchor_index,&listing.runs[0].text).unwrap();assert_eq!(selected.object_indices,vec![1,2]);
 let custom=e.fonts().register(fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
 let mut request=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:selected.object_indices,expected_text:selected.text,runs:vec![StyledParagraphRun{text:"Café in a substituted paragraph".into(),style_id:0}],style_changes:vec![StyledParagraphStyleChange{style_id:0,source_style_id:0,font_size:Some(14.),color:Some("#225588".into()),font_id:Some(custom.id.clone())}],bounds:AnnotationRect{x:355.,y:220.,width:240.,height:95.},alignment:ParagraphAlignment::Left,line_spacing:1.3};
 request.bounds=e.fit_form_styled_paragraph(&source.id,0,&ids,&request).unwrap();
 let edited=e.preview_form_styled_paragraph_occurrence(&source.id,&[0,1],0,&ids,&request).unwrap();
 let remaining=e.list_occurrence_form_text_runs(&edited.document.id,0).unwrap();assert_eq!(remaining.runs.len(),2);assert!(remaining.runs[0].bounds.x<100.);
 let output=temp.path().join("saved.pdf");save(&e,&edited.document,&output);let reopened=e.open_document(&output).unwrap();
 let field=e.list_form_fields(&reopened.id,0).unwrap().remove(0);assert_eq!(field.value,FormFieldValue::Text{text:"Original".into()});
 let filled=e.fill_form_field(&reopened.id,FillFormFieldRequest{page_index:0,annotation_index:field.annotation_index,expected_value:field.value,value:FormFieldValue::Text{text:"Still fillable".into()}}).unwrap();
 let saved=temp.path().join("filled.pdf");save(&e,&filled,&saved);let final_doc=e.open_document(&saved).unwrap();assert_eq!(e.list_form_fields(&final_doc.id,0).unwrap()[0].value,FormFieldValue::Text{text:"Still fillable".into()});
 assert_eq!(fs::read(&path).unwrap(),original);e.fonts().remove(&custom.id).unwrap();
 for id in [&final_doc.id,&filled.id,&reopened.id,&edited.document.id,&source.id]{e.close_document(id).unwrap();}
}

#[test]
fn form_paragraph_combines_crop_rotation_units_and_duplicate_saved_replay(){
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for rotation in [0,90,180,270] {for unit in [1.,1.7] {
  let input=temp.path().join(format!("combined-r{rotation}-u{unit}.pdf"));fixture(&input,unit);
  let mut pdf=Document::load(&input).unwrap();let page=*pdf.get_pages().values().next().unwrap();let page=pdf.get_dictionary_mut(page).unwrap();
  page.set("MediaBox",vec![(-20).into(),(-10).into(),820.into(),510.into()]);page.set("CropBox",vec![10.into(),20.into(),790.into(),480.into()]);page.set("Rotate",rotation);pdf.save(&input).unwrap();
  let original=fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();let size=&source.pages[0];
  let expected=if rotation==90||rotation==270{(460.*unit,780.*unit)}else{(780.*unit,460.*unit)};
  assert!((size.width-expected.0).abs()<0.001&&(size.height-expected.1).abs()<0.001,"r{rotation} u{unit}: {size:?}");
  let original_forms=e.list_occurrence_form_text_runs(&source.id,0).unwrap();let ids=&original_forms.runs[0].object_path;
  let listing=e.list_form_paragraph_runs(&source.id,0,ids,&original_forms.runs[0].text).unwrap();let indices=listing.runs.iter().map(|r|r.object_index).collect::<Vec<_>>();
  let selected=e.inspect_form_styled_paragraph(&source.id,0,ids,&indices).unwrap();let mut runs=selected.runs;runs.last_mut().unwrap().text.push_str(" this added text wraps through a few more lines");
  let mut request=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:selected.object_indices,expected_text:selected.text,runs,style_changes:vec![],bounds:selected.bounds,alignment:ParagraphAlignment::Left,line_spacing:1.3};
  request.bounds=e.fit_form_styled_paragraph(&source.id,0,ids,&request).unwrap();
  let changed=e.preview_form_styled_paragraph_occurrence(&source.id,&[0,1,0],2,ids,&request).unwrap();assert_eq!(changed.page_indices,vec![0,1,2]);
  assert!((changed.document.pages[2].width-size.width).abs()<0.001&&(changed.document.pages[2].height-size.height).abs()<0.001);
  for index in [0,1]{assert_eq!(e.render_page(&source.id,index,800).unwrap(),e.render_page(&changed.document.id,index,800).unwrap(),"untouched page{index} r{rotation} u{unit}");}
  let remaining=e.list_occurrence_form_text_runs(&changed.document.id,2).unwrap();assert_eq!(remaining.runs.len(),2);
  for (before,after) in original_forms.runs[2..].iter().zip(&remaining.runs){
   assert_eq!(before.text,after.text);assert!((before.font_size-after.font_size).abs()<0.001);
   let a=&before.bounds;let b=&after.bounds;for delta in [a.x-b.x,a.y-b.y,a.width-b.width,a.height-b.height]{assert!(delta.abs()<0.001,"sibling geometry r{rotation} u{unit}");}
  }
  let output=temp.path().join(format!("saved-r{rotation}-u{unit}.pdf"));save(&e,&changed.document,&output);let reopened=e.open_document(&output).unwrap();
  for index in 0..3{assert_eq!(e.render_page(&changed.document.id,index,800).unwrap(),e.render_page(&reopened.id,index,800).unwrap());}
  let text=e.list_occurrence_text_runs(&reopened.id,2).unwrap();let anchor=text.runs.iter().find(|r|r.text.starts_with("First")).unwrap();let saved=e.suggest_occurrence_styled_paragraph(&reopened.id,2,anchor.object_index,&anchor.text).unwrap();assert!(saved.text.contains("this added text"));
  let unchanged=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:saved.object_indices,expected_text:saved.text,runs:saved.runs,style_changes:saved.styles.iter().map(|s|folio_engine::StyledParagraphStyleChange{style_id:s.style_id,source_style_id:s.style_id,font_size:Some(s.font_size),color:None,font_id:None}).collect(),bounds:saved.bounds,alignment:saved.alignment.unwrap_or(ParagraphAlignment::Left),line_spacing:saved.line_spacing.unwrap_or(1.3)};
  let noop=e.preview_styled_paragraph_occurrence(&reopened.id,&[0,1,2],2,&unchanged).unwrap();
  for index in 0..3{assert_eq!(e.render_page(&reopened.id,index,800).unwrap(),e.render_page(&noop.document.id,index,800).unwrap(),"unchanged replay page{index} r{rotation} u{unit}");}
  assert_eq!(fs::read(&input).unwrap(),original);
  for id in [&noop.document.id,&reopened.id,&changed.document.id,&source.id]{e.close_document(id).unwrap();}
 }}
}

#[test]
fn form_paragraph_accepts_reportlab_draw_string_text_lines_and_explicit_lines(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let fixtures=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/form-text-flow");let mut failures=vec![];
 for mode in ["draw-string","text-lines","explicit-lines"] {
  let input=fixtures.join(format!("{mode}-original.pdf"));let original=fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();let mut owned=vec![];
  let result=(||->Result<(),Box<dyn std::error::Error>>{
   let forms=e.list_occurrence_form_text_runs(&source.id,0)?;assert_eq!(forms.runs.len(),4,"{mode}");let anchor=&forms.runs[0];
   let scope=e.list_form_paragraph_runs(&source.id,0,&anchor.object_path,&anchor.text)?;assert_eq!(scope.runs.len(),2,"{mode}");
   let selected=e.inspect_form_styled_paragraph(&source.id,0,&anchor.object_path,&scope.runs.iter().map(|r|r.object_index).collect::<Vec<_>>())?;
   let mut runs=selected.runs;runs.last_mut().unwrap().text.push_str(" with added paragraph text for reflow");
   let mut draft=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:selected.object_indices,expected_text:selected.text,runs,style_changes:vec![],bounds:selected.bounds,alignment:ParagraphAlignment::Left,line_spacing:1.3};
   draft.bounds=e.fit_form_styled_paragraph(&source.id,0,&anchor.object_path,&draft)?;
   let changed=e.preview_form_styled_paragraph_occurrence(&source.id,&[0,1],0,&anchor.object_path,&draft)?;owned.push(changed.document.id.clone());
   assert_eq!(e.render_page(&source.id,1,800)?,e.render_page(&changed.document.id,1,800)?);
   let siblings=e.list_occurrence_form_text_runs(&changed.document.id,0)?;assert_eq!(siblings.runs.len(),2);for (a,b) in forms.runs[2..].iter().zip(&siblings.runs){assert_eq!(a.text,b.text);}
   let output=temp.path().join(format!("{mode}-edited.pdf"));save(&e,&changed.document,&output);let reopened=e.open_document(&output)?;owned.push(reopened.id.clone());
   for page in 0..2{assert_eq!(e.render_page(&changed.document.id,page,800)?,e.render_page(&reopened.id,page,800)?);}
   let runs=e.list_occurrence_text_runs(&reopened.id,0)?.runs;let first=runs.iter().find(|r|r.text.starts_with("First")).unwrap();let saved=e.suggest_occurrence_styled_paragraph(&reopened.id,0,first.object_index,&first.text)?;assert!(saved.text.contains("added paragraph text"));
   let unchanged=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:saved.object_indices,expected_text:saved.text,runs:saved.runs,style_changes:vec![],bounds:saved.bounds,alignment:saved.alignment.unwrap_or(ParagraphAlignment::Left),line_spacing:saved.line_spacing.unwrap_or(1.3)};
   let noop=e.preview_styled_paragraph_occurrence(&reopened.id,&[0,1],0,&unchanged)?;owned.push(noop.document.id.clone());assert_eq!(e.render_page(&reopened.id,0,800)?,e.render_page(&noop.document.id,0,800)?);
   if let Some(root)=std::env::var_os("FOLIO_FORM_PARAGRAPH_CORPUS"){let root=PathBuf::from(root);fs::create_dir_all(&root)?;fs::copy(&input,root.join(format!("{mode}-original.pdf")))?;fs::copy(&output,root.join(format!("{mode}-edited.pdf")))?;}
   Ok(())
  })();
  if let Err(error)=result{failures.push(format!("{mode}: {error}"));}
  assert_eq!(fs::read(&input).unwrap(),original);for id in owned{e.close_document(&id).unwrap();}e.close_document(&source.id).unwrap();
 }
 assert!(failures.is_empty(),"{}",failures.join("\n"));
}

#[test]
fn form_paragraph_accepts_platypus_fonts_colors_and_imported_page_wrappers(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let fixtures=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/form-paragraph");let mut failures=vec![];
 for mode in ["standard","embedded","mixed","imported"] {
  let input=fixtures.join(format!("{mode}.pdf"));let original=fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();let mut owned=vec![];let mut stage="discovery";
  let result=(||->Result<(),Box<dyn std::error::Error>>{
   let forms=e.list_occurrence_form_text_runs(&source.id,0)?;
   let anchor=forms.runs.iter().find(|r|r.bounds.x<300.&&r.text.starts_with("First")).ok_or_else(||format!("no paragraph anchor: {forms:?}"))?;
   stage="preparation";let scope=e.list_form_paragraph_runs(&source.id,0,&anchor.object_path,&anchor.text)?;
   let indices=scope.runs.iter().filter(|r|r.bounds.x<300.).map(|r|r.object_index).collect::<Vec<_>>();assert!(indices.len()>1,"{mode}");
   stage="inspection";let selected=e.inspect_form_styled_paragraph(&source.id,0,&anchor.object_path,&indices)?;assert!(selected.styles.len()>=2,"{mode}: source colors remain distinct");
   let mut runs=selected.runs;runs.last_mut().unwrap().text.push_str(" with added paragraph text for reflow");
   let mut draft=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:selected.object_indices,expected_text:selected.text,runs,style_changes:vec![],bounds:selected.bounds,alignment:ParagraphAlignment::Left,line_spacing:1.3};
   stage="fitting";draft.bounds=e.fit_form_styled_paragraph(&source.id,0,&anchor.object_path,&draft)?;
   stage="publication";let changed=e.preview_form_styled_paragraph_occurrence(&source.id,&[0,1],0,&anchor.object_path,&draft)?;owned.push(changed.document.id.clone());
   assert_eq!(e.render_page(&source.id,1,800)?,e.render_page(&changed.document.id,1,800)?);
   stage="save/reopen";let output=temp.path().join(format!("{mode}-edited.pdf"));save(&e,&changed.document,&output);let reopened=e.open_document(&output)?;owned.push(reopened.id.clone());
   for page in 0..2{assert_eq!(e.render_page(&changed.document.id,page,800)?,e.render_page(&reopened.id,page,800)?);}
   stage="saved paragraph discovery";let runs=e.list_occurrence_text_runs(&reopened.id,0)?.runs;let first=runs.iter().find(|r|r.bounds.x<300.&&r.text.starts_with("First")).unwrap();let saved=e.suggest_occurrence_styled_paragraph(&reopened.id,0,first.object_index,&first.text)?;assert!(saved.text.contains("added paragraph text"));
   let unchanged=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:saved.object_indices,expected_text:saved.text,runs:saved.runs,style_changes:vec![],bounds:saved.bounds,alignment:saved.alignment.unwrap_or(ParagraphAlignment::Left),line_spacing:saved.line_spacing.unwrap_or(1.3)};
   stage="unchanged replay";let noop=e.preview_styled_paragraph_occurrence(&reopened.id,&[0,1],0,&unchanged)?;owned.push(noop.document.id.clone());assert_eq!(e.render_page(&reopened.id,0,800)?,e.render_page(&noop.document.id,0,800)?);
   if let Some(root)=std::env::var_os("FOLIO_FORM_PARAGRAPH_CORPUS"){let root=PathBuf::from(root);fs::create_dir_all(&root)?;fs::copy(&input,root.join(format!("{mode}-original.pdf")))?;fs::copy(&output,root.join(format!("{mode}-edited.pdf")))?;save(&e,&noop.document,&root.join(format!("{mode}-noop.pdf")));}
   Ok(())
  })();
  if let Err(error)=result{failures.push(format!("{mode}/{stage}: {error}"));}
  assert_eq!(fs::read(&input).unwrap(),original);for id in owned{e.close_document(&id).unwrap();}e.close_document(&source.id).unwrap();
 }
 assert!(failures.is_empty(),"{}",failures.join("\n"));
}
