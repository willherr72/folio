//! Bounded cursor-width arithmetic for already proved glyphs; no font authority.
//! New code widths may vary within the existing outline-metric tolerance. Kept
//! code widths never change. Refusal leaves every input untouched.
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub(super) struct Glyph {
    pub code: u16,
    pub width: f32,
    pub nominal: f32,
    pub adjustable: bool,
}
#[derive(Debug)]
pub(super) struct Plan {
    pub changes: Vec<(u16, f32)>,
    pub width_delta: f64,
}
type Result<T> = std::result::Result<T, &'static str>;
const LIMIT: usize = 4096;
pub(super) const REPAIR_BUDGET: usize = 1_000_000;
const UNSAFE: &str = "The following text cursor cannot be preserved across PDF readers with these glyphs. Use paragraph editing for this change.";

fn rounded(w: f32) -> f64 {
    f64::from((w + 0.5).floor())
}
fn residue(w: f32) -> f64 {
    f64::from(w) - rounded(w)
}
fn valid(w: f32) -> bool {
    w.is_finite() && (0.0..=10000.).contains(&w)
}
fn permitted(g: &Glyph, w: f32) -> bool {
    valid(w) && (w - g.nominal).abs() <= 1.01
}
#[derive(Clone)]
struct Group {
    glyph: Glyph,
    count: usize,
    width: f32,
}

// A width within1.01 of its nominal metric can belong to a neighboring integer
// bin. Try those bins explicitly; holding the original rounded width fixed would
// unnecessarily reject e.g. a new zero-advance glyph requiring residue -0.5.
fn candidate(g: &Glyph, desired: f64) -> Option<f32> {
    if !desired.is_finite() || desired.abs() > 0.500001 {
        return None;
    }
    let base = g.nominal.floor() as i32;
    let mut best: Option<(f64, f64, f32)> = None;
    for integer in base - 2..=base + 2 {
        let mut w = (f64::from(integer) + desired) as f32;
        // Rounding the proposed f32 at the upper half-bin may cross its edge.
        // Move back inside that bin before comparing the represented residue.
        for _ in 0..4 {
            if rounded(w) == f64::from(integer) {
                break;
            }
            w = if rounded(w) > f64::from(integer) {
                w.next_down()
            } else {
                w.next_up()
            };
        }
        if rounded(w) != f64::from(integer) || !permitted(g, w) {
            continue;
        }
        let error = (residue(w) - desired).abs();
        let distance = (f64::from(w) - f64::from(g.width)).abs();
        if best.is_none_or(|(e, d, old)| {
            error < e || (error == e && (distance < d || (distance == d && w < old)))
        }) {
            best = Some((error, distance, w));
        }
    }
    best.map(|(_, _, w)| w)
}

fn finish(before: &[f32], after: &[Glyph], groups: &[Group]) -> Result<Plan> {
    let widths = groups
        .iter()
        .map(|g| (g.glyph.code, g.width))
        .collect::<BTreeMap<_, _>>();
    let source_residue = before.iter().map(|w| residue(*w)).sum::<f64>();
    let mut target_residue = 0.;
    let mut integer = 0.;
    for g in after {
        let w = widths[&g.code];
        if !permitted(g, w) || (!g.adjustable && w != g.width) {
            return Err(UNSAFE);
        }
        target_residue += residue(w);
        integer += rounded(w);
    }
    // No epsilon: a nearly matching aggregate is not authority to publish.
    if target_residue != source_residue {
        return Err(UNSAFE);
    }
    integer -= before.iter().map(|w| rounded(*w)).sum::<f64>();
    let changes = groups
        .iter()
        .filter(|g| g.width != g.glyph.width)
        .map(|g| (g.glyph.code, g.width))
        .collect();
    Ok(Plan {
        changes,
        width_delta: integer,
    })
}

pub(super) fn plan(before: &[f32], after: &[Glyph]) -> Result<Plan> {
    plan_with_budget(before, after, &mut REPAIR_BUDGET.clone())
}

