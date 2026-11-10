use folio_engine::protection::{unlock_pdf, PdfProtection, ProtectionError};
use lopdf::{Document, LoadOptions, Object};
use std::{fs, path::PathBuf};

fn fixture(name: &str) -> Vec<u8> {
    fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/protection")
            .join(format!("{name}.pdf")),
    )
    .unwrap()
}
const CASES: [(&str, i64); 5] = [
    ("rc4_40", 2),
    ("rc4_128", 3),
    ("aes_128", 4),
    ("aes_256_r5", 5),
    ("aes_256", 6),
];

fn record_context_output(name: &str, mode: &str, sealed: &[u8], pdf: &[u8]) {
    if let Some(directory) = std::env::var_os("FOLIO_RECOVERY_CONTEXT_OUTPUT") {
        let directory = PathBuf::from(directory);
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join(format!("{name}-{mode}.context")), sealed).unwrap();
        fs::write(directory.join(format!("{name}-{mode}.pdf")), pdf).unwrap();
    }
}

#[test]
fn account_protected_context_restores_credentials_rights_and_owner_mode() {
    for (name, revision) in CASES {
        for password in ["reader", "owner"] {
            let unlocked = unlock_pdf(&fixture(name), Some(password)).unwrap();
            let context = unlocked.protection.unwrap();
            let sealed = context.seal_recovery_context(&unlocked.bytes).unwrap();
            assert!(sealed.starts_with(b"FOLIO-SEALED-"));
            for value in [b"reader".as_slice(), b"owner", b"/Encrypt"] {
                assert!(!sealed.windows(value.len()).any(|s| s == value));
            }
            let restored = PdfProtection::from_recovery_context(&sealed, &unlocked.bytes).unwrap();
            assert_eq!(restored.revision(), revision);
            assert_eq!(restored.is_owner(), password == "owner");
            assert_eq!(restored.permissions(), context.permissions());
            let protected = restored.protect(&unlocked.bytes).unwrap();
            record_context_output(name, password, &sealed, &protected);
            for credential in ["reader", "owner"] {
                let again = unlock_pdf(&protected, Some(credential)).unwrap();
                assert_eq!(again.protection.unwrap().is_owner(), credential == "owner");
                assert_eq!(
                    Document::load_mem(&again.bytes).unwrap().get_pages().len(),
                    1
                );
            }
        }
    }
}

#[test]
fn recovery_context_rejects_wrong_source_purpose_corruption_and_plaintext() {
    use folio_engine::private_storage::{seal, Purpose};
    let unlocked = unlock_pdf(&fixture("aes_256"), Some("reader")).unwrap();
    let context = unlocked.protection.unwrap();
    let sealed = context.seal_recovery_context(&unlocked.bytes).unwrap();
    let mut different_source = unlocked.bytes.clone();
    different_source.push(b' ');
    assert!(PdfProtection::from_recovery_context(&sealed, &different_source).is_err());
    assert!(PdfProtection::from_recovery_context(&unlocked.bytes, &unlocked.bytes).is_err());
    assert!(PdfProtection::from_recovery_context(
        &seal(Purpose::Source, b"wrong purpose").unwrap(),
        &unlocked.bytes
    )
    .is_err());
    let mut altered = sealed.clone();
    *altered.last_mut().unwrap() ^= 1;
    assert!(PdfProtection::from_recovery_context(&altered, &unlocked.bytes).is_err());
    assert!(
        PdfProtection::from_recovery_context(&sealed[..sealed.len() - 1], &unlocked.bytes).is_err()
    );
    let debug = format!("{context:?}");
    assert!(
        !debug.contains("reader")
            && !debug.contains("owner_value")
            && !debug.contains("file_encryption_key")
    );
}

