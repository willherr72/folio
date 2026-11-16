use super::*;
use lopdf::{dictionary,Stream};

const ORIGINAL:&[u8]=include_bytes!("../../tests/fixtures/embedded-text/DejaVuSerif-subset.ttf");
const NO_CMAP:&[u8]=include_bytes!("../../tests/fixtures/embedded-text/DejaVuSerif-cid-no-cmap.ttf");

fn fixture(kind:&str)->(Document,Dictionary){
 let face=ttf_parser::Face::parse(ORIGINAL,0).unwrap();
 let mut chars:Vec<_>="Old Edit ".chars().collect();chars.sort();chars.dedup();
 let mut widths=vec![];let mut unicode=String::new();let mut glyphs=vec![0;4096];
 for(i,ch)in chars.iter().enumerate(){
  let gid=face.glyph_index(*ch).unwrap();let code=if kind=="shifted"{0x200+i as u16}else{gid.0};
  glyphs[code as usize*2..code as usize*2+2].copy_from_slice(&gid.0.to_be_bytes());
  widths.extend([Object::Integer(code as i64),Object::Array(vec![Object::Real(face.glyph_hor_advance(gid).unwrap()as f32*1000./face.units_per_em()as f32)])]);
  unicode.push_str(&format!("<{code:04X}> <{:04X}>\n",*ch as u32));
 }
 let mut pdf=Document::with_version("1.7");
 let program=pdf.add_object(Stream::new(dictionary!{},NO_CMAP.to_vec()));
 let descriptor=pdf.add_object(dictionary!{"Flags"=>32,"FontFile2"=>program});
 let mut child=dictionary!{"Subtype"=>"CIDFontType2","FontDescriptor"=>descriptor,"W"=>widths};
 if kind=="identity"{child.set("CIDToGIDMap","Identity");}
 if kind=="shifted"{child.set("CIDToGIDMap",pdf.add_object(Stream::new(dictionary!{},glyphs)));}
 let child=pdf.add_object(child);
 let map=pdf.add_object(Stream::new(dictionary!{},format!("1 begincodespacerange <0000> <FFFF> endcodespacerange {} beginbfchar {unicode}endbfchar",chars.len()).into_bytes()));
 let font=dictionary!{"Subtype"=>"Type0","BaseFont"=>"ABCDEF+DejaVuSerif","Encoding"=>"Identity-H","DescendantFonts"=>vec![child.into()],"ToUnicode"=>map};
 (pdf,font)
}
fn child_id(font:&Dictionary)->ObjectId{font.get(b"DescendantFonts").unwrap().as_array().unwrap()[0].as_reference().unwrap()}

#[test]
fn cid_without_cmap_uses_exact_pdf_glyph_codes_and_preserves_source_encoding(){
 assert!(ttf_parser::Face::parse(NO_CMAP,0).unwrap().raw_face().table(ttf_parser::Tag::from_bytes(b"cmap")).is_none());
 for kind in ["identity","shifted","default"]{
  let(pdf,font)=fixture(kind);let proof=verify(&pdf,&font,NO_CMAP).expect("PDF CID mapping must prove map-free source glyphs");
  assert!(proof.requires_source_operands&&proof.preserve_source_encoding,"{kind}");
  let encoded=proof.encode("Old Edit").unwrap();assert_eq!(proof.decode(&encoded).unwrap(),"Old Edit");
  if kind=="shifted"{assert_ne!(encoded[..2],ttf_parser::Face::parse(ORIGINAL,0).unwrap().glyph_index('O').unwrap().0.to_be_bytes());}
  assert!(proof.encode("this").is_err());assert!(proof.decode(&[0xff,0xff]).is_err());
  assert!(verify_domain(&pdf,&font,NO_CMAP,true).is_err(),"Complete form repertoire proof is unchanged");
 }
}

#[test]
fn cid_without_cmap_refuses_missing_semantics_simple_fonts_and_malformed_present_cmap(){
 let(pdf,mut font)=fixture("identity");font.remove(b"ToUnicode");assert!(verify(&pdf,&font,NO_CMAP).is_err());
 let(pdf,font)=fixture("identity");let mut simple=pdf.get_dictionary(child_id(&font)).unwrap().clone();simple.set("Subtype","TrueType");simple.set("Encoding","WinAnsiEncoding");assert!(verify(&pdf,&simple,NO_CMAP).is_err());
 let mut malformed=ORIGINAL.to_vec();let count=u16::from_be_bytes([malformed[4],malformed[5]])as usize;
 let entry=(0..count).map(|i|12+i*16).find(|at|&malformed[*at..*at+4]==b"cmap").unwrap();malformed[entry+12..entry+16].copy_from_slice(&0u32.to_be_bytes());
 let face=ttf_parser::Face::parse(&malformed,0).unwrap();assert!(face.raw_face().table(ttf_parser::Tag::from_bytes(b"cmap")).is_some());assert!(face.tables().cmap.is_none());
 assert!(verify(&pdf,&font,&malformed).is_err(),"Malformed-present table must not enter the absent-table exception");
}

#[test]
fn cid_without_cmap_keeps_mapping_and_metric_refusals(){
 for kind in ["width","nonfinite","gid","odd-map","encoding","ambiguous"]{
  let(mut pdf,mut font)=fixture("shifted");let id=child_id(&font);
  match kind{
   "width"|"nonfinite"=>{let child=pdf.get_dictionary_mut(id).unwrap();let widths=child.get_mut(b"W").unwrap().as_array_mut().unwrap();widths[1]=Object::Array(vec![Object::Real(if kind=="nonfinite"{f32::INFINITY}else{9000.})]);},
   "gid"|"odd-map"=>{let value=pdf.get_dictionary(id).unwrap().get(b"CIDToGIDMap").unwrap().as_reference().unwrap();let bytes=&mut pdf.get_object_mut(value).unwrap().as_stream_mut().unwrap().content;if kind=="gid"{bytes[0x200*2..0x200*2+2].copy_from_slice(&u16::MAX.to_be_bytes());}else{bytes.push(0);}},
   "encoding"=>font.set("Encoding","Identity-V"),
   "ambiguous"=>{let value=font.get(b"ToUnicode").unwrap().as_reference().unwrap();let map=&mut pdf.get_object_mut(value).unwrap().as_stream_mut().unwrap().content;map.extend_from_slice(b" 1 beginbfchar <0200> <004F> endbfchar");},
   _=>unreachable!(),
  }
  assert!(verify(&pdf,&font,NO_CMAP).is_err(),"{kind} must not gain map-free admission");
 }
}

#[test]
fn cid_without_cmap_rejects_declared_but_out_of_bounds_cmap(){
 let(pdf,font)=fixture("identity");let mut malformed=ORIGINAL.to_vec();let count=u16::from_be_bytes([malformed[4],malformed[5]])as usize;
 let entry=(0..count).map(|i|12+i*16).find(|at|&malformed[*at..*at+4]==b"cmap").unwrap();malformed[entry+8..entry+12].copy_from_slice(&u32::MAX.to_be_bytes());
 let face=ttf_parser::Face::parse(&malformed,0).unwrap();assert!(face.tables().cmap.is_none());assert!(face.raw_face().table(ttf_parser::Tag::from_bytes(b"cmap")).is_none());
 assert!(face.raw_face().table_records.into_iter().any(|record|record.tag==ttf_parser::Tag::from_bytes(b"cmap")));
 assert!(verify(&pdf,&font,&malformed).is_err(),"A declared invalid cmap must not be treated as deliberately absent");
}
