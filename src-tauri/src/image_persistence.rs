//! Original image attachments and bounded proof of their PDF drawing resources.
use crate::{prepare_pdf_image, ImageAsset, PreparedPdfImage};
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream};
use sha2::{Digest, Sha256};
use std::{borrow::Cow, collections::HashMap, sync::Arc};

const MAX_IMAGE_BYTES: usize = 64 * 1024 * 1024;
#[derive(Clone, Copy)]
struct ImportLimits {
    assets: usize,
    encoded: usize,
    prepared: usize,
    streams: usize,
}
const IMPORT_LIMITS: ImportLimits = ImportLimits {
    assets: 64,
    encoded: 256 * 1024 * 1024,
    prepared: 512 * 1024 * 1024,
    streams: 4096,
};

pub(crate) struct PreparedImageAsset {
    pub asset: Arc<ImageAsset>,
    pub prepared: Arc<PreparedPdfImage>,
}

/// A private, versioned attachment retains the original encoded file. It is not a
/// drawing resource and is omitted from flattened exports. PDF stream Length is
/// transport bookkeeping; Params/Size describes the decoded original-file bytes.
pub(crate) fn attachment(asset: &ImageAsset) -> Stream {
    Stream::new(attachment_dictionary(asset),asset.bytes().to_vec())
}
// The generated graph budget can inspect this small header and the borrowed
// original bytes before attachment() copies the encoded image into a stream.
pub(crate) fn attachment_dictionary(asset:&ImageAsset)->Dictionary {
    dictionary! {
            "Type"=>"EmbeddedFile", "Subtype"=>Object::Name(asset.info().mime_type.as_bytes().to_vec()),
            "FolioVersion"=>1, "FolioImageId"=>Object::string_literal(asset.info().id.clone()),
            "Params"=>dictionary! {"Size"=>asset.bytes().len() as i64},
    }
}

/// One cache borrows one immutable PDF for its entire lifetime. Object-ID cache
/// hits therefore cannot cross documents or survive mutation of the source graph.
/// Entries validate assets only; every calling appearance still needs full proof.
/// No registry publication or acquisition happens here.
pub(crate) struct ImageImportCache<'a> {
    document: &'a Document,
    assets: HashMap<String, Arc<PreparedImageAsset>>,
    streams: HashMap<ObjectId, Arc<PreparedImageAsset>>,
    encoded: usize,
    prepared: usize,
    limits: ImportLimits,
}
impl<'a> ImageImportCache<'a> {
    pub fn new(document: &'a Document) -> Self {
        Self::with_limits(document, IMPORT_LIMITS)
    }
    fn with_limits(document: &'a Document, limits: ImportLimits) -> Self {
        Self {
            document,
            assets: HashMap::new(),
            streams: HashMap::new(),
            encoded: 0,
            prepared: 0,
            limits,
        }
    }
    #[cfg(test)]
    pub fn verify(
        &mut self,
        file: &Object,
        expected_id: &str,
        form: &Object,
    ) -> Option<Arc<PreparedImageAsset>> {
        let asset = self.read_asset(file, expected_id)?;
        matches_form(self.document, form, asset.prepared.form()).then_some(asset)
    }
    pub fn read_asset(
        &mut self,
        file: &Object,
        expected_id: &str,
    ) -> Option<Arc<PreparedImageAsset>> {
        if expected_id.len() != 64
            || !expected_id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return None;
        }
        let id = file.as_reference().ok()?;
        if let Some(cached) = self.streams.get(&id) {
            return (cached.asset.info().id == expected_id).then(|| cached.clone());
        }
        if self.streams.len() >= self.limits.streams {
            return None;
        }
        let stream = resolve(self.document, file, &mut 64)?.as_stream().ok()?;
        let allowed: &[&[u8]] = &[
            b"Type",
            b"Subtype",
            b"FolioVersion",
            b"FolioImageId",
            b"Params",
            b"Length",
            b"Filter",
        ];
        if stream
            .dict
            .iter()
            .any(|(key, _)| !allowed.contains(&key.as_slice()))
            || stream.dict.get(b"Type").ok()?.as_name().ok()? != b"EmbeddedFile"
            || stream.dict.get(b"FolioVersion").ok()?.as_i64().ok()? != 1
            || stream.dict.get(b"FolioImageId").ok()?.as_str().ok()? != expected_id.as_bytes()
        {
            return None;
        }
        let mime = stream.dict.get(b"Subtype").ok()?.as_name().ok()?;
        if mime != b"image/png" && mime != b"image/jpeg" {
            return None;
        }
        let params = resolve(self.document, stream.dict.get(b"Params").ok()?, &mut 64)?
            .as_dict()
            .ok()?;
        if params.len() != 1 {
            return None;
        }
        let length = usize::try_from(params.get(b"Size").ok()?.as_i64().ok()?).ok()?;
        if length == 0 || length > MAX_IMAGE_BYTES || stream.content.len() > MAX_IMAGE_BYTES {
            return None;
        }
        // Only a single lossless transport is permitted around the original file;
        // in particular, never decode an attachment through DCT or predictors.
        let bytes = if stream.dict.has(b"Filter") {
            if stream.dict.get(b"Filter").ok()?.as_name().ok()? != b"FlateDecode" {
                return None;
            }
            Cow::Owned(stream.decompressed_content_with_limit(length).ok()?)
        } else {
            Cow::Borrowed(stream.content.as_slice())
        };
        if bytes.len() != length
            || Sha256::digest(&bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
                != expected_id
        {
            return None;
        }
        let asset = if let Some(cached) = self.assets.get(expected_id) {
            cached.clone()
        } else {
            let encoded = self.encoded.checked_add(length)?;
            if self.assets.len() >= self.limits.assets || encoded > self.limits.encoded {
                return None;
            }
            let asset = Arc::new(ImageAsset::parse(bytes.into_owned()).ok()?);
            if asset.info().mime_type.as_bytes() != mime {
                return None;
            }
            let prepared = prepare_pdf_image(&asset).ok()?;
            let prepared_bytes = resource_bytes(prepared.form());
            let total_prepared = self.prepared.checked_add(prepared_bytes)?;
            if total_prepared > self.limits.prepared {
                return None;
            }
            let value = Arc::new(PreparedImageAsset { asset, prepared:Arc::new(prepared) });
            // Publish cache entries only after every admission check succeeds.
            self.encoded = encoded;
            self.prepared = total_prepared;
            self.assets.insert(expected_id.to_owned(), value.clone());
            value
        };
        if asset.asset.info().mime_type.as_bytes() != mime {
            return None;
        }
        self.streams.insert(id, asset.clone());
        Some(asset)
    }
}