#[test]
fn authenticated_file_key_reconstruction_rejects_invalid_key_lengths_and_aes_permission_keys() {
    use lopdf::encryption::EncryptionState;
    for (name, revision) in CASES {
        let original = fixture(name);
        let decrypted = Document::load_mem_with_options(
            &original,
            LoadOptions {
                password: Some("reader".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let retained = decrypted.encryption_state.as_ref().unwrap();
        let key = retained.file_encryption_key();
        let metadata = Document::load_mem_with_options(
            &original,
            LoadOptions {
                skip_decryption: true,
                ..Default::default()
            },
        )
        .unwrap();
        let reconstructed = EncryptionState::from_authenticated_file_key(&metadata, key).unwrap();
        assert_eq!(reconstructed.revision(), revision);
        assert_eq!(reconstructed.permissions(), retained.permissions());
        assert!(
            EncryptionState::from_authenticated_file_key(&metadata, &key[..key.len() - 1]).is_err()
        );
        if revision >= 5 {
            let mut wrong = key.to_vec();
            wrong[0] ^= 1;
            assert!(EncryptionState::from_authenticated_file_key(&metadata, &wrong).is_err());
        }
    }
}

#[test]
fn private_context_validates_authenticated_structure_and_empty_unicode_credentials() {
    use folio_engine::private_storage::{seal, unseal, Purpose};
    for (name, user, owner) in [
        ("empty_user", "", "owner"),
        ("unicode", "café-读者", "拥有者-é"),
    ] {
        let unlocked = unlock_pdf(&fixture(name), Some(owner)).unwrap();
        let context = unlocked.protection.unwrap();
        let sealed = context.seal_recovery_context(&unlocked.bytes).unwrap();
        let restored = PdfProtection::from_recovery_context(&sealed, &unlocked.bytes).unwrap();
        let output = restored.protect(&unlocked.bytes).unwrap();
        record_context_output(name, "owner", &sealed, &output);
        for password in [user, owner] {
            assert!(unlock_pdf(&output, Some(password)).is_ok());
        }
        let plain = unseal(Purpose::ProtectionContext, &sealed, 128 * 1024).unwrap();
        let magic_length = b"FOLIO-PDF-CONTEXT-V1\0".len();
        for (index, value) in [
            (0, b'X'),
            (magic_length, 2),
            (magic_length + 1, 0),
            (magic_length + 2, plain[magic_length + 2] ^ 1),
        ] {
            let mut changed = plain.clone();
            changed[index] = value;
            let changed = seal(Purpose::ProtectionContext, &changed).unwrap();
            assert!(PdfProtection::from_recovery_context(&changed, &unlocked.bytes).is_err());
        }
    }
}

#[test]
fn distinguishes_missing_wrong_user_and_owner_passwords() {
    for (name, revision) in CASES {
        let bytes = fixture(name);
        assert!(
            matches!(
                unlock_pdf(&bytes, None),
                Err(ProtectionError::PasswordRequired)
            ),
            "{name}"
        );
        assert!(
            matches!(
                unlock_pdf(&bytes, Some("wrong")),
                Err(ProtectionError::IncorrectPassword)
            ),
            "{name}"
        );
        let user = unlock_pdf(&bytes, Some("reader")).unwrap();
        let context = user.protection.unwrap();
        assert_eq!(context.revision(), revision);
        assert!(!context.is_owner());
        let p = context.permissions();
        assert!(p.print && p.annotate && p.fill_forms);
        assert!(!p.edit_content && !p.copy_content && !p.assemble);
        assert_eq!(p.print_high_quality, revision == 2);
        assert!(Document::load_mem(&user.bytes)
            .unwrap()
            .encryption_state
            .is_none());
        let owner = unlock_pdf(&bytes, Some("owner"))
            .unwrap()
            .protection
            .unwrap();
        assert!(owner.is_owner());
        let p = owner.permissions();
        assert!(p.edit_content && p.copy_content && p.assemble && p.print_high_quality);
    }
}

#[test]
fn content_editing_does_not_grant_separate_assembly_rights() {
    for (name, revision) in CASES {
        let bytes = fixture(&format!("edit_only_{name}"));
        let user = unlock_pdf(&bytes, Some("reader"))
            .unwrap()
            .protection
            .unwrap();
        assert!(user.permissions().edit_content);
        assert_eq!(user.permissions().assemble, revision == 2, "{name}");
        let owner = unlock_pdf(&bytes, Some("owner"))
            .unwrap()
            .protection
            .unwrap();
        assert!(owner.permissions().assemble);
    }
}

#[test]
fn rejects_invalid_crypt_filter_types_before_authentication() {
    for name in ["aes_128", "aes_256", "empty_user"] {
        for value in [Object::Name(b"Wrong".to_vec()), Object::Integer(1)] {
            let mut doc = Document::load_mem_with_options(
                &fixture(name),
                LoadOptions {
                    skip_decryption: true,
                    ..Default::default()
                },
            )
            .unwrap();
            let id = doc.trailer.get(b"Encrypt").unwrap().as_reference().unwrap();
            doc.get_dictionary_mut(id)
                .unwrap()
                .get_mut(b"CF")
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .get_mut(b"StdCF")
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set("Type", value);
            let mut changed = vec![];
            doc.save_to(&mut changed).unwrap();
            for password in [None, Some("reader"), Some("owner")] {
                assert!(
                    matches!(
                        unlock_pdf(&changed, password),
                        Err(ProtectionError::Invalid(_))
                    ),
                    "{name}"
                );
            }
        }
    }
}

#[test]
fn transformed_documents_keep_original_credentials_permissions_and_identifier() {
    for (name, _) in CASES {
        let source = fixture(name);
        let unlocked = unlock_pdf(&source, Some("reader")).unwrap();
        let context = unlocked.protection.unwrap();
        let mut doc = Document::load_mem(&unlocked.bytes).unwrap();
        let original_id = doc.trailer.get(b"ID").unwrap().as_array().unwrap()[0].clone();
        doc.renumber_objects_with(100);
        doc.trailer.set(
            "ID",
            vec![
                Object::string_literal("different-id-first"),
                Object::string_literal("different-id-last"),
            ],
        );
        let info = doc.trailer.get(b"Info").unwrap().as_reference().unwrap();
        doc.get_dictionary_mut(info)
            .unwrap()
            .set("Title", Object::string_literal("Edited protected copy"));
        let mut edited = vec![];
        doc.save_to(&mut edited).unwrap();
        let encrypted = context.protect(&edited).unwrap();
        assert!(matches!(
            unlock_pdf(&encrypted, None),
            Err(ProtectionError::PasswordRequired)
        ));
        assert!(matches!(
            unlock_pdf(&encrypted, Some("wrong")),
            Err(ProtectionError::IncorrectPassword)
        ));
        for password in ["reader", "owner"] {
            let next = unlock_pdf(&encrypted, Some(password)).unwrap();
            assert_eq!(
                next.protection.as_ref().unwrap().is_owner(),
                password == "owner"
            );
            if password == "reader" {
                assert_eq!(
                    next.protection.as_ref().unwrap().permissions(),
                    context.permissions()
                );
            }
            let result = Document::load_mem(&next.bytes).unwrap();
            assert_eq!(
                result.trailer.get(b"ID").unwrap().as_array().unwrap()[0],
                original_id
            );
            assert_eq!(
                result
                    .get_dictionary(result.trailer.get(b"Info").unwrap().as_reference().unwrap())
                    .unwrap()
                    .get(b"Title")
                    .unwrap()
                    .as_str()
                    .unwrap(),
                b"Edited protected copy",
                "{name}, {password}"
            );
        }
        if let Some(output) = std::env::var_os("FOLIO_PROTECTION_OUTPUT") {
            fs::create_dir_all(&output).unwrap();
            fs::write(PathBuf::from(output).join(format!("{name}.pdf")), encrypted).unwrap();
        }
    }
}

#[test]
fn rejects_corrupt_aes_ciphertext_instead_of_publishing_undecoded_objects() {
    let mut bytes = fixture("aes_128");
    let start = bytes.windows(7).position(|w| w == b"stream\n").unwrap() + 7;
    let length = bytes[start..]
        .windows(10)
        .position(|w| w == b"\nendstream")
        .unwrap();
    assert!(length > 32 && length % 16 == 0);
    // Alter the preceding CBC block's final byte, making the last padding byte invalid.
    bytes[start + length - 17] ^= 0xff;
    assert!(matches!(
        unlock_pdf(&bytes, Some("reader")),
        Err(ProtectionError::Invalid(_))
    ));
}

#[test]
fn encrypts_strings_in_stream_dictionaries() {
    let unlocked = unlock_pdf(&fixture("aes_256"), Some("owner")).unwrap();
    let mut doc = Document::load_mem(&unlocked.bytes).unwrap();
    let (_, object) = doc
        .objects
        .iter_mut()
        .find(|(_, o)| o.as_stream().is_ok())
        .unwrap();
    object.as_stream_mut().unwrap().dict.set(
        "FolioSecretLabel",
        Object::string_literal("private-stream-dictionary-value"),
    );
    let mut plain = vec![];
    doc.save_to(&mut plain).unwrap();
    let encrypted = unlocked.protection.unwrap().protect(&plain).unwrap();
    output("stream_dictionary", &encrypted);
    assert!(!encrypted
        .windows(b"private-stream-dictionary-value".len())
        .any(|w| w == b"private-stream-dictionary-value"));
    let restored = unlock_pdf(&encrypted, Some("reader")).unwrap();
    let doc = Document::load_mem(&restored.bytes).unwrap();
    assert!(doc
        .objects
        .values()
        .filter_map(|o| o.as_stream().ok())
        .any(|s| s
            .dict
            .get(b"FolioSecretLabel")
            .ok()
            .and_then(|o| o.as_str().ok())
            == Some(b"private-stream-dictionary-value")));
}

#[test]
fn empty_user_unicode_and_unprotected_bytes_are_handled_explicitly() {
    let bytes = fixture("empty_user");
    let result = unlock_pdf(&bytes, None).unwrap();
    assert!(!result.protection.unwrap().is_owner());
    assert!(unlock_pdf(&bytes, Some("owner"))
        .unwrap()
        .protection
        .unwrap()
        .is_owner());
    assert!(matches!(
        unlock_pdf(&bytes, Some("wrong")),
        Err(ProtectionError::IncorrectPassword)
    ));
    let unicode = unlock_pdf(&fixture("unicode"), Some("café-读者")).unwrap();
    let saved = unicode
        .protection
        .as_ref()
        .unwrap()
        .protect(&unicode.bytes)
        .unwrap();
    assert!(unlock_pdf(&saved, Some("拥有者-é"))
        .unwrap()
        .protection
        .unwrap()
        .is_owner());
    let plain = fixture("plain");
    let opened = unlock_pdf(&plain, None).unwrap();
    assert!(opened.protection.is_none());
    assert_eq!(plain, opened.bytes);
}

#[test]
fn rejects_unsupported_or_malformed_protection_and_redacts_diagnostics() {
    for name in ["aes_128", "empty_user"] {
        let source = fixture(name);
        for (key, value) in [
            ("Filter", Object::Name(b"Adobe.PubSec".to_vec())),
            ("EFF", Object::Name(b"AttachmentsOnly".to_vec())),
            ("P", Object::string_literal("invalid")),
            ("O", Object::string_literal("short")),
        ] {
            let mut d = Document::load_mem_with_options(
                &source,
                LoadOptions {
                    skip_decryption: true,
                    ..Default::default()
                },
            )
            .unwrap();
            let id = d.trailer.get(b"Encrypt").unwrap().as_reference().unwrap();
            d.get_dictionary_mut(id).unwrap().set(key, value);
            let mut changed = vec![];
            d.save_to(&mut changed).unwrap();
            assert!(
                matches!(
                    unlock_pdf(&changed, Some("reader")),
                    Err(ProtectionError::Unsupported(_) | ProtectionError::Invalid(_))
                ),
                "{name} {key}"
            );
        }
    }
    assert!(matches!(
        unlock_pdf(&fixture("aes_256"), Some(&"x".repeat(1025))),
        Err(ProtectionError::Limit(_))
    ));
    let unlocked = unlock_pdf(&fixture("unicode"), Some("café-读者")).unwrap();
    let debug = format!("{unlocked:?}");
    for secret in [
        "café-读者",
        "拥有者-é",
        "file_encryption_key",
        "owner_value",
        "Protected Folio fixture",
    ] {
        assert!(!debug.contains(secret));
    }
    assert!(matches!(
        unlocked.protection.unwrap().protect(&fixture("empty_user")),
        Err(ProtectionError::Invalid(_))
    ));
}

#[test]
fn rejects_tampered_aes256_permissions_with_both_passwords() {
    let source = fixture("aes_256");
    let header = Document::load_mem_with_options(
        &source,
        LoadOptions {
            skip_decryption: true,
            ..Default::default()
        },
    )
    .unwrap();
    let original = header
        .get_encrypted()
        .unwrap()
        .get(b"P")
        .unwrap()
        .as_i64()
        .unwrap();
    for bit in [1i64 << 3, 1i64 << 24] {
        let old = format!("/P {original}");
        let new = format!("/P {}", original ^ bit);
        assert_eq!(old.len(), new.len());
        let offset = source
            .windows(old.len())
            .position(|w| w == old.as_bytes())
            .unwrap();
        let mut changed = source.clone();
        changed[offset..offset + old.len()].copy_from_slice(new.as_bytes());
        for password in ["reader", "owner"] {
            assert!(
                matches!(
                    unlock_pdf(&changed, Some(password)),
                    Err(ProtectionError::Invalid(_))
                ),
                "bit {bit}, {password}"
            );
        }
    }
}

fn output(name: &str, bytes: &[u8]) {
    if let Some(directory) = std::env::var_os("FOLIO_PROTECTION_OUTPUT") {
        fs::create_dir_all(&directory).unwrap();
        fs::write(PathBuf::from(directory).join(format!("{name}.pdf")), bytes).unwrap();
    }
}

#[test]
fn preserves_compressed_objects_and_indirect_extension_declarations() {
    let source = fixture("object_streams");
    let header = Document::load_mem_with_options(
        &source,
        LoadOptions {
            skip_decryption: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(header
        .reference_table
        .entries
        .values()
        .any(|entry| matches!(entry, lopdf::xref::XrefEntry::Compressed { .. })));
    for password in ["reader", "owner"] {
        let unlocked = unlock_pdf(&source, Some(password)).unwrap();
        let mut doc = Document::load_mem(&unlocked.bytes).unwrap();
        let adobe = doc.add_object(lopdf::dictionary! {"BaseVersion"=>"1.7","ExtensionLevel"=>12});
        let other = doc.add_object(lopdf::dictionary! {"BaseVersion"=>"1.7","ExtensionLevel"=>1});
        let extensions = doc.add_object(lopdf::dictionary! {"ADBE"=>adobe,"FOLIO"=>other});
        let root = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
        doc.get_dictionary_mut(root)
            .unwrap()
            .set("Extensions", extensions);
        let mut plain = vec![];
        doc.save_to(&mut plain).unwrap();
        output("object_streams_plain", &plain);
        let protected = unlocked.protection.unwrap().protect(&plain).unwrap();
        let next = unlock_pdf(&protected, Some("reader")).unwrap();
        let doc = Document::load_mem(&next.bytes).unwrap();
        let extensions = doc
            .catalog()
            .unwrap()
            .get(b"Extensions")
            .unwrap()
            .as_dict()
            .unwrap();
        assert!(extensions.has(b"FOLIO"));
        assert_eq!(
            extensions
                .get(b"ADBE")
                .unwrap()
                .as_dict()
                .unwrap()
                .get(b"ExtensionLevel")
                .unwrap()
                .as_i64()
                .unwrap(),
            12
        );
        assert!(doc
            .extract_text(&[1])
            .unwrap()
            .contains("Protected Folio fixture"));
        output("object_streams", &protected);
    }
}

#[test]
fn retains_clear_metadata_policy_and_signed_document_boundary() {
    use lopdf::encryption::{
        crypt_filters::Aes128CryptFilter, EncryptionState, EncryptionVersion, Permissions,
    };
    use std::{collections::BTreeMap, sync::Arc};
    let mut d = Document::load_mem(&fixture("plain")).unwrap();
    let metadata = d.add_object(lopdf::Stream::new(
        lopdf::dictionary! {"Type"=>"Metadata","Subtype"=>"XML",
        "FolioSecretLabel"=>Object::string_literal("private-metadata-dictionary-value")},
        b"<metadata>deliberately clear metadata</metadata>".to_vec(),
    ));
    let root = d.trailer.get(b"Root").unwrap().as_reference().unwrap();
    d.get_dictionary_mut(root)
        .unwrap()
        .set("Metadata", metadata);
    let mut filters: BTreeMap<Vec<u8>, Arc<dyn lopdf::encryption::crypt_filters::CryptFilter>> =
        BTreeMap::new();
    filters.insert(b"StdCF".to_vec(), Arc::new(Aes128CryptFilter));
    let state = EncryptionState::try_from(EncryptionVersion::V4 {
        document: &d,
        encrypt_metadata: false,
        crypt_filters: filters,
        stream_filter: b"StdCF".to_vec(),
        string_filter: b"StdCF".to_vec(),
        owner_password: "owner",
        user_password: "reader",
        permissions: Permissions::all(),
    })
    .unwrap();
    d.encrypt(&state).unwrap();
    let mut bytes = vec![];
    d.save_to(&mut bytes).unwrap();
    let unlocked = unlock_pdf(&bytes, Some("reader")).unwrap();
    let protected = unlocked
        .protection
        .as_ref()
        .unwrap()
        .protect(&unlocked.bytes)
        .unwrap();
    let header = Document::load_mem_with_options(
        &protected,
        LoadOptions {
            password: Some("reader".into()),
            ..Default::default()
        },
    )
    .unwrap();
    let stream = header.get_object(metadata).unwrap().as_stream().unwrap();
    assert_eq!(
        stream.dict.get(b"Filter").unwrap().as_name().unwrap(),
        b"Crypt"
    );
    assert_eq!(
        stream
            .dict
            .get(b"DecodeParms")
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"Name")
            .unwrap()
            .as_name()
            .unwrap(),
        b"Identity"
    );
    assert!(protected
        .windows(b"deliberately clear metadata".len())
        .any(|w| w == b"deliberately clear metadata"));
    assert!(!protected
        .windows(b"private-metadata-dictionary-value".len())
        .any(|w| w == b"private-metadata-dictionary-value"));
    let restored = unlock_pdf(&protected, Some("reader")).unwrap();
    let restored = Document::load_mem(&restored.bytes).unwrap();
    let metadata = restored
        .catalog()
        .unwrap()
        .get(b"Metadata")
        .unwrap()
        .as_reference()
        .unwrap();
    assert_eq!(
        restored
            .get_object(metadata)
            .unwrap()
            .as_stream()
            .unwrap()
            .dict
            .get(b"FolioSecretLabel")
            .unwrap()
            .as_str()
            .unwrap(),
        b"private-metadata-dictionary-value"
    );
    assert!(!restored
        .get_object(metadata)
        .unwrap()
        .as_stream()
        .unwrap()
        .dict
        .has(b"Filter"));
    output("metadata_clear", &protected);
    let mut edited = Document::load_mem(&unlocked.bytes).unwrap();
    edited.add_object(lopdf::dictionary!{"Type"=>"Sig","ByteRange"=>vec![0.into(),10.into(),20.into(),30.into()],"Contents"=>Object::string_literal("signature")});
    let mut signed = vec![];
    edited.save_to(&mut signed).unwrap();
    assert!(matches!(
        unlocked.protection.unwrap().protect(&signed),
        Err(ProtectionError::Unsupported(_))
    ));
}
