//! Bounded immutable pixel/JPEG payloads for private structural redaction.
use crate::{
    redaction_content::{invalid, require},
    redaction_jpeg::{self as jpeg, Header as JpegHeader, Transform},
    redaction_jpeg_coverage::{Coverage, CoverageBudget, PaintVersion},
    EngineResult,
};
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

pub(crate) const SAMPLE_BUDGET: usize = 64 * 1024 * 1024;

#[path = "redaction_image_cache.rs"]
mod cache;
pub(crate) use cache::ImageContext;

#[path = "redaction_image_paint.rs"]
mod paint;
pub(crate) use paint::{publish_operations, strip_owned_paint};

#[path = "redaction_image_predictors.rs"]
mod predictors;

pub(crate) struct Image {
    pub width: u32,
    pub height: u32,
    pub components: usize,
    payload: Payload,
    pub digest: [u8; 32],
}
enum Payload {
    Pixels(Arc<[u8]>),
    Jpeg {
        bytes: Arc<[u8]>,
        header: JpegHeader,
        base_digest: [u8; 32],
        coverage: Option<Arc<Coverage>>,
        paint_version: PaintVersion,
    },
}
#[derive(Default)]
pub(crate) struct Registry {
    images: HashMap<ObjectId, Arc<Image>>,
    bytes: usize,
    input_bytes: usize,
    jpeg_work: u64,
    coverage_budget: CoverageBudget,
    reuse: Option<cache::Reuse>,
    // Enable only after the complete JPEG occurrence/disclosure gate passes.
    jpeg_pages: bool,
}
fn resolve<'a>(pdf: &'a Document, mut value: &'a Object) -> EngineResult<&'a Object> {
    let mut seen = HashSet::new();
    while let Object::Reference(id) = value {
        require(seen.len() < 64 && seen.insert(*id))?;
        value = pdf.objects.get(id).ok_or_else(invalid)?;
    }
    Ok(value)
}
impl Registry {
    pub fn load(&mut self, pdf: &Document, value: &Object) -> EngineResult<Arc<Image>> {
        let mut id = value.as_reference().map_err(|_| invalid())?;
        let mut seen = HashSet::new();
        let stream = loop {
            require(seen.len() < 64 && seen.insert(id))?;
            match pdf.objects.get(&id).ok_or_else(invalid)? {
                Object::Reference(next) => id = *next,
                Object::Stream(stream) => break stream,
                _ => return Err(invalid()),
            }
        };
        if let Some(image) = self.images.get(&id) {
            return Ok(image.clone());
        }
        require(self.images.len() < 256)?;
        let d = &stream.dict;
        require(d.iter().all(|(key, _)| {
            [
                b"Type".as_slice(),
                b"Subtype",
                b"Width",
                b"Height",
                b"BitsPerComponent",
                b"ColorSpace",
                b"Interpolate",
                b"Decode",
                b"Length",
                b"Filter",
                b"DecodeParms",
                b"FolioJpegRedaction",
            ]
            .contains(&key.as_slice())
        }))?;
        let field = |name: &[u8]| resolve(pdf, d.get(name).map_err(|_| invalid())?);
        if d.has(b"Type") {
            require(field(b"Type")?.as_name().ok() == Some(b"XObject"))?;
        }
        require(field(b"Subtype")?.as_name().ok() == Some(b"Image"))?;
        let dimension = |key: &[u8]| -> EngineResult<u32> {
            let n = field(key)?.as_i64().map_err(|_| invalid())?;
            require((1..=32768).contains(&n))?;
            Ok(n as u32)
        };
        let width = dimension(b"Width")?;
        let height = dimension(b"Height")?;
        require(u64::from(width) * u64::from(height) <= 16_000_000)?;
        require(field(b"BitsPerComponent")?.as_i64().ok() == Some(8))?;
        let components = match field(b"ColorSpace")?.as_name().map_err(|_| invalid())? {
            b"DeviceGray" => 1,
            b"DeviceRGB" => 3,
            _ => return Err(invalid()),
        };
        if d.has(b"Interpolate") {
            require(matches!(field(b"Interpolate")?, Object::Boolean(false)))?;
        }
        if d.has(b"Decode") {
            let a = field(b"Decode")?.as_array().map_err(|_| invalid())?;
            require(a.len() == components * 2)?;
            for (index, n) in a.iter().enumerate() {
                require(crate::redaction_content::number(resolve(pdf, n)?)? == (index % 2) as f64)?;
            }
        }
        // Keep the decompressor's interpretation identical to validation: only
        // direct filter/parameter forms are admitted in this first image class.
        let filter = d
            .get(b"Filter")
            .ok()
            .map(|v| v.as_name().map_err(|_| invalid()))
            .transpose()?;
        require(matches!(
            filter,
            None | Some(b"FlateDecode") | Some(b"DCTDecode")
        ))?;
        require(!d.has(b"FolioJpegRedaction") || filter == Some(b"DCTDecode"))?;
        require(stream.content.len() <= SAMPLE_BUDGET)?;
        if filter == Some(b"DCTDecode") {
            let transform = jpeg_transform(d)?;
            let shape = (width, height, components as u32);
            let header = jpeg::inspect(&stream.content, shape, transform)?;
            let claim = if let Ok(value) = d.get(b"FolioJpegRedaction") {
                let record = value.as_dict().map_err(|_| invalid())?;
                require(
                    record.len() == 3
                        && record.iter().all(|(key, _)| {
                            [b"V".as_slice(), b"Mask", b"Binding"].contains(&key.as_slice())
                        }),
                )?;
                let paint_version = match record.get(b"V") {
                    Ok(Object::Integer(1)) => PaintVersion::Rectangles,
                    Ok(Object::Integer(2)) => PaintVersion::Triangles,
                    Ok(Object::Integer(3)) => PaintVersion::Contours,
                    _ => return Err(invalid()),
                };
                let mask = record
                    .get(b"Mask")
                    .and_then(Object::as_str)
                    .map_err(|_| invalid())?;
                let binding = record
                    .get(b"Binding")
                    .and_then(Object::as_str)
                    .map_err(|_| invalid())?;
                require(
                    mask.len() == (header.columns * header.rows) as usize && binding.len() == 32,
                )?;
                Some((mask, binding, paint_version))
            } else {
                None
            };
            let (mask, binding, paint_version) =
                claim.unwrap_or((&[], &[], PaintVersion::Contours));
            let key = self.input_key(
                &stream.content,
                (width, height, components),
                cache::Interpretation::Jpeg {
                    transformed: header.transformed,
                    mask,
                    binding,
                    paint_version: if claim.is_none() {
                        0
                    } else {
                        paint_version as u8
                    },
                },
            )?;
            if let Some(image) = self.reuse_input(id, key)? {
                return Ok(image);
            }
            self.charge_input(stream.content.len() + mask.len() + binding.len())?;
            let claim = claim
                .map(|(mask, binding, paint_version)| -> EngineResult<_> {
                    let coverage = Coverage::from_mask(&header, mask, &mut self.coverage_budget)?;
                    require(coverage.mask().contains(&1))?;
                    Ok((
                        coverage,
                        <[u8; 32]>::try_from(binding).map_err(|_| invalid())?,
                        paint_version,
                    ))
                })
                .transpose()?;
            let work_left = jpeg::Limits::default()
                .coefficient_work
                .saturating_sub(self.jpeg_work);
            let mask_bytes = claim
                .as_ref()
                .map_or(0, |(coverage, _, _)| coverage.mask().len());
            require(mask_bytes <= SAMPLE_BUDGET.saturating_sub(self.bytes))?;
            let output_left = SAMPLE_BUDGET.saturating_sub(self.bytes + mask_bytes);
            require(header.coefficient_work <= work_left && output_left > 0)?;
            // Attempted native work remains charged on failure; only a fully
            // validated output becomes a retained image/cache entry.
            self.jpeg_work += header.coefficient_work;
            let mask = vec![0; (header.columns * header.rows) as usize];
            let encoded = jpeg::rewrite(
                &stream.content,
                shape,
                transform,
                &mask,
                jpeg::Limits {
                    output_bytes: output_left,
                    coefficient_work: work_left,
                    ..jpeg::Limits::default()
                },
            )?;
            self.charge_hash(encoded.bytes.len() + 128)?;
            let mut image = Image::owned_jpeg(encoded);
            if let Some((coverage, binding, paint_version)) = claim {
                // A public checksum is only a binding, never a sanitation proof.
                // Reapplying the claimed mask must leave canonical bytes exact.
                self.charge_hash(2 * (coverage.mask().len() + 128))?;
                require(coverage.version_binding(&image.digest, paint_version) == binding)?;
                let work_left = jpeg::Limits::default()
                    .coefficient_work
                    .saturating_sub(self.jpeg_work);
                let h = image.jpeg_header().ok_or_else(invalid)?;
                require(h.coefficient_work <= work_left)?;
                self.jpeg_work += h.coefficient_work;
                let proof = image.rewrite_jpeg(
                    coverage.mask(),
                    jpeg::Limits {
                        output_bytes: image.retained_bytes(),
                        coefficient_work: work_left,
                        ..jpeg::Limits::default()
                    },
                )?;
                let Payload::Jpeg { bytes: before, .. } = &image.payload else {
                    return Err(invalid());
                };
                require(before.as_ref() == proof.bytes)?;
                image.attach_coverage_with_paint(coverage, paint_version)?;
            }
            return self.install_source(id, Arc::new(image), key);
        }
        let predictor = predictors::Predictor::new(d, width, height, components)?;
        let key = self.input_key(
            &stream.content,
            (width, height, components),
            cache::Interpretation::Pixels {
                compressed: filter.is_some(),
                predictor: predictor.kind(),
            },
        )?;
        if let Some(image) = self.reuse_input(id, key)? {
            return Ok(image);
        }
        self.charge_input(stream.content.len())?;
        let length = width as usize * height as usize * components;
        require(
            length <= SAMPLE_BUDGET.saturating_sub(self.bytes)
                && stream.content.len() <= SAMPLE_BUDGET,
        )?;
        let encoded = if !d.has(b"Filter") {
            require(stream.content.len() == length)?;
            stream.content.clone()
        } else if matches!(d.get(b"DecodeParms"), Ok(Object::Dictionary(_))) {
            // Parameters were validated above and are decoded separately below.
            // The strict generic decoder refuses all parameter dictionaries.
            // Normalize only this bounded temporary copy, retaining checksum,
            // full-consumption and exact predicted-output-limit guarantees.
            // Both original and temporary encoded payloads are <= SAMPLE_BUDGET.
            let mut normalized = stream.clone();
            normalized.dict.remove(b"DecodeParms");
            normalized
                .decompressed_content_strict_with_limit(predictor.encoded_len())
                .map_err(|_| invalid())?
        } else {
            stream
                .decompressed_content_strict_with_limit(predictor.encoded_len())
                .map_err(|_| invalid())?
        };
        let samples = predictor.decode(encoded)?;
        require(samples.len() == length)?;
        self.charge_hash(samples.len() + 128)?;
        let image = Arc::new(Image::owned(width, height, components, samples));
        self.install_source(id, image, key)
    }
}
impl Image {
    pub fn redacted_jpeg(
        &self,
        selected: &Coverage,
        limits: jpeg::Limits,
        budget: &mut CoverageBudget,
    ) -> EngineResult<Self> {
        require(limits.output_bytes <= jpeg::Limits::default().output_bytes)?;
        let header = self.jpeg_header().ok_or_else(invalid)?;
        selected.check_header(header)?;
        let coverage = if let Some(prior) = self.jpeg_coverage() {
            prior.union(selected, budget)?
        } else {
            Coverage::from_mask(header, selected.mask(), budget)?
        };
        require(selected.mask().contains(&1))?;
        require(coverage.mask().len() < limits.output_bytes)?;
        let encoded = self.rewrite_jpeg(
            coverage.mask(),
            jpeg::Limits {
                output_bytes: limits.output_bytes - coverage.mask().len(),
                ..limits
            },
        )?;
        let mut result = Self::owned_jpeg(encoded);
        result.attach_coverage(coverage)?;
        Ok(result)
    }
    pub fn jpeg_coverage(&self) -> Option<&Coverage> {
        match &self.payload {
            Payload::Jpeg { coverage, .. } => coverage.as_deref(),
            _ => None,
        }
    }
    pub fn jpeg_paint_version(&self) -> Option<PaintVersion> {
        match &self.payload {
            Payload::Jpeg {
                coverage: Some(_),
                paint_version,
                ..
            } => Some(*paint_version),
            _ => None,
        }
    }
    pub fn jpeg_needs_paint_upgrade(&self) -> bool {
        self.jpeg_paint_version()
            .is_some_and(|v| v != PaintVersion::Contours)
    }
    pub fn jpeg_paint(
        &self,
        budget: &mut CoverageBudget,
    ) -> EngineResult<Option<crate::redaction_jpeg_coverage::Paint>> {
        self.jpeg_coverage()
            .map(|coverage| {
                coverage.version_paint(budget, self.jpeg_paint_version().ok_or_else(invalid)?)
            })
            .transpose()
    }
    /// Checks one occurrence's following operations. The page caller must also
    /// validate path/clip/graphics context before removing these from its plan.
    pub fn verify_paint_prefix(
        &self,
        following: &[lopdf::content::Operation],
        budget: &mut CoverageBudget,
    ) -> EngineResult<usize> {
        let Some(paint) = self.jpeg_paint(budget)? else {
            return Ok(0);
        };
        let expected = paint.operations();
        if following.len() >= expected.len() && following.iter().zip(expected).all(|(a,b)| {
                a.operator == b.operator && a.operands.len() == b.operands.len()
                    && a.operands.iter().zip(&b.operands).all(|(a,b)| {
                        matches!((crate::redaction_content::number(a), crate::redaction_content::number(b)), (Ok(a), Ok(b)) if a == b)
                    })
            }) {
                return Ok(expected.len());
            }
        Err(invalid())
    }
    fn attach_coverage(&mut self, coverage: Coverage) -> EngineResult<()> {
        self.attach_coverage_with_paint(coverage, PaintVersion::Contours)
    }
    fn attach_coverage_with_paint(
        &mut self,
        coverage: Coverage,
        paint_version: PaintVersion,
    ) -> EngineResult<()> {
        let Payload::Jpeg {
            header,
            base_digest,
            coverage: owned,
            paint_version: shape,
            ..
        } = &mut self.payload
        else {
            return Err(invalid());
        };
        coverage.check_header(header)?;
        require(coverage.mask().contains(&1) && owned.is_none())?;
        self.digest = coverage.version_binding(base_digest, paint_version);
        *shape = paint_version;
        *owned = Some(Arc::new(coverage));
        Ok(())
    }
    fn hasher(width: u32, height: u32, components: usize) -> Sha256 {
        let mut digest = Sha256::new();
        digest.update(b"Folio sanitized image samples v1");
        digest.update(width.to_be_bytes());
        digest.update(height.to_be_bytes());
        digest.update([components as u8]);
        digest
    }
    fn owned(width: u32, height: u32, components: usize, samples: Vec<u8>) -> Self {
        let mut digest = Self::hasher(width, height, components);
        digest.update(&samples);
        Self {
            width,
            height,
            components,
            payload: Payload::Pixels(samples.into()),
            digest: digest.finalize().into(),
        }
    }
    fn owned_jpeg(encoded: jpeg::Encoded) -> Self {
        let header = encoded.header;
        let mut digest = Sha256::new();
        digest.update(b"Folio canonical JPEG coefficients v1");
        digest.update(header.width.to_be_bytes());
        digest.update(header.height.to_be_bytes());
        digest.update([header.components as u8, u8::from(header.transformed)]);
        digest.update(&encoded.bytes);
        let base_digest: [u8; 32] = digest.finalize().into();
        Self {
            width: header.width,
            height: header.height,
            components: header.components as usize,
            payload: Payload::Jpeg {
                bytes: encoded.bytes.into(),
                header,
                base_digest,
                coverage: None,
                paint_version: PaintVersion::Contours,
            },
            digest: base_digest,
        }
    }
    pub fn retained_bytes(&self) -> usize {
        match &self.payload {
            Payload::Pixels(bytes) => bytes.len(),
            Payload::Jpeg {
                bytes, coverage, ..
            } => bytes.len() + coverage.as_ref().map_or(0, |c| c.mask().len()),
        }
    }
    pub fn pixel_samples(&self) -> EngineResult<&[u8]> {
        match &self.payload {
            Payload::Pixels(bytes) => Ok(bytes),
            Payload::Jpeg { .. } => Err(invalid()),
        }
    }
    pub fn jpeg_header(&self) -> Option<&JpegHeader> {
        match &self.payload {
            Payload::Jpeg { header, .. } => Some(header),
            _ => None,
        }
    }
    pub fn jpeg_variant_key(&self, mask: &[u8]) -> EngineResult<[u8; 32]> {
        let h = self.jpeg_header().ok_or_else(invalid)?;
        require(mask.len() == (h.columns * h.rows) as usize && mask.iter().all(|v| *v <= 1))?;
        let mut digest = Sha256::new();
        digest.update(b"Folio JPEG MCU rewrite request v1");
        digest.update(self.digest);
        digest.update((mask.len() as u64).to_be_bytes());
        digest.update(mask);
        Ok(digest.finalize().into())
    }
    pub fn sanitized_jpeg(&self, mask: &[u8], limits: jpeg::Limits) -> EngineResult<Self> {
        // This low-level coefficient API must not silently drop owned paint.
        require(self.jpeg_coverage().is_none())?;
        Ok(Self::owned_jpeg(self.rewrite_jpeg(mask, limits)?))
    }
    fn rewrite_jpeg(&self, mask: &[u8], limits: jpeg::Limits) -> EngineResult<jpeg::Encoded> {
        let Payload::Jpeg { bytes, header, .. } = &self.payload else {
            return Err(invalid());
        };
        let transform = if self.components == 1 {
            Transform::Default
        } else if header.transformed {
            Transform::YCbCr
        } else {
            Transform::Rgb
        };
        jpeg::rewrite(
            bytes,
            (self.width, self.height, self.components as u32),
            transform,
            mask,
            limits,
        )
    }
    pub fn stream(&self) -> EngineResult<Stream> {
        let mut stream = Stream::new(
            dictionary! {"Type"=>"XObject","Subtype"=>"Image","Width"=>self.width as i64,
            "Height"=>self.height as i64,"BitsPerComponent"=>8,"ColorSpace"=>if self.components==1 {"DeviceGray"}else{"DeviceRGB"},
            "Interpolate"=>false},
            match &self.payload {
                Payload::Pixels(bytes) | Payload::Jpeg { bytes, .. } => bytes.to_vec(),
            },
        );
        match &self.payload {
            Payload::Pixels(_) => {
                stream.compress().map_err(|_| invalid())?;
            }
            Payload::Jpeg {
                header,
                coverage,
                paint_version,
                ..
            } => {
                stream.dict.set("Filter", "DCTDecode");
                if self.components == 3 {
                    stream.dict.set(
                        "DecodeParms",
                        dictionary! {"ColorTransform"=>i64::from(header.transformed)},
                    );
                }
                if let Some(coverage) = coverage {
                    stream.dict.set("FolioJpegRedaction",dictionary!{"V"=>*paint_version as i64,"Mask"=>Object::String(coverage.mask().to_vec(),lopdf::StringFormat::Hexadecimal),"Binding"=>Object::String(self.digest.to_vec(),lopdf::StringFormat::Hexadecimal)});
                }
            }
        }
        Ok(stream)
    }
    pub fn sanitized(&self, mask: &[bool]) -> EngineResult<Self> {
        require(mask.len() == self.width as usize * self.height as usize)?;
        let mut samples = self.pixel_samples()?.to_vec();
        for (pixel, selected) in samples.chunks_exact_mut(self.components).zip(mask) {
            if *selected {
                pixel.fill(0);
            }
        }
        Ok(Self::owned(
            self.width,
            self.height,
            self.components,
            samples,
        ))
    }
    pub fn sanitized_digest(&self, mask: &[bool]) -> EngineResult<[u8; 32]> {
        require(mask.len() == self.width as usize * self.height as usize)?;
        let mut digest = Self::hasher(self.width, self.height, self.components);
        for (pixel, selected) in self
            .pixel_samples()?
            .chunks_exact(self.components)
            .zip(mask)
        {
            digest.update(if *selected {
                &[0u8; 3][..self.components]
            } else {
                pixel
            });
        }
        Ok(digest.finalize().into())
    }
}

