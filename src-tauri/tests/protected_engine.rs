use folio_engine::protection::{unlock_pdf, ProtectionError};
use folio_engine::{DocumentInfo, EngineError, ExportRequest, PagePlan, PdfEngine, RecoveryStore};
use serde_json::{json, Value};
use std::{fs, path::PathBuf};
#[path = "support/protected_fixture.rs"]
mod protected_fixture;
use lopdf::encryption::Permissions;

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}
fn fixture(name: &str) -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../tests/fixtures/protection/{name}.pdf")),
    )
    .unwrap()
}
fn pages(info: &DocumentInfo) -> Vec<PagePlan> {
    info.pages
        .iter()
        .enumerate()
        .map(|(i, p)| {
            serde_json::from_value(json!({
        "id":format!("page-{i}"),"sourceId":info.id,"pageIndex":i,"width":p.width,"height":p.height,
        "rotation":0,"overlays":p.overlays
    })).unwrap()
        })
        .collect()
}
fn workspace(pages: &[PagePlan]) -> Value {
    json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","dirty":true,"savedDigest":"old","zoom":100,
        "scrollPosition":{"top":0,"left":0},"document":{"name":"Protected.pdf","pages":pages,"selectedPageId":"page-0","selectedOverlayId":null}}]})
}

#[test]
fn crop_requires_native_edit_permission_even_when_ui_claims_owner() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("protected.pdf");
    fs::write(&path,fixture("aes_256")).unwrap();
    let reader=engine.open_document_with_password(&path,"reader").unwrap();
    let owner=engine.open_document_with_password(&path,"owner").unwrap();
    let mut page=serde_json::to_value(&pages(&reader)[0]).unwrap();
    page["crop"]=json!({"x":0,"y":0,"width":100,"height":100});
    page["protection"]=json!({"owner":true,"permissions":{"editContent":true}});
    let plan:PagePlan=serde_json::from_value(page).unwrap();
    let denied=temp.path().join("denied.pdf");
    assert!(matches!(engine.export_pdf(ExportRequest{pages:vec![plan.clone()],flatten:false},&denied),Err(EngineError::PermissionDenied(_))));
    assert!(!denied.exists());
    assert!(matches!(engine.render_page_plan(plan.clone(),200),Err(EngineError::PermissionDenied(_))));
    let recovery=RecoveryStore::new(temp.path().join("crop-recovery")).unwrap();
    assert!(recovery.save(&engine,workspace(&[plan.clone()])).is_err(),"Recovery cannot authorize a forbidden crop");
    let allowed=PagePlan{source_id:owner.id.clone(),..plan};
    let output=temp.path().join("owner-crop.pdf");engine.export_pdf(ExportRequest{pages:vec![allowed],flatten:false},&output).unwrap();
    let reopened=engine.open_document_with_password(&output,"owner").unwrap();
    assert_eq!((reopened.pages[0].width,reopened.pages[0].height),(100.,100.));
    for id in [&reader.id,&owner.id,&reopened.id]{engine.close_document(id).unwrap();}
}

#[test]
fn native_password_challenges_and_empty_user_inputs_retain_protection() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for name in [
        "rc4_40",
        "rc4_128",
        "aes_128",
        "aes_256_r5",
        "aes_256",
        "empty_user",
    ] {
        let path = temp.path().join(format!("{name}.pdf"));
        let original = fixture(name);
        fs::write(&path, &original).unwrap();
        if name != "empty_user" {
            assert!(matches!(
                e.open_document(&path),
                Err(EngineError::Protection(ProtectionError::PasswordRequired))
            ));
        }
        assert!(matches!(
            e.open_document_with_password(&path, "wrong"),
            Err(EngineError::Protection(ProtectionError::IncorrectPassword))
        ));
        let user = if name == "empty_user" {
            e.open_document(&path).unwrap()
        } else {
            e.open_document_with_password(&path, "reader").unwrap()
        };
        let policy = user.protection.as_ref().unwrap();
        assert!(!policy.owner && !policy.permissions.edit_content);
        assert!(e.render_page(&user.id, 0, 360).is_ok());
        assert!(matches!(
            e.page_text(&user.id, 0),
            Err(EngineError::PermissionDenied(_))
        ));
        assert!(matches!(
            e.list_text_runs(&user.id, 0),
            Err(EngineError::PermissionDenied(_))
        ));
        let output = temp.path().join(format!("copy-{name}.pdf"));
        e.export_pdf(
            ExportRequest {
                pages: pages(&user),
                flatten: false,
            },
            &output,
        )
        .unwrap();
        let saved = fs::read(&output).unwrap();
        protected_fixture::record(name,&original,&saved);
        assert!(unlock_pdf(
            &saved,
            Some(if name == "empty_user" { "" } else { "reader" })
        )
        .is_ok());
        assert!(unlock_pdf(&saved, Some("owner"))
            .unwrap()
            .protection
            .is_some());
        assert_eq!(fs::read(&path).unwrap(), original);
        e.close_document(&user.id).unwrap();
    }
}

