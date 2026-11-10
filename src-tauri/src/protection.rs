//! In-memory protection preservation. Callers must enforce permissions, protect
//! recovery state and encrypt output before publishing it. This is not an ACL.
use lopdf::{
    dictionary, encryption::EncryptionState, xref::XrefEntry, Dictionary, Document, LoadOptions,
    Object,
};
use std::{
    fmt,
    io::{self, Write},
};

#[path = "protection_recovery.rs"]
mod recovery;
#[path = "protection_output.rs"]
mod output;
pub use output::ProtectionChange;

const MAX_BYTES: usize = 512 * 1024 * 1024;
const MAX_STREAM: usize = 64 * 1024 * 1024;
const MAX_OBJECTS: usize = 100_000;
const MAX_PASSWORD: usize = 1024;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProtectionError {
    #[error("This PDF requires a password.")]
    PasswordRequired,
    #[error("The PDF password is incorrect.")]
    IncorrectPassword,
    #[error("Unsupported PDF protection: {0}")]
    Unsupported(&'static str),
    #[error("Invalid protected PDF: {0}")]
    Invalid(&'static str),
    #[error("PDF protection limit: {0}")]
    Limit(&'static str),
}
type Result<T> = std::result::Result<T, ProtectionError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DocumentPermissions {
    pub print: bool,
    pub print_high_quality: bool,
    pub copy_content: bool,
    pub edit_content: bool,
    pub annotate: bool,
    pub fill_forms: bool,
    pub assemble: bool,
}

pub struct UnlockedPdf {
    pub bytes: Vec<u8>,
    pub protection: Option<PdfProtection>,
}
impl fmt::Debug for UnlockedPdf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnlockedPdf")
            .field("byte_length", &self.bytes.len())
            .field("protection", &self.protection)
            .finish()
    }
}

#[derive(Clone)]
pub struct PdfProtection {
    state: EncryptionState,
    dictionary: Dictionary,
    id: Object,
    owner: bool,
    version: String,
}
impl fmt::Debug for PdfProtection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PdfProtection")
            .field("revision", &self.revision())
            .field("owner", &self.owner)
            .finish_non_exhaustive()
    }
}
impl PdfProtection {
    pub(crate) fn same_encryption(&self, other: &Self) -> bool {
        self.dictionary == other.dictionary
            && self.id.as_array().ok().and_then(|v|v.first()) == other.id.as_array().ok().and_then(|v|v.first())
            && self.state.file_encryption_key() == other.state.file_encryption_key()
    }
    pub fn revision(&self) -> i64 {
        self.state.revision()
    }
    pub fn is_owner(&self) -> bool {
        self.owner
    }
    /// Reauthenticate retained credentials without changing the existing capability
    /// or the permissions/credentials written into future saved copies.
    pub(crate) fn authenticate_owner(&self, password:&str) -> Result<Self> {
        if password.len()>MAX_PASSWORD {return Err(ProtectionError::Limit("password exceeds 1024 bytes"));}
        let auth=authentication_document(&self.dictionary,&self.id)?;
        auth.authenticate_owner_password(password).map_err(|_|ProtectionError::IncorrectPassword)?;
        let mut owner=self.clone();owner.owner=true;Ok(owner)
    }
    pub fn permissions(&self) -> DocumentPermissions {
        let p = self.state.permissions().bits();
        let has = |bit: u32| self.owner || (p & (1u64 << (bit - 1))) != 0u64;
        DocumentPermissions {
            print: has(3),
            print_high_quality: has(3) && (self.revision() == 2 || has(12)),
            copy_content: has(5),
            edit_content: has(4),
            annotate: has(6),
            fill_forms: has(6) || (self.revision() >= 3 && has(9)),
            assemble: has(if self.revision() == 2 { 4 } else { 11 }),
        }
    }
    /// Preserve credentials/rights on an already-authorized transformed plaintext
    /// document. Deliberately does not grant permission to perform that transform.
    pub fn protect(&self, bytes: &[u8]) -> Result<Vec<u8>> {
        let mut doc = load(bytes, None, true)?;
        if doc.trailer.has(b"Encrypt") || doc.encryption_state.is_some() {
            return Err(ProtectionError::Invalid(
                "expected an unencrypted editing document",
            ));
        }
        validate_loaded(&doc, None)?;
        let first = self
            .id
            .as_array()
            .map_err(|_| ProtectionError::Invalid("invalid retained identifier"))?[0]
            .clone();
        doc.trailer.set(
            "ID",
            vec![
                first,
                Object::String(
                    uuid::Uuid::new_v4().as_bytes().to_vec(),
                    lopdf::StringFormat::Hexadecimal,
                ),
            ],
        );
        if doc.version.as_str() < self.version.as_str() {
            doc.version = self.version.clone();
        }
        if self.revision() >= 5 {
            if doc.version.as_str() < "1.7" {
                doc.version = "1.7".into();
            }
            // AES-256 in PDF1.7 uses the Adobe extension declaration. PDF2.0
            // includes it directly; preserving this declaration is harmless there.
            let root = doc
                .trailer
                .get(b"Root")
                .and_then(Object::as_reference)
                .map_err(|_| ProtectionError::Invalid("missing catalog"))?;
            let catalog = doc
                .get_dictionary(root)
                .map_err(|_| ProtectionError::Invalid("invalid catalog"))?;
            let mut extensions = match catalog.get(b"Extensions") {
                Ok(value) => doc
                    .dereference(value)
                    .and_then(|(_, o)| o.as_dict())
                    .cloned()
                    .map_err(|_| ProtectionError::Invalid("invalid extensions dictionary"))?,
                Err(_) => Dictionary::new(),
            };
            let minimum = if self.revision() == 5 { 3 } else { 8 };
            let level = extensions
                .get(b"ADBE")
                .ok()
                .and_then(|o| doc.dereference(o).ok())
                .and_then(|(_, o)| o.as_dict().ok())
                .and_then(|d| d.get(b"ExtensionLevel").ok())
                .and_then(|o| o.as_i64().ok())
                .unwrap_or(0)
                .max(minimum);
            extensions.set(
                "ADBE",
                dictionary! {"BaseVersion"=>"1.7","ExtensionLevel"=>level},
            );
            doc.get_dictionary_mut(root)
                .map_err(|_| ProtectionError::Invalid("invalid catalog"))?
                .set("Extensions", extensions);
        }
        if !self.state.encrypt_metadata() {
            prepare_clear_metadata(&mut doc, true)?;
        }
        doc.encrypt(&self.state)
            .map_err(|_| ProtectionError::Invalid("could not preserve encryption"))?;
        // Keep original crypt-filter parameters and the signed /P representation.
        let encrypt = doc
            .trailer
            .get(b"Encrypt")
            .and_then(Object::as_reference)
            .map_err(|_| ProtectionError::Invalid("missing output encryption"))?;
        doc.objects
            .insert(encrypt, Object::Dictionary(self.dictionary.clone()));
        save(&mut doc)
    }
}

