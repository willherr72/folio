use folio_engine::private_storage::{seal, unseal, Purpose};
use folio_engine::{EngineError, ExportRequest, PagePlan, PdfEngine, RecoveryStore};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

fn engine() -> PdfEngine {
    let path = std::env::var_os("FOLIO_PDFIUM_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")
        });
    PdfEngine::start(path).unwrap()
}

fn write_pdf(path: &Path, label: &str) {
    let stream = format!("BT /F1 20 Tf 18 265 Td ({label}) Tj ET");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 300] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>".to_string(),
        format!("<< /Length {} >>\nstream\n{stream}\nendstream", stream.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    let mut pdf = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend_from_slice(b"xref\n0 6\n0000000000 65535 f \n");
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!("trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    fs::write(path, pdf).unwrap();
}

fn workspace(source: &str) -> Value {
    json!({"version":1,"activeId":"tab-a","tabs":[{
        "id":"tab-a","savedDigest":"old digest","dirty":true,"zoom":110,
        "scrollPosition":{"top":80,"left":12},
        "document":{"name":"Original.pdf","selectedPageId":"page-a","selectedOverlayId":"text-a","pages":[{
            "id":"page-a","sourceId":source,"pageIndex":0,"width":200,"height":300,"rotation":90,
            "overlays":[{"type":"text","id":"text-a","text":"Editable note","x":20,"y":30,"fontSize":14,"color":"#123456"}]
        }]}
    }]})
}

#[test]
fn recovery_keeps_crop_in_version_four_and_rejects_invalid_replacement() {
    let temp=tempfile::tempdir().unwrap();let original=temp.path().join("original.pdf");
    write_pdf(&original,"Crop recovery");let engine=engine();let source=engine.open_document(&original).unwrap();
    let root=temp.path().join("recovery");let store=RecoveryStore::new(root.clone()).unwrap();
    let mut saved=workspace(&source.id);let crop=json!({"x":10,"y":20,"width":120,"height":160});
    saved["tabs"][0]["document"]["pages"][0]["crop"]=crop.clone();
    store.save(&engine,saved.clone()).unwrap();
    let checkpoint=fs::read_dir(&root).unwrap().map(|e|e.unwrap().path()).find(|p|p.extension().is_some_and(|e|e=="json")).unwrap();
    let manifest:Value=serde_json::from_slice(&unseal(Purpose::Workspace,&fs::read(checkpoint).unwrap(),32*1024*1024).unwrap()).unwrap();
    assert_eq!(manifest["version"],4);
    saved["tabs"][0]["document"]["pages"][0]["crop"]["width"]=json!(201);
    assert!(store.save(&engine,saved).is_err());
    engine.close_document(&source.id).unwrap();
    let restored=store.load(&engine).unwrap().unwrap();let page=&restored["tabs"][0]["document"]["pages"][0];
    assert_eq!(page["crop"],crop);
    let output=temp.path().join("recovered-crop.pdf");let plan:PagePlan=serde_json::from_value(page.clone()).unwrap();
    engine.export_pdf(ExportRequest{pages:vec![plan],flatten:false},&output).unwrap();
    let opened=engine.open_document(&output).unwrap();
    assert_eq!((opened.pages[0].width,opened.pages[0].height),(160.,120.));
    engine.close_document(&opened.id).unwrap();engine.close_document(page["sourceId"].as_str().unwrap()).unwrap();
}

#[test]
fn recovery_payloads_are_account_protected_and_restore_without_plaintext_files() {
    let temp = tempfile::tempdir().unwrap();
    let original = temp.path().join("original.pdf");
    write_pdf(&original, "SECRET42");
    let engine = engine();
    let source = engine.open_document(&original).unwrap();
    let saved = workspace(&source.id);
    let root = temp.path().join("recovery");
    let store = RecoveryStore::new(root.clone()).unwrap();
    store.save(&engine, saved).unwrap();
    for directory in [&root, &root.join("sources"), &root.join("fonts")] {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if !path.is_file() || path.file_name().unwrap() == "recovery.lock" {
                continue;
            }
            let bytes = fs::read(&path).unwrap();
            assert!(
                bytes.starts_with(b"FOLIO-SEALED-"),
                "{} was not sealed",
                path.display()
            );
            for secret in [b"SECRET42".as_slice(), b"Editable note"] {
                assert!(!bytes.windows(secret.len()).any(|w| w == secret));
            }
        }
    }
    fs::remove_file(original).unwrap();
    engine.close_document(&source.id).unwrap();
    drop(store);
    let store = RecoveryStore::new(root.clone()).unwrap();
    let restored = store.load(&engine).unwrap().unwrap();
    let id = restored["tabs"][0]["document"]["pages"][0]["sourceId"]
        .as_str()
        .unwrap();
    assert!(engine.extract_text(id, 0).unwrap().contains("SECRET42"));
    assert_eq!(
        restored["tabs"][0]["document"]["pages"][0]["overlays"][0]["text"],
        "Editable note"
    );
    for entry in fs::read_dir(root.join("sources")).unwrap() {
        assert!(fs::read(entry.unwrap().path())
            .unwrap()
            .starts_with(b"FOLIO-SEALED-"));
    }
    engine.close_document(id).unwrap();
}

