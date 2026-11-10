//! Immutable original image bytes and independent acquisition leases.
//! This resource foundation does not yet expose image placement in the editor.
use crate::{EngineError, EngineResult};
use image::{metadata::Orientation, DynamicImage, ImageDecoder, ImageFormat};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Read},
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
};
#[path="image_prepared_cache.rs"]
mod prepared_cache;
use prepared_cache::PreparedEntry;

const MAX_IMAGE_BYTES: usize = 64 * 1024 * 1024;
const MAX_IMAGE_PIXELS: u64 = 32_000_000;
const MAX_DIMENSION: u32 = 32_768;
// PNG's decoder limit is best effort. JPEG has no total allocation limiter;
// both formats have strict input, dimension and pixel admission limits below.
const PNG_DECODE_ALLOCATION_HINT: u64 = 256 * 1024 * 1024;

fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::InvalidRequest(message.into())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageInfo {
    pub id: String,
    pub width: u32,
    pub height: u32,
    pub mime_type: String,
    pub has_alpha: bool,
    pub bits_per_channel: u8,
    pub byte_length: usize,
}

/// Original profiles are retained as metadata; this is not a color-management proof.
pub struct DecodedImage {
    pub pixels: DynamicImage,
    pub icc_profile: Option<Vec<u8>>,
}

pub struct ImageAsset {
    info: ImageInfo,
    bytes: Arc<[u8]>,
    decoded_bytes: usize,
}

impl std::fmt::Debug for ImageAsset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageAsset")
            .field("info", &self.info)
            .finish_non_exhaustive()
    }
}

fn check_pixels(width: u32, height: u32) -> EngineResult<()> {
    if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(invalid(
            "Image dimensions must be between 1 and 32,768 pixels.",
        ));
    }
    if u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS {
        return Err(invalid("Each image must contain 1–32 million pixels."));
    }
    Ok(())
}

// image's PNG decoder stops at the decoded frame and does not validate IEND.
// Validate every chunk's framing and CRC, including those after the pixel data.
fn validate_png_chunks(bytes: &[u8]) -> EngineResult<()> {
    let malformed = || invalid("The PNG has incomplete or corrupt chunks.");
    let mut offset = 8;
    while let Some(header) = bytes.get(offset..offset + 8) {
        let length = u32::from_be_bytes(header[..4].try_into().unwrap()) as usize;
        let end = offset
            .checked_add(12)
            .and_then(|n| n.checked_add(length))
            .ok_or_else(malformed)?;
        let chunk = bytes.get(offset..end).ok_or_else(malformed)?;
        let crc = u32::from_be_bytes(chunk[chunk.len() - 4..].try_into().unwrap());
        if crc32fast::hash(&chunk[4..chunk.len() - 4]) != crc {
            return Err(malformed());
        }
        if &header[4..8] == b"IEND" {
            return if length == 0 && end == bytes.len() {
                Ok(())
            } else {
                Err(malformed())
            };
        }
        offset = end;
    }
    Err(malformed())
}

fn decode_jpeg(bytes: &[u8]) -> EngineResult<DecodedImage> {
    use jpeg_decoder::PixelFormat;
    let mut decoder = jpeg_decoder::Decoder::new(Cursor::new(bytes));
    decoder.set_max_decoding_buffer_size(256 * 1024 * 1024);
    let bad = |e| invalid(format!("The JPEG cannot be decoded: {e}"));
    decoder.read_info().map_err(bad)?;
    let info = decoder
        .info()
        .ok_or_else(|| invalid("The JPEG has no dimensions."))?;
    let (width, height) = (u32::from(info.width), u32::from(info.height));
    check_pixels(width, height)?;
    if info.pixel_format == PixelFormat::L16 {
        return Err(invalid("Only 8-bit JPEG images are supported."));
    }
    let samples = decoder.decode().map_err(bad)?;
    let orientation = decoder
        .exif_data()
        .and_then(Orientation::from_exif_chunk)
        .unwrap_or(Orientation::NoTransforms);
    let icc_profile = decoder.icc_profile();
    let missing = || invalid("The JPEG sample count does not match its dimensions.");
    let mut pixels = match info.pixel_format {
        PixelFormat::L8 => DynamicImage::ImageLuma8(
            image::GrayImage::from_raw(width, height, samples).ok_or_else(missing)?,
        ),
        PixelFormat::RGB24 => DynamicImage::ImageRgb8(
            image::RgbImage::from_raw(width, height, samples).ok_or_else(missing)?,
        ),
        PixelFormat::CMYK32 => {
            // Conventional unprofiled CMYK preview. Original CMYK bytes/profile remain
            // canonical; a retained CMYK ICC must not be attached to these RGB samples.
            let rgb = samples
                .chunks_exact(4)
                .flat_map(|cmyk| {
                    let k = 255 - u16::from(cmyk[3]);
                    [0, 1, 2].map(|i| (((255 - u16::from(cmyk[i])) * k + 127) / 255) as u8)
                })
                .collect();
            DynamicImage::ImageRgb8(
                image::RgbImage::from_raw(width, height, rgb).ok_or_else(missing)?,
            )
        }
        PixelFormat::L16 => unreachable!("rejected before decode"),
    };
    pixels.apply_orientation(orientation);
    Ok(DecodedImage {
        pixels,
        icc_profile,
    })
}