#[test]
fn annotation_rights_do_not_grant_flattening_rotation_or_bookmark_changes() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    fs::write(&path, fixture("aes_256")).unwrap();
    let source = e.open_document_with_password(&path, "reader").unwrap();
    let mut plan = pages(&source);
    plan[0].overlays.push(serde_json::from_value(json!({"type":"text","id":"note","text":"Allowed annotation","x":30,"y":230,"fontSize":12,"color":"#123456"})).unwrap());
    let output = temp.path().join("copy.pdf");
    e.export_pdf(
        ExportRequest {
            pages: plan.clone(),
            flatten: false,
        },
        &output,
    )
    .unwrap();
    let before = fs::read(&output).unwrap();
    protected_fixture::record("annotated",&fixture("aes_256"),&before);
    assert!(matches!(
        e.export_pdf(
            ExportRequest {
                pages: plan.clone(),
                flatten: true
            },
            &output
        ),
        Err(EngineError::PermissionDenied(_))
    ));
    plan[0].rotation = 90;
    assert!(matches!(
        e.export_pdf(
            ExportRequest {
                pages: plan.clone(),
                flatten: false
            },
            &output
        ),
        Err(EngineError::PermissionDenied(_))
    ));
    plan[0].rotation = 0;
    let bookmarks =
        serde_json::from_value(json!([{"id":"b","title":"Unauthorized","level":0}])).unwrap();
    assert!(matches!(
        e.export_pdf_with_bookmarks(
            ExportRequest {
                pages: plan,
                flatten: false
            },
            Some(bookmarks),
            &output
        ),
        Err(EngineError::PermissionDenied(_))
    ));
    assert_eq!(fs::read(output).unwrap(), before);
    e.close_document(&source.id).unwrap();
}

#[test]
fn owner_edits_and_private_recovery_retain_both_original_credentials() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    let original = fixture("aes_128");
    fs::write(&path, &original).unwrap();
    let source = e.open_document_with_password(&path, "owner").unwrap();
    assert!(source.protection.as_ref().unwrap().owner);
    let run = e
        .list_text_runs(&source.id, 0)
        .unwrap()
        .runs
        .into_iter()
        .find(|r| r.text.contains("Protected Folio fixture"))
        .unwrap();
    let edited = e
        .replace_text(&source.id, 0, run.object_index, &run.text, "Edited Folio")
        .unwrap();
    assert!(edited.protection.as_ref().unwrap().owner);
    let store = RecoveryStore::new(temp.path().join("recovery")).unwrap();
    store.save(&e, workspace(&pages(&edited))).unwrap();
    e.close_document(&edited.id).unwrap();
    e.close_document(&source.id).unwrap();
    fs::remove_file(&path).unwrap();
    let restored = store.load(&e).unwrap().unwrap();
    let plan: Vec<PagePlan> =
        serde_json::from_value(restored["tabs"][0]["document"]["pages"].clone()).unwrap();
    assert!(e.page_text(&plan[0].source_id, 0).is_ok());
    let output = temp.path().join("recovered.pdf");
    e.export_pdf(
        ExportRequest {
            pages: plan.clone(),
            flatten: false,
        },
        &output,
    )
    .unwrap();
    for password in ["reader", "owner"] {
        let unlocked = unlock_pdf(&fs::read(&output).unwrap(), Some(password)).unwrap();
        assert!(unlocked.protection.is_some());
        assert!(lopdf::Document::load_mem(&unlocked.bytes)
            .unwrap()
            .extract_text(&[1])
            .unwrap()
            .contains("Edited Folio"));
    }
    protected_fixture::record("recovered-edit",&original,&fs::read(&output).unwrap());
    e.close_document(&plan[0].source_id).unwrap();
}

