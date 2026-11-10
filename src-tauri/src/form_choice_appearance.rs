//! Private appearance geometry and unchanged-source proof.
#![allow(dead_code)]
use crate::{
    form_appearance::Copier, form_choice_text::ChoiceTextBlock, EngineError, EngineResult,
};
use lopdf::{
    content::{Content, Operation},
    dictionary, Dictionary, Document, Object, ObjectId, Stream,
};
use pdfium_render::prelude::*;

const MAX_CONTENT: usize = 256 * 1024;
const MAX_PROBE: usize = 32 * 1024 * 1024;
const IDENTITY: [f64; 6] = [1., 0., 0., 1., 0., 0.];

fn bad(reason: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidRequest(format!("Choice appearance: {reason}"))
}
fn resolve<'a>(doc: &'a Document, value: &'a Object) -> EngineResult<&'a Object> {
    Ok(doc.dereference(value).map_err(bad)?.1)
}
fn dict<'a>(doc: &'a Document, value: &'a Object) -> EngineResult<&'a Dictionary> {
    resolve(doc, value)?.as_dict().map_err(bad)
}
fn numbers<const N: usize>(doc: &Document, value: &Object) -> EngineResult<[f64; N]> {
    let values = resolve(doc, value)?.as_array().map_err(bad)?;
    if values.len() != N {
        return Err(bad("invalid geometry array"));
    }
    let mut result = [0.; N];
    for (out, value) in result.iter_mut().zip(values) {
        let n = resolve(doc, value)?.as_float().map_err(bad)? as f64;
        if !n.is_finite() || n.abs() > 1_000_000. {
            return Err(bad("geometry exceeds the supported coordinate limit"));
        }
        *out = n;
    }
    Ok(result)
}
fn positive_box(doc: &Document, value: &Object) -> EngineResult<[f64; 4]> {
    let v = numbers::<4>(doc, value)?;
    if v[2] <= v[0] || v[3] <= v[1] {
        return Err(bad("empty appearance bounds"));
    }
    Ok(v)
}
fn matrix(doc: &Document, stream: &Stream) -> EngineResult<[f64; 6]> {
    let m = stream
        .dict
        .get(b"Matrix")
        .ok()
        .map(|v| numbers::<6>(doc, v))
        .transpose()?
        .unwrap_or(IDENTITY);
    let determinant = m[0] * m[3] - m[1] * m[2];
    if determinant == 0. || !determinant.is_finite() {
        return Err(bad("singular appearance matrix"));
    }
    Ok(m)
}
fn axis_matrix(values: [f64; 6]) -> EngineResult<[f32; 6]> {
    if values
        .iter()
        .any(|n| !n.is_finite() || n.abs() > 1_000_000.)
        || !(0.000001..=1_000_000.).contains(&values[0])
        || !(0.000001..=1_000_000.).contains(&values[3])
    {
        return Err(bad("appearance mapping exceeds the supported limit"));
    }
    Ok(values.map(|n| n as f32))
}

#[derive(Default)]
pub(crate) struct AppearanceBudget {
    pixels: u64,
    retained: usize,
}

impl AppearanceBudget {
    pub(crate) fn remaining_bytes(&self) -> usize {
        (32 * 1024 * 1024) - self.retained
    }
    pub(crate) fn reserve_staged(&mut self, bytes: usize) -> EngineResult<()> {
        if bytes > self.remaining_bytes() {
            return Err(bad("retained appearances exceed the byte budget"));
        }
        self.retained += bytes;
        Ok(())
    }
    fn reserve_renders(
        &mut self,
        width: f64,
        height: f64,
        unit: f32,
    ) -> EngineResult<[(i32, i32); 3]> {
        let mut sizes = [(0, 0); 3];
        let mut charge = 0u64;
        for (size, scale) in sizes.iter_mut().zip([1., 2., 4.]) {
            let w = (width * unit as f64 * scale).ceil();
            let h = (height * unit as f64 * scale).ceil();
            if !w.is_finite()
                || !h.is_finite()
                || !(1. ..=32768.).contains(&w)
                || !(1. ..=32768.).contains(&h)
            {
                return Err(bad(
                    "physical appearance dimensions exceed the rendering limit",
                ));
            }
            let pixels = w as u64 * h as u64;
            if pixels > 16_000_000 {
                return Err(bad("appearance bitmap exceeds the pixel budget"));
            }
            charge += pixels * 2;
            *size = (w as i32, h as i32);
        }
        if charge > 64_000_000u64.saturating_sub(self.pixels) {
            return Err(bad("linked appearances exceed the rendering budget"));
        }
        self.pixels += charge;
        Ok(sizes)
    }
}

