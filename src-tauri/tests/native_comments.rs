use folio_engine::{EditNativeCommentRequest, ExportRequest, PdfEngine};
use lopdf::{dictionary, Document, Object, Stream};
use std::{fs, path::PathBuf};
#[path = "support/protected_fixture.rs"]
mod protected_fixture;

fn fixture(flags: i64, rich: bool, direct: bool, rotation: i64) -> Vec<u8> {
    let mut d = Document::with_version("1.7");
    let pages = d.new_object_id();
    let page = d.new_object_id();
    let ap=d.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),18.into(),18.into()]},b"1 0.6 0 rg 0 0 18 18 re f".to_vec()));
    let note = d.new_object_id();
    let popup=d.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Popup","Parent"=>note,"Rect"=>vec![100.into(),200.into(),200.into(),300.into()],"Open"=>false});
    let mut data = dictionary! {"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![70.into(),230.into(),88.into(),248.into()],"F"=>flags,"Contents"=>Object::string_literal("Original note"),"T"=>Object::string_literal("Reviewer"),"Subj"=>Object::string_literal("Check this"),"Name"=>"Help","NM"=>Object::string_literal("stable-name"),"M"=>Object::string_literal("D:20261023090000"),"AP"=>dictionary!{"N"=>ap},"Popup"=>popup,"P"=>page};
    if direct {
        data.remove(b"Popup");
        d.get_dictionary_mut(popup).unwrap().remove(b"Parent");
    }
    if rich {
        data.set("RC", Object::string_literal("<body>Original note</body>"));
    }
    let note_value = if direct {
        Object::Dictionary(data)
    } else {
        d.objects.insert(note, Object::Dictionary(data));
        Object::Reference(note)
    };
    let imported=d.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Highlight","Rect"=>vec![100.into(),230.into(),118.into(),248.into()],"QuadPoints"=>vec![100.into(),248.into(),118.into(),248.into(),100.into(),230.into(),118.into(),230.into()],"C"=>vec![1.into(),1.into(),0.into()],"F"=>4,"Contents"=>Object::string_literal("Imported overlay")});
    let annots = d.add_object(vec![
        Object::Reference(imported),
        note_value,
        Object::Reference(popup),
    ]);
    d.objects.insert(page,Object::Dictionary(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"CropBox"=>vec![40.into(),50.into(),240.into(),350.into()],"Rotate"=>rotation,"Resources"=>dictionary!{},"Annots"=>annots}));
    d.objects.insert(
        pages,
        Object::Dictionary(
            dictionary! {"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1},
        ),
    );
    let root = d.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    d.trailer.set("Root", root);
    let mut bytes = vec![];
    d.save_to(&mut bytes).unwrap();
    bytes
}
fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}
fn request(text: &str) -> EditNativeCommentRequest {
    EditNativeCommentRequest {
        page_index: 0,
        annotation_index: 1,
        expected_text: "Original note".into(),
        text: text.into(),
        position:None,color:None,
    }
}

#[test]
fn native_notes_exclude_imported_overlays_and_use_displayed_crop_rotation_bounds() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    for (rotation, x, y) in [
        (0, 30.0, 102.0),
        (90, 180.0, 30.0),
        (180, 152.0, 180.0),
        (270, 102.0, 152.0),
    ] {
        let path = dir.path().join(format!("r{rotation}.pdf"));
        fs::write(&path, fixture(28, false, false, rotation)).unwrap();
        let source = e.open_document(path).unwrap();
        assert_eq!(source.pages[0].overlays.len(), 1);
        let notes = e.list_native_comments(&source.id).unwrap();
        assert_eq!(notes.len(), 1);
        let note = &notes[0];
        assert_eq!(note.annotation_index, 1);
        assert_eq!(note.text, "Original note");
        assert_eq!(note.author.as_deref(), Some("Reviewer"));
        assert_eq!((note.bounds.x, note.bounds.y), (x, y));
        assert!(note.reason.is_none());
        e.close_document(&source.id).unwrap();
    }
}

