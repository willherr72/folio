use folio_engine::{DocumentInfo,ExportRequest,PagePlan,PdfEngine,ParagraphAlignment,StyledParagraphLayoutRequest,StyledParagraphRun};
use lopdf::{content::{Content,Operation},dictionary,Document,Object,Stream};
use std::{fs,path::{Path,PathBuf}};

fn engine()->PdfEngine{PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture(path:&Path,kind:&str,excluded:bool){
 let original=include_bytes!("../../tests/fixtures/embedded-text/DejaVuSerif-subset.ttf");
 let bytes=include_bytes!("../../tests/fixtures/embedded-text/DejaVuSerif-cid-no-cmap.ttf");
 let face=ttf_parser::Face::parse(original,0).unwrap();
 let mut chars:Vec<_>="Old Edit Neighbor safe ".chars().collect();chars.sort();chars.dedup();
 let mut widths=vec![];let mut unicode=String::new();let mut glyphs=vec![0;4096];let mut codes=std::collections::BTreeMap::new();
 for(i,ch)in chars.iter().enumerate(){
  let gid=face.glyph_index(*ch).unwrap();let code=if kind=="shifted"{0x200+i as u16}else{gid.0};codes.insert(*ch,code);
  glyphs[code as usize*2..code as usize*2+2].copy_from_slice(&gid.0.to_be_bytes());
  widths.extend([Object::Integer(code as i64),Object::Array(vec![Object::Real(face.glyph_hor_advance(gid).unwrap()as f32*1000./face.units_per_em()as f32)])]);
  unicode.push_str(&format!("<{code:04X}> <{:04X}>\n",*ch as u32));
 }
 if excluded{unicode.push_str("<FFFF> <D800>\n");}
 let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
 let program=pdf.add_object(Stream::new(dictionary!{"Length1"=>bytes.len()as i64},bytes.to_vec()));
 let descriptor=pdf.add_object(dictionary!{"Type"=>"FontDescriptor","FontName"=>"ABCDEF+DejaVuSerif","Flags"=>32,"FontBBox"=>vec![(-1000).into(),(-1000).into(),2000.into(),2000.into()],"ItalicAngle"=>0,"Ascent"=>1000,"Descent"=>-300,"CapHeight"=>800,"StemV"=>80,"FontFile2"=>program});
 let mut child=dictionary!{"Type"=>"Font","Subtype"=>"CIDFontType2","BaseFont"=>"ABCDEF+DejaVuSerif","CIDSystemInfo"=>dictionary!{"Registry"=>Object::string_literal("Adobe"),"Ordering"=>Object::string_literal("Identity"),"Supplement"=>0},"FontDescriptor"=>descriptor,"DW"=>0,"W"=>widths};
 if kind=="identity"{child.set("CIDToGIDMap","Identity");}
 if kind=="shifted"{child.set("CIDToGIDMap",pdf.add_object(Stream::new(dictionary!{},glyphs)));}
 let child=pdf.add_object(child);
 let map=pdf.add_object(Stream::new(dictionary!{},format!("1 begincodespacerange <0000> <FFFF> endcodespacerange {} beginbfchar {unicode}endbfchar",chars.len()+usize::from(excluded)).into_bytes()));
 let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type0","BaseFont"=>"ABCDEF+DejaVuSerif","Encoding"=>"Identity-H","DescendantFonts"=>vec![child.into()],"ToUnicode"=>map});
 let standard=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
 let mut encoded:Vec<_>="Old".chars().flat_map(|ch|codes[&ch].to_be_bytes()).collect();if excluded{encoded.splice(2..2,[0xff,0xff]);}
 let mut ops=vec![];
 for(alias,text,y)in[("F1",encoded,300),("F2",b"Neighbor safe".to_vec(),80)]{
  ops.extend([Operation::new("BT",vec![]),Operation::new("Tf",vec![Object::Name(alias.as_bytes().to_vec()),14.into()]),Operation::new("Td",vec![50.into(),y.into()]),Operation::new("Tj",vec![Object::string_literal(text)]),Operation::new("ET",vec![])]);
 }
 let stream=pdf.add_object(Stream::new(dictionary!{},Content{operations:ops}.encode().unwrap()));
 let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),400.into(),400.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font,"F2"=>standard}},"Contents"=>stream});
 pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());let catalog=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",catalog);pdf.save(path).unwrap();
}
fn export(e:&PdfEngine,source:&DocumentInfo,path:&Path){e.export_pdf(ExportRequest{flatten:false,pages:vec![PagePlan{crop:None,id:"page".into(),source_id:source.id.clone(),page_index:0,width:source.pages[0].width,height:source.pages[0].height,rotation:0,overlays:vec![]}]},path).unwrap();}
fn assets(bytes:&[u8])->Vec<(String,Vec<u8>)>{
 let pdf=Document::load_mem(bytes).unwrap();let mut found=vec![];
 for dict in pdf.objects.values().filter_map(|v|v.as_dict().ok()){
  for name in [b"FontFile2".as_slice(),b"ToUnicode",b"CIDToGIDMap"]{if let Ok(value)=dict.get(name){let value=pdf.dereference(value).unwrap().1;let payload=if let Ok(stream)=value.as_stream(){stream.get_plain_content().unwrap()}else{value.as_name().unwrap().to_vec()};found.push((String::from_utf8(name.to_vec()).unwrap(),payload));}}
 }
 found.sort();found.dedup();found
}
fn retain(name:&str,source:&Path,output:&Path){if let Some(root)=std::env::var_os("FOLIO_CID_NO_CMAP_OUTPUTS"){let root=PathBuf::from(root);fs::create_dir_all(&root).unwrap();fs::copy(source,root.join(format!("{name}-original.pdf"))).unwrap();fs::copy(output,root.join(format!("{name}-saved.pdf"))).unwrap();}}

