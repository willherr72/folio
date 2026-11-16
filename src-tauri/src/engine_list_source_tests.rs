//! Native preparation and source-authority tests, before publisher integration.
use super::*;
use crate::form_answer_appearance::{CapturedListSources, prepare_list_source, FillAppearanceBudget};
use lopdf::{dictionary, Document, Object, Stream};

fn runtime() -> WorkerRuntime {
    let library = std::env::var_os("FOLIO_TEST_PDFIUM_LIBRARY").map(PathBuf::from)
        .unwrap_or_else(||PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"));
    WorkerRuntime::new(&library, Arc::new(FontRegistry::new())).unwrap()
}
fn fixture(name:&str)->Vec<u8> {
    std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("tests/fixtures/form-compound-list-{name}.pdf"))).unwrap()
}
fn source(raw:&[u8])->(Document,lopdf::ObjectId,Stream,lopdf::Dictionary) {
    let doc=Document::load_mem(raw).unwrap();
    let page=doc.get_dictionary(doc.get_pages()[&1]).unwrap();
    let id=page.get(b"Annots").unwrap().as_array().unwrap()[0].as_reference().unwrap();
    let widget=doc.get_dictionary(id).unwrap();
    let ap=doc.dereference(widget.get(b"AP").unwrap()).unwrap().1.as_dict().unwrap();
    let appearance=doc.dereference(ap.get(b"N").unwrap()).unwrap().1.as_stream().unwrap().clone();
    let resources=doc.dereference(appearance.dict.get(b"Resources").unwrap()).unwrap().1.as_dict().unwrap().clone();
    (doc,id,appearance,resources)
}
#[test]
fn list_source_native_preparation_keeps_frame_and_removes_only_proved_old_selection() {
    let r=runtime();
    for name in ["single-selection-paint","multi-selection-paint","scrolled-selection-paint"] {
        let raw=fixture(name);let(doc,id,appearance,resources)=source(&raw);
        let native=crate::form_fill::NativeForm::open(r.bindings,&raw).unwrap();
        let placements=[(0,0)];
        let captured=CapturedListSources::capture(&native,&doc,[(id,placements.as_slice())].into_iter(),true);
        let mut staging=Document::new();
        let mut budget=FillAppearanceBudget::new(&doc,&doc,&mut staging).unwrap();
        let prepared=prepare_list_source(&r.pdfium,&doc,&appearance,&resources,id,&captured,&mut budget).unwrap().unwrap();
        let mut expected=appearance.dict.clone();let mut actual=prepared.dict.clone();
        for key in [b"Length".as_slice(),b"Filter",b"DecodeParms"]{expected.remove(key);actual.remove(key);}
        assert_eq!(actual,expected,"temporary source must retain its exact resource/geometry context");
        let before=appearance.get_plain_content_with_limit(4*1024*1024).unwrap();
        let after=prepared.get_plain_content_with_limit(4*1024*1024).unwrap();
        let old_ops=lopdf::content::Content::decode_strict(&before).unwrap().operations;
        let new_ops=lopdf::content::Content::decode_strict(&after).unwrap().operations;
        assert!(old_ops.iter().any(|op|op.operator=="f"));
        assert!(!new_ops.iter().any(|op|matches!(op.operator.as_str(),"f"|"F"|"f*"|"B"|"B*"|"b"|"b*")),"stale selection paint remains");
        let border=old_ops.windows(2).find(|pair|pair[0].operator=="re"&&pair[1].operator=="s").unwrap();
        let preserved=new_ops.windows(2).filter(|pair|pair[0].operator=="re"&&pair[1].operator=="s").collect::<Vec<_>>();
        assert_eq!(preserved.len(),1);
        for(actual,expected)in preserved[0].iter().zip(border){
            assert_eq!(actual.operator,expected.operator);assert_eq!(actual.operands,expected.operands,"source border changed");
        }
        assert!(crate::form_answer_appearance::AnswerSlot::parse(&after,crate::form_answer_appearance::AnswerKind::List).is_ok());
        assert_eq!(std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("tests/fixtures/form-compound-list-{name}.pdf"))).unwrap(),raw,"original fixture bytes changed");
    }
}
#[test]
fn list_source_native_captured_authority_refuses_other_document_widget_or_changed_appearance() {
    let r=runtime();let raw=fixture("single-selection-paint");let(doc,id,appearance,resources)=source(&raw);
    let native=crate::form_fill::NativeForm::open(r.bindings,&raw).unwrap();let placements=[(0,0)];
    let captured=CapturedListSources::capture(&native,&doc,[(id,placements.as_slice())].into_iter(),true);
    let other=Document::load_mem(&raw).unwrap();
    for (owner,widget,changed) in [(&other,id,false),(&doc,(id.0+1000,id.1),false),(&doc,id,true)] {
        let mut candidate=appearance.clone();if changed{candidate.dict.set("BBox",vec![0.into(),0.into(),251.into(),70.into()]);}
        let mut staging=Document::new();let mut budget=FillAppearanceBudget::new(owner,owner,&mut staging).unwrap();
        assert!(prepare_list_source(&r.pdfium,owner,&candidate,&resources,widget,&captured,&mut budget).is_err());
    }
}
#[test]
fn list_source_native_absent_capture_keeps_supported_plain_path_and_refuses_repair() {
    let r=runtime();
    for (name,plain) in [("original-single",true),("single-selection-paint",false)] {
        let raw=fixture(name);let(doc,id,appearance,resources)=source(&raw);
        let native=crate::form_fill::NativeForm::open(r.bindings,&raw).unwrap();let placements=[(0,0)];
        let captured=CapturedListSources::capture(&native,&doc,[(id,placements.as_slice())].into_iter(),false);
        let mut staging=Document::new();let mut budget=FillAppearanceBudget::new(&doc,&doc,&mut staging).unwrap();
        let result=prepare_list_source(&r.pdfium,&doc,&appearance,&resources,id,&captured,&mut budget);
        if plain{assert!(result.unwrap().is_none());}else{assert!(result.is_err());}
    }
}

