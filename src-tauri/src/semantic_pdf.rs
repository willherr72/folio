//! Experimental exact outlines plus a scalar Unicode layer with cluster boxes.
//! Scalar origins divide cluster advances equally; they are not caret positions.
//! Mixed directional runs and more than 255 distinct definitions are refused.
//! The explicitly named font-bank probe retains known reader failures only.
//! RTL separators and joiners await a proven reader-order strategy.
use crate::{
    shape_text, shaping::OutlineBudget, EngineError, EngineResult, FontAsset, PositionedGlyph,
    ShapedPdf, TextDirection,
};
use lopdf::{dictionary, Document, Object, Stream};
use std::{collections::BTreeMap, fmt::Write, io};
use ttf_parser::{GlyphId, OutlineBuilder};
use unicode_bidi::{bidi_class, BidiClass};

const CODES_PER_FONT: usize = 255;
const MAX_OUTLINE_OPS: usize = 100_000;
const MAX_CONTENT_BYTES: usize = 8 * 1024 * 1024;
const MAX_PDF_BYTES: usize = 32 * 1024 * 1024;
fn invalid(message: &str) -> EngineError {
    EngineError::InvalidRequest(message.into())
}
fn numbers(values: &[f32]) -> Object {
    Object::Array(values.iter().copied().map(Object::Real).collect())
}

#[derive(Default)]
struct Pen {
    content: String,
    position: (f32, f32),
    start: (f32, f32),
    ops: usize,
    invalid: bool,
}
impl Pen {
    fn emit(&mut self, values: &[f32], operator: &str) {
        self.ops += 1;
        if self.invalid
            || self.ops > MAX_OUTLINE_OPS
            || values
                .iter()
                .any(|n| !n.is_finite() || n.abs() > 1_000_000.)
        {
            self.invalid = true;
            return;
        }
        for value in values {
            write!(&mut self.content, "{value:.7} ").unwrap();
        }
        writeln!(&mut self.content, "{operator}").unwrap();
        if self.content.len() > MAX_CONTENT_BYTES {
            self.invalid = true;
        }
    }
}
impl OutlineBuilder for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.emit(&[x, y], "m");
        self.position = (x, y);
        self.start = (x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.emit(&[x, y], "l");
        self.position = (x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (px, py) = self.position;
        self.emit(
            &[
                px + 2. * (x1 - px) / 3.,
                py + 2. * (y1 - py) / 3.,
                x + 2. * (x1 - x) / 3.,
                y + 2. * (y1 - y) / 3.,
                x,
                y,
            ],
            "c",
        );
        self.position = (x, y);
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.emit(&[x1, y1, x2, y2, x, y], "c");
        self.position = (x, y);
    }
    fn close(&mut self) {
        self.emit(&[], "h");
        self.position = self.start;
    }
}

