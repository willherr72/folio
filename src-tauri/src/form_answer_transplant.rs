use super::*;
use crate::form_appearance::Copier;
use lopdf::{dictionary, Dictionary, Document, Stream};

#[path = "form_answer_prune.rs"]
mod prune;

pub(crate) struct FillAppearanceBudget<'a> {
    copy: Copier<'a>,
    source: &'a Document,
    native: &'a Document,
    normal_admission: NormalAppearanceAdmission<'a>,
    failed: bool,
}
impl<'a> FillAppearanceBudget<'a> {
    pub(crate) fn new(
        source: &'a Document,
        native: &'a Document,
        staging: &'a mut Document,
    ) -> EngineResult<Self> {
        Self::with_limit(source, native, staging, 64 * 1024 * 1024)
    }
    pub(crate) fn with_limit(
        source: &'a Document,
        native: &'a Document,
        staging: &'a mut Document,
        limit: usize,
    ) -> EngineResult<Self> {
        if !staging.objects.is_empty() {
            return Err(bad("appearance staging must start empty"));
        }
        staging.max_id = source.max_id;
        Ok(Self {
            copy: Copier::with_byte_limit(native, staging, limit),
            source,
            native,
            normal_admission: NormalAppearanceAdmission::new(native),
            failed: false,
        })
    }
    pub(crate) fn reserve_dictionary(&mut self, dict: &Dictionary) -> EngineResult<()> {
        self.copy.reserve_dictionary(dict)
    }
    pub(crate) fn reserve_object(&mut self, value: &Object) -> EngineResult<()> {
        self.copy.reserve_object(value)
    }
    pub(crate) fn reserve_bytes(&mut self, bytes: usize) -> EngineResult<()> {
        self.copy.reserve_bytes(bytes)
    }
    pub(crate) fn retain_local(&mut self, value: Object) -> EngineResult<lopdf::ObjectId> {
        self.copy.retain_local(value)
    }
    pub(crate) fn copy_native(&mut self, value: &Object) -> EngineResult<Object> {
        if self.failed {
            return Err(bad("an earlier appearance failed preservation"));
        }
        let result = self
            .normal_admission
            .admit(value)
            .and_then(|()| self.copy.copy(value));
        self.failed = result.is_err();
        result
    }
}
pub(crate) struct StagedAnswerAppearance {
    pub(crate) appearance: Stream,
}
pub(crate) fn transplant_answer(
    source: &Document,
    original: &Stream,
    native: &Document,
    generated: &Stream,
    kind: AnswerKind,
    budget: &mut FillAppearanceBudget<'_>,
) -> EngineResult<StagedAnswerAppearance> {
    transplant_answer_allow_empty(
        source, original, native, generated, kind, budget, false, false,
    )
}
pub(crate) fn transplant_answer_allow_empty(
    source: &Document,
    original: &Stream,
    native: &Document,
    generated: &Stream,
    kind: AnswerKind,
    budget: &mut FillAppearanceBudget<'_>,
    empty: bool,
    source_empty: bool,
) -> EngineResult<StagedAnswerAppearance> {
    transplant_answer_with_resources(
        source,
        original,
        native,
        generated,
        kind,
        budget,
        empty,
        source_empty,
        None,
    )
}
pub(crate) fn transplant_answer_with_resources(
    source: &Document,
    original: &Stream,
    native: &Document,
    generated: &Stream,
    kind: AnswerKind,
    budget: &mut FillAppearanceBudget<'_>,
    empty: bool,
    source_empty: bool,
    context: Option<&Dictionary>,
) -> EngineResult<StagedAnswerAppearance> {
    if budget.failed {
        return Err(bad("an earlier appearance failed preservation"));
    }
    let result = (|| {
        if !std::ptr::eq(source, budget.source) || !std::ptr::eq(native, budget.native) {
            return Err(bad("appearance budget belongs to a different source"));
        }
        let old_bytes = original
            .get_plain_content_with_limit(MAX_BYTES)
            .map_err(|_| bad("source appearance exceeds its decoding limit"))?;
        let new_bytes = generated
            .get_plain_content_with_limit(MAX_BYTES)
            .map_err(|_| bad("generated appearance exceeds its decoding limit"))?;
        let source_bounds = geometry_array::<4>(
            source,
            original
                .dict
                .get(b"BBox")
                .map_err(|_| bad("missing source bounds"))?,
        )?;
        let generated_bounds = geometry_array::<4>(
            native,
            generated
                .dict
                .get(b"BBox")
                .map_err(|_| bad("missing generated bounds"))?,
        )?;
        let old = match AnswerSlot::parse_bounds(&old_bytes, kind, Some(source_bounds)) {
            // An unselected list still paints its option rows. Their existing
            // list-specific frame/region proof remains valid with no selection.
            Ok(slot) if !source_empty || slot.marked || matches!(kind, AnswerKind::List) => slot,
            // An empty logical value cannot identify an unmarked text object as
            // the answer: it may be a static caption. Only glyph-free source
            // paint can establish a new slot without an explicit answer mark.
            _ if source_empty => AnswerSlot::parse_empty_source(&old_bytes, kind, source_bounds)?,
            Err(error) => return Err(error),
            Ok(_) => unreachable!(),
        };
        let new = AnswerSlot::parse_bounds(&new_bytes, kind, Some(generated_bounds)).or_else(
            |error| {
                if empty {
                    AnswerSlot::parse_empty_generated(&new_bytes)
                } else {
                    Err(error)
                }
            },
        )?;
        let mapping = compose(
            inverse(unit_mapping(source, original)?)?,
            unit_mapping(native, generated)?,
        )?;
        let empty_resources = Dictionary::new();
        let generated_resources = generated
            .dict
            .get(b"Resources")
            .ok()
            .map(|v| dictionary(native, v))
            .transpose()?
            .unwrap_or(&empty_resources);
        let reset_name = format!("FolioReset_{}", uuid::Uuid::new_v4().simple());
        let body = answer_body(&new, native, generated_resources, &reset_name)?;
        let unused = prune::unused_answers(native, generated_resources, &body, &reset_name)?;
        budget.copy.reserve_dictionary(generated_resources)?;
        let mut retained_resources = generated_resources.clone();
        if !unused.is_empty() {
            let objects = dictionary(
                native,
                generated_resources
                    .get(b"XObject")
                    .map_err(|_| bad("missing generated XObjects"))?,
            )?;
            budget.copy.reserve_dictionary(objects)?;
            let mut objects = objects.clone();
            for name in unused {
                objects.remove(&name);
            }
            retained_resources.set("XObject", objects);
        }
        let mut resources = budget.copy.copy_dictionary(&retained_resources)?;
        let old_resources = match context {
            Some(resources) => Some(resources),
            None => original
                .dict
                .get(b"Resources")
                .ok()
                .map(|value| dictionary(source, value))
                .transpose()?,
        };
        if can_inline_answer(&old)
            && inline_splice_fits(&old, &body)
            && old_resources.is_none_or(|resources| {
                resources
                    .iter()
                    .all(|(key, _)| matches!(key.as_slice(), b"Font" | b"ProcSet"))
            })
            && font_only_resources(native, generated)?
            && source_bounds == generated_bounds
            && appearance_matrix(source, original)? == appearance_matrix(native, generated)?
            && original.dict.iter().all(|(key, _)| {
                matches!(
                    key.as_slice(),
                    b"Type"
                        | b"Subtype"
                        | b"FormType"
                        | b"BBox"
                        | b"Matrix"
                        | b"Resources"
                        | b"Length"
                        | b"Filter"
                        | b"DecodeParms"
                )
            })
        {
            // Retain the exact source frame, but inline the isolated answer when
            // the retained drawing cannot depend on resource bindings or text /
            // transparency state. This keeps plain fields editable by the existing
            // appearance and resize editors, without substituting native borders.
            let mut inline = body;
            // Only these three constructed defaults need ExtGState/unsupported
            // appearance instructions; the admitted source has no such state.
            inline.remove(10); // constructed rendering intent
            inline.remove(9); // constructed flatness
            inline.remove(1); // constructed default ExtGState
            inline.retain(|op| !matches!(op.operator.as_str(), "BMC" | "EMC"));
            let output = old.splice_operations(&inline)?;
            budget.copy.reserve_dictionary(&original.dict)?;
            budget.copy.reserve_bytes(encoded_bound(&output)?)?;
            let mut dict = original.dict.clone();
            dict.set("Resources", resources);
            let mut appearance = Stream::new(dict, Vec::new());
            appearance.set_plain_content(encode_bounded(&output)?);
            return Ok(StagedAnswerAppearance { appearance });
        }
        let mut states = generated_resources
            .get(b"ExtGState")
            .ok()
            .map(|v| dictionary(native, v))
            .transpose()?
            .map(|d| budget.copy.copy_dictionary(d))
            .transpose()?
            .unwrap_or_default();
        if states.has(reset_name.as_bytes()) {
            return Err(bad("answer reset resource name collision"));
        }
        states.set(reset_name.clone(), reset_state());
        resources.set("ExtGState", states);
        budget.copy.reserve_bytes(encoded_bound(&body)?)?;
        let bbox = geometry_array::<4>(
            native,
            generated
                .dict
                .get(b"BBox")
                .map_err(|_| bad("missing generated bounds"))?,
        )?;
        let answer = Stream::new(
            dictionary! {"Type"=>"XObject","Subtype"=>"Form","BBox"=>bbox.into_iter().map(|v|Object::Real(v as f32)).collect::<Vec<_>>(),"Matrix"=>mapping.into_iter().map(|v|Object::Real(v as f32)).collect::<Vec<_>>(),"Resources"=>resources},
            encode_bounded(&body)?,
        );
        let answer = budget.copy.retain_local(Object::Stream(answer))?;
        budget.copy.reserve_dictionary(&original.dict)?;
        let mut result_dict = original.dict.clone();
        if let Some(d) = old_resources {
            budget.copy.reserve_dictionary(d)?;
        }
        let mut outer_resources = old_resources.cloned().unwrap_or_default();
        let xobjects = outer_resources
            .get(b"XObject")
            .ok()
            .map(|v| dictionary(source, v))
            .transpose()?;
        if let Some(d) = xobjects {
            budget.copy.reserve_dictionary(d)?;
        }
        let mut xobjects = xobjects.cloned().unwrap_or_default();
        let name = format!("FolioAnswer_{}", uuid::Uuid::new_v4().simple());
        if xobjects.has(name.as_bytes()) {
            return Err(bad("answer resource name collision"));
        }
        let output = old.splice_operations(&[Operation::new(
            "Do",
            vec![Object::Name(name.as_bytes().to_vec())],
        )])?;
        // Adding a local XObject category shadows any page-inherited category.
        // Require retained consumers to resolve locally (including nested forms)
        // before doing so; no placement-context inference is authorized here.
        prune::verify_resources(
            source,
            old_resources.unwrap_or(&empty_resources),
            &output,
            &name,
        )?;
        if let Some(resources) = old_resources {
            for unused in prune::unused_answers(source, resources, &output, &name)? {
                xobjects.remove(&unused);
            }
        }
        xobjects.set(name.clone(), answer);
        outer_resources.set("XObject", xobjects);
        result_dict.set("Resources", outer_resources);
        budget.copy.reserve_bytes(encoded_bound(&output)?)?;
        let mut appearance = Stream::new(result_dict, Vec::new());
        appearance.set_plain_content(encode_bounded(&output)?);
        Ok(StagedAnswerAppearance { appearance })
    })();
    budget.failed = result.is_err();
    result
}

