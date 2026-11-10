use folio_engine::{PdfEngine,StyledParagraphLayoutRequest,ParagraphAlignment};
use lopdf::{dictionary,Document,Object,Stream};
use std::path::{Path,PathBuf};

fn engine()->PdfEngine{PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture(path:&Path,rotation:i32,unit:f32,mixed:bool){
 let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
 let plain=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
 let bold=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica-Bold"});
 let content=if mixed {b"BT /F1 12 Tf 1 0 0 1 30 440 Tm (One ) Tj ET BT /F2 18 Tf 1 0 0 1 54 440 Tm (two) Tj ET BT /F1 12 Tf 1 0 0 1 280 180 Tm (Neighbor) Tj ET".as_slice()}
 else {b"BT /F1 12 Tf 1 0 0 1 30 440 Tm (One two) Tj ET BT /F1 12 Tf 1 0 0 1 280 180 Tm (Neighbor) Tj ET".as_slice()};
 let stream=pdf.add_object(Stream::new(dictionary!{},content.to_vec()));
 let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),400.into(),500.into()],"CropBox"=>vec![10.into(),20.into(),390.into(),480.into()],"Rotate"=>rotation,"UserUnit"=>Object::Real(unit),"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>plain,"F2"=>bold}},"Contents"=>stream});
 pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",root);pdf.save(path).unwrap();
}
fn check(mixed:bool,units:&[f32]){
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for &unit in units {for rotation in [0,90,180,270] {
  let input=temp.path().join("input.pdf");fixture(&input,rotation,unit,mixed);let original=std::fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();
  let indices=if mixed{vec![0,1]}else{vec![0]};let selected=e.inspect_styled_paragraph(&source.id,0,&indices).unwrap();
  let mut request=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:selected.object_indices,expected_text:selected.text,runs:selected.runs,style_changes:vec![],bounds:selected.bounds,alignment:ParagraphAlignment::Left,line_spacing:1.2};
  let layout=e.layout_styled_paragraph(&source.id,0,&request).unwrap();
  assert!((layout.lines[0].baseline-440.).abs()<0.001,"first baseline moved: mixed={mixed} rotation={rotation} unit={unit} baseline={}",layout.lines[0].baseline);
  request.runs[0].text=request.runs[0].text.replace("One","One this");
  request.bounds=e.fit_styled_paragraph(&source.id,0,&request).unwrap();
  assert!((e.layout_styled_paragraph(&source.id,0,&request).unwrap().lines[0].baseline-440.).abs()<0.001,"inserting a word changed the source baseline");
  let preview=e.preview_styled_paragraph(&source.id,0,&request).unwrap();let saved=temp.path().join("saved.pdf");std::fs::write(&saved,e.source_bytes(&preview.id).unwrap()).unwrap();
  let reopened=e.open_document(&saved).unwrap();let next_indices=e.list_text_runs(&reopened.id,0).unwrap().runs.iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect::<Vec<_>>();
  let remembered=e.inspect_styled_paragraph(&reopened.id,0,&next_indices).unwrap();assert_eq!(remembered.bounds,request.bounds);
  let again=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:remembered.object_indices,expected_text:remembered.text,runs:remembered.runs,style_changes:vec![],bounds:remembered.bounds,alignment:ParagraphAlignment::Left,line_spacing:1.2};
  assert!((e.layout_styled_paragraph(&reopened.id,0,&again).unwrap().lines[0].baseline-440.).abs()<0.001);
  let repeated=e.preview_styled_paragraph(&reopened.id,0,&again).unwrap();assert_eq!(e.render_page(&preview.id,0,600).unwrap(),e.render_page(&repeated.id,0,600).unwrap());
  assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),original.as_slice());assert_eq!(std::fs::read(&input).unwrap(),original);
  for id in [&source.id,&preview.id,&reopened.id,&repeated.id]{e.close_document(id).unwrap();}
 }}
}
#[test] fn fresh_uniform_paragraph_keeps_baseline_through_publication_and_reedit(){check(false,&[1.]);}
#[test] fn fresh_mixed_paragraph_keeps_baseline_when_initial_fitting_changes_first_line(){check(true,&[1.]);}
#[test] fn fresh_baseline_is_invariant_under_physical_page_scale(){check(false,&[1.7,2.]);check(true,&[1.7,2.]);}

#[test]
fn fresh_full_font_highlights_use_publication_metrics_for_the_source_baseline(){
 let e=engine();let temp=tempfile::tempdir().unwrap();
 let fixture=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/paragraphs/native-ink-v3.pdf");
 let original=std::fs::read(&fixture).unwrap();
 for (rotation,unit) in [(0,1.),(90,1.7),(180,1.),(270,2.)]{
  let mut pdf=Document::load_mem(&original).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
  let content=lopdf::content::Content::decode(&pdf.get_page_content(page)).unwrap();
  let source_baseline=content.operations.iter().find(|op|op.operator=="Tm").unwrap().operands[5].as_float().unwrap();
  let dictionary=pdf.get_dictionary_mut(page).unwrap();
  // Remove only Folio's hints so that inspection must derive a fresh area from
  // the real marked, embedded-font text instead of replaying remembered bounds.
  assert!(dictionary.remove(b"FolioStyledParagraphs").is_some());
  dictionary.set("Rotate",rotation);dictionary.set("UserUnit",Object::Real(unit));
  let input=temp.path().join(format!("fresh-marked-{rotation}.pdf"));pdf.save(&input).unwrap();
  let source=e.open_document(&input).unwrap();
  let indices=e.list_text_runs(&source.id,0).unwrap().runs.into_iter()
   .filter(|r|r.text!="Neighbor"&&!r.text.trim().is_empty()).map(|r|r.object_index).collect::<Vec<_>>();
  let s=e.inspect_styled_paragraph(&source.id,0,&indices).unwrap();
  assert!(s.styles.iter().any(|style|style.background_color.is_some()&&style.font_name.starts_with("Folio")));
  let request=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:s.object_indices,expected_text:s.text,runs:s.runs,style_changes:vec![],bounds:s.bounds,alignment:ParagraphAlignment::Left,line_spacing:1.2};
  let layout=e.layout_styled_paragraph(&source.id,0,&request).unwrap();
  assert!((layout.lines[0].baseline-source_baseline).abs()<0.001,
   "highlighted first baseline moved: rotation={rotation} unit={unit} source={source_baseline} actual={}",layout.lines[0].baseline);
  let preview=e.preview_styled_paragraph(&source.id,0,&request).unwrap();
  e.close_document(&preview.id).unwrap();e.close_document(&source.id).unwrap();
 }
 assert_eq!(std::fs::read(fixture).unwrap(),original);
}
