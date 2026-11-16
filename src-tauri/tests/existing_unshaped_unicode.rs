use std::fs;
#[path="support/unshaped_fixture.rs"]
#[allow(dead_code)]
mod fixture_support;
use fixture_support::{engine,fixture,fixture_with_extra,font_streams,export,capture};

fn original_font_roundtrip(label:&str,values:[&str;3]) {
    roundtrip_with_extra(label,values,"");
}

fn roundtrip_with_extra(label:&str,values:[&str;3],extra:&str) {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    for rotation in [0,90,180,270] {for unit in [1.,1.7] {
        let label=format!("{label}-{rotation}-{unit}");let input=temp.path().join("original.pdf");
        fixture_with_extra(&input,&values,rotation,unit,false,extra);capture(&label,&input);
        let bytes=fs::read(&input).unwrap();let source=engine.open_document(&input).unwrap();
        let before=engine.list_occurrence_text_runs(&source.id,0).unwrap().runs;
        let selected=&before[0];assert_eq!(selected.text,values[0]);
        assert!(selected.supported,"{label}: {:?}",selected.reason);
        let noop=engine.replace_text_occurrence(&source.id,&[0],0,selected.object_index,values[0],values[0],None).unwrap();
        assert_eq!(engine.render_page(&source.id,0,800).unwrap(),engine.render_page(&noop.document.id,0,800).unwrap());
        engine.close_document(&noop.document.id).unwrap();
        let edited=engine.replace_text_occurrence(&source.id,&[0],0,selected.object_index,values[0],values[1],None).unwrap();
        let saved=temp.path().join("saved.pdf");export(&engine,&edited.document,&saved);capture(&label,&saved);
        assert_eq!(font_streams(&input),font_streams(&saved));
        let reopened=engine.open_document(&saved).unwrap();let after=engine.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;
        assert_eq!(after[0].text,values[1]);assert!(after[0].supported,"{label}: {:?}",after[0].reason);
        assert_eq!(after[0].font_name,selected.font_name);assert_eq!(after[0].font_size,selected.font_size);
        assert_eq!(after[0].display_font_size,selected.display_font_size);
        assert_eq!(after[1].text,before[1].text);assert_eq!(after[1].bounds,before[1].bounds);
        let again=engine.replace_text_occurrence(&reopened.id,&[0],0,after[0].object_index,values[1],values[2],None).unwrap();
        let second=temp.path().join("re-edited.pdf");export(&engine,&again.document,&second);capture(&label,&second);
        let final_doc=engine.open_document(&second).unwrap();assert_eq!(engine.list_occurrence_text_runs(&final_doc.id,0).unwrap().runs[0].text,values[2]);
        assert_eq!(font_streams(&input),font_streams(&second));assert_eq!(fs::read(&input).unwrap(),bytes);
        assert_eq!(engine.source_bytes(&source.id).unwrap().as_ref(),bytes.as_slice());
        for id in [&final_doc.id,&again.document.id,&reopened.id,&edited.document.id,&source.id] {engine.close_document(id).unwrap();}
    }}
}

#[test]
fn greek_original_font_survives_save_and_reedit() {
    original_font_roundtrip("greek",["Δοκιμή","Νέα δοκιμή","Αλλαγή"]);
}
#[test]
fn cyrillic_original_font_survives_save_and_reedit() {
    original_font_roundtrip("cyrillic",["Привет","Новый привет","Проверка"]);
}
#[test]
fn mixed_scripts_and_supported_symbols_keep_original_font() {
    original_font_roundtrip("mixed",["Value αβ Привет ±∞","New αβ Привет ±∞","Value γδ Мир ±∞"]);
}

#[test]
fn unshaped_edit_rejects_combining_bidi_shaping_and_missing_mappings_atomically() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();let input=temp.path().join("original.pdf");
    fixture(&input,&["Δοκιμή"],0,1.,false);let bytes=fs::read(&input).unwrap();let source=engine.open_document(&input).unwrap();
    let selected=engine.list_occurrence_text_runs(&source.id,0).unwrap().runs.remove(0);
    assert!(selected.supported,"{:?}",selected.reason);
    let raster=engine.render_page(&source.id,0,800).unwrap();
    for replacement in ["α\u{301}","α\u{202e}","مرحبا","क","😀","Ψ",&"Δ".repeat(2049)] {
        assert!(engine.replace_text_occurrence(&source.id,&[0],0,selected.object_index,&selected.text,replacement,None).is_err(),"Accepted {replacement:?}");
        assert_eq!(engine.render_page(&source.id,0,800).unwrap(),raster);assert_eq!(engine.source_bytes(&source.id).unwrap().as_ref(),bytes.as_slice());
    }
    assert_eq!(fs::read(input).unwrap(),bytes);engine.close_document(&source.id).unwrap();
}

#[test]
fn unused_shaping_entries_do_not_block_verified_greek_source() {
    roundtrip_with_extra("unused-map",["Δοκιμή","Νέα δοκιμή","Αλλαγή"],"م\u{301}");
}

#[test]
fn actual_shaping_source_and_incorrect_widths_stay_ineligible() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    for (index,text,bad_width)in[(0,"مرحبا",false),(1,"α\u{301}",false),(2,"Δοκιμή",true)] {
        let input=temp.path().join(format!("refused-{index}.pdf"));fixture(&input,&[text],0,1.,bad_width);
        let bytes=fs::read(&input).unwrap();let source=engine.open_document(&input).unwrap();
        let selected=engine.list_occurrence_text_runs(&source.id,0).unwrap().runs.remove(0);
        assert!(!selected.supported,"Unsafe source was admitted: {text}");
        assert!(engine.replace_text_occurrence(&source.id,&[0],0,selected.object_index,&selected.text,"Δοκιμή",None).is_err());
        assert_eq!(engine.source_bytes(&source.id).unwrap().as_ref(),bytes.as_slice());assert_eq!(fs::read(input).unwrap(),bytes);
        engine.close_document(&source.id).unwrap();
    }
}
