//! Rename only standard field-name action targets, preserving shared objects.
use super::{bad, resolve, text};
use crate::EngineResult;
use lopdf::{Dictionary, Document, Object, ObjectId};
#[derive(Clone)]
enum Step {
    Key(Vec<u8>),
    Index(usize),
}
struct Patch {
    root: ObjectId,
    path: Vec<Step>,
    key: &'static str,
    value: Object,
}
struct Budget {
    values: usize,
    bytes: usize,
}
impl Budget {
    fn take(&mut self, bytes: usize, depth: usize) -> EngineResult<()> {
        self.values = self
            .values
            .checked_sub(1)
            .ok_or_else(|| bad("action scan exceeds 100000 values"))?;
        self.bytes = self
            .bytes
            .checked_sub(bytes)
            .ok_or_else(|| bad("action scan exceeds 16 MiB"))?;
        if depth > 64 {
            return Err(bad("action nesting exceeds 64 levels"));
        }
        Ok(())
    }
}
fn targets(
    d: &Document,
    v: &Object,
    old: &str,
    new: &str,
    b: &mut Budget,
    copy: &mut crate::form_graph::CopyBudget,
    depth: usize,
) -> EngineResult<Option<Object>> {
    b.take(64, depth)?;
    let resolved = resolve(d, v)?;
    match resolved {
        Object::String(raw, _) => {
            b.take(raw.len().saturating_mul(3) + new.len(), depth)?;
            Ok((text(d, resolved, 32768)? == old).then(|| lopdf::text_string(new)))
        }
        Object::Array(a) => {
            copy.array(a)?;
            let mut changed = false;
            let mut result = Vec::new();
            for item in a {
                let next = targets(d, item, old, new, b, copy, depth + 1)?;
                changed |= next.is_some();
                result.push(next.unwrap_or_else(|| item.clone()));
            }
            Ok(changed.then_some(Object::Array(result)))
        }
        Object::Dictionary(_) => Ok(None),
        _ => Err(bad("unsupported field-name action target")),
    }
}
fn scan(
    doc: &Document,
    rename: Option<(&str, &str)>,
) -> EngineResult<(Option<String>, Vec<Patch>)> {
    if doc.objects.len() > 100000 {
        return Err(bad("action scan exceeds 100000 objects"));
    }
    let mut budget = Budget {
        values: 100000,
        bytes: 16 * 1024 * 1024,
    };
    let mut patches = vec![];
    let mut reason = None;
    let mut copy = crate::form_graph::CopyBudget::default();
    fn walk(
        d: &Document,
        v: &Object,
        root: ObjectId,
        path: &mut Vec<Step>,
        b: &mut Budget,
        copy: &mut crate::form_graph::CopyBudget,
        rename: Option<(&str, &str)>,
        reason: &mut Option<String>,
        patches: &mut Vec<Patch>,
    ) -> EngineResult<()> {
        b.take(64, path.len())?;
        match v {
            Object::Dictionary(dict) | Object::Stream(lopdf::Stream { dict, .. }) => {
                if dict.has(b"JS") {
                    *reason=Some("This document contains scripts; field names cannot be safely rewritten. Labels and flags can still be changed.".into());
                }
                let kind = dict
                    .get(b"S")
                    .ok()
                    .map(|v| resolve(d, v))
                    .transpose()?
                    .and_then(|v| v.as_name().ok());
                match kind {
     Some(b"JavaScript"|b"ImportData")=>*reason=Some("This document contains scripts or imported form data; field names cannot be safely rewritten. Labels and flags can still be changed.".into()),
     Some(b"ResetForm"|b"SubmitForm"|b"Hide")=>{
      let key=if kind==Some(b"Hide".as_slice()){"T"}else{"Fields"};
      if let (Some((old,new)),Ok(value))=(rename,dict.get(key.as_bytes())){if let Some(value)=targets(d,value,old,new,b,copy,0)?{b.take(path.iter().map(|s|match s{Step::Key(k)=>k.len()+64,Step::Index(_)=>64}).sum(),path.len())?;patches.push(Patch{root,path:path.clone(),key,value});}}
     },
     Some(b"GoTo"|b"GoToR"|b"GoToE"|b"URI"|b"Named"|b"Launch"|b"Thread"|b"Sound"|b"Movie"|b"SetOCGState"|b"Rendition"|b"Trans"|b"GoTo3DView"|b"RichMediaExecute"|b"S"|b"D"|b"B"|b"I"|b"U"|b"Transparency"|b"Alpha"|b"Luminosity")|None=>{},
     Some(_)=>*reason=Some("This document contains an unsupported action or structure; field names cannot be safely rewritten. Labels and flags can still be changed.".into()),
    }
                for (key, value) in dict.iter() {
                    b.take(key.len(), path.len())?;
                    path.push(Step::Key(key.clone()));
                    walk(d, value, root, path, b, copy, rename, reason, patches)?;
                    path.pop();
                }
            }
            Object::Array(a) => {
                for (i, value) in a.iter().enumerate() {
                    path.push(Step::Index(i));
                    walk(d, value, root, path, b, copy, rename, reason, patches)?;
                    path.pop();
                }
            }
            Object::String(s, _) | Object::Name(s) => b.take(s.len(), path.len())?,
            _ => {}
        }
        Ok(())
    }
    for (id, obj) in &doc.objects {
        walk(
            doc,
            obj,
            *id,
            &mut vec![],
            &mut budget,
            &mut copy,
            rename,
            &mut reason,
            &mut patches,
        )?;
    }
    Ok((reason, patches))
}
pub(super) fn rename_reason(doc: &Document) -> EngineResult<Option<String>> {
    scan(doc, None).map(|v| v.0)
}
fn dictionary(v: &mut Object) -> EngineResult<&mut Dictionary> {
    match v {
        Object::Dictionary(d) => Ok(d),
        Object::Stream(s) => Ok(&mut s.dict),
        _ => Err(bad("action path changed")),
    }
}
pub(super) fn rename(doc: &mut Document, old: &str, new: &str) -> EngineResult<()> {
    let (reason, patches) = scan(doc, Some((old, new)))?;
    if let Some(reason) = reason {
        return Err(bad(reason));
    }
    for patch in patches {
        let mut node = doc.get_object_mut(patch.root).map_err(bad)?;
        for step in patch.path {
            node = match step {
                Step::Key(k) => dictionary(node)?.get_mut(&k).map_err(bad)?,
                Step::Index(i) => node
                    .as_array_mut()
                    .map_err(bad)?
                    .get_mut(i)
                    .ok_or_else(|| bad("action array changed"))?,
            };
        }
        dictionary(node)?.set(patch.key, patch.value);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;
    #[test]
    fn shared_target_arrays_have_one_aggregate_copy_budget() {
        let mut doc = Document::new();
        let targets=doc.add_object(Object::Array(vec![Object::string_literal("old"),dictionary!{"Subtype"=>"Widget","Private"=>Object::string_literal(vec![b'x';1024*1024])}.into()]));
        for _ in 0..20 {
            doc.add_object(dictionary! {"S"=>"Hide","T"=>targets});
        }
        assert!(
            rename(&mut doc, "old", "new").is_err(),
            "repeated shared targets bypassed the aggregate copy budget"
        );
    }
    #[test]
    fn unrelated_non_name_s_entries_do_not_block_property_inspection() {
        let mut doc = Document::new();
        doc.add_object(dictionary! {"S"=>42,"Private"=>true});
        assert!(rename_reason(&doc).unwrap().is_none());
    }
}
