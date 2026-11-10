use super::*;

fn fixture(unit:f32,rotation:i64,affine:bool)->Vec<u8>{
    let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
    let root_font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica-Bold"});
    let child_font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Times-Roman"});
    let collision=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
    let leaf=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),180.into(),60.into()],"Matrix"=>vec![1.into(),0.into(),0.into(),1.into(),3.into(),2.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>child_font}}},b"BT 1 0 0 1 5 35 Tm (Inherited) Tj ET BT /F1 10 Tf 1 0 0 1 5 15 Tm (Inner line) Tj ET".to_vec()));
    let matrix=if affine{vec![1.into(),0.1.into(),0.2.into(),1.into(),0.into(),0.into()]}else{vec![1.into(),0.into(),0.into(),1.into(),0.into(),0.into()]};
    let wrapper=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),210.into(),90.into()],"Matrix"=>matrix,"Resources"=>dictionary!{"XObject"=>dictionary!{"Leaf"=>leaf}}},b"q 1.1 0 0 0.9 0 0 cm /Leaf Do Q".to_vec()));
    let content=pdf.add_object(Stream::new(dictionary!{},b"0.2 0.3 0.4 rg BT /FolioFlat0 12 Tf 0.2 Tc 0.3 Tw 1 0 0 1 30 400 Tm (Root before) Tj ET q 1 0 0 1 40 220 cm /Wrapper Do Q q 1 0 0 1 350 220 cm /Wrapper Do Q BT 1 0 0 1 30 80 Tm (Root after) Tj ET".to_vec()));
    let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),800.into(),500.into()],"CropBox"=>vec![10.into(),20.into(),790.into(),480.into()],"Rotate"=>rotation,"UserUnit"=>unit,"Resources"=>dictionary!{"Font"=>dictionary!{"FolioFlat0"=>root_font,"FolioFlat1"=>collision,"FolioParagraph0"=>collision,"FolioParagraph1"=>collision},"XObject"=>dictionary!{"Wrapper"=>wrapper},"ProcSet"=>vec![Object::Name(b"PDF".to_vec()),Object::Name(b"Text".to_vec())]},"Contents"=>content});
    pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]}.into());
    let catalog=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",catalog);save(&mut pdf).unwrap()
}
fn runtime()->WorkerRuntime{WorkerRuntime::new(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),Arc::new(FontRegistry::new())).unwrap()}

#[test]
fn form_paragraph_projection_preserves_shared_graph_state_and_mapping(){
 let mut rt=runtime();
 for (unit,rotation,affine) in [(1.,0,false),(1.7,90,false),(1.,270,true)] {
  let bytes=fixture(unit,rotation,affine);let source=rt.register_generated_document(bytes.clone(),"Nested.pdf").unwrap();let count=rt.documents.len();
  let original=rt.editing_document(&source.id).unwrap().bytes().unwrap();
  let value=rt.with_form_paragraph_projection(&source.id,0,&[1,0,1],|rt,id,scope|{
   assert_eq!(scope.anchor_index,2);assert_eq!(scope.object_indices,vec![1,2]);
   let lease=rt.editing_document(id)?.bytes()?;let promoted=lease.as_ref();let pdf=Document::load_mem(promoted).unwrap();let page=*pdf.get_pages().values().next().unwrap();
   let operations=Content::decode(&pdf.get_page_content_with_limit(page,LIMIT).unwrap()).unwrap().operations;
   assert_eq!(operations.iter().filter(|op|op.operator=="Do").count(),1,"the second shared invocation stays a form");
   let resources=inherited_resources(&pdf,page).unwrap();let fonts=dictionary_ref(&pdf,resources.get(b"Font").unwrap()).unwrap();
   assert_eq!(dictionary_ref(&pdf,fonts.get(b"FolioFlat0").unwrap()).unwrap().get(b"BaseFont").unwrap().as_name().unwrap(),b"Helvetica-Bold");
   assert_eq!(dictionary_ref(&pdf,fonts.get(b"FolioFlat1").unwrap()).unwrap().get(b"BaseFont").unwrap().as_name().unwrap(),b"Courier");
   assert!(resources.has(b"ProcSet"));
   assert_eq!(dictionary_ref(&pdf,fonts.get(b"FolioParagraph0").unwrap()).unwrap().get(b"BaseFont").unwrap().as_name().unwrap(),b"Courier");
   let before=rt.editing_document(&source.id)?.pdf().unwrap().pages().get(0)?;let after=rt.editing_document(id)?.pdf().unwrap().pages().get(0)?;
   assert_eq!(raster(&before)?,raster(&after)?);assert!(same_glyphs(&physical_glyphs(&before)?,&physical_glyphs(&after)?));
   assert!(same_glyphs(&glyph_snapshot(&before)?,&glyph_snapshot(&after)?),"actual promotion must preserve generated separators too");
   let runs=snapshot(&after,Some(&FontProofs::read(promoted)?))?;assert_eq!(runs.len(),4);assert!(runs[2].text.contains("Inner line"));
   Ok(42)
  }).unwrap();assert_eq!(value,42);assert_eq!(rt.documents.len(),count);assert_eq!(rt.editing_document(&source.id).unwrap().bytes().unwrap(),original);
  rt.discard_unpublished_source(&source.id);
 }
}

