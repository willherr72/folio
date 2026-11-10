use super::*;
use lopdf::{dictionary,Stream};
use crate::composition_structure::{inspect,Parents};

struct Fixture {catalog:ObjectId,page:ObjectId,root:ObjectId,head:ObjectId,cell:ObjectId,stream:ObjectId,annotation:ObjectId,attribute:ObjectId,namespace:ObjectId}
fn fixture(doc:&mut Document,role:&str)->Fixture {
    let pages=doc.new_object_id();let page=doc.new_object_id();let root=doc.new_object_id();
    let namespace=doc.add_object(dictionary!{"Type"=>"Namespace","NS"=>Object::string_literal("urn:custom:shared"),"RoleMapNS"=>dictionary!{"Custom"=>"P"}});
    let head=doc.add_object(dictionary!{"Type"=>"StructElem","S"=>"P","P"=>root,"Pg"=>page,"K"=>0,"ID"=>Object::string_literal(vec![0xfe,0xff,0,72]),"C"=>vec![Object::Name(b"Shared".to_vec()),2.into(),Object::Name(b"Empty".to_vec())]});
    let attribute=doc.add_object(dictionary!{"O"=>"Table","Headers"=>vec![Object::string_literal(vec![0xfe,0xff,0,72])],"RowSpan"=>2});
    let stream=doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","StructParents"=>7,"BBox"=>vec![0.into(),0.into(),10.into(),10.into()]},b"q Q".to_vec()));
    let annotation=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Link","StructParent"=>9,"Rect"=>vec![0.into(),0.into(),10.into(),10.into()]});
    let cell=doc.add_object(dictionary!{"Type"=>"StructElem","S"=>"Custom","NS"=>namespace,"P"=>root,"Pg"=>page,"K"=>vec![Object::Integer(2),Object::Dictionary(dictionary!{"Type"=>"MCR","Stm"=>stream,"MCID"=>0}),Object::Dictionary(dictionary!{"Type"=>"OBJR","Obj"=>annotation})],"ID"=>Object::string_literal("cell"),"A"=>vec![Object::Reference(attribute),3.into()],"C"=>"Shared","Alt"=>Object::string_literal("preserve alternate text"),"Ref"=>vec![Object::Reference(head)]});
    let parent=doc.add_object(dictionary!{"Nums"=>vec![0.into(),Object::Array(vec![head.into(),Object::Null,cell.into()]),7.into(),Object::Array(vec![cell.into()]),9.into(),cell.into()]});
    let idtree=doc.add_object(dictionary!{"Names"=>vec![Object::string_literal("cell"),cell.into(),Object::string_literal(vec![0xfe,0xff,0,72]),head.into()]});
    doc.objects.insert(root,dictionary!{"Type"=>"StructTreeRoot","K"=>vec![Object::Reference(head),cell.into()],"ParentTree"=>parent,"IDTree"=>idtree,"RoleMap"=>dictionary!{"P"=>"Alias","Alias"=>Object::Name(role.as_bytes().to_vec()),"Custom"=>"Sect"},"ClassMap"=>dictionary!{"Shared"=>attribute},"Namespaces"=>vec![Object::Reference(namespace)]}.into());
    doc.objects.insert(page,dictionary!{"Type"=>"Page","Parent"=>pages,"StructParents"=>0,"Annots"=>vec![Object::Reference(annotation)],"Resources"=>dictionary!{"XObject"=>dictionary!{"Fm"=>stream}}}.into());
    doc.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1}.into());
    let catalog=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages,"StructTreeRoot"=>root});
    Fixture {catalog,page,root,head,cell,stream,annotation,attribute,namespace}
}
fn check()->EngineResult<()>{Ok(())}
fn infos(doc:&Document,fixtures:&[Fixture])->Vec<StructureInfo>{fixtures.iter().map(|f|inspect(doc,f.catalog,&[f.page],&mut check).unwrap()).collect()}
#[test]
fn composition_structure_merge_isolates_roles_classes_ids_and_owner_keys() {
    let mut doc=Document::with_version("2.0");
    let a=fixture(&mut doc,"TH");let b=fixture(&mut doc,"H1");let fixtures=[a,b];
    // Source B defines a class which was deliberately empty in source A.
    doc.get_dictionary_mut(fixtures[1].root).unwrap().get_mut(b"ClassMap").unwrap().as_dict_mut().unwrap().set("Empty",dictionary!{"O"=>"Layout","Placement"=>"Block"});
    let source=infos(&doc,&fixtures);
    let root=merge(&mut doc,&source,&mut check).unwrap();
    let tree=doc.get_dictionary(root).unwrap();
    assert_eq!(tree.get(b"K").unwrap().as_array().unwrap(),&vec![fixtures[0].head.into(),fixtures[0].cell.into(),fixtures[1].head.into(),fixtures[1].cell.into()]);
    let roles=tree.get(b"RoleMap").unwrap().as_dict().unwrap();
    let classes=tree.get(b"ClassMap").unwrap().as_dict().unwrap();
    let mut ids=std::collections::BTreeSet::new();
    for (index,f) in fixtures.iter().enumerate() {
        let head=doc.get_dictionary(f.head).unwrap();let cell=doc.get_dictionary(f.cell).unwrap();
        let role=head.get(b"S").unwrap().as_name().unwrap();
        let alias=roles.get(role).unwrap().as_name().unwrap();
        assert_eq!(roles.get(alias).unwrap().as_name().unwrap(),if index==0 {b"TH".as_slice()} else {b"H1"});
        assert_eq!(cell.get(b"S").unwrap().as_name().unwrap(),b"Custom","explicit namespaces retain role names");
        assert_eq!(cell.get(b"NS").unwrap().as_reference().unwrap(),f.namespace);
        assert_eq!(head.get(b"P").unwrap().as_reference().unwrap(),root);
        let id=head.get(b"ID").unwrap().as_str().unwrap();assert!(ids.insert(id.to_vec()));
        let headers=doc.get_dictionary(f.attribute).unwrap().get(b"Headers").unwrap().as_array().unwrap();
        assert_eq!(headers[0].as_str().unwrap(),id);
        let names=head.get(b"C").unwrap().as_array().unwrap();
        assert_eq!(names[1].as_i64().unwrap(),2);
        assert_eq!(classes.has(names[2].as_name().unwrap()),index==1);
        assert_eq!(classes.get(names[0].as_name().unwrap()).unwrap().as_reference().unwrap(),f.attribute);
        assert_eq!(cell.get(b"A").unwrap().as_array().unwrap()[1],Object::Integer(3));
        assert_eq!(cell.get(b"Ref").unwrap().as_array().unwrap()[0],Object::Reference(f.head));
    }
    let catalog=doc.add_object(dictionary!{"Type"=>"Catalog","StructTreeRoot"=>root,"Sources"=>fixtures.iter().map(|f|Object::Reference(f.page)).collect::<Vec<_>>()});
    let merged=inspect(&doc,catalog,&fixtures.iter().map(|f|f.page).collect::<Vec<_>>(),&mut check).unwrap();
    assert_eq!(merged.owners.len(),6);
    assert_eq!(merged.elements.len(),4);
    for (index,f) in fixtures.iter().enumerate(){
        assert_eq!(merged.owners[&(index as i64*3)].parents,Parents::Marked(vec![Some(f.head),None,Some(f.cell)]));
        assert_eq!(merged.owners[&(index as i64*3+1)].object,f.stream);
        assert_eq!(merged.owners[&(index as i64*3+2)].object,f.annotation);
    }
}

