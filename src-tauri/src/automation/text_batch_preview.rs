//! Strict explicit batches; every selection belongs to the original snapshot.
use super::{invalid,text_preview::{TextEdit,FileProof,valid_hash},font_inputs::{FontChoice,FontInputs},inspection::bounded_json};
use crate::{DocumentInfo,EngineResult,PdfEngine,TextBatchEdit,ExportRequest};
use serde::{Deserialize,Serialize};
use std::{collections::BTreeSet,path::Path};
use serde_json::Value;
use sha2::{Digest,Sha256};
use std::{fs,io::Write};

pub(crate) const MAX_REQUEST:usize=512*1024;
pub(crate) const MAX_MANIFEST:usize=512*1024;

#[derive(Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct BatchEdit{
    pub version:u32,
    pub edits:Vec<TextEdit>,
}
impl BatchEdit{
    pub fn validate(&self)->EngineResult<()>{
        if self.version!=1||self.edits.is_empty()||self.edits.len()>64{return Err(invalid("Use version 1 and 1 to 64 inspected text edits."));}
        let mut selections=BTreeSet::new();let mut pages=BTreeSet::new();
        for edit in &self.edits{edit.validate()?;if !selections.insert((edit.page,edit.selection_id.as_str())){return Err(invalid("Use each inspected text selection only once."));}pages.insert(edit.page);}
        if pages.len()>32{return Err(invalid("A text batch can change at most 32 pages."));}
        bounded_json(self,MAX_REQUEST)?;Ok(())
    }
    pub fn read(path:&Path)->EngineResult<Self>{
        let bytes=super::text_preview::read_bounded(path,MAX_REQUEST as u64)?;
        let request:Self=serde_json::from_slice(&bytes).map_err(|_|invalid("The batch request must match its declared schema."))?;
        request.validate()?;Ok(request)
    }
}

