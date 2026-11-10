//! Ordinary PDF clips with a narrowly proven logical image boundary.
use super::*;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageCrop {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}
impl ImageCrop {
    pub const FULL: Self = Self {
        left: 0.,
        top: 0.,
        right: 1.,
        bottom: 1.,
    };
    pub(crate) fn validate(self) -> EngineResult<()> {
        let values = [self.left, self.top, self.right, self.bottom];
        if values
            .iter()
            .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
            || self.right - self.left < 0.001 - 8. * f64::EPSILON
            || self.bottom - self.top < 0.001 - 8. * f64::EPSILON
        {
            return Err(bad(
                "crop must retain at least 0.1% of each image axis within its full frame",
            ));
        }
        Ok(())
    }
    pub(crate) fn rect(self) -> EngineResult<[f64; 4]> {
        self.validate()?;
        let axis = |start: f64, extent: f64| {
            let mut x = start as f32;
            let mut width = extent as f32;
            // Keep a requested minimum valid after the unit BBox clips f32
            // endpoints. At the far edge, round the origin inward by one ULP.
            if 1. - (x as f64) < 0.001 {
                let edge = 0.999f32;
                x = f32::from_bits(edge.to_bits() - 1);
            }
            if (width as f64) < 0.001 {
                width = 0.001f32;
            }
            [x as f64, width as f64]
        };
        let [x, width] = axis(self.left, self.right - self.left);
        let [y, height] = axis(1. - self.bottom, self.bottom - self.top);
        Ok([x, y, width, height])
    }

    pub(crate) fn serialized(self) -> EngineResult<Self> {
        Self::from_rect(self.rect()?)
    }
    fn from_rect(r: [f64; 4]) -> EngineResult<Self> {
        // A rectangle's endpoints contain at most two rounded PDF operands.
        let ulp = |v: f64| {
            let f = (v.abs() as f32).max(f32::MIN_POSITIVE);
            (f32::from_bits(f.to_bits() + 1) - f) as f64
        };
        let epsilon = ulp(r[0]) + ulp(r[1]) + ulp(r[2]) + ulp(r[3]) + 8. * f64::EPSILON;
        if r.iter().any(|v| !v.is_finite())
            || r[0] < -epsilon
            || r[1] < -epsilon
            || r[2] < 0.001 - ulp(r[2])
            || r[3] < 0.001 - ulp(r[3])
            || r[0] + r[2] > 1. + epsilon
            || r[1] + r[3] > 1. + epsilon
        {
            return Err(bad("invalid serialized image crop"));
        }
        // Values already inside the unit BBox are actual clip geometry;
        // never snap an inside edge merely because it is near zero or one.
        let clean = |v: f64| if v <= 0. { 0. } else { v.min(1.) };
        let crop = Self {
            left: clean(r[0]),
            top: clean(1. - r[1] - r[3]),
            right: clean(r[0] + r[2]),
            bottom: clean(1. - r[1]),
        };
        // Public metadata must itself be admissible after the BBox clips any
        // operand overshoot. Otherwise retain ordinary traversal and its clip.
        crop.validate()?;
        Ok(crop)
    }
    pub(crate) fn corners(self, m: [f64; 6]) -> [[f64; 2]; 4] {
        [
            [self.left, 1. - self.bottom],
            [self.right, 1. - self.bottom],
            [self.right, 1. - self.top],
            [self.left, 1. - self.top],
        ]
        .map(|[x, y]| [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]])
    }
}

