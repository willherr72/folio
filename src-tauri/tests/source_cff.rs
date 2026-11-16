//! Public native transaction coverage over independently authored source CFFs.
use folio_engine::{AnnotationRect,DocumentInfo,EngineError,ExportRequest,PagePlan,ParagraphAlignment,ParagraphFormatting,PdfEngine,StyledParagraphLayoutRequest,StyledParagraphRun,StyledParagraphSelection,TextGroupPreview,TextParagraphSelection};
use lopdf::Document;
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::{collections::BTreeSet,fs,path::{Path,PathBuf}};

type Result<T> = std::result::Result<T,Box<dyn std::error::Error>>;
fn require(value:bool,message:&str)->Result<()> {if value {Ok(())}else{Err(message.into())}}
fn fixtures()->PathBuf {PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/source-cff/transactions-v2")}
fn hash(bytes:&[u8])->String {Sha256::digest(bytes).iter().map(|b|format!("{b:02x}")).collect()}
fn compact(value:&str)->String {value.chars().filter(|c|!c.is_whitespace()).collect()}
fn uniform_order(engine:&PdfEngine,id:&str,page:usize,mode:&str)->Result<()> {
    if mode!="uniform" {return Ok(());}
    let runs=engine.list_occurrence_text_runs(id,page)?.runs;
    require(runs.len()==3 && runs.last().is_some_and(|run|run.text=="Unchanged neighbor"),
        "Uniform paragraph moved behind its following neighbor in source reading order")
}
fn save(engine:&PdfEngine,doc:&DocumentInfo,path:&Path)->Result<()> {
    engine.export_pdf(ExportRequest{flatten:false,pages:doc.pages.iter().enumerate().map(|(page_index,p)|PagePlan{crop:None,id:format!("p{page_index}"),source_id:doc.id.clone(),page_index,width:p.width,height:p.height,rotation:0,overlays:vec![]}).collect()},path)?;Ok(())
}
fn programs(bytes:&[u8])->Result<BTreeSet<String>> {
    let pdf=Document::load_mem(bytes)?;let mut result=BTreeSet::new();
    for object in pdf.objects.values() {
        let Ok(dict)=object.as_dict()else{continue};
        for key in [b"FontFile".as_slice(),b"FontFile2",b"FontFile3"] {
            let Ok(value)=dict.get(key)else{continue};let stream=pdf.dereference(value)?.1.as_stream()?;
            let bytes=if stream.dict.has(b"Filter") {stream.decompressed_content_with_limit(16*1024*1024)?}else{stream.content.clone()};
            result.insert(hash(&bytes));
        }
    }
    require(!result.is_empty(),"Expected original embedded font programs")?;Ok(result)
}
fn request(selection:StyledParagraphSelection)->StyledParagraphLayoutRequest {
    StyledParagraphLayoutRequest{baseline_anchor:selection.baseline_anchor,object_indices:selection.object_indices,expected_text:selection.text,runs:selection.runs,style_changes:vec![],bounds:selection.bounds,alignment:selection.alignment.unwrap_or(ParagraphAlignment::Left),line_spacing:selection.line_spacing.unwrap_or(1.2)}
}

#[derive(Clone)]
enum Draft {
    Single{index:usize,expected:String,replacement:String},
    Group{selection:TextGroupPreview,replacement:String},
    Uniform{selection:TextParagraphSelection,replacement:String,bounds:AnnotationRect},
    Styled{request:StyledParagraphLayoutRequest,path:Option<Vec<usize>>},
}
impl Draft {
    fn inspect(engine:&PdfEngine,id:&str,page:usize,mode:&str,anchor:Option<&str>)->Result<Self> {
        if mode=="nested" {
            let forms=engine.list_occurrence_form_text_runs(id,page)?.runs;
            require(forms.len()==4,"Expected two paragraphs in distinct shared-form invocations")?;
            let first=forms.first().ok_or("Missing shared-form anchor")?;
            let listing=engine.list_form_paragraph_runs(id,page,&first.object_path,&first.text)?;
            let indices=listing.runs.iter().map(|run|run.object_index).collect::<Vec<_>>();
            require(indices.len()==2,"Expected the complete two-style shared-form paragraph")?;
            let selected=engine.inspect_form_styled_paragraph(id,page,&first.object_path,&indices)?;
            return Ok(Self::Styled{request:request(selected),path:Some(first.object_path.clone())});
        }
        let runs=engine.list_occurrence_text_runs(id,page)?.runs;
        let selected:Vec<_>=runs.iter().filter(|run|run.text!="Unchanged neighbor").collect();
        require(!selected.is_empty(),"Missing editable page paragraph")?;
        for run in &selected {require(run.supported,&format!("Source run rejected: {:?}",run.reason))?;}
        let indices=selected.iter().map(|run|run.object_index).collect::<Vec<_>>();
        if mode=="group" {
            if anchor.is_some() && selected.len()==1 {
                return Ok(Self::Single{index:selected[0].object_index,expected:selected[0].text.clone(),replacement:selected[0].text.clone()});
            }
            let selection=engine.inspect_occurrence_text_group(id,page,&indices)?;let replacement=selection.text.clone();
            Ok(Self::Group{selection,replacement})
        }else if mode=="uniform" {
            let selection=if let Some(anchor)=anchor {
                let run=selected.iter().find(|run|run.text.contains(anchor)).ok_or("Missing saved uniform anchor")?;
                engine.suggest_occurrence_text_paragraph(id,page,run.object_index,&run.text)?
            }else{engine.inspect_occurrence_text_paragraph(id,page,&indices)?};
            Ok(Self::Uniform{replacement:selection.text.clone(),bounds:selection.bounds.clone(),selection})
        }else{
            let selection=if let Some(anchor)=anchor {
                let run=selected.iter().find(|run|run.text.contains(anchor)).ok_or("Missing saved styled anchor")?;
                engine.suggest_occurrence_styled_paragraph(id,page,run.object_index,&run.text)?
            }else{engine.inspect_occurrence_styled_paragraph(id,page,&indices)?};
            Ok(Self::Styled{request:request(selection),path:None})
        }
    }
    fn text(&self)->&str {match self {Self::Single{expected,..}=>expected,Self::Group{selection,..}=>&selection.text,Self::Uniform{selection,..}=>&selection.text,Self::Styled{request,..}=>&request.expected_text}}
    fn change_first(&mut self,old:&str,new:&str)->Result<()> {
        match self {
            Self::Single{replacement,..}=>{require(replacement.contains(old),"Single text omitted source operand")?;*replacement=replacement.replacen(old,new,1);},
            Self::Group{replacement,..}=>{require(replacement.contains(old),"Group text omitted source operand")?;*replacement=replacement.replacen(old,new,1);},
            Self::Uniform{replacement,bounds,..}=>{require(replacement.contains(old),"Uniform text omitted source operand")?;*replacement=replacement.replacen(old,new,1);bounds.width=bounds.width.max(160.);},
            Self::Styled{request,..}=>{
                let run=request.runs.iter_mut().find(|run|run.text.contains(old)).ok_or("Styled draft omitted source operand")?;
                run.text=run.text.replacen(old,new,1);request.bounds.width=request.bounds.width.max(160.);
            },
        }
        Ok(())
    }
    fn fit(&mut self,engine:&PdfEngine,id:&str,page:usize)->Result<()> {
        if let Self::Styled{request,path}=self {
            request.bounds=if let Some(path)=path {engine.fit_form_styled_paragraph(id,page,path,request)?}else{engine.fit_occurrence_styled_paragraph(id,page,request)?};
        }
        Ok(())
    }
    fn explicit_lines(&mut self,first:&str,second:&str)->Result<()> {
        match self {
            Self::Uniform{replacement,..}=>*replacement=format!("{first}\n{second}"),
            Self::Styled{request,..}=>{
                let first_style=request.runs.iter().find(|r|r.text.contains(first)).ok_or("Missing first line style")?.style_id;
                let second_style=request.runs.iter().rev().find(|r|r.text.contains(second)).ok_or("Missing second line style")?.style_id;
                request.runs=vec![StyledParagraphRun{text:format!("{first}\n"),style_id:first_style},StyledParagraphRun{text:second.into(),style_id:second_style}];
            },
            _=>{},
        }
        Ok(())
    }
    fn preview(&self,engine:&PdfEngine,id:&str,order:&[usize],selected:usize)->std::result::Result<DocumentInfo,EngineError> {
        match self {
            Self::Single{index,expected,replacement}=>engine.replace_text_occurrence(id,order,selected,*index,expected,replacement,None).map(|value|value.document),
            Self::Group{selection,replacement}=>engine.replace_text_group_occurrence(id,order,selected,&selection.object_indices,&selection.text,replacement).map(|value|value.document),
            Self::Uniform{selection,replacement,bounds}=>engine.preview_text_paragraph_occurrence(id,order,selected,&selection.object_indices,&selection.text,replacement,bounds,&ParagraphFormatting{alignment:selection.alignment.clone(),line_spacing:selection.line_spacing,..Default::default()}).map(|value|value.document),
            Self::Styled{request,path:None}=>engine.preview_styled_paragraph_occurrence(id,order,selected,request).map(|value|value.document),
            Self::Styled{request,path:Some(path)}=>engine.preview_form_styled_paragraph_occurrence(id,order,selected,path,request).map(|value|value.document),
        }
    }
}

fn case(row:&Value,stage:&mut String,out:&Path)->Result<Value> {
    *stage="open".into();let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))?;
    let input=fixtures().join(row["file"].as_str().ok_or("Missing fixture filename")?);let original=fs::read(&input)?;
    require(hash(&original)==row["sha256"].as_str().ok_or("Missing fixture SHA")?,"Changed independently authored input")?;
    let source=engine.open_document(&input)?;let mode=row["mode"].as_str().ok_or("Missing mode")?;
    let old=row["text"].as_str().ok_or("Missing source text")?;let new=row["replacement"].as_str().ok_or("Missing replacement text")?;
    let order=if mode=="occurrences" {vec![0,1,0]}else{vec![0]};let target=order.len()-1;
    let before=(0..source.pages.len()).map(|p|engine.render_page(&source.id,p,900)).collect::<std::result::Result<Vec<_>,_>>()?;
    fs::copy(&input,out.join("original.pdf"))?;
    *stage="inspect".into();let draft=Draft::inspect(&engine,&source.id,0,mode,None)?;
    require(compact(draft.text()).contains(old),"Inspection lost the original source text")?;
    fs::write(out.join("inspected.json"),serde_json::to_vec_pretty(&json!({"text":draft.text(),"mode":mode}))?)?;
    let other_forms=if mode=="nested" {Some(engine.list_occurrence_form_text_runs(&source.id,0)?.runs.into_iter().filter(|r|r.bounds.x>300.).map(|r|json!({"text":r.text,"font":r.font_name,"size":r.font_size,"bounds":r.bounds})).collect::<Vec<_>>())}else{None};
    *stage="no-op".into();let noop=draft.preview(&engine,&source.id,&order,target)?;
    for (page,original_page) in order.iter().enumerate() {require(engine.render_page(&noop.id,page,900)?==before[*original_page],"No-op changed page pixels")?;}
    uniform_order(&engine,&noop.id,target,mode)?;
    save(&engine,&noop,&out.join("noop.pdf"))?;engine.close_document(&noop.id)?;
    *stage="missing-glyph-refusal".into();let mut invalid=draft.clone();invalid.change_first(old,"Ǵ")?;
    let error=invalid.preview(&engine,&source.id,&order,target).err().ok_or("Missing source glyph was silently accepted")?;
    require(matches!(error,EngineError::InvalidRequest(_)),"Missing glyph must produce a typed atomic refusal")?;
    require(engine.source_bytes(&source.id)?.as_ref()==original.as_slice(),"Refusal mutated source bytes")?;
    *stage="fit".into();let mut changed=draft.clone();changed.change_first(old,new)?;changed.explicit_lines(new,old)?;changed.fit(&engine,&source.id,0)?;
    *stage="edit".into();
    let edited=changed.preview(&engine,&source.id,&order,target)?;
    uniform_order(&engine,&edited.id,target,mode)?;
    for (page,original_page) in order.iter().enumerate().filter(|(page,_)|*page!=target) {require(engine.render_page(&edited.id,page,900)?==before[*original_page],"Edit changed another page occurrence")?;}
    if let Some(expected)=&other_forms {
        let actual=engine.list_occurrence_form_text_runs(&edited.id,target)?.runs.into_iter().map(|r|json!({"text":r.text,"font":r.font_name,"size":r.font_size,"bounds":r.bounds})).collect::<Vec<_>>();
        require(&actual==expected,"Editing one shared form changed another invocation")?;
    }
    require(engine.source_bytes(&source.id)?.as_ref()==original.as_slice(),"Editing mutated original source bytes")?;
    *stage="source-close-save".into();engine.close_document(&source.id)?;
    let saved=out.join("edited.pdf");save(&engine,&edited,&saved)?;
    require(programs(&fs::read(&saved)?)?==programs(&original)?,"Save replaced or lost an original font program")?;
    *stage="reopen".into();let reopened=engine.open_document(&saved)?;
    uniform_order(&engine,&reopened.id,target,mode)?;
    for page in 0..reopened.pages.len() {require(engine.render_page(&edited.id,page,900)?==engine.render_page(&reopened.id,page,900)?,"Save/reopen changed rendering")?;}
    let replay_mode=if mode=="nested" {"styled"}else{mode};
    *stage="re-edit-inspect".into();let saved_draft=Draft::inspect(&engine,&reopened.id,target,replay_mode,Some(new))?;
    require(saved_draft.text().contains(new),"Saved draft lost new text or paragraph memory")?;
    let output_order=(0..reopened.pages.len()).collect::<Vec<_>>();
    *stage="re-edit-no-op".into();let stable=saved_draft.preview(&engine,&reopened.id,&output_order,target)?;
    uniform_order(&engine,&stable.id,target,mode)?;
    for page in 0..reopened.pages.len() {require(engine.render_page(&stable.id,page,900)?==engine.render_page(&reopened.id,page,900)?,"Unchanged saved re-edit changed pixels")?;}
    save(&engine,&stable,&out.join("reedit-noop.pdf"))?;engine.close_document(&stable.id)?;
    *stage="restore".into();let mut restored_draft=saved_draft;restored_draft.change_first(new,old)?;
    restored_draft.fit(&engine,&reopened.id,target)?;
    let restored=restored_draft.preview(&engine,&reopened.id,&output_order,target)?;save(&engine,&restored,&out.join("restored.pdf"))?;
    uniform_order(&engine,&restored.id,target,mode)?;
    require(programs(&fs::read(out.join("restored.pdf"))?)?==programs(&original)?,"Re-edit replaced an original font program")?;
    require(fs::read(&input)?==original,"Input file bytes changed")?;
    for id in [&restored.id,&reopened.id,&edited.id] {engine.close_document(id)?;}
    *stage="passed".into();Ok(json!({"passed":true,"mode":mode,"file":row["file"],"outputPages":order.len(),"selectedOccurrence":target,"sourceClosedBeforeSave":true}))
}

