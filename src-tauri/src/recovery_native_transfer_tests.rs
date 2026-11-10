use super::*;
use crate::DocumentInfo;
#[path = "../tests/support/protected_fixture.rs"]
mod protected_fixture;
#[path = "recovery_copied_transfer_tests.rs"]
mod copied_transactions;

fn image_fixture(path: &Path) {
    let mut pdf = Document::load(path).unwrap();
    let page = pdf.get_pages()[&1];
    let image = pdf.add_object(Stream::new(dictionary! {"Type"=>"XObject","Subtype"=>"Image",
        "Width"=>2,"Height"=>2,"ColorSpace"=>"DeviceRGB","BitsPerComponent"=>8},
        vec![255,0,0,0,255,0,0,0,255,255,255,0]));
    let content = pdf.add_object(Stream::new(dictionary!{},b"q 80 0 0 40 200 320 cm /Photo Do Q".to_vec()));
    let dict = pdf.get_dictionary_mut(page).unwrap();
    dict.get_mut(b"Resources").unwrap().as_dict_mut().unwrap().set("XObject",dictionary!{"Photo"=>image});
    let original = dict.get(b"Contents").unwrap().clone();
    dict.set("Contents",vec![original,content.into()]);
    fs::write(path,crate::pdf_output::save(&mut pdf).unwrap()).unwrap();
}

fn form_fixture(path: &Path, field_first: bool) {
    text_fixture(path);
    let mut pdf = Document::load(path).unwrap();
    let page = pdf.get_pages()[&1];
    let resources = pdf
        .get_dictionary(page)
        .unwrap()
        .get(b"Resources")
        .unwrap()
        .clone();
    let mut fields = vec![];
    for (name, kind, y, value) in [("answer", "Tx", 40, "Alpha"), ("choice", "Ch", 100, "A")] {
        let ap = pdf.add_object(Stream::new(
            dictionary! {"Type"=>"XObject","Subtype"=>"Form",
            "BBox"=>vec![0.into(),0.into(),160.into(),24.into()],"Resources"=>resources.clone()},
            format!("/Tx BMC q BT /F1 12 Tf 0 g 2 6 Td ({value}) Tj ET Q EMC").into_bytes(),
        ));
        let mut widget = dictionary! {"Type"=>"Annot","Subtype"=>"Widget","FT"=>kind,"P"=>page,"F"=>4,
        "T"=>Object::string_literal(name),"TU"=>Object::string_literal(name),"V"=>Object::string_literal(value),
        "Rect"=>vec![20.into(),y.into(),180.into(),(y+24).into()],"DA"=>Object::string_literal("/F1 12 Tf 0 g"),
        "AP"=>dictionary!{"N"=>ap},"BS"=>dictionary!{"W"=>0,"S"=>"S"}};
        if kind == "Ch" {
            widget.set("Ff", 1 << 17);
            widget.set(
                "Opt",
                vec![Object::string_literal("A"), Object::string_literal("B")],
            );
        }
        fields.push(Object::Reference(pdf.add_object(widget)));
    }
    let mut annotations = pdf
        .get_dictionary(page)
        .unwrap()
        .get(b"Annots")
        .unwrap()
        .as_array()
        .unwrap()
        .clone();
    if field_first {
        let direct = pdf
            .get_dictionary(annotations[1].as_reference().unwrap())
            .unwrap()
            .clone();
        annotations[1] = direct.into();
        annotations.insert(0, fields[0].clone());
        annotations.push(fields[1].clone());
    } else {
        annotations.extend(fields.clone());
    }
    pdf.get_dictionary_mut(page)
        .unwrap()
        .set("Annots", annotations);
    pdf.catalog_mut().unwrap().set(
        "AcroForm",
        dictionary! {"Fields"=>fields,"DR"=>resources,
        "DA"=>Object::string_literal("/F1 12 Tf 0 g"),"NeedAppearances"=>false},
    );
    fs::write(path, crate::pdf_output::save(&mut pdf).unwrap()).unwrap();
}

