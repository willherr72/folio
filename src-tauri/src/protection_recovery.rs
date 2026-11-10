//! Authenticated private context transport. No password or plaintext file storage.
use super::{
    authentication_document, validate_dictionary, PdfProtection, ProtectionError, Result, MAX_BYTES,
};
use crate::private_storage::{self, Purpose, Sensitive};
use lopdf::{dictionary, encryption::EncryptionState, Document, Object};
use sha2::{Digest, Sha256};

const MAGIC: &[u8] = b"FOLIO-PDF-CONTEXT-V1\0";
const HEADER: usize = MAGIC.len() + 2 + 32;
const METADATA_LIMIT: usize = 64 * 1024;
const CONTEXT_LIMIT: usize = METADATA_LIMIT + 128;

fn invalid() -> ProtectionError {
    ProtectionError::Invalid("private authentication context is invalid or unavailable")
}
fn source_limit(source: &[u8]) -> Result<()> {
    if source.is_empty() || source.len() > MAX_BYTES {
        Err(ProtectionError::Limit(
            "recovery source must be between 1 byte and 512 MiB",
        ))
    } else {
        Ok(())
    }
}

// Context dictionaries are self-contained. Never serialize dangling references or
// streams from the original PDF into the tiny recovery metadata document.
fn bounded_value(value: &Object, depth: usize, nodes: &mut usize) -> Result<()> {
    *nodes += 1;
    if depth > 32 || *nodes > 4096 {
        return Err(invalid());
    }
    match value {
        Object::Array(values) => {
            for value in values {
                bounded_value(value, depth + 1, nodes)?;
            }
        }
        Object::Dictionary(dictionary) => {
            for (_, value) in dictionary.iter() {
                bounded_value(value, depth + 1, nodes)?;
            }
        }
        Object::Reference(_) | Object::Stream(_) => return Err(invalid()),
        Object::Real(value) if !value.is_finite() => return Err(invalid()),
        _ => (),
    }
    Ok(())
}

impl PdfProtection {
    /// Seal the retained file key and permissions for this exact recovery source.
    /// The returned bytes are account-protected ciphertext, never a password.
    pub fn seal_recovery_context(&self, source: &[u8]) -> Result<Vec<u8>> {
        source_limit(source)?;
        bounded_value(&Object::Dictionary(self.dictionary.clone()), 0, &mut 0)?;
        let key = self.state.file_encryption_key();
        if !(5..=32).contains(&key.len()) {
            return Err(invalid());
        }
        let mut metadata = Document::with_version(&self.version);
        // Keep this private two-object format stable across source PDF versions.
        // lopdf otherwise adds a third object for its default cross-reference stream.
        metadata.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceTable;
        let pages = metadata
            .add_object(dictionary! {"Type"=>"Pages", "Count"=>0, "Kids"=>Vec::<Object>::new()});
        let root = metadata.add_object(dictionary! {"Type"=>"Catalog", "Pages"=>pages,
        "FolioProtection"=>self.dictionary.clone()});
        metadata.trailer.set("Root", root);
        metadata.trailer.set("ID", self.id.clone());
        let mut writer = crate::pdf_output::BoundedWriter::new(METADATA_LIMIT);
        let saved = metadata.save_to(&mut writer);
        let metadata = writer
            .finish()
            .map_err(|_| ProtectionError::Limit("recovery protection metadata exceeds 64 KiB"))?;
        saved.map_err(|_| invalid())?;
        let mut inner = Sensitive(Vec::with_capacity(HEADER + key.len() + metadata.len()));
        inner.0.extend_from_slice(MAGIC);
        inner.0.push(u8::from(self.owner));
        inner.0.push(key.len() as u8);
        inner.0.extend_from_slice(&Sha256::digest(source));
        inner.0.extend_from_slice(key);
        inner.0.extend_from_slice(&metadata);
        private_storage::seal(Purpose::ProtectionContext, &inner.0).map_err(|_| invalid())
    }

    /// Restore only from account-authenticated state bound to the supplied source.
    /// User-supplied PDFs still require normal password authentication.
    pub fn from_recovery_context(sealed: &[u8], source: &[u8]) -> Result<Self> {
        source_limit(source)?;
        let inner = Sensitive(
            private_storage::unseal(Purpose::ProtectionContext, sealed, CONTEXT_LIMIT)
                .map_err(|_| invalid())?,
        );
        let bytes = &inner.0;
        if bytes.len() < HEADER || !bytes.starts_with(MAGIC) {
            return Err(invalid());
        }
        let owner = bytes[MAGIC.len()];
        let key_length = bytes[MAGIC.len() + 1] as usize;
        if owner > 1
            || !(5..=32).contains(&key_length)
            || bytes.len() <= HEADER + key_length
            || bytes[MAGIC.len() + 2..HEADER] != Sha256::digest(source)[..]
        {
            return Err(invalid());
        }
        let metadata = &bytes[HEADER + key_length..];
        if metadata.len() > METADATA_LIMIT {
            return Err(invalid());
        }
        let metadata = Document::load_mem_with_options(
            metadata,
            lopdf::LoadOptions {
                strict: true,
                skip_decryption: true,
                max_decompressed_size: Some(METADATA_LIMIT),
                ..Default::default()
            },
        )
        .map_err(|_| invalid())?;
        if metadata.trailer.has(b"Encrypt")
            || metadata.objects.len() != 2
            || !matches!(
                metadata.version.as_str(),
                "1.0" | "1.1" | "1.2" | "1.3" | "1.4" | "1.5" | "1.6" | "1.7" | "2.0"
            )
        {
            return Err(invalid());
        }
        let dictionary = metadata
            .catalog()
            .and_then(|d| d.get(b"FolioProtection"))
            .and_then(Object::as_dict)
            .map_err(|_| invalid())?
            .clone();
        bounded_value(&Object::Dictionary(dictionary.clone()), 0, &mut 0)?;
        validate_dictionary(&dictionary)?;
        let id = metadata.trailer.get(b"ID").map_err(|_| invalid())?.clone();
        let ids = id.as_array().map_err(|_| invalid())?;
        if ids.len() != 2
            || ids
                .iter()
                .any(|o| o.as_str().map_or(true, |s| s.is_empty() || s.len() > 1024))
        {
            return Err(invalid());
        }
        let authentication = authentication_document(&dictionary, &id)?;
        let state = EncryptionState::from_authenticated_file_key(
            &authentication,
            &bytes[HEADER..HEADER + key_length],
        )
        .map_err(|_| invalid())?;
        Ok(Self {
            state,
            dictionary,
            id,
            owner: owner == 1,
            version: metadata.version,
        })
    }
}