pub fn unlock_pdf(bytes: &[u8], password: Option<&str>) -> Result<UnlockedPdf> {
    if password.is_some_and(|p| p.len() > MAX_PASSWORD) {
        return Err(ProtectionError::Limit("password exceeds 1,024 UTF-8 bytes"));
    }
    let mut doc = load(bytes, None, true)?;
    if !doc.trailer.has(b"Encrypt") && doc.encryption_state.is_none() {
        return Ok(UnlockedPdf {
            bytes: bytes.to_vec(),
            protection: None,
        });
    }
    let encryption_id = doc
        .trailer
        .get(b"Encrypt")
        .ok()
        .and_then(|o| o.as_reference().ok())
        .or_else(|| {
            doc.encryption_state
                .as_ref()
                .and_then(EncryptionState::encrypt_object_id)
        });
    let dictionary = doc
        .get_encrypted()
        .cloned()
        .map_err(|_| ProtectionError::Invalid("could not read original encryption dictionary"))?;
    validate_dictionary(&dictionary)?;
    let id = doc
        .trailer
        .get(b"ID")
        .map_err(|_| ProtectionError::Invalid("missing document identifier"))?
        .clone();
    let ids = id
        .as_array()
        .map_err(|_| ProtectionError::Invalid("invalid document identifier"))?;
    if ids.len() != 2
        || ids
            .iter()
            .any(|o| o.as_str().map_or(true, |s| s.is_empty() || s.len() > 1024))
    {
        return Err(ProtectionError::Invalid("invalid document identifier"));
    }
    // Authenticate the actual supplied password, even when the library has already
    // opened a file with its empty user password. Its convenience loader prefers
    // empty passwords; that must not turn an incorrect owner password into success.
    let auth = authentication_document(&dictionary, &id)?;
    let candidate = password.unwrap_or("");
    let owner = auth.authenticate_owner_password(candidate).is_ok();
    if !owner && auth.authenticate_user_password(candidate).is_err() {
        return Err(if password.is_none() {
            ProtectionError::PasswordRequired
        } else {
            ProtectionError::IncorrectPassword
        });
    }
    if doc.encryption_state.is_none() {
        doc = load(bytes, Some(candidate), false)?;
    }
    let state = doc.encryption_state.take().ok_or(ProtectionError::Invalid(
        "password did not unlock the document",
    ))?;
    if !state.encrypt_metadata() {
        prepare_clear_metadata(&mut doc, false)?;
    }
    validate_loaded(&doc, encryption_id)?;
    let protection = PdfProtection {
        state,
        dictionary,
        id,
        owner,
        version: doc.version.clone(),
    };
    Ok(UnlockedPdf {
        bytes: save(&mut doc)?,
        protection: Some(protection),
    })
}

