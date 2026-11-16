//! Explicit substitution must supply a glyph absent from the original CFF code map.
use folio_engine::{AnnotationRect,ExportRequest,PagePlan,ParagraphFormatting,PdfEngine};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::{fs,path::{Path,PathBuf}};

fn save(engine:&PdfEngine,doc:&folio_engine::DocumentInfo,path:&Path) {
    engine.export_pdf(ExportRequest{flatten:false,pages:doc.pages.iter().enumerate().map(|(page_index,p)|PagePlan{crop:None,id:format!("p{page_index}"),source_id:doc.id.clone(),page_index,width:p.width,height:p.height,rotation:0,overlays:vec![]}).collect()},path).unwrap();
}
#[test]
fn explicit_substitute_supplies_missing_cff_glyph_and_saved_pdf_owns_it() {
    let root=PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixtures=root.join("../tests/fixtures/source-cff/transactions-v2");
    let manifest:Value=serde_json::from_slice(&fs::read(fixtures.join("manifest.json")).unwrap()).unwrap();
    let font_bytes=fs::read(root.join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap();
    let font_sha:String=Sha256::digest(&font_bytes).iter().map(|b|format!("{b:02x}")).collect();
    let temp=tempfile::tempdir().unwrap();
    let output=std::env::var_os("FOLIO_CFF_SUBSTITUTION_OUTPUTS").map(PathBuf::from).unwrap_or_else(||temp.path().to_owned());
    let engine=PdfEngine::start(root.join("resources/pdfium/pdfium.dll")).unwrap();let mut count=0;
    for row in manifest["cases"].as_array().unwrap().iter().filter(|row|row["mode"]=="uniform") {
        count+=1;let filename=row["file"].as_str().unwrap();let folder=output.join(Path::new(filename).file_stem().unwrap());fs::create_dir_all(&folder).unwrap();
        let path=fixtures.join(filename);let original=fs::read(&path).unwrap();fs::write(folder.join("original.pdf"),&original).unwrap();
        let source=engine.open_document(&path).unwrap();let before=engine.list_occurrence_text_runs(&source.id,0).unwrap().runs;
        let indices:Vec<_>=before.iter().filter(|r|r.text!="Unchanged neighbor").map(|r|r.object_index).collect();
        let selected=engine.inspect_occurrence_text_paragraph(&source.id,0,&indices).unwrap();
        let area=AnnotationRect{width:selected.bounds.width.max(160.),height:selected.bounds.height.max(80.),..selected.bounds.clone()};
        assert!(engine.preview_text_paragraph_occurrence(&source.id,&[0],0,&indices,&selected.text,"Ǵ",&area,&ParagraphFormatting::default()).is_err());
        let font=engine.fonts().register(font_bytes.clone()).unwrap();assert_eq!(font.id,font_sha);
        let formatting=ParagraphFormatting{font_id:Some(font.id.clone()),..Default::default()};
        let edited=engine.preview_text_paragraph_occurrence(&source.id,&[0],0,&indices,&selected.text,"Ǵ",&area,&formatting).unwrap().document;
        assert_eq!(engine.source_bytes(&source.id).unwrap().as_ref(),original.as_slice());
        engine.close_document(&source.id).unwrap();engine.fonts().remove(&font.id).unwrap();
        let saved=folder.join("substituted.pdf");save(&engine,&edited,&saved);
        let reopened=engine.open_document(&saved).unwrap();
        let runs=engine.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;
        assert_eq!(runs.len(),2);assert_eq!(runs[0].text,"Ǵ");assert!(runs[0].supported&&runs[0].is_embedded);
        assert_eq!(runs[0].font_name,format!("Folio{}",font.id));
        assert_eq!(runs[1].text,"Unchanged neighbor");assert_eq!(runs[1].bounds,before.last().unwrap().bounds);
        let remembered=engine.suggest_occurrence_text_paragraph(&reopened.id,0,runs[0].object_index,"Ǵ").unwrap();assert_eq!(remembered.text,"Ǵ");
        let again=engine.preview_text_paragraph_occurrence(&reopened.id,&[0],0,&remembered.object_indices,&remembered.text,"ǵ",&area,&ParagraphFormatting::default()).unwrap().document;
        let reedited=folder.join("reedited.pdf");save(&engine,&again,&reedited);
        assert_eq!(engine.list_occurrence_text_runs(&again.id,0).unwrap().runs[0].text,"ǵ");
        assert_eq!(fs::read(&path).unwrap(),original);
        fs::write(folder.join("result.json"),serde_json::to_vec_pretty(&json!({"passed":true,"file":filename,"substituteFontSha256":font_sha,"fontRemovedBeforeSave":true,"sourceClosedBeforeSave":true,"selectedSize":selected.font_size,"requestedArea":area})).unwrap()).unwrap();
        for id in [&again.id,&reopened.id,&edited.id]{engine.close_document(id).unwrap();}
    }
    assert_eq!(count,8);
}