#[test]
fn composition_structure_merge_refuses_stale_ids_and_malformed_name_trees() {
    for fault in 0..6 {
        let mut doc=Document::with_version("2.0");let a=fixture(&mut doc,"TH");let b=fixture(&mut doc,"H1");let fixtures=[a,b];
        let root=fixtures[0].root;
        let idtree=doc.get_dictionary(root).unwrap().get(b"IDTree").unwrap().as_reference().unwrap();
        match fault {
            0=>doc.get_dictionary_mut(fixtures[0].cell).unwrap().set("ID",Object::string_literal("missing")),
            1=>doc.get_dictionary_mut(idtree).unwrap().set("Names",vec![Object::string_literal("cell"),fixtures[0].head.into()]),
            2=>doc.get_dictionary_mut(idtree).unwrap().set("Limits",vec![Object::string_literal("wrong"),Object::string_literal("z")]),
            3=>doc.get_dictionary_mut(idtree).unwrap().set("Names",vec![Object::string_literal("cell"),fixtures[0].cell.into(),Object::string_literal("cell"),fixtures[0].head.into()]),
            4=>doc.get_dictionary_mut(fixtures[0].head).unwrap().set("ID",Object::string_literal("cell")),
            _=>doc.get_dictionary_mut(fixtures[0].attribute).unwrap().set("Headers",vec![Object::string_literal("missing")]),
        }
        let source=infos(&doc,&fixtures);
        assert!(merge(&mut doc,&source,&mut check).is_err(),"fault {fault}");
    }
}

