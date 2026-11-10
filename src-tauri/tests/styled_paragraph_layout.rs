use folio_engine::{AnnotationRect,ParagraphAlignment,PdfEngine,StyledParagraphLayoutRequest,StyledParagraphRun};
use lopdf::{dictionary,Document,Stream};
use std::path::{Path,PathBuf};
fn engine()->PdfEngine{PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture(path:&Path,rotation:i32){
 let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
 let plain=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
 let bold=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica-Bold"});
 let stream=pdf.add_object(Stream::new(dictionary!{},b"BT /F1 12 Tf 50 500 Td (One ) Tj ET BT /F2 20 Tf 95 500 Td (Two) Tj ET".to_vec()));
 let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),500.into(),600.into()],"CropBox"=>vec![10.into(),20.into(),490.into(),580.into()],"Rotate"=>rotation,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>plain,"F2"=>bold}},"Contents"=>stream});
 pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",root);pdf.save(path).unwrap();
}
fn run(text:&str,style_id:usize)->StyledParagraphRun{StyledParagraphRun{text:text.into(),style_id}}
fn request()->StyledParagraphLayoutRequest{StyledParagraphLayoutRequest{
 baseline_anchor: None,
 style_changes:vec![],
 object_indices:vec![0,1],expected_text:"One Two".into(),
 runs:vec![run("pre",0),run("fix ",1),run("word",0)],
 bounds:AnnotationRect{x:40.,y:60.,width:70.,height:160.},
 alignment:ParagraphAlignment::Left,line_spacing:1.2,
}}
#[test]
fn lays_out_cross_style_words_with_real_font_metrics_without_mutating_source(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("layout.pdf");fixture(&path,0);
 let source=e.open_document(&path).unwrap();let bytes=e.source_bytes(&source.id).unwrap();let before=e.render_page(&source.id,0,600).unwrap();
 let layout=e.layout_styled_paragraph(&source.id,0,&request()).unwrap();
 assert_eq!(layout.lines.len(),2);assert_eq!(layout.lines[0].runs.iter().map(|r|r.text.as_str()).collect::<String>(),"prefix ");assert!(layout.lines[0].soft_wrapped);
 assert_eq!(layout.lines[0].runs.iter().map(|r|r.style_id).collect::<Vec<_>>(),vec![0,1]);assert_eq!(layout.lines[1].runs[0].text,"word");assert!(!layout.lines[1].soft_wrapped);
 assert!(layout.lines[0].ascent>0.&&layout.lines[1].descent>=0.);
 assert!(layout.lines[0].baseline-layout.lines[0].descent>=layout.lines[1].baseline+layout.lines[1].ascent-0.001);
 assert_eq!(layout.pdf_bounds,[50.,360.,120.,520.]);
 assert_eq!(e.source_bytes(&source.id).unwrap(),bytes);assert_eq!(e.render_page(&source.id,0,600).unwrap(),before);assert_eq!(std::fs::read(&path).unwrap(),bytes.as_ref());e.close_document(&source.id).unwrap();
}
#[test]
fn refuses_unknown_style_stale_text_overlong_words_and_bad_geometry_atomically(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("refuse.pdf");fixture(&path,0);let source=e.open_document(&path).unwrap();let bytes=e.source_bytes(&source.id).unwrap();
 let mut cases=Vec::new();let mut value=request();value.runs[1].style_id=99;cases.push(value);
 let mut value=request();value.expected_text="stale".into();cases.push(value);
 let mut value=request();value.bounds.width=30.;cases.push(value); // prefix cannot split at its style boundary
 let mut value=request();value.bounds.height=5.;cases.push(value);
 let mut value=request();value.bounds.x=f32::NAN;cases.push(value);
 let mut value=request();value.runs=vec![run("bad\r\nline",0)];cases.push(value);
 let mut value=request();value.runs=vec![run("bad\ttext",0)];cases.push(value);
 let mut value=request();value.line_spacing=f32::INFINITY;cases.push(value);
 for value in cases{assert!(e.layout_styled_paragraph(&source.id,0,&value).is_err());assert_eq!(e.source_bytes(&source.id).unwrap(),bytes);}
 e.close_document(&source.id).unwrap();
}
#[test]
fn rotation_and_crop_change_display_bounds_but_not_pdf_layout(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("rotation.pdf");let mut reference=None;
 for (rotation,bounds) in [(0,[40.,60.,70.,160.]),(90,[340.,40.,160.,70.]),(180,[370.,340.,70.,160.]),(270,[60.,370.,160.,70.])]{
  fixture(&path,rotation);let source=e.open_document(&path).unwrap();let mut value=request();value.bounds=AnnotationRect{x:bounds[0],y:bounds[1],width:bounds[2],height:bounds[3]};
  let actual=e.layout_styled_paragraph(&source.id,0,&value).unwrap();
  if let Some(expected)=&reference{assert_eq!(&actual,expected);}else{reference=Some(actual);}
  e.close_document(&source.id).unwrap();
 }
}
#[test]
fn embedded_subset_metrics_and_encoding_remain_bound_to_the_selected_style(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("embedded.pdf");
 let mut pdf=Document::load(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/embedded-text/reportlab-subset.pdf")).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
 let stream=pdf.add_object(Stream::new(dictionary!{},b"BT /F2+0 16 Tf 1 0 0 1 30 300 Tm (Caf\x01) Tj ET BT /F1 12 Tf 1 0 0 1 90 300 Tm (mix) Tj ET".to_vec()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);pdf.save(&path).unwrap();
 let source=e.open_document(&path).unwrap();let selected=e.inspect_styled_paragraph(&source.id,0,&[0,1]).unwrap();let bytes=e.source_bytes(&source.id).unwrap();
 let mut value=StyledParagraphLayoutRequest{baseline_anchor: None,style_changes:vec![],object_indices:vec![0,1],expected_text:selected.text,runs:vec![run("Café ",0),run("mix\n",1),run("old",0)],bounds:AnnotationRect{x:20.,y:30.,width:200.,height:160.},alignment:ParagraphAlignment::Right,line_spacing:1.2};
 let result=e.layout_styled_paragraph(&source.id,0,&value).unwrap();assert_eq!(result.lines.len(),2);assert!(result.lines.iter().all(|line|line.ascent>0.&&line.descent>=0.));assert_eq!(result.lines[0].runs[0].text,"Café ");
 value.runs[0].text="Ω".into();assert!(e.layout_styled_paragraph(&source.id,0,&value).is_err());
 assert_eq!(e.source_bytes(&source.id).unwrap(),bytes);e.close_document(&source.id).unwrap();
}
