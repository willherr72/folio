//! Owned, bounded font resources for structural text removal.
use crate::{
    redaction_content::{invalid, number, require, TextFonts},
    EngineResult,
};
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};

const MAX_PROGRAM: usize = 16 * 1024 * 1024;
const MAX_MAP: usize = 2 * 1024 * 1024;
const MAX_PAYLOAD: usize = 64 * 1024 * 1024;
const MAX_NODES: usize = 100_000;

struct OwnedStream {
    value: Stream,
    digest: [u8; 32],
}
type Streams = BTreeMap<ObjectId, Arc<OwnedStream>>;
pub(super) struct Font {
    root: Object,
    streams: Streams,
    pub digest: [u8; 32],
    code_len: usize,
    standard: bool,
    codes: BTreeMap<u16, (char, f32)>,
}
impl Font {
    pub fn supports_space_measurement_probe(&self) -> bool {
        self.standard
    }
    pub fn code_len(&self) -> usize {
        self.code_len
    }
    pub fn glyph(&self, code: u16) -> EngineResult<(char, f32)> {
        self.codes.get(&code).copied().ok_or_else(invalid)
    }
    pub fn source_character(&self, code: u16) -> EngineResult<char> {
        let actual = self.glyph(code)?.0;
        Ok(if self.standard {
            char::from(code as u8)
        } else {
            actual
        })
    }
    pub fn decode(&self, bytes: &[u8]) -> EngineResult<String> {
        require(bytes.len() % self.code_len == 0)?;
        bytes
            .chunks(self.code_len)
            .map(|chunk| self.source_character(code(chunk)))
            .collect()
    }
}
pub(super) fn code(bytes: &[u8]) -> u16 {
    if bytes.len() == 1 {
        u16::from(bytes[0])
    } else {
        u16::from_be_bytes([bytes[0], bytes[1]])
    }
}