#[test]
fn generated_blank_source_recovers_without_an_original_disk_file() {
    let engine = engine();
    let blank = engine.create_blank_document(200.0, 300.0).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let store = RecoveryStore::new(temp.path().join("recovery")).unwrap();
    store.save(&engine, workspace(&blank.id)).unwrap();
    engine.close_document(&blank.id).unwrap();
    let restored = store.load(&engine).unwrap().unwrap();
    let page = &restored["tabs"][0]["document"]["pages"][0];
    let id = page["sourceId"].as_str().unwrap();
    assert_ne!(id, blank.id);
    assert!(engine.extract_text(id, 0).unwrap().trim().is_empty());
    assert_eq!(page["overlays"][0]["text"], "Editable note");
    let output = temp.path().join("restored.pdf");
    engine
        .export_pdf(
            serde_json::from_value(json!({"pages":[page]})).unwrap(),
            &output,
        )
        .unwrap();
    assert_eq!(engine.open_document(&output).unwrap().pages.len(), 1);
}

fn manifests(root: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("checkpoint-")
                && p.extension().is_some_and(|e| e == "json")
        })
        .collect();
    paths.sort();
    paths
}

#[test]
fn abandoned_temporary_payloads_are_removed_on_restore_and_discard() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("recovery");
    let store = RecoveryStore::new(root.clone()).unwrap();
    let engine = engine();
    for operation in ["load", "clear", "interrupted clear"] {
        let paths: Vec<_> = [&root, &root.join("sources"), &root.join("fonts")]
            .into_iter()
            .map(|dir| dir.join(".folio-recovery-Ab12Cd"))
            .collect();
        for path in &paths {
            fs::write(path, b"Legacy private document content").unwrap();
        }
        let unrelated = root.join("unrelated.txt");
        fs::write(&unrelated, "leave alone").unwrap();
        // A matching directory must never be recursively traversed or removed.
        let directory = root.join(".folio-recovery-directory");
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("private.txt"), "leave alone").unwrap();
        if operation == "clear" {
            store.clear().unwrap();
        } else {
            if operation == "interrupted clear" {
                fs::write(root.join("cleared"), "1").unwrap();
            }
            assert!(store.load(&engine).unwrap().is_none());
        }
        assert!(paths.iter().all(|path| !path.exists()), "{operation}");
        assert_eq!(fs::read_to_string(unrelated).unwrap(), "leave alone");
        assert!(directory.join("private.txt").exists());
    }
}

