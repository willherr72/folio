use super::*;

fn glyph(text:&str,bounds:[f32;4])->Glyph{Glyph{text:text.into(),position:[bounds[0],bounds[1],bounds[0],bounds[1],bounds[2],bounds[3]]}}
#[test]
fn glyph_coverage_uses_ink_not_multiline_unions_and_preserves_partial_ranges(){
 let mark=Rectangle{object_index:1,bounds:[10.,10.,50.,30.],color:[255,255,0]};
 let glyphs=HashMap::from([(2,vec![glyph("A",[12.,12.,20.,25.]),glyph(" ",[0.;4]),glyph("B",[60.,12.,68.,25.]),glyph("C",[15.,40.,23.,53.])]),(3,vec![glyph("D",[0.,12.,8.,25.]),glyph("E",[52.,12.,60.,25.])])]);
 let covered=coverage(&mark,&glyphs).unwrap().unwrap();assert_eq!(covered.len(),1);assert_eq!(covered[0].object_index,2);assert_eq!(covered[0].glyph_indices,vec![0]);assert!(!covered[0].whole_visible_run);
}
#[test]
fn rejects_foreground_partial_visible_glyphs_invalid_geometry_and_ambiguous_marks(){
 let mark=Rectangle{object_index:1,bounds:[10.,10.,50.,30.],color:[255,255,0]};
 for (index,bounds) in [(2,[8.,12.,20.,25.]),(0,[12.,12.,20.,25.]),(1,[12.,12.,20.,25.])]{assert!(coverage(&mark,&HashMap::from([(index,vec![glyph("A",bounds)])])).unwrap().is_none());}
 assert!(coverage(&mark,&HashMap::from([(2,vec![glyph("A",[f32::NAN,12.,20.,25.])])])).is_err());
 let glyphs=HashMap::from([(3,vec![glyph("A",[12.,12.,20.,25.])])]);
 assert!(classify(vec![mark.clone(),Rectangle{object_index:2,..mark}],&glyphs).unwrap().is_empty());
}
#[test]
fn whitespace_is_not_coverage_and_limits_are_atomic(){
 let mark=Rectangle{object_index:0,bounds:[10.,10.,50.,30.],color:[255,255,0]};
 assert!(coverage(&mark,&HashMap::from([(1,vec![glyph(" ",[10.,10.,20.,20.])])])).unwrap().is_none());
 let glyphs=HashMap::from([(1,vec![glyph("A",[12.,12.,20.,25.]);MAX_GLYPHS+1])]);assert!(classify(vec![mark.clone()],&glyphs).is_err());
 let glyphs=HashMap::from([(1,vec![glyph("A",[12.,12.,20.,25.]);MAX_COVERED_GLYPHS+1])]);assert!(classify(vec![mark.clone()],&glyphs).is_err());
 assert!(classify(vec![mark;MAX_RECTANGLES+1],&HashMap::new()).is_err());
}
#[test]
fn logical_background_ranges_require_exact_glyph_mapping_and_complete_selection(){
 let glyphs=HashMap::from([(3,vec![glyph("A",[0.,0.,1.,1.]),glyph("é",[2.,0.,3.,1.]),glyph(" ",[0.;4]),glyph("é",[4.,0.,5.,1.]),glyph("B",[6.,0.,7.,1.]),glyph("é",[8.,0.,9.,1.])])]);
 let source=HashMap::from([(3,"Aé éBé".to_string())]);let covered=vec![Coverage{object_index:3,glyph_indices:vec![1,3,5],whole_visible_run:false}];
 let spans=logical_ranges(&covered,&glyphs,&source,&[3]).unwrap();assert_eq!(spans.iter().map(|s|(s.object_index,s.byte_start,s.byte_end)).collect::<Vec<_>>(),vec![(3,1,6),(3,7,9)]);
 assert!(logical_ranges(&covered,&glyphs,&source,&[2]).is_err());
 assert!(logical_ranges(&covered,&glyphs,&HashMap::from([(3,"Aé éBdifferent".to_string())]),&[3]).is_err());
 let invalid=vec![Coverage{object_index:3,glyph_indices:vec![1,1],whole_visible_run:false}];assert!(logical_ranges(&invalid,&glyphs,&source,&[3]).is_err());
}

