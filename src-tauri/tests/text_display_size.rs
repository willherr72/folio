use folio_engine::PdfEngine;
use lopdf::{dictionary,Document,Object,Stream};
use std::{fs,path::PathBuf};

fn engine()->PdfEngine{PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture(matrix:[f32;4],unit:f32,form:bool,group:bool)->Document{
 let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
 let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
 let fonts=dictionary!{"Font"=>dictionary!{"F1"=>font}};
 let mut resources=fonts.clone();
 let content=if form{
  let leaf=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),100.into(),30.into()],"Matrix"=>vec![2.into(),0.into(),0.into(),2.into(),0.into(),0.into()],"Resources"=>fonts.clone()},b"BT /F1 1 Tf 1 0 0 1 5 10 Tm (Old) Tj ET".to_vec()));
  let wrapper=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),200.into(),60.into()],"Matrix"=>vec![3.into(),0.into(),0.into(),3.into(),0.into(),0.into()],"Resources"=>dictionary!{"XObject"=>dictionary!{"Leaf"=>leaf}}},b"/Leaf Do".to_vec()));
  resources.set("XObject",dictionary!{"Wrapper"=>wrapper});"q 2 0 0 2 100 200 cm /Wrapper Do Q".to_string()
 }else{format!("q {} {} {} {} 200 300 cm BT /F1 1 Tf 1 0 0 1 0 0 Tm (Old) Tj {} ET Q",matrix[0],matrix[1],matrix[2],matrix[3],if group{"( words) Tj"}else{""})};
 let stream=pdf.add_object(Stream::new(dictionary!{},content.into_bytes()));
 let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),800.into(),800.into()],"UserUnit"=>Object::Real(unit),"Resources"=>resources,"Contents"=>stream});
 pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1}.into());
 let catalog=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",catalog);pdf
}
fn measured(value:impl serde::Serialize)->f32{serde_json::to_value(value).unwrap()["displayFontSize"].as_f64().unwrap_or(-1.) as f32}

#[test]
fn direct_size_labels_include_text_transforms_and_physical_units_without_changing_operands(){
 let engine=engine();let temp=tempfile::tempdir().unwrap();
 for (label,matrix,scale) in [("scale",[12.,0.,0.,12.],12.),("turn",[0.,12.,-12.,0.],12.),("reflect",[-12.,0.,0.,12.],12.),("stretch",[8.,0.,0.,12.],12.),("shear",[12.,0.,5.,12.],13.)]{for unit in [1.,1.7]{
  let path=temp.path().join(format!("{label}-{unit}.pdf"));fixture(matrix,unit,false,false).save(&path).unwrap();let original=fs::read(&path).unwrap();let source=engine.open_document(&path).unwrap();
  let runs=engine.list_text_runs(&source.id,0).unwrap();assert_eq!(runs.runs.len(),1);let run=&runs.runs[0];
  assert!((run.font_size-unit).abs()<0.0001,"raw Tf remains unchanged");assert!((measured(run)-scale*unit).abs()<0.0001,"{label} unit{unit}: {:?}",serde_json::to_value(run).unwrap());
  let occurrence=engine.list_occurrence_text_runs(&source.id,0).unwrap();assert!((measured(&occurrence.runs[0])-scale*unit).abs()<0.0001);
  if run.supported {let copy=engine.replace_text(&source.id,0,run.object_index,&run.text,&run.text).unwrap();assert_eq!(engine.render_page(&copy.id,0,600).unwrap(),engine.render_page(&source.id,0,600).unwrap());engine.close_document(&copy.id).unwrap();}
  assert_eq!(fs::read(&path).unwrap(),original);engine.close_document(&source.id).unwrap();
 }}
}

#[test]
fn nested_form_and_group_size_labels_include_the_complete_transform(){
 let engine=engine();let temp=tempfile::tempdir().unwrap();
 for unit in [1.,1.7]{for form in [false,true]{
  let path=temp.path().join(format!("form-{form}-unit-{unit}.pdf"));fixture([12.,0.,0.,12.],unit,form,!form).save(&path).unwrap();let original=fs::read(&path).unwrap();let source=engine.open_document(&path).unwrap();
  if form {
   let runs=engine.list_form_text_runs(&source.id,0).unwrap();assert_eq!(runs.runs.len(),1,"{runs:?}");assert!((runs.runs[0].font_size-unit).abs()<0.0001);assert!((measured(&runs.runs[0])-12.*unit).abs()<0.0001);
   let occurrences=engine.list_occurrence_form_text_runs(&source.id,0).unwrap();assert!((measured(&occurrences.runs[0])-12.*unit).abs()<0.0001);
  }else{
   let group=engine.inspect_text_group(&source.id,0,&[0,1]).unwrap();assert!((group.font_size-unit).abs()<0.0001);assert!((measured(&group)-12.*unit).abs()<0.0001);
   let copy=engine.replace_text_group(&source.id,0,&[0,1],&group.text,&group.text).unwrap();assert_eq!(engine.render_page(&copy.id,0,600).unwrap(),engine.render_page(&source.id,0,600).unwrap());engine.close_document(&copy.id).unwrap();
  }
  assert_eq!(fs::read(path).unwrap(),original);engine.close_document(&source.id).unwrap();
 }}
}