#[test]
fn editing_one_page_preserves_original_page_and_bookmark_provenance() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("plain.pdf");
    fs::write(&path, fixture("plain")).unwrap();
    let plain = e.open_document(&path).unwrap();
    let mut plan = pages(&plain);
    let mut second = plan[0].clone();
    second.id = "page-1".into();
    plan.push(second);
    let composed = temp.path().join("two.pdf");
    let bookmarks=serde_json::from_value(json!([{"id":"b","title":"First page","level":0,"destination":{"kind":"array","value":[{"kind":"page","value":"page-0"},{"kind":"name","value":[70,105,116]}]}}])).unwrap();
    e.export_pdf_with_bookmarks(
        ExportRequest {
            pages: plan,
            flatten: false,
        },
        Some(bookmarks),
        &composed,
    )
    .unwrap();
    let protected = temp.path().join("protected.pdf");
    fs::write(
        &protected,
        protected_fixture::encrypt(&fs::read(composed).unwrap(), Permissions::MODIFIABLE),
    )
    .unwrap();
    let source = e.open_document_with_password(&protected, "reader").unwrap();
    let run = e
        .list_text_runs(&source.id, 0)
        .unwrap()
        .runs
        .into_iter()
        .find(|r| r.text.contains("Protected Folio fixture"))
        .unwrap();
    let edited = e
        .replace_text(&source.id, 0, run.object_index, &run.text, "Edited Folio")
        .unwrap();
    assert!(!edited.protection.as_ref().unwrap().permissions.assemble);
    assert!(matches!(
        e.extract_text(&edited.id, 0),
        Err(EngineError::PermissionDenied(_))
    ));
    let mut plan = pages(&source);
    plan[0].source_id = edited.id.clone();
    let output = temp.path().join("edited.pdf");
    e.export_pdf(
        ExportRequest {
            pages: plan.clone(),
            flatten: false,
        },
        &output,
    )
    .unwrap();
    let saved = fs::read(&output).unwrap();
    protected_fixture::record("edited-bookmark",&fs::read(&protected).unwrap(),&saved);
    let unlocked = unlock_pdf(&saved, Some("owner")).unwrap();
    assert_eq!(
        folio_engine::bookmarks::read(&unlocked.bytes)
            .unwrap()
            .len(),
        1
    );
    assert!(lopdf::Document::load_mem(&unlocked.bytes)
        .unwrap()
        .extract_text(&[1])
        .unwrap()
        .contains("Edited Folio"));
    let mut variants = vec![
        vec![plan[0].clone()],
        vec![plan[1].clone(), plan[0].clone()],
    ];
    let mut duplicate = plan.clone();
    duplicate[1].source_id = edited.id.clone();
    duplicate[1].page_index = 0;
    variants.push(duplicate);
    for pages in variants {
        assert!(matches!(
            e.export_pdf(
                ExportRequest {
                    pages,
                    flatten: false
                },
                &output
            ),
            Err(EngineError::PermissionDenied(_))
        ));
    }
    assert_eq!(fs::read(output).unwrap(), saved);
    for id in [&plain.id, &source.id, &edited.id] {
        e.close_document(id).unwrap();
    }
}

#[test]
fn combining_sources_requires_compatible_protection_and_each_sources_rights() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join("first.pdf");
    let second = temp.path().join("second.pdf");
    fs::write(
        &first,
        protected_fixture::encrypt(&fixture("plain"), Permissions::all()),
    )
    .unwrap();
    fs::write(
        &second,
        protected_fixture::encrypt(&fixture("plain"), Permissions::all()),
    )
    .unwrap();
    let user = e.open_document_with_password(&first, "reader").unwrap();
    let owner = e.open_document_with_password(&first, "owner").unwrap();
    let other = e.open_document_with_password(&second, "owner").unwrap();
    e.check_source_combination(&[user.id.clone(),owner.id.clone()]).unwrap();
    assert!(e.check_source_combination(&[owner.id.clone(),other.id.clone()]).is_err());
    let mut plan = pages(&user);
    let mut second = pages(&owner).remove(0);
    second.id = "page-1".into();
    plan.push(second);
    let output = temp.path().join("combined.pdf");
    e.export_pdf(
        ExportRequest {
            pages: plan.clone(),
            flatten: false,
        },
        &output,
    )
    .unwrap();
    let before = fs::read(&output).unwrap();
    assert_eq!(
        lopdf::Document::load_mem(&unlock_pdf(&before, Some("reader")).unwrap().bytes)
            .unwrap()
            .get_pages()
            .len(),
        2
    );
    plan[1].source_id = other.id.clone();
    assert!(
        matches!(e.export_pdf(ExportRequest{pages:plan,flatten:false},&output),Err(EngineError::InvalidRequest(message)) if message.contains("different protection"))
    );
    assert_eq!(fs::read(&output).unwrap(), before);
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 3);
    for id in [&user.id, &owner.id, &other.id] {
        e.close_document(id).unwrap();
    }
}

