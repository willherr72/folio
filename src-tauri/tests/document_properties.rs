use folio_engine::{DocumentInfo, ExportRequest, PagePlan, PdfEngine, DocumentMetadataEdit};
use lopdf::{dictionary, Document, Object, Stream};
use std::{fs, path::{Path, PathBuf}};

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()
}
fn fixture(path: &Path, tagged: bool) -> Vec<u8> {
    let mut doc=Document::with_version("1.7");let pages=doc.new_object_id();let structure=doc.new_object_id();
    let font=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let mut kids=Vec::new();let mut elements=Vec::new();let mut parents=Vec::new();
    for index in 0..2 {
        let text=format!("BT /F1 18 Tf 40 300 Td (Metadata page {index}) Tj ET");
        let content=doc.add_object(Stream::new(dictionary!{},if tagged {format!("/P <</MCID 0>> BDC {text} EMC").into_bytes()}else{text.into_bytes()}));
        let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),400.into(),400.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});
        if tagged {
            doc.get_dictionary_mut(page).unwrap().set("StructParents",index);
            let element=doc.add_object(dictionary!{"Type"=>"StructElem","S"=>"P","P"=>structure,"Pg"=>page,"K"=>0});
            elements.push(Object::Reference(element));parents.extend([Object::Integer(index),Object::Array(vec![element.into()])]);
        }
        kids.push(Object::Reference(page));
    }
    doc.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>kids,"Count"=>2}.into());
    let xml=r#"<?xpacket begin="﻿" id="W5M0MpCehiHzreSzNTczkc9d"?><x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:v="urn:folio-metadata-fixture" xmlns:a="http://www.aiim.org/pdfa/ns/id/" a:part="2"><dc:title><rdf:Alt><rdf:li xml:lang="x-default">Résumé 文書</rdf:li></rdf:Alt></dc:title><dc:rights>Retained author rights</dc:rights><v:Custom>Retain &amp; preserve</v:Custom></rdf:Description></rdf:RDF></x:xmpmeta><?xpacket end="w"?>"#;
    let metadata=doc.add_object(Stream::new(dictionary!{"Type"=>"Metadata","Subtype"=>"XML"},xml.as_bytes().to_vec()));
    let root=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages,"Metadata"=>metadata,"Lang"=>Object::string_literal("en-US")});
    if tagged {
        let parent_tree=doc.add_object(dictionary!{"Nums"=>parents});
        doc.objects.insert(structure,dictionary!{"Type"=>"StructTreeRoot","K"=>elements,"ParentTree"=>parent_tree,"ParentTreeNextKey"=>2}.into());
        doc.get_dictionary_mut(root).unwrap().set("StructTreeRoot",structure);
        doc.get_dictionary_mut(root).unwrap().set("MarkInfo",dictionary!{"Marked"=>true});
    }
    let info=doc.add_object(dictionary!{"Title"=>Object::string_literal("Metadata fixture"),"Author"=>Object::string_literal("Folio test author"),"CustomCompany"=>Object::string_literal("Retain custom Info"),"GTS_PDFXVersion"=>Object::string_literal("PDF/X-4")});
    doc.trailer.set("Root",root);doc.trailer.set("Info",info);doc.save(path).unwrap();fs::read(path).unwrap()
}
fn request(source:&DocumentInfo)->ExportRequest {
    ExportRequest{flatten:false,pages:source.pages.iter().enumerate().map(|(i,p)|PagePlan{id:format!("page-{i}"),source_id:source.id.clone(),page_index:i,width:p.width,height:p.height,rotation:0,crop:None,overlays:p.overlays.clone()}).collect()}
}