fn bound_mark(object_index:usize,paint_operator:usize,spans:Vec<SourceSpan>)->BoundHighlight{
 BoundHighlight{cell:None,rectangle:Rectangle{object_index,bounds:[0.,0.,100.,20.],color:[255,255,0]},paint_operator,spans}
}
fn span(object_index:usize,byte_start:usize,byte_end:usize)->SourceSpan{SourceSpan{object_index,byte_start,byte_end}}
fn attribute_members(members:&[(usize,&str,usize)],marks:&[BoundHighlight])->EngineResult<ParagraphAttribution>{
 let mut builder=paragraph_spans::Builder::default();let mut source=HashMap::new();
 for &(id,text,style) in members{builder.push_source(text,style,id);source.insert(id,text.to_string());}
 let assembled=builder.finish();let logical=assembled.runs.iter().map(|r|r.text.as_str()).collect::<String>();
 let ranges=project_ranges(&logical,&assembled.sources,&source,marks)?;
 attribute_runs(&assembled.runs,&logical,&ranges)
}
#[test]
fn attributed_backgrounds_split_exact_occurrences_and_retain_font_and_mark_identity(){
 let marks=vec![bound_mark(0,4,vec![span(3,3,5),span(4,0,1)]),bound_mark(1,9,vec![span(3,6,8)])];
 let source=vec![(3,"é é é",0),(4,"A B",1),(5,"plain",0)];
 let result=attribute_members(&source,&marks).unwrap();
 assert_eq!(result.styles.iter().map(|s|(s.source_style_id,s.background_id)).collect::<Vec<_>>(),vec![(0,None),(0,Some(0)),(0,Some(1)),(1,Some(0)),(1,None)]);
 assert_eq!(result.runs.iter().map(|r|(r.text.as_str(),r.style_id)).collect::<Vec<_>>(),vec![("é ",0),("é",1),(" ",0),("é",2),("A",3),(" B",4),("plain",0)]);
 assert_eq!(result.runs.iter().map(|r|r.text.as_str()).collect::<String>(),source.iter().map(|m|m.1).collect::<String>());
}
#[test]
fn attribution_rejects_ambiguous_ranges_and_counts_active_palette_entries(){
 let source=vec![(3,"é A",0)];
 for spans in [vec![span(3,1,2)],vec![span(3,0,0)],vec![span(3,0,5)],vec![span(7,0,1)],vec![span(3,0,2),span(3,0,2)]]{
  assert!(attribute_members(&source,&[bound_mark(0,4,spans)]).is_err());
 }
 assert!(attribute_members(&source,&[bound_mark(0,4,vec![span(3,0,2)]),bound_mark(1,9,vec![span(3,0,2)])]).is_err());
 assert!(attribute_members(&source,&[bound_mark(0,4,vec![span(3,0,2)]),bound_mark(1,4,vec![span(3,3,4)])]).is_err());
 assert!(attribute_members(&[(3,"A",0),(3,"A",0)],&[]).is_err());
 let source=(0..63).map(|i|(100+i,"AB",i)).collect::<Vec<_>>();
 assert_eq!(attribute_members(&source,&[bound_mark(0,4,vec![span(100,0,1)])]).unwrap().styles.len(),64);
 let source=(0..64).map(|i|(100+i,"AB",i)).collect::<Vec<_>>();
 assert!(attribute_members(&source,&[bound_mark(0,4,vec![span(100,0,1)])]).is_err());
 assert_eq!(attribute_members(&source,&[bound_mark(0,4,vec![span(100,0,2)])]).unwrap().styles.len(),64);
 let huge="A".repeat(4097);assert!(attribute_members(&[(3,huge.as_str(),0)],&[]).is_err());
}

