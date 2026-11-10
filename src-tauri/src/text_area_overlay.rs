//! Compose bounded line preparations into one editable area appearance.
use crate::{
    layout_text_area, prepare_text_overlay, AnnotationRect, EngineError, EngineResult, FontAsset,
    NativeTextPreview, PdfTextCharacter, PreparedTextOverlay, TextAreaRequest, TextDirection,
    TextOverlay,
};
use lopdf::{content::Content, dictionary, Dictionary, Object};
use std::collections::{BTreeMap, BTreeSet};

const MAX_BYTES: usize = 8 * 1024 * 1024;
fn invalid(message: &str) -> EngineError {
    EngineError::InvalidRequest(message.into())
}
fn reserve(remaining: &mut usize, bytes: usize) -> EngineResult<()> {
    *remaining = remaining
        .checked_sub(bytes)
        .ok_or_else(|| invalid("Text area appearance exceeds its byte limit."))?;
    Ok(())
}
// Resources are trusted, acyclic native output. Bound the complete tree before
// retaining another line; streams include uncompressed font programs and CMaps.
fn resource_budget(object: &Object, remaining: &mut usize) -> EngineResult<()> {
    reserve(remaining, 32)?;
    match object {
        Object::String(bytes, _) | Object::Name(bytes) => reserve(remaining, bytes.len())?,
        Object::Array(values) => {
            for value in values {
                resource_budget(value, remaining)?;
            }
        }
        Object::Dictionary(dict) => {
            for (key, value) in dict.iter() {
                reserve(remaining, key.len())?;
                resource_budget(value, remaining)?;
            }
        }
        Object::Stream(stream) => {
            reserve(remaining, stream.content.len())?;
            for (key, value) in stream.dict.iter() {
                reserve(remaining, key.len())?;
                resource_budget(value, remaining)?;
            }
        }
        _ => (),
    }
    Ok(())
}

pub(crate) fn prepare(
    font: &FontAsset,
    overlay: &TextOverlay,
    color: [f32; 3],
) -> EngineResult<PreparedTextOverlay> {
    overlay.validate_area_settings()?;
    let area = overlay.area.as_ref().expect("area dispatch");
    let layout = layout_text_area(
        font,
        &TextAreaRequest {
            version: u32::from(area.version),
            text: overlay.text.clone(),
            font_size: overlay.font_size,
            width: area.width,
            height: area.height,
            inset: area.inset,
            line_spacing: area.line_spacing,
            alignment: area.alignment,
            direction: TextDirection::Ltr,
            ligatures: overlay
                .shaping
                .as_ref()
                .expect("validated shaping")
                .ligatures,
        },
    )?;
    if !layout.can_export {
        return Err(invalid("Text overflows its saved area. Shorten the text or reduce its font size before saving."));
    }
    let mut result = PreparedTextOverlay {
        preview: NativeTextPreview {
            version: 1,
            font_id: font.info.id.clone(),
            text: overlay.text.clone(),
            width: area.width,
            height: area.height,
            rotation: 0,
            color,
            outlines: vec![],
            glyphs: vec![],
        },
        origin: [0., area.height - overlay.font_size],
        bounds: AnnotationRect {
            x: 0.,
            y: 0.,
            width: area.width,
            height: area.height,
        },
        characters: vec![],
        content: vec![],
        resources: Dictionary::new(),
    };
    let mut fonts = Dictionary::new();
    let mut outlines = BTreeSet::new();
    let (mut content_budget, mut preview_budget, mut fonts_budget) =
        (MAX_BYTES, MAX_BYTES, MAX_BYTES * 2);
    reserve(&mut preview_budget, overlay.text.len() * 6 + 1024)?;
    for (index, line) in layout.lines.iter().enumerate() {
        if line.shaped.is_some() {
            let mut single = overlay.clone();
            single.area = None;
            single.text = overlay.text[line.source.utf8_start..line.source.utf8_end].into();
            let prepared = prepare_text_overlay(font, &single)?;
            let dx = line.baseline_x - prepared.origin[0];
            let dy = area.height - line.baseline_y - prepared.origin[1];
            let mut names = BTreeMap::new();
            if let Ok(line_fonts) = prepared.resources.get(b"Font").and_then(Object::as_dict) {
                for (name, object) in line_fonts.iter() {
                    resource_budget(object, &mut fonts_budget)?;
                    let unique =
                        format!("Area{index}_{}", String::from_utf8_lossy(name)).into_bytes();
                    fonts.set(unique.clone(), object.clone());
                    names.insert(name.clone(), unique);
                }
            }
            let mut content = Content::decode(&prepared.content)
                .map_err(|_| invalid("Could not compose area content."))?;
            for operation in &mut content.operations {
                if operation.operator == "Tf" {
                    let name = operation
                        .operands
                        .first_mut()
                        .ok_or_else(|| invalid("Missing native font name."))?;
                    let old = name
                        .as_name()
                        .map_err(|_| invalid("Invalid native font name."))?;
                    *name = Object::Name(
                        names
                            .get(old)
                            .ok_or_else(|| invalid("Missing native font resource."))?
                            .clone(),
                    );
                }
            }
            let encoded = content
                .encode()
                .map_err(|_| invalid("Could not encode area content."))?;
            let prefix = format!("q 1 0 0 1 {dx:.7} {dy:.7} cm\n");
            reserve(&mut content_budget, prefix.len() + encoded.len() + 3)?;
            result.content.extend_from_slice(prefix.as_bytes());
            result.content.extend_from_slice(&encoded);
            result.content.extend_from_slice(b"\nQ\n");
            for outline in prepared.preview.outlines {
                if outlines.insert(outline.glyph_id) {
                    reserve(&mut preview_budget, outline.path.len() + 64)?;
                    result.preview.outlines.push(outline);
                }
            }
            for mut glyph in prepared.preview.glyphs {
                reserve(&mut preview_budget, 256)?;
                glyph.transform[4] += f64::from(dx);
                glyph.transform[5] += f64::from(dy);
                result.preview.glyphs.push(glyph);
            }
            for mut character in prepared.characters {
                character.x += line.baseline_x;
                character.y += line.baseline_y - overlay.font_size;
                result.characters.push(character);
            }
        }
        // Delimiters are logical source text, never inferred from reader output.
        // Zero-width boxes keep unpainted spaces and CR/LF addressable in Folio.
        for character in overlay.text[line.delimiter.utf8_start..line.delimiter.utf8_end].chars() {
            result.characters.push(PdfTextCharacter {
                text: character.to_string(),
                x: line.baseline_x + line.advance_width,
                y: line.baseline_y - overlay.font_size,
                width: 0.,
                height: overlay.font_size,
            });
        }
    }
    result.resources = dictionary! {"Font"=>fonts};
    if result.content.is_empty() {
        result.content.extend_from_slice(b"q Q\n");
    }
    Ok(result)
}
