//! Authored public preservation fixture for page organization transports.
use lopdf::{dictionary, Document, Object, Stream};
use std::{fs,path::Path};

pub const LABELS:[&str;3]=["Page one content","Page two content","Page three content"];
pub fn write(path:&Path)->Vec<u8>{
 let mut d=Document::with_version("1.7");let pages=d.new_object_id();
 let ids:Vec<_>=(0..3).map(|_|d.new_object_id()).collect();
 let font=d.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica","Encoding"=>"WinAnsiEncoding"});
 let tree=d.new_object_id();let structure=d.new_object_id();let elems:Vec<_>=(0..3).map(|_|d.new_object_id()).collect();
 let field=d.new_object_id();
 let appearance=d.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),170.into(),30.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}},b"q 1 g 0 0 170 30 re f 0 G 0 0 170 30 re S BT /F1 10 Tf 0 g 5 10 Td (Retained answer) Tj ET Q".to_vec()));
 let widget=d.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","Parent"=>field,"P"=>ids[1],"Rect"=>vec![30.into(),80.into(),200.into(),110.into()],"F"=>4,"AP"=>dictionary!{"N"=>appearance}});
 d.objects.insert(field,dictionary!{"FT"=>"Tx","T"=>Object::string_literal("Shared answer"),"V"=>Object::string_literal("Retained answer"),"DA"=>Object::string_literal("/F1 10 Tf 0 g"),"Kids"=>vec![widget.into()]}.into());
 for(i,id)in ids.iter().enumerate(){
  let stream=d.add_object(Stream::new(dictionary!{},format!("q {} 0.2 0.4 rg 25 140 180 25 re f Q /P <</MCID 0>> BDC BT /F1 12 Tf 30 180 Td ({}) Tj ET EMC",0.2+i as f32*0.2,LABELS[i]).into_bytes()));
  let mut annots=vec![];
  if i<2{
   let target=if i==0{ids[2]}else{*id};
   let link=d.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Link","P"=>*id,"Rect"=>vec![30.into(),30.into(),180.into(),50.into()],"Border"=>vec![0.into(),0.into(),0.into()],"Dest"=>vec![target.into(),Object::Name(b"XYZ".to_vec()),30.into(),180.into(),Object::Null]});annots.push(link.into());
  }
  if i==1{annots.push(widget.into());}
  let mut page=dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),300.into(),240.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>stream,"StructParents"=>i as i64,"Annots"=>annots};
  if i==1{page.set("Rotate",90);page.set("UserUnit",1.7f32);page.set("CropBox",vec![10.into(),20.into(),290.into(),220.into()]);}
  if i==2{page.set("Rotate",270);}
  d.objects.insert(*id,page.into());
  d.objects.insert(elems[i],dictionary!{"Type"=>"StructElem","S"=>"P","P"=>structure,"Pg"=>*id,"K"=>0}.into());
 }
 d.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>3,"Kids"=>ids.iter().copied().map(Object::Reference).collect::<Vec<_>>()}.into());
 d.objects.insert(structure,dictionary!{"Type"=>"StructElem","S"=>"Document","P"=>tree,"K"=>elems.iter().copied().map(Object::Reference).collect::<Vec<_>>()}.into());
 let mut nums=Vec::new();for(i,id)in elems.iter().enumerate(){nums.push(Object::Integer(i as i64));nums.push(Object::Array(vec![(*id).into()]));}
 let parent_tree=d.add_object(dictionary!{"Nums"=>nums});
 d.objects.insert(tree,dictionary!{"Type"=>"StructTreeRoot","K"=>structure,"ParentTree"=>parent_tree,"ParentTreeNextKey"=>3}.into());
 let outlines=d.new_object_id();let bookmarks:Vec<_>=(0..3).map(|_|d.new_object_id()).collect();
 for(i,id)in bookmarks.iter().enumerate(){
  let mut item=dictionary!{"Title"=>Object::string_literal(format!("Chapter {}",i+1)),"Parent"=>outlines,"Dest"=>vec![ids[i].into(),Object::Name(b"Fit".to_vec())]};
  if i>0{item.set("Prev",bookmarks[i-1]);}if i<2{item.set("Next",bookmarks[i+1]);}d.objects.insert(*id,item.into());
 }
 d.objects.insert(outlines,dictionary!{"Type"=>"Outlines","First"=>bookmarks[0],"Last"=>bookmarks[2],"Count"=>3}.into());
 let catalog=d.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages,"StructTreeRoot"=>tree,"MarkInfo"=>dictionary!{"Marked"=>true},"Lang"=>Object::string_literal("en-US"),"Outlines"=>outlines,"AcroForm"=>dictionary!{"Fields"=>vec![field.into()],"DR"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"DA"=>Object::string_literal("/F1 10 Tf 0 g"),"NeedAppearances"=>false}});
 let info=d.add_object(dictionary!{"Title"=>Object::string_literal("Folio page organization QA"),"Subject"=>Object::string_literal("Public authored preservation fixture"),"FolioCustom"=>Object::string_literal("keep-me")});
 d.trailer.set("Root",catalog);d.trailer.set("Info",info);let mut bytes=Vec::new();d.save_to(&mut bytes).unwrap();fs::write(path,&bytes).unwrap();bytes
}