fn comment_fixture(path: &Path, direct: bool) {
    let mut pdf = Document::load(path).unwrap();
    let page = pdf.get_pages()[&1];
    let note = pdf.new_object_id();
    let mut data = dictionary! {"Type"=>"Annot","Subtype"=>"Text","P"=>page,"F"=>28,
        "Rect"=>vec![240.into(),240.into(),258.into(),258.into()],"Name"=>"Note",
        "NM"=>Object::string_literal("same-nm"),"Contents"=>Object::string_literal("Original native note")};
    let mut added = vec![];
    if direct {
        added.push(data.into());
    } else {
        let popup = pdf.add_object(dictionary! {"Type"=>"Annot","Subtype"=>"Popup","P"=>page,
            "Parent"=>note,"Rect"=>vec![180.into(),200.into(),280.into(),300.into()]});
        data.set("Popup", popup);
        pdf.objects.insert(note, data.into());
        added.extend([Object::Reference(note), popup.into()]);
    }
    let mut annotations = pdf.get_dictionary(page).unwrap().get(b"Annots").unwrap().as_array().unwrap().clone();
    // Direct and indirect imported roots must both retain ownership when the
    // earlier native note and its popup disappear.
    annotations[1] = pdf.get_dictionary(annotations[1].as_reference().unwrap()).unwrap().clone().into();
    added.extend(annotations);
    pdf.get_dictionary_mut(page).unwrap().set("Annots", added);
    fs::write(path, crate::pdf_output::save(&mut pdf).unwrap()).unwrap();
}

fn shared_removal_fixture(path: &Path) {
    form_fixture(path, true);
    let mut pdf = Document::load(path).unwrap();
    let page = pdf.get_pages()[&1];
    let original_page = pdf.get_dictionary(page).unwrap().clone();
    let annots = original_page.get(b"Annots").unwrap().as_array().unwrap();
    let widget = annots[0].as_reference().unwrap();
    let owner = pdf.new_object_id();
    let next_page = pdf.new_object_id();
    let mut field = dictionary! {};
    for key in [b"FT".as_slice(), b"T", b"TU", b"V"] {
        let value = pdf.get_dictionary_mut(widget).unwrap().remove(key).unwrap();
        field.set(key.to_vec(), value);
    }
    field.set("DA", Object::string_literal("/F1 12 Tf 0 g"));
    pdf.get_dictionary_mut(widget).unwrap().set("Parent", owner);
    let mut second_widget = pdf.get_dictionary(widget).unwrap().clone();
    second_widget.set("P", next_page);
    let second_widget = pdf.add_object(second_widget);
    field.set("Kids", vec![Object::Reference(widget), second_widget.into()]);
    pdf.objects.insert(owner, field.into());
    let mut first = pdf.get_dictionary(annots[1].as_reference().unwrap()).unwrap().clone();
    first.set("P", next_page);
    let first = pdf.add_object(first);
    let mut second = annots[2].as_dict().unwrap().clone();
    second.set("P", next_page);
    let popup = pdf.add_object(dictionary! {"Type"=>"Annot","Subtype"=>"Popup","Parent"=>first,
        "P"=>next_page,"Rect"=>vec![180.into(),310.into(),280.into(),350.into()]});
    pdf.get_dictionary_mut(first).unwrap().set("Popup", popup);
    let mut next = original_page.clone();
    next.set("Annots", vec![Object::Reference(second_widget), first.into(), second.into(), popup.into()]);
    pdf.objects.insert(next_page, next.into());
    let pages = original_page.get(b"Parent").unwrap().as_reference().unwrap();
    let tree = pdf.get_dictionary_mut(pages).unwrap();
    tree.set("Kids", vec![Object::Reference(page), next_page.into()]);
    tree.set("Count", 2);
    pdf.catalog_mut().unwrap().get_mut(b"AcroForm").unwrap().as_dict_mut().unwrap()
        .get_mut(b"Fields").unwrap().as_array_mut().unwrap()[0] = owner.into();
    fs::write(path, crate::pdf_output::save(&mut pdf).unwrap()).unwrap();
}

