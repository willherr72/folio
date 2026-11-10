//! Private structural text-removal kernel; not an exposed area-redaction command.
//! Only bounded verified font resources and retained content enter the fresh graph.
use crate::redaction_content::{canonicalize, invalid, number, require, ContentBudget, Program};
use crate::EngineResult;
use lopdf::{content::Content, dictionary, Dictionary, Document, Object, ObjectId, Stream};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeSet, HashSet},
    io::{self, Write},
};

const PAGE_BYTES: usize = 8 * 1024 * 1024;
const TOTAL_BYTES: usize = 64 * 1024 * 1024;
pub(crate) struct TextBlock {
    pub page_index: usize,
    pub block_index: usize,
    pub text: String,
    pub selection_id: String,
}
pub(crate) struct RedactedTextPdf {
    pub bytes: Vec<u8>,
    pub removed_blocks: usize,
    pub omitted_metadata: Vec<&'static str>,
}
pub(super) struct Page {
    pub width: Object,
    pub height: Object,
    pub fonts: Vec<std::sync::Arc<crate::redaction_fonts::Font>>,
    pub graphics: Vec<std::sync::Arc<crate::redaction_graphics::State>>,
    pub images: Vec<std::sync::Arc<crate::redaction_images::Image>>,
    pub program: Program,
}
pub(super) struct Prepared {
    pub pages: Vec<Page>,
    digest: Vec<u8>,
    pub omitted: BTreeSet<&'static str>,
}
#[derive(Clone, Copy, Default)]
struct Inherited<'a> {
    media: Option<&'a Object>,
    crop: Option<&'a Object>,
    resources: Option<&'a Object>,
    rotate: Option<&'a Object>,
}