fn mode(mode:&str) {
    let manifest:Value=serde_json::from_slice(&fs::read(fixtures().join("manifest.json")).unwrap()).unwrap();
    let temp=tempfile::tempdir().unwrap();let output=std::env::var_os("FOLIO_CFF_TRANSACTION_OUTPUTS").map(PathBuf::from).unwrap_or_else(||temp.path().to_owned());
    let mut failures=Vec::new();let mut count=0;
    for row in manifest["cases"].as_array().unwrap().iter().filter(|row|row["mode"]==mode) {
        count+=1;let stem=Path::new(row["file"].as_str().unwrap()).file_stem().unwrap();let out=output.join(stem);fs::create_dir(&out).unwrap();
        let mut stage=String::new();let result=case(row,&mut stage,&out);
        let report=match result {Ok(report)=>report,Err(error)=>{let failure=format!("{} [{stage}]: {error}",row["file"]);failures.push(failure.clone());json!({"passed":false,"file":row["file"],"stage":stage,"error":error.to_string()})}};
        fs::write(out.join("result.json"),serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    }
    assert_eq!(count,8,"Incomplete source-font flavor membership");
    assert!(failures.is_empty(),"{}",failures.join("\n"));
}
#[test]fn source_cff_grouped_transactions(){mode("group");}
#[test]fn source_cff_uniform_paragraph_transactions(){mode("uniform");}
#[test]fn source_cff_styled_paragraph_transactions(){mode("styled");}
#[test]fn source_cff_mixed_paragraph_transactions(){mode("mixed");}
#[test]fn source_cff_shared_form_transactions(){mode("nested");}
#[test]fn source_cff_source_name_alias_transactions(){mode("alias");}
#[test]fn source_cff_clipped_paragraph_transactions(){mode("clipped");}
#[test]fn source_cff_tagged_paragraph_transactions(){mode("tagged");}
#[test]fn source_cff_page_occurrence_transactions(){mode("occurrences");}