fn choice_fixture(path: &Path) {
    // Arbitrary hand-positioned choice paint intentionally fails unchanged
    // regeneration. Start from the established producer appearance instead.
    let highlights = Document::load(path).unwrap();
    let hp = highlights.get_pages()[&1];
    let annotations = highlights
        .get_dictionary(hp)
        .unwrap()
        .get(b"Annots")
        .unwrap()
        .as_array()
        .unwrap();
    let mut pdf = Document::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/choice-text/combo-source.pdf"),
    )
    .unwrap();
    let page = pdf.get_pages()[&1];
    let mut kept = pdf
        .dereference(pdf.get_dictionary(page).unwrap().get(b"Annots").unwrap())
        .unwrap()
        .1
        .as_array()
        .unwrap()
        .clone();
    for (i, source) in annotations.iter().take(2).enumerate() {
        let mut h = highlights
            .dereference(source)
            .unwrap()
            .1
            .as_dict()
            .unwrap()
            .clone();
        let x = 20 + i as i64 * 100;
        h.set("P", page);
        h.set(
            "Rect",
            vec![x.into(), 40.into(), (x + 60).into(), 60.into()],
        );
        h.set(
            "QuadPoints",
            vec![
                x.into(),
                60.into(),
                (x + 60).into(),
                60.into(),
                x.into(),
                40.into(),
                (x + 60).into(),
                40.into(),
            ],
        );
        kept.push(pdf.add_object(h).into());
    }
    pdf.get_dictionary_mut(page).unwrap().set("Annots", kept);
    fs::write(path, crate::pdf_output::save(&mut pdf).unwrap()).unwrap();
}