#[test]
fn paragraph_ranges_follow_source_ownership_and_bridge_only_the_same_mark(){
 let mut builder=paragraph_spans::Builder::default();builder.push_source(" é é  ",0,3);builder.trim_end_spaces();builder.push(" ",0);builder.push_source("é",1,4);builder.push(" ",1);builder.push_source("é",0,5);
 let assembled=builder.finish();let logical=assembled.runs.iter().map(|r|r.text.as_str()).collect::<String>();
 let source=HashMap::from([(3," é é  ".to_string()),(4,"é".to_string()),(5,"é".to_string())]);
 let marks=vec![bound_mark(0,4,vec![span(3,4,6),span(4,0,2)]),bound_mark(1,9,vec![span(5,0,2)])];
 let projected=project_ranges(&logical,&assembled.sources,&source,&marks).unwrap();
 assert_eq!(projected.iter().map(|s|(s.start,s.end,s.background_id)).collect::<Vec<_>>(),vec![(3,8,0),(9,11,1)]);
 let attributed=attribute_runs(&assembled.runs,&logical,&projected).unwrap();
 assert_eq!(attributed.styles.iter().map(|s|(s.source_style_id,s.background_id)).collect::<Vec<_>>(),vec![(0,None),(0,Some(0)),(1,Some(0)),(1,None),(0,Some(1))]);
 assert_eq!(attributed.runs.iter().map(|r|(r.text.as_str(),r.style_id)).collect::<Vec<_>>(),vec![("é ",0),("é ",1),("é",2),(" ",3),("é",4)]);
 let mut forged=assembled.sources.clone();forged[1].logical_start=9;assert!(project_ranges(&logical,&forged,&source,&marks).is_err());
 let mut duplicate=assembled.sources.clone();duplicate.push(duplicate[0].clone());assert!(project_ranges(&logical,&duplicate,&source,&marks).is_err());
}
#[test]
fn logical_attribution_limits_active_combinations_after_trimming(){
 let mut builder=paragraph_spans::Builder::default();builder.push_source(" A",0,100);
 for id in 1..64{builder.push_source("B",id,100+id);}
 let assembled=builder.finish();let logical=assembled.runs.iter().map(|r|r.text.as_str()).collect::<String>();
 let mut source=HashMap::from([(100," A".to_string())]);for id in 1..64{source.insert(100+id,"B".to_string());}
 let marks=vec![bound_mark(0,4,vec![span(100,1,2)])];
 let projected=project_ranges(&logical,&assembled.sources,&source,&marks).unwrap();
 assert_eq!(attribute_runs(&assembled.runs,&logical,&projected).unwrap().styles.len(),64);
 let runs=vec![StyledParagraphRun{text:"ABA".into(),style_id:0}];
 let ranges=vec![LogicalRange{start:1,end:2,background_id:0}];
 assert_eq!(attribute_runs(&runs,"ABA",&ranges).unwrap().runs.len(),3);
 assert!(attribute_runs(&runs,"different",&ranges).is_err());
 assert!(attribute_runs(&runs,"ABA",&[LogicalRange{start:0,end:2,background_id:0},LogicalRange{start:1,end:3,background_id:1}]).is_err());
}

fn fixture(background:&str,foreground:bool,rotation:i32)->Vec<u8>{
 fixture_text(background,foreground,rotation,"BT /F1 12 Tf 1 0 0 1 50 200 Tm (Mark) Tj ET BT /F1 12 Tf 1 0 0 1 50 100 Tm (Neighbor) Tj ET")
}
fn fixture_text(background:&str,foreground:bool,rotation:i32,text:&str)->Vec<u8>{
 let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
 let second_font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Times-Roman"});
 let content=if foreground{format!("{text} {background}")}else{format!("{background} {text}")};let stream=pdf.add_object(Stream::new(dictionary!{},content.into_bytes()));
 let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Rotate"=>rotation,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font,"F2"=>second_font},"ExtGState"=>dictionary!{"Half"=>dictionary!{"Type"=>"ExtGState","ca"=>0.5}}},"Contents"=>stream});
 pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>1,"Kids"=>vec![page.into()]}.into());let catalog=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",catalog);let mut bytes=Vec::new();pdf.save_to(&mut bytes).unwrap();bytes
}

fn italic_fixture(background:&str)->Vec<u8>{
 let mut pdf=Document::load_mem(&fixture_text(background,false,0,"BT /F1 12 Tf 50 200 Td (review) Tj ET")).unwrap();
 let id=*pdf.get_pages().get(&1).unwrap();
 let font=pdf.get_dictionary(id).unwrap().get(b"Resources").unwrap().as_dict().unwrap().get(b"Font").unwrap().as_dict().unwrap().get(b"F1").unwrap().as_reference().unwrap();
 pdf.get_dictionary_mut(font).unwrap().set("BaseFont",Object::Name(b"Helvetica-Oblique".to_vec()));
 let mut bytes=Vec::new();pdf.save_to(&mut bytes).unwrap();bytes
}