#[test]
fn migration_failure_in_unused_font_or_latest_checkpoint_does_not_block_recovery_or_save() {
    let temp = tempfile::tempdir().unwrap();
    let original = temp.path().join("original.pdf");
    write_pdf(&original, "VALID");
    let engine = engine();
    let source = engine.open_document(&original).unwrap();
    let root = temp.path().join("recovery");
    let store = RecoveryStore::new(root.clone()).unwrap();
    store.save(&engine, workspace(&source.id)).unwrap();
    let orphan = root.join("fonts").join(format!("{}.font", "a".repeat(64)));
    fs::File::create(&orphan)
        .unwrap()
        .set_len(16 * 1024 * 1024 + 1)
        .unwrap();
    let corrupt = root.join("checkpoint-00000000000000000002.json");
    fs::File::create(&corrupt)
        .unwrap()
        .set_len(32 * 1024 * 1024 + 1)
        .unwrap();
    let restored = store.load(&engine).unwrap().unwrap();
    assert_eq!(restored["tabs"][0]["document"]["name"], "Original.pdf");
    assert!(!orphan.exists());
    assert!(!corrupt.exists());
    // Repeat with no load first: a new durable save must be possible too.
    fs::File::create(&orphan)
        .unwrap()
        .set_len(16 * 1024 * 1024 + 1)
        .unwrap();
    store.save(&engine, restored).unwrap();
    assert!(!orphan.exists());
}

#[test]
fn failed_required_legacy_migration_is_retained_for_retry_or_explicit_discard() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("recovery");
    let store = RecoveryStore::new(root.clone()).unwrap();
    let checkpoint = root.join("checkpoint-00000000000000000001.json");
    fs::File::create(&checkpoint)
        .unwrap()
        .set_len(32 * 1024 * 1024 + 1)
        .unwrap();
    assert!(store.load(&engine()).unwrap_err().contains("checkpoint"));
    assert_eq!(
        fs::metadata(&checkpoint).unwrap().len(),
        32 * 1024 * 1024 + 1
    );
    store.clear().unwrap();
    assert!(!checkpoint.exists());
}

#[test]
fn migrates_plaintext_and_interrupted_mixed_legacy_recovery() {
    for mixed in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let original = temp.path().join("original.pdf");
        write_pdf(&original, "LEGACY42");
        let before = fs::read(&original).unwrap();
        let engine = engine();
        let source = engine.open_document(&original).unwrap();
        let root = temp.path().join("recovery");
        let store = RecoveryStore::new(root.clone()).unwrap();
        store.save(&engine, workspace(&source.id)).unwrap();
        drop(store);
        let checkpoint = manifests(&root).pop().unwrap();
        let source_path = fs::read_dir(root.join("sources"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        for (path, purpose) in [
            (&checkpoint, Purpose::Workspace),
            (&source_path, Purpose::Source),
        ] {
            if mixed && purpose == Purpose::Workspace {
                continue;
            }
            let plain = unseal(purpose, &fs::read(path).unwrap(), 512 * 1024 * 1024).unwrap();
            fs::write(path, plain).unwrap(); // Synthetic pre-upgrade disk format.
        }
        let unchanged_ciphertext = if mixed {
            Some(fs::read(&checkpoint).unwrap())
        } else {
            None
        };
        engine.close_document(&source.id).unwrap();
        let store = RecoveryStore::new(root.clone()).unwrap();
        let restored = store.load(&engine).unwrap().unwrap();
        let id = restored["tabs"][0]["document"]["pages"][0]["sourceId"]
            .as_str()
            .unwrap();
        assert!(engine.extract_text(id, 0).unwrap().contains("LEGACY42"));
        assert!(fs::read(&source_path)
            .unwrap()
            .starts_with(b"FOLIO-SEALED-"));
        assert!(fs::read(&checkpoint).unwrap().starts_with(b"FOLIO-SEALED-"));
        if let Some(before) = unchanged_ciphertext {
            assert_eq!(fs::read(checkpoint).unwrap(), before);
        }
        assert_eq!(fs::read(original).unwrap(), before);
        engine.close_document(id).unwrap();
    }
}

#[test]
fn swapped_payload_purpose_and_unknown_envelope_version_do_not_restore() {
    let temp = tempfile::tempdir().unwrap();
    let original = temp.path().join("original.pdf");
    write_pdf(&original, "PRIVATE");
    let engine = engine();
    let source = engine.open_document(&original).unwrap();
    let root = temp.path().join("recovery");
    let store = RecoveryStore::new(root.clone()).unwrap();
    store.save(&engine, workspace(&source.id)).unwrap();
    let path = manifests(&root).pop().unwrap();
    let protected = fs::read(&path).unwrap();
    let plain = unseal(Purpose::Workspace, &protected, 32 * 1024 * 1024).unwrap();
    fs::write(&path, seal(Purpose::Source, &plain).unwrap()).unwrap();
    assert!(store.load(&engine).is_err());
    let mut unknown = protected;
    unknown[14] = b'9';
    fs::write(&path, &unknown).unwrap();
    assert!(store.load(&engine).is_err());
    assert_eq!(
        fs::read(path).unwrap(),
        unknown,
        "unknown formats must not be rewrapped as legacy"
    );
}

#[test]
fn redirected_source_directory_never_changes_external_files() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("recovery");
    let store = RecoveryStore::new(root.clone()).unwrap();
    let original = temp.path().join("original.pdf");
    write_pdf(&original, "EXTERNAL_PRIVATE");
    let before = fs::read(&original).unwrap();
    let engine = engine();
    let source = engine.open_document(&original).unwrap();
    let outside = temp.path().join("outside");
    fs::create_dir(&outside).unwrap();
    let external_temporary = outside.join(".folio-recovery-Ab12Cd");
    fs::write(&external_temporary, "External private content").unwrap();
    fs::remove_dir(root.join("sources")).unwrap(); // Known empty test-owned directory.
    #[cfg(windows)]
    {
        let status = std::process::Command::new("cmd.exe")
            .args(["/c", "mklink", "/J"])
            .arg(root.join("sources"))
            .arg(&outside)
            .output()
            .unwrap();
        assert!(status.status.success(), "test junction creation failed");
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, root.join("sources")).unwrap();
    assert!(store.save(&engine, workspace(&source.id)).is_err());
    assert!(store.load(&engine).is_err());
    assert!(store.clear().is_err());
    assert_eq!(fs::read(original).unwrap(), before);
    assert_eq!(
        fs::read_to_string(&external_temporary).unwrap(),
        "External private content"
    );
    assert_eq!(fs::read_dir(outside).unwrap().count(), 1);
    #[cfg(windows)]
    fs::remove_dir(root.join("sources")).unwrap(); // Remove junction itself, never its target.
    #[cfg(unix)]
    fs::remove_file(root.join("sources")).unwrap();
}