fn decode(bytes: &[u8]) -> EngineResult<(DecodedImage, &'static str)> {
    if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
        return Err(invalid("Choose an image file no larger than 64 MiB."));
    }
    let format = image::guess_format(bytes)
        .map_err(|_| invalid("This file is not a readable PNG or JPEG."))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(PNG_DECODE_ALLOCATION_HINT);
    let bad = |e: image::ImageError| invalid(format!("The image cannot be decoded: {e}"));
    let (mut decoder, mime): (Box<dyn ImageDecoder + '_>, _) = match format {
        ImageFormat::Png => {
            validate_png_chunks(bytes)?;
            let decoder = image::codecs::png::PngDecoder::with_limits(Cursor::new(bytes), limits)
                .map_err(bad)?;
            if decoder.is_apng().map_err(bad)? {
                return Err(invalid(
                    "Animated PNG images need a still frame before placement.",
                ));
            }
            (Box::new(decoder), "image/png")
        }
        ImageFormat::Jpeg => {
            return decode_jpeg(bytes).map(|decoded| (decoded, "image/jpeg"));
        }
        _ => return Err(invalid("Only PNG and JPEG images are supported.")),
    };
    let (width, height) = decoder.dimensions();
    check_pixels(width, height)?;
    let orientation = decoder.orientation().map_err(bad)?;
    let icc_profile = decoder.icc_profile().map_err(bad)?;
    let mut pixels = DynamicImage::from_decoder(decoder).map_err(bad)?;
    pixels.apply_orientation(orientation);
    Ok((
        DecodedImage {
            pixels,
            icc_profile,
        },
        mime,
    ))
}