#[test]
fn composition_structure_merge_cancellation_keeps_every_source_unchanged() {
    let mut doc=Document::with_version("2.0");let a=fixture(&mut doc,"TH");let b=fixture(&mut doc,"H1");
    let source=infos(&doc,&[a,b]);let objects=doc.objects.clone();let max_id=doc.max_id;
    let mut completed=false;
    for stop in 0..10_000 {
        let mut calls=0;
        let result=merge(&mut doc,&source,&mut ||{calls+=1;if calls>stop{Err(EngineError::InvalidRequest("test cancellation".into()))}else{Ok(())}});
        if result.is_ok(){completed=true;break;}
        assert!(result.unwrap_err().to_string().contains("test cancellation"));
        assert_eq!(doc.objects,objects);assert_eq!(doc.max_id,max_id);
    }
    assert!(completed);
}

#[test]
fn composition_structure_merge_keeps_attribute_stream_bytes_and_aliases() {
    let mut doc=Document::with_version("2.0");let a=fixture(&mut doc,"TH");let b=fixture(&mut doc,"H1");
    let attribute=doc.objects.remove(&a.attribute).unwrap().as_dict().unwrap().clone();
    let stream=Stream::new(attribute,vec![83,77,65,76,76]);let pointer=stream.content.as_ptr();
    doc.objects.insert(a.attribute,Object::Stream(stream));
    let alias=doc.add_object(Object::Reference(a.attribute));
    let array=doc.add_object(Object::Array(vec![alias.into(),3.into()]));
    doc.get_dictionary_mut(a.cell).unwrap().set("A",array);
    let fixtures=[a,b];let source=infos(&doc,&fixtures);
    merge(&mut doc,&source,&mut check).unwrap();
    let stream=doc.get_object(fixtures[0].attribute).unwrap().as_stream().unwrap();
    assert_eq!(stream.content,[83,77,65,76,76]);assert_eq!(stream.content.as_ptr(),pointer);
    assert_eq!(doc.get_dictionary(fixtures[0].cell).unwrap().get(b"A").unwrap().as_reference().unwrap(),array);
    assert_eq!(doc.get_object(array).unwrap().as_array().unwrap()[0],Object::Reference(alias));
    assert_eq!(stream.dict.get(b"Headers").unwrap().as_array().unwrap()[0],doc.get_dictionary(fixtures[0].head).unwrap().get(b"ID").unwrap().clone());
}

#[test]
fn composition_structure_merge_metadata_budget_refusal_is_atomic() {
    let mut doc=Document::with_version("2.0");let a=fixture(&mut doc,"TH");let b=fixture(&mut doc,"H1");
    let huge=vec![b'x';8*1024*1024];
    doc.get_dictionary_mut(a.head).unwrap().set("ID",Object::String(huge.clone(),StringFormat::Hexadecimal));
    let tree=doc.get_dictionary(a.root).unwrap().get(b"IDTree").unwrap().as_reference().unwrap();
    doc.get_dictionary_mut(tree).unwrap().set("Names",vec![Object::string_literal("cell"),a.cell.into(),Object::String(huge.clone(),StringFormat::Hexadecimal),a.head.into()]);
    doc.get_dictionary_mut(a.attribute).unwrap().set("Headers",vec![Object::String(huge,StringFormat::Hexadecimal)]);
    let source=infos(&doc,&[a,b]);let before=doc.objects.clone();let max_id=doc.max_id;
    assert!(merge(&mut doc,&source,&mut check).unwrap_err().to_string().contains("metadata budget"));
    assert_eq!(doc.objects,before);assert_eq!(doc.max_id,max_id);
}