/// Counts retained encoded drawing streams, not decoder transient allocations or
/// allocator overhead. The generated resource graph is small, local and acyclic.
pub(crate) fn resource_bytes(form: &Stream) -> usize {
    fn dict(values: &Dictionary) -> usize {
        values.iter().map(|(_, value)| object(value)).sum()
    }
    fn object(value: &Object) -> usize {
        match value {
            Object::Stream(stream) => stream.content.len() + dict(&stream.dict),
            Object::Dictionary(values) => dict(values),
            Object::Array(values) => values.iter().map(object).sum(),
            _ => 0,
        }
    }
    form.content.len() + dict(&form.dict)
}

fn resolve<'a>(
    document: &'a Document,
    mut value: &'a Object,
    remaining: &mut usize,
) -> Option<&'a Object> {
    for _ in 0..16 {
        *remaining = remaining.checked_sub(1)?;
        if let Object::Reference(id) = value {
            value = document.objects.get(id)?;
        } else {
            return Some(value);
        }
    }
    None
}

/// Compare encoded drawing resources exactly, including Filter/DecodeParms.
/// Ignoring those as mere transport would miss JPEG color-transform changes.
/// Conservative refusal of third-party recompression leaves its native drawing
/// intact. No untrusted resource stream is decompressed during this proof.
#[cfg(test)]
pub(crate) fn matches_form(document: &Document, actual: &Object, expected: &Stream) -> bool {
    let mut remaining = 4096;
    resolve(document, actual, &mut remaining)
        .and_then(|actual| actual.as_stream().ok())
        .is_some_and(|actual| compare_stream(document, actual, expected, 0, &mut remaining))
}
pub(crate) fn matches_resources(
    document: &Document,
    actual: &Object,
    expected: &Dictionary,
) -> bool {
    let mut remaining = 4096;
    resolve(document, actual, &mut remaining)
        .and_then(|value| value.as_dict().ok())
        .is_some_and(|actual| compare_dict(document, actual, expected, 0, &mut remaining, false))
}
fn compare_stream(
    document: &Document,
    actual: &Stream,
    expected: &Stream,
    depth: usize,
    remaining: &mut usize,
) -> bool {
    actual.content == expected.content
        && compare_dict(
            document,
            &actual.dict,
            &expected.dict,
            depth,
            remaining,
            true,
        )
}
fn compare_dict(
    document: &Document,
    actual: &Dictionary,
    expected: &Dictionary,
    depth: usize,
    remaining: &mut usize,
    stream: bool,
) -> bool {
    let significant = |key: &[u8]| !stream || key != b"Length";
    if actual.iter().filter(|(key, _)| significant(key)).count()
        != expected.iter().filter(|(key, _)| significant(key)).count()
    {
        return false;
    }
    expected
        .iter()
        .filter(|(key, _)| significant(key))
        .all(|(key, value)| {
            actual
                .get(key)
                .ok()
                .is_some_and(|actual| compare(document, actual, value, depth + 1, remaining))
        })
}
fn compare(
    document: &Document,
    actual: &Object,
    expected: &Object,
    depth: usize,
    remaining: &mut usize,
) -> bool {
    if depth > 16 {
        return false;
    }
    let Some(actual) = resolve(document, actual, remaining) else {
        return false;
    };
    match (actual, expected) {
        (Object::Stream(a), Object::Stream(b)) => compare_stream(document, a, b, depth, remaining),
        (Object::Dictionary(a), Object::Dictionary(b)) => {
            compare_dict(document, a, b, depth, remaining, false)
        }
        (Object::Array(a), Object::Array(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b)
                    .all(|(a, b)| compare(document, a, b, depth + 1, remaining))
        }
        (Object::Integer(a), Object::Integer(b)) => a == b,
        (Object::Real(a), Object::Real(b)) => a.is_finite() && b.is_finite() && a == b,
        (Object::Integer(a), Object::Real(b)) | (Object::Real(b), Object::Integer(a)) => {
            b.is_finite() && *a as f64 == f64::from(*b)
        }
        (Object::Name(a), Object::Name(b)) | (Object::String(a, _), Object::String(b, _)) => a == b,
        (Object::Boolean(a), Object::Boolean(b)) => a == b,
        (Object::Null, Object::Null) => true,
        _ => false,
    }
}

#[cfg(test)]
#[path = "image_persistence_tests.rs"]
mod tests;
