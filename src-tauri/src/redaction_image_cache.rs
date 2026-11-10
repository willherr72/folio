//! One edit's explicitly owned image proofs; ObjectId aliases stay document-local.
use super::*;

pub(crate) struct ImageContext {
    registry: Registry,
    variant_bytes: usize,
    variants: HashMap<[u8; 32], Arc<Image>>,
}
impl Default for ImageContext {
    fn default() -> Self {
        Self {
            registry: Registry {
                reuse: Some(Reuse::default()),
                ..Registry::default()
            },
            variant_bytes: 0,
            variants: HashMap::new(),
        }
    }
}
impl ImageContext {
    /// Explicit combined-content preview admission; text-only/default callers
    /// retain their narrower contract.
    pub(crate) fn for_content_preview() -> Self {
        let mut context = Self::default();
        context.registry.jpeg_pages = true;
        context
    }
    #[cfg(test)]
    pub(crate) fn for_jpeg_validation() -> Self {
        Self::for_content_preview()
    }
    pub fn begin_document(&mut self) -> &mut Registry {
        self.registry.images.clear();
        &mut self.registry
    }
    fn owns(&self, image: &Arc<Image>) -> EngineResult<()> {
        require(
            self.registry
                .reuse
                .as_ref()
                .and_then(|r| r.owned.get(&image.digest))
                .is_some_and(|owned| Arc::ptr_eq(owned, image)),
        )
    }
    pub fn jpeg_selection(
        &mut self,
        original: &Arc<Image>,
        pixels: &[bool],
    ) -> EngineResult<Coverage> {
        self.owns(original)?;
        Coverage::from_pixels(
            original.jpeg_header().ok_or_else(invalid)?,
            pixels,
            &mut self.registry.coverage_budget,
        )
    }
    pub fn jpeg_paint_geometry(
        &mut self,
        selected: &Coverage,
        matrix: crate::redaction_area_paths::ImageTransform,
    ) -> EngineResult<(usize, Vec<crate::redaction_area_paths::AreaRect>)> {
        let rectangles = selected.rectangles(&mut self.registry.coverage_budget)?;
        let pixels = rectangles
            .iter()
            .map(|r| (r[2] - r[0]) as usize * (r[3] - r[1]) as usize)
            .sum();
        let paint = selected.paint(&mut self.registry.coverage_budget)?;
        Ok((pixels, paint.page_bounds(matrix)?))
    }
    pub fn redact_jpeg(
        &mut self,
        original: &Arc<Image>,
        selected: &Coverage,
    ) -> EngineResult<Arc<Image>> {
        self.owns(original)?;
        let header = original.jpeg_header().ok_or_else(invalid)?;
        selected.check_header(header)?;
        require(selected.mask().contains(&1))?;
        let coverage = if let Some(prior) = original.jpeg_coverage() {
            let union = prior.union(selected, &mut self.registry.coverage_budget)?;
            if union.mask() == prior.mask() && !original.jpeg_needs_paint_upgrade() {
                return Ok(original.clone());
            }
            union
        } else {
            Coverage::from_mask(header, selected.mask(), &mut self.registry.coverage_budget)?
        };
        self.registry.charge_hash(coverage.mask().len() + 128)?;
        let key = original.jpeg_variant_key(coverage.mask())?;
        if let Some(image) = self.variants.get(&key) {
            return Ok(image.clone());
        }
        require(self.variants.len() < 4096)?;
        self.registry.variant_capacity()?;
        let remaining = SAMPLE_BUDGET.saturating_sub(self.variant_bytes);
        require(coverage.mask().len() < remaining)?;
        let work = jpeg::Limits::default()
            .coefficient_work
            .saturating_sub(self.registry.jpeg_work);
        require(header.coefficient_work <= work)?;
        // An attempted codec call is charged even if output admission fails.
        self.registry.jpeg_work += header.coefficient_work;
        let encoded = original.rewrite_jpeg(
            coverage.mask(),
            jpeg::Limits {
                output_bytes: remaining - coverage.mask().len(),
                coefficient_work: work,
                ..jpeg::Limits::default()
            },
        )?;
        self.registry.charge_hash(encoded.bytes.len() + 128)?;
        self.registry.charge_hash(coverage.mask().len() + 128)?;
        let mut image = Image::owned_jpeg(encoded);
        image.attach_coverage(coverage)?;
        let (image, new) = self.registry.install_owned(Arc::new(image), None)?;
        if new {
            self.variant_bytes += image.retained_bytes();
        }
        self.variants.insert(key, image.clone());
        Ok(image)
    }
    pub fn redact_pixels(
        &mut self,
        original: &Arc<Image>,
        mask: &[bool],
    ) -> EngineResult<Arc<Image>> {
        self.owns(original)?;
        require(mask.len() == original.width as usize * original.height as usize)?;
        let length = original.pixel_samples()?.len();
        self.registry.charge_hash(length + 128)?;
        let digest = original.sanitized_digest(mask)?;
        if let Some(image) = self
            .registry
            .reuse
            .as_ref()
            .and_then(|r| r.owned.get(&digest))
        {
            return Ok(image.clone());
        }
        require(length <= SAMPLE_BUDGET.saturating_sub(self.variant_bytes))?;
        self.registry.variant_capacity()?;
        self.registry.charge_hash(length + 128)?;
        let image = Arc::new(original.sanitized(mask)?);
        require(image.digest == digest)?;
        let (image, new) = self.registry.install_owned(image, None)?;
        if new {
            self.variant_bytes += image.retained_bytes();
        }
        Ok(image)
    }
}

