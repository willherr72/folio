use super::*;
use styled_paragraph_layout::GlyphMetric;
use paragraph_highlights::{Rectangle,SourceSpan};
#[test]
fn cell_policy_reserves_full_ink_and_scales_each_style(){
 use paragraph_highlight_cells::CellPolicy;
 let cell=CellPolicy{offsets:[-0.004,-0.004,0.004,0.2]};
 let background=Background{cell:Some(cell),color:[255,255,0],padding:[0.;4]};
 for em in [1.,12.,300.]{
  let m=StyleMetrics{em,ascent:em,descent:em*0.2,glyphs:HashMap::from([('w',GlyphMetric{advance:em*0.7,ink:[-em*0.02,-em*0.2,em*0.8,em*0.6]})])};
  let padded=padded_metrics(std::slice::from_ref(&m),&[Some(0)],&[background.clone()]).unwrap();
  let original=m.glyphs[&'w'].ink;let reserved=padded[0].glyphs[&'w'].ink;
  assert_eq!(reserved[0],original[0]);assert_eq!(reserved[1],original[1]);assert_eq!(reserved[2],original[2]);assert!(reserved[3]>original[3]);
  let paint=cell.paint(0.,em*0.7,original,em).unwrap();assert!(paint[2]<original[2]);assert!(paint[1]>original[1]);
 }
 let sizes=vec![metrics(0.1),metrics(3.)];let ids=vec![Some(0),Some(0)];
 let padded=padded_metrics(&sizes,&ids,&[background.clone()]).unwrap();
 let layout=layout_in_units(&[run("AB",0),run("CD",1)],&sizes,Some(&padded),[0.,0.,200.,200.],ParagraphAlignment::Left,1.2,1.,[0.;2],true,true).unwrap();
 let paint=rectangles(&layout,&sizes,&ids,&[background.clone()]).unwrap();assert_eq!(paint.len(),1);
 assert_eq!(style_insets(&[background.clone()],[(Some(0),1.),(Some(0),300.)]).unwrap(),[1.2,1.2]);
 assert!(!valid_backgrounds(&[Background{padding:[1.;4],..background.clone()}]));
 for offsets in [[f32::NAN,0.,0.,0.],[0.006,0.,0.,0.],[0.,-0.006,0.,0.],[0.,0.,0.,2.01]]{assert!(!CellPolicy{offsets}.valid());}
 assert!(cell.paint(0.,0.,[0.,0.,1.,1.],1.).is_err());
 assert!(CellPolicy{offsets:[0.,-0.005,0.,-0.005]}.paint(0.,1.,[0.,0.,1.,0.001],1.).is_err());
}
fn glyph(text:&str,x:f32)->Glyph{Glyph{text:text.into(),position:[x,20.,x,20.,x+6.,30.]}}
fn mark()->BoundHighlight{BoundHighlight{cell:None,rectangle:Rectangle{object_index:0,bounds:[10.,18.,30.,33.],color:[255,255,0]},paint_operator:4,spans:vec![SourceSpan{object_index:1,byte_start:0,byte_end:3}]}}
fn metrics(scale:f32)->StyleMetrics{StyleMetrics{em:10.*scale,ascent:8.*scale,descent:2.*scale,glyphs:"ABCD ".chars().map(|ch|(ch,GlyphMetric{advance:if ch==' '{5.*scale}else{10.*scale},ink:if ch==' '{[0.;4]}else{[scale,-2.*scale,9.*scale,8.*scale]}})).collect()}}
fn run(text:&str,style_id:usize)->StyledParagraphRun{StyledParagraphRun{text:text.into(),style_id}}
#[test]
fn measured_vertical_ink_keeps_highlight_clearance_at_tight_leading(){
 let metrics=vec![metrics(1.)];let backgrounds=vec![Background{cell:None,color:[255,255,0],padding:[1.,3.,1.,4.]}];
 let padded=padded_metrics(&metrics,&[Some(0)],&backgrounds).unwrap();
 let layout=layout_in_units(&[run("A\nB",0)],&metrics,Some(&padded),[0.,0.,30.,100.],ParagraphAlignment::Left,0.6,1.,[1.,1.],true,true).unwrap();
 assert_eq!(layout.lines[0].baseline,88.);assert_eq!(layout.lines[1].baseline,71.);
 let painted=rectangles(&layout,&metrics,&[Some(0)],&backgrounds).unwrap();
 assert_eq!(painted[0].bounds[1],painted[1].bounds[3]);
 assert_eq!(painted[0].bounds[3],100.);
}
#[test]
fn reserved_highlight_padding_does_not_wrap_again_during_palette_validation(){
 let metrics=vec![metrics(1.)];let backgrounds=vec![Background{cell:None,color:[255,255,0],padding:[4.,0.,4.,0.]}];
 let padded=padded_metrics(&metrics,&[Some(0)],&backgrounds).unwrap();
 let text=std::iter::repeat_n("A A",256).collect::<Vec<_>>().join("\n");
 let layout=layout_in_units(&[run(&text,0)],&metrics,Some(&padded),[0.,0.,33.,3000.],ParagraphAlignment::Center,1.,1.,[4.,4.],true,false).expect("256 fitting text rows must not be counted twice because of already-reserved padding");
 assert_eq!(layout.lines.len(),256);assert!(rectangles(&layout,&metrics,&[Some(0)],&backgrounds).is_ok());
}
#[test]
fn source_padding_uses_covered_ink_and_preserves_physical_insets(){
 let glyphs=HashMap::from([(1,vec![glyph("A",12.),glyph(" ",18.),glyph("B",22.)])]);let source=HashMap::from([(1,"A B".to_string())]);
 let backgrounds=source_backgrounds(&[mark()],&glyphs,&source).unwrap();assert_eq!(backgrounds[0].color,[255,255,0]);assert_eq!(backgrounds[0].padding,[2.,2.,2.,3.]);
 let mut partial=mark();partial.spans[0].byte_end=1;partial.rectangle.bounds[2]=20.;let single=source_backgrounds(&[partial],&glyphs,&source).unwrap();assert_eq!(single[0].padding,[2.,2.,2.,3.]);
 let mut crossing=mark();crossing.rectangle.bounds[0]=12.01;assert!(source_backgrounds(&[crossing],&glyphs,&source).is_err());
 let mut multi=glyphs.clone();multi.get_mut(&1).unwrap()[2].position[1]=40.;assert!(source_backgrounds(&[mark()],&multi,&source).is_err());
}