struct BoundedPdf(Vec<u8>);
impl io::Write for BoundedPdf {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_PDF_BYTES.saturating_sub(self.0.len()) {
            return Err(io::Error::other("Semantic PDF byte limit exceeded."));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
struct Cell {
    unicode: char,
    x: f32,
    width: f32,
    bounds: [f32; 4],
}

/// Research output only: unpainted Type3 character programs carry Unicode and
/// full cluster boxes; native glyph outlines carry all visible ink. The original
/// complete font remains an unused page resource, without editable metadata.
pub fn create_semantic_pdf(
    font: &FontAsset,
    text: &str,
    font_size: f32,
    direction: TextDirection,
    ligatures: bool,
    rotation: u16,
) -> EngineResult<ShapedPdf> {
    semantic_pdf(
        font, text, font_size, direction, ligatures, rotation, false, false,
    )
}

/// Negative research control, NOT a supported writer. Font switches corrupt
/// PDFium order on rotated pages; whole-line ActualText collapses selection.
/// Kept solely to reproduce those failures under the nondefault shaped-text feature.
pub fn create_semantic_font_banks_probe(
    font: &FontAsset,
    text: &str,
    font_size: f32,
    direction: TextDirection,
    ligatures: bool,
    rotation: u16,
    actual_text: bool,
) -> EngineResult<ShapedPdf> {
    semantic_pdf(
        font,
        text,
        font_size,
        direction,
        ligatures,
        rotation,
        true,
        actual_text,
    )
}

fn semantic_pdf(
    font: &FontAsset,
    text: &str,
    font_size: f32,
    direction: TextDirection,
    ligatures: bool,
    rotation: u16,
    font_banks: bool,
    actual_text: bool,
) -> EngineResult<ShapedPdf> {
    if !matches!(rotation, 0 | 90 | 180 | 270) {
        return Err(invalid("Semantic evidence supports quarter turns only."));
    }
    let layout = shape_text(font, text, font_size, direction, ligatures)?;
    if layout
        .runs
        .iter()
        .any(|run| run.direction != layout.runs[0].direction)
    {
        return Err(invalid(
            "Mixed directional runs are not supported by the semantic PDF experiment.",
        ));
    }
    // PDFium can reverse each RTL word while retaining left-to-right word
    // geometry. Repeated identical words hide that corruption in copied text.
    // Neutral separators and joiners need a proven reader-order strategy before
    // this representation can accept them; native shaping still supports them.
    if layout.runs[0].direction == TextDirection::Rtl
        && text
            .chars()
            .any(|c| !matches!(bidi_class(c), BidiClass::R | BidiClass::AL | BidiClass::NSM))
    {
        return Err(invalid(
            "RTL semantic text currently supports strong RTL characters and combining marks only; separators and joiners require reader-order support.",
        ));
    }
    let face = font.face()?;
    let scale = font_size / f32::from(face.units_per_em());
    let b = layout
        .bounds
        .ok_or_else(|| invalid("Semantic evidence requires visible glyph ink."))?;
    let origin = (48. - b.x_min.min(0.), 48. - b.y_min.min(0.));
    let width = (b.x_max.max(layout.width) - b.x_min.min(0.) + 96.).max(300.);
    let height = (b.y_max.max(font_size) - b.y_min.min(0.) + 96.).max(160.);
    if !width.is_finite() || !height.is_finite() || width > 14400. || height > 14400. {
        return Err(invalid("The semantic page exceeds its geometry limit."));
    }
    let mut outline_budget = OutlineBudget::new(&face)?;
    let mut cache = BTreeMap::<u16, Pen>::new();
    let mut clusters = BTreeMap::<(usize, usize), (bool, Vec<&PositionedGlyph>)>::new();
    let mut visible = String::from("q 0.1 0.2 0.3 rg\n");
    let mut ops = 0;
    for run in &layout.runs {
        for glyph in &run.glyphs {
            if !cache.contains_key(&glyph.glyph_id) {
                let nonempty = outline_budget.validate(GlyphId(glyph.glyph_id))?;
                let mut pen = Pen::default();
                let outlined = face.outline_glyph(GlyphId(glyph.glyph_id), &mut pen);
                if pen.invalid || (outlined.is_none() && (nonempty || pen.ops != 0)) {
                    return Err(invalid(
                        "The glyph outline is malformed or exceeds its output limit.",
                    ));
                }
                cache.insert(glyph.glyph_id, pen);
            }
            let pen = &cache[&glyph.glyph_id];
            ops += pen.ops;
            if ops > MAX_OUTLINE_OPS || visible.len() + pen.content.len() + 200 > MAX_CONTENT_BYTES
            {
                return Err(invalid(
                    "The semantic outline content exceeds its output limit.",
                ));
            }
            if !pen.content.is_empty() {
                writeln!(
                    &mut visible,
                    "q {scale:.9} 0 0 {scale:.9} {:.7} {:.7} cm",
                    origin.0 + glyph.x,
                    origin.1 + glyph.y
                )
                .unwrap();
                visible.push_str(&pen.content);
                visible.push_str("f Q\n");
            }
            clusters
                .entry((glyph.cluster.utf8_start, glyph.cluster.utf8_end))
                .or_insert_with(|| (run.direction == TextDirection::Rtl, Vec::new()))
                .1
                .push(glyph);
        }
    }
    visible.push_str("Q\n");
    let mut cells = Vec::new();
    for ((start, end), (rtl, glyphs)) in clusters {
        let cluster = &text[start..end];
        let left = glyphs
            .iter()
            .map(|g| g.x - g.x_offset)
            .fold(f32::INFINITY, f32::min);
        let advance: f32 = glyphs.iter().map(|g| g.x_advance).sum();
        if advance < 0. {
            return Err(invalid(
                "Negative semantic cluster advances are unsupported.",
            ));
        }
        let ink: Vec<_> = glyphs.iter().filter_map(|g| g.bounds).collect();
        let ymin = ink
            .iter()
            .map(|b| b.y_min)
            .reduce(f32::min)
            .unwrap_or(-font_size * 0.2);
        let ymax = ink
            .iter()
            .map(|b| b.y_max)
            .reduce(f32::max)
            .unwrap_or(font_size * 0.8);
        let xmin = ink.iter().map(|b| b.x_min).fold(left, f32::min);
        let xmax = ink.iter().map(|b| b.x_max).fold(left + advance, f32::max);
        let count = cluster.chars().count();
        let cell_width = advance / count as f32;
        for (i, unicode) in cluster.chars().enumerate() {
            let x = left + cell_width * (if rtl { count - i - 1 } else { i }) as f32;
            cells.push(Cell {
                unicode,
                x,
                width: (cell_width / font_size * 1000.).round(),
                bounds: [
                    (xmin - x) / font_size * 1000.,
                    ymin / font_size * 1000.,
                    (xmax - x) / font_size * 1000.,
                    ymax / font_size * 1000.,
                ],
            });
        }
    }
    if cells.is_empty() || cells.len() != text.chars().count() {
        return Err(invalid(
            "Semantic clusters do not cover each source scalar exactly once.",
        ));
    }
    cells.sort_by(|a, b| a.x.total_cmp(&b.x));
    // A code describes Unicode and local metrics, not a page occurrence. Exact
    // keys preserve existing geometry; similar boxes must never be coalesced.
    // Keep all occurrences in one text object. Bank switches preserve the text
    // matrix and the previous TJ adjustment, including on rotated pages.
    let mut definition_codes = BTreeMap::new();
    let mut definitions = Vec::new();
    let mut codes = Vec::with_capacity(cells.len());
    for cell in &cells {
        let key = (
            cell.unicode,
            cell.width.to_bits(),
            cell.bounds.map(f32::to_bits),
        );
        let code = if let Some(&code) = definition_codes.get(&key) {
            code
        } else {
            if !font_banks && definitions.len() == CODES_PER_FONT {
                return Err(invalid(
                    "Semantic evidence supports at most 255 distinct character definitions (Unicode, advance and cluster bounds); font-bank probes fail reader order and selection.",
                ));
            }
            let code = definitions.len();
            definitions.push(cell);
            definition_codes.insert(key, code);
            code
        };
        codes.push(code);
    }
    let mut pdf = Document::with_version("1.7");
    let program = pdf.add_object(Stream::new(
        dictionary! {"Length1"=>font.bytes.len() as i64},
        font.bytes.to_vec(),
    ));
    let bbox = face.global_bounding_box();
    let font_scale = 1000. / face.units_per_em() as f32;
    let original_name = format!("FolioOriginal{}", font.info.id);
    let descriptor=pdf.add_object(dictionary!{"Type"=>"FontDescriptor","FontName"=>Object::Name(original_name.as_bytes().to_vec()),"Flags"=>32,"FontBBox"=>numbers(&[bbox.x_min as f32*font_scale,bbox.y_min as f32*font_scale,bbox.x_max as f32*font_scale,bbox.y_max as f32*font_scale]),"ItalicAngle"=>face.italic_angle(),"Ascent"=>face.ascender() as f32*font_scale,"Descent"=>face.descender() as f32*font_scale,"CapHeight"=>face.ascender() as f32*font_scale,"StemV"=>80,"FontFile2"=>program});
    let descendant=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"CIDFontType2","BaseFont"=>Object::Name(original_name.as_bytes().to_vec()),"CIDSystemInfo"=>dictionary!{"Registry"=>Object::string_literal("Adobe"),"Ordering"=>Object::string_literal("Identity"),"Supplement"=>0},"FontDescriptor"=>descriptor,"CIDToGIDMap"=>"Identity","DW"=>1000});
    let original=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type0","BaseFont"=>Object::Name(original_name.into_bytes()),"Encoding"=>"Identity-H","DescendantFonts"=>vec![Object::Reference(descendant)]});
    let mut font_resources = dictionary! {"Original" => original};
    for (bank_index, bank) in definitions.chunks(CODES_PER_FONT).enumerate() {
        let mut charprocs = lopdf::Dictionary::new();
        let mut differences = vec![Object::Integer(1)];
        let mut widths = Vec::new();
        let mut font_bbox = [
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ];
        let mut mappings = Vec::new();
        for (i, cell) in bank.iter().enumerate() {
            let name = format!("g{}", i + 1);
            differences.push(Object::Name(name.as_bytes().to_vec()));
            widths.push(Object::Real(cell.width));
            let [x0, y0, x1, y1] = cell.bounds;
            font_bbox[0] = font_bbox[0].min(x0);
            font_bbox[1] = font_bbox[1].min(y0);
            font_bbox[2] = font_bbox[2].max(x1);
            font_bbox[3] = font_bbox[3].max(y1);
            let proc = pdf.add_object(Stream::new(
                dictionary! {},
                format!("{} 0 {x0:.7} {y0:.7} {x1:.7} {y1:.7} d1\n", cell.width).into_bytes(),
            ));
            charprocs.set(name, proc);
            let unicode: String = cell
                .unicode
                .to_string()
                .encode_utf16()
                .map(|unit| format!("{unit:04X}"))
                .collect();
            mappings.push(format!("<{:02X}> <{unicode}>\n", i + 1));
        }
        let mut cmap=String::from("/CIDInit /ProcSet findresource begin 12 dict begin begincmap /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def /CMapName /FolioSemantic def /CMapType 2 def 1 begincodespacerange <00> <FF> endcodespacerange\n");
        for chunk in mappings.chunks(100) {
            writeln!(&mut cmap, "{} beginbfchar", chunk.len()).unwrap();
            for mapping in chunk {
                cmap.push_str(mapping);
            }
            cmap.push_str("endbfchar\n");
        }
        cmap.push_str("endcmap CMapName currentdict /CMap defineresource pop end end\n");
        let unicode = pdf.add_object(Stream::new(dictionary! {}, cmap.into_bytes()));
        let semantic=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type3","FontBBox"=>numbers(&font_bbox),"FontMatrix"=>numbers(&[0.001,0.,0.,0.001,0.,0.]),"CharProcs"=>charprocs,"Encoding"=>dictionary!{"Type"=>"Encoding","Differences"=>differences},"FirstChar"=>1,"LastChar"=>bank.len() as i64,"Widths"=>widths,"Resources"=>dictionary!{},"ToUnicode"=>unicode});
        let name = if bank_index == 0 {
            "S".to_owned()
        } else {
            format!("S{bank_index}")
        };
        font_resources.set(name, semantic);
    }
    if actual_text {
        let actual: String = text
            .encode_utf16()
            .map(|unit| format!("{unit:04X}"))
            .collect();
        writeln!(&mut visible, "/Span << /ActualText <FEFF{actual}> >> BDC").unwrap();
    }
    writeln!(
        &mut visible,
        "BT /S {font_size:.7} Tf 1 0 0 1 {:.7} {:.7} Tm [",
        origin.0 + cells[0].x,
        origin.1
    )
    .unwrap();
    for (i, cell) in cells.iter().enumerate() {
        let bank = codes[i] / CODES_PER_FONT;
        if i > 0 && bank != codes[i - 1] / CODES_PER_FONT {
            let name = if bank == 0 {
                "S".to_owned()
            } else {
                format!("S{bank}")
            };
            writeln!(&mut visible, "] TJ /{name} {font_size:.7} Tf [").unwrap();
        }
        write!(&mut visible, "<{:02X}> ", codes[i] % CODES_PER_FONT + 1).unwrap();
        if let Some(next) = cells.get(i + 1) {
            write!(
                &mut visible,
                "{:.7} ",
                (cell.x + cell.width * font_size / 1000. - next.x) / font_size * 1000.
            )
            .unwrap();
        }
    }
    visible.push_str("] TJ ET\n");
    if actual_text {
        visible.push_str("EMC\n");
    }
    if visible.len() > MAX_CONTENT_BYTES {
        return Err(invalid("The semantic content exceeds its byte limit."));
    }
    let content = pdf.add_object(Stream::new(dictionary! {}, visible.into_bytes()));
    let pages = pdf.new_object_id();
    let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>numbers(&[0.,0.,width,height]),"Rotate"=>rotation as i64,"Resources"=>dictionary!{"Font"=>font_resources},"Contents"=>content});
    pdf.objects.insert(
        pages,
        Object::Dictionary(
            dictionary! {"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1},
        ),
    );
    let catalog = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    pdf.trailer.set("Root", catalog);
    let mut bytes = BoundedPdf(Vec::new());
    pdf.save_to(&mut bytes)
        .map_err(|_| invalid("The semantic PDF exceeds its byte limit or could not be saved."))?;
    Ok(ShapedPdf {
        layout,
        bytes: bytes.0,
    })
}