pub(crate) fn geometry(
    source: &Document,
    original: &Stream,
    generated: &Document,
    replacement: &Stream,
) -> EngineResult<[f32; 6]> {
    let s = positive_box(source, original.dict.get(b"BBox").map_err(bad)?)?;
    let g = positive_box(generated, replacement.dict.get(b"BBox").map_err(bad)?)?;
    matrix(source, original)?;
    matrix(generated, replacement)?;
    let x = (s[2] - s[0]) / (g[2] - g[0]);
    let y = (s[3] - s[1]) / (g[3] - g[1]);
    axis_matrix([x, 0., 0., y, s[0] - x * g[0], s[1] - y * g[1]])
}

pub(crate) fn bound_resources(
    doc: &Document,
    appearance: &Stream,
    content: &[u8],
) -> EngineResult<()> {
    ChoiceTextBlock::parse(content)
        .or_else(|_| ChoiceTextBlock::parse_empty_background(content))?;
    let resources = appearance
        .dict
        .get(b"Resources")
        .ok()
        .map(|v| dict(doc, v))
        .transpose()?;
    // A standalone proof may not silently lose the page/AcroForm resource
    // fallback and then authorize an edit by comparing two blank renderings.
    for op in Content::decode_strict(content).map_err(bad)?.operations {
        let category: &[u8] = match op.operator.as_str() {
            "Tf" => b"Font",
            "Do" => b"XObject",
            "gs" => b"ExtGState",
            "sh" => b"Shading",
            _ => continue,
        };
        let name = op.operands[0].as_name().map_err(bad)?;
        let entries = resources
            .and_then(|r| r.get(category).ok())
            .ok_or_else(|| bad("a saved appearance resource is unavailable locally"))?;
        let entry = dict(doc, entries)?
            .get(name)
            .map_err(|_| bad("a saved appearance resource name is unavailable locally"))?;
        let resolved = resolve(doc, entry)?;
        match category {
            b"Font" | b"ExtGState" => {
                resolved.as_dict().map_err(bad)?;
            }
            b"XObject" => {
                resolved.as_stream().map_err(bad)?;
            }
            _ => {
                if !matches!(resolved, Object::Dictionary(_) | Object::Stream(_)) {
                    return Err(bad("invalid saved appearance resource"));
                }
            }
        }
    }
    Ok(())
}

fn write_probe(doc: &mut Document) -> EngineResult<Vec<u8>> {
    let mut writer = crate::pdf_output::BoundedWriter::new(MAX_PROBE);
    let saved = doc.save_to(&mut writer);
    let bytes = writer.finish()?;
    saved.map_err(bad)?;
    Ok(bytes)
}

fn raster(page: &PdfPage<'_>, width: i32, height: i32) -> EngineResult<Vec<u8>> {
    let w = page.width().value;
    let h = page.height().value;
    if !w.is_finite() || !h.is_finite() || w <= 0. || h <= 0. {
        return Err(bad("native appearance page has invalid dimensions"));
    }
    let sx = width as f32 / w;
    let sy = height as f32 / h;
    if !sx.is_finite() || !sy.is_finite() {
        return Err(bad("native appearance scale exceeds the limit"));
    }
    let bytes = width as usize * height as usize * 4;
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(bytes)
        .map_err(|_| bad("could not allocate the bounded appearance bitmap"))?;
    buffer.resize(bytes, 0);
    {
        let mut bitmap = PdfBitmap::from_bytes(width, height, PdfBitmapFormat::BGRA, &mut buffer)?;
        page.render_into_bitmap_with_config(
            &mut bitmap,
            &PdfRenderConfig::new()
                .set_fixed_size(width, height)
                .scale_page_width_by_factor(sx)
                .scale_page_height_by_factor(sy)
                .set_format(PdfBitmapFormat::BGRA)
                .set_reverse_byte_order(false)
                .clear_before_rendering(false)
                .render_annotations(false)
                .render_form_data(false),
        )?;
    }
    // The external bitmap is gone before its backing buffer is accessed. This
    // compares alpha as well as color and avoids extra image conversion copies.
    Ok(buffer)
}