fn resolve<'a>(doc: &'a Document, mut value: &'a Object) -> EngineResult<&'a Object> {
    let mut seen = std::collections::HashSet::new();
    while let Object::Reference(id) = value {
        if seen.len() >= 64 || !seen.insert(*id) {
            return Err(bad("cyclic appearance resource"));
        }
        value = doc
            .objects
            .get(id)
            .ok_or_else(|| bad("missing appearance resource"))?;
    }
    Ok(value)
}
fn dictionary<'a>(doc: &'a Document, value: &'a Object) -> EngineResult<&'a Dictionary> {
    resolve(doc, value)?
        .as_dict()
        .map_err(|_| bad("invalid appearance resource dictionary"))
}
fn geometry_array<const N: usize>(doc: &Document, value: &Object) -> EngineResult<[f64; N]> {
    let array = resolve(doc, value)?
        .as_array()
        .map_err(|_| bad("invalid appearance geometry"))?;
    if array.len() != N {
        return Err(bad("invalid appearance geometry length"));
    }
    let mut out = [0.; N];
    for (n, v) in out.iter_mut().zip(array) {
        *n = resolve(doc, v)?
            .as_float()
            .map_err(|_| bad("invalid appearance coordinate"))? as f64;
        if !n.is_finite() || n.abs() > 1_000_000. {
            return Err(bad("appearance coordinate exceeds its limit"));
        }
    }
    Ok(out)
}
fn appearance_matrix(doc: &Document, stream: &Stream) -> EngineResult<[f64; 6]> {
    stream
        .dict
        .get(b"Matrix")
        .ok()
        .map(|v| geometry_array::<6>(doc, v))
        .transpose()
        .map(|v| v.unwrap_or(IDENTITY))
}
fn font_only_resources(doc: &Document, stream: &Stream) -> EngineResult<bool> {
    let Some(value) = stream.dict.get(b"Resources").ok() else {
        return Ok(true);
    };
    // DefaultGray/RGB/CMYK are implicit consumers even without a named cs/CS
    // operation. Keep any richer resource context inside its original form.
    Ok(dictionary(doc, value)?
        .iter()
        .all(|(key, _)| matches!(key.as_slice(), b"Font" | b"ProcSet")))
}
fn can_inline_answer(old: &AnswerSlot) -> bool {
    old.marked
        && old
            .content
            .operations
            .iter()
            .enumerate()
            .filter(|(i, _)| !old.region.contains(i))
            .all(|(_, op)| {
                matches!(
                    op.operator.as_str(),
                    "q" | "Q"
                        | "g"
                        | "G"
                        | "rg"
                        | "RG"
                        | "k"
                        | "K"
                        | "w"
                        | "J"
                        | "j"
                        | "M"
                        | "d"
                        | "m"
                        | "l"
                        | "c"
                        | "v"
                        | "y"
                        | "h"
                        | "re"
                        | "W"
                        | "W*"
                        | "n"
                        | "S"
                        | "s"
                        | "f"
                        | "F"
                        | "f*"
                        | "B"
                        | "B*"
                        | "b"
                        | "b*"
                )
            })
}
/// Reuse publication's retained dependency proof without page fallback. Implicit
/// ColorSpace bindings remain the caller's responsibility. This is not answer
/// authorization: transplant still checks the logical value and actual slot.
pub(crate) fn retained_resources_are_local(
    source: &Document,
    original: &Stream,
    kind: AnswerKind,
    budget: &mut FillAppearanceBudget<'_>,
) -> EngineResult<bool> {
    let bytes = original
        .get_plain_content_with_limit(MAX_BYTES)
        .map_err(|_| bad("source appearance exceeds its decoding limit"))?;
    budget.reserve_bytes(preflight_cost(&bytes)?)?;
    // A styled empty widget can have a genuinely empty normal program. It has
    // no marked slot yet, but no retained resource consumers either. The actual
    // transplant separately checks its logical empty value before adding a slot.
    let bounds = geometry_array::<4>(source, original.dict.get(b"BBox")
        .map_err(|_| bad("missing source bounds"))?)?;
    let Ok(slot) =
        AnswerSlot::parse_bounds(&bytes, kind, Some(bounds)).or_else(|_| AnswerSlot::parse_empty_generated(&bytes))
    else {
        return Ok(false);
    };
    if can_inline_answer(&slot) {
        return Ok(true);
    }
    let empty = Dictionary::new();
    let resources = original
        .dict
        .get(b"Resources")
        .ok()
        .map(|value| dictionary(source, value))
        .transpose()?
        .unwrap_or(&empty);
    // A marked answer is replaced entirely. Unmarked splicing retains its
    // font/color/graphics-state instructions for later artwork, so prove the
    // whole original program in that case. Glyph-show/position operators do not
    // themselves consume resource names. Slices avoid cloning the program.
    // Failed local lookup falls back to binding; budget exhaustion stays poisoned.
    let omitted = if slot.marked { slot.region } else { 0..0 };
    Ok(
        prune::verify_local_resources(source, resources, &slot.content.operations, omitted, budget)
            .is_ok(),
    )
}
fn inline_splice_fits(old: &AnswerSlot, body: &[Operation]) -> bool {
    // Large answers remain in their bounded isolated form rather than widening
    // the direct-operation copy limit used by existing appearance editors.
    let mut budget = crate::form_graph::CopyBudget::with_byte_limit(64 * 1024 * 1024);
    old.content
        .operations
        .iter()
        .chain(body)
        .all(|op| budget.array(&op.operands).is_ok())
}
fn unit_mapping(doc: &Document, stream: &Stream) -> EngineResult<[f64; 6]> {
    let b = geometry_array::<4>(
        doc,
        stream
            .dict
            .get(b"BBox")
            .map_err(|_| bad("missing appearance bounds"))?,
    )?;
    if b[0] >= b[2] || b[1] >= b[3] {
        return Err(bad("empty appearance bounds"));
    }
    let m = stream
        .dict
        .get(b"Matrix")
        .ok()
        .map(|v| geometry_array::<6>(doc, v))
        .transpose()?
        .unwrap_or(IDENTITY);
    checked_matrix(m)?;
    let mut extent = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for x in [b[0], b[2]] {
        for y in [b[1], b[3]] {
            let p = [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]];
            extent[0] = extent[0].min(p[0]);
            extent[1] = extent[1].min(p[1]);
            extent[2] = extent[2].max(p[0]);
            extent[3] = extent[3].max(p[1]);
        }
    }
    let sx = 1. / (extent[2] - extent[0]);
    let sy = 1. / (extent[3] - extent[1]);
    compose([sx, 0., 0., sy, -extent[0] * sx, -extent[1] * sy], m)
}
fn reset_state() -> Dictionary {
    dictionary! {"Type"=>"ExtGState","CA"=>1,"ca"=>1,"BM"=>"Normal","SMask"=>"None","AIS"=>false,"TK"=>true,"OP"=>false,"op"=>false,"OPM"=>0,"SA"=>false,"SM"=>0.02,"TR2"=>"Default","BG2"=>"Default","UCR2"=>"Default","HT"=>"Default"}
}
fn answer_body(
    slot: &AnswerSlot,
    native: &Document,
    resources: &Dictionary,
    reset: &str,
) -> EngineResult<Vec<Operation>> {
    let mut body=Content::decode_strict(b"q 0 g 0 G 1 w 0 J 0 j 10 M [] 0 d 1 i /RelativeColorimetric ri BT 0 Tc 0 Tw 100 Tz 0 TL 0 Tr 0 Ts ET").map_err(|_|bad("invalid initial answer state"))?.operations;
    body.insert(
        1,
        Operation::new("gs", vec![Object::Name(reset.as_bytes().to_vec())]),
    );
    let mut depth = 1usize;
    for (i, op) in slot.content.operations[..slot.region.end]
        .iter()
        .enumerate()
    {
        let category = match op.operator.as_str() {
            "Tf" => Some(b"Font".as_slice()),
            "Do" => Some(b"XObject".as_slice()),
            "gs" => Some(b"ExtGState".as_slice()),
            "sh" => Some(b"Shading".as_slice()),
            _ => None,
        };
        if let Some(category) = category {
            // Resolve every used name in its own category before any graph
            // import. A present Font dictionary is not proof that /F1 exists.
            let category_dict = dictionary(
                native,
                resources
                    .get(category)
                    .map_err(|_| bad("generated paint depends on unavailable local resources"))?,
            )?;
            let name = op.operands[0]
                .as_name()
                .map_err(|_| bad("invalid paint resource name"))?;
            let target = resolve(
                native,
                category_dict
                    .get(name)
                    .map_err(|_| bad("generated paint resource name is not bound locally"))?,
            )?;
            let valid = match category {
                b"Font" | b"ExtGState" => matches!(target, Object::Dictionary(_)),
                b"XObject" => matches!(target, Object::Stream(_)),
                b"Shading" => matches!(target, Object::Dictionary(_) | Object::Stream(_)),
                _ => false,
            };
            if !valid {
                return Err(bad("generated paint resource has the wrong object type"));
            }
        }
        if op.operator == "q" {
            depth += 1;
        } else if op.operator == "Q" {
            depth = depth
                .checked_sub(1)
                .ok_or_else(|| bad("invalid answer graphics depth"))?;
        }
        if i < slot.region.start {
            match op.operator.as_str() {
                "Tj" | "TJ" | "'" | "Do" | "sh" => continue,
                "\"" => {
                    body.push(Operation::new("Tw", vec![op.operands[0].clone()]));
                    body.push(Operation::new("Tc", vec![op.operands[1].clone()]));
                    continue;
                }
                "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                    body.push(Operation::new("n", vec![]));
                    continue;
                }
                _ => {}
            }
        }
        body.push(op.clone());
    }
    for _ in 0..depth {
        body.push(Operation::new("Q", vec![]));
    }
    encoded_bound(&body)?;
    Ok(body)
}
#[cfg(test)]
#[path = "form_answer_transplant_tests.rs"]
mod tests;