#[test]
fn restores_editable_workspace_after_original_is_replaced_and_removed() {
    let temp = tempfile::tempdir().unwrap();
    let original = temp.path().join("original.pdf");
    write_pdf(&original, "ORIGINAL");
    let original_bytes = fs::read(&original).unwrap();
    let engine = engine();
    let source = engine.open_document(&original).unwrap();
    write_pdf(&original, "REPLACED");
    let root = temp.path().join("recovery");
    let store = RecoveryStore::new(root.clone()).unwrap();
    store.save(&engine, workspace(&source.id)).unwrap();
    fs::remove_file(&original).unwrap();
    engine.close_document(&source.id).unwrap();
    drop(store);
    let store = RecoveryStore::new(root.clone()).unwrap();
    let restored = store.load(&engine).unwrap().expect("saved workspace");
    let tab = &restored["tabs"][0];
    let page = &tab["document"]["pages"][0];
    let restored_source = page["sourceId"].as_str().unwrap();
    assert_ne!(restored_source, source.id);
    assert!(engine
        .extract_text(restored_source, 0)
        .unwrap()
        .contains("ORIGINAL"));
    assert!(!engine
        .extract_text(restored_source, 0)
        .unwrap()
        .contains("Editable note"));
    assert_eq!(page["overlays"][0]["text"], "Editable note");
    assert_eq!(page["rotation"], 90);
    assert_eq!(tab["scrollPosition"]["top"], 80);
    assert_eq!(tab["zoom"], 110);
    assert_eq!(tab["dirty"], true);
    let snapshots: Vec<_> = fs::read_dir(root.join("sources"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(
        unseal(
            Purpose::Source,
            &fs::read(&snapshots[0]).unwrap(),
            512 * 1024 * 1024
        )
        .unwrap(),
        original_bytes
    );
    engine.close_document(restored_source).unwrap();
}

#[test]
fn exclusive_lock_releases_when_store_is_dropped() {
    let temp = tempfile::tempdir().unwrap();
    let store = RecoveryStore::new(temp.path().to_path_buf()).unwrap();
    assert!(
        RecoveryStore::new(temp.path().to_path_buf()).is_err(),
        "second instance must not overwrite recovery"
    );
    drop(store);
    assert!(RecoveryStore::new(temp.path().to_path_buf()).is_ok());
}

#[test]
fn falls_back_after_interrupted_write_and_corrupt_latest_checkpoint() {
    let temp = tempfile::tempdir().unwrap();
    let original = temp.path().join("original.pdf");
    write_pdf(&original, "ORIGINAL");
    let engine = engine();
    let source = engine.open_document(&original).unwrap();
    let root = temp.path().join("recovery");
    let store = RecoveryStore::new(root.clone()).unwrap();
    store.save(&engine, workspace(&source.id)).unwrap();
    let mut newer = workspace(&source.id);
    newer["tabs"][0]["zoom"] = json!(200);
    store.save(&engine, newer).unwrap();
    fs::write(
        root.join("checkpoint-99999999999999999999.json.tmp"),
        "interrupted",
    )
    .unwrap();
    fs::write(manifests(&root).last().unwrap(), "broken JSON").unwrap();
    let recovered = store.load(&engine).unwrap().unwrap();
    assert_eq!(recovered["tabs"][0]["zoom"], 110);
}

#[test]
fn failed_checkpoint_keeps_previous_recoverable_state() {
    let temp = tempfile::tempdir().unwrap();
    let original = temp.path().join("original.pdf");
    write_pdf(&original, "ORIGINAL");
    let engine = engine();
    let source = engine.open_document(&original).unwrap();
    let store = RecoveryStore::new(temp.path().join("recovery")).unwrap();
    store.save(&engine, workspace(&source.id)).unwrap();
    assert!(store.save(&engine, workspace("unknown-source")).is_err());
    assert!(store.load(&engine).unwrap().is_some());
}

#[test]
fn missing_source_is_reported_and_clear_removes_all_checkpoints() {
    let temp = tempfile::tempdir().unwrap();
    let original = temp.path().join("original.pdf");
    write_pdf(&original, "ORIGINAL");
    let engine = engine();
    let source = engine.open_document(&original).unwrap();
    let root = temp.path().join("recovery");
    let store = RecoveryStore::new(root.clone()).unwrap();
    store.save(&engine, workspace(&source.id)).unwrap();
    for file in fs::read_dir(root.join("sources")).unwrap() {
        fs::remove_file(file.unwrap().path()).unwrap();
    }
    let error = store.load(&engine).unwrap_err();
    assert!(error.contains("source"), "{error}");
    store.clear().unwrap();
    assert!(store.load(&engine).unwrap().is_none());
    assert!(manifests(&root).is_empty());
}

#[test]
fn rejects_malformed_workspace_and_path_injection() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    let store = RecoveryStore::new(temp.path().to_path_buf()).unwrap();
    for invalid in [
        json!({"version":42,"activeId":null,"tabs":[]}),
        json!({"version":1,"activeId":null,"tabs":[{}]}),
        workspace("../../private.pdf"),
    ] {
        assert!(store.save(&engine, invalid).is_err());
    }
    assert!(store.load(&engine).unwrap().is_none());
}

#[test]
fn retention_keeps_last_good_generation_and_collects_unreferenced_sources() {
    let temp = tempfile::tempdir().unwrap();
    let original = temp.path().join("original.pdf");
    let engine = engine();
    let root = temp.path().join("recovery");
    let store = RecoveryStore::new(root.clone()).unwrap();
    for label in ["FIRST", "SECOND", "THIRD"] {
        write_pdf(&original, label);
        let source = engine.open_document(&original).unwrap();
        let mut saved = workspace(&source.id);
        saved["tabs"][0]["document"]["name"] = json!(label);
        store.save(&engine, saved).unwrap();
        if label == "SECOND" {
            fs::write(manifests(&root).last().unwrap(), "corrupt").unwrap();
        }
        engine.close_document(&source.id).unwrap();
    }
    assert_eq!(manifests(&root).len(), 2);
    assert_eq!(fs::read_dir(root.join("sources")).unwrap().count(), 2);
    fs::write(manifests(&root).last().unwrap(), "corrupt").unwrap();
    let restored = store.load(&engine).unwrap().unwrap();
    assert_eq!(restored["tabs"][0]["document"]["name"], "FIRST");
}

#[test]
fn source_corruption_and_manifest_paths_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let original = temp.path().join("original.pdf");
    write_pdf(&original, "ORIGINAL");
    let engine = engine();
    let source = engine.open_document(&original).unwrap();
    let root = temp.path().join("recovery");
    let store = RecoveryStore::new(root.clone()).unwrap();
    store.save(&engine, workspace(&source.id)).unwrap();
    let manifest_path = manifests(&root).pop().unwrap();
    let mut manifest: Value = serde_json::from_slice(
        &unseal(
            Purpose::Workspace,
            &fs::read(&manifest_path).unwrap(),
            32 * 1024 * 1024,
        )
        .unwrap(),
    )
    .unwrap();
    let source_path = root
        .join("sources")
        .join(manifest["sources"][&source.id]["file"].as_str().unwrap());
    let mut bytes = fs::read(&source_path).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0x80;
    fs::write(&source_path, bytes).unwrap();
    assert!(store.load(&engine).unwrap_err().contains("source"));
    manifest["sources"][&source.id]["file"] = json!("../../original.pdf");
    fs::write(
        manifest_path,
        seal(Purpose::Workspace, &serde_json::to_vec(&manifest).unwrap()).unwrap(),
    )
    .unwrap();
    assert!(store.load(&engine).is_err());
    assert!(fs::read_to_string(original).unwrap().contains("ORIGINAL"));
}