pub(crate) fn verify_unchanged(
    pdfium: &Pdfium,
    source: &Document,
    widget: ObjectId,
    page: ObjectId,
    candidate_content: &[u8],
    budget: &mut AppearanceBudget,
) -> EngineResult<()> {
    if candidate_content.len() > MAX_CONTENT {
        return Err(bad("replacement appearance exceeds the byte limit"));
    }
    let placement = source.get_dictionary(page).map_err(bad)?;
    let annots = resolve(source, placement.get(b"Annots").map_err(bad)?)?
        .as_array()
        .map_err(bad)?;
    if !annots.iter().any(|v| v.as_reference().ok() == Some(widget)) {
        return Err(bad("widget is not owned by the source page"));
    }
    let w = source.get_dictionary(widget).map_err(bad)?;
    let rect = positive_box(source, w.get(b"Rect").map_err(bad)?)?;
    let normal_object = dict(source, w.get(b"AP").map_err(bad)?)?
        .get(b"N")
        .map_err(bad)?;
    let normal = resolve(source, normal_object)?.as_stream().map_err(bad)?;
    let bbox = positive_box(source, normal.dict.get(b"BBox").map_err(bad)?)?;
    let m = matrix(source, normal)?;
    let width = rect[2] - rect[0];
    let height = rect[3] - rect[1];
    // Match the transformed appearance BBox to the normalized annotation Rect,
    // exactly as AnnotGetMatrix does. Its own Form Matrix is applied once by Do.
    let mut bounds = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for x in [bbox[0], bbox[2]] {
        for y in [bbox[1], bbox[3]] {
            let px = m[0] * x + m[2] * y + m[4];
            let py = m[1] * x + m[3] * y + m[5];
            bounds[0] = bounds[0].min(px);
            bounds[1] = bounds[1].min(py);
            bounds[2] = bounds[2].max(px);
            bounds[3] = bounds[3].max(py);
        }
    }
    let sx = width / (bounds[2] - bounds[0]);
    let sy = height / (bounds[3] - bounds[1]);
    let placement_matrix = axis_matrix([sx, 0., 0., sy, -bounds[0] * sx, -bounds[1] * sy])?;
    let unit = crate::engine::source_unit_from_pdf(source, page)?;
    let sizes = budget.reserve_renders(width, height, unit)?;
    let original_content = normal
        .get_plain_content_with_limit(MAX_CONTENT)
        .map_err(bad)?;
    bound_resources(source, normal, &original_content)?;
    bound_resources(source, normal, candidate_content)?;

    let mut probe = Document::with_version("1.7");
    let copied = Copier::new(source, &mut probe).copy(normal_object)?;
    let form = match copied {
        Object::Reference(id) => id,
        Object::Stream(stream) => probe.add_object(stream),
        _ => return Err(bad("normal appearance is not a stream")),
    };
    {
        let stream = probe
            .get_object_mut(form)
            .map_err(bad)?
            .as_stream_mut()
            .map_err(bad)?;
        // Annotation AP streams are interpreted as Forms even without these
        // entries. Supply them only in the private XObject probe, not the source.
        stream.dict.set("Type", "XObject");
        stream.dict.set("Subtype", "Form");
    }
    let content = Content {
        operations: vec![
            Operation::new("q", vec![]),
            Operation::new(
                "cm",
                placement_matrix.into_iter().map(Object::Real).collect(),
            ),
            Operation::new("Do", vec![Object::Name(b"ChoiceProof".to_vec())]),
            Operation::new("Q", vec![]),
        ],
    }
    .encode()
    .map_err(bad)?;
    let contents = probe.add_object(Stream::new(Dictionary::new(), content));
    let pages = probe.new_object_id();
    let p = probe.add_object(dictionary! {"Type"=>"Page", "Parent"=>pages,
    "MediaBox"=>vec![0.into(),0.into(),Object::Real(width as f32),Object::Real(height as f32)],
    "Resources"=>dictionary! {"XObject"=>dictionary! {"ChoiceProof"=>form}}, "Contents"=>contents});
    probe.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Kids"=>vec![p.into()],"Count"=>1}.into(),
    );
    let root = probe.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    probe.trailer.set("Root", root);
    let original_bytes = write_probe(&mut probe)?;
    probe
        .get_object_mut(form)
        .map_err(bad)?
        .as_stream_mut()
        .map_err(bad)?
        .set_plain_content(candidate_content.to_vec());
    let replacement_bytes = write_probe(&mut probe)?;
    drop(probe);
    let original_doc = pdfium.load_pdf_from_byte_slice(&original_bytes, None)?;
    let replacement_doc = pdfium.load_pdf_from_byte_slice(&replacement_bytes, None)?;
    let original_page = original_doc.pages().get(0)?;
    let replacement_page = replacement_doc.pages().get(0)?;
    for (width, height) in sizes {
        let before = raster(&original_page, width, height)?;
        let after = raster(&replacement_page, width, height)?;
        if before != after {
            return Err(bad("the saved appearance differs from unchanged regeneration; its text or artwork cannot be preserved"));
        }
    }
    Ok(())
}