#[test]
fn form_paragraph_projection_preserves_or_refuses_complete_generated_separators(){
 let mut rt=runtime();let mut accepted=0;
 for siblings in [false,true] { for (x,y) in [(5.,35.),(55.,35.),(65.,35.),(85.,35.),(5.,34.98),(5.,23.),(5.,15.)] {
  let mut pdf=Document::load_mem(&fixture(1.,0,false)).unwrap();
  let leaf=*pdf.objects.iter().find(|(_,v)|v.as_stream().is_ok_and(|s|s.content.starts_with(b"BT 1"))).unwrap().0;
  pdf.get_object_mut(leaf).unwrap().as_stream_mut().unwrap().set_plain_content(format!("BT 1 0 0 1 5 35 Tm (Inherited) Tj ET BT /F1 10 Tf 1 0 0 1 {x} {y} Tm (Inner line) Tj ET").into_bytes());
  if siblings {
   let wrapper=*pdf.objects.iter().find(|(_,v)|v.as_stream().is_ok_and(|s|s.content.starts_with(b"q 1.1"))).unwrap().0;
   let stream=pdf.get_object_mut(wrapper).unwrap().as_stream_mut().unwrap();stream.dict.set("BBox",vec![0.into(),0.into(),210.into(),200.into()]);
   stream.set_plain_content(b"/Leaf Do q 1 0 0 1 0 100 cm /Leaf Do Q".to_vec());
  }
  let source=rt.register_generated_document(save(&mut pdf).unwrap(),"Separators.pdf").unwrap();let count=rt.documents.len();
  let result=rt.with_form_paragraph_projection(&source.id,0,&[1,0,0],|rt,id,_|{
   let before=rt.editing_document(&source.id)?.pdf().unwrap().pages().get(0)?;let after=rt.editing_document(id)?.pdf().unwrap().pages().get(0)?;
   let a=glyph_snapshot(&before)?;let b=glyph_snapshot(&after)?;
   assert!(same_glyphs(&a,&b),"accepted changed generated separators at {x}, {y}, siblings={siblings}: counts {} vs {}, first difference {:?}",a.len(),b.len(),a.iter().zip(&b).find(|(a,b)|a!=b));
   assert!(raster(&before)?==raster(&after)?,"promotion changed pixels at {x}, {y}, siblings={siblings}");
   if !siblings&&x==5.&&y==35. {
    if let Some(output)=std::env::var_os("FOLIO_PARAGRAPH_PROJECTION_DIAGNOSTIC") {
     let output=PathBuf::from(output);std::fs::create_dir_all(&output).unwrap();
     std::fs::write(output.join("overlap-before.pdf"),rt.editing_document(&source.id)?.bytes().unwrap().as_ref()).unwrap();
     std::fs::write(output.join("overlap-after.pdf"),rt.editing_document(id)?.bytes().unwrap().as_ref()).unwrap();
    }
   }
   Ok(())
  });
  // Correct Form-space separator comparisons now preserve this formerly
  // refused case. The callback proves complete glyphs and exact pixels above.
  if !siblings&&x==5.&&y==35. {assert!(result.is_ok(),"faithful overlapping child baselines: {result:?}");}
  if result.is_ok(){accepted+=1;}
  assert_eq!(rt.documents.len(),count);rt.discard_unpublished_source(&source.id);
 }
 } assert!(accepted>0,"the proof must still admit faithful fixtures");
}