fn authentication_document(dictionary: &Dictionary, id: &Object) -> Result<Document> {
    let mut auth = Document::new();
    auth.trailer.set("ID", id.clone());
    let enc = auth.add_object(dictionary.clone());
    auth.trailer.set("Encrypt", enc);
    if let Ok(filters) = dictionary.get(b"CF").and_then(Object::as_dict) {
        let decoded = auth.get_crypt_filters();
        for key in [b"StmF", b"StrF"] {
            let name = dictionary
                .get(key)
                .and_then(Object::as_name)
                .map_err(|_| ProtectionError::Invalid("invalid default crypt filter"))?;
            let expected = filters
                .get(name)
                .and_then(Object::as_dict)
                .and_then(|f| f.get(b"CFM"))
                .and_then(Object::as_name)
                .map_err(|_| ProtectionError::Invalid("invalid crypt filter method"))?;
            if decoded.get(name).map(|f| f.method()) != Some(expected) {
                return Err(ProtectionError::Invalid(
                    "crypt filter could not be decoded",
                ));
            }
        }
    }
    lopdf::encryption::PasswordAlgorithm::try_from(&auth)
        .map_err(|_| ProtectionError::Invalid("invalid password or permission metadata"))?;
    Ok(auth)
}

// ISO32000-1 section7.6.5: clear metadata uses an explicit Identity Crypt filter.
// Remove that transport declaration for plaintext editing, restore it on protection.
// Other per-stream crypt overrides remain unsupported.
fn prepare_clear_metadata(doc: &mut Document, encrypting: bool) -> Result<()> {
    for object in doc.objects.values_mut() {
        let Object::Stream(stream) = object else {
            continue;
        };
        if !stream.dict.has_type(b"Metadata") {
            continue;
        }
        if let Ok(filter) = stream.dict.get(b"Filter") {
            let single_crypt = filter.as_name().ok() == Some(b"Crypt")
                || filter
                    .as_array()
                    .is_ok_and(|a| a.len() == 1 && a[0].as_name().ok() == Some(b"Crypt"));
            let identity = stream
                .dict
                .get(b"DecodeParms")
                .and_then(Object::as_dict)
                .is_ok_and(|p| p.get(b"Name").and_then(Object::as_name).ok() == Some(b"Identity"));
            if !single_crypt || !identity {
                return Err(ProtectionError::Unsupported("filtered clear metadata"));
            }
        } else if stream.dict.has(b"DecodeParms") {
            return Err(ProtectionError::Invalid(
                "metadata parameters without a filter",
            ));
        }
        stream.dict.remove(b"Filter");
        stream.dict.remove(b"DecodeParms");
        if encrypting {
            stream.dict.set("Filter", Object::Name(b"Crypt".to_vec()));
            stream.dict.set(
                "DecodeParms",
                dictionary! {"Type"=>"CryptFilterDecodeParms", "Name"=>"Identity"},
            );
        }
    }
    Ok(())
}

fn load(bytes: &[u8], password: Option<&str>, skip_decryption: bool) -> Result<Document> {
    if bytes.len() > MAX_BYTES {
        return Err(ProtectionError::Limit("PDF exceeds 512 MiB"));
    }
    let doc = Document::load_mem_with_options(
        bytes,
        LoadOptions {
            password: password.map(str::to_owned),
            skip_decryption,
            max_decompressed_size: Some(MAX_STREAM),
            ..Default::default()
        },
    )
    .map_err(|_| ProtectionError::Invalid("could not read PDF structure or encryption"))?;
    if doc.objects.len() > MAX_OBJECTS
        || doc.reference_table.entries.len() > MAX_OBJECTS
        || doc.max_id > 1_000_000
    {
        return Err(ProtectionError::Limit(
            "too many PDF objects or oversized object identifiers",
        ));
    }
    Ok(doc)
}

