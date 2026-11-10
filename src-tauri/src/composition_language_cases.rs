use super::*;
use std::sync::atomic::{AtomicUsize,Ordering};

fn form_source(page_program:&[u8],form_program:&[u8],nested:bool)->Vec<u8> {
    let mut doc=Document::load_mem(&fixture(false)).unwrap();doc.catalog_mut().unwrap().remove(b"Lang");
    let font=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let inner=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),100.into(),100.into()]},form_program.to_vec()));
    let outer=if nested {doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),100.into(),100.into()]},b"/Inner Do".to_vec()))}else{inner};
    let contents=doc.add_object(Stream::new(dictionary!{},page_program.to_vec()));
    let page=doc.get_pages()[&1];
    doc.get_dictionary_mut(page).unwrap().set("Resources",dictionary!{"Font"=>dictionary!{"F"=>font},"XObject"=>dictionary!{"Form"=>outer,"Inner"=>inner}});
    doc.get_dictionary_mut(page).unwrap().set("Contents",contents);
    let mut bytes=vec![];doc.save_to(&mut bytes).unwrap();bytes
}

#[test]
fn composition_language_reused_nested_forms_keep_original_operators_and_caller_overrides() {
    for nested in [false,true] {
        let bytes=form_source(b"/Form Do /Span << /Lang (es) >> BDC /Form Do EMC",b"BT /F 12 Tf (Inherited language) Tj ET",nested);
        let doc=graph(&[fixture(true),bytes]).assemble(&[0,1],&mut check).unwrap();
        let page=doc.get_pages()[&3];let body=doc.get_page_content_with_limit(page,1_048_576).unwrap();
        assert!(body.starts_with(b"/Span << /Lang <> >> BDC\n"));
        assert!(body.windows(b"/Span << /Lang (es) >> BDC /Form Do EMC".len()).any(|part|part==b"/Span << /Lang (es) >> BDC /Form Do EMC"));
        let resources=doc.get_dictionary(page).unwrap().get(b"Resources").unwrap().as_dict().unwrap();
        let inner=resources.get(b"XObject").unwrap().as_dict().unwrap().get(b"Inner").unwrap().as_reference().unwrap();
        assert_eq!(doc.get_object(inner).unwrap().as_stream().unwrap().content,b"BT /F 12 Tf (Inherited language) Tj ET");
    }
}

#[test]
fn composition_language_form_clipping_uses_each_invocation_rendering_mode() {
    for nested in [false,true] {
        for program in [b"7 Tr /Form Do".as_slice(),b"/Form Do q 7 Tr /Form Do Q"] {
            let bytes=form_source(program,b"BT /F 12 Tf (clipping) Tj ET",nested);
            assert!(graph(&[fixture(true),bytes]).assemble(&[0,1],&mut check).is_err());
        }
        let bytes=form_source(b"7 Tr /Form Do",b"0 Tr BT /F 12 Tf (visible) Tj ET",nested);
        assert!(graph(&[fixture(true),bytes]).assemble(&[0,1],&mut check).is_ok());
    }
}

#[test]
fn composition_language_controlled_parser_cancellation_reaches_native_predicate() {
    let bytes=form_source(b"/Form Do",b"BT /F 12 Tf (word) Tj ET",true);
    let calls=Arc::new(AtomicUsize::new(0));let observed=calls.clone();
    let error=graph(&[fixture(true),bytes]).assemble_controlled(&[0,1],Arc::new(move||observed.fetch_add(1,Ordering::Relaxed)>=12),&mut check).unwrap_err();
    assert!(error.to_string().contains("cancelled"),"{error}");assert!(calls.load(Ordering::Relaxed)>12);
}

#[test]
fn composition_language_refuses_bad_filters_without_using_raw_bytes_as_content() {
    let mut doc=Document::load_mem(&fixture(false)).unwrap();doc.catalog_mut().unwrap().remove(b"Lang");
    let contents=doc.add_object(Stream::new(dictionary!{"Filter"=>"FlateDecode"},b"BT (raw bytes are not decoded content) Tj ET".to_vec()));
    let page=doc.get_pages()[&1];doc.get_dictionary_mut(page).unwrap().set("Contents",contents);
    let mut bytes=vec![];doc.save_to(&mut bytes).unwrap();
    assert!(graph(&[fixture(true),bytes]).assemble(&[0,1],&mut check).is_err());
}

#[test]
fn composition_language_equivalent_defaults_do_not_restrict_unchanged_metadata() {
    for language in [Object::string_literal("EN-us"),Object::String(b"\xfe\xff\0E\0N\0-\0u\0s".to_vec(),lopdf::StringFormat::Hexadecimal)] {
        let mut doc=Document::load_mem(&fixture(false)).unwrap();doc.catalog_mut().unwrap().set("Lang",language);
        let page=doc.get_pages()[&1];doc.get_dictionary_mut(page).unwrap().set("UnknownHumanText",Object::string_literal("Same language"));
        let mut bytes=vec![];doc.save_to(&mut bytes).unwrap();
        assert!(graph(&[fixture(true),bytes]).assemble(&[0,1],&mut check).is_ok());
    }
}

#[test]
fn composition_language_version_promotion_does_not_activate_old_annotation_language() {
    fn source(version:&str,override_version:bool,annotation_lang:bool)->Vec<u8> {
        let mut doc=Document::load_mem(&fixture(true)).unwrap();doc.version=version.into();
        if override_version {doc.catalog_mut().unwrap().set("Version",Object::Name(b"2.0".to_vec()));}
        if annotation_lang {
            let page=doc.get_pages()[&1];
            let note=doc.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap()[0].as_reference().unwrap();
            let language=doc.add_object(Object::string_literal("fr"));
            doc.get_dictionary_mut(note).unwrap().set("Lang",language);
            let alias=doc.add_object(Object::Reference(note));
            doc.get_dictionary_mut(page).unwrap().get_mut(b"Annots").unwrap().as_array_mut().unwrap()[0]=alias.into();
        }
        let mut bytes=vec![];doc.save_to(&mut bytes).unwrap();bytes
    }
    let old=source("1.7",false,true);let new=source("2.0",false,false);
    for inputs in [vec![old.clone(),new.clone()],vec![new.clone(),old.clone()]] {
        assert!(graph(&inputs).assemble(&[0,1],&mut check).unwrap_err().to_string().contains("annotation language"),"version promotion must not activate a source extension");
    }
    // Without a version change the extension keeps its original meaning.
    assert!(graph(&[old,source("1.7",false,false)]).assemble(&[0,1],&mut check).is_ok());
    // A catalog version override already gives the source PDF2 semantics.
    assert!(graph(&[source("1.7",true,true),new]).assemble(&[0,1],&mut check).is_ok());
    let mut namespace_source=Document::load_mem(&source("1.7",false,false)).unwrap();
    let root=namespace_source.catalog().unwrap().get(b"StructTreeRoot").unwrap().as_reference().unwrap();
    let ns=namespace_source.add_object(dictionary!{"Type"=>"Namespace","NS"=>Object::string_literal("https://example.test/namespace")});
    namespace_source.get_dictionary_mut(root).unwrap().set("Namespaces",vec![Object::Reference(ns)]);
    let mut bytes=vec![];namespace_source.save_to(&mut bytes).unwrap();
    assert!(graph(&[source("1.7",false,true),bytes]).assemble(&[0,1],&mut check).unwrap_err().to_string().contains("annotation language"),"namespace reconciliation also promotes the output version");
}