impl ImageAsset {
    pub fn parse(bytes: Vec<u8>) -> EngineResult<Self> {
        let (decoded, mime) = decode(&bytes)?;
        let color = decoded.pixels.color();
        let info = ImageInfo {
            id: Sha256::digest(&bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
            width: decoded.pixels.width(),
            height: decoded.pixels.height(),
            mime_type: mime.into(),
            has_alpha: color.has_alpha(),
            bits_per_channel: (color.bits_per_pixel() / u16::from(color.channel_count())) as u8,
            byte_length: bytes.len(),
        };
        Ok(Self {
            info,
            decoded_bytes: decoded.pixels.as_bytes().len(),
            bytes: bytes.into(),
        })
    }

    pub fn read(path: &Path) -> EngineResult<Self> {
        let file = std::fs::File::open(path).map_err(|e| invalid(e.to_string()))?;
        let metadata = file.metadata().map_err(|e| invalid(e.to_string()))?;
        if !metadata.is_file() || metadata.len() > MAX_IMAGE_BYTES as u64 {
            return Err(invalid("Choose an image file no larger than 64 MiB."));
        }
        let mut bytes = Vec::new();
        file.take(MAX_IMAGE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| invalid(e.to_string()))?;
        Self::parse(bytes)
    }

    pub fn info(&self) -> &ImageInfo {
        &self.info
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn decode(&self) -> EngineResult<DecodedImage> {
        decode(&self.bytes).map(|(decoded, _)| decoded)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageRegistration {
    pub image: ImageInfo,
    /// Release this acquisition, never an asset ID shared by unrelated owners.
    pub token: String,
}

#[derive(Clone, Copy)]
struct RegistryLimits {
    assets: usize,
    encoded: usize,
    decoded: usize,
    leases: usize,
}
const REGISTRY_LIMITS: RegistryLimits = RegistryLimits {
    assets: 64,
    encoded: 256 * 1024 * 1024,
    decoded: 512 * 1024 * 1024,
    leases: 4096,
};
struct Entry {
    asset: Arc<ImageAsset>,
    owners: usize,
    access: ImageAccess,
}
/// Provenance is retained for the loaded asset's lifetime, including history
/// acquisitions after its original source closes. Acquiring a token never grants
/// broader access. Independently importing identical bytes legitimately does.
#[derive(Clone,Default)]
struct ImageAccess {unrestricted:bool,origins:BTreeSet<String>}
impl ImageAccess {
    fn for_source(restricted_origin:Option<&str>)->Self {
        match restricted_origin {Some(id)=>Self{unrestricted:false,origins:BTreeSet::from([id.to_owned()])},None=>Self{unrestricted:true,origins:BTreeSet::new()}}
    }
    fn merge(&mut self,other:&Self){
        self.unrestricted|=other.unrestricted;
        if self.unrestricted {self.origins.clear();} else {self.origins.extend(other.origins.iter().cloned());}
    }
}
#[derive(Default)]
struct RegistryState {
    assets: BTreeMap<String, Entry>,
    leases: BTreeMap<String, String>,
    encoded: usize,
    decoded: usize,
    prepared:BTreeMap<String,PreparedEntry>,
    prepared_bytes:usize,
    prepared_tick:u64,
}

pub struct ImageRegistry {
    state: Mutex<RegistryState>,
    limits: RegistryLimits,
    prepared_limit:usize,
}
impl Default for ImageRegistry {
    fn default() -> Self {
        Self {
            state: Mutex::new(RegistryState::default()),
            limits: REGISTRY_LIMITS,
            prepared_limit:128*1024*1024,
        }
    }
}

fn admitted(total: usize, extra: usize, limit: usize, resource: &str) -> EngineResult<usize> {
    total.checked_add(extra).filter(|sum| *sum <= limit)
        .ok_or_else(|| invalid(format!("Loaded image {resource} exceed the resource limit. Close documents using other images first.")))
}

impl ImageRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    fn lock(&self) -> EngineResult<MutexGuard<'_, RegistryState>> {
        self.state
            .lock()
            .map_err(|_| invalid("The image registry is unavailable."))
    }
    pub fn register(&self, bytes: Vec<u8>) -> EngineResult<ImageRegistration> {
        let asset = Arc::new(ImageAsset::parse(bytes)?);
        Ok(self.register_assets(&[asset])?.remove(0))
    }
    /// Callers can stage parsed assets without changing global ownership. The entire
    /// batch is admitted before any entry/token is installed, including duplicates.
    pub fn register_assets(
        &self,
        assets: &[Arc<ImageAsset>],
    ) -> EngineResult<Vec<ImageRegistration>> {
        self.register_with_access(assets,ImageAccess::for_source(None))
    }
    pub(crate) fn register_source_assets(&self,assets:&[Arc<ImageAsset>],restricted_origin:Option<&str>)->EngineResult<Vec<ImageRegistration>> {
        self.register_with_access(assets,ImageAccess::for_source(restricted_origin))
    }
    fn register_with_access(&self,assets:&[Arc<ImageAsset>],access:ImageAccess)->EngineResult<Vec<ImageRegistration>> {
        let mut state = self.lock()?;
        // Bound distinct protected provenance even if sources repeatedly open and
        // close while a history lease keeps the same image alive.
        for asset in assets {
            if let Some(entry)=state.assets.get(&asset.info.id) {
                if !access.unrestricted&&!entry.access.unrestricted&&entry.access.origins.union(&access.origins).count()>4096 {
                    return Err(invalid("Loaded image provenance exceeds the resource limit. Close documents using this image first."));
                }
            }
        }
        admitted(
            state.leases.len(),
            assets.len(),
            self.limits.leases,
            "acquisitions",
        )?;
        let added: BTreeMap<_, _> = assets
            .iter()
            .filter(|asset| !state.assets.contains_key(&asset.info.id))
            .map(|asset| (asset.info.id.clone(), asset.clone()))
            .collect();
        admitted(state.assets.len(), added.len(), self.limits.assets, "count")?;
        let (encoded, decoded) = added.values().try_fold(
            (state.encoded, state.decoded),
            |(encoded, decoded), asset| {
                Ok::<_, EngineError>((
                    admitted(encoded, asset.bytes.len(), self.limits.encoded, "bytes")?,
                    admitted(
                        decoded,
                        asset.decoded_bytes,
                        self.limits.decoded,
                        "decoded samples",
                    )?,
                ))
            },
        )?;
        state.assets.extend(
            added
                .into_iter()
                .map(|(id, asset)| (id, Entry { asset, owners: 0, access:ImageAccess::default() })),
        );
        state.encoded = encoded;
        state.decoded = decoded;
        let mut registrations = Vec::with_capacity(assets.len());
        for asset in assets {
            state.assets.get_mut(&asset.info.id).expect("admitted asset").access.merge(&access);
            let token = loop {
                let value = uuid::Uuid::new_v4().to_string();
                if !state.leases.contains_key(&value) {
                    break value;
                }
            };
            state
                .assets
                .get_mut(&asset.info.id)
                .expect("admitted asset")
                .owners += 1;
            state.leases.insert(token.clone(), asset.info.id.clone());
            registrations.push(ImageRegistration {
                image: asset.info.clone(),
                token,
            });
        }
        Ok(registrations)
    }
    pub fn acquire(&self, id: &str) -> EngineResult<ImageRegistration> {
        // A retained Arc remains valid if another thread releases the last registry
        // lease between this borrow and registering our independent acquisition.
        let (asset,access)={let state=self.lock()?;let entry=state.assets.get(id).ok_or_else(||invalid("The requested image is not loaded. Import it again."))?;(entry.asset.clone(),entry.access.clone())};
        self.register_with_access(&[asset],access)
            .map(|mut values| values.remove(0))
    }
    pub(crate) fn authorize(&self,id:&str,origin:Option<&str>)->EngineResult<()> {
        let state=self.lock()?;let entry=state.assets.get(id).ok_or_else(||invalid("The requested image is not loaded. Import it again."))?;
        if entry.access.unrestricted||origin.is_some_and(|id|entry.access.origins.contains(id)){Ok(())}
        else {Err(EngineError::PermissionDenied("copying an image from its protected document"))}
    }
    pub fn get(&self, id: &str) -> EngineResult<Arc<ImageAsset>> {
        self.lock()?
            .assets
            .get(id)
            .map(|entry| entry.asset.clone())
            .ok_or_else(|| invalid("The requested image is not loaded. Import it again."))
    }
    pub fn ids(&self) -> EngineResult<BTreeSet<String>> {
        Ok(self.lock()?.assets.keys().cloned().collect())
    }
    pub fn release(&self, token: &str) -> EngineResult<()> {
        let mut state = self.lock()?;
        Self::release_locked(&mut state, token);
        Ok(())
    }
    pub fn release_all(&self, registrations: &[ImageRegistration]) -> EngineResult<()> {
        let mut state = self.lock()?;
        for registration in registrations {
            Self::release_locked(&mut state, &registration.token);
        }
        Ok(())
    }
    fn release_locked(state: &mut RegistryState, token: &str) {
        let Some(id) = state.leases.remove(token) else {
            return;
        };
        let entry = state.assets.get_mut(&id).expect("leased asset");
        entry.owners -= 1;
        if entry.owners == 0 {
            let entry = state.assets.remove(&id).expect("last leased asset");
            state.encoded -= entry.asset.bytes.len();
            state.decoded -= entry.asset.decoded_bytes;
            if let Some(prepared)=state.prepared.remove(&id){state.prepared_bytes-=prepared.bytes;}
        }
    }
}

#[cfg(test)]
#[path = "image_assets_tests.rs"]
mod tests;