fn resolve<'a>(doc: &'a Document, mut o: &'a Object) -> EngineResult<&'a Object> {
    let mut seen = HashSet::new();
    while let Object::Reference(id) = o {
        require(seen.len() < 64 && seen.insert(*id))?;
        o = doc.objects.get(id).ok_or_else(invalid)?;
    }
    Ok(o)
}
fn dict<'a>(doc: &'a Document, o: &'a Object) -> EngineResult<&'a Dictionary> {
    resolve(doc, o)?.as_dict().map_err(|_| invalid())
}
fn keys(d: &Dictionary, allowed: &[&[u8]]) -> EngineResult<()> {
    require(d.iter().all(|(k, _)| allowed.contains(&k.as_slice())))
}
fn name(doc: &Document, o: &Object, expected: &[u8]) -> EngineResult<()> {
    require(resolve(doc, o)?.as_name().map_err(|_| invalid())? == expected)
}
fn inherited<'a>(d: &'a Dictionary, mut i: Inherited<'a>) -> Inherited<'a> {
    if let Ok(v) = d.get(b"MediaBox") {
        i.media = Some(v)
    }
    if let Ok(v) = d.get(b"CropBox") {
        i.crop = Some(v)
    }
    if let Ok(v) = d.get(b"Resources") {
        i.resources = Some(v)
    }
    if let Ok(v) = d.get(b"Rotate") {
        i.rotate = Some(v)
    }
    i
}
fn box_values(doc: &Document, o: &Object) -> EngineResult<[f64; 4]> {
    let a = resolve(doc, o)?.as_array().map_err(|_| invalid())?;
    require(a.len() == 4)?;
    Ok([
        number(resolve(doc, &a[0])?)?,
        number(resolve(doc, &a[1])?)?,
        number(resolve(doc, &a[2])?)?,
        number(resolve(doc, &a[3])?)?,
    ])
}
fn page_content(doc: &Document, d: &Dictionary, total: &mut usize) -> EngineResult<Vec<u8>> {
    let Some(o) = d.get(b"Contents").ok() else {
        return Ok(vec![]);
    };
    let o = resolve(doc, o)?;
    let mut result = vec![];
    let items: &[Object] = match o {
        Object::Array(a) => {
            require(a.len() <= 4096)?;
            a
        }
        _ => std::slice::from_ref(o),
    };
    for item in items {
        let s = resolve(doc, item)?.as_stream().map_err(|_| invalid())?;
        keys(&s.dict, &[b"Length", b"Filter", b"DecodeParms"])?;
        // Do not rely on the decompressor's permissive malformed-filter fallback.
        if let Ok(f) = s.dict.get(b"Filter") {
            require(matches!(f,Object::Name(n) if n==b"FlateDecode"))?;
        }
        if let Ok(p) = s.dict.get(b"DecodeParms") {
            require(matches!(p, Object::Null))?;
        }
        let available = (PAGE_BYTES - result.len()).min(TOTAL_BYTES - *total);
        require(available > 0)?;
        let decoded = s
            .decompressed_content_strict_with_limit(available - 1)
            .map_err(|_| invalid())?;
        *total += decoded.len() + 1;
        result.extend_from_slice(&decoded);
        result.push(b'\n');
    }
    Ok(result)
}
struct Builder<'a, 'images> {
    doc: &'a Document,
    visited: HashSet<ObjectId>,
    pages: Vec<Page>,
    budget: ContentBudget,
    fonts: crate::redaction_fonts::Registry,
    graphics: crate::redaction_graphics::Registry,
    images: &'images mut crate::redaction_images::Registry,
    allow_images: bool,
    bytes: usize,
    omitted: BTreeSet<&'static str>,
}
impl<'a> Builder<'a, '_> {
    fn walk(
        &mut self,
        id: ObjectId,
        parent: Option<ObjectId>,
        i: Inherited<'a>,
        depth: usize,
    ) -> EngineResult<usize> {
        require(depth < 64 && self.visited.insert(id))?;
        let d = self
            .doc
            .objects
            .get(&id)
            .ok_or_else(invalid)?
            .as_dict()
            .map_err(|_| invalid())?;
        match parent {
            Some(p) => require(
                d.get(b"Parent")
                    .map_err(|_| invalid())?
                    .as_reference()
                    .map_err(|_| invalid())?
                    == p,
            )?,
            None => require(!d.has(b"Parent"))?,
        }
        let i = inherited(d, i);
        let kind = resolve(self.doc, d.get(b"Type").map_err(|_| invalid())?)?
            .as_name()
            .map_err(|_| invalid())?;
        require(parent.is_some() || kind == b"Pages")?;
        if kind == b"Pages" {
            keys(
                d,
                &[
                    b"Type",
                    b"Parent",
                    b"Kids",
                    b"Count",
                    b"MediaBox",
                    b"CropBox",
                    b"Resources",
                    b"Rotate",
                ],
            )?;
            let kids = resolve(self.doc, d.get(b"Kids").map_err(|_| invalid())?)?
                .as_array()
                .map_err(|_| invalid())?;
            require(!kids.is_empty() && kids.len() <= 1000)?;
            let mut count = 0;
            for kid in kids {
                count += self.walk(
                    kid.as_reference().map_err(|_| invalid())?,
                    Some(id),
                    i,
                    depth + 1,
                )?;
            }
            require(
                resolve(self.doc, d.get(b"Count").map_err(|_| invalid())?)?
                    .as_i64()
                    .map_err(|_| invalid())?
                    == count as i64,
            )?;
            return Ok(count);
        }
        require(kind == b"Page" && self.pages.len() < 1000)?;
        keys(
            d,
            &[
                b"Type",
                b"Parent",
                b"MediaBox",
                b"CropBox",
                b"TrimBox",
                b"BleedBox",
                b"ArtBox",
                b"Resources",
                b"Rotate",
                b"UserUnit",
                b"Contents",
                b"Metadata",
                b"PieceInfo",
                b"LastModified",
            ],
        )?;
        for k in [b"Metadata".as_slice(), b"PieceInfo", b"LastModified"] {
            if d.has(k) {
                self.omitted.insert("page metadata");
            }
        }
        let media = box_values(self.doc, i.media.ok_or_else(invalid)?)?;
        require(
            media[0] == 0.0
                && media[1] == 0.0
                && (1.0..=20_000.0).contains(&media[2])
                && (1.0..=20_000.0).contains(&media[3]),
        )?;
        if let Some(crop) = i.crop {
            require(box_values(self.doc, crop)? == media)?;
        }
        for key in [b"TrimBox".as_slice(), b"BleedBox", b"ArtBox"] {
            if let Ok(v) = d.get(key) {
                require(box_values(self.doc, v)? == media)?;
            }
        }
        if let Some(v) = i.rotate {
            require(number(resolve(self.doc, v)?)? == 0.0)?;
        }
        if let Ok(v) = d.get(b"UserUnit") {
            require(number(resolve(self.doc, v)?)? == 1.0)?;
        }
        let resources = i.resources.map(|r| dict(self.doc, r)).transpose()?;
        if let Some(r) = resources {
            keys(
                r,
                &[
                    b"Font",
                    b"ColorSpace",
                    b"XObject",
                    b"ExtGState",
                    b"Pattern",
                    b"Shading",
                    b"Properties",
                    b"ProcSet",
                ],
            )?;
            if let Ok(colors) = r.get(b"ColorSpace") {
                let colors = dict(self.doc, colors)?;
                require(
                    ![b"DefaultGray".as_slice(), b"DefaultRGB", b"DefaultCMYK"]
                        .iter()
                        .any(|k| colors.has(k)),
                )?;
            }
        }
        let data = page_content(self.doc, d, &mut self.bytes)?;
        let mut resolver =
            crate::redaction_fonts::PageFonts::new(self.doc, resources, &mut self.fonts);
        let mut graphics =
            crate::redaction_graphics::PageStates::new(self.doc, resources, &mut self.graphics);
        let mut images = crate::redaction_images::PageImages::new(
            self.doc,
            resources,
            self.images,
            self.allow_images,
        );
        let program = canonicalize(
            &data,
            &mut resolver,
            &mut graphics,
            &mut images,
            &mut self.budget,
        )?;
        let fonts = resolver.fonts;
        let a = resolve(self.doc, i.media.unwrap())?
            .as_array()
            .map_err(|_| invalid())?;
        self.pages.push(Page {
            width: resolve(self.doc, &a[2])?.clone(),
            height: resolve(self.doc, &a[3])?.clone(),
            fonts,
            graphics: graphics.states,
            images: images.images,
            program,
        });
        Ok(1)
    }
}
fn hash_piece(h: &mut Sha256, data: &[u8]) {
    h.update((data.len() as u64).to_be_bytes());
    h.update(data);
}
pub(super) fn prepare(doc: &Document) -> EngineResult<Prepared> {
    prepare_mode(doc, false, &mut Default::default())
}
pub(super) fn prepare_area(doc: &Document) -> EngineResult<Prepared> {
    prepare_area_with_context(doc, &mut Default::default())
}
pub(super) fn prepare_area_with_context(
    doc: &Document,
    images: &mut crate::redaction_images::ImageContext,
) -> EngineResult<Prepared> {
    prepare_mode(doc, true, images.begin_document())
}
fn prepare_mode(
    doc: &Document,
    allow_images: bool,
    images: &mut crate::redaction_images::Registry,
) -> EngineResult<Prepared> {
    // Raw sources enter through redaction_numbers::load_pdf. An arbitrary
    // already-parsed Document cannot recover original numeric spellings.
    require(doc.objects.len() <= 100_000 && !doc.trailer.has(b"Encrypt"))?;
    let catalog = dict(doc, doc.trailer.get(b"Root").map_err(|_| invalid())?)?;
    name(
        doc,
        catalog.get(b"Type").map_err(|_| invalid())?,
        b"Catalog",
    )?;
    keys(
        catalog,
        &[
            b"Type",
            b"Pages",
            b"Version",
            b"Metadata",
            b"Names",
            b"Dests",
            b"Outlines",
            b"PageLabels",
            b"ViewerPreferences",
            b"PageLayout",
            b"PageMode",
            b"Lang",
            b"OpenAction",
            b"AA",
        ],
    )?;
    let mut b = Builder {
        doc,
        visited: HashSet::new(),
        pages: vec![],
        budget: ContentBudget::default(),
        fonts: Default::default(),
        graphics: Default::default(),
        images,
        allow_images,
        bytes: 0,
        omitted: BTreeSet::new(),
    };
    b.omitted
        .insert("comments, unused resources and unreachable objects");
    if doc.trailer.has(b"Info") || doc.trailer.has(b"ID") {
        b.omitted.insert("document information and identifiers");
    }
    if catalog
        .iter()
        .any(|(k, _)| !matches!(k.as_slice(), b"Type" | b"Pages" | b"Version"))
    {
        b.omitted
            .insert("catalog metadata, navigation and attachments");
    }
    b.walk(
        catalog
            .get(b"Pages")
            .map_err(|_| invalid())?
            .as_reference()
            .map_err(|_| invalid())?,
        None,
        Inherited::default(),
        0,
    )?;
    require(!b.pages.is_empty())?;
    let mut h = Sha256::new();
    h.update(b"Folio private whole-block redaction v1");
    for p in &b.pages {
        hash_piece(&mut h, &number(&p.width)?.to_be_bytes());
        hash_piece(&mut h, &number(&p.height)?.to_be_bytes());
        h.update((p.fonts.len() as u64).to_be_bytes());
        for f in &p.fonts {
            hash_piece(&mut h, &f.digest);
        }
        h.update((p.graphics.len() as u64).to_be_bytes());
        for state in &p.graphics {
            hash_piece(&mut h, &state.digest);
        }
        // Preserve existing text selection identities when images are absent.
        if !p.images.is_empty() {
            h.update(b"Folio area image resources v1");
            h.update((p.images.len() as u64).to_be_bytes());
            for image in &p.images {
                hash_piece(&mut h, &image.digest);
            }
        }
        hash_piece(
            &mut h,
            &Content {
                operations: &p.program.operations,
            }
            .encode()
            .map_err(|_| invalid())?,
        );
    }
    Ok(Prepared {
        pages: b.pages,
        digest: h.finalize().to_vec(),
        omitted: b.omitted,
    })
}
pub(super) fn selection_id(p: &Prepared, page: usize, block: usize) -> String {
    let mut h = Sha256::new();
    h.update(&p.digest);
    h.update((page as u64).to_be_bytes());
    h.update((block as u64).to_be_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}
pub(crate) fn inspect_standard_text(doc: &Document) -> EngineResult<Vec<TextBlock>> {
    let p = prepare(doc)?;
    Ok(p.pages
        .iter()
        .enumerate()
        .flat_map(|(page_index, page)| {
            page.program
                .blocks
                .iter()
                .map(move |block| (page_index, block))
        })
        .map(|(page_index, b)| TextBlock {
            page_index,
            block_index: b.index,
            text: b.text.clone(),
            selection_id: selection_id(&p, page_index, b.index),
        })
        .collect())
}
struct BoundedOutput(Vec<u8>);
impl Write for BoundedOutput {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if data.len() > TOTAL_BYTES - self.0.len() {
            return Err(io::Error::other("Redaction output limit"));
        }
        self.0.extend_from_slice(data);
        Ok(data.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub(crate) fn remove_standard_text(
    doc: &Document,
    ids: &[String],
) -> EngineResult<RedactedTextPdf> {
    require(!ids.is_empty() && ids.len() <= 4096 && ids.iter().all(|id| id.len() == 64))?;
    let targets: HashSet<&str> = ids.iter().map(String::as_str).collect();
    require(targets.len() == ids.len())?;
    let mut p = prepare(doc)?;
    let mut removals = vec![];
    let mut found = 0;
    for (page_index, page) in p.pages.iter().enumerate() {
        let mut shows = HashSet::new();
        for block in &page.program.blocks {
            if targets.contains(selection_id(&p, page_index, block.index).as_str()) {
                found += 1;
                shows.extend(block.shows.iter().copied());
            }
        }
        removals.push(shows);
    }
    require(found == targets.len())?;
    for (page, removed) in p.pages.iter_mut().zip(removals) {
        page.program.operations = std::mem::take(&mut page.program.operations)
            .into_iter()
            .enumerate()
            .filter_map(|(i, op)| (!removed.contains(&i)).then_some(op))
            .collect();
    }
    Ok(RedactedTextPdf {
        bytes: publish(p.pages)?,
        removed_blocks: found,
        omitted_metadata: p.omitted.into_iter().collect(),
    })
}

pub(super) fn publish(pages: Vec<Page>) -> EngineResult<Vec<u8>> {
    let mut out = Document::with_version("1.7");
    let tree = out.new_object_id();
    let mut kids = vec![];
    let mut publisher = crate::redaction_fonts::Publisher::default();
    let mut graphics_publisher = crate::redaction_graphics::Publisher::default();
    let mut image_ids = std::collections::HashMap::new();
    let mut image_bytes = 0usize;
    let mut image_paint_budget = crate::redaction_jpeg_coverage::CoverageBudget::default();
    for page in pages {
        let mut fonts = Dictionary::new();
        for (index, f) in page.fonts.into_iter().enumerate() {
            let id = publisher.publish(&f, &mut out);
            fonts.set(format!("F{index}"), id);
        }
        let operations = crate::redaction_images::publish_operations(
            page.program.operations,
            &page.images,
            &mut image_paint_budget,
        )?;
        let mut resources = dictionary! {"Font"=>fonts};
        let mut images = Dictionary::new();
        for op in operations.iter().filter(|op| op.operator == "Do") {
            require(op.operands.len() == 1)?;
            let alias = op.operands[0].as_name().map_err(|_| invalid())?;
            let index = std::str::from_utf8(alias)
                .map_err(|_| invalid())?
                .strip_prefix('I')
                .ok_or_else(invalid)?
                .parse::<usize>()
                .map_err(|_| invalid())?;
            let image = page.images.get(index).ok_or_else(invalid)?;
            let id = if let Some(id) = image_ids.get(&image.digest) {
                *id
            } else {
                require(
                    image.retained_bytes()
                        <= crate::redaction_images::SAMPLE_BUDGET.saturating_sub(image_bytes),
                )?;
                image_bytes += image.retained_bytes();
                let id = out.add_object(image.stream()?);
                image_ids.insert(image.digest, id);
                id
            };
            images.set(alias.to_vec(), id);
        }
        if !images.is_empty() {
            resources.set("XObject", images);
        }
        if !page.graphics.is_empty() {
            let mut graphics = Dictionary::new();
            for (index, state) in page.graphics.iter().enumerate() {
                graphics.set(
                    format!("G{index}"),
                    graphics_publisher.publish(state, &mut out),
                );
            }
            resources.set("ExtGState", graphics);
        }
        let contents = Content { operations }.encode().map_err(|_| invalid())?;
        require(contents.len() <= TOTAL_BYTES)?;
        crate::redaction_content::preflight(&contents, &mut ContentBudget::default())?;
        let stream = out.add_object(Stream::new(dictionary! {}, contents));
        let page=out.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),page.width,page.height],"Resources"=>resources,"Contents"=>stream});
        kids.push(Object::Reference(page));
    }
    out.objects.insert(
        tree,
        Object::Dictionary(dictionary! {"Type"=>"Pages","Count"=>kids.len() as i64,"Kids"=>kids}),
    );
    let catalog = out.add_object(dictionary! {"Type"=>"Catalog","Pages"=>tree});
    out.trailer.set("Root", catalog);
    crate::redaction_numbers::validate_document_numbers(&out)?;
    let mut output = BoundedOutput(vec![]);
    out.save_to(&mut output).map_err(|_| invalid())?;
    Ok(output.0)
}
