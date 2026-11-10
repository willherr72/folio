use super::*;
use crate::composition_graph::CompositionGraph;
use lopdf::{dictionary,Stream};
use std::sync::Arc;

fn fixture(tagged:bool)->Vec<u8> {
    let mut doc=Document::with_version("1.7");
    let pages=doc.new_object_id();
    let content=doc.add_object(Stream::new(dictionary!{},b"q Q".to_vec()));
    let image=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>1,"Height"=>1,"ColorSpace"=>"DeviceRGB","BitsPerComponent"=>8},vec![10,20,30]));
    let comment=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","Contents"=>Object::string_literal("keep comment"),"Rect"=>vec![0.into(),0.into(),10.into(),10.into()]});
    let first=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"Contents"=>content,"Annots"=>vec![Object::Reference(comment)],"UserUnit"=>1.7});
    let second=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"Contents"=>content});
    let link=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Link","Rect"=>vec![10.into(),0.into(),20.into(),10.into()],"Dest"=>vec![Object::Reference(second),Object::Name(b"Fit".to_vec())]});
    doc.get_dictionary_mut(first).unwrap().get_mut(b"Annots").unwrap().as_array_mut().unwrap().push(link.into());
    doc.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>2,"Kids"=>vec![Object::Reference(first),second.into()],"MediaBox"=>vec![0.into(),0.into(),200.into(),100.into()],"Rotate"=>90,"Resources"=>dictionary!{"XObject"=>dictionary!{"Shared"=>image}}}.into());
    let catalog=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages,"Lang"=>Object::string_literal("en-US"),"ViewerPreferences"=>dictionary!{"DisplayDocTitle"=>true}});
    if tagged {
        let root=doc.new_object_id();
        let elem=doc.add_object(dictionary!{"Type"=>"StructElem","S"=>"Custom","P"=>root,"Pg"=>first,"K"=>0,"Alt"=>Object::string_literal("retained")});
        let parent=doc.add_object(dictionary!{"Nums"=>vec![0.into(),Object::Array(vec![elem.into()])]});
        doc.objects.insert(root,dictionary!{"Type"=>"StructTreeRoot","K"=>vec![Object::Reference(elem)],"ParentTree"=>parent,"RoleMap"=>dictionary!{"Custom"=>"P"}}.into());
        doc.get_dictionary_mut(first).unwrap().set("StructParents",0);
        doc.get_dictionary_mut(catalog).unwrap().set("StructTreeRoot",root);
        doc.get_dictionary_mut(catalog).unwrap().set("MarkInfo",dictionary!{"Marked"=>true});
    }
    doc.trailer.set("Root",catalog);
    let mut bytes=vec![];doc.save_to(&mut bytes).unwrap();bytes
}
fn graph(bytes:&[Vec<u8>])->CompositionGraph {
    let mut graph=CompositionGraph::new();
    for (key,bytes) in bytes.iter().enumerate(){graph.import_bytes(key,bytes,Arc::new(||false)).unwrap();}
    graph
}
fn check()->EngineResult<()>{Ok(())}

#[test]
fn composition_language_integration_keeps_leading_info_and_unmarked_source_defaults() {
    let mut first=Document::load_mem(&fixture(true)).unwrap();
    let info=first.add_object(dictionary!{"Title"=>Object::string_literal("English title")});first.trailer.set("Info",info);
    let mut second=Document::load_mem(&fixture(false)).unwrap();second.catalog_mut().unwrap().remove(b"Lang");
    let mut a=vec![];let mut b=vec![];first.save_to(&mut a).unwrap();second.save_to(&mut b).unwrap();
    let output=graph(&[a,b]).assemble(&[0,1],&mut check).unwrap();
    assert_eq!(output.catalog().unwrap().get(b"Lang").unwrap().as_str().unwrap(),b"en-US","leading Info must keep its original language context");
    let pages=output.get_pages();
    let content=output.get_page_content_with_limit(pages[&3],1_048_576).unwrap();
    assert!(content.starts_with(b"/Span << /Lang <> >> BDC\n"),"later unmarked content needs its unknown source language");
    let catalog=output.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let structure=crate::composition_structure::inspect(&output,catalog,&pages.values().copied().collect::<Vec<_>>(),&mut check).unwrap();
    let note=output.get_dictionary(pages[&3]).unwrap().get(b"Annots").unwrap().as_array().unwrap()[0].as_reference().unwrap();
    let key=output.get_dictionary(note).unwrap().get(b"StructParent").unwrap().as_i64().unwrap();
    let crate::composition_structure::Parents::Whole(parent)=structure.owners[&key].parents else {panic!("whole annotation owner required")};
    assert_eq!(output.get_dictionary(parent).unwrap().get(b"Lang").unwrap().as_str().unwrap(),b"");
}