#[test]
fn composition_structure_merge_null_optional_attributes_and_nested_id_tree_remain_compatible() {
    let mut doc=Document::with_version("1.7");let a=fixture(&mut doc,"TH");let b=fixture(&mut doc,"H1");
    doc.get_dictionary_mut(a.cell).unwrap().set("A",Object::Null);
    let tree=doc.get_dictionary(a.root).unwrap().get(b"IDTree").unwrap().as_reference().unwrap();
    let names=doc.get_dictionary_mut(tree).unwrap().remove(b"Names").unwrap().as_array().unwrap().clone();
    let left=doc.add_object(dictionary!{"Names"=>names[..2].to_vec(),"Limits"=>vec![names[0].clone(),names[0].clone()]});
    let right=doc.add_object(dictionary!{"Names"=>names[2..].to_vec(),"Limits"=>vec![names[2].clone(),names[2].clone()]});
    doc.get_dictionary_mut(tree).unwrap().set("Kids",vec![Object::Reference(left),right.into()]);
    let fixtures=[a,b];let source=infos(&doc,&fixtures);
    merge(&mut doc,&source,&mut check).unwrap();
    assert_eq!(doc.get_dictionary(fixtures[0].cell).unwrap().get(b"A").unwrap(),&Object::Null);
}

#[test]
fn composition_structure_merge_keeps_unmapped_role_meanings_and_avoids_generated_name_capture() {
    let mut doc=Document::with_version("2.0");let a=fixture(&mut doc,"VendorRole");let b=fixture(&mut doc,"H1");
    doc.get_dictionary_mut(b.head).unwrap().set("S",Object::Name(b"FolioSource0:P".to_vec()));
    let fixtures=[a,b];let source=infos(&doc,&fixtures);
    let root=merge(&mut doc,&source,&mut check).unwrap();
    let roles=doc.get_dictionary(root).unwrap().get(b"RoleMap").unwrap().as_dict().unwrap();
    let role=doc.get_dictionary(fixtures[0].head).unwrap().get(b"S").unwrap().as_name().unwrap();
    let alias=roles.get(role).unwrap().as_name().unwrap();
    assert_eq!(roles.get(alias).unwrap().as_name().unwrap(),b"VendorRole");
    let unmapped=doc.get_dictionary(fixtures[1].head).unwrap().get(b"S").unwrap().as_name().unwrap();
    assert_eq!(unmapped,b"FolioSource0:P");assert!(!roles.has(unmapped));
}

#[test]
#[ignore="requires explicit new synthetic output path"]
fn composition_structure_merge_exports_collision_fixture() {
    let mut doc=Document::with_version("2.0");let a=fixture(&mut doc,"TH");let b=fixture(&mut doc,"H1");
    let fixtures=[a,b];
    for (index,f) in fixtures.iter().enumerate() {
        doc.get_dictionary_mut(f.page).unwrap().set("MediaBox",vec![0.into(),0.into(),200.into(),200.into()]);
        let font=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
        doc.get_dictionary_mut(f.page).unwrap().get_mut(b"Resources").unwrap().as_dict_mut().unwrap().set("Font",dictionary!{"F1"=>font});
        let content=doc.add_object(Stream::new(dictionary!{},format!("/P <</MCID 0>> BDC BT /F1 12 Tf 20 160 Td (Heading {index}) Tj ET EMC /Custom <</MCID 2>> BDC BT /F1 12 Tf 20 130 Td (Cell {index}) Tj ET EMC").into_bytes()));
        doc.get_dictionary_mut(f.page).unwrap().set("Contents",content);
        doc.get_object_mut(f.stream).unwrap().as_stream_mut().unwrap().set_content(b"/Span <</MCID 0>> BDC q Q EMC".to_vec());
    }
    let sources=fixtures.iter().map(|f|crate::composition_catalog::Source{catalog:f.catalog,version:"2.0".into(),info:None}).collect();
    let mut merged=crate::composition_catalog::assemble(doc,sources,&mut check,std::sync::Arc::new(||false)).unwrap();
    let bytes=crate::pdf_output::save(&mut merged).unwrap();
    let saved=Document::load_mem(&bytes).unwrap();
    let catalog=saved.trailer.get(b"Root").unwrap().as_reference().unwrap();
    let pages=crate::composition_pages::inspect(&saved,catalog,&mut check).unwrap();
    let info=inspect(&saved,catalog,&pages.pages,&mut check).unwrap();
    assert_eq!(info.owners.len(),6);assert_eq!(info.elements.len(),4);
    use std::io::Write;
    let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(std::env::var_os("FOLIO_STRUCTURE_SYNTHETIC_OUTPUT").unwrap()).unwrap();
    file.write_all(&bytes).unwrap();file.sync_all().unwrap();
}
