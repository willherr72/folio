//! Source-bound saved field text consistency, including native layout separators.
use crate::{form_text_appearance::bad, EngineResult};
use lopdf::{dictionary, Dictionary, Document, Object, Stream};
use pdfium_render::prelude::Pdfium;

pub(crate) const TEXT_LIMIT: usize = 128 * 1024;
fn mismatch() -> crate::EngineError {
    bad("the saved field text differs from its stored value or cannot be verified; use Fill forms to explicitly change the answer")
}

fn consistent(
    value: &str,
    characters: impl IntoIterator<Item = EngineResult<(String, bool)>>,
) -> EngineResult<()> {
    if value.len() > TEXT_LIMIT {
        return Err(bad("field value exceeds the text verification limit"));
    }
    let mut expected = value.chars();
    let mut retained = 0usize;
    let mut real_space = false;
    let mut generated_space = false;
    let mut first = true;
    for character in characters {
        let (text, generated) = character?;
        retained = retained.checked_add(text.len()).ok_or_else(mismatch)?;
        if retained > TEXT_LIMIT {
            return Err(bad("saved field text exceeds the verification limit"));
        }
        if text.is_empty() {
            return Err(mismatch());
        }
        for ch in text.chars() {
            if ch.is_whitespace() {
                if generated {
                    generated_space = true;
                } else {
                    real_space = true;
                }
                continue;
            }
            if generated || ch == '\u{fffd}' || ch.is_control() {
                return Err(mismatch());
            }
            let mut expected_space = false;
            let next = loop {
                match expected.next() {
                    Some(c) if c.is_whitespace() => expected_space = true,
                    other => break other,
                }
            };
            if next != Some(ch)
                || (!first
                    && ((real_space && !expected_space)
                        || (expected_space && !real_space && !generated_space)))
            {
                return Err(mismatch());
            }
            first = false;
            real_space = false;
            generated_space = false;
        }
    }
    if expected.any(|c| !c.is_whitespace()) {
        return Err(mismatch());
    }
    Ok(())
}

pub(crate) fn verify(
    pdfium: &Pdfium,
    source: &Document,
    widget: &Dictionary,
    value: &str,
) -> EngineResult<()> {
    let normal = widget
        .get(b"AP")
        .ok()
        .map(|ap| {
            source
                .dereference(ap)
                .map_err(bad)?
                .1
                .as_dict()
                .map_err(bad)
        })
        .transpose()?
        .and_then(|ap| ap.get(b"N").ok());
    let Some(normal) = normal else {
        return if value.is_empty() {
            Ok(())
        } else {
            Err(mismatch())
        };
    };
    // load() has already admitted an identity-matrix, zero-origin appearance.
    // Copy only that graph: no widget, field, page or form ownership is imported.
    let bounds = source
        .dereference(normal)
        .map_err(bad)?
        .1
        .as_stream()
        .map_err(bad)?
        .dict
        .get(b"BBox")
        .map_err(bad)?;
    let bounds = source
        .dereference(bounds)
        .map_err(bad)?
        .1
        .as_array()
        .map_err(bad)?;
    if bounds.len() != 4 {
        return Err(bad("invalid appearance bounds"));
    }
    // Source references cannot be transplanted into the new page tree. Resolve
    // both the array and its coordinates; the AP itself is remapped by Copier.
    let bounds = bounds
        .iter()
        .map(|v| {
            let n = source
                .dereference(v)
                .map_err(bad)?
                .1
                .as_float()
                .map_err(bad)?;
            if !n.is_finite() {
                return Err(bad("nonfinite appearance bounds"));
            }
            Ok(Object::Real(n))
        })
        .collect::<EngineResult<Vec<_>>>()?;
    let mut probe = Document::with_version("1.7");
    let copied = crate::form_appearance::Copier::new(source, &mut probe).copy(normal)?;
    let appearance = match copied {
        Object::Reference(id) => id,
        Object::Stream(s) => probe.add_object(s),
        _ => return Err(mismatch()),
    };
    let pages = probe.new_object_id();
    let content = probe.add_object(Stream::new(Dictionary::new(), b"q /Field Do Q".to_vec()));
    let page = probe.add_object(dictionary! {
        "Type"=>"Page", "Parent"=>pages, "MediaBox"=>bounds,
        "Resources"=>dictionary!{"XObject"=>dictionary!{"Field"=>appearance}},
        "Contents"=>content
    });
    probe.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages", "Kids"=>vec![Object::Reference(page)], "Count"=>1}.into(),
    );
    let catalog = probe.add_object(dictionary! {"Type"=>"Catalog", "Pages"=>pages});
    probe.trailer.set("Root", catalog);
    let mut bytes = Vec::new();
    probe.save_to(&mut bytes).map_err(bad)?;
    let doc = pdfium.load_pdf_from_byte_slice(&bytes, None).map_err(bad)?;
    let page = doc.pages().get(0).map_err(bad)?;
    let text = page.text().map_err(bad)?;
    let chars = text.chars();
    if chars.len() as usize > TEXT_LIMIT {
        return Err(bad(
            "saved field text exceeds the character verification limit",
        ));
    }
    consistent(
        value,
        chars.iter().map(|c| {
            Ok((
                c.unicode_string().ok_or_else(mismatch)?,
                c.is_generated().map_err(bad)?,
            ))
        }),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    fn check(value: &str, chars: &[(&str, bool)]) -> bool {
        consistent(value, chars.iter().map(|(s, g)| Ok((s.to_string(), *g)))).is_ok()
    }
    #[test]
    fn generated_boundaries_allow_wraps_and_comb_but_real_spaces_are_required() {
        assert!(check("AB", &[("A", false), (" ", true), ("B", false)]));
        assert!(check("A B", &[("A", false), ("\r\n", true), ("B", false)]));
        assert!(check(
            "A\n\nB",
            &[("A", false), ("\r\n", true), ("B", false)]
        ));
        assert!(!check("AB", &[("A", false), (" ", false), ("B", false)]));
        assert!(!check("A B", &[("A", false), ("B", false)]));
    }
    #[test]
    fn differing_extra_missing_and_unreadable_characters_are_rejected() {
        assert!(!check("Answer", &[("Other", false)]));
        assert!(!check("Answer", &[("Answer decoration", false)]));
        assert!(!check("Answer", &[]));
        assert!(!check("", &[("stale", false)]));
        assert!(!check("x", &[("", false)]));
        assert!(!check("x", &[("�", false)]));
        assert!(!check("x", &[("x", true)]));
    }
    #[test]
    fn real_unicode_is_exact_and_whitespace_runs_keep_their_boundaries() {
        assert!(check(
            "Café Ω",
            &[("Café", false), (" ", false), ("Ω", false)]
        ));
        assert!(!check(
            "Cafe Ω",
            &[("Café", false), (" ", false), ("Ω", false)]
        ));
        assert!(check(
            "  A   B  ",
            &[("A", false), (" ", false), ("B", false)]
        ));
    }
    #[test]
    fn value_and_retained_unicode_are_bounded() {
        assert!(!check(&"x".repeat(128 * 1024 + 1), &[]));
        let huge = "x".repeat(128 * 1024 + 1);
        assert!(!check("x", &[(&huge, false)]));
    }
}
