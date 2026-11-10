//! Deterministic, transparency-preserving image resources for PDF appearances.
use crate::{EngineError, EngineResult, ImageAsset};
use image::metadata::Orientation;
use lopdf::{
    content::{Content, Operation},
    dictionary, Dictionary, Document, Object, ObjectId, Stream,
};
use std::io::Cursor;

#[path = "image_pdf_color.rs"]
mod color;

pub struct PreparedPdfImage {
    form: Stream,
}
impl PreparedPdfImage {
    /// Complete immutable resource graph, before streams become indirect objects.
    pub fn form(&self) -> &Stream {
        &self.form
    }
    pub fn install(self, document: &mut Document) -> ObjectId {
        install(document, Object::Stream(self.form))
            .as_reference()
            .expect("root stream installed")
    }
    pub(crate) fn install_copy(&self, document: &mut Document) -> ObjectId {
        install(document, Object::Stream(self.form.clone())).as_reference().expect("root stream installed")
    }
}
fn install(document: &mut Document, object: Object) -> Object {
    match object {
        Object::Stream(mut stream) => {
            stream.dict = install_dictionary(document, stream.dict);
            Object::Reference(document.add_object(stream))
        }
        Object::Dictionary(dict) => Object::Dictionary(install_dictionary(document, dict)),
        Object::Array(values) => {
            Object::Array(values.into_iter().map(|v| install(document, v)).collect())
        }
        value => value,
    }
}
fn install_dictionary(document: &mut Document, source: Dictionary) -> Dictionary {
    let mut result = Dictionary::new();
    for (key, value) in source {
        result.set(key, install(document, value));
    }
    result
}
fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::InvalidRequest(message.into())
}
fn compressed(dict: Dictionary, bytes: Vec<u8>) -> EngineResult<Stream> {
    let mut stream = Stream::new(dict, bytes);
    stream.compress().map_err(|e| invalid(e.to_string()))?;
    Ok(stream)
}
fn image_dictionary(width: u32, height: u32, bits: u8, space: Object) -> Dictionary {
    dictionary! {"Type"=>"XObject","Subtype"=>"Image","Width"=>width as i64,"Height"=>height as i64,
    "BitsPerComponent"=>bits as i64,"ColorSpace"=>space,"Interpolate"=>false}
}

pub fn prepare_pdf_image(asset: &ImageAsset) -> EngineResult<PreparedPdfImage> {
    let (image, matrix) = if asset.info().mime_type == "image/jpeg" {
        jpeg(asset)?
    } else {
        png(asset)?
    };
    let content = Content {
        operations: vec![
            Operation::new("q", vec![]),
            Operation::new("cm", matrix.into_iter().map(Object::Real).collect()),
            Operation::new("Do", vec![Object::Name(b"Im0".to_vec())]),
            Operation::new("Q", vec![]),
        ],
    }
    .encode()
    .map_err(|e| invalid(e.to_string()))?;
    Ok(PreparedPdfImage {
        form: Stream::new(
            dictionary! {"Type"=>"XObject","Subtype"=>"Form",
            "BBox"=>vec![0.into(),0.into(),1.into(),1.into()],"Resources"=>dictionary!{"XObject"=>dictionary!{"Im0"=>Object::Stream(image)}}},
            content,
        ),
    })
}

fn png(asset: &ImageAsset) -> EngineResult<(Stream, [f32; 6])> {
    let decoded = asset.decode()?;
    let pixels = &decoded.pixels;
    let kind = pixels.color();
    let components = kind.channel_count() as usize;
    let bits = (kind.bits_per_pixel() / components as u16) as u8;
    if !matches!(bits, 8 | 16) {
        return Err(invalid("Unsupported PNG sample depth."));
    }
    let color_components = components - usize::from(kind.has_alpha());
    if !matches!(color_components, 1 | 3) {
        return Err(invalid("Unsupported PNG color components."));
    }
    let description = color::png(
        asset.bytes(),
        decoded.icc_profile.as_deref(),
        color_components as u8,
    )?;
    let sample_size = usize::from(bits / 8);
    let count = pixels.width() as usize * pixels.height() as usize;
    let mut samples = Vec::with_capacity(count * color_components * sample_size);
    let mut alpha = kind
        .has_alpha()
        .then(|| Vec::with_capacity(count * sample_size));
    for pixel in pixels.as_bytes().chunks_exact(components * sample_size) {
        for (index, sample) in pixel.chunks_exact(sample_size).enumerate() {
            let target = if index < color_components {
                &mut samples
            } else {
                alpha.as_mut().expect("alpha channel")
            };
            if bits == 8 {
                target.push(sample[0]);
            } else {
                target.extend_from_slice(&u16::from_ne_bytes([sample[0], sample[1]]).to_be_bytes());
            }
        }
    }
    let mut dict = image_dictionary(pixels.width(), pixels.height(), bits, description.space);
    if let Some(intent) = description.intent {
        dict.set("Intent", intent);
    }
    if let Some(alpha) = alpha {
        let mask = compressed(
            image_dictionary(
                pixels.width(),
                pixels.height(),
                bits,
                Object::Name(b"DeviceGray".to_vec()),
            ),
            alpha,
        )?;
        dict.set("SMask", Object::Stream(mask));
    }
    Ok((compressed(dict, samples)?, [1., 0., 0., 1., 0., 0.]))
}

