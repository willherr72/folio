//! One deterministic invisible CID font and text sequence per OCR line.
use crate::ocr_pdf::OcrMapping;
use crate::{
    ocr_types::invalid,
    semantic_font::{build_ocr_semantic_font, SemanticGlyph},
    EngineResult, OcrPage,
};
use lopdf::{dictionary, Document, Object, ObjectId, Stream};
use std::{collections::BTreeMap, fmt::Write};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Definition {
    scalar: char,
    advance: u16,
    bottom: i16,
    top: i16,
}
fn numbers(values: &[f32]) -> Object {
    Object::Array(values.iter().copied().map(Object::Real).collect())
}

pub(crate) fn build(
    document: &mut Document,
    page: &OcrPage,
    mapping: &OcrMapping,
) -> EngineResult<(ObjectId, Vec<u8>)> {
    let mut definitions = Vec::new();
    let mut indices = BTreeMap::new();
    let mut content = String::from("q\n");
    let mut code = |definition: Definition| -> EngineResult<u16> {
        if let Some(value) = indices.get(&definition) {
            return Ok(*value);
        }
        if definitions.len() >= 65_534 {
            return Err(invalid("OCR font exceeds 65534 distinct glyph definitions"));
        }
        definitions.push(definition);
        let value = definitions.len() as u16;
        indices.insert(definition, value);
        Ok(value)
    };
    for line in page.lines().filter(|line| !line.words.is_empty()) {
        let top = line
            .words
            .iter()
            .map(|w| w.bounds.y)
            .fold(f32::INFINITY, f32::min);
        let baseline = line
            .words
            .iter()
            .map(|w| w.bounds.y + w.bounds.height)
            .fold(0., f32::max);
        let scale = line
            .words
            .iter()
            .map(|w| w.bounds.width / w.text.chars().count() as f32)
            .fold(baseline - top, f32::max);
        let origin = mapping.point(line.words[0].bounds.x, baseline);
        writeln!(
            &mut content,
            "BT 0 Tc 0 Tw 100 Tz 0 Ts 3 Tr /F0 {:.7} Tf {:.9} {:.9} {:.9} {:.9} {:.7} {:.7} Tm [",
            scale,
            mapping.right[0],
            mapping.right[1],
            -mapping.down[0],
            -mapping.down[1],
            origin[0],
            origin[1]
        )
        .unwrap();
        let mut cursor = line.words[0].bounds.x;
        for (index, word) in line.words.iter().enumerate() {
            if index > 0 {
                let space = code(Definition {
                    scalar: ' ',
                    advance: 0,
                    bottom: 0,
                    top: 0,
                })?;
                write!(
                    &mut content,
                    "<{space:04X}> {:.7} ",
                    (cursor - word.bounds.x) / scale * 1000.
                )
                .unwrap();
            }
            let width = word.bounds.width / word.text.chars().count() as f32;
            let advance = (width / scale * 1000.).round().clamp(1., 1000.) as u16;
            let bottom =
                ((baseline - word.bounds.y - word.bounds.height) / scale * 1000.).round() as i16;
            let top = ((baseline - word.bounds.y) / scale * 1000.)
                .round()
                .max(f32::from(bottom) + 1.) as i16;
            for scalar in word.text.chars() {
                let id = code(Definition {
                    scalar,
                    advance,
                    bottom,
                    top,
                })?;
                write!(
                    &mut content,
                    "<{id:04X}> {:.7} ",
                    f32::from(advance) - width / scale * 1000.
                )
                .unwrap();
            }
            cursor = word.bounds.x + word.bounds.width;
        }
        content.push_str("] TJ ET\n");
        if content.len() > 8 * 1024 * 1024 {
            return Err(invalid("OCR content exceeds 8 MiB"));
        }
    }
    content.push_str("Q\n");
    let glyphs: Vec<_> = definitions
        .iter()
        .map(|d| SemanticGlyph {
            advance: f32::from(d.advance),
            bounds: [
                0.,
                f32::from(d.bottom),
                f32::from(d.advance),
                f32::from(d.top),
            ],
        })
        .collect();
    let bytes = build_ocr_semantic_font(&glyphs)?;
    let program = document.add_object(Stream::new(
        dictionary! {"Length1"=>bytes.len() as i64},
        bytes,
    ));
    let right = definitions.iter().map(|d| d.advance).max().unwrap_or(0) as f32;
    let top = definitions.iter().map(|d| d.top).max().unwrap_or(0) as f32;
    let descriptor=document.add_object(dictionary!{"Type"=>"FontDescriptor","FontName"=>"FolioOCRSemantic","Flags"=>4,
        "FontBBox"=>numbers(&[0.,0.,right,top]),"ItalicAngle"=>0,"Ascent"=>top,"Descent"=>0,"CapHeight"=>top,"StemV"=>80,"FontFile2"=>program});
    let cid_map = document.add_object(Stream::new(
        dictionary! {},
        (0..=definitions.len())
            .flat_map(|n| (n as u16).to_be_bytes())
            .collect(),
    ));
    let widths: Vec<_> = definitions
        .iter()
        .map(|d| Object::Integer(i64::from(d.advance)))
        .collect();
    let descendant=document.add_object(dictionary!{"Type"=>"Font","Subtype"=>"CIDFontType2","BaseFont"=>"FolioOCRSemantic",
        "CIDSystemInfo"=>dictionary!{"Registry"=>Object::string_literal("Adobe"),"Ordering"=>Object::string_literal("Identity"),"Supplement"=>0},
        "FontDescriptor"=>descriptor,"CIDToGIDMap"=>cid_map,"DW"=>0,"W"=>vec![Object::Integer(1),Object::Array(widths)]});
    let mut unicode=String::from("/CIDInit /ProcSet findresource begin 12 dict begin begincmap /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def /CMapName /FolioOCR def /CMapType 2 def 1 begincodespacerange <0000> <FFFF> endcodespacerange\n");
    for (chunk_index, chunk) in definitions.chunks(100).enumerate() {
        writeln!(&mut unicode, "{} beginbfchar", chunk.len()).unwrap();
        for (i, definition) in chunk.iter().enumerate() {
            let mut utf16 = [0; 2];
            let value: String = definition
                .scalar
                .encode_utf16(&mut utf16)
                .iter()
                .map(|u| format!("{u:04X}"))
                .collect();
            writeln!(
                &mut unicode,
                "<{:04X}> <{value}>",
                chunk_index * 100 + i + 1
            )
            .unwrap();
        }
        unicode.push_str("endbfchar\n");
    }
    unicode.push_str("endcmap CMapName currentdict /CMap defineresource pop end end\n");
    let unicode = document.add_object(Stream::new(dictionary! {}, unicode.into_bytes()));
    let font=document.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type0","BaseFont"=>"FolioOCRSemantic","Encoding"=>"Identity-H",
        "DescendantFonts"=>vec![Object::Reference(descendant)],"ToUnicode"=>unicode});
    Ok((font, content.into_bytes()))
}