#[test]
fn composition_language_integration_refuses_unresolved_residual_text_strings() {
    let first=fixture(true);let mut second=Document::load_mem(&fixture(false)).unwrap();
    second.catalog_mut().unwrap().remove(b"Lang");
    let page=second.get_pages()[&1];second.get_dictionary_mut(page).unwrap().set("UnknownHumanText",Object::string_literal("Unclassified language"));
    let mut bytes=vec![];second.save_to(&mut bytes).unwrap();
    assert!(graph(&[first,bytes]).assemble(&[0,1],&mut check).is_err(),"an outer page Span cannot establish language for arbitrary metadata strings");
}

#[test]
fn composition_catalog_isolates_unknown_and_untagged_source_languages() {
    for tagged in [true,false] {for null in [true,false] {
        let first=fixture(true);
        let mut second=Document::load_mem(&fixture(tagged)).unwrap();
        if null {second.catalog_mut().unwrap().set("Lang",Object::Null);} else {second.catalog_mut().unwrap().remove(b"Lang");}
        let mut bytes=vec![];second.save_to(&mut bytes).unwrap();
        let doc=graph(&[first,bytes]).assemble(&[0,1],&mut check).unwrap();
        let catalog=doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
        let pages=crate::composition_pages::inspect(&doc,catalog,&mut check).unwrap();
        let structure=crate::composition_structure::inspect(&doc,catalog,&pages.pages,&mut check).unwrap();
        assert_eq!(doc.get_dictionary(structure.top[0]).unwrap().get(b"Lang").unwrap().as_str().unwrap(),b"en-US");
        if tagged {
            assert_eq!(doc.get_dictionary(structure.top[1]).unwrap().get(b"Lang").unwrap().as_str().unwrap(),b"", "later tagged source must retain unknown language");
        } else {
            assert_eq!(doc.catalog().unwrap().get(b"Lang").unwrap().as_str().unwrap(),b"en-US", "leading information retains its original default");
            assert!(doc.get_page_content_with_limit(pages.pages[2],1_048_576).unwrap().starts_with(b"/Span << /Lang <> >> BDC\n"),"untagged source retains unknown language through its own scope");
        }
    }}
}

#[test]
fn composition_catalog_keeps_leading_language_and_isolates_other_untagged_defaults() {
    let mut french=Document::load_mem(&fixture(false)).unwrap();
    french.catalog_mut().unwrap().set("Lang",Object::string_literal("fr-FR"));
    let mut bytes=vec![];french.save_to(&mut bytes).unwrap();
    let doc=graph(&[fixture(true),bytes.clone()]).assemble(&[0,1],&mut check).unwrap();
    assert_eq!(doc.catalog().unwrap().get(b"Lang").unwrap().as_str().unwrap(),b"en-US");
    let doc=graph(&[fixture(true),fixture(false),bytes]).assemble(&[0,1,2],&mut check).unwrap();
    let pages=doc.get_pages();
    assert!(doc.get_page_content_with_limit(pages[&5],1_048_576).unwrap().starts_with(b"/Span << /Lang <66722D4652> >> BDC\n"));
}

#[test]
fn composition_catalog_keeps_complete_source_branches_links_comments_and_sharing() {
    let doc=graph(&[fixture(true),fixture(false)]).assemble(&[0,1],&mut check).unwrap();
    let catalog=doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let pages=crate::composition_pages::inspect(&doc,catalog,&mut check).unwrap();
    assert_eq!(pages.pages.len(),4);
    let branches=doc.get_dictionary(pages.root).unwrap().get(b"Kids").unwrap().as_array().unwrap();
    assert_eq!(branches.len(),2);
    for (index,branch) in branches.iter().enumerate() {
        let branch=doc.get_dictionary(branch.as_reference().unwrap()).unwrap();
        assert_eq!(branch.get(b"Rotate").unwrap().as_i64().unwrap(),90);
        assert!(branch.has(b"Resources") && branch.has(b"MediaBox"));
        let a=doc.get_dictionary(pages.pages[index*2]).unwrap();
        let b=doc.get_dictionary(pages.pages[index*2+1]).unwrap();
        assert_eq!(a.get(b"Contents").unwrap(),b.get(b"Contents").unwrap());
        let annotations=a.get(b"Annots").unwrap().as_array().unwrap();
        assert_eq!(doc.get_dictionary(annotations[0].as_reference().unwrap()).unwrap().get(b"Contents").unwrap().as_str().unwrap(),b"keep comment");
        let link=doc.get_dictionary(annotations[1].as_reference().unwrap()).unwrap();
        assert_eq!(link.get(b"Dest").unwrap().as_array().unwrap()[0],Object::Reference(pages.pages[index*2+1]));
    }
    let structure=crate::composition_structure::inspect(&doc,catalog,&pages.pages,&mut check).unwrap();
    assert_eq!(structure.owners.len(),1);
    assert_eq!(doc.get_dictionary(structure.top[0]).unwrap().get(b"Alt").unwrap().as_str().unwrap(),b"retained");
    let mark=doc.catalog().unwrap().get(b"MarkInfo").unwrap().as_dict().unwrap();
    assert!(!mark.get(b"Marked").unwrap().as_bool().unwrap());
    assert!(!doc.catalog().unwrap().has(b"Metadata"));
}