fn jpeg(asset: &ImageAsset) -> EngineResult<(Stream, [f32; 6])> {
    // The original DCT bytes preserve JPEG samples without a second lossy encode.
    // Decode through the same strict admission backend to obtain complete metadata,
    // including APP segments occurring after SOF. Discard preview samples afterwards.
    let mut decoder = jpeg_decoder::Decoder::new(Cursor::new(asset.bytes()));
    decoder.set_max_decoding_buffer_size(256 * 1024 * 1024);
    let _ = decoder
        .decode()
        .map_err(|e| invalid(format!("The JPEG cannot be decoded: {e}")))?;
    let info = decoder
        .info()
        .ok_or_else(|| invalid("JPEG dimensions are unavailable."))?;
    let components = match info.pixel_format {
        jpeg_decoder::PixelFormat::L8 => 1,
        jpeg_decoder::PixelFormat::RGB24 => 3,
        jpeg_decoder::PixelFormat::CMYK32 => 4,
        _ => return Err(invalid("Only 8-bit JPEG images can be placed.")),
    };
    let orientation = decoder
        .exif_data()
        .and_then(Orientation::from_exif_chunk)
        .unwrap_or(Orientation::NoTransforms);
    let (transform, declares_icc) = jpeg_metadata(asset.bytes(), components)?;
    let profile = decoder.icc_profile();
    if declares_icc && profile.is_none() {
        return Err(invalid("The JPEG declares an incomplete or unusable ICC color profile. Convert it to sRGB before placing it."));
    }
    let description = color::icc_or_device(profile.as_deref(), components)?;
    let mut dict = image_dictionary(
        u32::from(info.width),
        u32::from(info.height),
        8,
        description.space,
    );
    dict.set("Filter", "DCTDecode");
    dict.set("DecodeParms", dictionary! {"ColorTransform"=>transform});
    if components == 4 {
        dict.set(
            "Decode",
            vec![
                1.into(),
                0.into(),
                1.into(),
                0.into(),
                1.into(),
                0.into(),
                1.into(),
                0.into(),
            ],
        );
    }
    if let Some(intent) = description.intent {
        dict.set("Intent", intent);
    }
    let matrix = match orientation {
        Orientation::NoTransforms => [1., 0., 0., 1., 0., 0.],
        Orientation::FlipHorizontal => [-1., 0., 0., 1., 1., 0.],
        Orientation::Rotate180 => [-1., 0., 0., -1., 1., 1.],
        Orientation::FlipVertical => [1., 0., 0., -1., 0., 1.],
        Orientation::Rotate90FlipH => [0., -1., -1., 0., 1., 1.],
        Orientation::Rotate90 => [0., -1., 1., 0., 0., 1.],
        Orientation::Rotate270FlipH => [0., 1., 1., 0., 0., 0.],
        Orientation::Rotate270 => [0., 1., -1., 0., 1., 0.],
    };
    Ok((Stream::new(dict, asset.bytes().to_vec()), matrix))
}

// DCTDecode's transform hint is required for RGB JPEGs without an Adobe marker.
// An Adobe APP14 marker takes precedence inside conforming PDF decoders.
fn jpeg_metadata(bytes: &[u8], components: u8) -> EngineResult<(i64, bool)> {
    let mut offset = 2;
    let mut rgb_ids = false;
    let mut adobe = None;
    let mut dct = false;
    let mut declares_icc = false;
    let mut entropy = false;
    while offset < bytes.len() {
        // Scan all marker segments, including metadata between progressive scans.
        // Stuffed FF00 bytes and restart markers belong to entropy, not APP data.
        if entropy {
            while offset < bytes.len() && bytes[offset] != 255 {
                offset += 1;
            }
        }
        if bytes[offset] != 255 {
            return Err(invalid("Invalid JPEG marker framing."));
        }
        while bytes.get(offset) == Some(&255) {
            offset += 1;
        }
        let marker = *bytes
            .get(offset)
            .ok_or_else(|| invalid("Incomplete JPEG marker."))?;
        offset += 1;
        if marker == 0xd9 {
            break;
        }
        if entropy && (marker == 0 || (0xd0..=0xd7).contains(&marker)) {
            continue;
        }
        entropy = false;
        if marker == 0x01 || (0xd0..=0xd8).contains(&marker) {
            continue;
        }
        let size = bytes
            .get(offset..offset + 2)
            .ok_or_else(|| invalid("Incomplete JPEG segment."))?;
        let length = u16::from_be_bytes([size[0], size[1]]) as usize;
        if length < 2 {
            return Err(invalid("Invalid JPEG segment length."));
        }
        let payload = bytes
            .get(offset + 2..offset + length)
            .ok_or_else(|| invalid("Incomplete JPEG segment."))?;
        if marker == 0xe2 && payload.starts_with(b"ICC_PROFILE\0") {
            declares_icc = true;
        }
        if marker == 0xee && payload.len() >= 12 && &payload[..5] == b"Adobe" {
            adobe = Some(i64::from(payload[11] != 0));
        }
        if matches!(marker, 0xc0 | 0xc1 | 0xc2) {
            dct = true;
            rgb_ids = payload.len() >= 15
                && payload[5] == 3
                && [payload[6], payload[9], payload[12]] == *b"RGB";
        }
        offset += length;
        entropy = marker == 0xda;
    }
    if !dct {
        return Err(invalid(
            "This JPEG encoding cannot be embedded as a PDF image. Convert it to PNG first.",
        ));
    }
    Ok((
        adobe.unwrap_or(i64::from(components == 3 && !rgb_ids)),
        declares_icc,
    ))
}
