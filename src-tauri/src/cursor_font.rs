//! Selected-only resource adaptation for a proved dependent text cursor.
//! Names identify the exact planned resource; a name alone grants no authority.
use super::*;
use lopdf::{Dictionary, Document, Object};
use std::collections::BTreeSet;

pub(super) struct Plan {
    pub width_delta: f64,
    pub name: Option<String>,
    changes: Vec<(u16, f32)>,
    program: String,
}
#[derive(Clone, Copy)]
pub(super) enum AssetWidths {
    Integral,
    Fractional,
}
fn bad() -> EngineError {
    invalid("The dependent text cursor font cannot be verified safely.")
}
fn dict(pdf: &Document, value: &Object) -> EngineResult<Dictionary> {
    pdf.dereference(value)
        .and_then(|(_, v)| v.as_dict())
        .cloned()
        .map_err(|_| bad())
}
fn child(pdf: &Document, font: &Dictionary) -> EngineResult<Dictionary> {
    let values = pdf
        .dereference(font.get(b"DescendantFonts").map_err(|_| bad())?)
        .and_then(|(_, v)| v.as_array())
        .map_err(|_| bad())?;
    if values.len() != 1 {
        return Err(bad());
    }
    dict(pdf, &values[0])
}
fn static_face(bytes: &[u8]) -> EngineResult<ttf_parser::Face<'_>> {
    let face = ttf_parser::Face::parse(bytes, 0).map_err(|_| bad())?;
    if face.tables().glyf.is_none()
        || face.tables().hmtx.is_none()
        || face.tables().cff.is_some()
        || face
            .raw_face()
            .table(ttf_parser::Tag::from_bytes(b"CFF2"))
            .is_some()
        || face.is_variable()
    {
        return Err(bad());
    }
    Ok(face)
}
fn nominal(face: &ttf_parser::Face<'_>, gid: u16) -> EngineResult<f32> {
    Ok(face
        .glyph_hor_advance(ttf_parser::GlyphId(gid))
        .ok_or_else(bad)? as f32
        * 1000.
        / face.units_per_em() as f32)
}
fn name(pdf: &Document, program: &str, changes: &[(u16, f32)]) -> EngineResult<String> {
    let mut seed = program.as_bytes().to_vec();
    seed.extend_from_slice(&pdf.max_id.to_be_bytes());
    for (code, w) in changes {
        seed.extend_from_slice(&code.to_be_bytes());
        seed.extend_from_slice(&w.to_bits().to_be_bytes());
    }
    let base = format!("FolioCursor{}", &embedded_font::key(&seed)[..24]);
    let candidates = (0..=64)
        .map(|suffix| {
            if suffix == 0 {
                base.clone()
            } else {
                format!("{base}_{suffix}")
            }
        })
        .collect::<Vec<_>>();
    let mut used = [false; 65];
    // Inline font/descriptor dictionaries are not necessarily separate objects.
    // Scan direct containers once; references are already in pdf.objects. Only
    // the65 candidate labels are retained, avoiding copies of large PDF names.
    if pdf.objects.len() > 250_000 {
        return Err(bad());
    }
    let mut stack = pdf
        .objects
        .values()
        .map(|v| (v, 0usize))
        .collect::<Vec<_>>();
    let mut visited = 0usize;
    while let Some((object, depth)) = stack.pop() {
        visited += 1;
        if visited > 250_000 || depth > 32 {
            return Err(bad());
        }
        let dictionary = match object {
            Object::Dictionary(d) => d,
            Object::Stream(s) => &s.dict,
            Object::Array(values) => {
                stack.extend(values.iter().map(|v| (v, depth + 1)));
                continue;
            }
            _ => continue,
        };
        for key in [b"BaseFont".as_slice(), b"FontName"] {
            if let Some(value) = dictionary
                .get(key)
                .ok()
                .and_then(|v| pdf.dereference(v).ok())
                .and_then(|(_, v)| v.as_name().ok())
            {
                if value.starts_with(base.as_bytes()) {
                    for (index, candidate) in candidates.iter().enumerate() {
                        if value == candidate.as_bytes() {
                            used[index] = true;
                        }
                    }
                }
            }
        }
        stack.extend(dictionary.iter().map(|(_, v)| (v, depth + 1)));
    }
    candidates
        .into_iter()
        .zip(used)
        .find_map(|(name, used)| (!used).then_some(name))
        .ok_or_else(bad)
}

