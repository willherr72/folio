//! Admission limits for generated overlay graphs, before installation copies.
use super::{Dictionary, Document, EngineError, EngineResult, FontAsset, Object, Stream};
#[derive(Clone, Copy)]
pub(in crate::engine) struct Limits {
    pub objects: usize,
    pub values: usize,
    pub bytes: usize,
    pub stream_bytes: usize,
    pub depth: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            objects: 100_000,
            values: 1_000_000,
            bytes: 256 * 1024 * 1024,
            stream_bytes: 64 * 1024 * 1024,
            depth: 64,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Usage {
    objects: usize,
    values: usize,
    bytes: usize,
}
#[derive(Clone, Copy)]
pub(super) struct Budget {
    limits: Limits,
    used: Usage,
}
#[derive(Clone, Copy)]
enum Mode {
    Direct,
    Font,
    Image,
}
fn bad() -> EngineError {
    EngineError::InvalidRequest("generated overlay resources exceed the preservation budget".into())
}
fn take(value: &mut usize, amount: usize, limit: usize) -> EngineResult<()> {
    *value = value
        .checked_add(amount)
        .filter(|v| *v <= limit)
        .ok_or_else(bad)?;
    Ok(())
}
impl Budget {
    pub(super) fn new(
        document: &Document,
        limits: Limits,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<Self> {
        let mut result = Self {
            limits,
            used: Usage::default(),
        };
        take(
            &mut result.used.objects,
            document.objects.len(),
            limits.objects,
        )?;
        take(&mut result.used.bytes, document.version.len(), limits.bytes)?;
        result.dictionary(&document.trailer, Mode::Direct, 0, check)?;
        for value in document.objects.values() {
            result.value(value, Mode::Direct, 0, check)?;
        }
        Ok(result)
    }
    // Check the full-font writer's bounded program, BMP map and width table
    // before constructing them. Coverage may overestimate absent glyphs; exact
    // generated objects are charged once by font() before installation.
    pub(super) fn font_inputs(&self, asset: &FontAsset) -> EngineResult<()> {
        let mut preview = *self;
        let mut codes = 0usize;
        for [first, last] in &asset.info.coverage {
            let last = (*last).min(0xffff);
            if *first <= last {
                codes = codes
                    .checked_add((last - *first + 1) as usize)
                    .ok_or_else(bad)?;
            }
        }
        if asset.bytes.len() > self.limits.stream_bytes || 131_072 > self.limits.stream_bytes {
            return Err(bad());
        }
        take(&mut preview.used.objects, 16, self.limits.objects)?;
        take(
            &mut preview.used.values,
            codes
                .checked_mul(3)
                .and_then(|n| n.checked_add(256))
                .ok_or_else(bad)?,
            self.limits.values,
        )?;
        let bytes = asset
            .bytes
            .len()
            .checked_add(131_072 + 16_384)
            .and_then(|n| n.checked_add(asset.info.id.len() * 4))
            .ok_or_else(bad)?;
        take(&mut preview.used.bytes, bytes, self.limits.bytes)
    }
    pub(super) fn font(
        &mut self,
        value: &Object,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<()> {
        self.value(value, Mode::Font, 0, check)
    }
    pub(super) fn image(
        &mut self,
        stream: &Stream,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<()> {
        take(&mut self.used.objects, 1, self.limits.objects)?;
        take(&mut self.used.values, 2, self.limits.values)?;
        self.stream(&stream.dict, &stream.content, Mode::Image, 0, check)
    }
    pub(super) fn attachment(
        &mut self,
        dict: &Dictionary,
        bytes: &[u8],
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<()> {
        take(&mut self.used.objects, 1, self.limits.objects)?;
        take(&mut self.used.values, 2, self.limits.values)?;
        self.stream(dict, bytes, Mode::Direct, 0, check)
    }
    pub(super) fn object(
        &mut self,
        value: &Object,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<()> {
        take(&mut self.used.objects, 1, self.limits.objects)?;
        self.value(value, Mode::Direct, 0, check)
    }
    pub(super) fn annotation(
        &mut self,
        value: &Object,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<()> {
        // Also reserve the new page-array reference and possible Annots field.
        take(&mut self.used.values, 2, self.limits.values)?;
        take(&mut self.used.bytes, 6, self.limits.bytes)?;
        self.object(value, check)
    }
    fn value(
        &mut self,
        value: &Object,
        mode: Mode,
        depth: usize,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<()> {
        check()?;
        if depth > self.limits.depth {
            return Err(bad());
        }
        take(&mut self.used.values, 1, self.limits.values)?;
        let indirect = matches!(
            (mode, value),
            (Mode::Font, Object::Dictionary(_)) | (Mode::Font | Mode::Image, Object::Stream(_))
        );
        if indirect {
            take(&mut self.used.objects, 1, self.limits.objects)?;
            take(&mut self.used.values, 1, self.limits.values)?;
        }
        match value {
            Object::String(bytes, _) | Object::Name(bytes) => {
                take(&mut self.used.bytes, bytes.len(), self.limits.bytes)?
            }
            Object::Array(values) => {
                for value in values {
                    self.value(value, mode, depth + 1, check)?;
                }
            }
            Object::Dictionary(dict) => self.dictionary(dict, mode, depth, check)?,
            Object::Stream(stream) => {
                self.stream(&stream.dict, &stream.content, mode, depth, check)?
            }
            Object::Real(v) if !v.is_finite() => return Err(bad()),
            _ => (),
        }
        Ok(())
    }
    fn dictionary(
        &mut self,
        dict: &Dictionary,
        mode: Mode,
        depth: usize,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<()> {
        for (key, value) in dict.iter() {
            take(&mut self.used.bytes, key.len(), self.limits.bytes)?;
            self.value(value, mode, depth + 1, check)?;
        }
        Ok(())
    }
    fn stream(
        &mut self,
        dict: &Dictionary,
        bytes: &[u8],
        mode: Mode,
        depth: usize,
        check: &mut impl FnMut() -> EngineResult<()>,
    ) -> EngineResult<()> {
        check()?;
        if bytes.len() > self.limits.stream_bytes {
            return Err(bad());
        }
        take(&mut self.used.bytes, bytes.len(), self.limits.bytes)?;
        self.dictionary(dict, mode, depth, check)
    }
}
