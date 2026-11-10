use folio_engine::protection::{unlock_pdf, ProtectionError};
use folio_engine::{
    DocumentInfo, EngineError, ExportRequest, OpenOutcome, PagePlan, PdfEngine, RecoveryStore,
};
use serde_json::json;
use std::{fs, path::PathBuf};

static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
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
fn challenge(outcome: OpenOutcome) -> String {
    match outcome {
        OpenOutcome::PasswordRequired {
            challenge_id,
            incorrect,
            ..
        } => {
            assert!(!incorrect);
            challenge_id
        }
        _ => panic!("expected password challenge"),
    }
}
fn opened(outcome: OpenOutcome) -> DocumentInfo {
    match outcome {
        OpenOutcome::Opened { document } => document,
        _ => panic!("expected opened document"),
    }
}
fn pages(info: &DocumentInfo) -> Vec<PagePlan> {
    info.pages
        .iter()
        .enumerate()
        .map(|(i, p)| PagePlan {
            crop: None,
            id: format!("page-{i}"),
            source_id: info.id.clone(),
            page_index: i,
            width: p.width,
            height: p.height,
            rotation: 0,
            overlays: p.overlays.clone(),
        })
        .collect()
}

#[test]
fn challenge_retries_exact_chosen_bytes_and_cancel_invalidates_the_token() {
    let _serial = SERIAL.lock().unwrap();
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("chosen.pdf");
    fs::write(&path, fixture("aes_256")).unwrap();
    let token = challenge(e.begin_open_document(&path).unwrap());
    fs::write(
        &path,
        b"The original file changed while the password dialog was open.",
    )
    .unwrap();
    match e.submit_open_password(&token, "wrong").unwrap() {
        OpenOutcome::PasswordRequired {
            challenge_id,
            name,
            incorrect,
        } => {
            assert_eq!(challenge_id, token);
            assert_eq!(name, "chosen.pdf");
            assert!(incorrect);
        }
        _ => panic!("wrong password opened the source"),
    }
    let source = opened(e.submit_open_password(&token, "owner").unwrap());
    assert!(e
        .extract_text(&source.id, 0)
        .unwrap()
        .contains("Protected Folio fixture"));
    assert!(e.submit_open_password(&token, "owner").is_err());
    assert!(e
        .export_pdf(
            ExportRequest {
                pages: pages(&source),
                flatten: false
            },
            &path
        )
        .is_err());
    e.close_document(&source.id).unwrap();
    fs::write(&path, fixture("aes_128")).unwrap();
    let token = challenge(e.begin_open_document(&path).unwrap());
    e.cancel_open_document(&token).unwrap();
    e.cancel_open_document(&token).unwrap();
    assert!(e.submit_open_password(&token, "owner").is_err());
    assert!(e
        .submit_open_password(path.to_str().unwrap(), "owner")
        .is_err());
    fs::write(&path, fixture("plain")).unwrap();
    let plain = opened(e.begin_open_document(&path).unwrap());
    assert!(plain.protection.is_none());
    e.close_document(&plain.id).unwrap();
    fs::write(&path, fixture("empty_user")).unwrap();
    let empty = opened(e.begin_open_document(&path).unwrap());
    assert!(!empty.protection.as_ref().unwrap().owner);
    e.close_document(&empty.id).unwrap();
}

#[test]
fn challenge_capacity_is_released_on_cancel_and_success() {
    let _serial = SERIAL.lock().unwrap();
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    fs::write(&path, fixture("aes_256")).unwrap();
    let tokens = (0..4)
        .map(|_| challenge(e.begin_open_document(&path).unwrap()))
        .collect::<Vec<_>>();
    assert!(e.begin_open_document(&path).is_err());
    e.cancel_open_document(&tokens[0]).unwrap();
    let token = challenge(e.begin_open_document(&path).unwrap());
    let source = opened(e.submit_open_password(&token, "reader").unwrap());
    e.close_document(&source.id).unwrap();
    let next = challenge(e.begin_open_document(&path).unwrap());
    e.cancel_open_document(&next).unwrap();
    for token in &tokens[1..] {
        e.cancel_open_document(token).unwrap();
    }
}

