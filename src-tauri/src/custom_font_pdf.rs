//! Deterministic, full TrueType resources for unshaped BMP text. CIDs are Unicode
//! values, so the appearance, width table and extraction map share one encoding.
use super::{EngineError, EngineResult};
use crate::fonts::FontAsset;
use lopdf::{dictionary, Dictionary, Document, Object, Stream};
use std::{collections::HashMap, sync::Arc};

const MAX_FONT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Default)]
pub(in crate::engine) struct ImportCache {
    streams: HashMap<lopdf::ObjectId, Arc<FontAsset>>,
    assets: HashMap<String, Arc<FontAsset>>,
    bytes: usize,
}

pub(super) fn font(asset: &FontAsset) -> EngineResult<Dictionary> {
    let face = asset.face()?;
    let scale = 1000.0 / face.units_per_em() as f32;
    let name = format!("Folio{}", asset.info.id);
    let bbox = face.global_bounding_box();
    let mut mapping = vec![0u8; 65536 * 2];
    let mut widths = Vec::new();
    for [first, last] in &asset.info.coverage {
        for code in *first..=(*last).min(0xffff) {
            let Some(character) = char::from_u32(code) else {
                continue;
            };
            let Some(glyph) = face.glyph_index(character) else {
                continue;
            };
            let advance = face.glyph_hor_advance(glyph).ok_or_else(|| {
                EngineError::InvalidRequest("font lacks horizontal advances".into())
            })?;
            mapping[code as usize * 2..code as usize * 2 + 2]
                .copy_from_slice(&glyph.0.to_be_bytes());
            widths.push(Object::Integer(code as i64));
            widths.push(Object::Array(vec![Object::Real(advance as f32 * scale)]));
        }
    }
    let program = Stream::new(
        dictionary! {"Length1"=>asset.bytes.len() as i64},
        asset.bytes.to_vec(),
    );
    let descriptor = dictionary! {
        "Type"=>"FontDescriptor", "FontName"=>Object::Name(name.as_bytes().to_vec()),
        "Flags"=>32 + if face.is_monospaced() { 1 } else { 0 } + if asset.info.italic { 64 } else { 0 },
        "FontBBox"=>vec![Object::Real(bbox.x_min as f32 * scale),Object::Real(bbox.y_min as f32 * scale),Object::Real(bbox.x_max as f32 * scale),Object::Real(bbox.y_max as f32 * scale)],
        "ItalicAngle"=>Object::Real(face.italic_angle()),
        "Ascent"=>Object::Real(face.ascender() as f32 * scale),
        "Descent"=>Object::Real(face.descender() as f32 * scale),
        "CapHeight"=>Object::Real(face.capital_height().unwrap_or(face.ascender()) as f32 * scale),
        "StemV"=>80, "FontFile2"=>Object::Stream(program),
    };
    let descendant = dictionary! {
        "Type"=>"Font", "Subtype"=>"CIDFontType2", "BaseFont"=>Object::Name(name.as_bytes().to_vec()),
        "CIDSystemInfo"=>dictionary! {"Registry"=>Object::string_literal("Adobe"),"Ordering"=>Object::string_literal("Identity"),"Supplement"=>0},
        "FontDescriptor"=>descriptor, "DW"=>0, "W"=>widths,
        "CIDToGIDMap"=>Object::Stream(Stream::new(dictionary! {}, mapping)),
    };
    let unicode = b"/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /FolioUnicode def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n1 beginbfrange\n<0000> <FFFF> <0000>\nendbfrange\nendcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n";
    Ok(dictionary! {
        "Type"=>"Font", "Subtype"=>"Type0", "BaseFont"=>Object::Name(name.as_bytes().to_vec()),
        "Encoding"=>"Identity-H", "DescendantFonts"=>vec![Object::Dictionary(descendant)],
        "ToUnicode"=>Object::Stream(Stream::new(dictionary! {}, unicode.to_vec())),
    })
}

/// Streams and descendant dictionaries must become indirect PDF objects. The
/// caller caches the returned Type0 reference once per content-addressed font.
pub(super) fn install(document: &mut Document, object: Object) -> Object {
    match object {
        Object::Dictionary(values) => {
            let mut output = Dictionary::new();
            for (key, value) in values.iter() {
                let value = install(document, value.clone());
                output.set(key.clone(), value);
            }
            Object::Reference(document.add_object(output))
        }
        Object::Stream(stream) => Object::Reference(document.add_object(stream)),
        Object::Array(values) => Object::Array(
            values
                .into_iter()
                .map(|value| install(document, value))
                .collect(),
        ),
        value => value,
    }
}

/// Validate immutable bytes without registering them. The entire resource tree
/// is checked against a regenerated resource before the caller accepts metadata.
pub(super) fn embedded_asset(
    document: &Document,
    annotation: &Dictionary,
    expected_id: &str,
    cache: &mut ImportCache,
) -> Option<Arc<FontAsset>> {
    let dict = |object| document.dereference(object).ok()?.1.as_dict().ok();
    let ap = dict(annotation.get(b"AP").ok()?)?;
    let appearance = document
        .dereference(ap.get(b"N").ok()?)
        .ok()?
        .1
        .as_stream()
        .ok()?;
    let resources = dict(appearance.dict.get(b"Resources").ok()?)?;
    let fonts = dict(resources.get(b"Font").ok()?)?;
    let font = dict(fonts.get(b"FolioFont").ok()?)?;
    let descendants = document
        .dereference(font.get(b"DescendantFonts").ok()?)
        .ok()?
        .1
        .as_array()
        .ok()?;
    if descendants.len() != 1 {
        return None;
    }
    let descendant = dict(&descendants[0])?;
    let descriptor = dict(descendant.get(b"FontDescriptor").ok()?)?;
    let (program_id, program) = document
        .dereference(descriptor.get(b"FontFile2").ok()?)
        .ok()?;
    if let Some(existing) = program_id.and_then(|id| cache.streams.get(&id)) {
        return (existing.info.id == expected_id).then(|| existing.clone());
    }
    let program = program.as_stream().ok()?;
    let bytes = super::stream_content(program, MAX_FONT_BYTES)?;
    let asset = FontAsset::parse(bytes).ok()?;
    if asset.info.id != expected_id {
        return None;
    }
    let asset = if let Some(existing) = cache.assets.get(expected_id) {
        existing.clone()
    } else {
        if cache.assets.len() >= 64 || cache.bytes + asset.bytes.len() > 128 * 1024 * 1024 {
            return None;
        }
        cache.bytes += asset.bytes.len();
        let asset = Arc::new(asset);
        cache.assets.insert(expected_id.to_owned(), asset.clone());
        asset
    };
    if let Some(id) = program_id {
        cache.streams.insert(id, asset.clone());
    }
    Some(asset)
}
