use folio_engine::{PdfEngine,ParagraphAlignment,StyledParagraphLayoutRequest};
use lopdf::{dictionary,Document,Stream};
use std::path::{Path,PathBuf};

fn fixture(path:&Path,case:usize){
 let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
 let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
 let content=match case{
  0=>"BT /F 12 Tf 50 700 Td (Before) Tj ET BT /F 12 Tf 50 600 Td (Alpha beta) Tj ET BT /F 12 Tf 50 500 Td (After) Tj ET",
  1=>"BT /F 12 Tf 50 700 Td (Before) Tj 0 -100 Td (Alpha beta) Tj 0 -100 Td (After) Tj ET",
  2=>"BT /F 12 Tf 100 TL 50 700 Td (Before) Tj T* (a. ) Tj (Alpha beta) Tj T* (After) Tj ET",
  3=>"q 2 0 0 2 10 20 cm q BT /F 6 Tf 50 TL 20 340 Td (Before) Tj T* (a. ) Tj (Alpha beta) Tj T* (After) Tj ET Q Q",
  4=>"0 .5 .5 rg 300 300 20 20 re BT /F 12 Tf 50 700 Td (Before) Tj 0 -100 Td (Alpha beta) Tj 0 -100 Td (After) Tj ET f",
  5=>"q 1 1 0 rg 48 596 77 16 re f Q BT /F 12 Tf 50 700 Td (Before) Tj 0 -100 Td (Alpha beta) Tj 0 -100 Td (After) Tj ET",
  6=>"BT /F 12 Tf 100 TL 50 700 Td (Before) Tj T* (a. ) Tj (Alpha beta) Tj [-10000] TJ (After) Tj T* (Last) Tj ET",
  7=>"BT /F 12 Tf 100 TL 50 800 Td 2 0.5 (Before) \" (a. ) ' 2 Ts (Alpha beta) Tj [-10000] TJ (After) Tj T* (Last) Tj ET",
  8=>"q 1.25 0 0 1 10 20 cm BT /F 12 Tf 80 Tz 100 TL 40 680 Td (Before) Tj T* (a. ) Tj (Alpha beta) Tj T* (After) Tj ET Q",
  9=>"q 1 .1 0 1 0 0 cm BT /F 12 Tf 100 TL 1 -.1 0 1 50 695 Tm (Before) Tj T* (a. ) Tj (Alpha beta) Tj T* (After) Tj ET Q",
  _=>unreachable!(),
 };
 let stream=pdf.add_object(Stream::new(dictionary!{},content.as_bytes().to_vec()));
 let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),600.into(),800.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F"=>font}},"Contents"=>stream});
 pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());
 let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",root);pdf.save(path).unwrap();
}
fn text(e:&PdfEngine,id:&str)->String{e.page_text(id,0).unwrap().characters.into_iter().map(|c|c.text).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")}
fn check(case:usize){
 let e=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
 let temp=tempfile::tempdir().unwrap();let path=temp.path().join("source.pdf");fixture(&path,case);
 let original=std::fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();let before=text(&e,&source.id);
 let run=e.list_text_runs(&source.id,0).unwrap().runs.into_iter().find(|r|r.text.trim()=="Alpha beta").unwrap();
 let s=e.inspect_styled_paragraph(&source.id,0,&[run.object_index]).unwrap();
 let mut request=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:s.object_indices,expected_text:s.text,runs:s.runs,style_changes:vec![],bounds:s.bounds,alignment:ParagraphAlignment::Left,line_spacing:1.2};
 request.runs[0].text=request.runs[0].text.replace("Alpha","Alpha this");
 request.bounds=e.fit_styled_paragraph(&source.id,0,&request).unwrap();
 let edited=e.preview_styled_paragraph(&source.id,0,&request).unwrap();
 let expected=before.replace("Alpha beta","Alpha this beta");
 assert_eq!(text(&e,&edited.id),expected,"case {case}: paragraph moved out of source reading order");
 let saved=temp.path().join("saved.pdf");std::fs::write(&saved,e.source_bytes(&edited.id).unwrap()).unwrap();
 let reopened=e.open_document(&saved).unwrap();assert_eq!(text(&e,&reopened.id),expected);
 let r=e.list_text_runs(&reopened.id,0).unwrap().runs.into_iter().find(|r|r.text.contains("Alpha this")).unwrap();
 let s=e.suggest_styled_paragraph(&reopened.id,0,r.object_index,&r.text).unwrap();
 let again=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:s.object_indices,expected_text:s.text,runs:s.runs,style_changes:vec![],bounds:s.bounds,alignment:s.alignment.unwrap(),line_spacing:s.line_spacing.unwrap()};
 let repeated=e.preview_styled_paragraph(&reopened.id,0,&again).unwrap();assert_eq!(text(&e,&repeated.id),expected);
 assert_eq!(e.render_page(&edited.id,0,600).unwrap(),e.render_page(&repeated.id,0,600).unwrap());
 let count=|id:&str|{let pdf=Document::load_mem(&e.source_bytes(id).unwrap()).unwrap();let page=*pdf.get_pages().get(&1).unwrap();lopdf::content::Content::decode(&pdf.get_page_content(page)).unwrap().operations.len()};
 assert_eq!(count(&edited.id),count(&repeated.id),"re-edit accumulated state reconstruction");
 if let Some(directory)=std::env::var_os("FOLIO_PARAGRAPH_ORDER_DIR"){
  let directory=PathBuf::from(directory);std::fs::create_dir_all(&directory).unwrap();
  for (name,bytes) in [("source",original.clone()),("edited",e.source_bytes(&edited.id).unwrap().to_vec()),("repeated",e.source_bytes(&repeated.id).unwrap().to_vec())]{
   let target=directory.join(format!("case-{case}-{name}.pdf"));assert!(!target.exists(),"Preserve prior acceptance evidence");std::fs::write(target,bytes).unwrap();
  }
  let expected=serde_json::json!({"text":expected,"area":request.bounds,"originalArea":run.bounds});
  std::fs::write(directory.join(format!("case-{case}.json")),serde_json::to_vec_pretty(&expected).unwrap()).unwrap();
 }
 assert_eq!(std::fs::read(&path).unwrap(),original);assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),original.as_slice());
 for id in [&source.id,&edited.id,&reopened.id,&repeated.id]{e.close_document(id).unwrap();}
}
#[test]fn separate_text_objects_retain_source_order(){check(0);}
#[test]fn shared_text_object_restores_relative_line_position(){check(1);}
#[test]fn prefix_show_and_leading_keep_both_cursor_and_line_matrix(){check(2);}
#[test]fn nested_scaled_graphics_restore_source_order_and_cursor(){check(3);}
#[test]fn pending_path_survives_source_order_publication(){check(4);}
#[test]fn highlighted_paragraph_retains_order_paint_and_saved_replay(){check(5);}
#[test]fn later_show_and_line_movement_keep_the_original_cursor(){check(6);}
#[test]fn quote_spacing_and_text_rise_are_restored_without_copying_text(){check(7);}
#[test]fn horizontal_scale_compensated_by_graphics_transform_keeps_cursor(){check(8);}
#[test]fn compensated_shear_keeps_native_source_positions(){check(9);}