#[test]
fn owner_unlock_creates_new_versions_and_does_not_escalate_old_or_recovered_sources() {
    let _serial = SERIAL.lock().unwrap();
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("source.pdf");
    fs::write(&path, fixture("aes_128")).unwrap();
    let source = e.open_document_with_password(&path, "reader").unwrap();
    for password in ["wrong", "reader"] {
        assert!(matches!(
            e.unlock_document_sources(&[source.id.clone()], password),
            Err(EngineError::Protection(ProtectionError::IncorrectPassword))
        ));
    }
    fs::remove_file(&path).unwrap(); // Owner authentication uses retained metadata.
    let versions = e
        .unlock_document_sources(&[source.id.clone()], "owner")
        .unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].source_id, source.id);
    let owner = &versions[0].document;
    assert!(owner.protection.as_ref().unwrap().owner);
    assert_eq!(
        owner.protection.as_ref().unwrap().document_id,
        source.protection.as_ref().unwrap().document_id
    );
    assert!(matches!(
        e.list_text_runs(&source.id, 0),
        Err(EngineError::PermissionDenied(_))
    ));
    assert!(e.list_text_runs(&owner.id, 0).is_ok());
    let state = json!({"version":1,"activeId":"tab","tabs":[{"id":"tab","dirty":true,"savedDigest":"old","zoom":100,"scrollPosition":{"top":0,"left":0},"document":{"name":"Protected.pdf","pages":pages(&owner),"selectedPageId":"page-0","selectedOverlayId":null}}]});
    let store = RecoveryStore::new(temp.path().join("recovery")).unwrap();
    store.save(&e, state).unwrap();
    e.close_document(&owner.id).unwrap();
    let state = store.load(&e).unwrap().unwrap();
    let plans: Vec<PagePlan> =
        serde_json::from_value(state["tabs"][0]["document"]["pages"].clone()).unwrap();
    assert!(e.list_text_runs(&plans[0].source_id, 0).is_ok());
    assert!(matches!(
        e.list_text_runs(&source.id, 0),
        Err(EngineError::PermissionDenied(_))
    ));
    let output = temp.path().join("owner-saved.pdf");
    e.export_pdf(
        ExportRequest {
            pages: plans.clone(),
            flatten: false,
        },
        &output,
    )
    .unwrap();
    for password in ["reader", "owner"] {
        let unlocked = unlock_pdf(&fs::read(&output).unwrap(), Some(password)).unwrap();
        assert_eq!(unlocked.protection.unwrap().is_owner(), password == "owner");
    }
    e.close_document(&source.id).unwrap();
    e.close_document(&plans[0].source_id).unwrap();
}

#[test]
fn owner_unlock_checks_every_revision_and_rejects_unrelated_or_invalid_sets() {
    let _serial=SERIAL.lock().unwrap();let e=engine();let temp=tempfile::tempdir().unwrap();
    for name in ["rc4_40","rc4_128","aes_128","aes_256_r5","aes_256","empty_user","unicode"] {
        let (reader,owner)=match name {"empty_user"=>("","owner"),"unicode"=>("café-读者","拥有者-é"),_=>("reader","owner")};
        let path=temp.path().join(format!("{name}.pdf"));fs::write(&path,fixture(name)).unwrap();
        let source=e.open_document_with_password(&path,reader).unwrap();
        assert!(matches!(e.unlock_document_sources(&[source.id.clone()],reader),Err(EngineError::Protection(ProtectionError::IncorrectPassword))));
        let other=e.open_document_with_password(&path,reader).unwrap(); // Separate opening, separate provenance.
        for ids in [vec![],vec![source.id.clone(),source.id.clone()],vec![source.id.clone(),other.id.clone()],vec![source.id.clone(),"missing".into()]] {
            assert!(e.unlock_document_sources(&ids,owner).is_err());
        }
        let unlocked=e.unlock_document_sources(&[source.id.clone()],owner).unwrap().remove(0).document;
        assert!(unlocked.protection.as_ref().unwrap().owner);
        assert!(matches!(e.list_text_runs(&source.id,0),Err(EngineError::PermissionDenied(_))));
        let output=temp.path().join(format!("unlocked-{name}.pdf"));e.export_pdf(ExportRequest{pages:pages(&unlocked),flatten:false},&output).unwrap();
        for password in [reader,owner] {assert!(unlock_pdf(&fs::read(&output).unwrap(),Some(password)).unwrap().protection.is_some());}
        for id in [&source.id,&other.id,&unlocked.id] {e.close_document(id).unwrap();}
    }
}
