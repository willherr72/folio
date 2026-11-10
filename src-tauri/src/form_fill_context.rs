//! Bind retained appearance resources to their original widget placements.
use super::{bad, dict, resolve};
use crate::{
    form_answer_appearance::{retained_resources_are_local, AnswerKind, FillAppearanceBudget},
    EngineResult,
};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use std::collections::{BTreeMap, HashMap};

#[cfg(test)]
#[path = "form_fill_context_tests.rs"]
mod tests;

const CATEGORIES: &[&[u8]] = &[
    b"Font",
    b"XObject",
    b"ExtGState",
    b"ColorSpace",
    b"Pattern",
    b"Shading",
    b"Properties",
    b"ProcSet",
];

pub(super) struct Contexts<'a> {
    source: &'a Document,
    pages: BTreeMap<u32, ObjectId>,
    resources: HashMap<usize, Option<&'a Dictionary>>,
}

fn category<'a>(
    source: &'a Document,
    resources: Option<&'a Dictionary>,
    key: &[u8],
) -> EngineResult<Option<&'a Object>> {
    let Some(value) = resources.and_then(|d| d.get(key).ok()) else {
        return Ok(None);
    };
    let resolved = resolve(source, value)?;
    match resolved {
        Object::Null => Ok(None),
        Object::Array(_) if key == b"ProcSet" => Ok(Some(value)),
        Object::Dictionary(_) if key != b"ProcSet" => Ok(Some(value)),
        _ => Err(bad("invalid appearance resource category")),
    }
}

impl<'a> Contexts<'a> {
    pub(super) fn new(
        source: &'a Document,
        count: usize,
        budget: &mut FillAppearanceBudget<'_>,
    ) -> EngineResult<Self> {
        if count > 100_000 {
            return Err(bad("too many page resource contexts"));
        }
        budget.reserve_bytes(
            count
                .checked_mul(64)
                .ok_or_else(|| bad("page context budget overflow"))?,
        )?;
        let pages = source.get_pages();
        if pages.len() != count {
            return Err(bad("page resource mapping changed"));
        }
        Ok(Self {
            source,
            pages,
            resources: HashMap::new(),
        })
    }

    fn page(
        &mut self,
        index: usize,
        budget: &mut FillAppearanceBudget<'_>,
    ) -> EngineResult<Option<&'a Dictionary>> {
        if let Some(value) = self.resources.get(&index) {
            return Ok(*value);
        }
        let number = u32::try_from(index)
            .ok()
            .and_then(|v| v.checked_add(1))
            .ok_or_else(|| bad("invalid widget resource page"))?;
        let page = *self
            .pages
            .get(&number)
            .ok_or_else(|| bad("missing widget resource page"))?;
        let resources = crate::page_resources::inherited_page_resources(self.source, page)?
            .map(|value| dict(self.source, value))
            .transpose()?;
        budget.reserve_bytes(128)?;
        self.resources.insert(index, resources);
        Ok(resources)
    }

    pub(super) fn bind(
        &mut self,
        appearance: &Stream,
        placements: &[(usize, usize)],
        kind: AnswerKind,
        budget: &mut FillAppearanceBudget<'_>,
    ) -> EngineResult<Dictionary> {
        let local = appearance
            .dict
            .get(b"Resources")
            .ok()
            .map(|value| dict(self.source, value))
            .transpose()?;
        if let Some(local) = local {
            budget.reserve_dictionary(local)?;
        } else {
            budget.reserve_bytes(64)?;
        }
        let mut bound = local.cloned().unwrap_or_default();
        let local_dependencies =
            retained_resources_are_local(self.source, appearance, kind, budget)?;
        for &key in CATEGORIES {
            // PDFium selects the local category first. Never merge missing names
            // from a page into an existing local category.
            if category(self.source, local, key)?.is_some() {
                continue;
            }
            bound.remove(key);
            // Retain the established local-only publication path when all its
            // retained dependencies resolve locally, including nested programs.
            // Device colors still implicitly consume DefaultGray/RGB/CMYK.
            if local_dependencies && key != b"ColorSpace" {
                continue;
            }
            let mut first: Option<Option<&'a Object>> = None;
            for &(page, _) in placements {
                let inherited = self.page(page, budget)?;
                let candidate = category(self.source, inherited, key)?;
                if let Some(candidate) = candidate {
                    budget.reserve_object(candidate)?;
                }
                if let Some(previous) = first {
                    let previous = previous
                        .map(|value| resolve(self.source, value))
                        .transpose()?;
                    let candidate = candidate
                        .map(|value| resolve(self.source, value))
                        .transpose()?;
                    // References are cheap to retain, but equality must inspect
                    // their direct targets. Bound that traversal before comparing.
                    for value in [previous, candidate].into_iter().flatten() {
                        budget.reserve_object(value)?;
                    }
                    if previous != candidate {
                        return Err(bad(
                            "one widget has conflicting inherited appearance resource contexts",
                        ));
                    }
                } else {
                    first = Some(candidate);
                }
            }
            if let Some(Some(value)) = first {
                budget.reserve_bytes(key.len().saturating_add(64))?;
                bound.set(key.to_vec(), value.clone());
            }
        }
        Ok(bound)
    }
}