#[test]
fn source_baselines_cannot_drift_across_adjacent_owners(){
 let mut glyphs=HashMap::new();let mut source=HashMap::new();let mut spans=Vec::new();
 for i in 0..5 {let x=10.+10.*i as f32;let y=20.+0.015*i as f32;
  glyphs.insert(i+1,vec![Glyph{text:"A".into(),position:[x,y,x,y,x+6.,y+10.]}]);source.insert(i+1,"A".into());spans.push(SourceSpan{object_index:i+1,byte_start:0,byte_end:1});
 }
 let mut mark=BoundHighlight{cell:None,rectangle:Rectangle{object_index:0,bounds:[8.,18.,58.,33.],color:[255,255,0]},paint_operator:4,spans};
 assert!(source_backgrounds(&[mark.clone()],&glyphs,&source).is_err());
 mark.spans.reverse();assert!(source_backgrounds(&[mark.clone()],&glyphs,&source).is_err());
 mark.spans.truncate(2);assert!(source_backgrounds(&[mark],&glyphs,&source).is_ok());
}
#[test]
fn wrapping_reserves_padding_and_emits_one_rectangle_per_marked_line(){
 let background=Background{cell:None,color:[255,255,0],padding:[2.,1.,3.,2.]};let original=vec![metrics(1.)];let ids=vec![Some(0)];
 let measured=padded_metrics(&original,&ids,&[background.clone()]).unwrap();
 let layout=styled_paragraph_layout::layout(&[run("AB CD",0)],&measured,[10.,20.,40.,100.],ParagraphAlignment::Left,1.2).unwrap();assert_eq!(layout.lines.len(),2);
 let boxes=rectangles(&layout,&original,&ids,&[background]).unwrap();assert_eq!(boxes.len(),2);
 assert_eq!(boxes[0].bounds,[10.,87.,33.,100.]);assert_eq!(boxes[1].bounds,[10.,74.,33.,87.]);
}
#[test]
fn mixed_fonts_keep_fixed_padding_and_same_color_different_marks_stay_distinct(){
 let bg=Background{cell:None,color:[255,255,0],padding:[2.,1.,3.,2.]};let original=vec![metrics(1.),metrics(2.),metrics(1.)];let ids=vec![Some(0),Some(0),None];
 let measured=padded_metrics(&original,&ids,&[bg.clone()]).unwrap();let layout=styled_paragraph_layout::layout(&[run("A",0),run("B",1)],&measured,[0.,0.,100.,100.],ParagraphAlignment::Left,1.2).unwrap();
 let boxes=rectangles(&layout,&original,&ids,&[bg.clone()]).unwrap();assert_eq!(boxes.len(),1);assert_eq!(boxes[0].bounds,[0.,77.,32.,100.]);
 let original=vec![metrics(1.),metrics(1.),metrics(1.)];let ids=vec![Some(0),Some(1),None];let backgrounds=vec![bg.clone(),bg];let measured=padded_metrics(&original,&ids,&backgrounds).unwrap();
 let layout=styled_paragraph_layout::layout(&[run("A",0),run(" ",2),run("B",1)],&measured,[0.,0.,100.,100.],ParagraphAlignment::Left,1.2).unwrap();let boxes=rectangles(&layout,&original,&ids,&backgrounds).unwrap();assert_eq!(boxes.iter().map(|b|b.background_id).collect::<Vec<_>>(),vec![0,1]);
 let touching=styled_paragraph_layout::layout(&[run("A",0),run("B",1)],&measured,[0.,0.,100.,100.],ParagraphAlignment::Left,1.2).unwrap();assert!(rectangles(&touching,&original,&ids,&backgrounds).is_err());
 let plain=styled_paragraph_layout::layout(&[run("A",0),run("B",2)],&measured,[0.,0.,100.,100.],ParagraphAlignment::Left,1.2).unwrap();assert!(rectangles(&plain,&original,&ids,&backgrounds).is_err());
}