#[derive(Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct BatchItem{
    pub page:u32,pub selection_id:String,pub old_text:String,pub replacement:String,
    #[serde(default,skip_serializing_if="Option::is_none",deserialize_with="super::paragraph_preview::non_null")]
    pub substitute_font:Option<FontChoice>,
}
#[derive(Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct BatchPage{
    pub page:u32,pub before:FileProof,pub after:FileProof,
}
#[derive(Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct BatchMetadata{
    pub version:u32,pub edits:Vec<BatchItem>,pub pages:Vec<BatchPage>,
}
impl BatchMetadata{
    pub fn validate(&self)->EngineResult<()>{
        if self.version!=1||self.edits.is_empty()||self.edits.len()>64||self.pages.is_empty()||self.pages.len()>32{return Err(invalid("Invalid batch preview metadata."));}
        let mut selections=BTreeSet::new();let mut requested_pages=BTreeSet::new();
        for edit in &self.edits{
            if edit.old_text.len()>16*1024||!selections.insert((edit.page,edit.selection_id.as_str())){return Err(invalid("Invalid batch edit metadata."));}
            if let Some(font)=&edit.substitute_font{font.validate()?;}
            TextEdit{page:edit.page,selection_id:edit.selection_id.clone(),replacement:edit.replacement.clone(),font_id:edit.substitute_font.as_ref().map(|f|f.font_id.clone())}.validate()?;
            requested_pages.insert(edit.page);
        }
        let mut previous=0;
        for page in &self.pages{
            if page.page<=previous||page.page>10000||[&page.before,&page.after].iter().any(|p|!valid_hash(&p.sha256)||p.bytes==0||p.bytes>32*1024*1024){return Err(invalid("Invalid batch page image proof."));}
            previous=page.page;
        }
        if requested_pages!=self.pages.iter().map(|p|p.page).collect(){return Err(invalid("Batch images must cover every changed page exactly once."));}
        bounded_json(self,MAX_MANIFEST)?;Ok(())
    }
}
pub(crate) fn valid_selection(id:&str)->bool{id.strip_prefix("batch-text-v1-").is_some_and(valid_hash)}
pub(crate) fn image_name(page:u32,primary:u32,before:bool)->String{
    let side=if before{"before"}else{"after"};if page==primary{format!("{side}.png")}else{format!("page-{page}-{side}.png")}
}
pub(crate) fn add_response(mut value:Value,batch:&BatchMetadata,primary:u32,id:&str,bundle:&Path)->EngineResult<Value>{
    let object=value.as_object_mut().ok_or_else(||invalid("Invalid batch response."))?;object.remove("oldText");object.remove("replacement");
    value["batchId"]=serde_json::json!(id);value["editCount"]=serde_json::json!(batch.edits.len());
    value["pages"]=Value::Array(batch.pages.iter().map(|page|{
        let item=super::path_result(serde_json::json!({"page":page.page}),"before",&bundle.join(image_name(page.page,primary,true)));
        super::path_result(item,"after",&bundle.join(image_name(page.page,primary,false)))
    }).collect());
    bounded_json(&value,256*1024)?;Ok(value)
}
pub(super) fn preview(engine:&PdfEngine,info:&DocumentInfo,request:BatchEdit,parent:&Path,password:Option<&str>,original:&super::original_snapshot::OriginalSnapshot,fonts:&FontInputs)->EngineResult<Value>{
    use super::{inspection,io,path_result,selected,Source};
    use super::text_preview::{copy_hash,PreviewManifest,MAX_BUNDLE};
    request.validate()?;
    let parent=parent.canonicalize().map_err(io)?;
    let mut items=Vec::with_capacity(request.edits.len());let mut native=Vec::with_capacity(request.edits.len());
    let mut source_fingerprint=None;let mut unchanged=true;let mut page_numbers=BTreeSet::new();
    for edit in request.edits{
        let(run,hash)=inspection::resolve_any(engine,info,edit.page,&edit.selection_id)?;
        if source_fingerprint.as_ref().is_some_and(|expected|expected!=&hash){return Err(invalid("Batch selections must share one original source."));}
        source_fingerprint=Some(hash);
        let substitute_font=edit.font_id.as_deref().map(|id|fonts.get(id).and_then(|asset|FontChoice::from_asset(&asset))).transpose()?;
        let(path,expected,old_text)=match run{
            inspection::ResolvedText::Page(run)=>(vec![run.object_index],run.text.clone(),run.source_text.unwrap_or(run.text)),
            inspection::ResolvedText::Nested(run)=>(run.object_path,run.text.clone(),run.text),
        };
        unchanged&=old_text==edit.replacement&&edit.font_id.is_none();page_numbers.insert(edit.page);
        native.push(TextBatchEdit{page_index:edit.page as usize-1,object_path:path,expected_text:expected,replacement:edit.replacement.clone(),font_id:edit.font_id});
        items.push(BatchItem{page:edit.page,selection_id:edit.selection_id,old_text,replacement:edit.replacement,substitute_font});
    }
    let source_fingerprint=source_fingerprint.ok_or_else(||invalid("A batch needs at least one edit."))?;
    let semantic=bounded_json(&items,MAX_MANIFEST)?;
    let mut digest=Sha256::new();digest.update(b"folio-text-batch-v1\0");digest.update(source_fingerprint.as_bytes());digest.update(&semantic);
    let selection_id=format!("batch-text-v1-{}",digest.finalize().iter().map(|b|format!("{b:02x}")).collect::<String>());
    let _font_install=fonts.install(&engine.fonts())?;
    let changed=engine.replace_text_batch(&info.id,&native)?;
    let candidate=Source{engine,info:changed.document};
    if changed.page_indices!=(0..info.pages.len()).collect::<Vec<_>>()||candidate.info.pages.len()!=info.pages.len(){return Err(invalid("Batch editing changed the source page order."));}
    let staging=tempfile::Builder::new().prefix("Folio preview-").tempdir_in(parent).map_err(io)?;
    let folder=staging.path();let pdf=folder.join("candidate.pdf");
    let candidate_proof=if unchanged{original.write_candidate(&pdf)?}else{
        engine.export_pdf(ExportRequest{pages:selected(&candidate.info,None)?,flatten:false},&pdf)?;
        copy_hash(&pdf,MAX_BUNDLE,std::io::sink())?
    };
    let reopened=match password{Some(p)=>engine.open_document_with_password(&pdf,p)?,None=>engine.open_document(&pdf)?};
    let reopened=Source{engine,info:reopened};let primary=*page_numbers.first().ok_or_else(||invalid("A batch needs at least one page."))?;
    let mut used=candidate_proof.bytes;let mut pages=Vec::with_capacity(page_numbers.len());
    for page in page_numbers{
        let mut images=Vec::with_capacity(2);
        for (before,document) in [(true,info),(false,&reopened.info)]{
            let png=engine.render_page_plan(selected(document,Some(page))?.remove(0),1200)?;
            if png.is_empty()||png.len()>32*1024*1024{return Err(invalid("A batch page preview exceeds its image limit."));}
            used=used.checked_add(png.len() as u64).filter(|n|*n<=MAX_BUNDLE).ok_or_else(||invalid("The batch preview exceeds its bundle limit."))?;
            let path=folder.join(image_name(page,primary,before));
            let mut file=fs::OpenOptions::new().write(true).create_new(true).open(&path).map_err(io)?;
            file.write_all(&png).map_err(io)?;file.sync_all().map_err(io)?;drop(file);
            images.push(copy_hash(&path,32*1024*1024,std::io::sink())?);
        }
        let after=images.pop().unwrap();let before=images.pop().unwrap();pages.push(BatchPage{page,before,after});
    }
    let manifest=PreviewManifest{version:6,source_fingerprint,page:primary,selection_id,old_text:String::new(),replacement:String::new(),substitute_font:None,paragraph:None,form:None,
        candidate:candidate_proof,before:pages[0].before.clone(),after:pages[0].after.clone(),batch:Some(BatchMetadata{version:1,edits:items,pages})};
    manifest.validate()?;let bytes=bounded_json(&manifest,MAX_MANIFEST)?;
    if manifest.bytes().checked_add(bytes.len() as u64).is_none_or(|n|n>MAX_BUNDLE){return Err(invalid("The batch preview exceeds its bundle limit."));}
    let manifest_hash=Sha256::digest(&bytes).iter().map(|b|format!("{b:02x}")).collect::<String>();
    let mut file=fs::OpenOptions::new().write(true).create_new(true).open(folder.join("manifest.json")).map_err(io)?;file.write_all(&bytes).map_err(io)?;file.sync_all().map_err(io)?;drop(file);
    let mut result=serde_json::json!({"bundleName":folder.file_name().and_then(|s|s.to_str()).ok_or_else(||invalid("Invalid preview folder name."))?,"manifestSha256":manifest_hash,"page":primary,"sourceFingerprint":manifest.source_fingerprint,"candidateSha256":manifest.candidate.sha256,"protected":info.protection.is_some()});
    for(field,name)in [("folder",None),("candidate",Some("candidate.pdf")),("before",Some("before.png")),("after",Some("after.png"))]{
        result=path_result(result,field,&name.map_or_else(||folder.to_owned(),|name|folder.join(name)));
    }
    result=add_response(result,manifest.batch.as_ref().unwrap(),primary,&manifest.selection_id,folder)?;
    let _=staging.keep();Ok(result)
}