#[test]
fn cid_without_cmap_original_font_and_standard_neighbor_survive_saved_reedits(){
 let e=engine();let temp=tempfile::tempdir().unwrap();
 for kind in ["identity","shifted","default"]{
  let input=temp.path().join(format!("{kind}.pdf"));fixture(&input,kind,false);let original=fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();
  let before=e.list_occurrence_text_runs(&source.id,0).unwrap().runs;assert_eq!(before[0].text,"Old");assert!(before[0].supported&&before[1].supported,"{kind}: {before:?}");
  let noop=e.replace_text_occurrence(&source.id,&[0],0,0,"Old","Old",None).unwrap().document;assert_eq!(e.render_page(&source.id,0,800).unwrap(),e.render_page(&noop.id,0,800).unwrap());e.close_document(&noop.id).unwrap();
  for(label,text)in[("delete","Ol"),("growth","Old Edit")]{
   let edited=e.replace_text_occurrence(&source.id,&[0],0,0,"Old",text,None).unwrap().document;let saved=temp.path().join(format!("{kind}-{label}.pdf"));export(&e,&edited,&saved);let reopened=e.open_document(&saved).unwrap();
   let after=e.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;assert_eq!(after[0].text,text);assert!(after[0].supported);assert_eq!(after[0].font_name,before[0].font_name);assert_eq!(after[1].text,before[1].text);assert_eq!(after[1].bounds,before[1].bounds);assert_eq!(assets(&original),assets(&fs::read(&saved).unwrap()));
   let again=e.replace_text_occurrence(&reopened.id,&[0],0,0,text,"Old",None).unwrap().document;let final_path=temp.path().join(format!("{kind}-{label}-again.pdf"));export(&e,&again,&final_path);let final_doc=e.open_document(&final_path).unwrap();assert_eq!(e.list_occurrence_text_runs(&final_doc.id,0).unwrap().runs[0].text,"Old");assert_eq!(e.render_page(&source.id,0,800).unwrap(),e.render_page(&final_doc.id,0,800).unwrap());assert_eq!(assets(&original),assets(&fs::read(&final_path).unwrap()));retain(&format!("{kind}-{label}"),&input,&saved);
   for id in [&edited.id,&reopened.id,&again.id,&final_doc.id]{e.close_document(id).unwrap();}
  }
  let neighbor=e.replace_text_occurrence(&source.id,&[0],0,1,"Neighbor safe","Neighbor",None).unwrap().document;let saved=temp.path().join(format!("{kind}-neighbor.pdf"));export(&e,&neighbor,&saved);assert_eq!(e.list_occurrence_text_runs(&neighbor.id,0).unwrap().runs[0].text,"Old");assert_eq!(assets(&original),assets(&fs::read(&saved).unwrap()));retain(&format!("{kind}-neighbor"),&input,&saved);e.close_document(&neighbor.id).unwrap();
  assert!(e.replace_text_occurrence(&source.id,&[0],0,0,"Old","Old Z",None).is_err());assert_eq!(fs::read(&input).unwrap(),original);assert_eq!(e.source_bytes(&source.id).unwrap().as_ref(),original.as_slice());e.close_document(&source.id).unwrap();
 }
}