#[test]
fn movement_alignment_and_blank_lines_preserve_background_ownership(){
 let backgrounds=vec![Background{cell:None,color:[30,90,150],padding:[2.,1.,3.,2.]}];
 let original=vec![metrics(1.)];let ids=vec![Some(0)];let measured=padded_metrics(&original,&ids,&backgrounds).unwrap();
 for alignment in [ParagraphAlignment::Left,ParagraphAlignment::Center,ParagraphAlignment::Right,ParagraphAlignment::Justify]{
  let runs=vec![run("A B C D\n\nA",0)];
  let first=styled_paragraph_layout::layout(&runs,&measured,[0.,0.,40.,150.],alignment,1.2).unwrap();
  let moved=styled_paragraph_layout::layout(&runs,&measured,[70.,-20.,110.,130.],alignment,1.2).unwrap();
  let a=rectangles(&first,&original,&ids,&backgrounds).unwrap();let b=rectangles(&moved,&original,&ids,&backgrounds).unwrap();
  assert_eq!(a.len(),3);assert_eq!(a.len(),first.lines.len()-1);assert_eq!(a.len(),b.len());
  for (old,new) in a.iter().zip(&b){assert_eq!(old.color,new.color);assert_eq!(old.background_id,new.background_id);assert!(close_values(&new.bounds,&[old.bounds[0]+70.,old.bounds[1]-20.,old.bounds[2]+70.,old.bounds[3]-20.]));}
  if alignment==ParagraphAlignment::Justify{assert!(first.lines[0].runs.len()>1);assert!((a[0].bounds[2]-40.).abs()<EPSILON);}
 }
 // An unhighlighted whitespace span separates marks even when both have the
 // same original owner. It must not become highlighted during coalescing.
 let original=vec![metrics(1.),metrics(1.)];let ids=vec![Some(0),None];let measured=padded_metrics(&original,&ids,&backgrounds).unwrap();
 let layout=styled_paragraph_layout::layout(&[run("A",0),run("   ",1),run("B",0)],&measured,[0.,0.,100.,100.],ParagraphAlignment::Left,1.2).unwrap();
 let boxes=rectangles(&layout,&original,&ids,&backgrounds).unwrap();assert_eq!(boxes.len(),2);assert!(boxes[0].bounds[2]<boxes[1].bounds[0]);
}

