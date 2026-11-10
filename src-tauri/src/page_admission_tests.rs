use super::*;
use super::super::{WorkerRuntime, page_geometry};
use lopdf::{dictionary,Document,Object,Stream};
use std::{path::PathBuf,sync::Arc};

fn source(pages:usize)->Document {
    let mut pdf=Document::with_version("1.7");let root=pdf.new_object_id();
    let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let contents=pdf.add_object(Stream::new(dictionary!{},b"BT /F1 12 Tf 20 40 Td (Frame) Tj ET".to_vec()));
    let mut kids=Vec::new();
    for i in 0..pages {
        let mut page=dictionary!{"Type"=>"Page","Parent"=>root,"MediaBox"=>vec![(-30).into(),(-20).into(),300.into(),400.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>contents,"UserUnit"=>[0.5f32,1.,1.7,2.][i%4]};
        if i%3!=0 {page.set("Rotate",((i%4)*90) as i64);}
        match i%4 {
            1=>page.set("CropBox",vec![0.into(),0.into(),250.into(),300.into()]),
            2=>page.set("CropBox",vec![(-60).into(),(-40).into(),450.into(),500.into()]),
            3=>page.set("CropBox",vec![100.into(),100.into(),90.into(),90.into()]),
            _=>{},
        }
        kids.push(Object::Reference(pdf.add_object(page)));
    }
    pdf.objects.insert(root,dictionary!{"Type"=>"Pages","Kids"=>kids,"Count"=>pages as i64,"Rotate"=>90,"CropBox"=>vec![(-10).into(),0.into(),280.into(),350.into()]}.into());
    let catalog=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>root});pdf.trailer.set("Root",catalog);pdf
}

#[test]
fn eligible_writer_output_is_bound_to_exact_bytes_and_page_count(){
    let publication=Publication::save(&mut source(32),true).unwrap();let(bytes,proof)=publication.into_parts();
    assert!(proof.is_some(),"eligible writer output needs a certificate");let proof=proof.unwrap();
    assert!(proof.matches(&bytes,32));assert!(!proof.matches(&bytes,31));assert!(!proof.matches(&bytes,33));
    let mut changed=bytes.clone();changed[0]^=1;assert!(!proof.matches(&changed,32));
    let mut longer=bytes.clone();longer.push(b' ');assert!(!proof.matches(&longer,32));
}

#[test]
fn another_publication_cannot_use_the_previous_certificate(){
    let mut first=source(32);let(bytes,proof)=Publication::save(&mut first,true).unwrap().into_parts();
    assert!(proof.is_some());let proof=proof.unwrap();
    let mut other=source(32);other.trailer.set("Different",true);
    let alternate=Publication::save(&mut other,true).unwrap().into_bytes();
    assert!(proof.matches(&bytes,32));assert!(!proof.matches(&alternate,32));
}

#[test]
fn generic_publication_and_small_documents_stay_uncertified(){
    for (pages,allow) in [(2,true),(31,true),(32,false)] {
        assert!(Publication::save(&mut source(pages),allow).unwrap().into_parts().1.is_none());
    }
}

#[test]
fn annots_in_any_dictionary_location_refuses_the_fast_path(){
    for location in 0..5 {
        let mut pdf=source(32);let page=*pdf.get_pages().values().next().unwrap();
        match location {
            0=>pdf.get_dictionary_mut(page).unwrap().set("Annots",Object::Null),
            1=>pdf.get_dictionary_mut(page).unwrap().set("Annots",Vec::<Object>::new()),
            2=>pdf.trailer.set("Annots",Object::Null),
            3=>{pdf.add_object(dictionary!{"Nested"=>vec![Object::Dictionary(dictionary!{"Annots"=>Object::Null})]});},
            4=>{pdf.add_object(Stream::new(dictionary!{"Annots"=>Object::Null},vec![]));},
            _=>unreachable!(),
        }
        assert!(Publication::save(&mut pdf,true).unwrap().into_parts().1.is_none(),"location {location}");
    }
}

#[test]
fn indirect_annotation_arrays_are_uncertified(){
    let mut pdf=source(32);let page=*pdf.get_pages().values().next().unwrap();
    let annotation=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Link","Rect"=>vec![0.into(),0.into(),10.into(),10.into()]});
    let annotations=pdf.add_object(vec![Object::Reference(annotation)]);
    pdf.get_dictionary_mut(page).unwrap().set("Annots",annotations);
    assert!(Publication::save(&mut pdf,true).unwrap().into_parts().1.is_none());
}

#[test]
fn annotation_names_in_stream_text_do_not_claim_dictionary_entries(){
    let mut pdf=source(32);pdf.add_object(Stream::new(dictionary!{},b"BT (/Annots is text) Tj ET".to_vec()));
    assert!(Publication::save(&mut pdf,true).unwrap().into_parts().1.is_some());
}

#[test]
fn encoded_size_and_direct_walk_limits_fall_back_without_rejecting_output(){
    for case in 0..3 {
        let mut pdf=source(32);
        match case {
            0=>{pdf.add_object(Object::String(vec![b'X';8*1024*1024+1024],lopdf::StringFormat::Literal));},
            1=>{pdf.add_object(Object::Array(vec![Object::Null;100_001]));},
            2=>{let mut value=Object::Null;for _ in 0..66{value=Object::Array(vec![value]);}pdf.add_object(value);},
            _=>unreachable!(),
        }
        let(bytes,proof)=Publication::save(&mut pdf,true).unwrap().into_parts();assert!(!bytes.is_empty());assert!(proof.is_none(),"case {case}");
    }
}

