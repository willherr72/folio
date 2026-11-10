use folio_engine::{DocumentInfo, ExportRequest, PagePlan, PdfEngine};
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
fn verify_metadata(path:&Path) {
    let doc=Document::load(path).unwrap();let root=doc.catalog().unwrap();
    let metadata=root.get(b"Metadata").expect("composed Save must retain document XMP").as_reference().unwrap();
    let stream=doc.get_object(metadata).unwrap().as_stream().unwrap();
    let xml=String::from_utf8(stream.decompressed_content_strict_with_limit(4*1024*1024).unwrap()).unwrap();
    let tree=roxmltree::Document::parse(&xml).unwrap();
    assert!(xml.contains("Résumé 文書")&&xml.contains("Retained author rights")&&xml.contains("Retain &amp; preserve"));
    assert!(tree.descendants().filter(|n|n.is_element()).all(|n|n.attributes().all(|a|a.namespace()!=Some("http://www.aiim.org/pdfa/ns/id/"))));
    let info=doc.get_dictionary(doc.trailer.get(b"Info").unwrap().as_reference().unwrap()).unwrap();
    assert_eq!(info.get(b"Author").unwrap().as_str().unwrap(),b"Folio test author");
    assert_eq!(info.get(b"CustomCompany").unwrap().as_str().unwrap(),b"Retain custom Info");
    assert!(!info.has(b"GTS_PDFXVersion"));
}
fn reordered_save(tagged:bool) {
    let engine=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("original.pdf");let original=fixture(&input,tagged);
    let source=engine.open_document(&input).unwrap();let mut plan=request(&source);plan.pages.swap(0,1);
    let output=temp.path().join("saved.pdf");engine.export_pdf(plan,&output).unwrap();verify_metadata(&output);
    assert_eq!(fs::read(&input).unwrap(),original);
    let reopened=engine.open_document(&output).unwrap();assert_eq!(reopened.pages.len(),2);
    if let Some(dir)=std::env::var_os("FOLIO_METADATA_ARTIFACT_DIR") {
        let dir=PathBuf::from(dir);fs::create_dir_all(&dir).unwrap();let kind=if tagged{"structured"}else{"plain"};
        fs::copy(&input,dir.join(format!("{kind}-original.pdf"))).unwrap();fs::copy(&output,dir.join(format!("{kind}-saved.pdf"))).unwrap();
    }
}
#[test]
fn plain_reordered_save_preserves_ordinary_and_custom_metadata() {reordered_save(false);}
#[test]
fn structured_reordered_save_preserves_ordinary_and_custom_metadata() {reordered_save(true);}

#[test]
fn first_output_source_owns_document_metadata_in_both_composition_routes() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    for tagged in [false,true] {
        let mut sources=Vec::new();
        for name in ["First owner","Second owner"] {
            let path=temp.path().join(format!("{name}-{tagged}.pdf"));fixture(&path,tagged);
            let mut doc=Document::load(&path).unwrap();let info=doc.trailer.get(b"Info").unwrap().as_reference().unwrap();
            doc.get_dictionary_mut(info).unwrap().set("Author",Object::string_literal(name));
            let metadata=doc.catalog().unwrap().get(b"Metadata").unwrap().as_reference().unwrap();
            let stream=doc.get_object_mut(metadata).unwrap().as_stream_mut().unwrap();
            let xml=String::from_utf8(stream.content.clone()).unwrap().replace("Résumé 文書",name);stream.set_content(xml.into_bytes());
            doc.save(&path).unwrap();sources.push(engine.open_document(&path).unwrap());
        }
        for leading in 0..2 {
            let mut first=request(&sources[leading]).pages.remove(0);first.id="leading".into();
            let mut second=request(&sources[1-leading]).pages.remove(1);second.id="following".into();
            let output=temp.path().join(format!("owners-{tagged}-{leading}.pdf"));
            engine.export_pdf(ExportRequest{flatten:false,pages:vec![first,second]},&output).unwrap();
            let doc=Document::load(output).unwrap();let info=doc.get_dictionary(doc.trailer.get(b"Info").unwrap().as_reference().unwrap()).unwrap();
            let expected=["First owner","Second owner"][leading];assert_eq!(info.get(b"Author").unwrap().as_str().unwrap(),expected.as_bytes());
            let metadata=doc.get_object(doc.catalog().unwrap().get(b"Metadata").unwrap().as_reference().unwrap()).unwrap().as_stream().unwrap();
            let xml=String::from_utf8(metadata.decompressed_content_strict_with_limit(4*1024*1024).unwrap()).unwrap();
            assert!(xml.contains(expected));assert!(!xml.contains(["Second owner","First owner"][leading]));
        }
    }
}

#[test]
fn malformed_metadata_blocks_save_atomically_but_not_page_preview() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    for tagged in [false,true] {
        let input=temp.path().join(format!("malformed-{tagged}.pdf"));fixture(&input,tagged);
        let mut doc=Document::load(&input).unwrap();let metadata=doc.catalog().unwrap().get(b"Metadata").unwrap().as_reference().unwrap();
        doc.get_object_mut(metadata).unwrap().as_stream_mut().unwrap().set_content(b"<broken>".to_vec());doc.save(&input).unwrap();
        let before=fs::read(&input).unwrap();let source=engine.open_document(&input).unwrap();let mut plan=request(&source);plan.pages.reverse();
        assert!(!engine.render_page_plan(plan.pages[0].clone(),400).unwrap().is_empty());
        let output=temp.path().join(format!("protected-destination-{tagged}.pdf"));fs::write(&output,b"preserve destination").unwrap();
        let error=engine.export_pdf(plan,&output).unwrap_err().to_string();assert!(error.contains("metadata"),"{error}");
        assert_eq!(fs::read(&output).unwrap(),b"preserve destination");assert_eq!(fs::read(&input).unwrap(),before);
    }
}

