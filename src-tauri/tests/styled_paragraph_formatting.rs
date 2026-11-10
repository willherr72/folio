use folio_engine::{ExportRequest,PagePlan,PdfEngine,StyledParagraphLayoutRequest};
use lopdf::{dictionary,Document,Object,Stream};
use serde_json::{json,Value};
use std::path::{Path,PathBuf};
static REGISTRY:std::sync::Mutex<()>=std::sync::Mutex::new(());
fn root()->PathBuf{PathBuf::from(env!("CARGO_MANIFEST_DIR"))}
fn engine()->PdfEngine{PdfEngine::start(root().join("resources/pdfium/pdfium.dll")).unwrap()}
fn fixture(path:&Path){
 let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
 let f=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
 let g=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica-Bold"});
 let stream=pdf.add_object(Stream::new(dictionary!{},b"BT /F1 12 Tf 50 500 Td (One ) Tj ET BT /F2 20 Tf 0 .5 0 rg 95 500 Td (Two) Tj ET BT /FolioParagraph 12 Tf 50 100 Td (Neighbor) Tj ET".to_vec()));
 let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),500.into(),600.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>f,"F2"=>g,"FolioParagraph"=>f}},"Contents"=>stream});
 pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());let catalog=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",catalog);pdf.save(path).unwrap();
}
fn draft()->Value{json!({"objectIndices":[0,1],"expectedText":"One Two","runs":[{"text":"plain ","styleId":0},{"text":"large ","styleId":2},{"text":"bold","styleId":1}],"styleChanges":[{"styleId":2,"sourceStyleId":0,"fontSize":18,"color":"#993366"}],"bounds":{"x":50,"y":80,"width":350,"height":180},"alignment":"left","lineSpacing":1.2})}
fn request(v:Value)->StyledParagraphLayoutRequest{serde_json::from_value(v).unwrap()}
fn save(e:&PdfEngine,doc:&folio_engine::DocumentInfo,path:&Path){e.export_pdf(ExportRequest{flatten:false,pages:vec![PagePlan{crop: None,id:"p".into(),source_id:doc.id.clone(),page_index:0,width:doc.pages[0].width,height:doc.pages[0].height,rotation:0,overlays:vec![]}]},path).unwrap();}
#[test]
fn derived_styles_change_only_their_runs_and_resolve_from_original_palette(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("input.pdf");fixture(&path);let source=e.open_document(&path).unwrap();let original=e.source_bytes(&source.id).unwrap();
 let edited=e.preview_styled_paragraph(&source.id,0,&request(draft())).unwrap();let runs=e.list_text_runs(&edited.id,0).unwrap().runs;
 for (text,size,font) in [("plain",12.,"Courier"),("large",18.,"Courier"),("bold",20.,"Helvetica-Bold"),("Neighbor",12.,"Courier")]{let r=runs.iter().find(|r|r.text.trim()==text).unwrap();assert_eq!(r.font_size,size);assert_eq!(r.font_name,font);}
 let selected=e.inspect_styled_paragraph(&edited.id,0,&runs.iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect::<Vec<_>>()).unwrap();assert!(selected.styles.iter().any(|s|s.font_size==18.&&s.color.eq_ignore_ascii_case("#993366")));
 let mut a=draft();a["styleChanges"].as_array_mut().unwrap().push(json!({"styleId":0,"sourceStyleId":1,"fontSize":10}));
 let mut b=a.clone();b["styleChanges"].as_array_mut().unwrap().reverse();let a=request(a);let b=request(b);
 assert_eq!(e.layout_styled_paragraph(&source.id,0,&a).unwrap(),e.layout_styled_paragraph(&source.id,0,&b).unwrap());
 let changed=e.preview_styled_paragraph(&source.id,0,&a).unwrap();let runs=e.list_text_runs(&changed.id,0).unwrap().runs;assert!(runs.iter().any(|r|r.text.trim()=="large"&&r.font_name=="Courier"&&r.font_size==18.));assert!(runs.iter().any(|r|r.text.trim()=="plain"&&r.font_name=="Helvetica-Bold"&&r.font_size==10.));
 assert_eq!(e.source_bytes(&source.id).unwrap(),original);assert_eq!(std::fs::read(&path).unwrap(),original.as_ref());
 for id in [&source.id,&edited.id,&changed.id]{e.close_document(id).unwrap();}
}
#[test]
fn malformed_or_unused_style_definitions_refuse_atomically(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("input.pdf");fixture(&path);let source=e.open_document(&path).unwrap();let original=e.source_bytes(&source.id).unwrap();
 let definitions=vec![json!([{"styleId":2,"sourceStyleId":2}]),json!([{"styleId":3,"sourceStyleId":0}]),json!([{"styleId":2,"sourceStyleId":0},{"styleId":2,"sourceStyleId":1}]),json!([{"styleId":64,"sourceStyleId":0}]),json!([{"styleId":2,"sourceStyleId":0,"fontSize":0}]),json!([{"styleId":2,"sourceStyleId":0,"fontSize":301}]),json!([{"styleId":2,"sourceStyleId":0,"color":"#ff000080"}]),json!([{"styleId":2,"sourceStyleId":0,"fontId":"unknown"}])];
 for changes in definitions{let mut v=draft();v["runs"]=json!([{"text":"unchanged","styleId":0}]);v["styleChanges"]=changes;let r=request(v);assert!(e.layout_styled_paragraph(&source.id,0,&r).is_err());assert!(e.preview_styled_paragraph(&source.id,0,&r).is_err());assert_eq!(e.source_bytes(&source.id).unwrap(),original);}
 let mut bad=request(draft());bad.style_changes[0].font_size=Some(f32::NAN);assert!(e.preview_styled_paragraph(&source.id,0,&bad).is_err());e.close_document(&source.id).unwrap();
}
#[test]
fn multiple_substitute_fonts_preserve_resources_and_survive_registry_and_source_release(){
 let _guard=REGISTRY.lock().unwrap();let e=engine();let serif=e.fonts().register(std::fs::read(root().join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();let sans=e.fonts().register(std::fs::read(root().join("../tests/fixtures/shaped-text/DejaVuSans.ttf")).unwrap()).unwrap();
 let temp=tempfile::tempdir().unwrap();let path=temp.path().join("input.pdf");fixture(&path);let original=std::fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();
 let mut v=draft();v["runs"]=json!([{"text":"plain ","styleId":0},{"text":"Cafè ","styleId":2},{"text":"sans ","styleId":3},{"text":"small","styleId":4}]);v["styleChanges"]=json!([{"styleId":2,"sourceStyleId":0,"fontId":serif.id,"fontSize":18,"color":"#993366"},{"styleId":3,"sourceStyleId":1,"fontId":sans.id},{"styleId":4,"sourceStyleId":0,"fontId":serif.id,"fontSize":10}]);
 let edited=e.preview_styled_paragraph(&source.id,0,&request(v)).unwrap();e.fonts().remove(&serif.id).unwrap();e.fonts().remove(&sans.id).unwrap();e.close_document(&source.id).unwrap();let output=temp.path().join("saved.pdf");save(&e,&edited,&output);let reopened=e.open_document(&output).unwrap();
 let runs=e.list_text_runs(&reopened.id,0).unwrap().runs;for (text,id,size) in [("Cafè",&serif.id,18.),("sans",&sans.id,20.),("small",&serif.id,10.)]{let run=runs.iter().find(|r|r.text.trim()==text).unwrap();assert!(run.is_embedded&&run.supported);assert_eq!(run.font_name,format!("Folio{id}Paragraph"));assert_eq!(run.font_size,size);}
 assert!(runs.iter().any(|r|r.text=="Neighbor"&&r.font_name=="Courier"));assert_eq!(e.render_page(&edited.id,0,600).unwrap(),e.render_page(&reopened.id,0,600).unwrap());
 let pdf=Document::load(&output).unwrap();for id in [&serif.id,&sans.id]{let name=format!("Folio{id}Paragraph");let count=pdf.objects.values().filter(|o|o.as_dict().is_ok_and(|d|d.get(b"Subtype").and_then(Object::as_name).ok()==Some(b"Type0")&&d.get(b"BaseFont").and_then(Object::as_name).ok()==Some(name.as_bytes()))).count();assert_eq!(count,1);}
 assert_eq!(std::fs::read(&path).unwrap(),original);for id in [&edited.id,&reopened.id]{e.close_document(id).unwrap();}
}

#[test]
fn explicit_substitute_supplies_missing_subset_glyph_without_changing_other_runs(){
 let _guard=REGISTRY.lock().unwrap();let e=engine();let font=e.fonts().register(std::fs::read(root().join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
 let temp=tempfile::tempdir().unwrap();let path=temp.path().join("subset.pdf");let mut pdf=Document::load(root().join("../tests/fixtures/embedded-text/reportlab-subset.pdf")).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
 let content=pdf.add_object(Stream::new(dictionary!{},b"BT /F2+0 16 Tf 1 0 0 1 30 300 Tm (Caf\x01) Tj ET BT /F1 12 Tf 90 300 Td (mix) Tj ET BT /F1 12 Tf 30 60 Td (Neighbor) Tj ET".to_vec()));pdf.get_dictionary_mut(page).unwrap().set("Contents",content);pdf.save(&path).unwrap();let source=e.open_document(&path).unwrap();let original=e.source_bytes(&source.id).unwrap();let selected=e.inspect_styled_paragraph(&source.id,0,&[0,1]).unwrap();
 let mut v=draft();v["expectedText"]=json!(selected.text);v["bounds"]=json!({"x":20,"y":60,"width":220,"height":140});v["runs"]=json!([{"text":"Café ","styleId":0},{"text":"Cafè ","styleId":2},{"text":"mix","styleId":1}]);
 let error=e.preview_styled_paragraph(&source.id,0,&request(v.clone())).unwrap_err().to_string();assert!(error.contains("U+00E8"),"{error}");v["styleChanges"][0]["fontId"]=json!(font.id);let edited=e.preview_styled_paragraph(&source.id,0,&request(v.clone())).unwrap();
 let runs=e.list_text_runs(&edited.id,0).unwrap().runs;assert!(runs.iter().any(|r|r.text.trim()=="Café"&&r.font_size==16.&&r.is_embedded&&!r.font_name.starts_with("Folio")));assert!(runs.iter().any(|r|r.text.trim()=="Cafè"&&r.font_size==18.&&r.font_name==format!("Folio{}Paragraph",font.id)));
 for text in ["A\u{0301}","\u{03b1}","\u{4e00}"]{v["runs"][1]["text"]=json!(text);let r=request(v.clone());assert!(e.layout_styled_paragraph(&source.id,0,&r).is_err());assert!(e.preview_styled_paragraph(&source.id,0,&r).is_err());}
 assert_eq!(e.source_bytes(&source.id).unwrap(),original);e.fonts().remove(&font.id).unwrap();for id in [&source.id,&edited.id]{e.close_document(id).unwrap();}
}

#[test]
fn palette_limit_and_legacy_json_are_explicit(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("input.pdf");fixture(&path);let source=e.open_document(&path).unwrap();let mut v=draft();v["runs"]=json!([{"text":"plain","styleId":0}]);v.as_object_mut().unwrap().remove("styleChanges");assert!(request(v.clone()).style_changes.is_empty());assert!(e.layout_styled_paragraph(&source.id,0,&request(v.clone())).is_ok());
 v["styleChanges"]=Value::Array((2..64).map(|id|json!({"styleId":id,"sourceStyleId":0})).collect());v["runs"][0]["styleId"]=json!(63);assert!(e.layout_styled_paragraph(&source.id,0,&request(v.clone())).is_ok());
 v["styleChanges"].as_array_mut().unwrap().push(json!({"styleId":64,"sourceStyleId":0}));assert!(e.layout_styled_paragraph(&source.id,0,&request(v)).is_err());e.close_document(&source.id).unwrap();
}

#[test]
fn legacy_and_integral_resources_for_the_same_program_coexist_without_ambiguous_proofs(){
 let _guard=REGISTRY.lock().unwrap();let e=engine();let font=e.fonts().register(std::fs::read(root().join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("input.pdf");fixture(&path);let base=e.open_document(&path).unwrap();
 let selected=e.inspect_text_paragraph(&base.id,0,&[0]).unwrap();let formatting=serde_json::from_value(json!({"fontId":font.id})).unwrap();let legacy=e.preview_text_paragraph_with_format(&base.id,0,&[0],&selected.text,"Legacy",&folio_engine::AnnotationRect{x:50.,y:200.,width:200.,height:80.},&formatting).unwrap();
 let mut pdf=Document::load_mem(&e.source_bytes(&legacy.id).unwrap()).unwrap();let page=*pdf.get_pages().get(&1).unwrap();let resources=pdf.get_dictionary(page).unwrap().get(b"Resources").unwrap().as_dict().unwrap();let fonts=pdf.dereference(resources.get(b"Font").unwrap()).unwrap().1.as_dict().unwrap();let name=format!("Folio{}",font.id);
 let key=fonts.iter().find(|(_,v)|pdf.dereference(v).unwrap().1.as_dict().unwrap().get(b"BaseFont").and_then(Object::as_name).ok()==Some(name.as_bytes())).unwrap().0.clone();
 let data=format!("BT /F1 12 Tf 50 500 Td (One ) Tj ET BT /F2 20 Tf 0 .5 0 rg 95 500 Td (Two) Tj ET BT /{} 12 Tf 50 100 Td <004c00650067006100630079> Tj ET",String::from_utf8(key).unwrap());let stream=pdf.add_object(Stream::new(dictionary!{},data.into_bytes()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);let input=temp.path().join("coexist.pdf");pdf.save(&input).unwrap();
 let source=e.open_document(&input).unwrap();let mut v=draft();v["styleChanges"][0]["fontId"]=json!(font.id);let edited=e.preview_styled_paragraph(&source.id,0,&request(v)).unwrap();let runs=e.list_text_runs(&edited.id,0).unwrap().runs;
 assert!(runs.iter().any(|r|r.text=="Legacy"&&r.font_name==name&&r.supported));assert!(runs.iter().any(|r|r.text.trim()=="large"&&r.font_name==format!("{name}Paragraph")&&r.supported));
 e.fonts().remove(&font.id).unwrap();let output=temp.path().join("saved.pdf");save(&e,&edited,&output);let reopened=e.open_document(&output).unwrap();assert_eq!(e.render_page(&edited.id,0,600).unwrap(),e.render_page(&reopened.id,0,600).unwrap());for id in [&base.id,&legacy.id,&source.id,&edited.id,&reopened.id]{e.close_document(id).unwrap();}
}