// These cases reach the native ink gate: labels, indices, row baselines and
// full-column rectangles alone would accept both shortened and lowered paint.
#[test]
fn list_source_native_ink_refuses_incomplete_selected_ink_and_neighbor_overlap() {
    let r=runtime();let raw=fixture("single-selection-paint");
    for (replacement,reason)in [("1 49.000000 248 4 re","selection does not contain"),
        ("1 46.500000 248 18.5 re","selection overlaps an unselected row")] {
        let(mut doc,id,mut appearance,resources)=source(&raw);
        let body=String::from_utf8(appearance.get_plain_content_with_limit(4*1024*1024).unwrap()).unwrap()
            .replace("1 49.000000 248 16 re",replacement);
        appearance.set_plain_content(body.into_bytes());
        let value=doc.add_object(appearance.clone());
        doc.get_dictionary_mut(id).unwrap().set("AP",lopdf::dictionary!{"N"=>value});
        let modified=crate::pdf_output::save(&mut doc).unwrap();
        let native=crate::form_fill::NativeForm::open(r.bindings,&modified).unwrap();let placements=[(0,0)];
        let captured=CapturedListSources::capture(&native,&doc,[(id,placements.as_slice())].into_iter(),true);
        let mut staging=Document::new();let mut budget=FillAppearanceBudget::new(&doc,&doc,&mut staging).unwrap();
        let error=prepare_list_source(&r.pdfium,&doc,&appearance,&resources,id,&captured,&mut budget).err()
            .unwrap_or_else(||panic!("unproved selection paint accepted: {replacement}")).to_string();
        assert!(error.contains(reason),"expected native-ink failure {reason}, got {error}");
    }
}

#[test]
fn list_source_native_captures_before_filling_changes_its_selection() {
    let r=runtime();let raw=fixture("single-selection-paint");let(doc,id,appearance,resources)=source(&raw);
    let native=crate::form_fill::NativeForm::open(r.bindings,&raw).unwrap();let placements=[(0,0)];
    let captured=CapturedListSources::capture(&native,&doc,[(id,placements.as_slice())].into_iter(),true);
    native.fill(&crate::FillFormFieldRequest{page_index:0,annotation_index:0,
        expected_value:crate::FormFieldValue::Choice{indices:vec![0]},value:crate::FormFieldValue::Choice{indices:vec![2]}}).unwrap();
    let mut staging=Document::new();let mut budget=FillAppearanceBudget::new(&doc,&doc,&mut staging).unwrap();
    assert!(prepare_list_source(&r.pdfium,&doc,&appearance,&resources,id,&captured,&mut budget).unwrap().is_some());
    let late=CapturedListSources::capture(&native,&doc,[(id,placements.as_slice())].into_iter(),true);
    let mut staging=Document::new();let mut budget=FillAppearanceBudget::new(&doc,&doc,&mut staging).unwrap();
    assert!(prepare_list_source(&r.pdfium,&doc,&appearance,&resources,id,&late,&mut budget).is_err(),"post-fill state must not authorize old row paint");
}