#[test]
fn repeated_composition_preserves_metadata_and_cancel_preserves_destination() {
    use std::sync::{Arc,atomic::{AtomicBool,Ordering}};
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    for tagged in [false,true] {
        let input=temp.path().join(format!("repeated-{tagged}.pdf"));let original=fixture(&input,tagged);let mut source=engine.open_document(&input).unwrap();
        let mut counts=Vec::new();
        for round in 0..3 {
            let mut plan=request(&source);plan.pages.reverse();let output=temp.path().join(format!("repeated-{tagged}-{round}.pdf"));
            engine.export_pdf(plan,&output).unwrap();verify_metadata(&output);counts.push(Document::load(&output).unwrap().objects.len());source=engine.open_document(&output).unwrap();
        }
        assert_eq!(counts[1],counts[2],"saved metadata graph grew on repeated composition: {counts:?}");
        let output=temp.path().join(format!("cancelled-{tagged}.pdf"));fs::write(&output,b"keep destination").unwrap();
        let mut plan=request(&source);plan.pages.reverse();let control=Arc::new(folio_engine::PdfSaveControl::new());let cancel=control.clone();
        let observed=Arc::new(AtomicBool::new(false));let seen=observed.clone();
        assert!(!engine.export_pdf_controlled(plan,None,&output,control,Arc::new(move |event|{if event.stage=="preserving"{seen.store(true,Ordering::Relaxed);cancel.cancel();}})).unwrap());
        assert!(observed.load(Ordering::Relaxed));assert_eq!(fs::read(output).unwrap(),b"keep destination");assert_eq!(fs::read(input).unwrap(),original);
    }
}

#[test]
fn wide_unicode_metadata_survives_both_save_routes_as_valid_utf8() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    for tagged in [false,true] {for wide in [16,32] {for little in [false,true] {
        let input=temp.path().join(format!("wide-{tagged}-{wide}-{little}.pdf"));fixture(&input,tagged);
        let mut doc=Document::load(&input).unwrap();let id=doc.catalog().unwrap().get(b"Metadata").unwrap().as_reference().unwrap();
        let stream=doc.get_object_mut(id).unwrap().as_stream_mut().unwrap();
        let text=format!("<?xml version='1.0' encoding='UTF-{wide}'?>{}",String::from_utf8(stream.content.clone()).unwrap());
        let mut bytes=Vec::new();
        if wide==16 {
            bytes.extend(if little{[0xff,0xfe]}else{[0xfe,0xff]});
            for value in text.encode_utf16(){bytes.extend(if little{value.to_le_bytes()}else{value.to_be_bytes()});}
        } else {
            bytes.extend(if little{[0xff,0xfe,0,0]}else{[0,0,0xfe,0xff]});
            for value in text.chars(){bytes.extend(if little{(value as u32).to_le_bytes()}else{(value as u32).to_be_bytes()});}
        }
        stream.set_content(bytes);doc.save(&input).unwrap();let before=fs::read(&input).unwrap();
        let source=engine.open_document(&input).unwrap();let mut plan=request(&source);plan.pages.reverse();
        let output=temp.path().join(format!("saved-{tagged}-{wide}-{little}.pdf"));engine.export_pdf(plan,&output).unwrap();
        verify_metadata(&output);assert_eq!(fs::read(input).unwrap(),before);
    }}}
}

#[test]
fn page_metadata_aliases_keep_original_bytes_when_document_claims_are_filtered() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    for tagged in [false,true] {
        let input=temp.path().join(format!("alias-{tagged}.pdf"));fixture(&input,tagged);
        let mut doc=Document::load(&input).unwrap();let metadata=doc.catalog().unwrap().get(b"Metadata").unwrap().as_reference().unwrap();
        let original_xml=doc.get_object(metadata).unwrap().as_stream().unwrap().decompressed_content_strict_with_limit(4*1024*1024).unwrap();
        let page=doc.get_pages()[&1];doc.get_dictionary_mut(page).unwrap().set("Metadata",metadata);doc.save(&input).unwrap();
        let source=engine.open_document(&input).unwrap();let mut plan=request(&source);plan.pages.reverse();
        let output=temp.path().join(format!("alias-saved-{tagged}.pdf"));engine.export_pdf(plan,&output).unwrap();verify_metadata(&output);
        let saved=Document::load(output).unwrap();let catalog_metadata=saved.catalog().unwrap().get(b"Metadata").unwrap().as_reference().unwrap();
        let page_metadata=saved.get_dictionary(saved.get_pages()[&2]).unwrap().get(b"Metadata").unwrap().as_reference().unwrap();
        assert_ne!(page_metadata,catalog_metadata);
        assert_eq!(saved.get_object(page_metadata).unwrap().as_stream().unwrap().decompressed_content_strict_with_limit(4*1024*1024).unwrap(),original_xml);
    }
}