#[test]
fn decrypted_encryption_state_does_not_authorize_the_fast_path(){
    use lopdf::encryption::{EncryptionState,EncryptionVersion,Permissions,crypt_filters::{Aes128CryptFilter,CryptFilter}};
    use std::collections::BTreeMap;
    let mut pdf=source(32);let id=Object::string_literal(vec![7u8;16]);pdf.trailer.set("ID",vec![id.clone(),id]);
    let mut filters:BTreeMap<Vec<u8>,Arc<dyn CryptFilter>>=BTreeMap::new();filters.insert(b"StdCF".to_vec(),Arc::new(Aes128CryptFilter));
    let state=EncryptionState::try_from(EncryptionVersion::V4{document:&pdf,encrypt_metadata:true,crypt_filters:filters,stream_filter:b"StdCF".to_vec(),string_filter:b"StdCF".to_vec(),owner_password:"owner",user_password:"",permissions:Permissions::all()}).unwrap();
    pdf.encrypt(&state).unwrap();let mut bytes=Vec::new();pdf.save_to(&mut bytes).unwrap();
    let mut decrypted=Document::load_mem(&bytes).unwrap();assert!(decrypted.encryption_state.is_some());
    assert!(Publication::save(&mut decrypted,true).unwrap().into_parts().1.is_none());
}

#[test]
fn native_size_matches_full_crop_rotation_origin_inheritance_and_units(){
    let root=PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let runtime=WorkerRuntime::new(&root.join("resources/pdfium/pdfium.dll"),Arc::new(crate::fonts::FontRegistry::new())).unwrap();
    let bytes=Publication::save(&mut source(32),true).unwrap().into_bytes();
    let document=runtime.pdfium.load_pdf_from_byte_slice(&bytes,None).unwrap();
    for index in 0..document.pages().len(){
        let full=page_geometry(&document.pages().get(index).unwrap()).unwrap().displayed_size();
        let fast=native_size(&document,index);assert!(fast.is_some(),"size-only native page {index}");assert_eq!(fast.unwrap(),full,"page {index}");
    }
    assert!(native_size(&document,document.pages().len()).is_none());
}

#[test]
fn certified_and_full_openers_keep_identical_page_metadata(){
    use super::super::OpenSecurity;
    let root=PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut runtime=WorkerRuntime::new(&root.join("resources/pdfium/pdfium.dll"),Arc::new(crate::fonts::FontRegistry::new())).unwrap();
    let publication=Publication::save(&mut source(32),true).unwrap();let bytes=publication.bytes.clone();
    assert!(publication.proof.as_ref().unwrap().matches(&bytes,32));
    let original=root.join("admission-matrix.pdf");
    let full=runtime.open_bytes(bytes,root.join("full-admission.pdf"),original.clone(),true,true,false,OpenSecurity::Authenticated(None)).unwrap();
    let fast=runtime.open_text_publication(publication,root.join("fast-admission.pdf"),original,false,OpenSecurity::Authenticated(None)).unwrap();
    assert_eq!(full.pages,fast.pages);assert_eq!(full.bookmarks,fast.bookmarks);assert_eq!(full.bookmark_error,fast.bookmark_error);
    for i in 0..32 {assert_eq!(runtime.source_page_unit(&full.id,i).unwrap(),runtime.source_page_unit(&fast.id,i).unwrap());}
    assert_eq!(runtime.document(&full.id).unwrap().imported_annotations,runtime.document(&fast.id).unwrap().imported_annotations);
}

#[test]
fn uncertified_and_stale_certificates_preserve_native_comment_imports(){
    use super::super::OpenSecurity;
    let root=PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut runtime=WorkerRuntime::new(&root.join("resources/pdfium/pdfium.dll"),Arc::new(crate::fonts::FontRegistry::new())).unwrap();
    for stale in [false,true] {
        let mut pdf=source(32);let page=*pdf.get_pages().values().next().unwrap();
        let note=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![20.into(),20.into(),40.into(),40.into()],"Contents"=>Object::string_literal("Preserve this note"),"C"=>vec![1.into(),1.into(),0.into()],"F"=>4});
        pdf.get_dictionary_mut(page).unwrap().set("Annots",vec![Object::Reference(note)]);
        let mut publication=Publication::save(&mut pdf,true).unwrap();assert!(publication.proof.is_none());
        if stale {
            // Model an internal stale pairing. The new payload has annotations;
            // a proof from a different writer output must never hide them.
            publication.proof=Publication::save(&mut source(32),true).unwrap().proof;
        }
        let full=runtime.open_bytes(publication.bytes.clone(),root.join(format!("full-comment-{stale}.pdf")),root.join("comment-source.pdf"),true,true,true,OpenSecurity::Authenticated(None)).unwrap();
        assert!(full.pages.iter().flat_map(|p|&p.overlays).any(|overlay|matches!(overlay,super::super::Overlay::Comment(note) if note.text=="Preserve this note")),"fixture must be importable under the original full admission path");
        let info=runtime.open_text_publication(publication,root.join(format!("comment-{stale}.pdf")),root.join("comment-source.pdf"),true,OpenSecurity::Authenticated(None)).unwrap();
        assert!(info.pages.iter().flat_map(|p|&p.overlays).any(|overlay|matches!(overlay,super::super::Overlay::Comment(note) if note.text=="Preserve this note")));
        assert!(runtime.document(&info.id).unwrap().imported_annotations.get(&0).is_some_and(|slots|slots.len()==1));
        assert_eq!(runtime.document(&full.id).unwrap().imported_annotations,runtime.document(&info.id).unwrap().imported_annotations);
    }
}