fn apply(engine: &PdfEngine, source: &DocumentInfo, case: &str, temp: &Path) -> DocumentInfo {
    let fields = engine.list_form_fields(&source.id, 0).unwrap();
    let answer = fields
        .iter()
        .find(|f| f.name == "answer")
        .unwrap_or(&fields[0]);
    let index = answer.annotation_index;
    match case {
        "unlock"=>engine.unlock_document_sources(&[source.id.clone()],"owner").unwrap().remove(0).document,
        "image"=>{
            let image=engine.list_existing_images(&source.id,0).unwrap().images.remove(0);
            engine.preview_image_edit_occurrence(&source.id,&[0],0,&image.object_path,&image.selection_id,
                crate::ExistingImageEdit::Transform{transform:crate::ImageTransform{delta_x:8.5,delta_y:0.,scale_x:0.9,scale_y:1.,rotation_degrees:0.}}).unwrap().document
        },
        #[cfg(feature = "shaped-text")]
        "ocr"=>{
            let prepared=engine.prepare_ocr_page(&source.id,0,0).unwrap();
            let w=prepared.width as f32;let h=prepared.height as f32;
            let page=serde_json::from_value(json!({"width":prepared.width,"height":prepared.height,
                "blocks":[{"paragraphs":[{"lines":[{"words":[{"text":"Scanned phrase","confidence":95,
                "bounds":{"x":w*0.68,"y":h*0.11,"width":w*0.24,"height":h*0.035}}]}]}]}]})).unwrap();
            engine.apply_ocr(&source.id,vec![crate::OcrUpdate{page_index:0,rotation:0,preserve_owned_orientation:false,page}]).unwrap()
        },
        "session-chain"=>{
            let filled=apply(engine,source,"fill",temp);
            let noted=apply(engine,&filled,"comment-edit",temp);
            let deleted=apply(engine,&noted,"comment-delete",temp);
            engine.close_document(&filled.id).unwrap();
            engine.close_document(&noted.id).unwrap();
            deleted
        },
        "fill"=>engine.fill_form_field(&source.id,crate::FillFormFieldRequest{page_index:0,annotation_index:index,
            expected_value:answer.value.clone(),value:crate::FormFieldValue::Text{text:"Changed answer".into()}}).unwrap(),
        "comment-edit"=>{
            let note=engine.list_native_comments(&source.id).unwrap().remove(0);
            engine.edit_native_comment(&source.id,crate::EditNativeCommentRequest{page_index:note.page_index,
                annotation_index:note.annotation_index,expected_text:note.text,text:"Changed native note".into(),
                position:None,color:None}).unwrap()
        },
        "comment-delete"|"comment-direct-delete"=>{
            let note=engine.list_native_comments(&source.id).unwrap().remove(0);
            engine.delete_native_comment(&source.id,crate::DeleteNativeCommentRequest{page_index:note.page_index,
                annotation_index:note.annotation_index,expected_text:note.text}).unwrap()
        },
        "page"=>{
            let path=temp.join("incoming.pdf"); text_fixture(&path);
            let mut pdf=Document::load(&path).unwrap();let page=pdf.get_pages()[&1];
            let content=pdf.add_object(Stream::new(dictionary!{},b"BT /F1 12 Tf 30 200 Td (Replacement page) Tj ET".to_vec()));
            let dict=pdf.get_dictionary_mut(page).unwrap();dict.remove(b"Annots");dict.set("Contents",content);
            fs::write(&path,crate::pdf_output::save(&mut pdf).unwrap()).unwrap();
            let incoming=engine.open_document(&path).unwrap();
            let result=engine.replace_page_content(&source.id,0,&incoming.id,0).unwrap();
            engine.close_document(&incoming.id).unwrap();result
        },
        "properties"=>{
            let info=engine.inspect_form_properties(&source.id,0,index).unwrap();
            engine.set_form_properties(&source.id,serde_json::from_value(json!({"pageIndex":0,"annotationIndex":index,
                "expectedKey":info.expected_key,"changes":{"label":"Updated answer","required":true}})).unwrap()).unwrap()
        },
        "choices"=>{
            let index=fields.iter().find(|f|f.kind=="combo").unwrap().annotation_index;
            let info=engine.inspect_form_choices(&source.id,0,index).unwrap();
            engine.set_form_choices(&source.id,crate::SetFormChoicesRequest{page_index:0,annotation_index:index,expected_key:info.expected_key,
                edit:crate::ChoiceEdit{options:info.model.options.into_iter().enumerate().map(|(i,o)|
                    crate::ChoiceDraftOption{source_index:Some(i),display:if i==0 {format!("{} revised",o.display)}else{o.display},export_value:o.export_value}).collect(),
                    selection:crate::ChoiceEditSelection::Keep,defaults:crate::ChoiceEditSelection::Keep,manual_order:false}}).unwrap()
        },
        "position"=>{
            let mut bounds=answer.bounds.clone().unwrap();bounds.x+=10.;
            engine.position_form_field_occurrence(&source.id,&[0],0,serde_json::from_value(json!({"annotationIndex":index,
                "expectedBounds":answer.bounds,"expectedValue":answer.value,"bounds":bounds,"reflowText":false})).unwrap()).unwrap().document
        },
        "frame"=>{
            let style=engine.inspect_form_frame(&source.id,0,index).unwrap();
            engine.set_form_frame_occurrence(&source.id,&[0],0,serde_json::from_value(json!({"annotationIndex":index,
                "expectedBounds":answer.bounds,"expectedValue":answer.value,"expectedStyleKey":style.style_key,
                "changes":{"width":1.5,"border":{"mode":"rgb","color":"#0033CC"}}})).unwrap()).unwrap().document
        },
        "text-style"=>{
            let style=engine.inspect_form_text_appearance(&source.id,0,index).unwrap();
            engine.set_form_text_appearance_occurrence(&source.id,&[0],0,serde_json::from_value(json!({"annotationIndex":index,
                "expectedBounds":answer.bounds,"expectedValue":answer.value,"expectedStyleKey":style.style_key,
                "changes":{"color":"#993366"}})).unwrap()).unwrap().document
        },
        "create"=>engine.create_form_field_occurrence(&source.id,&[0],0,serde_json::from_value(json!({
            "bounds":{"x":340,"y":570,"width":140,"height":30},"rotation":0,"name":"added","label":"New answer",
            "required":false,"fontSize":12,"field":{"kind":"text","multiline":false}})).unwrap()).unwrap().document,
        "remove"|"remove-shift"|"remove-shared"=>{
            let info=engine.inspect_form_removal(&source.id,0,index).unwrap();
            engine.remove_form_field(&source.id,crate::RemoveFormFieldRequest{page_index:0,annotation_index:index,expected_key:info.expected_key}).unwrap()
        },
        _=>unreachable!(),
    }
}