#[test]
fn list_source_native_budget_refusal_keeps_original_and_plain_legacy_path() {
    let r=runtime();
    for(name,plain)in [("original-single",true),("single-selection-paint",false)] {
        let raw=fixture(name);let(doc,id,appearance,resources)=source(&raw);
        let native=crate::form_fill::NativeForm::open(r.bindings,&raw).unwrap();let placements=[(0,0)];
        let captured=CapturedListSources::capture(&native,&doc,[(id,placements.as_slice())].into_iter(),true);
        let mut staging=Document::new();
        let mut budget=FillAppearanceBudget::with_limit(&doc,&doc,&mut staging,1).unwrap();
        let result=prepare_list_source(&r.pdfium,&doc,&appearance,&resources,id,&captured,&mut budget);
        if plain{assert!(result.unwrap().is_none());}else{assert!(result.is_err());}
        drop(budget);assert!(staging.objects.is_empty(),"failed preparation must never retain graph objects");
        let original=source(&raw).2;assert_eq!(original.dict,appearance.dict);assert_eq!(original.content,appearance.content);
    }
}

#[test]
fn list_source_native_preparation_handles_shifted_bounds_and_appearance_rotation() {
    let r=runtime();let raw=fixture("single-selection-paint");
    for shifted in [false,true] {
        let(mut doc,id,mut appearance,resources)=source(&raw);
        if shifted {
            let mut content=lopdf::content::Content::decode_strict(&appearance.get_plain_content_with_limit(4*1024*1024).unwrap()).unwrap();
            let mut first_td=true;
            for op in &mut content.operations {
                if op.operator=="re"||op.operator=="Td"&&first_td {
                    op.operands[0]=Object::Real(op.operands[0].as_float().unwrap()+17.);
                    op.operands[1]=Object::Real(op.operands[1].as_float().unwrap()-13.);
                    if op.operator=="Td"{first_td=false;}
                }
            }
            appearance.set_plain_content(content.encode().unwrap());
            appearance.dict.set("BBox",vec![17.into(),(-13).into(),267.into(),57.into()]);
        }else{
            appearance.dict.set("Matrix",vec![0.into(),1.into(),(-1).into(),0.into(),70.into(),0.into()]);
        }
        let value=doc.add_object(appearance.clone());doc.get_dictionary_mut(id).unwrap().set("AP",dictionary!{"N"=>value});
        let modified=crate::pdf_output::save(&mut doc).unwrap();
        let native=crate::form_fill::NativeForm::open(r.bindings,&modified).unwrap();let placements=[(0,0)];
        let captured=CapturedListSources::capture(&native,&doc,[(id,placements.as_slice())].into_iter(),true);
        let mut staging=Document::new();let mut budget=FillAppearanceBudget::new(&doc,&doc,&mut staging).unwrap();
        let prepared=prepare_list_source(&r.pdfium,&doc,&appearance,&resources,id,&captured,&mut budget).unwrap().unwrap();
        for key in [b"BBox".as_slice(),b"Matrix"]{assert_eq!(prepared.dict.get(key).ok(),appearance.dict.get(key).ok());}
    }
}