#[test]
fn unicode_edit_preserves_other_objects_pixels_original_and_exported_note() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    for direct in [false, true] {
        let original = fixture(28, false, direct, 0);
        let path = dir.path().join(format!("source{direct}.pdf"));
        fs::write(&path, &original).unwrap();
        let source = e.open_document(&path).unwrap();
        let before = e.render_page(&source.id, 0, 400).unwrap();
        let edited = e
            .edit_native_comment(&source.id, request("Updated café 日本語\nSecond line"))
            .unwrap();
        assert_eq!(
            e.list_native_comments(&edited.id).unwrap()[0].text,
            "Updated café 日本語\nSecond line"
        );
        assert_eq!(
            e.list_native_comments(&source.id).unwrap()[0].text,
            "Original note"
        );
        assert_eq!(fs::read(&path).unwrap(), original);
        assert_eq!(before, e.render_page(&edited.id, 0, 400).unwrap());
        let a = Document::load_mem(&original).unwrap();
        let b = Document::load_mem(&e.source_bytes(&edited.id).unwrap()).unwrap();
        let changed:Vec<_>=a.objects.iter().filter(|(_,obj)|!matches!(obj,Object::Stream(stream) if stream.dict.get(b"Type").and_then(Object::as_name).is_ok_and(|n|n==b"XRef"))).filter(|(id,obj)|{
            let other=b.objects.get(id).unwrap();match (obj,other){(Object::Stream(a),Object::Stream(b))=>a.dict!=b.dict||a.content!=b.content,_=>*obj!=other}
        }).collect();
        assert_eq!(
            changed.len(),
            1,
            "only the annotation or its direct array container changes"
        );
        fn without_contents(mut value: Object) -> Object {
            match &mut value {
                Object::Dictionary(d) => {
                    d.remove(b"Contents");
                    for (_, v) in d.iter_mut() {
                        *v = without_contents(v.clone());
                    }
                }
                Object::Array(a) => {
                    for v in a {
                        *v = without_contents(v.clone());
                    }
                }
                _ => {}
            }
            value
        }
        assert_eq!(
            without_contents(changed[0].1.clone()),
            without_contents(b.objects[changed[0].0].clone()),
            "every other entry in the changed container remains identical"
        );
        let size = &source.pages[0];
        let plan=serde_json::from_value(serde_json::json!({"id":"p","sourceId":edited.id,"pageIndex":0,"width":size.width,"height":size.height,"rotation":0,"overlays":size.overlays})).unwrap();
        let output = dir.path().join(format!("copy{direct}.pdf"));
        e.export_pdf(
            ExportRequest {
                pages: vec![plan],
                flatten: false,
            },
            &output,
        )
        .unwrap();
        let reopened = e.open_document(&output).unwrap();
        assert_eq!(reopened.pages[0].overlays.len(), 1);
        assert_eq!(
            e.list_native_comments(&reopened.id).unwrap()[0].text,
            "Updated café 日本語\nSecond line"
        );
        for id in [&source.id, &edited.id, &reopened.id] {
            e.close_document(id).unwrap();
        }
    }
}

#[test]
fn unsafe_native_notes_remain_readable_and_edits_are_rejected() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    for (flags, rich) in [
        (28 | 64, false),
        (28 | 128, false),
        (28 | 512, false),
        (28 | 2, false),
        (28, true),
    ] {
        let path = dir.path().join(format!("{flags}-{rich}.pdf"));
        let bytes = fixture(flags, rich, false, 0);
        fs::write(&path, &bytes).unwrap();
        let source = e.open_document(&path).unwrap();
        assert!(e.list_native_comments(&source.id).unwrap()[0]
            .reason
            .is_some());
        assert!(e
            .edit_native_comment(&source.id, request("Changed"))
            .is_err());
        assert!(e.delete_native_comment(&source.id,folio_engine::DeleteNativeCommentRequest {page_index:0,annotation_index:1,expected_text:"Original note".into()}).is_err());
        assert_eq!(&*e.source_bytes(&source.id).unwrap(), bytes);
        e.close_document(&source.id).unwrap();
    }
}