#[test]
fn form_paragraph_projection_releases_failed_callback_and_rejects_stale_paths(){
 let mut rt=runtime();let source=rt.register_generated_document(fixture(1.,0,false),"Nested.pdf").unwrap();let count=rt.documents.len();
 for path in [vec![],vec![1],vec![0,0],vec![1,99,1],vec![2,0,99]] {
  let result=rt.with_form_paragraph_projection(&source.id,0,&path,|_,_,_|Ok(()));assert!(result.is_err(),"{path:?}");assert_eq!(rt.documents.len(),count);
 }
 let result:EngineResult<()>=rt.with_form_paragraph_projection(&source.id,0,&[1,0,0],|_,_,_|Err(invalid("test callback failure")));
 assert!(result.unwrap_err().to_string().contains("test callback failure"));assert_eq!(rt.documents.len(),count);
}

#[test]
fn form_paragraph_projection_metadata_is_not_retained_for_ordinary_form_edits(){
 let model=Model::read(&fixture(1.,0,false)).unwrap();
 assert!(model.flat_operations.is_empty()&&model.flat_fonts.is_empty()&&model.root_invocations.is_empty(),"ordinary single-text form edits must not retain paragraph-only copies");
}

#[test]
fn form_paragraph_projection_rejects_clips_cycles_and_oversized_direct_resources(){
 let mut rt=runtime();
 for case in ["clip","cycle","resources"] {
  let mut pdf=Document::load_mem(&fixture(1.,0,false)).unwrap();let page=*pdf.get_pages().values().next().unwrap();
  if case=="resources" {pdf.get_dictionary_mut(page).unwrap().get_mut(b"Resources").unwrap().as_dict_mut().unwrap().set("Unused",Object::String(vec![b'x';17*1024*1024],StringFormat::Literal));}
  else {
   let leaf=*pdf.objects.iter().find(|(_,v)|v.as_stream().is_ok_and(|s|s.content.starts_with(b"BT 1"))).unwrap().0;
   let stream=pdf.get_object_mut(leaf).unwrap().as_stream_mut().unwrap();
   if case=="clip"{stream.dict.set("BBox",vec![0.into(),0.into(),5.into(),5.into()]);}
   else {stream.set_plain_content(b"/Self Do".to_vec());stream.dict.set("Resources",dictionary!{"XObject"=>dictionary!{"Self"=>leaf}});}
  }
  // Raw graph/clip validation is exercised before any projected source survives.
  let bytes=save(&mut pdf).unwrap();
  if case=="cycle"||case=="resources" {assert!(crate::form_graph::preflight(&bytes).is_err(),"{case}");continue;}
  let source=rt.register_generated_document(bytes,"Clipped.pdf").unwrap();let count=rt.documents.len();
  assert!(rt.with_form_paragraph_projection(&source.id,0,&[1,0,0],|_,_,_|Ok(())).is_err());assert_eq!(rt.documents.len(),count);rt.discard_unpublished_source(&source.id);
 }
}