#[test]
fn list_source_native_preparation_uses_actual_embedded_cid_font_codes() {
    let r=runtime();let raw=fixture("single-selection-paint");let(mut doc,id,mut appearance,_)=source(&raw);
    let labels=["Red","Green","Blue"];
    let characters:std::collections::BTreeSet<_>=labels.iter().flat_map(|label|label.chars()).collect();
    let(font_doc,font,_)=crate::choice_font_tests::cid_for(&characters.into_iter().collect::<Vec<_>>());
    let proof=crate::choice_font::ChoiceFontProof::for_texts(&font_doc,&font,&labels).unwrap();
    let copied=crate::form_appearance::Copier::new(&font_doc,&mut doc).copy(&font).unwrap();
    let resources=dictionary!{"Font"=>dictionary!{"F1"=>copied}};
    appearance.dict.set("Resources",resources.clone());
    let mut body=String::from_utf8(appearance.get_plain_content_with_limit(4*1024*1024).unwrap()).unwrap().replace("/Helv 14 Tf","/F1 14 Tf");
    for label in labels {
        let hex:String=proof.encode(label).unwrap().iter().map(|byte|format!("{byte:02X}")).collect();
        body=body.replace(&format!("({label}) Tj"),&format!("<{hex}> Tj"));
    }
    appearance.set_plain_content(body.into_bytes());
    let value=doc.add_object(appearance.clone());doc.get_dictionary_mut(id).unwrap().set("AP",dictionary!{"N"=>value});
    let modified=crate::pdf_output::save(&mut doc).unwrap();
    let native=crate::form_fill::NativeForm::open(r.bindings,&modified).unwrap();let placements=[(0,0)];
    let captured=CapturedListSources::capture(&native,&doc,[(id,placements.as_slice())].into_iter(),true);
    let mut staging=Document::new();let mut budget=FillAppearanceBudget::new(&doc,&doc,&mut staging).unwrap();
    assert!(prepare_list_source(&r.pdfium,&doc,&appearance,&resources,id,&captured,&mut budget).unwrap().is_some());
}


#[test]
fn list_source_native_linked_widgets_keep_independent_top_index_authority() {
    let r=runtime();let raw=fixture("scrolled-selection-paint");let(mut doc,first,appearance,resources)=source(&raw);
    let mut owner=lopdf::Dictionary::new();
    for key in [b"FT".as_slice(),b"Ff",b"T",b"TU",b"Opt",b"V",b"I",b"DV",b"DA"]{
        if let Some(value)=doc.get_dictionary_mut(first).unwrap().remove(key){owner.set(key.to_vec(),value);}
    }
    owner.set("TI",5);
    let parent=doc.add_object(owner);doc.get_dictionary_mut(first).unwrap().set("Parent",parent);
    doc.get_dictionary_mut(first).unwrap().remove(b"TI");
    let mut sibling=doc.get_dictionary(first).unwrap().clone();
    // Same AP and owner, but an incompatible first-visible index on this widget.
    sibling.set("TI",0);sibling.set("Rect",vec![40.into(),60.into(),290.into(),130.into()]);
    let second=doc.add_object(sibling);
    doc.get_dictionary_mut(parent).unwrap().set("Kids",vec![first.into(),second.into()]);
    let page=doc.get_pages()[&1];doc.get_dictionary_mut(page).unwrap().set("Annots",vec![first.into(),second.into()]);
    let form=doc.catalog().unwrap().get(b"AcroForm").unwrap().as_reference().ok();
    if let Some(form)=form{doc.get_dictionary_mut(form).unwrap().set("Fields",vec![parent.into()]);}
    else{doc.catalog_mut().unwrap().get_mut(b"AcroForm").unwrap().as_dict_mut().unwrap().set("Fields",vec![parent.into()]);}
    let bytes=crate::pdf_output::save(&mut doc).unwrap();
    let native=crate::form_fill::NativeForm::open(r.bindings,&bytes).unwrap();let a=[(0,0)];let b=[(0,1)];
    let captured=CapturedListSources::capture(&native,&doc,[(first,a.as_slice()),(second,b.as_slice())].into_iter(),true);
    let mut staging=Document::new();let mut budget=FillAppearanceBudget::new(&doc,&doc,&mut staging).unwrap();
    assert!(prepare_list_source(&r.pdfium,&doc,&appearance,&resources,first,&captured,&mut budget).unwrap().is_some());
    let mut staging=Document::new();let mut budget=FillAppearanceBudget::new(&doc,&doc,&mut staging).unwrap();
    let error=prepare_list_source(&r.pdfium,&doc,&appearance,&resources,second,&captured,&mut budget).err().expect("sibling borrowed first widget's top index").to_string();
    assert!(error.contains("label does not match its option index"),"{error}");
}


#[test]
fn list_source_native_exhausted_optional_capture_preserves_supported_legacy_sources() {
    let r=runtime();
    for(name,plain)in [("original-single",true),("single-selection-paint",false)] {
        let raw=fixture(name);let(doc,id,appearance,resources)=source(&raw);
        let native=crate::form_fill::NativeForm::open(r.bindings,&raw).unwrap();let placements=[(0,0)];
        let captured=CapturedListSources::capture_with_limit(&native,&doc,[(id,placements.as_slice())].into_iter(),true,0);
        let mut staging=Document::new();let mut budget=FillAppearanceBudget::new(&doc,&doc,&mut staging).unwrap();
        let result=prepare_list_source(&r.pdfium,&doc,&appearance,&resources,id,&captured,&mut budget);
        if plain{assert!(result.unwrap().is_none());}else{
            let error=result.err().expect("repair must require complete captured evidence").to_string();
            assert!(error.contains("no captured native evidence"),"{error}");
        }
        assert_eq!(native.field_bounded(0,0,8*1024*1024).unwrap().selected_indices,vec![0]);
    }
}