#[test]
fn stale_text_and_imported_indices_cannot_edit_the_wrong_comment() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.pdf");
    fs::write(&path, fixture(28, false, false, 0)).unwrap();
    let source = e.open_document(path).unwrap();
    let mut edit = request("Changed");
    edit.expected_text = "stale".into();
    assert!(e.edit_native_comment(&source.id, edit).is_err());
    let mut edit = request("Changed");
    edit.annotation_index = 0;
    edit.expected_text = "Imported overlay".into();
    assert!(e.edit_native_comment(&source.id, edit).is_err());
    assert!(e
        .edit_native_comment(&source.id, request("bad\0text"))
        .is_err());
    e.close_document(&source.id).unwrap();
}

#[test]
fn signatures_in_direct_nested_dictionaries_are_readable_but_never_rewritten() {
    let mut d = Document::load_mem(&fixture(28, false, false, 0)).unwrap();
    d.add_object(dictionary!{"Nested"=>dictionary!{"Type"=>"Sig","ByteRange"=>vec![0.into(),10.into(),20.into(),10.into()],"Contents"=>Object::string_literal("signature")}});
    let mut bytes = vec![];
    d.save_to(&mut bytes).unwrap();
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("signed.pdf");
    fs::write(&path, &bytes).unwrap();
    let source = e.open_document(path).unwrap();
    assert!(e.list_native_comments(&source.id).unwrap()[0]
        .reason
        .as_deref()
        .is_some_and(|r| r.contains("signatures")));
    assert!(e
        .edit_native_comment(&source.id, request("Changed"))
        .is_err());
    e.close_document(&source.id).unwrap();
}

#[test]
fn indirect_annotation_and_array_reference_chains_keep_their_identity() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    for direct in [false, true] {
        let mut d = Document::load_mem(&fixture(28, false, direct, 0)).unwrap();
        let page = *d.get_pages().values().next().unwrap();
        let array = d
            .get_dictionary(page)
            .unwrap()
            .get(b"Annots")
            .unwrap()
            .as_reference()
            .unwrap();
        let array_alias = d.add_object(Object::Reference(array));
        d.get_dictionary_mut(page)
            .unwrap()
            .set("Annots", array_alias);
        let target = if direct {
            None
        } else {
            Some(
                d.get_object(array).unwrap().as_array().unwrap()[1]
                    .as_reference()
                    .unwrap(),
            )
        };
        let alias = target.map(|id| d.add_object(Object::Reference(id)));
        if let Some(alias) = alias {
            d.get_object_mut(array).unwrap().as_array_mut().unwrap()[1] = Object::Reference(alias);
        }
        let mut bytes = vec![];
        d.save_to(&mut bytes).unwrap();
        let path = dir.path().join(format!("chain{direct}.pdf"));
        fs::write(&path, bytes).unwrap();
        let source = e.open_document(path).unwrap();
        let edited = e
            .edit_native_comment(&source.id, request("Chain edit"))
            .unwrap();
        let after = Document::load_mem(&e.source_bytes(&edited.id).unwrap()).unwrap();
        assert_eq!(after.objects[&array_alias], Object::Reference(array));
        if let (Some(alias), Some(target)) = (alias, target) {
            assert_eq!(after.objects[&alias], Object::Reference(target));
            assert_eq!(
                lopdf::decode_text_string(
                    after
                        .get_dictionary(target)
                        .unwrap()
                        .get(b"Contents")
                        .unwrap()
                )
                .unwrap(),
                "Chain edit"
            );
        }
        e.close_document(&edited.id).unwrap();
        e.close_document(&source.id).unwrap();
    }
}