pub(super) fn prepare(
    pdf: &Document,
    resource: &Object,
    run: &RunSnapshot,
    source: &[u8],
    encoded: &[u8],
    replacement: &str,
    asset: Option<(&FontAsset, AssetWidths)>,
) -> EngineResult<Option<Plan>> {
    let Some(proof) = &run.proof else {
        return Ok(None);
    };
    let font = dict(pdf, resource)?;
    // Retain the separate simple/CFF/vertical policies; only this already
    // proved horizontal CID TrueType path supplies physical cursor atoms.
    if font.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Type0")
        || font
            .get(b"Encoding")
            .ok()
            .and_then(|v| pdf.dereference(v).ok())
            .and_then(|(_, v)| v.as_name().ok())
            != Some(b"Identity-H")
        || proof.single_byte()
    {
        return Ok(None);
    }
    let descendant = child(pdf, &font)?;
    if descendant.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"CIDFontType2") {
        return Ok(None);
    }
    if !embedded_font::matches_name(pdf, &font, &run.font, true) {
        return Err(bad());
    }
    let program = embedded_font::program(pdf, &font)?;
    if run.program.as_deref() != Some(embedded_font::key(&program).as_str()) {
        return Err(bad());
    }
    let face = static_face(&program)?;
    let old = proof.cursor_glyphs(source)?;
    let before = old.iter().map(|(_, _, w)| *w).collect::<Vec<_>>();
    let used = old.iter().map(|(c, _, _)| *c).collect::<BTreeSet<_>>();
    let (after, target_program) = if let Some((asset, policy)) = asset {
        if encoded
            != replacement
                .encode_utf16()
                .flat_map(u16::to_be_bytes)
                .collect::<Vec<_>>()
        {
            return Err(bad());
        }
        let face = static_face(&asset.bytes)?;
        let glyphs = replacement
            .chars()
            .map(|ch| {
                let code = u16::try_from(ch as u32).map_err(|_| bad())?;
                let gid = asset.glyph_id(ch)?;
                let advance = face
                    .glyph_hor_advance(ttf_parser::GlyphId(gid))
                    .ok_or_else(bad)?;
                let width = persistence::imported_font_glyph_width(
                    advance,
                    face.units_per_em(),
                    matches!(policy, AssetWidths::Integral),
                );
                Ok(cursor_widths::Glyph {
                    code,
                    width,
                    nominal: nominal(&face, gid)?,
                    adjustable: true,
                })
            })
            .collect::<EngineResult<Vec<_>>>()?;
        (glyphs, asset.info.id.clone())
    } else {
        let glyphs = proof
            .cursor_glyphs(encoded)?
            .into_iter()
            .map(|(code, gid, width)| {
                Ok(cursor_widths::Glyph {
                    code,
                    width,
                    nominal: nominal(&face, gid)?,
                    adjustable: !used.contains(&code),
                })
            })
            .collect::<EngineResult<Vec<_>>>()?;
        (glyphs, embedded_font::key(&program))
    };
    let mut repair_budget = cursor_widths::REPAIR_BUDGET;
    let plan = match cursor_widths::plan_with_budget(&before, &after, &mut repair_budget) {
        Ok(plan) => plan,
        Err(_) if asset.is_none() => {
            // These widths are installed only in a private, uniquely named
            // resource selected/restored around this show. Original widths,
            // mappings and font programs remain in every shared resource.
            // Prefer the old new-code-only plan; subset deletions can require
            // a bounded change to a retained code in the private clone too.
            let mut isolated = after.clone();
            for glyph in &mut isolated {
                glyph.adjustable = true;
            }
            cursor_widths::plan_with_budget(&before, &isolated, &mut repair_budget)
                .map_err(invalid)?
        }
        Err(error) => return Err(invalid(error)),
    };
    let name = if plan.changes.is_empty() {
        None
    } else {
        Some(name(pdf, &target_program, &plan.changes)?)
    };
    Ok(Some(Plan {
        width_delta: plan.width_delta,
        name,
        changes: plan.changes,
        program: target_program,
    }))
}

impl Plan {
    pub(super) fn install(&self, pdf: &mut Document, resource: &Object) -> EngineResult<Object> {
        let Some(name) = &self.name else {
            return Ok(resource.clone());
        };
        let mut font = dict(pdf, resource)?;
        if embedded_font::key(&embedded_font::program(pdf, &font)?) != self.program {
            return Err(bad());
        }
        let mut child = child(pdf, &font)?;
        if child.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"CIDFontType2") {
            return Err(bad());
        }
        let mut widths = embedded_font::widths(pdf, &child, true)?;
        for (code, width) in &self.changes {
            widths.insert(*code, *width);
        }
        if widths.len() > 65536 {
            return Err(bad());
        }
        // Compact consecutive entries without dropping any original code or
        // duplicating overrides. Original DW/mapping/program remain unchanged.
        let mut encoded = Vec::new();
        let mut start = None;
        let mut last = 0u16;
        let mut values = Vec::new();
        for (code, w) in widths {
            if start.is_some() && last.checked_add(1) != Some(code) {
                encoded.push(Object::Integer(i64::from(start.take().unwrap())));
                encoded.push(Object::Array(std::mem::take(&mut values)));
            }
            if start.is_none() {
                start = Some(code);
            }
            last = code;
            values.push(Object::Real(w));
        }
        if let Some(start) = start {
            encoded.push(Object::Integer(i64::from(start)));
            encoded.push(Object::Array(values));
        }
        child.set("W", encoded);
        child.set("BaseFont", Object::Name(name.as_bytes().to_vec()));
        let mut descriptor = dict(pdf, child.get(b"FontDescriptor").map_err(|_| bad())?)?;
        descriptor.set("FontName", Object::Name(name.as_bytes().to_vec()));
        child.set("FontDescriptor", pdf.add_object(descriptor));
        font.set(
            "DescendantFonts",
            vec![Object::Reference(pdf.add_object(child))],
        );
        font.set("BaseFont", Object::Name(name.as_bytes().to_vec()));
        Ok(Object::Reference(pdf.add_object(font)))
    }
}

#[cfg(test)]
#[path = "cursor_font_tests.rs"]
mod tests;