#[test]
fn composition_catalog_source_order_and_cancellation_are_explicit() {
    let bytes=[fixture(false),fixture(true)];
    let result=graph(&bytes).assemble(&[1,0],&mut check).unwrap();
    let pages=result.get_pages();
    assert!(result.get_dictionary(pages[&1]).unwrap().has(b"StructParents"));
    assert!(!result.get_dictionary(pages[&3]).unwrap().has(b"StructParents"));
    for order in [vec![0],vec![0,0],vec![0,2]] {assert!(graph(&bytes).assemble(&order,&mut check).is_err());}
    let mut count=0;
    assert!(graph(&bytes).assemble(&[0,1],&mut ||{count+=1;if count>10 {Err(EngineError::InvalidRequest("test cancellation".into()))} else {Ok(())}}).unwrap_err().to_string().contains("test cancellation"));
}

#[test]
fn composition_catalog_unadapted_catalogs_remain_refused() {
    for key in ["AcroForm","Names","OpenAction","OCProperties","OutputIntents","UnknownExtension"] {
        let mut source=Document::load_mem(&fixture(false)).unwrap();
        if key=="AcroForm" {
            source.catalog_mut().unwrap().set(key,dictionary!{});
            let mut bytes=vec![];source.save_to(&mut bytes).unwrap();
            let output=graph(&[fixture(true),bytes]).assemble(&[0,1],&mut check).unwrap();
            assert!(output.catalog().unwrap().get(b"AcroForm").unwrap().as_dict().unwrap().get(b"Fields").unwrap().as_array().unwrap().is_empty());
        }
        source.catalog_mut().unwrap().set(key,if key=="AcroForm"{dictionary!{"UnsupportedExtension"=>true}}else{dictionary!{}});
        let mut bytes=vec![];source.save_to(&mut bytes).unwrap();
        assert!(graph(&[fixture(true),bytes]).assemble(&[0,1],&mut check).is_err(),"{key}");
    }
}

#[test]
fn composition_catalog_combines_multiple_structured_sources_without_key_collisions() {
    let document=graph(&[fixture(true),fixture(true),fixture(false)]).assemble(&[0,1,2],&mut check).unwrap();
    let catalog=document.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let pages=crate::composition_pages::inspect(&document,catalog,&mut check).unwrap();
    let info=crate::composition_structure::inspect(&document,catalog,&pages.pages,&mut check).unwrap();
    assert_eq!(pages.pages.len(),6);assert_eq!(info.owners.len(),2);assert_eq!(info.top.len(),2);
    let a=document.get_dictionary(info.top[0]).unwrap();let b=document.get_dictionary(info.top[1]).unwrap();
    assert_ne!(a.get(b"S").unwrap(),b.get(b"S").unwrap());
    assert_eq!(a.get(b"Lang").unwrap().as_str().unwrap(),b"en-US");
    assert_eq!(b.get(b"Lang").unwrap().as_str().unwrap(),b"en-US");
}