#[test]
fn annotation_permission_is_authoritative_and_saved_copies_keep_passwords() {
    use lopdf::encryption::Permissions;
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    for (name, rights, can_edit, can_read) in [
        ("annotate", Permissions::ANNOTABLE, true, true),
        ("copy", Permissions::COPYABLE, false, true),
        ("none", Permissions::empty(), false, false),
    ] {
        let original = protected_fixture::encrypt(&fixture(28, false, false, 0), rights);
        let path = dir.path().join(format!("{name}.pdf"));
        fs::write(&path, &original).unwrap();
        let source = e.open_document_with_password(&path, "reader").unwrap();
        assert_eq!(e.list_native_comments(&source.id).is_ok(), can_read);
        let edited = e.edit_native_comment(&source.id, request("Protected comment"));
        assert_eq!(edited.is_ok(), can_edit);
        let deleted=e.delete_native_comment(&source.id,folio_engine::DeleteNativeCommentRequest {page_index:0,annotation_index:1,expected_text:"Original note".into()});
        assert_eq!(deleted.is_ok(),can_edit,"deletion obeys annotation permission independently of copying");
        if let Ok(deleted)=deleted {
            assert_eq!(deleted.protection,source.protection);
            assert!(e.list_native_comments(&deleted.id).unwrap().is_empty());
            let size=&source.pages[0];
            let plan=serde_json::from_value(serde_json::json!({"id":"p","sourceId":deleted.id,"pageIndex":0,"width":size.width,"height":size.height,"rotation":0,"overlays":size.overlays})).unwrap();
            let output=dir.path().join("protected-deletion.pdf");e.export_pdf(ExportRequest{pages:vec![plan],flatten:false},&output).unwrap();
            for password in ["reader","owner"] {let copy=e.open_document_with_password(&output,password).unwrap();assert!(e.list_native_comments(&copy.id).unwrap().is_empty());e.close_document(&copy.id).unwrap();}
            e.close_document(&deleted.id).unwrap();
        }
        if let Ok(edited) = edited {
            assert_eq!(edited.protection, source.protection);
            let size = &source.pages[0];
            let plan=serde_json::from_value(serde_json::json!({"id":"p","sourceId":edited.id,"pageIndex":0,"width":size.width,"height":size.height,"rotation":0,"overlays":size.overlays})).unwrap();
            let output = dir.path().join("protected-copy.pdf");
            e.export_pdf(
                ExportRequest {
                    pages: vec![plan],
                    flatten: false,
                },
                &output,
            )
            .unwrap();
            for password in ["reader", "owner"] {
                let copy = e.open_document_with_password(&output, password).unwrap();
                assert_eq!(
                    e.list_native_comments(&copy.id).unwrap()[0].text,
                    "Protected comment"
                );
                e.close_document(&copy.id).unwrap();
            }
            protected_fixture::record("native-comments", &original, &fs::read(output).unwrap());
            e.close_document(&edited.id).unwrap();
        }
        assert_eq!(fs::read(path).unwrap(), original);
        e.close_document(&source.id).unwrap();
    }
}

#[test]
fn recovery_keeps_native_edit_and_does_not_resurrect_deleted_overlay_comment() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.pdf");
    fs::write(&path, fixture(28, false, false, 0)).unwrap();
    let source = e.open_document(&path).unwrap();
    let edited = e
        .edit_native_comment(&source.id, request("Recovered native note"))
        .unwrap();
    let size = &edited.pages[0];
    let page = serde_json::json!({"id":"p","sourceId":edited.id,"pageIndex":0,"width":size.width,"height":size.height,"rotation":90,"overlays":[]});
    let workspace = serde_json::json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","savedDigest":"","dirty":true,"zoom":90,"scrollPosition":{"top":0,"left":0},"document":{"name":"source.pdf","pages":[page],"selectedPageId":"p","selectedOverlayId":null}}]});
    let store = folio_engine::RecoveryStore::new(dir.path().join("recovery")).unwrap();
    store.save(&e, workspace).unwrap();
    e.close_document(&edited.id).unwrap();
    e.close_document(&source.id).unwrap();
    fs::remove_file(path).unwrap();
    let restored = store.load(&e).unwrap().unwrap();
    let page = &restored["tabs"][0]["document"]["pages"][0];
    assert_eq!(page["overlays"].as_array().unwrap().len(), 0);
    let id = page["sourceId"].as_str().unwrap();
    let notes = e.list_native_comments(id).unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].text, "Recovered native note");
    let output = dir.path().join("recovered.pdf");
    e.export_pdf(
        serde_json::from_value(serde_json::json!({"pages":[page]})).unwrap(),
        &output,
    )
    .unwrap();
    let reopened = e.open_document(output).unwrap();
    assert!(reopened.pages[0].overlays.is_empty());
    assert_eq!(e.list_native_comments(&reopened.id).unwrap().len(), 1);
    e.close_document(&reopened.id).unwrap();
    e.close_document(id).unwrap();
}