fn check_native_transaction(case: &str) {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    form_fixture(&path, case == "remove-shift");
    if case == "remove-shared" {
        shared_removal_fixture(&path);
    }
    if case == "choices" {
        choice_fixture(&path);
    }
    if case.starts_with("comment-") || case == "session-chain" {
        comment_fixture(&path, case == "comment-direct-delete");
    }
    if matches!(case,"image"|"ocr") { image_fixture(&path); }
    if case == "unlock" {
        let bytes=protected_fixture::encrypt(&fs::read(&path).unwrap(),lopdf::encryption::Permissions::all());
        fs::write(&path,bytes).unwrap();
    }
    let original_bytes = fs::read(&path).unwrap();
    let engine = PdfEngine::start(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )
    .unwrap();
    let source = if case == "unlock" { engine.open_document_with_password(&path,"reader").unwrap() }
        else { engine.open_document(&path).unwrap() };
    let ownership =
        serde_json::to_value(engine.source_imported_overlay_snapshot(&source.id).unwrap()).unwrap();
    assert_eq!(source.pages[0].overlays.len(), 2);
    assert_eq!(ownership["bindings"].as_array().unwrap().len(), if case == "remove-shared" { 4 } else { 2 });
    if case == "remove-shared" {
        assert_eq!(engine.list_form_fields(&source.id, 1).unwrap().len(), 1);
        assert!(ownership["bindings"].as_array().unwrap().iter().any(|b|
            !b["popupIndices"].as_array().unwrap().is_empty()));
    }
    let edited = apply(&engine, &source, case, temp.path());
    let next =
        serde_json::to_value(engine.source_imported_overlay_snapshot(&edited.id).unwrap()).unwrap();
    let mut expected = ownership["bindings"].clone();
    let removed = match case {
        "remove-shift" | "remove-shared" | "comment-direct-delete" => 1,
        "comment-delete" | "session-chain" => 2,
        _ => 0,
    };
    if removed > 0 {
        for binding in expected.as_array_mut().unwrap() {
            binding["annotationIndex"] = json!(binding["annotationIndex"].as_u64().unwrap() - removed);
            for slot in binding["popupIndices"].as_array_mut().unwrap() {
                *slot = json!(slot.as_u64().unwrap() - removed);
            }
        }
    }
    assert_eq!(
        next["bindings"], expected,
        "{case} must preserve proved ownership, including direct slots after a removed widget"
    );
    if case == "unlock" {
        assert_eq!(next["sourceSha256"],ownership["sourceSha256"]);
        assert!(!source.protection.as_ref().unwrap().owner);
        assert!(edited.protection.as_ref().unwrap().owner);
    } else { assert_ne!(next["sourceSha256"], ownership["sourceSha256"]); }
    if matches!(case, "fill" | "session-chain") || case.starts_with("comment-") {
        assert!(edited.pages.iter().all(|page| page.overlays.is_empty()), "session drafts must not be replaced");
    } else { assert_eq!(
        serde_json::to_value(&edited.pages[0].overlays).unwrap(),
        serde_json::to_value(&source.pages[0].overlays).unwrap()
    ); }
    if case.starts_with("comment-") || case == "session-chain" {
        let notes = engine.list_native_comments(&edited.id).unwrap();
        if case == "comment-edit" { assert_eq!(notes[0].text, "Changed native note"); }
        else { assert!(notes.is_empty()); }
    }
    // Keep a changed note and a deletion through the new native source and recovery.
    let mut drafts = serde_json::to_value(&source.pages[0].overlays).unwrap();
    drafts.as_array_mut().unwrap().pop();
    drafts[0]["text"] = json!("Keep this draft after a native operation");
    let mut workspace = json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","dirty":true,"savedDigest":"","zoom":100,
        "scrollPosition":{"top":0,"left":0},"document":{"name":"Edited.pdf","pages":[{"id":"page","sourceId":edited.id,
        "pageIndex":0,"width":edited.pages[0].width,"height":edited.pages[0].height,"rotation":0,"overlays":drafts}],
        "selectedPageId":"page","selectedOverlayId":null}}]});
    if case == "remove-shared" {
        let mut drafts = serde_json::to_value(&source.pages[1].overlays).unwrap();
        drafts.as_array_mut().unwrap().pop();
        drafts[0]["text"] = json!("Second-page draft with a native popup");
        workspace["tabs"][0]["document"]["pages"].as_array_mut().unwrap().push(json!({
            "id":"second-page","sourceId":edited.id,"pageIndex":1,"width":edited.pages[1].width,
            "height":edited.pages[1].height,"rotation":0,"overlays":drafts}));
    }
    let edited_bytes = engine.source_bytes(&edited.id).unwrap();
    let form_state = serde_json::to_value(engine.list_form_fields(&edited.id, 0).unwrap()).unwrap();
    let comment_state = serde_json::to_value(engine.list_native_comments(&edited.id).unwrap()).unwrap();
    if matches!(case, "fill" | "session-chain") {
        assert_eq!(form_state.as_array().unwrap().iter().find(|f| f["name"] == "answer").unwrap()["value"]["text"], "Changed answer");
    }
    let store = RecoveryStore::new(temp.path().join("recovery")).unwrap();
    store.save(&engine, workspace.clone()).unwrap();
    engine.close_document(&edited.id).unwrap();
    let recovered = store.load(&engine).unwrap().unwrap();
    assert_eq!(
        recovered["tabs"][0]["document"]["pages"][0]["overlays"],
        workspace["tabs"][0]["document"]["pages"][0]["overlays"]
    );
    assert_eq!(
        serde_json::to_value(
            engine
                .source_imported_overlay_snapshot(source_id(&recovered))
                .unwrap()
        )
        .unwrap(),
        next
    );
    assert_eq!(
        engine.source_bytes(source_id(&recovered)).unwrap(),
        edited_bytes
    );
    assert_eq!(
        serde_json::to_value(engine.list_form_fields(source_id(&recovered), 0).unwrap()).unwrap(),
        form_state
    );
    assert_eq!(fs::read(path).unwrap(), original_bytes);
    if case == "ocr" {
        let text=engine.page_text(source_id(&recovered),0).unwrap().characters.into_iter().map(|c|c.text).collect::<String>();
        assert!(text.contains("Scanned phrase"),"{text}");
    }
    if case == "image" {
        assert_eq!(engine.list_existing_images(source_id(&recovered),0).unwrap().images.len(),1);
        assert_ne!(engine.render_page(&source.id,0,600).unwrap(),engine.render_page(source_id(&recovered),0,600).unwrap());
    }
    assert_eq!(serde_json::to_value(engine.list_native_comments(source_id(&recovered)).unwrap()).unwrap(), comment_state);
    if case == "remove-shared" {
        assert!(engine.list_form_fields(source_id(&recovered), 1).unwrap().is_empty());
        assert_eq!(recovered["tabs"][0]["document"]["pages"][1]["overlays"],
            workspace["tabs"][0]["document"]["pages"][1]["overlays"]);
        assert_eq!(serde_json::to_value(&edited.pages[1].overlays).unwrap(),
            serde_json::to_value(&source.pages[1].overlays).unwrap());
    }
    engine.close_document(source_id(&recovered)).unwrap();
    engine.close_document(&source.id).unwrap();
}