#[test]
fn whole_source_advance_highlight_admits_native_italic_overhang(){
 let runtime=WorkerRuntime::new(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),Arc::new(FontRegistry::new())).unwrap();
 let plain=italic_fixture("");let doc=runtime.pdfium.load_pdf_from_byte_slice(&plain,None).unwrap();let page=doc.pages().get(0).unwrap();
 let proofs=FontProofs::read(&plain).unwrap();let runs=bound_snapshot(&page,Some(&proofs),&plain).unwrap();
 let glyphs=page_object_glyphs(&page).unwrap();let chars=&glyphs[&0];
 let end=50.+advance(&runs[0],"review").unwrap()*12.;
 assert!(chars.last().unwrap().position[4]>end+EPSILON,"fixture must actually overhang its advance");
 let bottom=chars.iter().map(|g|g.position[3]).fold(f32::INFINITY,f32::min)-1.;
 let top=chars.iter().map(|g|g.position[5]).fold(f32::NEG_INFINITY,f32::max)+1.;
 let bytes=italic_fixture(&format!("q 1 1 0 rg 50 {bottom} {} {} re f Q",end-50.,top-bottom));
 let doc=runtime.pdfium.load_pdf_from_byte_slice(&bytes,None).unwrap();let page=doc.pages().get(0).unwrap();
 let proofs=FontProofs::read(&bytes).unwrap();let mut prepared=prepare(&bytes,bound_snapshot(&page,Some(&proofs),&bytes).unwrap(),&[1],page_geometry(&page).unwrap(),true).unwrap();
 prepared.glyphs=page_object_glyphs(&page).unwrap();
 let (marks,attribution)=bind_selection(&prepared,&page).unwrap();
 assert_eq!(marks.len(),1,"whole-run advance evidence must admit this italic highlight");
 assert!(attribution.styles.iter().any(|s|s.background_id==Some(0)));
 let mark=&marks[0].rectangle;let glyphs=&prepared.glyphs;
 assert!(marks[0].cell.is_some());
 for delta in [-1.,1.]{
  let mut shifted=mark.clone();shifted.bounds[0]+=delta;
  assert!(paragraph_highlight_cells::infer(&prepared,&shifted,glyphs).unwrap().is_none());
  let mut partial=mark.clone();partial.bounds[2]+=delta;
  assert!(paragraph_highlight_cells::infer(&prepared,&partial,glyphs).unwrap().is_none());
 }
 let mut foreground=mark.clone();foreground.object_index=1;
 assert!(paragraph_highlight_cells::infer(&prepared,&foreground,glyphs).unwrap().is_none());
 let mut other=glyphs.clone();other.insert(2,other[&1].clone());
 assert!(paragraph_highlight_cells::infer(&prepared,mark,&other).unwrap().is_none());
 let mut multiline=glyphs.clone();multiline.get_mut(&1).unwrap()[1].position[1]+=0.1;
 assert!(paragraph_highlight_cells::infer(&prepared,mark,&multiline).unwrap().is_none());
 let mut reversed=glyphs.clone();reversed.get_mut(&1).unwrap()[1].position[0]=50.;
 assert!(paragraph_highlight_cells::infer(&prepared,mark,&reversed).unwrap().is_none());
 prepared.reproducible_text_state=false;
 assert!(paragraph_highlight_cells::infer(&prepared,mark,&prepared.glyphs).unwrap().is_none());
}