fn jpeg_transform(dict: &Dictionary) -> EngineResult<Transform> {
    let params = match dict.get(b"DecodeParms") {
        Err(_) | Ok(Object::Null) => return Ok(Transform::Default),
        Ok(Object::Dictionary(v)) => v,
        _ => return Err(invalid()),
    };
    require(params.iter().all(|(k, _)| k == b"ColorTransform"))?;
    match params.get(b"ColorTransform") {
        Err(_) => Ok(Transform::Default),
        Ok(Object::Integer(0)) => Ok(Transform::Rgb),
        Ok(Object::Integer(1)) => Ok(Transform::YCbCr),
        _ => Err(invalid()),
    }
}

pub(crate) struct PageImages<'a, 'b> {
    pdf: &'a Document,
    resources: Option<&'a Dictionary>,
    registry: &'b mut Registry,
    enabled: bool,
    pub images: Vec<Arc<Image>>,
}
impl<'a, 'b> PageImages<'a, 'b> {
    pub fn new(
        pdf: &'a Document,
        resources: Option<&'a Dictionary>,
        registry: &'b mut Registry,
        enabled: bool,
    ) -> Self {
        Self {
            pdf,
            resources,
            registry,
            enabled,
            images: vec![],
        }
    }
    pub fn select(&mut self, alias: &[u8]) -> EngineResult<usize> {
        require(self.enabled)?;
        let resources = self.resources.ok_or_else(invalid)?;
        let table = resolve(self.pdf, resources.get(b"XObject").map_err(|_| invalid())?)?
            .as_dict()
            .map_err(|_| invalid())?;
        let value = table.get(alias).map_err(|_| invalid())?;
        let image = self.registry.load(self.pdf, value)?;
        require(image.jpeg_header().is_none() || self.registry.jpeg_pages)?;
        if let Some(index) = self.images.iter().position(|old| Arc::ptr_eq(old, &image)) {
            return Ok(index);
        }
        let index = self.images.len();
        self.images.push(image);
        Ok(index)
    }
    pub fn normalize_operations(
        &mut self,
        mut operations: Vec<lopdf::content::Operation>,
    ) -> EngineResult<Vec<lopdf::content::Operation>> {
        // Resolve original aliases exactly once. Owned paint must be verified
        // and removed before the text canonicalizer records operation indices.
        for op in &mut operations {
            if op.operator == "Do" {
                require(op.operands.len() == 1)?;
                let index = self.select(op.operands[0].as_name().map_err(|_| invalid())?)?;
                op.operands[0] = Object::Name(format!("I{index}").into_bytes());
            }
        }
        if self
            .images
            .iter()
            .any(|image| image.jpeg_coverage().is_some())
        {
            strip_owned_paint(operations, &self.images, &mut self.registry.coverage_budget)
        } else {
            Ok(operations)
        }
    }
}
#[cfg(test)]
#[path = "redaction_images_ownership_tests.rs"]
mod ownership_tests;
#[cfg(test)]
#[path = "redaction_image_source_tests.rs"]
mod source_tests;
#[cfg(test)]
#[path = "redaction_images_tests.rs"]
mod tests;