pub(super) fn plan_with_budget(before: &[f32], after: &[Glyph], budget: &mut usize) -> Result<Plan> {
    if *budget > REPAIR_BUDGET {
        return Err(UNSAFE);
    }
    if before.is_empty()
        || after.is_empty()
        || before.len() > LIMIT
        || after.len() > LIMIT
        || before.iter().any(|w| !valid(*w))
    {
        return Err(UNSAFE);
    }
    let mut grouped = BTreeMap::<u16, Group>::new();
    for g in after {
        if !valid(g.nominal) || !permitted(g, g.width) {
            return Err(UNSAFE);
        }
        if let Some(old) = grouped.get_mut(&g.code) {
            if old.glyph.width != g.width
                || old.glyph.nominal != g.nominal
                || old.glyph.adjustable != g.adjustable
            {
                return Err(UNSAFE);
            }
            old.count += 1;
        } else {
            grouped.insert(
                g.code,
                Group {
                    glyph: g.clone(),
                    count: 1,
                    width: g.width,
                },
            );
        }
    }
    let mut groups = grouped.into_values().collect::<Vec<_>>();
    let wanted = before.iter().map(|w| residue(*w)).sum::<f64>();
    let mut current = groups
        .iter()
        .map(|g| residue(g.width) * g.count as f64)
        .sum::<f64>();
    if current == wanted {
        return finish(before, after, &groups);
    }
    let eligible = groups
        .iter()
        .enumerate()
        .filter(|(_, g)| g.glyph.adjustable)
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    // Prefer one exact change, considering multiplicity of every occurrence.
    for &i in &eligible {
        let g = &groups[i];
        let target = residue(g.width) + (wanted - current) / g.count as f64;
        if let Some(w) = candidate(&g.glyph, target) {
            if current + (residue(w) - residue(g.width)) * g.count as f64 == wanted {
                let old = groups[i].width;
                groups[i].width = w;
                if let Ok(plan) = finish(before, after, &groups) {
                    return Ok(plan);
                }
                groups[i].width = old;
            }
        }
    }
    // Deterministic bounded allocation across codes. Keep the best represented
    // value; a later code or the bounded repair may absorb f32 quantization.
    for &i in &eligible {
        let g = &groups[i];
        let target = (residue(g.width) + (wanted - current) / g.count as f64)
            .clamp(-0.5, f64::from(0.5f32.next_down()));
        if let Some(w) = candidate(&g.glyph, target) {
            current += (residue(w) - residue(g.width)) * g.count as f64;
            groups[i].width = w;
        }
        if current == wanted {
            if let Ok(plan) = finish(before, after, &groups) {
                return Ok(plan);
            }
        }
    }
    // Two repeated codes can require compensating changes even when neither
    // can individually represent the remainder (e.g. multiplicities2 and3).
    // Explore local representable widths, solving the other code directly.
    for &i in &eligible {
        for upwards in [false, true] {
            let base = groups[i].width;
            let mut a = base;
            for _ in 0..LIMIT {
                a = if upwards { a.next_up() } else { a.next_down() };
                if !permitted(&groups[i].glyph, a) || rounded(a) != rounded(base) {
                    break;
                }
                let delta = (residue(a) - residue(base)) * groups[i].count as f64;
                for &j in &eligible {
                    if i == j {
                        continue;
                    }
                    if *budget == 0 {
                        return Err(UNSAFE);
                    }
                    *budget -= 1;
                    let g = &groups[j];
                    let target = residue(g.width) + (wanted - current - delta) / g.count as f64;
                    if let Some(b) = candidate(&g.glyph, target) {
                        if current + delta + (residue(b) - residue(g.width)) * g.count as f64
                            == wanted
                        {
                            let old = groups[j].width;
                            groups[i].width = a;
                            groups[j].width = b;
                            if let Ok(plan) = finish(before, after, &groups) {
                                return Ok(plan);
                            }
                            groups[i].width = base;
                            groups[j].width = old;
                        }
                    }
                }
            }
        }
    }
    Err(UNSAFE)
}
#[cfg(test)]
#[path = "cursor_widths_tests.rs"]
mod tests;