#[test]
fn tiny_ink_underhang_uses_cell_proof_after_legacy_tolerance_match(){
 let runtime=WorkerRuntime::new(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),Arc::new(FontRegistry::new())).unwrap();
 let text="BT /F1 12 Tf 50 200 Td (review) Tj ET";
 let plain=fixture_text("",false,0,text);let doc=runtime.pdfium.load_pdf_from_byte_slice(&plain,None).unwrap();let page=doc.pages().get(0).unwrap();
 let proofs=FontProofs::read(&plain).unwrap();let runs=bound_snapshot(&page,Some(&proofs),&plain).unwrap();let glyphs=page_object_glyphs(&page).unwrap();
 let bottom=glyphs[&0].iter().map(|g|g.position[3]).fold(f32::INFINITY,f32::min)+0.01;
 let top=glyphs[&0].iter().map(|g|g.position[5]).fold(f32::NEG_INFINITY,f32::max)+1.;
 let bytes=fixture_text(&format!("q 1 1 0 rg 50 {bottom} {} {} re f Q",advance(&runs[0],"review").unwrap()*12.,top-bottom),false,0,text);
 let doc=runtime.pdfium.load_pdf_from_byte_slice(&bytes,None).unwrap();let page=doc.pages().get(0).unwrap();let proofs=FontProofs::read(&bytes).unwrap();
 let mut prepared=prepare(&bytes,bound_snapshot(&page,Some(&proofs),&bytes).unwrap(),&[1],page_geometry(&page).unwrap(),true).unwrap();prepared.glyphs=page_object_glyphs(&page).unwrap();
 assert_eq!(discover(&page,prepared.geometry,&prepared.glyphs).unwrap().len(),1,"fixture must enter legacy discovery tolerance");
 install_source_backgrounds(&mut prepared,&page).expect("exact advance proof must recover a tiny underhang");
 assert!(prepared.backgrounds[0].cell.is_some());
}

