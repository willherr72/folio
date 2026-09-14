//! Deterministic, bounded rectangle TrueType font for invisible CID semantics.
//! This font never supplies the visible original-font outlines.
use crate::{EngineError, EngineResult};

#[derive(Clone, Copy)]
pub(crate) struct SemanticGlyph {
    pub advance: f32,
    pub bounds: [f32; 4],
}

/// Input metrics use 1000 units/em. The PDF caller compensates magnification
/// with fontSize / magnification; the font itself always retains UPM 1000.
pub(crate) fn build_semantic_font(
    glyphs: &[SemanticGlyph],
    magnification: u16,
) -> EngineResult<Vec<u8>> {
    if glyphs.is_empty() || glyphs.len() > 4096 {
        return Err(invalid("requires 1 to 4096 glyph definitions"));
    }
    if !(1..=32).contains(&magnification) {
        return Err(invalid("magnification must be between 1 and 32"));
    }
    let mut metrics = Vec::with_capacity(glyphs.len() + 1);
    metrics.push(Metric {
        advance: 0,
        bounds: [0; 4],
        ink: false,
    }); // .notdef
    for glyph in glyphs {
        if !glyph.advance.is_finite() || glyph.advance < 0. {
            return Err(invalid("advance must be finite and nonnegative"));
        }
        let advance = (f64::from(glyph.advance) * f64::from(magnification)).round();
        if advance > f64::from(u16::MAX) {
            return Err(invalid(
                "scaled advance exceeds unsigned 16-bit font metrics",
            ));
        }
        let mut bounds = [0i16; 4];
        for (output, value) in bounds.iter_mut().zip(glyph.bounds) {
            let scaled = (f64::from(value) * f64::from(magnification)).round();
            if !scaled.is_finite() || scaled < f64::from(i16::MIN) || scaled > f64::from(i16::MAX) {
                return Err(invalid(
                    "scaled rectangle coordinate exceeds signed 16-bit font metrics",
                ));
            }
            *output = scaled as i16;
        }
        if glyph.bounds[0] > glyph.bounds[2] || glyph.bounds[1] > glyph.bounds[3] {
            return Err(invalid("rectangle bounds are inverted"));
        }
        let ink = glyph.bounds[0] < glyph.bounds[2] && glyph.bounds[1] < glyph.bounds[3];
        let width = i32::from(bounds[2]) - i32::from(bounds[0]);
        let height = i32::from(bounds[3]) - i32::from(bounds[1]);
        if ink && (width == 0 || height == 0) {
            return Err(invalid(
                "positive rectangle collapses at the selected magnification",
            ));
        }
        // Four-point simple glyphs encode signed coordinate deltas, not just bounds.
        if ink && (width > i32::from(i16::MAX) || height > i32::from(i16::MAX)) {
            return Err(invalid(
                "rectangle extent exceeds signed 16-bit glyph deltas",
            ));
        }
        metrics.push(Metric {
            advance: advance as u16,
            bounds,
            ink,
        });
    }
    let ink: Vec<_> = metrics.iter().filter(|m| m.ink).collect();
    let bbox = [
        ink.iter().map(|m| m.bounds[0]).min().unwrap_or(0),
        ink.iter().map(|m| m.bounds[1]).min().unwrap_or(0),
        ink.iter().map(|m| m.bounds[2]).max().unwrap_or(0),
        ink.iter().map(|m| m.bounds[3]).max().unwrap_or(0),
    ];
    let ascender = (1000 * i32::from(magnification)).max(i32::from(bbox[3])) as i16;
    let descender = (-500 * i32::from(magnification)).min(i32::from(bbox[1])) as i16;
    let min_rsb = ink
        .iter()
        .map(|m| i32::from(m.advance) - i32::from(m.bounds[2]))
        .min()
        .unwrap_or(0);
    let min_rsb = i16::try_from(min_rsb)
        .map_err(|_| invalid("right side bearing exceeds signed 16-bit header metrics"))?;
    let nonzero: Vec<_> = metrics.iter().filter(|m| m.advance != 0).collect();
    let count = nonzero.len().max(1) as u32;
    let average = (nonzero.iter().map(|m| u32::from(m.advance)).sum::<u32>() + count / 2) / count;
    let average = i16::try_from(average)
        .map_err(|_| invalid("average advance exceeds signed 16-bit OS/2 metrics"))?;

    let mut glyf = Vec::with_capacity(glyphs.len() * 36);
    let mut loca = Vec::with_capacity((metrics.len() + 1) * 4);
    let mut hmtx = Vec::with_capacity(metrics.len() * 4);
    for metric in &metrics {
        push_u32(&mut loca, glyf.len() as u32);
        push_u16(&mut hmtx, metric.advance);
        push_i16(&mut hmtx, if metric.ink { metric.bounds[0] } else { 0 });
        if !metric.ink {
            continue;
        }
        let [x0, y0, x1, y1] = metric.bounds;
        push_i16(&mut glyf, 1); // one clockwise contour, no curves or instructions
        for value in metric.bounds {
            push_i16(&mut glyf, value);
        }
        push_u16(&mut glyf, 3); // last point index
        push_u16(&mut glyf, 0); // instruction length
        glyf.extend_from_slice(&[1; 4]); // every point is on-curve, signed 16-bit deltas
        for value in [
            i32::from(x0),
            0,
            i32::from(x1) - i32::from(x0),
            0,
            i32::from(y0),
            i32::from(y1) - i32::from(y0),
            0,
            i32::from(y0) - i32::from(y1),
        ] {
            push_i16(&mut glyf, value as i16);
        }
        pad(&mut glyf);
    }
    push_u32(&mut loca, glyf.len() as u32);

    // OpenType head/hhea/maxp; reserved fields and timestamps remain zero.
    let mut head = vec![0; 54];
    put_u32(&mut head, 0, 0x0001_0000);
    put_u32(&mut head, 4, 0x0001_0000);
    put_u32(&mut head, 12, 0x5F0F_3CF5);
    put_u16(&mut head, 16, 3); // baseline at zero; lsb == xMin
    put_u16(&mut head, 18, 1000);
    for (index, value) in bbox.iter().enumerate() {
        put_i16(&mut head, 36 + index * 2, *value);
    }
    put_u16(&mut head, 46, 8); // lowest recommended ppem
    put_i16(&mut head, 48, 2); // required deprecated direction hint
    put_i16(&mut head, 50, 1); // long loca offsets
    let mut hhea = vec![0; 36];
    put_u32(&mut hhea, 0, 0x0001_0000);
    put_i16(&mut hhea, 4, ascender);
    put_i16(&mut hhea, 6, descender);
    put_u16(
        &mut hhea,
        10,
        metrics.iter().map(|m| m.advance).max().unwrap_or(0),
    );
    put_i16(&mut hhea, 12, bbox[0]);
    put_i16(&mut hhea, 14, min_rsb);
    put_i16(&mut hhea, 16, bbox[2]);
    put_i16(&mut hhea, 18, 1); // vertical caret slope
    put_u16(&mut hhea, 34, metrics.len() as u16);
    let mut maxp = vec![0; 32];
    put_u32(&mut maxp, 0, 0x0001_0000);
    put_u16(&mut maxp, 4, metrics.len() as u16);
    put_u16(&mut maxp, 6, if ink.is_empty() { 0 } else { 4 });
    put_u16(&mut maxp, 8, if ink.is_empty() { 0 } else { 1 });
    put_u16(&mut maxp, 14, 1); // only glyph zone, no hinting/twilight zone

    // A format-4 private-use cmap supports font readers; PDF ToUnicode owns text.
    let mut cmap = vec![0; 44];
    put_u16(&mut cmap, 2, 1);
    put_u16(&mut cmap, 4, 3); // Windows Unicode BMP
    put_u16(&mut cmap, 6, 1);
    put_u32(&mut cmap, 8, 12);
    for (offset, value) in [
        (12, 4),
        (14, 32),
        (18, 4),
        (20, 4),
        (22, 1),
        (26, 0xE000 + glyphs.len() as u16 - 1),
        (28, 0xFFFF),
        (32, 0xE000),
        (34, 0xFFFF),
        (36, 0x2001),
        (38, 1),
    ] {
        put_u16(&mut cmap, offset, value);
    }
    let mut os2 = vec![0; 78]; // OS/2 version 0; own outlines allow embedding
    put_i16(&mut os2, 2, average);
    put_u16(&mut os2, 4, 400);
    put_u16(&mut os2, 6, 5);
    put_u32(&mut os2, 46, 1 << 28); // Unicode range bit 60: private use area
    os2[58..62].copy_from_slice(b"FOLI");
    put_u16(&mut os2, 62, 0x40); // regular style
    put_u16(&mut os2, 64, 0xE000);
    put_u16(&mut os2, 66, 0xE000 + glyphs.len() as u16 - 1);
    put_i16(&mut os2, 68, ascender);
    put_i16(&mut os2, 70, descender);
    put_u16(&mut os2, 74, ascender as u16);
    put_u16(&mut os2, 76, (-i32::from(descender)) as u16);
    let mut post = vec![0; 32];
    put_u32(&mut post, 0, 0x0003_0000); // no PostScript glyph name array
    let names = [
        (1, "FolioSemanticWide"),
        (2, "Regular"),
        (3, "FolioSemanticWide"),
        (4, "FolioSemanticWide"),
        (5, "Version 1.0"),
        (6, "FolioSemanticWide"),
    ];
    let mut name = vec![0; 6 + names.len() * 12];
    put_u16(&mut name, 2, names.len() as u16);
    let strings = name.len();
    put_u16(&mut name, 4, strings as u16);
    for (index, (id, text)) in names.iter().enumerate() {
        let offset = 6 + index * 12;
        put_u16(&mut name, offset, 3);
        put_u16(&mut name, offset + 2, 1);
        put_u16(&mut name, offset + 4, 0x0409);
        put_u16(&mut name, offset + 6, *id);
        put_u16(&mut name, offset + 8, (text.len() * 2) as u16);
        let relative = (name.len() - strings) as u16;
        put_u16(&mut name, offset + 10, relative);
        for value in text.encode_utf16() {
            push_u16(&mut name, value);
        }
    }
    let tables = vec![
        (*b"OS/2", os2),
        (*b"cmap", cmap),
        (*b"glyf", glyf),
        (*b"head", head),
        (*b"hhea", hhea),
        (*b"hmtx", hmtx),
        (*b"loca", loca),
        (*b"maxp", maxp),
        (*b"name", name),
        (*b"post", post),
    ];
    let length = 12
        + tables.len() * 16
        + tables
            .iter()
            .map(|(_, data)| (data.len() + 3) & !3)
            .sum::<usize>();
    if length > 256 * 1024 {
        return Err(invalid("font exceeds the 256 KiB allocation bound"));
    }
    let mut font = Vec::with_capacity(length);
    font.resize(12 + tables.len() * 16, 0);
    put_u32(&mut font, 0, 0x0001_0000);
    put_u16(&mut font, 4, tables.len() as u16);
    put_u16(&mut font, 6, 128);
    put_u16(&mut font, 8, 3);
    put_u16(&mut font, 10, 32);
    let mut head_offset = 0;
    for (index, (tag, data)) in tables.into_iter().enumerate() {
        let entry = 12 + index * 16;
        let offset = font.len();
        font[entry..entry + 4].copy_from_slice(&tag);
        put_u32(&mut font, entry + 4, checksum(&data));
        put_u32(&mut font, entry + 8, offset as u32);
        put_u32(&mut font, entry + 12, data.len() as u32);
        if &tag == b"head" {
            head_offset = offset;
        }
        font.extend_from_slice(&data);
        pad(&mut font);
    }
    let adjustment = 0xB1B0_AFBAu32.wrapping_sub(checksum(&font));
    put_u32(&mut font, head_offset + 8, adjustment);
    Ok(font)
}