#[test]
fn recovery_annotation_sources_page_replacement() {
    check_native_transaction("page");
}
#[test]
fn recovery_annotation_sources_form_properties() {
    check_native_transaction("properties");
}
#[test]
fn recovery_annotation_sources_form_choices() {
    check_native_transaction("choices");
}
#[test]
fn recovery_annotation_sources_form_position() {
    check_native_transaction("position");
}
#[test]
fn recovery_annotation_sources_form_frame() {
    check_native_transaction("frame");
}
#[test]
fn recovery_annotation_sources_form_text_style() {
    check_native_transaction("text-style");
}
#[test]
fn recovery_annotation_sources_form_creation() {
    check_native_transaction("create");
}
#[test]
fn recovery_annotation_sources_form_removal() {
    check_native_transaction("remove");
}
#[test]
fn recovery_annotation_sources_form_removal_reindexes_direct_annotations() {
    check_native_transaction("remove-shift");
}
#[test]
fn recovery_annotation_sources_shared_form_removal_reindexes_popups_and_direct_annotations() {
    check_native_transaction("remove-shared");
}
#[test]
fn recovery_annotation_sources_form_answer_keeps_owned_session_drafts() { check_native_transaction("fill"); }
#[test]
fn recovery_annotation_sources_native_comment_edit_keeps_owned_session_drafts() { check_native_transaction("comment-edit"); }
#[test]
fn recovery_annotation_sources_native_comment_deletion_reindexes_owned_roots() { check_native_transaction("comment-delete"); }
#[test]
fn recovery_annotation_sources_direct_native_comment_deletion_reindexes_owned_roots() { check_native_transaction("comment-direct-delete"); }
#[test]
fn recovery_annotation_sources_consecutive_fill_comment_and_deletion_keep_ownership() { check_native_transaction("session-chain"); }
#[test]
#[cfg(feature = "shaped-text")]
fn recovery_annotation_sources_ocr_preserves_imported_highlights() { check_native_transaction("ocr"); }
#[test]
fn recovery_annotation_sources_owner_unlock_preserves_imported_highlights() { check_native_transaction("unlock"); }
#[test]
fn recovery_annotation_sources_existing_image_edit_preserves_imported_highlights() { check_native_transaction("image"); }