#[test]
fn exported_popup_parent_points_to_its_own_note_after_page_duplication() {
    let e = engine();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.pdf");
    fs::write(&path, fixture(28, false, false, 0)).unwrap();
    let source = e.open_document(path).unwrap();
    let edited = e
        .edit_native_comment(&source.id, request("Popup relationship"))
        .unwrap();
    let pages=(0..2).map(|i|serde_json::from_value(serde_json::json!({"id":format!("page-{i}"),"sourceId":edited.id,"pageIndex":0,"width":200,"height":300,"rotation":i*90,"overlays":[]})).unwrap()).collect();
    let output = dir.path().join("duplicated.pdf");
    e.export_pdf(
        ExportRequest {
            pages,
            flatten: false,
        },
        &output,
    )
    .unwrap();
    let d = Document::load(&output).unwrap();
    let mut note_ids = std::collections::HashSet::new();
    for page in d.get_pages().values() {
        let annots = d.get_dictionary(*page).unwrap().get(b"Annots").unwrap();
        let annots = d.dereference(annots).unwrap().1.as_array().unwrap();
        let note = annots
            .iter()
            .find(|value| {
                d.dereference(value)
                    .unwrap()
                    .1
                    .as_dict()
                    .unwrap()
                    .get(b"Subtype")
                    .unwrap()
                    .as_name()
                    .unwrap()
                    == b"Text"
            })
            .unwrap();
        let note_id = note.as_reference().unwrap();
        assert!(note_ids.insert(note_id));
        let note = d.get_dictionary(note_id).unwrap();
        let popup = note.get(b"Popup").unwrap().as_reference().unwrap();
        assert_eq!(
            d.get_dictionary(popup)
                .unwrap()
                .get(b"Parent")
                .unwrap()
                .as_reference()
                .unwrap(),
            note_id,
            "popup must belong to its note, not itself or another page"
        );
        assert_eq!(note.get(b"P").unwrap().as_reference().unwrap(), *page);
    }
    e.close_document(&edited.id).unwrap();
    e.close_document(&source.id).unwrap();
}


#[test]
fn physical_units_scale_comment_hit_bounds_and_position_edits() {
    let e=engine();let dir=tempfile::tempdir().unwrap();
    for rotation in [0,90,180,270] {for unit in [0.5_f32,2.] {
        let mut pdf=Document::load_mem(&fixture(4,false,false,rotation)).unwrap();let page=*pdf.get_pages().values().next().unwrap();
        let baseline=dir.path().join(format!("comment-base-{rotation}-{unit}.pdf"));pdf.save(&baseline).unwrap();let base=e.open_document(&baseline).unwrap();let raw=e.list_native_comments(&base.id).unwrap().remove(0);
        pdf.get_dictionary_mut(page).unwrap().set("UserUnit",unit);let path=dir.path().join(format!("comment-units-{rotation}-{unit}.pdf"));pdf.save(&path).unwrap();let original=fs::read(&path).unwrap();let source=e.open_document(&path).unwrap();
        let note=e.list_native_comments(&source.id).unwrap().remove(0);
        for(a,b)in [(&note.bounds,&raw.bounds),(&note.hit_bounds,&raw.hit_bounds)]{assert_eq!([a.x,a.y,a.width,a.height],[b.x*unit,b.y*unit,b.width*unit,b.height*unit]);}
        let mut edit=request("Moved note");edit.position=Some(folio_engine::Point{x:20.*unit,y:30.*unit});let edited=e.edit_native_comment(&source.id,edit).unwrap();
        assert_eq!((edited.pages[0].width,edited.pages[0].height),(source.pages[0].width,source.pages[0].height));
        let changed=e.list_native_comments(&edited.id).unwrap().remove(0);assert_eq!((changed.bounds.x,changed.bounds.y),(20.*unit,30.*unit));assert_eq!(changed.text,"Moved note");assert_eq!(fs::read(&path).unwrap(),original);
        for id in [&base.id,&source.id,&edited.id]{e.close_document(id).unwrap();}
    }}
}