#[test]
fn cid_without_cmap_supports_explicit_substitution_and_refuses_excluded_source_codes(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("original.pdf");fixture(&input,"shifted",false);let source=e.open_document(&input).unwrap();let font=e.fonts().register(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec()).unwrap();
 let edited=e.replace_text_occurrence(&source.id,&[0],0,0,"Old","Old Z",Some(&font.id)).unwrap().document;let output=temp.path().join("substitute.pdf");export(&e,&edited,&output);let reopened=e.open_document(&output).unwrap();let after=e.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;assert_eq!(after[0].text,"Old Z");assert_eq!(after[1].text,"Neighbor safe");retain("substitute",&input,&output);for id in [&source.id,&edited.id,&reopened.id]{e.close_document(id).unwrap();}
 let bad=temp.path().join("excluded.pdf");fixture(&bad,"shifted",true);let source=e.open_document(&bad).unwrap();let bytes=e.source_bytes(&source.id).unwrap();let runs=e.list_occurrence_text_runs(&source.id,0).unwrap().runs;assert!(!runs[0].supported);assert!(e.replace_text_occurrence(&source.id,&[0],0,0,&runs[0].text,"Old",None).is_err());assert_eq!(e.source_bytes(&source.id).unwrap(),bytes);e.close_document(&source.id).unwrap();
}

#[test]
fn cid_without_cmap_paragraph_reflows_and_replays_after_save(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("original.pdf");fixture(&input,"shifted",false);let source=e.open_document(&input).unwrap();let selected=e.inspect_styled_paragraph(&source.id,0,&[0]).unwrap();let mut bounds=selected.bounds;bounds.width=90.;bounds.height=90.;
 let text="Old Edit Old Edit Old";let request=StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:vec![0],expected_text:selected.text,runs:vec![StyledParagraphRun{text:text.into(),style_id:0}],style_changes:vec![],bounds,alignment:ParagraphAlignment::Left,line_spacing:1.2};
 let edited=e.preview_styled_paragraph_occurrence(&source.id,&[0],0,&request).unwrap().document;let output=temp.path().join("paragraph.pdf");export(&e,&edited,&output);let reopened=e.open_document(&output).unwrap();let runs=e.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;let indices:Vec<_>=runs.iter().filter(|r|r.text!="Neighbor safe").map(|r|r.object_index).collect();assert!(indices.len()>1);let replay=e.inspect_styled_paragraph(&reopened.id,0,&indices).unwrap();assert_eq!(replay.text,text);let noop=e.preview_styled_paragraph_occurrence(&reopened.id,&[0],0,&StyledParagraphLayoutRequest{baseline_anchor:None,object_indices:replay.object_indices,expected_text:replay.text,runs:replay.runs,style_changes:vec![],bounds:replay.bounds,alignment:replay.alignment.unwrap(),line_spacing:replay.line_spacing.unwrap()}).unwrap().document;assert_eq!(e.render_page(&reopened.id,0,800).unwrap(),e.render_page(&noop.id,0,800).unwrap());assert_eq!(assets(&fs::read(&input).unwrap()),assets(&fs::read(&output).unwrap()));retain("paragraph",&input,&output);for id in [&source.id,&edited.id,&reopened.id,&noop.id]{e.close_document(id).unwrap();}
}
