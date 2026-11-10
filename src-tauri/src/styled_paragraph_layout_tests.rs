use super::*;
fn run(text:&str,style_id:usize)->StyledParagraphRun{StyledParagraphRun{text:text.into(),style_id}}
fn styles()->Vec<StyleMetrics>{
 (0..2).map(|id|{let scale=(id+1) as f32;
  let glyphs="abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ ".chars().map(|c|(c,GlyphMetric{
   advance:if c==' '{3.*scale}else{5.*scale},
   ink:if c==' '{[0.;4]}else{[0.,-2.*scale,5.*scale,8.*scale]},
  })).collect();StyleMetrics{em:10.*scale,ascent:8.*scale,descent:2.*scale,glyphs}
 }).collect()
}
fn text(line:&StyledParagraphLayoutLine)->String{line.runs.iter().map(|r|r.text.as_str()).collect()}
#[test]
fn measured_vertical_policy_keeps_requested_leading_and_legacy_font_extents(){
 let mut metrics=styles();metrics[0].ascent=13.;metrics[0].descent=5.;
 let runs=[run("a\nb",0)];let bounds=[0.,0.,50.,100.];
 let old=layout_with_text_metrics(&runs,&metrics,&metrics,bounds,ParagraphAlignment::Left,1.,1.,false).unwrap();
 let new=layout_with_text_metrics(&runs,&metrics,&metrics,bounds,ParagraphAlignment::Left,1.,1.,true).unwrap();
 assert_eq!(old.lines.iter().map(|l|l.baseline).collect::<Vec<_>>(),[87.,69.]);
 assert_eq!(new.lines.iter().map(|l|l.baseline).collect::<Vec<_>>(),[92.,82.]);
 assert_eq!(new.lines[0].runs[0].origin[0],old.lines[0].runs[0].origin[0]);
 assert_eq!(first_line_ascent_with_text_metrics(&runs,&metrics,&metrics,50.,1.,true).unwrap(),8.);
 assert!(layout_with_text_metrics(&runs,&metrics,&metrics,[0.,80.,50.,100.],ParagraphAlignment::Left,1.,1.,true).is_ok());
 assert!(layout_with_text_metrics(&runs,&metrics,&metrics,[0.,80.1,50.,100.],ParagraphAlignment::Left,1.,1.,true).is_err());
}
#[test]
fn measured_vertical_policy_reserves_visible_accents_descenders_and_blank_styles(){
 let mut metrics=styles();metrics[0].glyphs.get_mut(&'a').unwrap().ink=[-3.,-7.,7.,15.];
 let rows=layout_with_text_metrics(&[run("a\nb",0)],&metrics,&metrics,[0.,0.,50.,100.],ParagraphAlignment::Left,0.6,1.,true).unwrap();
 assert_eq!(rows.lines[0].baseline,85.);assert_eq!(rows.lines[1].baseline,70.);
 assert_eq!(rows.lines[0].runs[0].origin[0],3.);
 let metrics=styles();
 let rows=layout_with_text_metrics(&[run("a",0),run(" ",1),run("\nb\n",0),run(" \n",1)],&metrics,&metrics,[0.,0.,50.,100.],ParagraphAlignment::Left,0.6,1.,true).unwrap();
 assert_eq!(rows.lines.iter().map(|l|l.baseline).collect::<Vec<_>>(),[92.,80.,62.,42.]);
 assert_eq!(rows.lines.iter().map(|l|l.ascent).collect::<Vec<_>>(),[8.,8.,16.,16.]);
 assert_eq!(rows.lines[2].descent,4.);assert_eq!(rows.lines[3].descent,4.);
}
#[test]
fn measured_vertical_policy_still_limits_rows_and_rejects_invalid_ink(){
 let mut metrics=styles();let text=std::iter::repeat_n("a",256).collect::<Vec<_>>().join("\n");
 assert_eq!(layout_with_text_metrics(&[run(&text,0)],&metrics,&metrics,[0.,0.,50.,3000.],ParagraphAlignment::Left,0.6,1.,true).unwrap().lines.len(),256);
 assert!(layout_with_text_metrics(&[run(&(text+"\na"),0)],&metrics,&metrics,[0.,0.,50.,3000.],ParagraphAlignment::Left,0.6,1.,true).is_err());
 metrics[0].glyphs.get_mut(&'a').unwrap().ink[3]=f32::NAN;
 assert!(layout_with_text_metrics(&[run("a",0)],&metrics,&metrics,[0.,0.,50.,100.],ParagraphAlignment::Left,1.,1.,true).is_err());
}
#[test]
fn first_line_measurement_tracks_wrapping_without_admitting_an_oversized_word(){
 let runs=vec![run("a ",0),run("b",1)];let metrics=styles();
 assert_eq!(first_line_ascent(&runs,&metrics,10.,1.).unwrap(),8.);
 assert_eq!(first_line_ascent(&runs,&metrics,30.,1.).unwrap(),16.);
 assert_eq!(first_line_ascent(&runs,&metrics,1.,1.).unwrap(),8.);
 assert!(layout(&runs,&metrics,[0.,0.,1.,100.],ParagraphAlignment::Left,1.2).is_err());
 assert!(first_line_ascent(&runs,&metrics,f32::NAN,1.).is_err());
}
#[test]
fn words_cross_styles_and_only_spaces_allow_wrapping(){
 let runs=vec![run("pre",0),run("fix ",1),run("word",0)];
 let result=layout(&runs,&styles(),[0.,0.,60.,100.],ParagraphAlignment::Left,1.2).unwrap();
 assert_eq!(result.lines.iter().map(text).collect::<Vec<_>>(),["prefix ","word"]);
 assert_eq!(result.lines[0].runs[1].origin,[15.,84.]);assert_eq!(result.lines[1].baseline,60.);
 assert!(layout(&runs,&styles(),[0.,0.,40.,100.],ParagraphAlignment::Left,1.2).is_err());
}
#[test]
fn alignment_accounts_for_negative_bearings_and_ignores_trailing_spaces(){
 let mut metrics=styles();metrics[0].glyphs.get_mut(&'j').unwrap().ink=[-3.,-2.,7.,8.];
 for (alignment,x) in [(ParagraphAlignment::Left,13.),(ParagraphAlignment::Center,21.5),(ParagraphAlignment::Right,30.)]{
  let result=layout(&[run("jA  ",0)],&metrics,[10.,0.,40.,100.],alignment,1.2).unwrap();
  assert_eq!(result.lines[0].runs[0].origin,[x,92.]);assert_eq!(text(&result.lines[0]),"jA  ");
 }
 let result=layout(&[run("a   ",0)],&metrics,[0.,0.,5.,100.],ParagraphAlignment::Left,1.2).unwrap();assert_eq!(result.lines.len(),1);assert_eq!(text(&result.lines[0]),"a   ");
}
#[test]
fn justification_expands_each_cross_style_space_group_once(){
 let result=layout(&[run("a ",0),run(" ",1),run("b c d",0)],&styles(),[0.,0.,30.,100.],ParagraphAlignment::Justify,1.2).unwrap();
 assert_eq!(result.lines.iter().map(text).collect::<Vec<_>>(),["a  b c ","d"]);
 let runs=&result.lines[0].runs;assert_eq!(runs.len(),4);
 assert!((runs[2].origin[0]-15.5).abs()<0.001);assert!((runs[3].origin[0]-25.).abs()<0.001);
 assert_eq!(result.lines[1].runs[0].origin[0],0.);
 let hard=layout(&[run("a b\nc",0)],&styles(),[0.,0.,30.,100.],ParagraphAlignment::Justify,1.2).unwrap();assert_eq!(hard.lines[0].runs.len(),1);assert!(!hard.lines[0].soft_wrapped);
}
#[test]
fn blank_and_final_hard_break_rows_keep_style_height_and_exact_vertical_fit(){
 let runs=[run("a\n",0),run("\n",1),run("b\n",0)];
 let result=layout(&runs,&styles(),[0.,30.,50.,100.],ParagraphAlignment::Left,1.2).unwrap();
 assert_eq!(result.lines.iter().map(text).collect::<Vec<_>>(),["a","","b",""]);
 assert_eq!(result.lines.iter().map(|l|l.baseline).collect::<Vec<_>>(),[92.,68.,44.,32.]);
 assert_eq!(result.lines[1].ascent,16.);assert_eq!(result.lines[3].descent,2.);
 assert!(layout(&runs,&styles(),[0.,30.1,50.,100.],ParagraphAlignment::Left,1.2).is_err());
}
#[test]
fn actual_ink_extents_expand_the_font_line_box(){
 let mut metrics=styles();metrics[0].glyphs.get_mut(&'a').unwrap().ink=[0.,-7.,5.,15.];
 let result=layout(&[run("a\nb",0)],&metrics,[0.,0.,50.,100.],ParagraphAlignment::Left,0.6).unwrap();
 assert_eq!(result.lines[0].baseline,85.);assert_eq!(result.lines[0].ascent,15.);assert_eq!(result.lines[0].descent,7.);assert_eq!(result.lines[1].baseline,70.);
}
#[test]
fn input_fragmentation_does_not_change_logical_text_or_placement(){
 let original=layout(&[run("alpha beta gamma",0)],&styles(),[0.,0.,35.,100.],ParagraphAlignment::Center,1.2).unwrap();
 let split="alpha beta gamma".chars().map(|c|run(&c.to_string(),0)).collect::<Vec<_>>();
 assert_eq!(layout(&split,&styles(),[0.,0.,35.,100.],ParagraphAlignment::Center,1.2).unwrap(),original);
}
#[test]
fn wrapped_rows_do_not_inherit_a_large_style_from_the_first_row(){
 let result=layout(&[run("A ",1),run("bb cc dd ee",0)],&styles(),[0.,0.,20.,100.],ParagraphAlignment::Left,1.).unwrap();
 assert_eq!(result.lines.iter().map(text).collect::<Vec<_>>(),["A ","bb ","cc ","dd ","ee"]);
 assert_eq!(result.lines.iter().map(|l|l.ascent).collect::<Vec<_>>(),[16.,8.,8.,8.,8.]);
}
#[test]
fn justification_handles_later_negative_bearing_and_earlier_right_overhang(){
 let mut metrics=styles();metrics[0].glyphs.get_mut(&'a').unwrap().ink[2]=50.;metrics[0].glyphs.get_mut(&'b').unwrap().ink[0]=-20.;
 let result=layout(&[run("a ",0),run(" ",1),run("b cccccccccc",0)],&metrics,[0.,0.,65.,100.],ParagraphAlignment::Justify,1.2).unwrap();
 assert_eq!(result.lines.iter().map(text).collect::<Vec<_>>(),["a  b ","cccccccccc"]);
 let first=&result.lines[0].runs;assert_eq!(first.len(),3);assert_eq!(first[0].origin[0],0.);assert_eq!(first[1].origin[0],8.);
 assert!((first[2].origin[0]-60.).abs()<0.001); // b's right edge is 65, a's is 50
 assert_eq!(result.lines[1].runs[0].origin[0],0.);
}
#[test]
fn leading_blank_final_break_style_and_tall_trailing_spaces_keep_their_metrics(){
 let runs=[run("\na",0),run("\n",1)];
 let result=layout(&runs,&styles(),[0.,60.,50.,100.],ParagraphAlignment::Left,0.6).unwrap();
 assert_eq!(result.lines.iter().map(text).collect::<Vec<_>>(),["","a",""]);
 assert_eq!(result.lines.iter().map(|l|l.baseline).collect::<Vec<_>>(),[92.,82.,64.]);
 assert!(layout(&runs,&styles(),[0.,60.1,50.,100.],ParagraphAlignment::Left,0.6).is_err());
 let result=layout(&[run("a",0),run(" ",1),run("\nb",0)],&styles(),[0.,70.,50.,100.],ParagraphAlignment::Left,0.6).unwrap();
 assert_eq!(result.lines[0].ascent,16.);assert_eq!(result.lines[1].baseline,72.);
}
#[test]
fn byte_palette_and_fragment_boundaries_are_measured_without_truncation(){
 let mut metrics=styles();let glyph=metrics[0].glyphs[&'a'];metrics[0].glyphs.insert('é',glyph);let bounds=[0.,0.,30_000.,10_000.];
 assert!(layout(&[run(&"é".repeat(2048),0)],&metrics,bounds,ParagraphAlignment::Left,1.).is_ok());
 assert!(layout(&[run(&"é".repeat(2049),0)],&metrics,bounds,ParagraphAlignment::Left,1.).is_err());
 let mut palette:Vec<_>=(0..64).map(|_|styles().remove(0)).collect();
 let runs=(0..4096).map(|i|run("a",i%64)).collect::<Vec<_>>();
 let result=layout(&runs,&palette,bounds,ParagraphAlignment::Left,1.).unwrap();assert_eq!(result.lines[0].runs.len(),4096);assert_eq!(text(&result.lines[0]).len(),4096);
 palette.push(styles().remove(0));assert!(layout(&runs,&palette,bounds,ParagraphAlignment::Left,1.).is_err());
}
#[test]
fn refuses_invalid_ids_metrics_and_text_without_truncating_limits(){
 let metrics=styles();let bounds=[0.,0.,30_000.,10_000.];
 assert!(layout(&[run(&"a".repeat(4096),0)],&metrics,bounds,ParagraphAlignment::Left,1.).is_ok());
 assert!(layout(&[run(&"a".repeat(4097),0)],&metrics,bounds,ParagraphAlignment::Left,1.).is_err());
 assert!(layout(&[run(&vec!["a";256].join("\n"),0)],&metrics,bounds,ParagraphAlignment::Left,1.).is_ok());
 assert!(layout(&[run(&vec!["a";257].join("\n"),0)],&metrics,bounds,ParagraphAlignment::Left,1.).is_err());
 for runs in [vec![],vec![run("",0)],vec![run("a",99)],vec![run("a",0),run("",99)],vec![run("a\r",0)],vec![run("a\t",0)],vec![run("   ",0)],vec![run("a",0);4097]]{
  assert!(layout(&runs,&metrics,bounds,ParagraphAlignment::Left,1.).is_err());
 }
 let mut bad=styles();bad[0].glyphs.get_mut(&'a').unwrap().advance=f32::NAN;assert!(layout(&[run("a",0)],&bad,bounds,ParagraphAlignment::Left,1.).is_err());
 let mut bad=styles();bad[0].descent=-1.;assert!(layout(&[run("a",0)],&bad,bounds,ParagraphAlignment::Left,1.).is_err());
}