#[test]
fn content_editing_does_not_grant_annotation_or_copy_rights() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    fs::write(&path, fixture("edit_only_aes_256")).unwrap();
    let source = e.open_document_with_password(&path, "reader").unwrap();
    assert!(e.list_text_runs(&source.id, 0).is_ok());
    assert!(matches!(
        e.page_text(&source.id, 0),
        Err(EngineError::PermissionDenied(_))
    ));
    let mut plan = pages(&source);
    assert!(e.render_page_plan(plan[0].clone(), 360).is_ok());
    plan[0].overlays.push(serde_json::from_value(json!({"type":"comment","id":"note","text":"Not allowed","x":20,"y":200,"color":"#ff8800"})).unwrap());
    assert!(matches!(
        e.render_page_plan(plan[0].clone(), 360),
        Err(EngineError::PermissionDenied(_))
    ));
    let output = temp.path().join("not-created.pdf");
    assert!(matches!(
        e.export_pdf(
            ExportRequest {
                pages: plan,
                flatten: false
            },
            &output
        ),
        Err(EngineError::PermissionDenied(_))
    ));
    assert!(!output.exists());
    e.close_document(&source.id).unwrap();
}

#[test]
fn recovery_rejects_absent_corrupt_or_wrong_source_context_and_ignores_ui_privileges() {
    use folio_engine::private_storage::{seal, unseal, Purpose};
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    fs::write(&path, fixture("aes_256")).unwrap();
    let source = e.open_document_with_password(&path, "reader").unwrap();
    let root = temp.path().join("recovery");
    let store = RecoveryStore::new(root.clone()).unwrap();
    let mut state = workspace(&pages(&source));
    state["tabs"][0]["document"]["pages"][0]["protection"] =
        json!({"owner":true,"permissions":{"editContent":true}});
    store.save(&e, state).unwrap();
    e.close_document(&source.id).unwrap();
    let checkpoint = fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|e| e == "json"))
        .unwrap();
    let original = fs::read(&checkpoint).unwrap();
    let decoded = unseal(Purpose::Workspace, &original, 32 * 1024 * 1024).unwrap();
    let manifest: Value = serde_json::from_slice(&decoded).unwrap();
    assert_eq!(manifest["version"], 4);
    let restored = store.load(&e).unwrap().unwrap();
    let page = &restored["tabs"][0]["document"]["pages"][0];
    assert_eq!(page["protection"]["owner"], false);
    let id = page["sourceId"].as_str().unwrap();
    assert!(matches!(
        e.list_text_runs(id, 0),
        Err(EngineError::PermissionDenied(_))
    ));
    e.close_document(id).unwrap();
    for mode in ["missing", "corrupt", "wrong-source"] {
        let mut invalid = manifest.clone();
        let snapshot = invalid["sources"]
            .as_object_mut()
            .unwrap()
            .values_mut()
            .next()
            .unwrap();
        if mode == "missing" {
            snapshot.as_object_mut().unwrap().remove("policy");
        } else if mode == "corrupt" {
            snapshot["policy"]["context"][0] = json!(0);
        } else {
            let unrelated = unlock_pdf(&fixture("aes_256"), Some("reader")).unwrap();
            let mut changed = unrelated.bytes;
            changed.push(b' ');
            snapshot["policy"]["context"] = json!(unrelated
                .protection
                .unwrap()
                .seal_recovery_context(&changed)
                .unwrap());
        }
        fs::write(
            &checkpoint,
            seal(Purpose::Workspace, &serde_json::to_vec(&invalid).unwrap()).unwrap(),
        )
        .unwrap();
        assert!(store.load(&e).is_err(), "{mode}");
    }
    fs::write(checkpoint, original).unwrap();
}