struct Metric {
    advance: u16,
    bounds: [i16; 4],
    ink: bool,
}
fn invalid(message: &str) -> EngineError {
    EngineError::InvalidRequest(format!("Semantic font {message}."))
}
fn push_u16(data: &mut Vec<u8>, value: u16) {
    data.extend_from_slice(&value.to_be_bytes());
}
fn push_i16(data: &mut Vec<u8>, value: i16) {
    data.extend_from_slice(&value.to_be_bytes());
}
fn push_u32(data: &mut Vec<u8>, value: u32) {
    data.extend_from_slice(&value.to_be_bytes());
}
fn put_u16(data: &mut [u8], offset: usize, value: u16) {
    data[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}
fn put_i16(data: &mut [u8], offset: usize, value: i16) {
    data[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}
fn put_u32(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}
fn pad(data: &mut Vec<u8>) {
    data.resize((data.len() + 3) & !3, 0);
}
fn checksum(data: &[u8]) -> u32 {
    data.chunks(4).fold(0u32, |sum, bytes| {
        let mut word = [0; 4];
        word[..bytes.len()].copy_from_slice(bytes);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ttf_parser::{Face, GlyphId, Rect, Tag};

    const SAMPLE: SemanticGlyph = SemanticGlyph {
        advance: 600.25,
        bounds: [-10.125, -200.25, 500.375, 750.5],
    };

    #[test]
    fn round_trips_wide_glyph_ids_metrics_rectangles_and_private_cmap() {
        let glyphs = vec![SAMPLE; 4096];
        let bytes = build_semantic_font(&glyphs, 16).unwrap();
        let face = Face::parse(&bytes, 0).unwrap();
        assert_eq!(face.units_per_em(), 1000);
        assert_eq!(face.number_of_glyphs(), 4097);
        assert_eq!(face.glyph_hor_advance(GlyphId(0)), Some(0));
        assert_eq!(face.glyph_bounding_box(GlyphId(0)), None);
        let expected = Rect {
            x_min: -162,
            y_min: -3204,
            x_max: 8006,
            y_max: 12008,
        };
        for id in 1..=4096 {
            let gid = GlyphId(id);
            assert_eq!(face.glyph_hor_advance(gid), Some(9604));
            assert_eq!(face.glyph_hor_side_bearing(gid), Some(-162));
            assert_eq!(face.glyph_bounding_box(gid), Some(expected));
            let mut outline = RectangleOutline::default();
            assert_eq!(face.outline_glyph(gid, &mut outline), Some(expected));
            assert_eq!(
                outline.points,
                [
                    (-162., -3204.),
                    (-162., 12008.),
                    (8006., 12008.),
                    (8006., -3204.),
                    (-162., -3204.)
                ]
            );
            assert_eq!(outline.closes, 1);
            assert_eq!(
                face.glyph_index(char::from_u32(0xE000 + u32::from(id) - 1).unwrap()),
                Some(gid)
            );
        }
        assert_eq!(face.global_bounding_box(), expected);
        assert_eq!(face.glyph_index('A'), None);
        assert_eq!(face.glyph_index('\u{f000}'), None);
        assert!(bytes.len() < 256 * 1024);
        assert!(face
            .names()
            .into_iter()
            .any(|name| name.name_id == 6
                && name.to_string().as_deref() == Some("FolioSemanticWide")));
    }

    #[test]
    fn writes_deterministic_aligned_tables_and_valid_checksums() {
        let bytes = build_semantic_font(&[SAMPLE], 16).unwrap();
        assert_eq!(bytes, build_semantic_font(&[SAMPLE], 16).unwrap());
        assert_eq!(sum(&bytes), 0xB1B0_AFBA);
        let count = read_u16(&bytes, 4) as usize;
        assert_eq!(count, 10);
        assert_eq!(&bytes[6..12], &[0, 128, 0, 3, 0, 32]);
        let mut previous = [0; 4];
        for entry in bytes[12..12 + count * 16].chunks_exact(16) {
            let tag: [u8; 4] = entry[..4].try_into().unwrap();
            assert!(tag > previous);
            previous = tag;
            let offset = read_u32(entry, 8) as usize;
            let length = read_u32(entry, 12) as usize;
            assert_eq!(offset % 4, 0);
            let mut table = bytes[offset..offset + length].to_vec();
            if &tag == b"head" {
                table[8..12].fill(0);
            }
            assert_eq!(sum(&table), read_u32(entry, 4), "table {:?}", tag);
        }
        let face = Face::parse(&bytes, 0).unwrap();
        for tag in [
            b"OS/2", b"cmap", b"glyf", b"head", b"hhea", b"hmtx", b"loca", b"maxp", b"name",
            b"post",
        ] {
            assert!(face.raw_face().table(Tag::from_bytes(tag)).is_some());
        }
        let os2 = face.raw_face().table(Tag::from_bytes(b"OS/2")).unwrap();
        assert_eq!(read_u16(os2, 8), 0, "our own outlines permit embedding");
        let head = face.raw_face().table(Tag::from_bytes(b"head")).unwrap();
        assert_eq!(&head[20..36], &[0; 16], "timestamps are deterministic");
    }

    #[test]
    fn preserves_empty_glyph_advance_and_rounds_once_at_the_requested_scale() {
        let glyphs = [
            SemanticGlyph {
                advance: 250.,
                bounds: [0.; 4],
            },
            SemanticGlyph {
                advance: 12.03125,
                bounds: [-0.03125, 0., 2.03125, 3.],
            },
        ];
        let bytes = build_semantic_font(&glyphs, 16).unwrap();
        let face = Face::parse(&bytes, 0).unwrap();
        assert_eq!(face.glyph_hor_advance(GlyphId(1)), Some(4000));
        assert_eq!(face.glyph_bounding_box(GlyphId(1)), None);
        assert_eq!(face.glyph_hor_advance(GlyphId(2)), Some(193));
        assert_eq!(
            face.glyph_bounding_box(GlyphId(2)),
            Some(Rect {
                x_min: -1,
                y_min: 0,
                x_max: 33,
                y_max: 48
            })
        );
    }

    #[test]
    fn rejects_unbounded_counts_scales_and_nonrepresentable_metrics() {
        assert!(build_semantic_font(&[], 16).is_err());
        assert!(build_semantic_font(&vec![SAMPLE; 4097], 16).is_err());
        assert!(build_semantic_font(&[SAMPLE], 0).is_err());
        assert!(build_semantic_font(&[SAMPLE], 33).is_err());
        for advance in [f32::NAN, f32::INFINITY, -1., 4096.] {
            assert!(build_semantic_font(&[SemanticGlyph { advance, ..SAMPLE }], 16).is_err());
        }
        for coordinate in [f32::NAN, f32::NEG_INFINITY, 2048., -2048.0625] {
            for index in 0..4 {
                let mut glyph = SAMPLE;
                glyph.bounds[index] = coordinate;
                assert!(build_semantic_font(&[glyph], 16).is_err());
            }
        }
        for bounds in [[2., 0., 1., 1.], [0., 2., 1., 1.], [-2000., 0., 2000., 1.]] {
            assert!(build_semantic_font(
                &[SemanticGlyph {
                    advance: 0.,
                    bounds
                }],
                16
            )
            .is_err());
        }
        assert!(
            build_semantic_font(
                &[SemanticGlyph {
                    advance: 1.,
                    bounds: [0., 0., 0.001, 1.]
                }],
                16
            )
            .is_err(),
            "do not silently erase a positive rectangle by rounding"
        );
    }

    #[test]
    fn supports_signed_coordinate_edges_without_delta_overflow() {
        let glyph = SemanticGlyph {
            advance: 0.,
            bounds: [-32768., -32768., -1., -1.],
        };
        let bytes = build_semantic_font(&[glyph], 1).unwrap();
        let face = Face::parse(&bytes, 0).unwrap();
        assert_eq!(
            face.glyph_bounding_box(GlyphId(1)),
            Some(Rect {
                x_min: -32768,
                y_min: -32768,
                x_max: -1,
                y_max: -1
            })
        );
    }

    fn read_u16(bytes: &[u8], offset: usize) -> u16 {
        u16::from_be_bytes(bytes[offset..offset + 2].try_into().unwrap())
    }
    fn read_u32(bytes: &[u8], offset: usize) -> u32 {
        u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap())
    }
    fn sum(bytes: &[u8]) -> u32 {
        bytes.chunks(4).fold(0u32, |total, chunk| {
            let mut word = [0; 4];
            word[..chunk.len()].copy_from_slice(chunk);
            total.wrapping_add(u32::from_be_bytes(word))
        })
    }
    #[derive(Default)]
    struct RectangleOutline {
        points: Vec<(f32, f32)>,
        closes: usize,
    }
    impl ttf_parser::OutlineBuilder for RectangleOutline {
        fn move_to(&mut self, x: f32, y: f32) {
            self.points.push((x, y));
        }
        fn line_to(&mut self, x: f32, y: f32) {
            self.points.push((x, y));
        }
        fn quad_to(&mut self, _: f32, _: f32, _: f32, _: f32) {
            panic!("synthetic rectangle must not contain curves");
        }
        fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32) {
            panic!("synthetic rectangle must not contain curves");
        }
        fn close(&mut self) {
            self.closes += 1;
        }
    }
}