pub(super) fn recognized(
    pdf: &Document,
    stream: &Stream,
    node: &Node,
    images: &[Occurrence],
    path: &[usize],
    matrix: [f64; 6],
) -> EngineResult<Option<ImageCrop>> {
    if stream.dict.iter().any(|(key, _)| {
        !matches!(
            key.as_slice(),
            b"Type" | b"Subtype" | b"BBox" | b"Resources" | b"Length" | b"Filter" | b"DecodeParms"
        )
    }) || images.len() != 1
        || images[0].crop.is_some()
        || images[0].matrix != matrix
        || node.ops.len() != 6
    {
        return Ok(None);
    }
    let mut child_path = path.to_vec();
    child_path.push(4);
    if images[0].path != child_path {
        return Ok(None);
    }
    if numbers::<4>(
        resolve(pdf, stream.dict.get(b"BBox").map_err(bad)?)?
            .as_array()
            .map_err(bad)?,
    )? != [0., 0., 1., 1.]
    {
        return Ok(None);
    }
    let ops = &node.ops;
    if !empty(&ops[0], "q")
        || ops[1].operator != "re"
        || ops[1].operands.len() != 4
        || !empty(&ops[2], "W")
        || !empty(&ops[3], "n")
        || ops[4].operator != "Do"
        || ops[4].operands.len() != 1
        || !empty(&ops[5], "Q")
    {
        return Ok(None);
    }
    if dict(pdf, node.resources.get(b"XObject").map_err(bad)?)?.len() != 1 {
        return Ok(None);
    }
    let image = resolve(
        pdf,
        resource(
            pdf,
            &node.resources,
            ops[4].operands[0].as_name().map_err(bad)?,
        )?,
    )?
    .as_stream()
    .map_err(bad)?;
    if !named(pdf, &image.dict, b"Subtype", b"Image")? {
        let Some(child) = node.children.get(&4) else {
            return Ok(None);
        };
        if frame_dimensions(pdf, image, child)?.is_none() {
            return Ok(None);
        }
    }
    Ok(ImageCrop::from_rect(numbers(&ops[1].operands)?).ok())
}
fn clip_ops(crop: ImageCrop) -> EngineResult<Vec<Operation>> {
    Ok(vec![
        Operation::new("q", vec![]),
        Operation::new(
            "re",
            crop.rect()?
                .into_iter()
                .map(|v| Object::Real(v as f32))
                .collect(),
        ),
        Operation::new("W", vec![]),
        Operation::new("n", vec![]),
        Operation::new("Do", vec![Object::Name(b"Picture".to_vec())]),
        Operation::new("Q", vec![]),
    ])
}
pub(super) fn new_frame(
    pdf: &mut Document,
    scope: &Dictionary,
    value: &Object,
    crop: ImageCrop,
    copies: &mut crate::form_graph::CopyBudget,
) -> EngineResult<ObjectId> {
    // Charge before cloning even direct stream resources. Keep named/default
    // color spaces in the invoking scope; the only Do binding is isolated.
    copies.dictionary(scope)?;
    let mut resources = Dictionary::new();
    for (key, value) in scope.iter() {
        if key.as_slice() != b"XObject" {
            resources.set(key.clone(), value.clone());
        }
    }
    resources.set("XObject", dictionary! {"Picture"=>value.clone()});
    Ok(pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),1.into(),1.into()],"Resources"=>resources},Content{operations:clip_ops(crop)?}.encode().map_err(bad)?)))
}
pub(super) fn reframe(
    pdf: &mut Document,
    node: &Node,
    index: usize,
    crop: ImageCrop,
    existing: bool,
    copies: &mut crate::form_graph::CopyBudget,
) -> EngineResult<ObjectId> {
    if existing {
        let child = node
            .children
            .get(&index)
            .ok_or_else(|| bad("missing crop wrapper"))?;
        let template = child
            .template
            .as_ref()
            .ok_or_else(|| bad("missing crop dictionary"))?;
        copies.dictionary(template)?;
        // Keep the logical child boundary when unwrapping could promote an
        // enclosing clipping/fitting form on the next discovery pass.
        let fixed_parent_clip = node.template.is_some() && node.ops.iter().any(|op| matches!(op.operator.as_str(),"re"|"W"|"W*"));
        if crop == ImageCrop::FULL && !fixed_parent_clip {
            let name = child.ops[4].operands[0].as_name().map_err(bad)?;
            let value = resource(pdf, &child.resources, name)?;
            // A redundant full clip can change edge antialiasing in independent
            // readers. Remove it only when the retained image's effective color
            // context is identical at the invoking parent. Otherwise its local
            // resource scope remains necessary and must stay wrapped.
            if fingerprint(pdf,value,&child.resources)? == fingerprint(pdf,value,&node.resources)? {
                if let Object::Reference(id) = value { return Ok(*id); }
                let value=value.clone();
                return Ok(pdf.add_object(value));
            }
        }
        let mut stream = Stream::new(template.clone(), vec![]);
        let mut ops = child.ops.clone();
        ops[1] = clip_ops(crop)?.remove(1);
        stream.set_plain_content(Content { operations: ops }.encode().map_err(bad)?);
        return Ok(pdf.add_object(stream));
    }
    let name = node.ops[index].operands[0].as_name().map_err(bad)?;
    // Borrowed value must be copied before mutating the object table.
    copies.dictionary(dict(pdf, node.resources.get(b"XObject").map_err(bad)?)?)?;
    let value = resource(pdf, &node.resources, name)?.clone();
    new_frame(pdf, &node.resources, &value, crop, copies)
}