#[test]
fn composition_catalog_keeps_tagged_source_language_and_ordinary_info_without_conformance_claims() {
    let mut plain=Document::load_mem(&fixture(false)).unwrap();
    plain.catalog_mut().unwrap().set("Lang",Object::string_literal("de-DE"));
    let info=plain.add_object(dictionary!{"Title"=>Object::string_literal("Combined title")});
    plain.trailer.set("Info",info);
    let mut tagged=Document::load_mem(&fixture(true)).unwrap();
    tagged.version="2.0".into();
    tagged.catalog_mut().unwrap().set("Lang",Object::string_literal("fr-FR"));
    let metadata=tagged.add_object(Stream::new(dictionary!{"Type"=>"Metadata","Subtype"=>"XML"},b"source-only conformance claim".to_vec()));
    tagged.catalog_mut().unwrap().set("Metadata",metadata);
    tagged.catalog_mut().unwrap().set("MarkInfo",dictionary!{"Marked"=>true,"UserProperties"=>true,"Suspects"=>true});
    let mut a=vec![];let mut b=vec![];plain.save_to(&mut a).unwrap();tagged.save_to(&mut b).unwrap();
    let result=graph(&[a,b]).assemble(&[0,1],&mut check).unwrap();
    assert_eq!(result.version,"2.0");
    assert_eq!(result.catalog().unwrap().get(b"Lang").unwrap().as_str().unwrap(),b"de-DE");
    let catalog=result.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let pages=crate::composition_pages::inspect(&result,catalog,&mut check).unwrap();
    let structure=crate::composition_structure::inspect(&result,catalog,&pages.pages,&mut check).unwrap();
    assert_eq!(result.get_dictionary(structure.top[0]).unwrap().get(b"Lang").unwrap().as_str().unwrap(),b"fr-FR");
    let mark=result.catalog().unwrap().get(b"MarkInfo").unwrap().as_dict().unwrap();
    assert!(mark.get(b"UserProperties").unwrap().as_bool().unwrap());
    assert!(mark.get(b"Suspects").unwrap().as_bool().unwrap());
    assert!(!result.catalog().unwrap().has(b"Metadata"));
    let info=result.get_dictionary(result.trailer.get(b"Info").unwrap().as_reference().unwrap()).unwrap();
    assert_eq!(info.get(b"Title").unwrap().as_str().unwrap(),b"Combined title");
}

#[test]
fn composition_catalog_direct_header_copy_is_bounded() {
    let mut source=Document::load_mem(&fixture(false)).unwrap();
    source.catalog_mut().unwrap().set("Lang",Object::string_literal(vec![b'x';1_048_576]));
    let mut bytes=vec![];source.save_to(&mut bytes).unwrap();
    assert!(graph(&[bytes]).assemble(&[0],&mut check).unwrap_err().to_string().contains("header limit"));
}

#[test]
#[ignore="requires explicit authored input and new output paths"]
fn composition_catalog_authored_corpus() {
    let input=std::path::PathBuf::from(std::env::var_os("FOLIO_STRUCTURE_CORPUS").unwrap());
    let output=std::path::PathBuf::from(std::env::var_os("FOLIO_STRUCTURE_OUTPUT").unwrap());
    let sources=[std::fs::read(input.join("Word.pdf")).unwrap(),std::fs::read(input.join("Pictures.pdf")).unwrap()];
    let mut document=graph(&sources).assemble(&[0,1],&mut check).unwrap();
    let bytes=crate::pdf_output::save(&mut document).unwrap();
    let saved=Document::load_mem(&bytes).unwrap();
    let catalog=saved.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let pages=crate::composition_pages::inspect(&saved,catalog,&mut check).unwrap();
    let structure=crate::composition_structure::inspect(&saved,catalog,&pages.pages,&mut check).unwrap();
    assert_eq!(pages.pages.len(),4);assert_eq!(structure.elements.len(),37);assert_eq!(structure.owners.len(),4);
    use std::io::Write;
    let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(output).unwrap();
    file.write_all(&bytes).unwrap();file.sync_all().unwrap();
}

#[test]
#[ignore="requires explicit authored input and new output paths"]
fn composition_catalog_multi_authored_corpus_and_remerge() {
    let input=std::path::PathBuf::from(std::env::var_os("FOLIO_STRUCTURE_CORPUS").unwrap());
    let word=std::fs::read(input.join("Word.pdf")).unwrap();
    let pictures=std::fs::read(input.join("Pictures.pdf")).unwrap();
    let mut document=graph(&[word.clone(),word.clone(),pictures]).assemble(&[0,1,2],&mut check).unwrap();
    let first=crate::pdf_output::save(&mut document).unwrap();
    let mut remerged=graph(&[first.clone(),word]).assemble(&[0,1],&mut check).unwrap();
    let second=crate::pdf_output::save(&mut remerged).unwrap();
    for (bytes,env,pages_count,elements,owners) in [
        (first,"FOLIO_STRUCTURE_MULTI_OUTPUT",6,73,7),
        (second,"FOLIO_STRUCTURE_REMERGE_OUTPUT",8,109,10),
    ] {
        let saved=Document::load_mem(&bytes).unwrap();
        let catalog=saved.trailer.get(b"Root").unwrap().as_reference().unwrap();
        let pages=crate::composition_pages::inspect(&saved,catalog,&mut check).unwrap();
        let info=crate::composition_structure::inspect(&saved,catalog,&pages.pages,&mut check).unwrap();
        assert_eq!(pages.pages.len(),pages_count);assert_eq!(info.elements.len(),elements);assert_eq!(info.owners.len(),owners);
        use std::io::Write;
        let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(std::env::var_os(env).unwrap()).unwrap();
        file.write_all(&bytes).unwrap();file.sync_all().unwrap();
    }
}

#[path="composition_language_cases.rs"]
mod language_cases;