#[derive(Clone, Copy)]
enum Role {
    Root,
    Descendant,
    Descriptor,
    Encoding,
    System,
    Program,
    Map,
    Value,
}
struct Budget {
    bytes: usize,
    nodes: usize,
    max_bytes: usize,
    max_nodes: usize,
    streams: Streams,
    stream_sources: HashMap<usize, ObjectId>,
    stream_digests: HashMap<[u8; 32], ObjectId>,
    used_streams: HashSet<ObjectId>,
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            bytes: 0,
            nodes: 0,
            max_bytes: MAX_PAYLOAD,
            max_nodes: MAX_NODES,
            streams: Default::default(),
            stream_sources: Default::default(),
            stream_digests: Default::default(),
            used_streams: Default::default(),
        }
    }
}
impl Budget {
    fn take(&mut self, bytes: usize) -> EngineResult<()> {
        require(bytes <= self.max_bytes - self.bytes && self.nodes < self.max_nodes)?;
        self.bytes += bytes;
        self.nodes += 1;
        Ok(())
    }
    fn copy(
        &mut self,
        pdf: &Document,
        value: &Object,
        role: Role,
        depth: usize,
        active: &mut HashSet<ObjectId>,
    ) -> EngineResult<Object> {
        require(depth < 64)?;
        self.take(0)?;
        if let Object::Reference(id) = value {
            require(active.insert(*id))?;
            let result = self.copy(
                pdf,
                pdf.objects.get(id).ok_or_else(invalid)?,
                role,
                depth + 1,
                active,
            );
            active.remove(id);
            return result;
        }
        match value {
            Object::Stream(s) => {
                let limit = match role {
                    Role::Program => MAX_PROGRAM,
                    Role::Map => MAX_MAP,
                    _ => return Err(invalid()),
                };
                for (k, _) in &s.dict {
                    require(
                        matches!(k.as_slice(), b"Length" | b"Filter" | b"DecodeParms")
                            || (matches!(role, Role::Program) && k == b"Length1"),
                    )?;
                }
                if let Ok(v) = s.dict.get(b"Filter") {
                    require(matches!(v,Object::Name(n) if n==b"FlateDecode"))?;
                }
                if let Ok(v) = s.dict.get(b"DecodeParms") {
                    require(matches!(v, Object::Null))?;
                }
                let source = value as *const Object as usize;
                if let Some(&id) = self.stream_sources.get(&source) {
                    require(self.streams[&id].value.content.len() <= limit)?;
                    self.used_streams.insert(id);
                    return Ok(Object::Reference(id));
                }
                let available = limit.min(self.max_bytes - self.bytes);
                let bytes = s
                    .decompressed_content_strict_with_limit(available)
                    .map_err(|_| invalid())?;
                let mut dict = Dictionary::new();
                if let Ok(v) = s.dict.get(b"Length1") {
                    require(matches!(v,Object::Integer(n) if *n>=0&&*n as usize==bytes.len()))?;
                    dict.set("Length1", v.clone());
                }
                let stream = Stream::new(dict, bytes);
                let digest = stream_digest(&stream);
                let id = if let Some(id) = self.stream_digests.get(&digest) {
                    *id
                } else {
                    self.take(stream.content.len())?;
                    let id = (self.streams.len() as u32 + 1, 0);
                    self.streams.insert(
                        id,
                        Arc::new(OwnedStream {
                            value: stream,
                            digest,
                        }),
                    );
                    self.stream_digests.insert(digest, id);
                    id
                };
                self.stream_sources.insert(source, id);
                self.used_streams.insert(id);
                Ok(Object::Reference(id))
            }
            Object::Dictionary(d) => {
                let allowed: &[&[u8]] = match role {
                    Role::Root => &[
                        b"Type",
                        b"Subtype",
                        b"BaseFont",
                        b"Name",
                        b"Encoding",
                        b"FirstChar",
                        b"LastChar",
                        b"Widths",
                        b"FontDescriptor",
                        b"ToUnicode",
                        b"DescendantFonts",
                    ],
                    Role::Descendant => &[
                        b"Type",
                        b"Subtype",
                        b"BaseFont",
                        b"CIDSystemInfo",
                        b"FontDescriptor",
                        b"DW",
                        b"W",
                        b"CIDToGIDMap",
                    ],
                    Role::Descriptor => &[
                        b"Type",
                        b"FontName",
                        b"FontFamily",
                        b"FontStretch",
                        b"FontWeight",
                        b"Flags",
                        b"FontBBox",
                        b"ItalicAngle",
                        b"Ascent",
                        b"Descent",
                        b"Leading",
                        b"CapHeight",
                        b"XHeight",
                        b"StemV",
                        b"StemH",
                        b"AvgWidth",
                        b"MaxWidth",
                        b"MissingWidth",
                        b"CharSet",
                        b"FontFile2",
                        b"CIDSet",
                    ],
                    Role::Encoding => &[b"Type", b"BaseEncoding", b"Differences"],
                    Role::System => &[b"Registry", b"Ordering", b"Supplement"],
                    _ => return Err(invalid()),
                };
                require(d.iter().all(|(k, _)| allowed.contains(&k.as_slice())))?;
                let mut result = Dictionary::new();
                for (key, value) in d {
                    self.take(key.len())?;
                    let child = match key.as_slice() {
                        b"DescendantFonts" => Role::Descendant,
                        b"FontDescriptor" => Role::Descriptor,
                        b"Encoding" => Role::Encoding,
                        b"CIDSystemInfo" => Role::System,
                        b"FontFile2" => Role::Program,
                        b"ToUnicode" | b"CIDToGIDMap" | b"CIDSet" => Role::Map,
                        _ => Role::Value,
                    };
                    result.set(
                        key.clone(),
                        self.copy(pdf, value, child, depth + 1, active)?,
                    );
                }
                Ok(Object::Dictionary(result))
            }
            Object::Array(a) => {
                require(a.len() <= self.max_nodes - self.nodes)?;
                let mut result = Vec::with_capacity(a.len());
                for item in a {
                    result.push(self.copy(pdf, item, role, depth + 1, active)?);
                }
                Ok(Object::Array(result))
            }
            Object::Name(b) => {
                require(b.len() <= 256)?;
                self.take(b.len())?;
                Ok(value.clone())
            }
            Object::String(b, _) => {
                require(b.len() <= 4096)?;
                self.take(b.len())?;
                Ok(value.clone())
            }
            Object::Integer(_) | Object::Real(_) => {
                number(value)?;
                Ok(value.clone())
            }
            Object::Null => Ok(Object::Null),
            _ => Err(invalid()),
        }
    }
}
fn hash(value: &Object, h: &mut Sha256, streams: &Streams) {
    fn piece(h: &mut Sha256, b: &[u8]) {
        h.update((b.len() as u64).to_be_bytes());
        h.update(b);
    }
    match value {
        Object::Null => h.update(b"0"),
        Object::Boolean(b) => h.update([b'b', u8::from(*b)]),
        Object::Integer(n) => {
            h.update(b"i");
            h.update(n.to_be_bytes());
        }
        Object::Real(n) => {
            h.update(b"r");
            h.update(n.to_bits().to_be_bytes());
        }
        Object::Name(b) => {
            h.update(b"n");
            piece(h, b);
        }
        Object::String(b, _) => {
            h.update(b"s");
            piece(h, b);
        }
        Object::Array(a) => {
            h.update(b"a");
            h.update((a.len() as u64).to_be_bytes());
            for v in a {
                hash(v, h, streams);
            }
        }
        Object::Dictionary(d) => {
            h.update(b"d");
            h.update((d.len() as u64).to_be_bytes());
            let sorted: BTreeMap<_, _> = d.iter().collect();
            for (k, v) in sorted {
                piece(h, k);
                hash(v, h, streams);
            }
        }
        Object::Stream(s) => {
            h.update(b"t");
            hash(&Object::Dictionary(s.dict.clone()), h, streams);
            piece(h, &s.content);
        }
        Object::Reference(id) => {
            h.update(b"owned-stream");
            h.update(streams[id].digest);
        }
    }
}
fn stream_digest(stream: &Stream) -> [u8; 32] {
    let mut h = Sha256::new();
    hash(
        &Object::Dictionary(stream.dict.clone()),
        &mut h,
        &Streams::new(),
    );
    h.update((stream.content.len() as u64).to_be_bytes());
    h.update(&stream.content);
    h.finalize().into()
}
fn resolve<'a>(pdf: &'a Document, mut v: &'a Object) -> EngineResult<&'a Object> {
    let mut seen = HashSet::new();
    while let Object::Reference(id) = v {
        require(seen.len() < 64 && seen.insert(*id))?;
        v = pdf.objects.get(id).ok_or_else(invalid)?;
    }
    Ok(v)
}
#[derive(Default)]
pub(super) struct Registry {
    cache: HashMap<usize, Arc<Font>>,
    owned: HashMap<[u8; 32], Arc<Font>>,
    embedded: usize,
    budget: Budget,
}
impl Registry {
    fn load(&mut self, pdf: &Document, value: &Object) -> EngineResult<Arc<Font>> {
        let value = resolve(pdf, value)?;
        let key = value as *const Object as usize;
        if let Some(font) = self.cache.get(&key) {
            return Ok(font.clone());
        }
        let d = value.as_dict().map_err(|_| invalid())?;
        let subtype = resolve(pdf, d.get(b"Subtype").map_err(|_| invalid())?)?
            .as_name()
            .map_err(|_| invalid())?;
        self.budget.used_streams.clear();
        let (root, code_len, codes) = if subtype == b"Type1" {
            require(d.iter().all(|(k, _)| {
                [
                    b"Type".as_slice(),
                    b"Subtype",
                    b"BaseFont",
                    b"Encoding",
                    b"Name",
                ]
                .contains(&k.as_slice())
            }))?;
            require(
                resolve(pdf, d.get(b"Type").map_err(|_| invalid())?)?
                    .as_name()
                    .ok()
                    == Some(b"Font"),
            )?;
            let base = resolve(pdf, d.get(b"BaseFont").map_err(|_| invalid())?)?
                .as_name()
                .map_err(|_| invalid())?;
            require(matches!(
                base,
                b"Helvetica"
                    | b"Helvetica-Bold"
                    | b"Helvetica-Oblique"
                    | b"Helvetica-BoldOblique"
                    | b"Times-Roman"
                    | b"Times-Bold"
                    | b"Times-Italic"
                    | b"Times-BoldItalic"
                    | b"Courier"
                    | b"Courier-Bold"
                    | b"Courier-Oblique"
                    | b"Courier-BoldOblique"
            ))?;
            let ansi = if let Ok(v) = d.get(b"Encoding") {
                require(resolve(pdf, v)?.as_name().ok() == Some(b"WinAnsiEncoding"))?;
                true
            } else {
                false
            };
            let table = crate::engine::nominal_standard_ascii_metrics(base, ansi)?;
            let codes = (32u16..=126)
                .map(|c| {
                    (
                        c,
                        (
                            char::from_u32(table.1[usize::from(c - 32)]).unwrap(),
                            table.0[usize::from(c - 32)] as f32,
                        ),
                    )
                })
                .collect();
            let mut root = dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>Object::Name(base.to_vec())};
            if ansi {
                root.set("Encoding", "WinAnsiEncoding");
            }
            (Object::Dictionary(root), 1, codes)
        } else {
            require(matches!(subtype, b"TrueType" | b"Type0") && self.embedded < 64)?;
            let root = self
                .budget
                .copy(pdf, value, Role::Root, 0, &mut HashSet::new())?;
            let dictionary = root.as_dict().map_err(|_| invalid())?;
            require(dictionary.get(b"Type").and_then(Object::as_name).ok() == Some(b"Font"))?;
            // Proof input is temporary and contains only streams reached by this
            // resource. Retained resources share Arc allocations across fonts.
            let mut proof = Document::new();
            for id in &self.budget.used_streams {
                proof
                    .objects
                    .insert(*id, Object::Stream(self.budget.streams[id].value.clone()));
            }
            let (length, codes) = crate::engine::redaction_font_codes(&proof, dictionary)?;
            require(matches!(length, 1 | 2))?;
            self.embedded += 1;
            (root, length, codes)
        };
        let mut h = Sha256::new();
        let streams: Streams = self
            .budget
            .used_streams
            .iter()
            .map(|id| (*id, self.budget.streams[id].clone()))
            .collect();
        hash(&root, &mut h, &streams);
        let digest = h.finalize().into();
        if let Some(font) = self.owned.get(&digest) {
            self.cache.insert(key, font.clone());
            return Ok(font.clone());
        }
        let font = Arc::new(Font {
            root,
            streams,
            digest,
            code_len,
            standard: subtype == b"Type1",
            codes,
        });
        self.owned.insert(digest, font.clone());
        self.cache.insert(key, font.clone());
        Ok(font)
    }
}
pub(super) struct PageFonts<'a, 'b> {
    pdf: &'a Document,
    resources: Option<&'a Dictionary>,
    registry: &'b mut Registry,
    pub fonts: Vec<Arc<Font>>,
}
impl<'a, 'b> PageFonts<'a, 'b> {
    pub fn new(
        pdf: &'a Document,
        resources: Option<&'a Dictionary>,
        registry: &'b mut Registry,
    ) -> Self {
        Self {
            pdf,
            resources,
            registry,
            fonts: vec![],
        }
    }
}
impl TextFonts for PageFonts<'_, '_> {
    fn select(&mut self, alias: &[u8]) -> EngineResult<usize> {
        let fonts = resolve(
            self.pdf,
            self.resources
                .ok_or_else(invalid)?
                .get(b"Font")
                .map_err(|_| invalid())?,
        )?
        .as_dict()
        .map_err(|_| invalid())?;
        let font = self
            .registry
            .load(self.pdf, fonts.get(alias).map_err(|_| invalid())?)?;
        if let Some(i) = self.fonts.iter().position(|f| f.digest == font.digest) {
            return Ok(i);
        }
        self.fonts.push(font);
        Ok(self.fonts.len() - 1)
    }
    fn decode(&self, index: usize, bytes: &[u8]) -> EngineResult<String> {
        self.fonts.get(index).ok_or_else(invalid)?.decode(bytes)
    }
}
#[derive(Default)]
pub(super) struct Publisher {
    fonts: HashMap<[u8; 32], ObjectId>,
    streams: HashMap<[u8; 32], ObjectId>,
}
impl Publisher {
    pub fn publish(&mut self, font: &Font, pdf: &mut Document) -> ObjectId {
        if let Some(id) = self.fonts.get(&font.digest) {
            return *id;
        }
        fn install(
            v: &Object,
            pdf: &mut Document,
            resources: &Streams,
            published: &mut HashMap<[u8; 32], ObjectId>,
        ) -> Object {
            match v {
                Object::Dictionary(d) => {
                    let mut out = Dictionary::new();
                    for (k, v) in d {
                        out.set(k.clone(), install(v, pdf, resources, published));
                    }
                    Object::Reference(pdf.add_object(out))
                }
                Object::Array(a) => Object::Array(
                    a.iter()
                        .map(|v| install(v, pdf, resources, published))
                        .collect(),
                ),
                Object::Reference(id) => {
                    let stream = &resources[id];
                    let output = *published
                        .entry(stream.digest)
                        .or_insert_with(|| pdf.add_object(stream.value.clone()));
                    Object::Reference(output)
                }
                Object::Stream(_) => unreachable!("owned streams are indirect"),
                _ => v.clone(),
            }
        }
        let id = install(&font.root, pdf, &font.streams, &mut self.streams)
            .as_reference()
            .expect("owned font root dictionary");
        self.fonts.insert(font.digest, id);
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redaction_font_equal_standard_resources_share_metric_tables() {
        let mut pdf = Document::new();
        let ids: Vec<_> = (0..128)
            .map(|_| {
                pdf.add_object(
                    dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"},
                )
            })
            .collect();
        let mut registry = Registry::default();
        let first = registry.load(&pdf, &Object::Reference(ids[0])).unwrap();
        for id in ids {
            let next = registry.load(&pdf, &Object::Reference(id)).unwrap();
            assert!(
                Arc::ptr_eq(&first, &next),
                "identical resources must share their retained metric table"
            );
        }
    }
    #[test]
    fn redaction_font_shared_stream_is_charged_and_retained_once() {
        let pdf = Document::new();
        let stream = Object::Stream(Stream::new(dictionary! {}, vec![42; 24]));
        let mut budget = Budget::default();
        let first = budget
            .copy(&pdf, &stream, Role::Program, 0, &mut HashSet::new())
            .unwrap();
        let second = budget
            .copy(&pdf, &stream, Role::Program, 0, &mut HashSet::new())
            .unwrap();
        assert_eq!(
            budget.bytes, 24,
            "same program allocation must not count or copy twice"
        );
        assert_eq!(
            first.as_reference().unwrap(),
            second.as_reference().unwrap()
        );
    }
    #[test]
    fn redaction_font_budget_refuses_before_copying_streams_and_large_containers() {
        assert_eq!(
            (MAX_PROGRAM, MAX_MAP, MAX_PAYLOAD, MAX_NODES),
            (16 * 1024 * 1024, 2 * 1024 * 1024, 64 * 1024 * 1024, 100_000)
        );
        let pdf = Document::new();
        let stream = Object::Stream(Stream::new(dictionary! {}, vec![42; 33]));
        let mut budget = Budget {
            max_bytes: 32,
            ..Default::default()
        };
        assert!(budget
            .copy(&pdf, &stream, Role::Map, 0, &mut HashSet::new())
            .is_err());
        assert_eq!(budget.bytes, 0);
        let mut stream = Stream::new(dictionary! {}, vec![42; 1000]);
        stream.compress().unwrap();
        let mut budget = Budget {
            max_bytes: 32,
            ..Default::default()
        };
        assert!(budget
            .copy(
                &pdf,
                &Object::Stream(stream),
                Role::Map,
                0,
                &mut HashSet::new()
            )
            .is_err());
        assert_eq!(budget.bytes, 0);
        let mut budget = Budget {
            max_nodes: 4,
            ..Default::default()
        };
        assert!(budget
            .copy(
                &pdf,
                &Object::Array(vec![0.into(); 8]),
                Role::Value,
                0,
                &mut HashSet::new()
            )
            .is_err());
        assert_eq!(budget.nodes, 1);
        let mut budget = Budget {
            max_bytes: 40,
            ..Default::default()
        };
        let first = Object::Stream(Stream::new(dictionary! {}, vec![42; 24]));
        budget
            .copy(&pdf, &first, Role::Map, 0, &mut HashSet::new())
            .unwrap();
        assert!(budget
            .copy(
                &pdf,
                &Object::Stream(Stream::new(dictionary! {}, vec![43; 24])),
                Role::Map,
                0,
                &mut HashSet::new()
            )
            .is_err());
        assert_eq!(budget.bytes, 24);
    }
    #[test]
    fn redaction_font_cached_program_does_not_bypass_smaller_map_limit() {
        let pdf = Document::new();
        let stream = Object::Stream(Stream::new(dictionary! {}, vec![0; MAX_MAP + 1]));
        let mut budget = Budget::default();
        budget
            .copy(&pdf, &stream, Role::Program, 0, &mut HashSet::new())
            .unwrap();
        assert!(budget
            .copy(&pdf, &stream, Role::Map, 0, &mut HashSet::new())
            .is_err());
    }
    #[test]
    fn redaction_font_graph_cycles_unknown_fields_and_filters_refuse() {
        let mut pdf = Document::new();
        let id = pdf.new_object_id();
        pdf.objects.insert(id, Object::Reference(id));
        assert!(Budget::default()
            .copy(
                &pdf,
                &Object::Reference(id),
                Role::Root,
                0,
                &mut HashSet::new()
            )
            .is_err());
        for (root, role) in [
            (
                Object::Dictionary(dictionary! {"Metadata"=>Object::Reference(id)}),
                Role::Root,
            ),
            (
                Object::Stream(Stream::new(
                    dictionary! {"F"=>Object::string_literal("external-file")},
                    vec![],
                )),
                Role::Program,
            ),
            (
                Object::Stream(Stream::new(dictionary! {"Filter"=>"LZWDecode"}, vec![])),
                Role::Map,
            ),
        ] {
            assert!(Budget::default()
                .copy(&pdf, &root, role, 0, &mut HashSet::new())
                .is_err());
        }
        let mut deep = Object::Integer(0);
        for _ in 0..65 {
            deep = Object::Array(vec![deep]);
        }
        assert!(Budget::default()
            .copy(&pdf, &deep, Role::Value, 0, &mut HashSet::new())
            .is_err());
    }
}