struct Fingerprint<'a> {
    pdf: &'a Document,
    memo: HashMap<ObjectId, [u8; 32]>,
    active: HashSet<ObjectId>,
    visits: usize,
    bytes: usize,
}
impl Fingerprint<'_> {
    fn charge(&mut self, bytes: usize) -> EngineResult<()> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .filter(|n| *n <= 512 * 1024 * 1024)
            .ok_or_else(|| bad("image appearance proof exceeds byte limit"))?;
        Ok(())
    }
    fn value(&mut self, value: &Object, depth: usize) -> EngineResult<[u8; 32]> {
        self.visits += 1;
        if depth > 64 || self.visits > 100_000 {
            return Err(bad("image appearance proof exceeds graph limit"));
        }
        if let Object::Reference(id) = value {
            if let Some(hash) = self.memo.get(id) {
                return Ok(*hash);
            }
            if !self.active.insert(*id) {
                return Err(bad("cyclic image appearance graph"));
            }
            let hash = self.value(
                self.pdf
                    .objects
                    .get(id)
                    .ok_or_else(|| bad("missing appearance resource"))?,
                depth + 1,
            )?;
            self.active.remove(id);
            self.memo.insert(*id, hash);
            return Ok(hash);
        }
        let mut hash = Sha256::new();
        match value {
            Object::Null => hash.update(b"null"),
            Object::Boolean(v) => {
                hash.update(b"bool");
                hash.update([u8::from(*v)]);
            }
            Object::Integer(v) => {
                hash.update(b"int");
                hash.update(v.to_le_bytes());
            }
            Object::Real(v) => {
                hash.update(b"real");
                hash.update(v.to_bits().to_le_bytes());
            }
            Object::Name(v) | Object::String(v, _) => {
                hash.update(if matches!(value, Object::Name(_)) {
                    b"name".as_slice()
                } else {
                    b"string"
                });
                self.charge(v.len())?;
                hash.update((v.len() as u64).to_le_bytes());
                hash.update(v);
            }
            Object::Array(v) => {
                hash.update(b"array");
                hash.update((v.len() as u64).to_le_bytes());
                for item in v {
                    hash.update(self.value(item, depth + 1)?);
                }
            }
            Object::Dictionary(v) => {
                hash.update(b"dict");
                self.dictionary(&mut hash, v, depth, false)?;
            }
            Object::Stream(v) => {
                hash.update(b"stream");
                self.dictionary(&mut hash, &v.dict, depth, true)?;
                self.charge(v.content.len())?;
                hash.update((v.content.len() as u64).to_le_bytes());
                hash.update(&v.content);
            }
            Object::Reference(_) => unreachable!(),
        }
        Ok(hash.finalize().into())
    }
    fn dictionary(
        &mut self,
        hash: &mut Sha256,
        d: &Dictionary,
        depth: usize,
        stream: bool,
    ) -> EngineResult<()> {
        if d.len() > 100_000usize.saturating_sub(self.visits) {
            return Err(bad("image appearance proof exceeds graph limit"));
        }
        let mut entries = d
            .iter()
            .filter(|(key, _)| !stream || key.as_slice() != b"Length")
            .collect::<Vec<_>>();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        hash.update((entries.len() as u64).to_le_bytes());
        for (key, value) in entries {
            self.charge(key.len())?;
            hash.update((key.len() as u64).to_le_bytes());
            hash.update(key);
            hash.update(self.value(value, depth + 1)?);
        }
        Ok(())
    }
}
pub(super) fn fingerprint(
    pdf: &Document,
    value: &Object,
    scope: &Dictionary,
) -> EngineResult<[u8; 32]> {
    let mut proof = Fingerprint {
        pdf,
        memo: HashMap::new(),
        active: HashSet::new(),
        visits: 0,
        bytes: 0,
    };
    let mut hash = Sha256::new();
    hash.update(proof.value(value, 0)?);
    if let Ok(colors) = scope.get(b"ColorSpace") {
        hash.update([1]);
        hash.update(proof.value(colors, 0)?);
    } else {
        hash.update([0]);
    }
    Ok(hash.finalize().into())
}