const HASH_BYTES: usize = 512 * 1024 * 1024;
#[derive(Default)]
pub(super) struct Reuse {
    inputs: HashMap<[u8; 32], Arc<Image>>,
    owned: HashMap<[u8; 32], Arc<Image>>,
    hashed_bytes: usize,
    source_misses: usize,
}
pub(super) enum Interpretation<'a> {
    Pixels {
        compressed: bool,
        predictor: i64,
    },
    Jpeg {
        transformed: bool,
        mask: &'a [u8],
        binding: &'a [u8],
        paint_version: u8,
    },
}
impl Registry {
    fn variant_capacity(&self) -> EngineResult<()> {
        let reuse = self.reuse.as_ref().ok_or_else(invalid)?;
        require(reuse.inputs.len() <= 1022 && reuse.owned.len() < 512)
    }
    pub(super) fn charge_hash(&mut self, amount: usize) -> EngineResult<()> {
        if let Some(reuse) = self.reuse.as_mut() {
            require(amount <= HASH_BYTES.saturating_sub(reuse.hashed_bytes))?;
            reuse.hashed_bytes += amount;
        }
        Ok(())
    }
    pub(super) fn charge_input(&mut self, amount: usize) -> EngineResult<()> {
        require(amount <= SAMPLE_BUDGET.saturating_sub(self.input_bytes))?;
        self.input_bytes += amount;
        Ok(())
    }
    pub(super) fn input_key(
        &mut self,
        bytes: &[u8],
        shape: (u32, u32, usize),
        interpretation: Interpretation<'_>,
    ) -> EngineResult<Option<[u8; 32]>> {
        if self.reuse.is_none() {
            return Ok(None);
        };
        let record_bytes = match &interpretation {
            Interpretation::Jpeg { mask, binding, .. } => mask.len() + binding.len(),
            _ => 0,
        };
        // Include a conservative envelope charge, not only the large payload.
        let amount = bytes
            .len()
            .checked_add(record_bytes)
            .and_then(|n| n.checked_add(128))
            .ok_or_else(invalid)?;
        self.charge_hash(amount)?;
        let mut hash = Sha256::new();
        hash.update(b"Folio validated image input v1");
        hash.update(shape.0.to_be_bytes());
        hash.update(shape.1.to_be_bytes());
        hash.update((shape.2 as u32).to_be_bytes());
        match interpretation {
            Interpretation::Pixels {
                compressed,
                predictor,
            } => {
                hash.update([u8::from(compressed)]);
                hash.update(predictor.to_be_bytes());
            }
            Interpretation::Jpeg {
                transformed,
                mask,
                binding,
                paint_version,
            } => {
                hash.update([2, u8::from(transformed), paint_version]);
                hash.update((mask.len() as u64).to_be_bytes());
                hash.update(mask);
                hash.update(binding);
            }
        }
        hash.update((bytes.len() as u64).to_be_bytes());
        hash.update(bytes);
        Ok(Some(hash.finalize().into()))
    }
    pub(super) fn reuse_input(
        &mut self,
        id: ObjectId,
        key: Option<[u8; 32]>,
    ) -> EngineResult<Option<Arc<Image>>> {
        let Some(key) = key else { return Ok(None) };
        let reuse = self.reuse.as_mut().ok_or_else(invalid)?;
        if let Some(image) = reuse.inputs.get(&key) {
            let image = image.clone();
            self.images.insert(id, image.clone());
            return Ok(Some(image));
        }
        require(
            reuse.source_misses < 256 && reuse.inputs.len() <= 1022 && reuse.owned.len() < 512,
        )?;
        reuse.source_misses += 1;
        Ok(None)
    }
    fn canonical_key(&mut self, image: &Image) -> EngineResult<Option<[u8; 32]>> {
        if self.reuse.is_none() {
            return Ok(None);
        }
        let shape = (image.width, image.height, image.components);
        match &image.payload {
            Payload::Jpeg {
                bytes,
                header,
                coverage,
                ..
            } => {
                let binding = coverage.as_ref().map(|_| image.digest);
                self.input_key(
                    bytes,
                    shape,
                    Interpretation::Jpeg {
                        transformed: header.transformed,
                        mask: coverage.as_ref().map_or(&[], |c| c.mask()),
                        binding: binding.as_ref().map_or(&[], |b| b.as_slice()),
                        paint_version: image.jpeg_paint_version().map_or(0, |v| v as u8),
                    },
                )
            }
            Payload::Pixels(_) => {
                // Match the exact fresh image representation the publisher uses.
                // This temporary compression is bounded by admitted sample size.
                let stream = image.stream()?;
                self.input_key(
                    &stream.content,
                    shape,
                    Interpretation::Pixels {
                        compressed: stream.dict.has(b"Filter"),
                        predictor: 1,
                    },
                )
            }
        }
    }
    pub(super) fn install_source(
        &mut self,
        id: ObjectId,
        image: Arc<Image>,
        key: Option<[u8; 32]>,
    ) -> EngineResult<Arc<Image>> {
        let (image, new) = self.install_owned(image, key)?;
        if new {
            self.bytes += image.retained_bytes();
        }
        self.images.insert(id, image.clone());
        Ok(image)
    }
    fn install_owned(
        &mut self,
        image: Arc<Image>,
        input: Option<[u8; 32]>,
    ) -> EngineResult<(Arc<Image>, bool)> {
        if self.reuse.is_none() {
            return Ok((image, true));
        }
        let canonical = self.canonical_key(&image)?.ok_or_else(invalid)?;
        let reuse = self.reuse.as_mut().ok_or_else(invalid)?;
        require(reuse.inputs.len() <= 1022)?;
        let (image, new) = if let Some(previous) = reuse.owned.get(&image.digest) {
            (previous.clone(), false)
        } else {
            require(reuse.owned.len() < 512)?;
            (image, true)
        };
        if let Some(previous) = reuse.inputs.get(&canonical) {
            require(previous.digest == image.digest)?;
        }
        // All expensive validation, canonical-key hashing and bounds succeeded.
        if new {
            reuse.owned.insert(image.digest, image.clone());
        }
        if let Some(key) = input {
            reuse.inputs.insert(key, image.clone());
        }
        reuse.inputs.insert(canonical, image.clone());
        Ok((image, new))
    }
}

#[cfg(test)]
#[path = "redaction_image_cache_tests.rs"]
mod tests;
