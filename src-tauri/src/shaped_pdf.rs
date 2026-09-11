//! Experimental serialization gate: actual glyph positions and logical Unicode.
//! The editor does not yet consume this standalone representation.
use crate::{shape_text, EngineError, EngineResult, FontAsset, ShapedText, TextDirection};
use lopdf::{
    content::{Content, Operation},
    dictionary, Document, Object, Stream, StringFormat,
};

const MAX_GLYPHS: usize = 16_384;
const MAX_CMAP_BYTES: usize = 2 * 1024 * 1024;
const MAX_PAGE_POINTS: f32 = 14_400.0;

pub struct ShapedPdf {
    pub layout: ShapedText,
    pub bytes: Vec<u8>,
}

fn invalid(message: &str) -> EngineError {
    EngineError::InvalidRequest(message.into())
}
fn utf16_hex(text: &str) -> String {
    text.encode_utf16()
        .map(|unit| format!("{unit:04X}"))
        .collect()
}
fn numbers(values: &[f32]) -> Object {
    Object::Array(values.iter().map(|value| Object::Real(*value)).collect())
}

/// Produce a self-contained single-line evidence PDF. Input layout is always
/// recomputed from validated immutable bytes; callers cannot inject glyph IDs,
/// positions, cluster ranges or font dictionaries through serialized metadata.
pub fn create_shaped_pdf(
    font: &FontAsset,
    text: &str,
    font_size: f32,
    direction: TextDirection,
    ligatures: bool,
    rotation: u16,
) -> EngineResult<ShapedPdf> {
    if !matches!(rotation, 0 | 90 | 180 | 270) {
        return Err(invalid("Use a quarter-turn page rotation."));
    }
    let layout = shape_text(font, text, font_size, direction, ligatures)?;
    let glyphs: Vec<_> = layout.runs.iter().flat_map(|run| &run.glyphs).collect();
    if glyphs.is_empty() || glyphs.len() > MAX_GLYPHS {
        return Err(invalid("The shaped PDF exceeds its glyph limit."));
    }
    let face = font.face()?;
    let scale = 1000.0 / f32::from(face.units_per_em());
    let bbox = face.global_bounding_box();
    let min_x = layout.bounds.map_or(0.0, |b| b.x_min.min(0.0));
    let min_y = layout.bounds.map_or(0.0, |b| b.y_min.min(0.0));
    let max_x = layout
        .bounds
        .map_or(layout.width, |b| b.x_max.max(layout.width));
    let max_y = layout.bounds.map_or(font_size, |b| b.y_max.max(font_size));
    let width = (max_x - min_x + 96.0).max(300.0);
    let height = (max_y - min_y + 96.0).max(160.0);
    if !width.is_finite()
        || !height.is_finite()
        || width > MAX_PAGE_POINTS
        || height > MAX_PAGE_POINTS
    {
        return Err(invalid("The shaped line exceeds the PDF page-size limit."));
    }
    let origin = (48.0 - min_x, 48.0 - min_y);
    let mut cid_map = vec![0u8; (glyphs.len() + 1) * 2];
    let mut widths = Vec::with_capacity(glyphs.len());
    let mut mappings = Vec::with_capacity(glyphs.len());
    let mut cmap_size = 0usize;
    for (index, glyph) in glyphs.iter().enumerate() {
        let cid = index + 1;
        cid_map[cid * 2..cid * 2 + 2].copy_from_slice(&glyph.glyph_id.to_be_bytes());
        let advance = face
            .glyph_hor_advance(ttf_parser::GlyphId(glyph.glyph_id))
            .ok_or_else(|| invalid("A shaped glyph has no horizontal metric."))?;
        widths.push(Object::Real(f32::from(advance) * scale));
        let cluster = text
            .get(glyph.cluster.utf8_start..glyph.cluster.utf8_end)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| invalid("A shaped cluster has invalid Unicode bounds."))?;
        let mapping = format!("<{cid:04X}> <{}>\n", utf16_hex(cluster));
        cmap_size = cmap_size
            .checked_add(mapping.len())
            .filter(|size| *size <= MAX_CMAP_BYTES)
            .ok_or_else(|| invalid("The shaped Unicode map exceeds its size limit."))?;
        mappings.push(mapping);
    }
    let mut cmap=String::from("/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /FolioShapedUnicode def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n");
    for chunk in mappings.chunks(100) {
        cmap.push_str(&format!("{} beginbfchar\n", chunk.len()));
        for item in chunk {
            cmap.push_str(item);
        }
        cmap.push_str("endbfchar\n");
    }
    cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    let mut pdf = Document::with_version("1.7");
    let program = pdf.add_object(Stream::new(
        dictionary! {"Length1"=>font.bytes.len() as i64},
        font.bytes.to_vec(),
    ));
    let name = format!("FolioShaped{}", font.info.id);
    let descriptor=pdf.add_object(dictionary!{
        "Type"=>"FontDescriptor","FontName"=>Object::Name(name.as_bytes().to_vec()),
        "Flags"=>32+if face.is_monospaced(){1}else{0}+if font.info.italic{64}else{0},
        "FontBBox"=>numbers(&[f32::from(bbox.x_min)*scale,f32::from(bbox.y_min)*scale,f32::from(bbox.x_max)*scale,f32::from(bbox.y_max)*scale]),
        "ItalicAngle"=>face.italic_angle(),"Ascent"=>f32::from(face.ascender())*scale,
        "Descent"=>f32::from(face.descender())*scale,"CapHeight"=>f32::from(face.capital_height().unwrap_or(face.ascender()))*scale,
        "StemV"=>80,"FontFile2"=>program
    });
    let cid_map = pdf.add_object(Stream::new(dictionary! {}, cid_map));
    let descendant=pdf.add_object(dictionary!{
        "Type"=>"Font","Subtype"=>"CIDFontType2","BaseFont"=>Object::Name(name.as_bytes().to_vec()),
        "CIDSystemInfo"=>dictionary!{"Registry"=>Object::string_literal("Adobe"),"Ordering"=>Object::string_literal("Identity"),"Supplement"=>0},
        "FontDescriptor"=>descriptor,"DW"=>0,"W"=>vec![Object::Integer(1),Object::Array(widths)],"CIDToGIDMap"=>cid_map
    });
    let unicode = pdf.add_object(Stream::new(dictionary! {}, cmap.into_bytes()));
    let resource=pdf.add_object(dictionary!{
        "Type"=>"Font","Subtype"=>"Type0","BaseFont"=>Object::Name(name.into_bytes()),"Encoding"=>"Identity-H",
        "DescendantFonts"=>vec![Object::Reference(descendant)],"ToUnicode"=>unicode
    });
    let actual: Vec<u8> = std::iter::once(0xfeffu16)
        .chain(text.encode_utf16())
        .flat_map(u16::to_be_bytes)
        .collect();
    let mut operations = vec![
        Operation::new("q", vec![]),
        Operation::new(
            "BDC",
            vec![
                Object::Name(b"Span".to_vec()),
                Object::Dictionary(
                    dictionary! {"ActualText"=>Object::String(actual,StringFormat::Hexadecimal)},
                ),
            ],
        ),
        Operation::new("BT", vec![]),
        Operation::new(
            "Tf",
            vec![Object::Name(b"FolioShaped".to_vec()), font_size.into()],
        ),
        Operation::new("rg", vec![0.1.into(), 0.2.into(), 0.3.into()]),
    ];
    for (index, glyph) in glyphs.iter().enumerate() {
        let x = origin.0 + glyph.x;
        let y = origin.1 + glyph.y;
        if !x.is_finite() || !y.is_finite() {
            return Err(invalid("A shaped glyph has a non-finite position."));
        }
        operations.push(Operation::new(
            "Tm",
            vec![1.into(), 0.into(), 0.into(), 1.into(), x.into(), y.into()],
        ));
        operations.push(Operation::new(
            "Tj",
            vec![Object::String(
                ((index + 1) as u16).to_be_bytes().to_vec(),
                StringFormat::Hexadecimal,
            )],
        ));
    }
    operations.extend([
        Operation::new("ET", vec![]),
        Operation::new("EMC", vec![]),
        Operation::new("Q", vec![]),
    ]);
    let content = Content { operations }
        .encode()
        .map_err(|_| invalid("Shaped PDF content could not be encoded."))?;
    let stream = pdf.add_object(Stream::new(dictionary! {}, content));
    let pages = pdf.new_object_id();
    let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>numbers(&[0.0,0.0,width,height]),"Rotate"=>rotation as i64,"Resources"=>dictionary!{"Font"=>dictionary!{"FolioShaped"=>resource}},"Contents"=>stream});
    pdf.objects.insert(
        pages,
        Object::Dictionary(
            dictionary! {"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1},
        ),
    );
    let catalog = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    pdf.trailer.set("Root", catalog);
    let mut bytes = Vec::new();
    pdf.save_to(&mut bytes)
        .map_err(|_| invalid("Shaped PDF could not be saved."))?;
    Ok(ShapedPdf { layout, bytes })
}