fn precision_source(program:&str,bbox:[i64;4])->(Vec<u8>,Document,lopdf::ObjectId,Stream,lopdf::Dictionary) {
    let raw=fixture("single-selection-paint");let(mut doc,id,mut appearance,resources)=source(&raw);
    appearance.set_plain_content(program.as_bytes().to_vec());
    appearance.dict.set("BBox",bbox.into_iter().map(Object::Integer).collect::<Vec<_>>());
    let normal=doc.add_object(appearance.clone());doc.get_dictionary_mut(id).unwrap().set("AP",dictionary!{"N"=>normal});
    let bytes=crate::pdf_output::save(&mut doc).unwrap();(bytes,doc,id,appearance,resources)
}

#[test]
fn list_source_native_scaled_text_preserves_actual_ink_and_row_band() {
    let r=runtime();let raw=fixture("single-selection-paint");let(_,_,old,_)=source(&raw);
    let body=String::from_utf8(old.get_plain_content_with_limit(4*1024*1024).unwrap()).unwrap()
        .replace("2 68 Td","2 0 0 2 0 0 Tm 1 34 Td")
        .replace("0 -15.624001 Td","0 -7.8120005 Td").replace("/Helv 14 Tf","/Helv 7 Tf");
    let(raw,doc,id,appearance,resources)=precision_source(&body,[0,0,250,70]);
    let native=crate::form_fill::NativeForm::open(r.bindings,&raw).unwrap();let placements=[(0,0)];
    let captured=CapturedListSources::capture(&native,&doc,[(id,placements.as_slice())].into_iter(),true);
    let mut staging=Document::new();let mut budget=FillAppearanceBudget::new(&doc,&doc,&mut staging).unwrap();
    assert!(prepare_list_source(&r.pdfium,&doc,&appearance,&resources,id,&captured,&mut budget).unwrap().is_some());
}

#[test]
fn list_source_native_large_coordinate_offset_cannot_authorize_a_partial_column() {
    let r=runtime();
    let(raw,doc,id,appearance,resources)=precision_source("/Tx BMC q .125 w 0 0 0 RG\n900000.0625 .0625 .875 69.875 re s\n900000.125 1 .75 68 re W n\nq .7 .8 1 rg 900000.3125 49 .375 16 re f Q\nBT 0 g 900000.25 68 Td 0 -15.624001 Td /Helv .1 Tf (Red) Tj\n0 -15.624001 Td /Helv .1 Tf (Green) Tj\n0 -15.624001 Td /Helv .1 Tf (Blue) Tj ET Q EMC",[900000,0,900001,70]);
    let native=crate::form_fill::NativeForm::open(r.bindings,&raw).unwrap();let placements=[(0,0)];
    let captured=CapturedListSources::capture(&native,&doc,[(id,placements.as_slice())].into_iter(),true);
    let mut staging=Document::new();let mut budget=FillAppearanceBudget::new(&doc,&doc,&mut staging).unwrap();
    let error=prepare_list_source(&r.pdfium,&doc,&appearance,&resources,id,&captured,&mut budget).err()
        .expect("absolute coordinate magnitude authorized a half-column rectangle").to_string();
    assert!(error.contains("does not span its row column"),"{error}");
}

#[test]
fn list_source_native_large_coordinate_offset_cannot_hide_truncated_selected_ink() {
    let r=runtime();
    let(raw,doc,id,appearance,resources)=precision_source("/Tx BMC q 1 w 0 0 0 RG\n.5 900000.5 249 69 re s\n1 900001 248 68 re W n\nq .7 .8 1 rg 1 900049 248 13.25 re f Q\nBT 0 g 2 900068 Td 0 -15.624001 Td /Helv 14 Tf (Red) Tj\n0 -15.624001 Td /Helv 14 Tf (Green) Tj\n0 -15.624001 Td /Helv 14 Tf (Blue) Tj ET Q EMC",[0,900000,250,900070]);
    let native=crate::form_fill::NativeForm::open(r.bindings,&raw).unwrap();let placements=[(0,0)];
    let captured=CapturedListSources::capture(&native,&doc,[(id,placements.as_slice())].into_iter(),true);
    let mut staging=Document::new();let mut budget=FillAppearanceBudget::new(&doc,&doc,&mut staging).unwrap();
    let error=prepare_list_source(&r.pdfium,&doc,&appearance,&resources,id,&captured,&mut budget).err()
        .expect("absolute coordinate magnitude hid truncated glyph ink").to_string();
    assert!(error.contains("selection does not contain"),"{error}");
}