fn changes()->DocumentMetadataEdit{DocumentMetadataEdit::Update{clear_existing:false,title:Some("Updated Résumé 文書".into()),author:Some("Changed author".into()),subject:Some("".into()),keywords:Some("one & two".into())}}
#[test]
fn native_inspection_exposes_source_values_and_conflicts(){
 let engine=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("source.pdf");fixture(&input,false);let source=engine.open_document(&input).unwrap();let data=engine.inspect_document_properties(&source.id).unwrap();assert_eq!(data.title,"Metadata fixture");assert_eq!(data.author,"Folio test author");assert_eq!(data.conflicts,vec!["title"]);assert!(data.has_info&&data.has_xmp);
}
#[test]
fn metadata_progress_is_bounded_independently_of_reachable_scalar_count(){
 use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
 let engine=engine();let temp=tempfile::tempdir().unwrap();
 for count in [100,100_000]{
  let input=temp.path().join(format!("graph-{count}.pdf"));fixture(&input,false);let mut doc=Document::load(&input).unwrap();
  let values=doc.add_object(Object::Array(vec![Object::Integer(1);count]));let root=doc.trailer.get(b"Root").unwrap().as_reference().unwrap();doc.get_dictionary_mut(root).unwrap().set("PrivateGraph",values);doc.save(&input).unwrap();
  let source=engine.open_document(&input).unwrap();let reports=Arc::new(AtomicUsize::new(0));let seen=reports.clone();let progress=Arc::new(move|event:folio_engine::PdfSaveProgress|{if event.stage=="preparingFile"{seen.fetch_add(1,Ordering::SeqCst);}});
  assert!(engine.export_pdf_with_options(request(&source),None,temp.path().join(format!("saved-{count}.pdf")),None,Some(changes()),Arc::new(folio_engine::PdfSaveControl::new()),progress).unwrap());
  assert!(reports.load(Ordering::SeqCst)<=8,"{count} values emitted {} preparation reports",reports.load(Ordering::SeqCst));
 }
}
#[test]
fn native_property_updates_survive_complete_and_composed_save(){
 let engine=engine();let temp=tempfile::tempdir().unwrap();
 for tagged in [false,true]{for reorder in [false,true]{
  let input=temp.path().join(format!("source-{tagged}-{reorder}.pdf"));let original=fixture(&input,tagged);let source=engine.open_document(&input).unwrap();let mut plan=request(&source);if reorder{plan.pages.swap(0,1)}
  let output=temp.path().join(format!("saved-{tagged}-{reorder}.pdf"));engine.export_pdf_with_metadata(plan,None,changes(),&output).unwrap();
  let pdf=Document::load(&output).unwrap();let info=pdf.get_dictionary(pdf.trailer.get(b"Info").unwrap().as_reference().unwrap()).unwrap();assert_eq!(lopdf::decode_text_string(info.get(b"Title").unwrap()).unwrap(),"Updated Résumé 文書");assert_eq!(lopdf::decode_text_string(info.get(b"Author").unwrap()).unwrap(),"Changed author");assert!(info.has(b"CustomCompany"));
  let reopened=engine.open_document(&output).unwrap();for index in 0..2{assert_eq!(engine.render_page(&source.id,if reorder{1-index}else{index},400).unwrap(),engine.render_page(&reopened.id,index,400).unwrap());}if let Some(dir)=std::env::var_os("FOLIO_PROPERTIES_ARTIFACT_DIR"){let dir=PathBuf::from(dir);fs::create_dir_all(&dir).unwrap();fs::copy(&input,dir.join(format!("update-{tagged}-{reorder}-original.pdf"))).unwrap();fs::copy(&output,dir.join(format!("update-{tagged}-{reorder}-saved.pdf"))).unwrap();}let data=engine.inspect_document_properties(&reopened.id).unwrap();assert_eq!(data.title,"Updated Résumé 文書");assert!(data.conflicts.is_empty());assert_eq!(data.keywords,"one & two");assert_eq!(fs::read(&input).unwrap(),original);
 }}
}
#[test]
fn explicit_metadata_removal_saves_malformed_packets_in_both_routes(){
 let engine=engine();let temp=tempfile::tempdir().unwrap();
 for tagged in [false,true]{for reorder in [false,true]{
  let input=temp.path().join(format!("malformed-{tagged}-{reorder}.pdf"));fixture(&input,tagged);let mut doc=Document::load(&input).unwrap();let id=doc.catalog().unwrap().get(b"Metadata").unwrap().as_reference().unwrap();doc.get_object_mut(id).unwrap().as_stream_mut().unwrap().set_content(b"UNIQUE_MALFORMED_PRIVATE_METADATA".to_vec());doc.save(&input).unwrap();let original=fs::read(&input).unwrap();let source=engine.open_document(&input).unwrap();let mut plan=request(&source);if reorder{plan.pages.swap(0,1)}
  let output=temp.path().join(format!("removed-{tagged}-{reorder}.pdf"));engine.export_pdf_with_metadata(plan,None,DocumentMetadataEdit::Remove{},&output).unwrap();let saved=Document::load(&output).unwrap();assert!(!saved.catalog().unwrap().has(b"Metadata"));assert!(!saved.trailer.has(b"Info"));let bytes=fs::read(&output).unwrap();assert!(!bytes.windows(b"UNIQUE_MALFORMED_PRIVATE_METADATA".len()).any(|s|s==b"UNIQUE_MALFORMED_PRIVATE_METADATA"));assert_eq!(fs::read(&input).unwrap(),original);
 }}
}

