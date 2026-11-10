//! Local original producer corpus audit. No product code or source PDF mutation.
use folio_engine::{PdfEngine,ParagraphAlignment,StyledParagraphLayoutRequest};
use std::{path::PathBuf,io::Write};
fn save(path:&std::path::Path,bytes:&[u8])->std::io::Result<()> {
    std::fs::OpenOptions::new().create_new(true).write(true).open(path)?.write_all(bytes)
}
fn main()->Result<(),Box<dyn std::error::Error>>{
    let root=PathBuf::from(std::env::args().nth(1).ok_or("input directory")?);
    let output=PathBuf::from(std::env::args().nth(2).ok_or("new output directory")?);std::fs::create_dir(&output)?;
    let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))?;
    let mut files=std::fs::read_dir(&root)?.collect::<Result<Vec<_>,_>>()?;files.sort_by_key(|p|p.path());
    let mut report=vec![];
    for entry in files {
        let path=entry.path();if path.extension().and_then(|p|p.to_str())!=Some("pdf"){continue;}
        let original=std::fs::read(&path)?;let source=engine.open_document(&path)?;
        let runs=engine.list_occurrence_text_runs(&source.id,0)?;
        let mut cases=vec![];let occurrences=(0..source.pages.len()).collect::<Vec<_>>();
        for prefix in ["Simple","Résumé"] {
            let Some(run)=runs.runs.iter().find(|r|r.text.starts_with(prefix)) else {cases.push(serde_json::json!({"prefix":prefix,"error":"Target not found"}));continue;};
            let expected=run.source_text.as_deref().unwrap_or(&run.text);
            let replacement=expected.replacen(' '," this ",1);
            let single=(||->Result<_,Box<dyn std::error::Error>>{
                let result=engine.replace_text_occurrence(&source.id,&occurrences,0,run.object_index,&run.text,&replacement,None)?;
                let bytes=engine.source_bytes(&result.document.id)?;engine.close_document(&result.document.id)?;
                let target=output.join(format!("{}-{prefix}-single.pdf",path.file_stem().unwrap().to_string_lossy()));save(&target,&bytes)?;
                Ok(serde_json::json!({"passed":true,"output":target,"replacement":replacement}))
            })().unwrap_or_else(|e|serde_json::json!({"passed":false,"error":e.to_string()}));
            let paragraph=(||->Result<_,Box<dyn std::error::Error>>{
                let selection=engine.suggest_occurrence_styled_paragraph(&source.id,0,run.object_index,&run.text)?;
                let mut styled=selection.runs.clone();styled.first_mut().ok_or("empty suggestion")?.text.push_str(" this");
                let mut area=selection.bounds.clone();area.width=450.;area.height=45.;
                let request=StyledParagraphLayoutRequest{baseline_anchor:selection.baseline_anchor,object_indices:selection.object_indices,expected_text:selection.text,runs:styled,style_changes:vec![],bounds:area,alignment:selection.alignment.unwrap_or(ParagraphAlignment::Left),line_spacing:selection.line_spacing.unwrap_or(1.2)};
                let result=engine.preview_styled_paragraph_occurrence(&source.id,&occurrences,0,&request)?;
                let bytes=engine.source_bytes(&result.document.id)?;engine.close_document(&result.document.id)?;
                let target=output.join(format!("{}-{prefix}-paragraph.pdf",path.file_stem().unwrap().to_string_lossy()));save(&target,&bytes)?;
                Ok(serde_json::json!({"passed":true,"output":target,"request":request}))
            })().unwrap_or_else(|e|serde_json::json!({"passed":false,"error":e.to_string()}));
            cases.push(serde_json::json!({"prefix":prefix,"selected":run,"single":single,"paragraph":paragraph}));
        }
        assert_eq!(std::fs::read(&path)?,original);
        report.push(serde_json::json!({"file":path,"runs":runs,"cases":cases}));engine.close_document(&source.id)?;
    }
    save(&output.join("results.json"),&serde_json::to_vec_pretty(&report)?)?;
    println!("Audited {} producer PDFs; see{}",report.len(),output.join("results.json").display());Ok(())
}