#[cfg(test)]
mod tests{
    use super::*;
    use serde_json::{json,Value};
    fn valid()->Value{json!({"version":1,"edits":[{"page":1,"selectionId":format!("text-v1-{}","a".repeat(64)),"replacement":"First"},{"page":2,"selectionId":format!("nested-text-v1-{}","b".repeat(64)),"replacement":"Second"}]})}
    #[test]
    fn batch_preview_request_accepts_bounded_root_nested_and_explicit_font(){
        let mut value=valid();serde_json::from_value::<BatchEdit>(value.clone()).unwrap().validate().unwrap();
        value["edits"][0]["fontId"]=json!("f".repeat(64));serde_json::from_value::<BatchEdit>(value).unwrap().validate().unwrap();
    }
    #[test]
    fn batch_preview_request_strict_version_members_duplicates_and_limits(){
        for value in [json!({"version":0,"edits":valid()["edits"]}),json!({"version":1,"edits":[]}),json!({"version":1,"edits":null}),json!({"version":1,"edits":valid()["edits"],"path":"untrusted"})]{
            assert!(serde_json::from_value::<BatchEdit>(value).map_or(true,|edit|edit.validate().is_err()));
        }
        let mut value=valid();value["edits"][1]=value["edits"][0].clone();assert!(serde_json::from_value::<BatchEdit>(value).unwrap().validate().is_err());
        let mut value=valid();value["edits"][0]["replacement"]=json!("x".repeat(4097));assert!(serde_json::from_value::<BatchEdit>(value).unwrap().validate().is_err());
        let mut value=valid();value["edits"][0]["fontId"]=Value::Null;assert!(serde_json::from_value::<BatchEdit>(value).is_err());
        let value=json!({"version":1,"edits":vec![valid()["edits"][0].clone();65]});assert!(serde_json::from_value::<BatchEdit>(value).unwrap().validate().is_err());
        let rows=(1..=33).map(|page|json!({"page":page,"selectionId":format!("text-v1-{}",format!("{page:064x}")),"replacement":"Next"})).collect::<Vec<_>>();
        assert!(serde_json::from_value::<BatchEdit>(json!({"version":1,"edits":rows})).unwrap().validate().is_err());
    }
    fn input(path:&Path){
        use lopdf::{dictionary,Document,Object,Stream};
        let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
        let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
        let leaf=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),200.into(),30.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}},b"BT /F1 12 Tf 1 0 0 1 5 10 Tm (Shared) Tj ET".to_vec()));
        let mut kids=vec![];
        for stream in [b"BT /F1 12 Tf 1 0 0 1 30 330 Tm (Alpha) Tj ET BT /F1 12 Tf 1 0 0 1 30 280 Tm (Beta) Tj ET q 1 0 0 1 30 150 cm /Shared Do Q".as_slice(),b"BT /F1 12 Tf 1 0 0 1 30 330 Tm (Gamma) Tj ET q 1 0 0 1 30 150 cm /Shared Do Q"]{
            let content=pdf.add_object(Stream::new(dictionary!{},stream.to_vec()));
            let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),400.into(),500.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font},"XObject"=>dictionary!{"Shared"=>leaf}},"Contents"=>content});kids.push(Object::Reference(page));
        }
        pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>2,"Kids"=>kids}.into());let catalog=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",catalog);pdf.save(path).unwrap();
    }
    fn edits(engine:&PdfEngine,path:&Path,unchanged:bool)->Vec<Value>{
        use crate::automation::{execute,Request};
        let mut rows=vec![];
        for (page,old,new) in [(1,"Alpha","First"),(1,"Beta","Second"),(2,"Gamma","Third")]{
            let listed=execute(engine,Request::InspectText{input:path.into(),page,offset:0},None).unwrap();
            let run=listed["runs"].as_array().unwrap().iter().find(|run|run["text"]==old).unwrap();
            rows.push(json!({"page":page,"selectionId":run["selectionId"],"replacement":if unchanged{old}else{new}}));
        }
        let listed=execute(engine,Request::InspectNestedText{input:path.into(),page:1,offset:0},None).unwrap();
        rows.push(json!({"page":1,"selectionId":listed["runs"][0]["selectionId"],"replacement":if unchanged{"Shared"}else{"Only here"}}));rows
    }
    #[test]
    fn batch_preview_native_publish_save_and_late_failure_are_atomic(){
        use crate::automation::{execute,Request};
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("Original.pdf");input(&path);let original=std::fs::read(&path).unwrap();
        let parent=dir.path().join("previews");std::fs::create_dir(&parent).unwrap();let request=dir.path().join("edits.json");
        let engine=PdfEngine::start(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
        let rows=edits(&engine,&path,false);std::fs::write(&request,serde_json::to_vec(&json!({"version":1,"edits":rows})).unwrap()).unwrap();
        let result=execute(&engine,Request::PreviewTextBatch{input:path.clone(),request_file:request.clone(),parent:parent.clone()},None).unwrap();
        assert_eq!(result["editCount"],4);assert_eq!(result["pages"].as_array().unwrap().len(),2);
        for page in result["pages"].as_array().unwrap(){for side in ["before","after"]{assert!(Path::new(page[side].as_str().unwrap()).is_file());}}
        let output=dir.path().join("Saved.pdf");let folder=std::path::PathBuf::from(result["folder"].as_str().unwrap());
        execute(&engine,Request::SaveTextPreview{input:folder.clone(),manifest_hash:result["manifestSha256"].as_str().unwrap().into(),output:output.clone()},None).unwrap();
        assert_eq!(std::fs::read(&output).unwrap(),std::fs::read(folder.join("candidate.pdf")).unwrap());
        let saved=engine.open_document(&output).unwrap();let first=engine.extract_text(&saved.id,0).unwrap();assert!(first.contains("First")&&first.contains("Second")&&first.contains("Only here"),"{first}");
        let second=engine.extract_text(&saved.id,1).unwrap();assert!(second.contains("Third")&&second.contains("Shared"),"{second}");engine.close_document(&saved.id).unwrap();
        let children=std::fs::read_dir(&parent).unwrap().count();let mut failing=edits(&engine,&path,false);failing[0]["replacement"]=json!("W".repeat(300));
        std::fs::write(&request,serde_json::to_vec(&json!({"version":1,"edits":failing})).unwrap()).unwrap();
        assert!(execute(&engine,Request::PreviewTextBatch{input:path.clone(),request_file:request,parent:parent.clone()},None).is_err());
        assert_eq!(std::fs::read_dir(parent).unwrap().count(),children);assert_eq!(std::fs::read(path).unwrap(),original);
    }
    #[test]
    fn batch_preview_native_noop_copies_exact_original_bytes(){
        use crate::automation::{execute,Request};
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("Original.pdf");input(&path);let original=std::fs::read(&path).unwrap();
        let engine=PdfEngine::start(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
        let request=dir.path().join("edits.json");std::fs::write(&request,serde_json::to_vec(&json!({"version":1,"edits":edits(&engine,&path,true)})).unwrap()).unwrap();
        let result=execute(&engine,Request::PreviewTextBatch{input:path,request_file:request,parent:dir.path().into()},None).unwrap();
        assert_eq!(std::fs::read(result["candidate"].as_str().unwrap()).unwrap(),original);
    }
}
