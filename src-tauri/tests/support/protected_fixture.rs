use lopdf::{
    encryption::{
        crypt_filters::{Aes128CryptFilter, CryptFilter},
        EncryptionState, EncryptionVersion, Permissions,
    },
    Document, Object,
};
use std::{collections::BTreeMap, sync::Arc};

/// Test-only independent creation of a protected input with precisely chosen rights.
pub fn encrypt(bytes: &[u8], permissions: Permissions) -> Vec<u8> {
    let mut document = Document::load_mem(bytes).unwrap();
    let id = Object::string_literal(uuid::Uuid::new_v4().as_bytes().to_vec());
    document.trailer.set("ID", vec![id.clone(), id]);
    let mut filters: BTreeMap<Vec<u8>, Arc<dyn CryptFilter>> = BTreeMap::new();
    filters.insert(b"StdCF".to_vec(), Arc::new(Aes128CryptFilter));
    let state = EncryptionState::try_from(EncryptionVersion::V4 {
        document: &document,
        encrypt_metadata: true,
        crypt_filters: filters,
        stream_filter: b"StdCF".to_vec(),
        string_filter: b"StdCF".to_vec(),
        owner_password: "owner",
        user_password: "reader",
        permissions,
    })
    .unwrap();
    document.encrypt(&state).unwrap();
    let mut bytes = vec![];
    document.save_to(&mut bytes).unwrap();
    bytes
}

pub fn record(name: &str, original: &[u8], output: &[u8]) {
    if let Some(root) = std::env::var_os("FOLIO_ENGINE_PROTECTION_OUTPUT") {
        let root = std::path::PathBuf::from(root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join(format!("{name}.source.pdf")), original).unwrap();
        std::fs::write(root.join(format!("{name}.output.pdf")), output).unwrap();
    }
}