#[test]
fn native_rectangles_and_glyphs_are_checked_without_enabling_publication(){
 let root=PathBuf::from(env!("CARGO_MANIFEST_DIR"));let runtime=WorkerRuntime::new(&root.join("resources/pdfium/pdfium.dll"),Arc::new(FontRegistry::new())).unwrap();
 for rotation in [0,90,180,270]{for (shape,accepted) in [
  ("q 1 1 0 rg 45 195 50 20 re f Q",true),
  ("q 50 0 0 20 45 195 cm 1 1 0 rg 0 0 1 1 re f Q",true),
  ("q 1 1 0 rg 45 195 m 95 195 l 95 215 l 45 215 l h f Q",true),
  ("q 1 1 0 rg 45 195 m 95 195 l 95 215 l 45 215 l f Q",false),
  ("q 1 1 0 rg 0 0 300 400 re f Q",false),
  ("q 1 1 0 rg 45 195 50 20 re B Q",false),
  ("q /Half gs 1 1 0 rg 45 195 50 20 re f Q",false),
  ("q 1 1 0 rg 45 195 50 20 re 60 200 5 5 re f* Q",false),
  ("q 1 1 0 rg 45 195 m 95 195 l 90 215 l 45 215 l h f Q",false),
  ("q 1 1 0 rg 45 195 m 50 230 90 230 95 195 c h f Q",false),
  ("q 1 1 0 rg 52 195 43 20 re f Q",false),
 ]{
  let bytes=fixture(shape,false,rotation);let document=runtime.pdfium.load_pdf_from_byte_slice(&bytes,None).unwrap();let page=document.pages().get(0).unwrap();
  let detected=discover(&page,page_geometry(&page).unwrap(),&page_object_glyphs(&page).unwrap()).unwrap();assert_eq!(!detected.is_empty(),accepted,"rotation={rotation}, shape={shape}");
  if accepted{
   assert_eq!(detected.len(),1);assert_eq!(detected[0].rectangle.color,[255,255,0]);assert_eq!(detected[0].coverage.len(),1);assert!(detected[0].coverage[0].whole_visible_run);
   let pdf=Document::load_mem(&bytes).unwrap();let id=*pdf.get_pages().get(&1).unwrap();let content=Content::decode(&pdf.get_page_content(id)).unwrap();
   let bound=bind_paint_operator(&detected[0].rectangle,&content,&page).unwrap();assert!(matches!(content.operations[bound].operator.as_str(),"f"|"f*"));
   let mut forged=Content{operations:content.operations.clone()};forged.operations.insert(0,Operation::new("f",vec![]));assert!(bind_paint_operator(&detected[0].rectangle,&forged,&page).is_err());
   let mut cursor=Content{operations:content.operations.clone()};cursor.operations.insert(0,Operation::new("TJ",vec![Object::Array(vec![Object::Real(2.)])]));assert_eq!(bind_paint_operator(&detected[0].rectangle,&cursor,&page).unwrap(),bound+1);
   // Authored native geometry proof only: source rectangle removal and new
   // painting stay in this scratch test, outside the public preview path.
   let proofs=FontProofs::read(&bytes).unwrap();let mut prepared=prepare(&bytes,snapshot(&page,Some(&proofs)).unwrap(),&[1],page_geometry(&page).unwrap(),true).unwrap();
   let (marks,attributed)=bind_selection(&prepared,&page).unwrap();let backgrounds=paragraph_highlight_geometry::source_backgrounds(&marks,&page_object_glyphs(&page).unwrap(),&prepared.source_text).unwrap();
   let mut styles=attributed.styles.iter().map(|s|styled_paragraph_plan::ResolvedStyle::original(&prepared.styles[s.source_style_id])).collect::<Vec<_>>();
   let definitions=styles.iter().map(styled_paragraph_memory::Style::source).collect::<EngineResult<Vec<_>>>().unwrap();let (probes,_)=runtime.styled_font_probes(&prepared.pdf,prepared.page_id,&definitions).unwrap();
   let metrics=styled_paragraph_plan::measure(&probes,&attributed.runs,&styles).unwrap();
   let layout=StyledParagraphLayout{pdf_bounds:[0.,0.,300.,400.],lines:vec![StyledParagraphLayoutLine{runs:vec![StyledParagraphPlacedRun{text:"Mark".into(),style_id:0,origin:[50.,200.]}],baseline:200.,ascent:12.,descent:3.,soft_wrapped:false}]};
   let ids=attributed.styles.iter().map(|s|s.background_id).collect::<Vec<_>>();let boxes=paragraph_highlight_geometry::rectangles(&layout,&metrics,&ids,&backgrounds).unwrap();assert_eq!(boxes.len(),1);assert!(close_values(&boxes[0].bounds,&marks[0].rectangle.bounds));
   prepared.content.operations[marks[0].paint_operator]=Operation::new("n",vec![]);let emission=styled_paragraph_publish::emit_located(&mut prepared,&layout,&mut styles).unwrap();let before_text=emission.block_start;let output=emission.bytes;
   let emitted=runtime.pdfium.load_pdf_from_byte_slice(&output,None).unwrap();let emitted_page=emitted.pages().get(0).unwrap();let native=page_object_glyphs(&emitted_page).unwrap();
   let output_proofs=FontProofs::read(&output).unwrap();let raw=snapshot(&emitted_page,Some(&output_proofs)).unwrap();let range=styled_paragraph_publish::fragment_range(&prepared,1,raw.len()).unwrap();
   // Preparation now binds nominal source encodings. A raw native snapshot
   // cannot prove that resource identity; production emission binds it too.
   assert!(styles[0].source.standard.is_some());
   assert!(styled_paragraph_publish::verify_fragments(&layout,&styles,&[&raw[range.start]],&emitted_page,&HashMap::new()).is_err());
   let after=bound_snapshot(&emitted_page,Some(&output_proofs),&output).unwrap();let fragment=&after[range.start];
   styled_paragraph_publish::verify_fragments(&layout,&styles,&[fragment],&emitted_page,&HashMap::new()).unwrap();
   let boxes=paragraph_highlight_geometry::native_rectangles(&layout,&[native[&fragment.index].clone()],&ids,&backgrounds).unwrap();
   let mut rebuilt=Document::load_mem(&output).unwrap();let id=*rebuilt.get_pages().get(&1).unwrap();let mut content=Content::decode(&rebuilt.get_page_content(id)).unwrap();
   content.operations.splice(before_text..before_text,paragraph_highlight_geometry::paint_operations(&boxes).unwrap());
   let stream=rebuilt.add_object(Stream::new(dictionary!{},content.encode().unwrap()));rebuilt.get_dictionary_mut(id).unwrap().set("Contents",stream);let mut output=Vec::new();rebuilt.save_to(&mut output).unwrap();let rendered=runtime.pdfium.load_pdf_from_byte_slice(&output,None).unwrap();
   assert!(raster(&rendered.pages().get(0).unwrap()).unwrap()==raster(&page).unwrap(),"native source padding replay rotation={rotation} shape={shape}, source={:?}, rebuilt={:?}, padding={:?}",marks[0].rectangle.bounds,boxes[0].bounds,backgrounds[0].padding);
  }
 }}
 let bytes=fixture("q 1 1 0 rg 45 195 50 20 re f Q",true,0);let document=runtime.pdfium.load_pdf_from_byte_slice(&bytes,None).unwrap();let page=document.pages().get(0).unwrap();assert!(discover(&page,page_geometry(&page).unwrap(),&page_object_glyphs(&page).unwrap()).unwrap().is_empty());
 for rotation in [0,90,180,270]{
  let bytes=fixture_text("q 1 1 0 rg 69 195 41 20 re f Q",false,rotation,"BT /F1 12 Tf 1 0 0 1 50 200 Tm (AB CD) Tj ET BT /F2 12 Tf 1 0 0 1 90 200 Tm (GH) Tj ET BT /F1 12 Tf 1 0 0 1 120 200 Tm (EF) Tj ET");
  let document=runtime.pdfium.load_pdf_from_byte_slice(&bytes,None).unwrap();let page=document.pages().get(0).unwrap();let proofs=FontProofs::read(&bytes).unwrap();
  let prepared=prepare(&bytes,snapshot(&page,Some(&proofs)).unwrap(),&[1,2,3],page_geometry(&page).unwrap(),true).unwrap();
  let (backgrounds,attributed)=bind_selection(&prepared,&page).unwrap();
  assert_eq!(backgrounds.len(),1);assert_eq!(backgrounds[0].spans,vec![span(1,3,5),span(2,0,2)]);
  assert_eq!(attributed.styles.iter().map(|s|(s.source_style_id,s.background_id)).collect::<Vec<_>>(),vec![(0,None),(0,Some(0)),(1,Some(0)),(1,None)]);
  assert_eq!(attributed.runs.iter().map(|r|(r.text.as_str(),r.style_id)).collect::<Vec<_>>(),vec![("AB ",0),("CD ",1),("GH",2),(" ",3),("EF",0)]);
  let mut remembered=prepared.clone();remembered.remembered=true;assert!(bind_selection(&remembered,&page).is_err());
  let partial=prepare(&bytes,snapshot(&page,Some(&proofs)).unwrap(),&[1],page_geometry(&page).unwrap(),true).unwrap();assert!(bind_selection(&partial,&page).is_err());
  let unrelated=prepare(&bytes,snapshot(&page,Some(&proofs)).unwrap(),&[3],page_geometry(&page).unwrap(),true).unwrap();assert!(bind_selection(&unrelated,&page).unwrap().0.is_empty());
 }
 // Opt-in private probe emits only numeric metadata. No private PDF copy/words.
 if let Some(path)=std::env::var_os("FOLIO_PRIVATE_HIGHLIGHT_PDF"){
  let original=std::fs::read(&path).unwrap();let document=runtime.pdfium.load_pdf_from_byte_slice(&original,None).unwrap();let page=document.pages().get(0).unwrap();
  let candidates=discover(&page,page_geometry(&page).unwrap(),&page_object_glyphs(&page).unwrap()).unwrap();
  println!("private highlight candidates={}, covered objects={}, visible glyphs={}",candidates.len(),candidates.iter().map(|c|c.coverage.len()).sum::<usize>(),candidates.iter().flat_map(|c|&c.coverage).map(|c|c.glyph_indices.len()).sum::<usize>());
  assert_eq!(candidates.len(),1);
  let ids=candidates[0].coverage.iter().map(|c|c.object_index).collect::<Vec<_>>();let proofs=FontProofs::read(&original).unwrap();let prepared=prepare(&original,snapshot(&page,Some(&proofs)).unwrap(),&ids,page_geometry(&page).unwrap(),true).unwrap();
  let operator=bind_paint_operator(&candidates[0].rectangle,&prepared.content,&page).unwrap();
  let spans=logical_ranges(&candidates[0].coverage,&page_object_glyphs(&page).unwrap(),&prepared.source_text,&ids).unwrap();
  println!("private highlight bound paint operator={operator}, attributed source spans={}",spans.len());
  let (backgrounds,attributed)=bind_selection(&prepared,&page).unwrap();
  println!("private bound backgrounds={}, attributed palette entries={}",backgrounds.len(),attributed.styles.len());
  let templates=paragraph_highlight_geometry::source_backgrounds(&backgrounds,&page_object_glyphs(&page).unwrap(),&prepared.source_text).unwrap();println!("private highlight geometry templates={}, padding={:?}",templates.len(),templates[0].padding);
  assert_eq!(std::fs::read(&path).unwrap(),original);
 }
}
