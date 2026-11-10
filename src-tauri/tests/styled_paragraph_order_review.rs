//! Review regressions for source cursor admission and bounded state reconstruction.
use folio_engine::{PdfEngine,ParagraphAlignment,StyledParagraphLayoutRequest};
use lopdf::{dictionary,Document,Stream};
use std::path::{Path,PathBuf};
fn engine()->PdfEngine{PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture(path:&Path,content:&str){
 let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
 let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
 let content=pdf.add_object(Stream::new(dictionary!{},content.as_bytes().to_vec()));
 let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),600.into(),800.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F"=>font}},"Contents"=>content});
 pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",root);pdf.save(path).unwrap();
}
fn text(e:&PdfEngine,id:&str)->String{e.page_text(id,0).unwrap().characters.into_iter().map(|c|c.text).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")}
fn draft(e:&PdfEngine,id:&str,old:&str,new:&str)->StyledParagraphLayoutRequest{
 let run=e.list_text_runs(id,0).unwrap().runs.into_iter().find(|r|r.text.trim()==old).unwrap();
 let s=e.inspect_styled_paragraph(id,0,&[run.object_index]).unwrap();
 let mut request=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:s.object_indices,expected_text:s.text,runs:s.runs,style_changes:vec![],bounds:s.bounds,alignment:ParagraphAlignment::Left,line_spacing:1.2};
 request.runs[0].text=new.into();request.bounds=e.fit_styled_paragraph(id,0,&request).unwrap();request
}
fn check(content:&str){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("source.pdf");fixture(&path,content);let bytes=std::fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();
 let expected=text(&e,&source.id).replace("Alpha beta","Alpha this beta");
 let edited=e.preview_styled_paragraph(&source.id,0,&draft(&e,&source.id,"Alpha beta","Alpha this beta")).unwrap();
 assert_eq!(text(&e,&edited.id),expected);assert_eq!(std::fs::read(path).unwrap(),bytes);
 e.close_document(&edited.id).unwrap();e.close_document(&source.id).unwrap();
}
#[test]fn leading_tj_adjustment_is_not_applied_twice_to_the_following_show(){
 check("BT /F 12 Tf 100 TL 50 700 Td (Before) Tj T* [-1000 (Alpha beta)] TJ [-10000] TJ (After) Tj T* (Last) Tj ET");
}
#[test]fn multiple_leading_adjustments_and_empty_strings_with_scaled_state_keep_cursor(){
 check("q 1.25 0 0 1 10 20 cm BT /F 12 Tf 80 Tz 100 TL 40 680 Td (Before) Tj T* [500 () -750 () -750 (Alpha beta)] TJ [-10000] TJ (After) Tj T* (Last) Tj ET Q");
}
#[test]fn renderer_disagreement_on_empty_strings_and_trailing_adjustment_remains_refused(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("source.pdf");
 fixture(&path,"q 1.25 0 0 1 10 20 cm BT /F 12 Tf 80 Tz 100 TL 40 680 Td (Before) Tj T* [500 () -750 () -750 (Alpha beta) 20] TJ [-10000] TJ (After) Tj T* (Last) Tj ET Q");
 let source=e.open_document(&path).unwrap();let original=e.source_bytes(&source.id).unwrap();let pixels=e.render_page(&source.id,0,800).unwrap();
 let error=e.preview_styled_paragraph(&source.id,0,&draft(&e,&source.id,"Alpha beta","Alpha this beta")).unwrap_err();
 assert!(error.to_string().contains("surrounding text"));assert_eq!(e.source_bytes(&source.id).unwrap(),original);assert_eq!(e.render_page(&source.id,0,800).unwrap(),pixels);e.close_document(&source.id).unwrap();
}
#[test]fn a_closed_unrelated_text_clip_does_not_block_normal_paragraph_editing(){
 check("q BT /F 12 Tf 7 Tr 50 700 Td (Clip) Tj ET Q BT /F 12 Tf 50 600 Td (Alpha beta) Tj ET BT /F 12 Tf 50 500 Td (After) Tj ET");
}
#[test]fn editing_successive_paragraphs_does_not_exponentially_copy_closed_state(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("many.pdf");
 let content=(0..12).map(|i|format!("BT /F 12 Tf 50 {} Td (Line{i:02} old) Tj ET\n",700-i*35)).collect::<String>();
 fixture(&path,&content);let original=std::fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();let mut current=source.id.clone();
 let mut expected=text(&e,&current);
 for index in 0..12{
  let old=format!("Line{index:02} old");let new=format!("Line{index:02} new");let request=draft(&e,&current,&old,&new);
  let edited=e.preview_styled_paragraph(&current,0,&request).unwrap();expected=expected.replace(&old,&new);assert_eq!(text(&e,&edited.id),expected);
  let pdf=Document::load_mem(&e.source_bytes(&edited.id).unwrap()).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
  let count=lopdf::content::Content::decode(&pdf.get_page_content(page)).unwrap().operations.len();
  assert!(count<=60*(index+2)*3,"edit {index} copied closed graphics histories: {count} operations");
  if current!=source.id{e.close_document(&current).unwrap();}current=edited.id;
 }
 assert_eq!(std::fs::read(path).unwrap(),original);e.close_document(&current).unwrap();e.close_document(&source.id).unwrap();
}
