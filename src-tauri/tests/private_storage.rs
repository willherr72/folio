#![cfg(windows)]
use folio_engine::private_storage::{seal, unseal, Purpose};

#[test]
fn account_protection_roundtrips_binary_unicode_and_empty_data() {
    for value in [
        b"".as_slice(),
        "Private note: café 读者".as_bytes(),
        &[0, 255, 13, 10],
    ] {
        for purpose in [Purpose::Workspace, Purpose::Source, Purpose::Font, Purpose::Image] {
            let first = seal(purpose, value).unwrap();
            let second = seal(purpose, value).unwrap();
            assert_ne!(first, second, "DPAPI must use fresh ciphertext");
            assert_eq!(unseal(purpose, &first, value.len()).unwrap(), value);
            if !value.is_empty() {
                assert!(unseal(purpose, &first, value.len() - 1).is_err());
            }
        }
    }
}

#[test]
fn rejects_wrong_purpose_and_plaintext_instead_of_falling_back() {
    let sealed = seal(Purpose::Source, b"private source bytes").unwrap();
    assert!(unseal(Purpose::Workspace, &sealed, 100).is_err());
    assert!(unseal(Purpose::Font, &sealed, 100).is_err());
    assert!(unseal(Purpose::Source, b"%PDF-1.7 plaintext", 100).is_err());
    assert!(unseal(Purpose::Source, &[], 100).is_err());
}

#[test]
fn outer_checksum_is_not_the_only_integrity_check() {
    use sha2::{Digest, Sha256};
    let mut bytes = seal(
        Purpose::Source,
        b"protected content must remain authenticated",
    )
    .unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    let digest = Sha256::digest(&bytes[65..]);
    bytes[33..65].copy_from_slice(&digest);
    assert!(unseal(Purpose::Source, &bytes, 100).is_err());
}

#[test]
fn rejects_header_and_ciphertext_tampering_and_truncation() {
    let sealed = seal(Purpose::Workspace, b"sensitive workspace contents").unwrap();
    for offset in 0..sealed.len() {
        let mut changed = sealed.clone();
        changed[offset] ^= 0x80;
        assert!(
            unseal(Purpose::Workspace, &changed, 100).is_err(),
            "offset {offset}"
        );
    }
    for length in [0, 1, 16, 32, sealed.len() - 1] {
        assert!(unseal(Purpose::Workspace, &sealed[..length], 100).is_err());
    }
    let mut trailing = sealed.clone();
    trailing.push(0);
    assert!(unseal(Purpose::Workspace, &trailing, 100).is_err());
}

#[test]
fn errors_never_include_payload_and_large_binary_roundtrip_is_bounded() {
    let payload = b"private-not-for-diagnostics";
    let sealed = seal(Purpose::Font, payload).unwrap();
    let error = unseal(Purpose::Source, &sealed, 100).unwrap_err();
    assert!(!error.contains("private-not-for-diagnostics"));
    let large = vec![0xA5; 2 * 1024 * 1024];
    let sealed = seal(Purpose::Font, &large).unwrap();
    assert_eq!(unseal(Purpose::Font, &sealed, large.len()).unwrap(), large);
}