#[test]
fn legacy_empty_password_recovery_is_upgraded_without_downgrading_protection() {
    use folio_engine::private_storage::{seal, unseal, Purpose};
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    let original = fixture("empty_user");
    fs::write(&path, &original).unwrap();
    let source = e.open_document(&path).unwrap();
    let root = temp.path().join("recovery");
    let store = RecoveryStore::new(root.clone()).unwrap();
    store.save(&e, workspace(&pages(&source))).unwrap();
    e.close_document(&source.id).unwrap();
    let checkpoint = fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|e| e == "json"))
        .unwrap();
    let mut manifest: Value = serde_json::from_slice(
        &unseal(
            Purpose::Workspace,
            &fs::read(&checkpoint).unwrap(),
            32 * 1024 * 1024,
        )
        .unwrap(),
    )
    .unwrap();
    manifest["version"] = json!(1);
    let snapshot = manifest["sources"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .next()
        .unwrap();
    snapshot.as_object_mut().unwrap().remove("policy");
    snapshot["length"] = json!(original.len());
    snapshot["checksum"] = json!(format!(
        "{:016x}",
        original
            .iter()
            .fold(0xcbf29ce484222325u64, |h, b| (h ^ *b as u64)
                .wrapping_mul(0x100000001b3))
    ));
    fs::write(
        root.join("sources")
            .join(snapshot["file"].as_str().unwrap()),
        seal(Purpose::Source, &original).unwrap(),
    )
    .unwrap();
    fs::write(
        checkpoint,
        seal(Purpose::Workspace, &serde_json::to_vec(&manifest).unwrap()).unwrap(),
    )
    .unwrap();
    drop(store);
    let store = RecoveryStore::new(root).unwrap();
    let restored = store.load(&e).unwrap().unwrap();
    let id = restored["tabs"][0]["document"]["pages"][0]["sourceId"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(matches!(
        e.list_text_runs(&id, 0),
        Err(EngineError::PermissionDenied(_))
    ));
    store.save(&e, restored).unwrap();
    e.close_document(&id).unwrap();
    let restored = store.load(&e).unwrap().unwrap();
    let plan: Vec<PagePlan> =
        serde_json::from_value(restored["tabs"][0]["document"]["pages"].clone()).unwrap();
    let output = temp.path().join("upgraded.pdf");
    e.export_pdf(
        ExportRequest {
            pages: plan.clone(),
            flatten: false,
        },
        &output,
    )
    .unwrap();
    assert!(unlock_pdf(&fs::read(output).unwrap(), Some("owner"))
        .unwrap()
        .protection
        .is_some());
    e.close_document(&plan[0].source_id).unwrap();
}


#[test]
fn physical_units_survive_password_recovery_unlock_and_protected_export() {
    use lopdf::{dictionary,Document,Object,Stream};use folio_engine::OpenOutcome;
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for unit in [0.5_f32,2.] {
        let mut pdf=Document::with_version("1.7");let tree=pdf.new_object_id();let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
        let content=pdf.add_object(Stream::new(dictionary!{},b"BT /F1 12 Tf 20 250 Td (Protected scale) Tj ET".to_vec()));
        let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),200.into(),300.into()],"UserUnit"=>unit,"Contents"=>content,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}});
        pdf.objects.insert(tree,dictionary!{"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1}.into());let root=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>tree});pdf.trailer.set("Root",root);let mut bytes=vec![];pdf.save_to(&mut bytes).unwrap();
        let bytes=protected_fixture::encrypt(&bytes,Permissions::PRINTABLE);let path=temp.path().join(format!("protected-units-{unit}.pdf"));fs::write(&path,&bytes).unwrap();
        let OpenOutcome::PasswordRequired{challenge_id,..}=e.begin_open_document(&path).unwrap()else{panic!("Password required")};
        let OpenOutcome::Opened{document:info}=e.submit_open_password(&challenge_id,"reader").unwrap()else{panic!("Expected authenticated source")};
        assert_eq!((info.pages[0].width,info.pages[0].height),(200.*unit,300.*unit));assert!(!info.protection.as_ref().unwrap().owner);assert!(matches!(e.page_text(&info.id,0),Err(EngineError::PermissionDenied(_))));
        let plan=pages(&info);let store=RecoveryStore::new(temp.path().join(format!("recovery-{unit}"))).unwrap();store.save(&e,workspace(&plan)).unwrap();
        let restored=store.load(&e).unwrap().unwrap();let rp=&restored["tabs"][0]["document"]["pages"][0];assert_eq!(rp["width"],json!(200.*unit));let restored_id=rp["sourceId"].as_str().unwrap();assert!(matches!(e.page_text(restored_id,0),Err(EngineError::PermissionDenied(_))));
        let owner=e.unlock_document_sources(&[info.id.clone()],"owner").unwrap().remove(0).document;assert!(owner.protection.as_ref().unwrap().owner);assert_eq!(owner.pages[0].width,200.*unit);assert_eq!(e.list_text_runs(&owner.id,0).unwrap().runs[0].font_size,12.*unit);
        let output=temp.path().join(format!("protected-copy-{unit}.pdf"));e.export_pdf(ExportRequest{pages:plan,flatten:false},&output).unwrap();let copied=e.open_document_with_password(&output,"reader").unwrap();assert_eq!(copied.pages[0].width,200.*unit);assert!(!copied.protection.as_ref().unwrap().permissions.edit_content);assert_eq!(fs::read(&path).unwrap(),bytes);
        for id in [&info.id,restored_id,&owner.id,&copied.id]{e.close_document(id).unwrap();}
    }
}
