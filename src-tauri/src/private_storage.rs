//! Current-user Windows protection for private application payloads.
//! No plaintext fallback, disk I/O, frontend commands or retained passwords.
use sha2::{Digest, Sha256};

const MAGIC: &[u8; 16] = b"FOLIO-SEALED-V1\0";
pub(crate) const FAMILY: &[u8] = b"FOLIO-SEALED-";
pub(crate) const HEADER_LEN: usize = 65;
pub(crate) const MAX_OVERHEAD: u64 = 64 * 1024;
const MAX_PAYLOAD: usize = 512 * 1024 * 1024;
const PREFIX_LEN: usize = 25;
const INNER_LEN: usize = PREFIX_LEN + 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Purpose {
    Workspace = 1,
    Source = 2,
    Font = 3,
    ProtectionContext = 4,
    Image = 5,
    HistorySourceBlock = 6,
}
impl Purpose {
    fn entropy(self) -> &'static [u8] {
        match self {
            Self::Workspace => b"Folio recovery v1/workspace",
            Self::Source => b"Folio recovery v1/source",
            Self::Font => b"Folio recovery v1/font",
            Self::ProtectionContext => b"Folio recovery v1/protection context",
            Self::Image => b"Folio recovery v1/image",
            Self::HistorySourceBlock => b"Folio retained history v1/source block",
        }
    }
}

pub(crate) struct Sensitive(pub(crate) Vec<u8>);
impl Drop for Sensitive {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}
fn wipe(bytes: &mut [u8]) {
    for byte in bytes {
        // Volatile writes keep temporary plaintext clearing observable to the compiler.
        unsafe { std::ptr::write_volatile(byte, 0) };
    }
}

pub fn seal(purpose: Purpose, data: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() > MAX_PAYLOAD {
        return Err("Private recovery payload exceeds 512 MiB".into());
    }
    let mut inner = Sensitive(Vec::with_capacity(INNER_LEN + data.len()));
    inner.0.extend_from_slice(MAGIC);
    inner.0.push(purpose as u8);
    inner
        .0
        .extend_from_slice(&(data.len() as u64).to_le_bytes());
    inner.0.extend_from_slice(&Sha256::digest(data));
    inner.0.extend_from_slice(data);
    let ciphertext = platform_crypt(true, purpose, &inner.0, data.len() + MAX_OVERHEAD as usize)?;
    if ciphertext.len() + HEADER_LEN > data.len() + MAX_OVERHEAD as usize {
        return Err("Private recovery envelope exceeds its overhead limit".into());
    }
    let mut result = Vec::with_capacity(HEADER_LEN + ciphertext.len());
    result.extend_from_slice(&inner.0[..PREFIX_LEN]);
    result.extend_from_slice(&(ciphertext.len() as u64).to_le_bytes());
    result.extend_from_slice(&Sha256::digest(&ciphertext));
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

pub fn unseal(purpose: Purpose, bytes: &[u8], limit: usize) -> Result<Vec<u8>, String> {
    let length = plaintext_length(purpose, bytes, bytes.len() as u64, limit)?;
    // DPAPI may ignore changes to descriptive blob-header fields. Detect accidental
    // corruption of the complete stored representation, in addition to its protected
    // payload checks. This outer checksum is not an authentication mechanism.
    if bytes[33..65] != Sha256::digest(&bytes[HEADER_LEN..])[..] {
        return Err("Private recovery ciphertext checksum failed".into());
    }
    let inner = Sensitive(platform_crypt(
        false,
        purpose,
        &bytes[HEADER_LEN..],
        length + INNER_LEN,
    )?);
    if inner.0.len() != length + INNER_LEN
        || inner.0[..PREFIX_LEN] != bytes[..PREFIX_LEN]
        || inner.0[PREFIX_LEN..INNER_LEN] != Sha256::digest(&inner.0[INNER_LEN..])[..]
    {
        return Err("Private recovery integrity check failed".into());
    }
    Ok(inner.0[INNER_LEN..].to_vec())
}

// Header-only checks bound reads and support immutable-source reuse. They are not
// authentication; unseal always verifies the protected inner header and checksum.
pub(crate) fn plaintext_length(
    purpose: Purpose,
    header: &[u8],
    stored_size: u64,
    limit: usize,
) -> Result<usize, String> {
    if header.len() < HEADER_LEN || &header[..16] != MAGIC || header[16] != purpose as u8 {
        return Err("Private recovery envelope version or purpose is invalid".into());
    }
    let length = u64::from_le_bytes(header[17..25].try_into().unwrap());
    let ciphertext = u64::from_le_bytes(header[25..33].try_into().unwrap());
    if length > limit.min(MAX_PAYLOAD) as u64
        || ciphertext == 0
        || stored_size < HEADER_LEN as u64
        || ciphertext != stored_size - HEADER_LEN as u64
        || stored_size > length + MAX_OVERHEAD
    {
        return Err("Private recovery envelope length is invalid or exceeds its limit".into());
    }
    Ok(length as usize)
}

#[cfg(windows)]
fn platform_crypt(
    encrypt: bool,
    purpose: Purpose,
    bytes: &[u8],
    output_limit: usize,
) -> Result<Vec<u8>, String> {
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        },
    };
    struct Output {
        blob: CRYPT_INTEGER_BLOB,
        sensitive: bool,
    }
    impl Drop for Output {
        fn drop(&mut self) {
            if !self.blob.pbData.is_null() {
                unsafe {
                    if self.sensitive {
                        wipe(std::slice::from_raw_parts_mut(
                            self.blob.pbData,
                            self.blob.cbData as usize,
                        ));
                    }
                    LocalFree(self.blob.pbData.cast());
                }
            }
        }
    }
    let size = u32::try_from(bytes.len()).map_err(|_| "Private recovery input is too large")?;
    let input = CRYPT_INTEGER_BLOB {
        cbData: size,
        pbData: bytes.as_ptr().cast_mut(),
    };
    let entropy_bytes = purpose.entropy();
    let entropy = CRYPT_INTEGER_BLOB {
        cbData: entropy_bytes.len() as u32,
        pbData: entropy_bytes.as_ptr().cast_mut(),
    };
    let mut output = Output {
        blob: CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: null_mut(),
        },
        sensitive: !encrypt,
    };
    // DPAPI reads the borrowed inputs and owns the returned LocalAlloc buffer.
    // No LOCAL_MACHINE flag: protection remains bound to this Windows user.
    let ok = unsafe {
        if encrypt {
            CryptProtectData(
                &input,
                null(),
                &entropy,
                null(),
                null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output.blob,
            )
        } else {
            CryptUnprotectData(
                &input,
                null_mut(),
                &entropy,
                null(),
                null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output.blob,
            )
        }
    };
    if ok == 0 {
        return Err(if encrypt {
            "Windows could not protect private recovery data; nothing will be written in plaintext"
        } else {
            "Private recovery cannot be unlocked by this Windows account, or its data is damaged"
        }
        .into());
    }
    if output.blob.pbData.is_null() || output.blob.cbData as usize > output_limit {
        return Err("Windows returned an invalid private recovery payload".into());
    }
    Ok(
        unsafe { std::slice::from_raw_parts(output.blob.pbData, output.blob.cbData as usize) }
            .to_vec(),
    )
}

#[cfg(not(windows))]
fn platform_crypt(_: bool, _: Purpose, _: &[u8], _: usize) -> Result<Vec<u8>, String> {
    Err(
        "Account-protected recovery currently requires Windows; plaintext storage is disabled"
            .into(),
    )
}