#[test]
fn list_source_native_large_offsets_still_accept_complete_column_and_ink() {
    let r=runtime();
    for(program,bbox)in [
        ("/Tx BMC q .125 w 0 0 0 RG\n900000.0625 .0625 .875 69.875 re s\n900000.125 1 .75 68 re W n\nq .7 .8 1 rg 900000.125 49 .75 16 re f Q\nBT 0 g 900000.25 68 Td 0 -15.624001 Td /Helv .1 Tf (Red) Tj\n0 -15.624001 Td /Helv .1 Tf (Green) Tj\n0 -15.624001 Td /Helv .1 Tf (Blue) Tj ET Q EMC",[900000,0,900001,70]),
        ("/Tx BMC q 1 w 0 0 0 RG\n.5 900000.5 249 69 re s\n1 900001 248 68 re W n\nq .7 .8 1 rg 1 900049 248 16 re f Q\nBT 0 g 2 900068 Td 0 -15.624001 Td /Helv 14 Tf (Red) Tj\n0 -15.624001 Td /Helv 14 Tf (Green) Tj\n0 -15.624001 Td /Helv 14 Tf (Blue) Tj ET Q EMC",[0,900000,250,900070]),
    ] {
        let(raw,doc,id,appearance,resources)=precision_source(program,bbox);
        let native=crate::form_fill::NativeForm::open(r.bindings,&raw).unwrap();let placements=[(0,0)];
        let captured=CapturedListSources::capture(&native,&doc,[(id,placements.as_slice())].into_iter(),true);
        let mut staging=Document::new();let mut budget=FillAppearanceBudget::new(&doc,&doc,&mut staging).unwrap();
        assert!(prepare_list_source(&r.pdfium,&doc,&appearance,&resources,id,&captured,&mut budget).unwrap().is_some());
    }
}


// Test-only independent native generation: never connected to a desktop command.
#[test]
fn list_source_native_reference_fill_clear_refill_states_are_reopenable() {
    let r=runtime();
    for(name,multi)in [("single-selection-paint",false),("multi-selection-paint",true),("scrolled-selection-paint",false)] {
        let raw=fixture(name);let initial=crate::form_fill::NativeForm::open(r.bindings,&raw).unwrap();
        let source=initial.field_bounded(0,0,8*1024*1024).unwrap();drop(initial);
        let mut current=raw;
        let values=if multi{vec![vec![1,2],vec![],vec![0,2]]}else{vec![vec![1],vec![],vec![2]]};
        for(cycle,indices)in values.iter().cycle().take(9).enumerate(){
            let native=crate::form_fill::NativeForm::open(r.bindings,&current).unwrap();
            let field=native.field_bounded(0,0,8*1024*1024).unwrap();
            let wanted=crate::FormFieldValue::Choice{indices:indices.clone()};
            let bytes=native.fill(&crate::FillFormFieldRequest{page_index:0,annotation_index:0,expected_value:field.value,value:wanted.clone()}).unwrap();
            let reopened=crate::form_fill::NativeForm::open(r.bindings,&bytes).unwrap();
            let after=reopened.field_bounded(0,0,8*1024*1024).unwrap();
            assert_eq!(after.value,wanted);assert_eq!(after.selected_indices,*indices);
            assert_eq!(after.options,source.options);assert_eq!(after.multi_select,source.multi_select);
            if let Some(root)=std::env::var_os("FOLIO_LIST_SOURCE_REFERENCES") {
                use std::io::Write;
                let dir=PathBuf::from(root);std::fs::create_dir_all(&dir).unwrap();
                let path=dir.join(format!("{name}-native-{cycle:02}.pdf"));
                std::fs::OpenOptions::new().create_new(true).write(true).open(path).unwrap().write_all(&bytes).unwrap();
            }
            drop(reopened);drop(native);current=bytes;
        }
    }
}