fn validate_dictionary(d: &Dictionary) -> Result<()> {
    if d.get(b"Filter").and_then(Object::as_name).ok() != Some(b"Standard") {
        return Err(ProtectionError::Unsupported(
            "only the Standard password handler is supported",
        ));
    }
    let v = d
        .get(b"V")
        .and_then(Object::as_i64)
        .map_err(|_| ProtectionError::Invalid("missing encryption version"))?;
    let r = d
        .get(b"R")
        .and_then(Object::as_i64)
        .map_err(|_| ProtectionError::Invalid("missing encryption revision"))?;
    if !matches!((v, r), (1, 2) | (2, 3) | (4, 4) | (5, 5) | (5, 6)) {
        return Err(ProtectionError::Unsupported("encryption version/revision"));
    }
    if d.has(b"EFF") || d.has(b"SubFilter") {
        return Err(ProtectionError::Unsupported(
            "selective embedded-file or alternate encryption",
        ));
    }
    if v >= 4 {
        let filters = d
            .get(b"CF")
            .and_then(Object::as_dict)
            .map_err(|_| ProtectionError::Invalid("missing crypt filters"))?;
        for (_, value) in filters.iter() {
            let filter = value
                .as_dict()
                .map_err(|_| ProtectionError::Unsupported("indirect crypt filter"))?;
            if filter
                .get(b"Type")
                .is_ok_and(|t| t.as_name().ok() != Some(b"CryptFilter"))
            {
                return Err(ProtectionError::Invalid("invalid crypt filter type"));
            }
            let method = filter
                .get(b"CFM")
                .and_then(Object::as_name)
                .map_err(|_| ProtectionError::Invalid("missing crypt filter method"))?;
            if !(v == 4 && (method == b"AESV2" || method == b"V2") || v == 5 && method == b"AESV3")
            {
                return Err(ProtectionError::Unsupported("crypt filter method"));
            }
            if filter
                .get(b"AuthEvent")
                .is_ok_and(|event| event.as_name().ok() != Some(b"DocOpen"))
            {
                return Err(ProtectionError::Unsupported(
                    "crypt filter authentication event",
                ));
            }
        }
        for name in [b"StmF", b"StrF"] {
            let filter = d
                .get(name)
                .and_then(Object::as_name)
                .map_err(|_| ProtectionError::Invalid("missing default crypt filter"))?;
            if !filters.has(filter) {
                return Err(ProtectionError::Unsupported(
                    "identity or unresolved default crypt filter",
                ));
            }
        }
    }
    Ok(())
}

fn validate_loaded(doc: &Document, removed: Option<(u32, u16)>) -> Result<()> {
    for (number, entry) in &doc.reference_table.entries {
        let id = match entry {
            XrefEntry::Normal { generation, .. } => (*number, *generation),
            XrefEntry::Compressed { .. } => (*number, 0),
            _ => continue,
        };
        if Some(id) != removed && !doc.objects.contains_key(&id) {
            return Err(ProtectionError::Invalid(
                "a PDF object could not be decoded",
            ));
        }
    }
    fn walk(o: &Object, doc: &Document, depth: usize, budget: &mut usize) -> Result<()> {
        if depth > 64 || *budget == 0 {
            return Err(ProtectionError::Limit("PDF object graph exceeds limits"));
        }
        *budget -= 1;
        match o {
            Object::Reference(id) if !doc.objects.contains_key(id) => {
                return Err(ProtectionError::Invalid("unresolved PDF reference"))
            }
            Object::Array(values) => {
                for value in values {
                    walk(value, doc, depth + 1, budget)?;
                }
            }
            Object::Dictionary(d) => {
                if d.has_type(b"Sig") || (d.has(b"ByteRange") && d.has(b"Contents")) {
                    return Err(ProtectionError::Unsupported(
                        "signed documents require signature-aware handling",
                    ));
                }
                for (_, value) in d.iter() {
                    walk(value, doc, depth + 1, budget)?;
                }
            }
            Object::Stream(s) => {
                // The reader retains the original xref stream, whose /Encrypt
                // reference intentionally points to the removed security object.
                // The writer regenerates xrefs and never serializes this container.
                if s.dict.has_type(b"XRef") {
                    return Ok(());
                }
                let crypt = match s.dict.get(b"Filter") {
                    Ok(Object::Name(n)) => n == b"Crypt",
                    Ok(Object::Array(a)) => a.iter().any(|o| o.as_name().ok() == Some(b"Crypt")),
                    _ => false,
                };
                if crypt {
                    return Err(ProtectionError::Unsupported("per-stream Crypt filters"));
                }
                walk(&Object::Dictionary(s.dict.clone()), doc, depth + 1, budget)?;
            }
            _ => {}
        }
        Ok(())
    }
    let mut budget = 1_000_000;
    for object in doc.objects.values() {
        walk(object, doc, 0, &mut budget)?;
    }
    doc.catalog()
        .map_err(|_| ProtectionError::Invalid("missing PDF catalog"))?;
    Ok(())
}

struct BoundedBytes(Vec<u8>);
impl Write for BoundedBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.0.len().saturating_add(bytes.len()) > MAX_BYTES {
            return Err(io::Error::other("PDF output limit"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn save(doc: &mut Document) -> Result<Vec<u8>> {
    let mut output = BoundedBytes(vec![]);
    doc.save_to(&mut output)
        .map_err(|_| ProtectionError::Limit("could not serialize PDF within 512 MiB"))?;
    Ok(output.0)
}
