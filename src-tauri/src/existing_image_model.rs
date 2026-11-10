//! Bounded, occurrence-specific rewriting of existing image XObjects.
use crate::{EngineError, EngineResult, PreparedPdfImage};
use lopdf::{
    content::{Content, Operation},
    dictionary, Dictionary, Document, Object, ObjectId, Stream,
};
use std::collections::{BTreeMap, HashSet};
#[path = "existing_image_crop.rs"]
mod crop;
pub use crop::ImageCrop;
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImageFitMode {
    Fit,
    Fill,
    Stretch,
}
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageTransform {
    pub delta_x: f64,
    pub delta_y: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    pub rotation_degrees: f64,
}
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ExistingImageEdit {
    Transform { transform: ImageTransform },
    Adjust { transform: ImageTransform, crop: ImageCrop },
    Remove,
}
impl ImageTransform {
    pub(crate) fn validate(&self) -> EngineResult<()> {
        if [
            self.delta_x,
            self.delta_y,
            self.scale_x,
            self.scale_y,
            self.rotation_degrees,
        ]
        .iter()
        .any(|v| !v.is_finite())
            || self.delta_x.abs() > 1e6
            || self.delta_y.abs() > 1e6
            || !(0.001..=1000.).contains(&self.scale_x)
            || !(0.001..=1000.).contains(&self.scale_y)
            || self.rotation_degrees.abs() > 360.
        {
            return Err(bad("invalid image position, scale or angle"));
        }
        Ok(())
    }
    // Translation is already converted to raw displayed units by the facade.
    pub(crate) fn matrix(&self, m: [f64; 6], rotation: u16) -> EngineResult<[f64; 6]> {
        if self.delta_x == 0.
            && self.delta_y == 0.
            && self.scale_x == 1.
            && self.scale_y == 1.
            && self.rotation_degrees == 0.
        {
            return Ok(m);
        }
        let (dx, dy) = match rotation {
            0 => (self.delta_x, -self.delta_y),
            90 => (self.delta_y, self.delta_x),
            180 => (-self.delta_x, self.delta_y),
            270 => (-self.delta_y, -self.delta_x),
            _ => return Err(bad("invalid page rotation")),
        };
        let (s, c) = (-self.rotation_degrees).to_radians().sin_cos();
        let a = (c * m[0] - s * m[1]) * self.scale_x;
        let b = (s * m[0] + c * m[1]) * self.scale_x;
        let cc = (c * m[2] - s * m[3]) * self.scale_y;
        let d = (s * m[2] + c * m[3]) * self.scale_y;
        checked([
            a,
            b,
            cc,
            d,
            m[4] + (m[0] + m[2] - a - cc) / 2. + dx,
            m[5] + (m[1] + m[3] - b - d) / 2. + dy,
        ])
    }
}
#[derive(Debug, Clone)]
pub(crate) struct Occurrence {
    pub crop: Option<ImageCrop>,
    pub path: Vec<usize>,
    pub matrix: [f64; 6],
    pub corners: [[f64; 2]; 4],
    pub width: u32,
    pub height: u32,
}
const LIMIT: usize = 16 * 1024 * 1024;
const IDENTITY: [f64; 6] = [1., 0., 0., 1., 0., 0.];
struct Node {
    ops: Vec<Operation>,
    resources: Dictionary,
    template: Option<Dictionary>,
    children: BTreeMap<usize, Node>,
}
pub(crate) struct Model {
    pdf: Document,
    page: ObjectId,
    root: Node,
    images: Vec<Occurrence>,
}
fn bad(reason: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidRequest(format!("This image cannot be safely replaced: {reason}"))
}
fn resolve<'a>(pdf: &'a Document, mut value: &'a Object) -> EngineResult<&'a Object> {
    for _ in 0..64 {
        if let Object::Reference(id) = value {
            value = pdf.objects.get(id).ok_or_else(|| bad("missing resource"))?;
        } else {
            return Ok(value);
        }
    }
    Err(bad("cyclic or excessively nested resource"))
}
fn dict<'a>(pdf: &'a Document, value: &'a Object) -> EngineResult<&'a Dictionary> {
    resolve(pdf, value)?.as_dict().map_err(bad)
}
fn number(value: &Object) -> EngineResult<f64> {
    match value {
        Object::Integer(v) => Ok(*v as f64),
        Object::Real(v) => Ok(*v as f64),
        _ => Err(bad("invalid coordinate")),
    }
}
fn numbers<const N: usize>(values: &[Object]) -> EngineResult<[f64; N]> {
    if values.len() != N {
        return Err(bad("invalid matrix or rectangle"));
    }
    let mut result = [0.; N];
    for (i, v) in values.iter().enumerate() {
        result[i] = number(v)?;
        if !result[i].is_finite() || result[i].abs() > 1e8 {
            return Err(bad("coordinate exceeds limits"));
        }
    }
    Ok(result)
}
fn checked(m: [f64; 6]) -> EngineResult<[f64; 6]> {
    let x = m[0].hypot(m[1]);
    let y = m[2].hypot(m[3]);
    if m.iter().any(|v| !v.is_finite() || v.abs() > 1e8)
        || x < 1e-8
        || y < 1e-8
        || (m[0] * m[3] - m[1] * m[2]).abs() < x * y * 1e-10
    {
        return Err(bad("degenerate or oversized image transform"));
    }
    Ok(m)
}
fn compose(a: [f64; 6], b: [f64; 6]) -> EngineResult<[f64; 6]> {
    checked([
        a[0] * b[0] + a[2] * b[1],
        a[1] * b[0] + a[3] * b[1],
        a[0] * b[2] + a[2] * b[3],
        a[1] * b[2] + a[3] * b[3],
        a[0] * b[4] + a[2] * b[5] + a[4],
        a[1] * b[4] + a[3] * b[5] + a[5],
    ])
}
fn inverse(m: [f64; 6]) -> EngineResult<[f64; 6]> {
    checked(m)?;
    let det = m[0] * m[3] - m[1] * m[2];
    checked([
        m[3] / det,
        -m[1] / det,
        -m[2] / det,
        m[0] / det,
        (m[2] * m[5] - m[3] * m[4]) / det,
        (m[1] * m[4] - m[0] * m[5]) / det,
    ])
}
fn corners(m: [f64; 6]) -> [[f64; 2]; 4] {
    [
        [m[4], m[5]],
        [m[0] + m[4], m[1] + m[5]],
        [m[0] + m[2] + m[4], m[1] + m[3] + m[5]],
        [m[2] + m[4], m[3] + m[5]],
    ]
}
fn named(pdf: &Document, d: &Dictionary, key: &[u8], name: &[u8]) -> EngineResult<bool> {
    match d.get(key) {
        Ok(v) => Ok(resolve(pdf, v)?.as_name().map_err(bad)? == name),
        Err(_) => Ok(false),
    }
}
fn resource<'a>(
    pdf: &'a Document,
    resources: &'a Dictionary,
    name: &[u8],
) -> EngineResult<&'a Object> {
    dict(pdf, resources.get(b"XObject").map_err(bad)?)?
        .get(name)
        .map_err(bad)
}
fn decode(data: &[u8]) -> EngineResult<Vec<Operation>> {
    Content::decode_strict(data)
        .map(|v| v.operations)
        .map_err(bad)
}
fn inherited_resources(pdf: &Document, page: ObjectId) -> EngineResult<&Dictionary> {
    let mut id = page;
    let mut seen = HashSet::new();
    for _ in 0..64 {
        if !seen.insert(id) {
            return Err(bad("cyclic page tree"));
        }
        let d = pdf.get_dictionary(id).map_err(bad)?;
        if let Ok(v) = d.get(b"Resources") {
            return dict(pdf, v);
        }
        id = d
            .get(b"Parent")
            .and_then(Object::as_reference)
            .map_err(bad)?;
    }
    Err(bad("page resources exceed nesting limit"))
}
fn safe_scope(pdf: &Document, resources: &Dictionary) -> EngineResult<()> {
    if let Ok(v) = resources.get(b"Pattern") {
        if !dict(pdf, v)?.is_empty() {
            return Err(bad("pattern resource inheritance is not supported yet"));
        }
    }
    if let Ok(v) = resources.get(b"Font") {
        for (_, font) in dict(pdf, v)?.iter() {
            if named(pdf, dict(pdf, font)?, b"Subtype", b"Type3")? {
                return Err(bad("Type3 resource inheritance is not supported yet"));
            }
        }
    }
    if let Ok(v) = resources.get(b"ExtGState") {
        for (_, state) in dict(pdf, v)?.iter() {
            if let Ok(mask) = dict(pdf, state)?.get(b"SMask") {
                if resolve(pdf, mask)?.as_name().ok() != Some(b"None") {
                    return Err(bad(
                        "graphics-state soft-mask resources are not supported yet",
                    ));
                }
            }
        }
    }
    Ok(())
}
struct Reader<'a> {
    pdf: &'a Document,
    bytes: usize,
    ops: usize,
    pixels: u64,
    seen_images: HashSet<usize>,
    active_images: HashSet<usize>,
    active: HashSet<usize>,
    copies: crate::form_graph::CopyBudget,
    images: Vec<Occurrence>,
    image_visits: usize,
}
impl Reader<'_> {
    fn image(&mut self, value: &Object, depth: usize) -> EngineResult<(u32, u32)> {
        if depth > 12 {
            return Err(bad("image mask nesting exceeds limits"));
        }
        let object = resolve(self.pdf, value)?;
        let stream = object.as_stream().map_err(bad)?;
        if !named(self.pdf, &stream.dict, b"Subtype", b"Image")? {
            return Err(bad("image mask is not an image"));
        }
        let width = resolve(self.pdf, stream.dict.get(b"Width").map_err(bad)?)?
            .as_i64()
            .map_err(bad)?;
        let height = resolve(self.pdf, stream.dict.get(b"Height").map_err(bad)?)?
            .as_i64()
            .map_err(bad)?;
        if !(1..=32768).contains(&width)
            || !(1..=32768).contains(&height)
            || width * height > 32_000_000
        {
            return Err(bad(
                "image exceeds 32768 pixels per axis or 32 million pixels",
            ));
        }
        let key = object as *const Object as usize;
        if self.seen_images.contains(&key) {
            return Ok((width as u32, height as u32));
        }
        if !self.active_images.insert(key) {
            return Err(bad("cyclic image mask"));
        }
        self.pixels = self
            .pixels
            .checked_add((width * height) as u64)
            .ok_or_else(|| bad("image pixel budget overflow"))?;
        if self.pixels > 80_000_000 {
            return Err(bad("page images exceed 80 million pixels"));
        }
        for name in [b"SMask".as_slice(), b"Mask"] {
            if let Ok(v) = stream.dict.get(name) {
                let v = resolve(self.pdf, v)?;
                if v.as_stream().is_ok() {
                    self.image(v, depth + 1)?;
                } else if name == b"SMask" && v.as_name().ok() != Some(b"None") {
                    return Err(bad("invalid image soft mask"));
                }
            }
        }
        self.active_images.remove(&key);
        self.seen_images.insert(key);
        Ok((width as u32, height as u32))
    }
    fn node(
        &mut self,
        data: Vec<u8>,
        resources: &Dictionary,
        template: Option<&Dictionary>,
        mut matrix: [f64; 6],
        path: &mut Vec<usize>,
        depth: usize,
    ) -> EngineResult<Node> {
        if depth > 12 {
            return Err(bad("image form nesting exceeds 12 levels"));
        }
        self.bytes = self
            .bytes
            .checked_add(data.len())
            .filter(|n| *n <= LIMIT)
            .ok_or_else(|| bad("decoded image model exceeds 16 MiB"))?;
        safe_scope(self.pdf, resources)?;
        self.copies.dictionary(resources)?;
        let resources = resources.clone();
        let template = template
            .map(|d| -> EngineResult<Dictionary> {
                self.copies.dictionary(d)?;
                Ok(d.clone())
            })
            .transpose()?;
        let ops = decode(&data)?;
        self.ops = self
            .ops
            .checked_add(ops.len())
            .filter(|n| *n <= 100_000)
            .ok_or_else(|| bad("image model exceeds 100000 operations"))?;
        let mut stack = Vec::new();
        let mut children = BTreeMap::new();
        let mut text = false;
        let mut marked = 0usize;
        for (i, op) in ops.iter().enumerate() {
            match op.operator.as_str() {
                "q" => {
                    if !op.operands.is_empty() || stack.len() >= 256 {
                        return Err(bad("graphics stack exceeds limits"));
                    }
                    stack.push(matrix);
                }
                "Q" => {
                    if !op.operands.is_empty() {
                        return Err(bad("invalid graphics restore"));
                    }
                    matrix = stack
                        .pop()
                        .ok_or_else(|| bad("unbalanced graphics restore"))?;
                }
                "cm" => matrix = compose(matrix, numbers(&op.operands)?)?,
                "BT" if !text => text = true,
                "ET" if text => text = false,
                "BT" | "ET" => return Err(bad("unbalanced text state")),
                "BMC" | "BDC" => {
                    marked += 1;
                    if marked > 256 {
                        return Err(bad("marked-content nesting exceeds limits"));
                    }
                }
                "EMC" => {
                    marked = marked
                        .checked_sub(1)
                        .ok_or_else(|| bad("unbalanced marked content"))?
                }
                "Do" => {
                    if text || op.operands.len() != 1 {
                        return Err(bad("invalid image invocation"));
                    }
                    let name = op.operands[0].as_name().map_err(bad)?;
                    let value = resource(self.pdf, &resources, name)?;
                    let object = resolve(self.pdf, value)?;
                    let stream = object.as_stream().map_err(bad)?;
                    path.push(i);
                    if named(self.pdf, &stream.dict, b"Subtype", b"Image")? {
                        let (width, height) = self.image(value, 0)?;
                        self.image_visits += 1;
                        if self.image_visits > 5000 {
                            return Err(bad("page exceeds 5000 image occurrences"));
                        }
                        self.images.push(Occurrence {
                            crop: None,
                            path: path.clone(),
                            matrix,
                            corners: corners(matrix),
                            width,
                            height,
                        });
                    } else if named(self.pdf, &stream.dict, b"Subtype", b"Form")? {
                        if stream.dict.has(b"Ref") || stream.dict.has(b"OPI") {
                            return Err(bad("external form resources are unsupported"));
                        }
                        let key = object as *const Object as usize;
                        if !self.active.insert(key) {
                            return Err(bad("cyclic form invocation"));
                        }
                        let child_resources = dict(
                            self.pdf,
                            stream
                                .dict
                                .get(b"Resources")
                                .map_err(|_| bad("inherited form resources are unsupported"))?,
                        )?;
                        let form_matrix = match stream.dict.get(b"Matrix") {
                            Ok(v) => numbers(resolve(self.pdf, v)?.as_array().map_err(bad)?)?,
                            Err(_) => IDENTITY,
                        };
                        let bbox = numbers::<4>(
                            resolve(self.pdf, stream.dict.get(b"BBox").map_err(bad)?)?
                                .as_array()
                                .map_err(bad)?,
                        )?;
                        if bbox[0] >= bbox[2] || bbox[1] >= bbox[3] {
                            return Err(bad("invalid form bounds"));
                        }
                        let before = self.images.len();
                        let child_data = stream
                            .get_plain_content_with_limit(LIMIT.saturating_sub(self.bytes))
                            .map_err(bad)?;
                        let child = self.node(
                            child_data,
                            child_resources,
                            Some(&stream.dict),
                            compose(matrix, form_matrix)?,
                            path,
                            depth + 1,
                        )?;
                        if let Some((width, height)) = frame_dimensions(self.pdf, stream, &child)? {
                            self.images.truncate(before);
                            self.images.push(Occurrence {
                                crop: None,
                                path: path.clone(),
                                matrix,
                                corners: corners(matrix),
                                width,
                                height,
                            });
                        }
                        if let Some(crop) = crop::recognized(
                            self.pdf,
                            stream,
                            &child,
                            &self.images[before..],
                            path,
                            matrix,
                        )? {
                            let image = self.images[before].clone();
                            self.images.truncate(before);
                            self.images.push(Occurrence {
                                crop: Some(crop),
                                path: path.clone(),
                                matrix,
                                corners: corners(matrix),
                                width: image.width,
                                height: image.height,
                            });
                        }
                        children.insert(i, child);
                        self.active.remove(&key);
                    } else {
                        return Err(bad("unsupported XObject type"));
                    }
                    path.pop();
                }
                "BI" => return Err(bad("inline image editing is unsupported")),
                "w" | "J" | "j" | "M" | "d" | "ri" | "i" | "gs" | "m" | "l" | "c" | "v" | "y"
                | "h" | "re" | "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" | "n"
                | "W" | "W*" | "Tc" | "Tw" | "Tz" | "TL" | "Tf" | "Tr" | "Ts" | "Td" | "TD"
                | "Tm" | "T*" | "Tj" | "TJ" | "'" | "\"" | "CS" | "cs" | "SC" | "SCN" | "sc"
                | "scn" | "G" | "g" | "RG" | "rg" | "K" | "k" | "sh" | "MP" | "DP" => {}
                _ => return Err(bad("unsupported content operator")),
            }
        }
        if !stack.is_empty() || text || marked != 0 {
            return Err(bad("unbalanced page or form state"));
        }
        Ok(Node {
            ops,
            resources,
            template,
            children,
        })
    }
}
// A frame is recognized by its complete semantics, never by a producer marker.
fn unit_form(pdf: &Document, stream: &Stream) -> EngineResult<bool> {
    if stream.dict.iter().any(|(key, _)| {
        !matches!(
            key.as_slice(),
            b"Type" | b"Subtype" | b"BBox" | b"Resources" | b"Length" | b"Filter" | b"DecodeParms"
        )
    }) {
        return Ok(false);
    }
    if !named(pdf, &stream.dict, b"Subtype", b"Form")? {
        return Ok(false);
    }
    let bbox = numbers::<4>(
        resolve(pdf, stream.dict.get(b"BBox").map_err(bad)?)?
            .as_array()
            .map_err(bad)?,
    )?;
    let resources = dict(pdf, stream.dict.get(b"Resources").map_err(bad)?)?;
    Ok(bbox == [0., 0., 1., 1.]
        && resources.len() == 1
        && resources.has(b"XObject")
        && dict(pdf, resources.get(b"XObject").map_err(bad)?)?.len() == 1)
}
fn empty(op: &Operation, name: &str) -> bool {
    op.operator == name && op.operands.is_empty()
}
fn frame_dimensions(
    pdf: &Document,
    stream: &Stream,
    node: &Node,
) -> EngineResult<Option<(u32, u32)>> {
    if !unit_form(pdf, stream)? {
        return Ok(None);
    }
    let ops = &node.ops;
    // A unit form containing exactly one full-frame image is the image's
    // container, not an independent viewport. Move that invocation as a whole
    // so its BBox travels with the pixels. Keep arbitrary form clips unchanged.
    let simple = match ops.as_slice() {
        [draw] if draw.operator == "Do" => Some((draw, IDENTITY)),
        [save,draw,restore] if empty(save,"q") && draw.operator=="Do" && empty(restore,"Q") => Some((draw,IDENTITY)),
        [matrix,draw] if matrix.operator=="cm" && draw.operator=="Do" => Some((draw,numbers::<6>(&matrix.operands)?)),
        [save,matrix,draw,restore] if empty(save,"q") && matrix.operator=="cm" && draw.operator=="Do" && empty(restore,"Q") => Some((draw,numbers::<6>(&matrix.operands)?)),
        _=>None,
    };
    if let Some((draw,matrix))=simple {
        if draw.operands.len()==1 && corners(matrix).iter().flatten().all(|value|*value==0. || *value==1.)
            && (matrix[0]*matrix[3]-matrix[1]*matrix[2]).abs()==1.
        {
            let image=resolve(pdf,resource(pdf,&node.resources,draw.operands[0].as_name().map_err(bad)?)?)?.as_stream().map_err(bad)?;
            if named(pdf,&image.dict,b"Subtype",b"Image")? {
                let width=resolve(pdf,image.dict.get(b"Width").map_err(bad)?)?.as_i64().map_err(bad)? as u32;
                let height=resolve(pdf,image.dict.get(b"Height").map_err(bad)?)?.as_i64().map_err(bad)? as u32;
                return Ok(Some(if matrix[0]==0. {(height,width)}else{(width,height)}));
            }
        }
    }
    if ops.len()!=7 {return Ok(None);}
    if !empty(&ops[0], "q")
        || ops[1].operator != "re"
        || numbers::<4>(&ops[1].operands)? != [0., 0., 1., 1.]
        || !empty(&ops[2], "W")
        || !empty(&ops[3], "n")
        || ops[4].operator != "cm"
        || ops[5].operator != "Do"
        || !empty(&ops[6], "Q")
    {
        return Ok(None);
    }
    let m = numbers::<6>(&ops[4].operands)?;
    if m[0] <= 0.
        || m[3] <= 0.
        || m[1] != 0.
        || m[2] != 0.
        || (m[0] != 1. && m[3] != 1.)
        || (m[4] - (1. - m[0]) / 2.).abs() > 1e-6
        || (m[5] - (1. - m[3]) / 2.).abs() > 1e-6
    {
        return Ok(None);
    }
    let prepared = resolve(
        pdf,
        resource(
            pdf,
            &node.resources,
            ops[5].operands[0].as_name().map_err(bad)?,
        )?,
    )?
    .as_stream()
    .map_err(bad)?;
    if !unit_form(pdf, prepared)? {
        return Ok(None);
    }
    let Some(child) = node.children.get(&5) else {
        return Ok(None);
    };
    let ops = &child.ops;
    if ops.len() != 4
        || !empty(&ops[0], "q")
        || ops[1].operator != "cm"
        || ops[2].operator != "Do"
        || !empty(&ops[3], "Q")
    {
        return Ok(None);
    }
    let m = numbers::<6>(&ops[1].operands)?;
    let quad = corners(m);
    if quad.iter().any(|p| p.iter().any(|v| *v != 0. && *v != 1.))
        || (m[0] * m[3] - m[1] * m[2]).abs() != 1.
    {
        return Ok(None);
    }
    let image = resolve(
        pdf,
        resource(
            pdf,
            &child.resources,
            ops[2].operands[0].as_name().map_err(bad)?,
        )?,
    )?
    .as_stream()
    .map_err(bad)?;
    if !named(pdf, &image.dict, b"Subtype", b"Image")? {
        return Ok(None);
    }
    let w = resolve(pdf, image.dict.get(b"Width").map_err(bad)?)?
        .as_i64()
        .map_err(bad)? as u32;
    let h = resolve(pdf, image.dict.get(b"Height").map_err(bad)?)?
        .as_i64()
        .map_err(bad)? as u32;
    Ok(Some(if m[0] == 0. { (h, w) } else { (w, h) }))
}
fn fitting(matrix: [f64; 6], ratio: f64, mode: ImageFitMode) -> EngineResult<[f64; 6]> {
    if !ratio.is_finite() || ratio <= 0. {
        return Err(bad("invalid replacement aspect ratio"));
    }
    let r = ratio / (matrix[0].hypot(matrix[1]) / matrix[2].hypot(matrix[3]));
    let (x, y) = match mode {
        ImageFitMode::Stretch => (1., 1.),
        ImageFitMode::Fit if r > 1. => (1., 1. / r),
        ImageFitMode::Fit => (r, 1.),
        ImageFitMode::Fill if r > 1. => (r, 1.),
        ImageFitMode::Fill => (1., 1. / r),
    };
    checked([x, 0., 0., y, (1. - x) / 2., (1. - y) / 2.])
}
fn new_frame(
    pdf: &mut Document,
    prepared: &PreparedPdfImage,
    matrix: [f64; 6],
) -> EngineResult<ObjectId> {
    if crate::image_persistence::resource_bytes(prepared.form()) > 128 * 1024 * 1024 {
        return Err(bad("prepared replacement exceeds 128 MiB"));
    }
    let id = prepared.install_copy(pdf);
    let ops = vec![
        Operation::new("q", vec![]),
        Operation::new("re", vec![0.into(), 0.into(), 1.into(), 1.into()]),
        Operation::new("W", vec![]),
        Operation::new("n", vec![]),
        Operation::new(
            "cm",
            matrix.into_iter().map(|v| Object::Real(v as f32)).collect(),
        ),
        Operation::new("Do", vec![Object::Name(b"Picture".to_vec())]),
        Operation::new("Q", vec![]),
    ];
    Ok(pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),1.into(),1.into()],"Resources"=>dictionary!{"XObject"=>dictionary!{"Picture"=>id}}},Content{operations:ops}.encode().map_err(bad)?)))
}
#[derive(Clone, Copy)]
enum Change {
    Keep,
    Replace(ObjectId),
    Transform([f64; 6]),
    Adjust {
        relative: Option<[f64; 6]>,
        crop: ImageCrop,
        existing: bool,
    },
    Remove,
}
fn clone_chain(
    pdf: &mut Document,
    node: &Node,
    path: &[usize],
    change: Change,
    copies: &mut crate::form_graph::CopyBudget,
) -> EngineResult<(Vec<u8>, Dictionary)> {
    let (&index, tail) = path.split_first().ok_or_else(|| bad("empty image path"))?;
    let op = node
        .ops
        .get(index)
        .filter(|op| op.operator == "Do" && op.operands.len() == 1)
        .ok_or_else(|| bad("stale image path"))?;
    let old = op.operands[0].as_name().map_err(bad)?;
    let local = if tail.is_empty() {
        change
    } else {
        let child = node
            .children
            .get(&index)
            .ok_or_else(|| bad("stale form path"))?;
        let (bytes, resources) = clone_chain(pdf, child, tail, change, copies)?;
        let template = child
            .template
            .as_ref()
            .ok_or_else(|| bad("missing form template"))?;
        copies.dictionary(template)?;
        let mut stream = Stream::new(template.clone(), vec![]);
        stream.set_plain_content(bytes);
        stream.dict.set("Resources", resources);
        Change::Replace(pdf.add_object(stream))
    };
    copies.dictionary(&node.resources)?;
    let mut resources = node.resources.clone();
    let mut ops = node.ops.clone();
    let replacement = match local {
        Change::Replace(id) => Some(id),
        Change::Adjust { crop, existing, .. } => {
            Some(crop::reframe(pdf, node, index, crop, existing, copies)?)
        }
        _ => None,
    };
    if let Some(id) = replacement {
        let table = dict(pdf, resources.get(b"XObject").map_err(bad)?)?;
        copies.dictionary(table)?;
        let mut table = table.clone();
        let shared = node.ops.iter().enumerate().any(|(i, other)| {
            i != index
                && other.operator == "Do"
                && other.operands.first().and_then(|v| v.as_name().ok()) == Some(old)
        });
        let alias = if shared {
            let mut n = 0;
            loop {
                let name = format!("FolioImage{n}").into_bytes();
                if !table.has(&name) {
                    break name;
                }
                n += 1;
            }
        } else {
            old.to_vec()
        };
        table.set(alias.clone(), id);
        resources.set("XObject", table);
        ops[index].operands = vec![Object::Name(alias)];
    }
    if matches!(
        local,
        Change::Transform(_)
            | Change::Remove
            | Change::Adjust {
                relative: Some(_),
                ..
            }
    ) {
        // Only this exact isolated group can be folded. No arbitrary preceding cm
        // is rewritten: it may position later content or establish a clip.
        let grouped = index >= 2
            && index + 1 < ops.len()
            && empty(&ops[index - 2], "q")
            && ops[index - 1].operator == "cm"
            && empty(&ops[index + 1], "Q");
        match local {
            Change::Transform(relative)
            | Change::Adjust {
                relative: Some(relative),
                ..
            } => {
                let matrix = if grouped {
                    compose(numbers::<6>(&ops[index - 1].operands)?, relative)?
                } else {
                    relative
                };
                let cm = Operation::new(
                    "cm",
                    matrix.into_iter().map(|v| Object::Real(v as f32)).collect(),
                );
                if grouped {
                    ops[index - 1] = cm;
                } else {
                    ops.splice(
                        index..=index,
                        [
                            Operation::new("q", vec![]),
                            cm,
                            ops[index].clone(),
                            Operation::new("Q", vec![]),
                        ],
                    );
                }
            }
            Change::Remove => {
                if grouped {
                    ops.drain(index - 2..=index + 1);
                } else {
                    ops.remove(index);
                }
            }
            _ => unreachable!(),
        }
    }
    Ok((
        Content { operations: ops }.encode().map_err(bad)?,
        resources,
    ))
}
impl Model {
    pub fn read(bytes: &[u8], page_index: usize) -> EngineResult<Self> {
        crate::form_graph::preflight(bytes)?;
        let pdf = crate::pdf_output::load(bytes)?;
        let page = *pdf
            .get_pages()
            .values()
            .nth(page_index)
            .ok_or_else(|| bad("page is missing"))?;
        let mut reader = Reader {
            pdf: &pdf,
            bytes: 0,
            ops: 0,
            pixels: 0,
            seen_images: HashSet::new(),
            active_images: HashSet::new(),
            active: HashSet::new(),
            copies: Default::default(),
            images: vec![],
            image_visits: 0,
        };
        let data = pdf.get_page_content_with_limit(page, LIMIT).map_err(bad)?;
        let root = reader.node(
            data,
            inherited_resources(&pdf, page)?,
            None,
            IDENTITY,
            &mut vec![],
            0,
        )?;
        let images = reader.images;
        Ok(Self {
            pdf,
            page,
            root,
            images,
        })
    }
    pub fn images(&self) -> &[Occurrence] {
        &self.images
    }
    pub fn rewrite(
        &mut self,
        path: &[usize],
        replacement: Option<(&PreparedPdfImage, ImageFitMode, f64)>,
    ) -> EngineResult<Vec<u8>> {
        let selected = self
            .images
            .iter()
            .find(|v| v.path == path)
            .ok_or_else(|| bad("stale image path"))?;
        let change = if let Some((prepared, mode, ratio)) = replacement {
            let frame = new_frame(
                &mut self.pdf,
                prepared,
                fitting(selected.matrix, ratio, mode)?,
            )?;
            let frame = if let Some(crop) = selected.crop {
                crop::new_frame(
                    &mut self.pdf,
                    &Dictionary::new(),
                    &Object::Reference(frame),
                    crop,
                    &mut Default::default(),
                )?
            } else {
                frame
            };
            Change::Replace(frame)
        } else {
            Change::Keep
        };
        self.write(path, change)
    }
    pub fn adjust(
        &mut self,
        path: &[usize],
        target: [f64; 6],
        crop: ImageCrop,
    ) -> EngineResult<Vec<u8>> {
        crop.validate()?;
        checked(target)?;
        let selected = self
            .images
            .iter()
            .find(|v| v.path == path)
            .ok_or_else(|| bad("stale image path"))?;
        let relative = if target == selected.matrix {
            None
        } else {
            Some(compose(inverse(selected.matrix)?, target)?)
        };
        if (selected.crop == Some(crop) && crop != ImageCrop::FULL) || (selected.crop.is_none() && crop == ImageCrop::FULL) {
            return self.write(
                path,
                relative.map(Change::Transform).unwrap_or(Change::Keep),
            );
        }
        self.write(
            path,
            Change::Adjust {
                relative,
                crop,
                existing: selected.crop.is_some(),
            },
        )
    }
    pub fn image_fingerprint(&self, path: &[usize]) -> EngineResult<[u8; 32]> {
        let image = self
            .images
            .iter()
            .find(|v| v.path == path)
            .ok_or_else(|| bad("stale image path"))?;
        let mut node = &self.root;
        for index in &path[..path.len() - 1] {
            node = node
                .children
                .get(index)
                .ok_or_else(|| bad("missing image scope"))?;
        }
        let mut index = *path.last().unwrap();
        if image.crop.is_some() {
            node = node
                .children
                .get(&index)
                .ok_or_else(|| bad("missing crop scope"))?;
            index = 4;
        }
        let name = node.ops[index].operands[0].as_name().map_err(bad)?;
        crop::fingerprint(
            &self.pdf,
            resource(&self.pdf, &node.resources, name)?,
            &node.resources,
        )
    }
    pub fn change(&mut self, path: &[usize], matrix: Option<[f64; 6]>) -> EngineResult<Vec<u8>> {
        let original = self
            .images
            .iter()
            .find(|v| v.path == path)
            .ok_or_else(|| bad("stale image path"))?
            .matrix;
        let change = match matrix {
            Some(target) if target == original => Change::Keep,
            Some(target) => Change::Transform(compose(inverse(original)?, checked(target)?)?),
            None => Change::Remove,
        };
        self.write(path, change)
    }
    fn write(&mut self, path: &[usize], change: Change) -> EngineResult<Vec<u8>> {
        let (bytes, resources) = clone_chain(
            &mut self.pdf,
            &self.root,
            path,
            change,
            &mut Default::default(),
        )?;
        let contents = self.pdf.add_object(Stream::new(dictionary! {}, bytes));
        let page = self.pdf.get_dictionary_mut(self.page).map_err(bad)?;
        page.set("Contents", contents);
        page.set("Resources", resources);
        crate::page_resources::prune_unreachable(&mut self.pdf)?;
        crate::pdf_output::save(&mut self.pdf)
    }
}
#[cfg(test)]
#[path = "existing_image_model_tests.rs"]
mod tests;