#[test]
fn recovery_annotation_sources_owner_unlock_keeps_independent_embedded_image_lease() {
    owner_image_lease_case(false);
}
#[test]
fn recovery_annotation_sources_owner_unlock_keeps_recovered_draft_image_lease() {
    owner_image_lease_case(true);
}
fn owner_image_lease_case(recovered_draft: bool) {
    let temp=tempfile::tempdir().unwrap();
    let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
    let mut png=std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(4,2,image::Rgba([211,30,80,180])))
        .write_to(&mut png,image::ImageFormat::Png).unwrap();
    let image_path=temp.path().join("photo.png");fs::write(&image_path,png.into_inner()).unwrap();
    let image=engine.import_image(&image_path).unwrap();
    let blank=engine.create_blank_document(200.,300.).unwrap();
    let plain=temp.path().join("plain.pdf");
    engine.export_pdf(serde_json::from_value(json!({"pages":[{"id":"page","sourceId":blank.id,"pageIndex":0,
        "width":200,"height":300,"rotation":0,"overlays":[{"id":"photo","type":"image","assetId":image.image.id,
        "x":40,"y":60,"width":100,"height":50,"rotation":0}]}],"flatten":false})).unwrap(),&plain).unwrap();
    engine.close_document(&blank.id).unwrap();
    engine.images().release(&image.token).unwrap();
    assert!(engine.images().get(&image.image.id).is_err());
    let bytes=protected_fixture::encrypt(&fs::read(&plain).unwrap(),lopdf::encryption::Permissions::all());
    let path=temp.path().join("protected.pdf");fs::write(&path,&bytes).unwrap();
    let mut reader=engine.open_document_with_password(&path,"reader").unwrap();
    assert_eq!(reader.pages[0].overlays.len(),1);
    let ownership=engine.source_imported_overlay_snapshot(&reader.id).unwrap();
    let store=RecoveryStore::new(temp.path().join("recovery")).unwrap();
    let mut drafts=serde_json::to_value(&reader.pages[0].overlays).unwrap();
    let mut extra_id=None;
    if recovered_draft {
        let mut png=std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(3,3,image::Rgba([17,220,30,255])))
            .write_to(&mut png,image::ImageFormat::Png).unwrap();
        let extra_path=temp.path().join("draft.png");fs::write(&extra_path,png.into_inner()).unwrap();
        let extra=engine.import_image(&extra_path).unwrap();
        drafts.as_array_mut().unwrap().push(json!({"id":"draft-photo","type":"image","assetId":extra.image.id,
            "x":80,"y":160,"width":70,"height":70,"rotation":0}));
        let workspace=json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","dirty":true,"savedDigest":"","zoom":100,
            "scrollPosition":{"top":0,"left":0},"document":{"name":"Protected.pdf","pages":[{"id":"page",
            "sourceId":reader.id,"pageIndex":0,"width":200,"height":300,"rotation":0,"overlays":drafts}],
            "selectedPageId":"page","selectedOverlayId":null}}]});
        store.save(&engine,workspace).unwrap();
        engine.close_document(&reader.id).unwrap();engine.images().release(&extra.token).unwrap();
        fs::remove_file(&extra_path).unwrap();
        assert!(engine.images().get(&extra.image.id).is_err());
        let restored=store.load(&engine).unwrap().unwrap();reader.id=source_id(&restored).into();
        extra_id=Some(extra.image.id);
    }
    let owner=engine.unlock_document_sources(&[reader.id.clone()],"owner").unwrap().remove(0).document;
    engine.close_document(&reader.id).unwrap();
    assert!(engine.images().get(&image.image.id).is_ok(),"owner replacement must retain its own embedded-image lease");
    if let Some(id)=&extra_id { assert!(engine.images().get(id).is_ok(),"recovered draft image must survive old source close"); }
    assert_eq!(engine.source_imported_overlay_snapshot(&owner.id).unwrap(),ownership);
    let page=json!({"id":"page","sourceId":owner.id,"pageIndex":0,"width":200,"height":300,"rotation":0,"overlays":drafts});
    let rendered=engine.render_page_plan(serde_json::from_value(page.clone()).unwrap(),400).unwrap();
    let workspace=json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","dirty":true,"savedDigest":"","zoom":100,
        "scrollPosition":{"top":0,"left":0},"document":{"name":"Protected.pdf","pages":[page],"selectedPageId":"page","selectedOverlayId":null}}]});
    store.save(&engine,workspace).unwrap();engine.close_document(&owner.id).unwrap();
    assert!(engine.images().get(&image.image.id).is_err());
    if let Some(id)=&extra_id { assert!(engine.images().get(id).is_err()); }
    let recovered=store.load(&engine).unwrap().unwrap();
    let page:crate::PagePlan=serde_json::from_value(recovered["tabs"][0]["document"]["pages"][0].clone()).unwrap();
    assert_eq!(engine.render_page_plan(page,400).unwrap(),rendered);
    assert_eq!(engine.source_imported_overlay_snapshot(source_id(&recovered)).unwrap(),ownership);
    assert_eq!(fs::read(&path).unwrap(),bytes);
    engine.close_document(source_id(&recovered)).unwrap();
    assert!(engine.images().get(&image.image.id).is_err());
    if let Some(id)=&extra_id { assert!(engine.images().get(id).is_err()); }
}