#[test]
fn malformed_geometry_is_rejected_before_painting(){
 let valid=PlacedBackground{background_id:0,color:[255,128,0],bounds:[-10.,20.,30.,50.]};
 let ops=paint_operations(&[valid.clone()]).unwrap();assert_eq!(ops.iter().map(|o|o.operator.as_str()).collect::<Vec<_>>(),vec!["q","rg","re","f","Q"]);
 assert_eq!(ops[2].operands,vec![Object::Real(-10.),Object::Real(20.),Object::Real(40.),Object::Real(30.)]);
 assert!(paint_operations(&[]).unwrap().is_empty());
 for bounds in [[0.,0.,0.,1.],[1.,0.,0.,1.],[0.,0.,1.,f32::INFINITY],[f32::NAN,0.,1.,1.],[0.,0.,1_000_001.,1.]]{assert!(paint_operations(&[PlacedBackground{bounds,..valid.clone()}]).is_err());}
 assert!(paint_operations(&[PlacedBackground{background_id:64,..valid.clone()}]).is_err());assert!(paint_operations(&vec![valid;4097]).is_err());
 let original=vec![metrics(1.)];let background=Background{cell:None,color:[255,255,0],padding:[2.,1.,3.,2.]};
 assert!(padded_metrics(&original,&[],&[background.clone()]).is_err());assert!(padded_metrics(&original,&[Some(1)],&[background.clone()]).is_err());
 assert!(padded_metrics(&original,&[Some(0)],&[Background{padding:[-1.,0.,0.,0.],..background.clone()}]).is_err());
 let measured=padded_metrics(&original,&[Some(0)],&[background.clone()]).unwrap();let mut layout=styled_paragraph_layout::layout(&[run("A",0)],&measured,[0.,0.,40.,100.],ParagraphAlignment::Left,1.2).unwrap();
 layout.lines[0].runs[0].origin[0]-=10.;assert!(rectangles(&layout,&original,&[Some(0)],&[background.clone()]).is_err());
 layout.lines[0].runs[0].origin[0]=f32::NAN;assert!(rectangles(&layout,&original,&[Some(0)],&[background]).is_err());
}

#[test]
fn native_ink_requires_matching_fragments_and_bounded_geometry(){
 let original=vec![metrics(1.)];let ids=vec![Some(0)];let backgrounds=vec![Background{cell:None,color:[255,255,0],padding:[2.,1.,3.,2.]}];
 let measured=padded_metrics(&original,&ids,&backgrounds).unwrap();let mut layout=styled_paragraph_layout::layout(&[run("A",0)],&measured,[0.,0.,40.,100.],ParagraphAlignment::Left,1.2).unwrap();
 let [x,y]=layout.lines[0].runs[0].origin;let glyph=Glyph{text:"A".into(),position:[x,y,x+1.,y-2.,x+9.,y+8.]};
 assert_eq!(native_rectangles(&layout,&[vec![glyph.clone()]],&ids,&backgrounds).unwrap(),rectangles(&layout,&original,&ids,&backgrounds).unwrap());
 layout.lines[0].runs[0].text="A  ".into();assert_eq!(native_rectangles(&layout,&[vec![glyph.clone()]],&ids,&backgrounds).unwrap(),rectangles(&layout,&original,&ids,&backgrounds).unwrap());
 layout.lines[0].runs[0].text="A".into();
 assert!(native_rectangles(&layout,&[],&ids,&backgrounds).is_err());
 assert!(native_rectangles(&layout,&[vec![Glyph{text:"B".into(),..glyph.clone()}]],&ids,&backgrounds).is_err());
 assert!(native_rectangles(&layout,&[vec![Glyph{position:[x,y,10.,0.,1.,10.],..glyph.clone()}]],&ids,&backgrounds).is_err());
 layout.lines[0].runs[0].style_id=1;assert!(native_rectangles(&layout,&[vec![glyph.clone()]],&ids,&backgrounds).is_err());
 layout.lines[0].runs=vec![StyledParagraphPlacedRun{text:String::new(),style_id:0,origin:[0.,0.]};4097];
 assert!(native_rectangles(&layout,&[],&ids,&backgrounds).is_err());assert!(rectangles(&layout,&original,&ids,&backgrounds).is_err());
 let mut source_glyphs=HashMap::from([(1,vec![super::tests::glyph("A",12.),super::tests::glyph(" ",18.),super::tests::glyph("B",22.)])]);
 source_glyphs.get_mut(&1).unwrap()[1].position[4]=0.;assert!(source_backgrounds(&[mark()],&source_glyphs,&HashMap::from([(1,"A B".into())])).is_err());
}