#[test]
fn interrupted_discard_cannot_resurface_after_a_later_save() {
    let temp = tempfile::tempdir().unwrap();
    let original = temp.path().join("original.pdf");
    write_pdf(&original, "ORIGINAL");
    let engine = engine();
    let source = engine.open_document(&original).unwrap();
    let root = temp.path().join("recovery");
    let store = RecoveryStore::new(root.clone()).unwrap();
    store.save(&engine, workspace(&source.id)).unwrap();
    // Simulate death immediately after discard committed, before old files were removed.
    fs::write(root.join("cleared"), "1").unwrap();
    drop(store);
    let store = RecoveryStore::new(root.clone()).unwrap();
    assert!(store.load(&engine).unwrap().is_none());
    store.save(&engine, workspace(&source.id)).unwrap();
    fs::write(manifests(&root).last().unwrap(), "corrupt").unwrap();
    assert!(
        store.load(&engine).is_err(),
        "explicitly discarded state must never return"
    );
}

#[test]
fn unchanged_sources_are_reused_across_checkpoints_and_runtime_remapping() {
    let temp = tempfile::tempdir().unwrap();
    let original = temp.path().join("original.pdf");
    write_pdf(&original, "ORIGINAL");
    let engine = engine();
    let source = engine.open_document(&original).unwrap();
    let root = temp.path().join("recovery");
    let store = RecoveryStore::new(root.clone()).unwrap();
    store.save(&engine, workspace(&source.id)).unwrap();
    let path = fs::read_dir(root.join("sources"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    store.save(&engine, workspace(&source.id)).unwrap();
    engine.close_document(&source.id).unwrap();
    let restored = store.load(&engine).unwrap().unwrap();
    store.save(&engine, restored).unwrap();
    assert_eq!(fs::read_dir(root.join("sources")).unwrap().count(), 1);
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
}

#[test]
fn duplicated_pages_can_keep_their_original_overlay_ids() {
    let temp = tempfile::tempdir().unwrap();
    let original = temp.path().join("original.pdf");
    write_pdf(&original, "ORIGINAL");
    let engine = engine();
    let source = engine.open_document(&original).unwrap();
    let store = RecoveryStore::new(temp.path().join("recovery")).unwrap();
    let mut saved = workspace(&source.id);
    let mut duplicate = saved["tabs"][0]["document"]["pages"][0].clone();
    duplicate["id"] = json!("duplicate-page");
    saved["tabs"][0]["document"]["pages"]
        .as_array_mut()
        .unwrap()
        .push(duplicate);
    store.save(&engine, saved).unwrap();
    let restored = store.load(&engine).unwrap().unwrap();
    assert_eq!(
        restored["tabs"][0]["document"]["pages"][1]["overlays"][0]["text"],
        "Editable note"
    );
}

#[test]
fn restored_sources_still_protect_original_paths_and_hardlink_aliases() {
    let temp = tempfile::tempdir().unwrap();
    let original = temp.path().join("original.pdf");
    let alias = temp.path().join("alias.pdf");
    write_pdf(&original, "ORIGINAL");
    fs::hard_link(&original, &alias).unwrap();
    let original_bytes = fs::read(&original).unwrap();
    let engine = engine();
    let source = engine.open_document(&original).unwrap();
    let store = RecoveryStore::new(temp.path().join("recovery")).unwrap();
    store.save(&engine, workspace(&source.id)).unwrap();
    engine.close_document(&source.id).unwrap();
    let restored = store.load(&engine).unwrap().unwrap();
    let page: PagePlan =
        serde_json::from_value(restored["tabs"][0]["document"]["pages"][0].clone()).unwrap();
    for destination in [&alias, &original] {
        let result = engine.export_pdf(
            ExportRequest {
                flatten: true,
                pages: vec![page.clone()],
            },
            destination,
        );
        assert!(
            result.is_err(),
            "recovery must preserve original source export protection"
        );
        assert_eq!(fs::read(destination).unwrap(), original_bytes);
    }
    // Saving the remapped workspace must retain the original identity too.
    store.save(&engine, restored).unwrap();
    engine.close_document(&page.source_id).unwrap();
    let restored_again = store.load(&engine).unwrap().unwrap();
    let page: PagePlan =
        serde_json::from_value(restored_again["tabs"][0]["document"]["pages"][0].clone()).unwrap();
    assert!(engine
        .export_pdf(
            ExportRequest {
                flatten: true,
                pages: vec![page]
            },
            &original
        )
        .is_err());
}

#[cfg(windows)]
#[test]
fn oversized_sparse_source_is_rejected_before_reading_its_contents() {
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;
    // Keep this test-only declaration local; no new runtime dependencies are needed.
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn DeviceIoControl(
            handle: *mut std::ffi::c_void,
            code: u32,
            input: *const std::ffi::c_void,
            input_size: u32,
            output: *mut std::ffi::c_void,
            output_size: u32,
            returned: *mut u32,
            overlapped: *mut std::ffi::c_void,
        ) -> i32;
    }
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("oversized.pdf");
    // Deny content reads as well: a missing preallocation guard fails promptly,
    // without making the regression test itself allocate half a GiB.
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .share_mode(0)
        .open(&path)
        .unwrap();
    let mut returned = 0;
    let sparse = unsafe {
        DeviceIoControl(
            file.as_raw_handle(),
            0x000900c4,
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            0,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    assert_ne!(
        sparse,
        0,
        "mark fixture sparse: {}",
        std::io::Error::last_os_error()
    );
    file.set_len(512 * 1024 * 1024 + 1).unwrap();
    let error = engine().open_document(&path).unwrap_err();
    assert!(
        matches!(error, EngineError::InvalidRequest(ref message) if message.contains("512 MiB")),
        "{error}"
    );
    assert_eq!(file.metadata().unwrap().len(), 512 * 1024 * 1024 + 1);
}


#[test]
fn recovery_retains_document_metadata_drafts_and_rejects_malformed_actions(){
 let temp=tempfile::tempdir().unwrap();let input=temp.path().join("source.pdf");write_pdf(&input,"Metadata recovery");let engine=engine();let source=engine.open_document(&input).unwrap();let store=RecoveryStore::new(temp.path().join("recovery")).unwrap();let mut state=workspace(&source.id);
 let metadata=json!({"mode":"update","clearExisting":true,"title":"Public title"});state["tabs"][0]["document"]["metadata"]=metadata.clone();store.save(&engine,state.clone()).unwrap();
 for invalid in [json!({"mode":"remove","title":"ignored"}),json!({"mode":"update","title":"x".repeat(16385)}),json!({"mode":"update"})]{state["tabs"][0]["document"]["metadata"]=invalid;assert!(store.save(&engine,state.clone()).is_err());}
 engine.close_document(&source.id).unwrap();let restored=store.load(&engine).unwrap().unwrap();assert_eq!(restored["tabs"][0]["document"]["metadata"],metadata);
}