#[test]
fn recovery_annotation_sources_legacy_session_edits_do_not_invent_ownership() {
    for case in ["fill", "comment-edit", "comment-delete", "comment-direct-delete"] {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("legacy.pdf");
        form_fixture(&path, false);
        if case.starts_with("comment-") { comment_fixture(&path, case == "comment-direct-delete"); }
        let original_bytes = fs::read(&path).unwrap();
        let engine = PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
        let mut original = engine.open_document(&path).unwrap();
        let mut drafts = serde_json::to_value(&original.pages[0].overlays).unwrap();
        drafts.as_array_mut().unwrap().pop();
        drafts[0]["text"] = json!("Keep the legacy draft");
        let workspace = json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","dirty":true,"savedDigest":"",
            "zoom":100,"scrollPosition":{"top":0,"left":0},"document":{"name":"Legacy.pdf",
            "pages":[{"id":"page","sourceId":original.id,"pageIndex":0,"width":original.pages[0].width,
                "height":original.pages[0].height,"rotation":0,"overlays":drafts}],"selectedPageId":"page","selectedOverlayId":null}}]});
        let store = RecoveryStore::new(temp.path().join("recovery")).unwrap();
        store.save(&engine,workspace).unwrap();
        let (manifest_path,mut manifest)=latest(&store);
        manifest["annotationImportVersion"]=json!(3);
        manifest["sources"][&original.id].as_object_mut().unwrap().remove("annotationSources");
        private_write(&manifest_path,Purpose::Workspace,&serde_json::to_vec(&manifest).unwrap()).unwrap();
        engine.close_document(&original.id).unwrap();
        let mut restored=store.load(&engine).unwrap().unwrap();
        original.id=source_id(&restored).into();
        assert!(engine.source_imported_overlay_snapshot(&original.id).unwrap().is_none());
        let edited=apply(&engine,&original,case,temp.path());
        assert!(edited.pages.iter().all(|p|p.overlays.is_empty()));
        assert!(engine.source_imported_overlay_snapshot(&edited.id).unwrap().is_none(),"{case}");
        let bytes=engine.source_bytes(&edited.id).unwrap();
        restored["tabs"][0]["document"]["pages"][0]["sourceId"]=json!(edited.id);
        store.save(&engine,restored).unwrap();
        engine.close_document(&edited.id).unwrap();
        engine.close_document(&original.id).unwrap();
        let recovered=store.load(&engine).unwrap().unwrap();
        assert!(engine.source_imported_overlay_snapshot(source_id(&recovered)).unwrap().is_none());
        assert_eq!(recovered["tabs"][0]["document"]["pages"][0]["overlays"],drafts);
        assert_eq!(engine.source_bytes(source_id(&recovered)).unwrap(),bytes);
        assert_eq!(fs::read(&path).unwrap(),original_bytes);
        engine.close_document(source_id(&recovered)).unwrap();
    }
}