#[path="support/protected_fixture.rs"] mod protected_fixture;
#[test]
fn properties_respect_permissions_and_are_applied_before_password_protection(){
 use std::sync::Arc;
 let engine=engine();let temp=tempfile::tempdir().unwrap();let plain=temp.path().join("plain.pdf");let original=fixture(&plain,false);
 let protected=temp.path().join("protected.pdf");fs::write(&protected,protected_fixture::encrypt(&original,lopdf::encryption::Permissions::PRINTABLE)).unwrap();
 let reader=engine.open_document_with_password(&protected,"reader").unwrap();assert_eq!(engine.inspect_document_properties(&reader.id).unwrap().title,"Metadata fixture");
 let output=temp.path().join("output.pdf");fs::write(&output,b"unchanged destination").unwrap();assert!(engine.export_pdf_with_metadata(request(&reader),None,DocumentMetadataEdit::Remove{},&output).is_err());assert_eq!(fs::read(&output).unwrap(),b"unchanged destination");
 let owner=engine.open_document_with_password(&protected,"owner").unwrap();engine.export_pdf_with_metadata(request(&owner),None,changes(),&output).unwrap();assert!(engine.open_document(&output).is_err());let saved=engine.open_document_with_password(&output,"reader").unwrap();assert_eq!(engine.inspect_document_properties(&saved.id).unwrap().title,"Updated Résumé 文書");assert!(!saved.protection.unwrap().permissions.edit_content);
 let security:folio_engine::ProtectionChange=serde_json::from_value(serde_json::json!({"mode":"passwords","userPassword":"new-reader","ownerPassword":"new-owner","permissions":{"print":true,"printHighQuality":true,"copyContent":true,"editContent":false,"annotate":false,"fillForms":true,"assemble":false}})).unwrap();
 let output=temp.path().join("new-passwords.pdf");assert!(engine.export_pdf_with_options(request(&owner),None,&output,Some(security),Some(changes()),Arc::new(folio_engine::PdfSaveControl::new()),Arc::new(|_|{})).unwrap());let saved=engine.open_document_with_password(&output,"new-reader").unwrap();assert_eq!(engine.inspect_document_properties(&saved.id).unwrap().author,"Changed author");
}
#[test]
fn metadata_drafts_reach_extracted_and_split_pdfs(){
 use std::sync::Arc;
 let engine=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("source.pdf");fixture(&input,true);let source=engine.open_document(&input).unwrap();let mut extract=request(&source);extract.pages.remove(0);let output=temp.path().join("extracted.pdf");engine.export_pdf_with_metadata(extract,None,changes(),&output).unwrap();let saved=engine.open_document(&output).unwrap();assert_eq!(saved.pages.len(),1);assert_eq!(engine.inspect_document_properties(&saved.id).unwrap().title,"Updated Résumé 文書");
 let split=folio_engine::SplitPdfRequest{pages:request(&source).pages,parts:vec![folio_engine::SplitPdfPart{start:0,end:1},folio_engine::SplitPdfPart{start:1,end:2}],bookmarks:None,metadata:Some(changes())};
 let result=engine.split_pdf(split,temp.path(),Arc::new(folio_engine::PageImageControl::new()),Arc::new(|_|{})).unwrap().unwrap();for index in 1..=2{let saved=engine.open_document(result.folder.join(format!("part-{index:04}.pdf"))).unwrap();assert_eq!(engine.inspect_document_properties(&saved.id).unwrap().title,"Updated Résumé 文書");}
}
#[test]
fn cancellation_at_metadata_preparation_preserves_existing_destination(){
 use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
 let engine=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("source.pdf");let original=fixture(&input,false);let source=engine.open_document(&input).unwrap();let output=temp.path().join("output.pdf");fs::write(&output,b"keep destination").unwrap();
 let control=Arc::new(folio_engine::PdfSaveControl::new());let cancel=control.clone();let calls=Arc::new(AtomicUsize::new(0));let observed=calls.clone();
 let progress=Arc::new(move|event:folio_engine::PdfSaveProgress|{if event.stage=="preparingFile"&&observed.fetch_add(1,Ordering::SeqCst)==0{assert!(cancel.cancel());}});
 assert!(!engine.export_pdf_with_options(request(&source),None,&output,None,Some(changes()),control,progress).unwrap());assert!(calls.load(Ordering::SeqCst)>=1);assert_eq!(fs::read(&output).unwrap(),b"keep destination");assert_eq!(fs::read(&input).unwrap(),original);
}

#[test]
fn flattened_copy_keeps_metadata_edits_and_visible_overlay_content(){
 let engine=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("source.pdf");let original=fixture(&input,false);let source=engine.open_document(&input).unwrap();let mut plan=request(&source);plan.flatten=true;
 plan.pages[0].overlays.push(serde_json::from_value(serde_json::json!({"id":"new-text","type":"text","x":50,"y":50,"text":"Added overlay","fontSize":12,"fontName":"Helvetica","color":"#000000"})).unwrap());
 let reference=temp.path().join("flattened-reference.pdf");engine.export_pdf(plan.clone(),&reference).unwrap();let reference_source=engine.open_document(&reference).unwrap();
 let output=temp.path().join("flattened.pdf");engine.export_pdf_with_metadata(plan,None,changes(),&output).unwrap();let saved=engine.open_document(&output).unwrap();assert_eq!(engine.inspect_document_properties(&saved.id).unwrap().title,"Updated Résumé 文書");
 assert!(engine.extract_text(&saved.id,0).unwrap().contains("Added overlay"));
 for index in 0..2{assert_eq!(engine.render_page(&reference_source.id,index,400).unwrap(),engine.render_page(&saved.id,index,400).unwrap());}
 if let Some(dir)=std::env::var_os("FOLIO_PROPERTIES_ARTIFACT_DIR"){let dir=PathBuf::from(dir);fs::create_dir_all(&dir).unwrap();fs::copy(&output,dir.join("flattened-saved.pdf")).unwrap();fs::copy(&reference,dir.join("flattened-reference.pdf")).unwrap();}
 assert_eq!(fs::read(&input).unwrap(),original);
}
