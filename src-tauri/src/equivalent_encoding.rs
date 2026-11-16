//! Recognize a named encoding without changing any character-to-glyph mapping.
//! Unicode equivalence is insufficient: alternate glyph names can paint differently.
use lopdf::{Document, Object};

fn nominal_glyph(base: &[u8], code: usize) -> Option<&'static [u8]> {
    // Literal resource equivalence is separate from per-run source-byte proof.
    if !(32..=255).contains(&code) { return None; }
    let table = match base {
        b"WinAnsiEncoding" => &super::encoding_names::WIN_ANSI_ENCODING,
        b"MacRomanEncoding" => &super::encoding_names::MAC_ROMAN_ENCODING,
        _ => return None,
    };
    table[code].map(str::as_bytes)
}

/// A dictionary is accepted only with an explicit supported base and no effective
/// Differences. The original PDF object is retained verbatim by callers.
pub(super) fn name<'a>(pdf: &'a Document, value: &'a Object) -> Option<&'a [u8]> {
    let value = pdf.dereference(value).ok()?.1;
    if let Ok(name) = value.as_name() {
        return matches!(name, b"StandardEncoding" | b"WinAnsiEncoding" | b"MacRomanEncoding")
            .then_some(name);
    }
    let dict = value.as_dict().ok()?;
    if dict.len() > 3 || dict.iter().any(|(key, _)|
        !matches!(key.as_slice(), b"Type" | b"BaseEncoding" | b"Differences")) {
        return None;
    }
    if let Ok(kind) = dict.get(b"Type") {
        if pdf.dereference(kind).ok()?.1.as_name().ok()? != b"Encoding" { return None; }
    }
    let base = pdf.dereference(dict.get(b"BaseEncoding").ok()?).ok()?.1.as_name().ok()?;
    if !matches!(base, b"WinAnsiEncoding" | b"MacRomanEncoding") { return None; }
    if let Ok(differences) = dict.get(b"Differences") {
        let values = pdf.dereference(differences).ok()?.1.as_array().ok()?;
        if values.len() > 512 { return None; }
        let mut cursor = None;
        let mut awaiting_glyph = false;
        let mut assigned = [false; 256];
        for value in values {
            match pdf.dereference(value).ok()?.1 {
                Object::Integer(code) if (0..=255).contains(code) && !awaiting_glyph => {
                    cursor = Some(*code as usize);
                    awaiting_glyph = true;
                }
                Object::Name(glyph) => {
                    let code = cursor?;
                    if nominal_glyph(base, code)? != glyph || *assigned.get(code)? { return None; }
                    assigned[code] = true;
                    cursor = Some(code + 1);
                    awaiting_glyph = false;
                }
                _ => return None,
            }
        }
        if awaiting_glyph { return None; }
    }
    Some(base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;
    fn encoding(base: &str, values: Vec<Object>) -> Object {
        dictionary! {"Type"=>"Encoding","BaseEncoding"=>base,"Differences"=>values}.into()
    }
    #[test]
    fn exact_ascii_names_and_empty_dictionaries_retain_the_named_encoding() {
        let pdf = Document::new();
        for base in ["WinAnsiEncoding", "MacRomanEncoding"] {
            let mut differences = vec![32.into()];
            differences.extend((32..=126).map(|code| Object::Name(nominal_glyph(base.as_bytes(), code).unwrap().to_vec())));
            let value = encoding(base, differences);
            assert_eq!(name(&pdf, &value), Some(base.as_bytes()));
            assert_eq!(name(&pdf, &encoding(base, vec![])), Some(base.as_bytes()));
            let value = dictionary! {"BaseEncoding"=>base}.into();
            assert_eq!(name(&pdf, &value), Some(base.as_bytes()));
        }
    }
    #[test]
    fn malformed_remapped_duplicate_or_aliased_glyphs_are_not_equivalent() {
        let pdf = Document::new();
        for values in [
            vec![65.into(), Object::Name(b"Z".to_vec())],
            vec![65.into(), Object::Name(b"uni0041".to_vec())],
            vec![32.into(), Object::Name(b"nbspace".to_vec())],
            vec![127.into(), Object::Name(b"Euro".to_vec())],
            vec![65.into()], vec![65.into(), 66.into(), Object::Name(b"B".to_vec())],
            vec![Object::Name(b"A".to_vec())], vec![(-1).into()], vec![256.into()],
            vec![Object::Real(65.), Object::Name(b"A".to_vec())],
            vec![65.into(), Object::Name(b"A".to_vec()), 65.into(), Object::Name(b"A".to_vec())],
            vec![Object::Null; 513],
        ] { assert!(name(&pdf, &encoding("WinAnsiEncoding", values)).is_none()); }
        for value in [
            dictionary! {"BaseEncoding"=>"StandardEncoding"}.into(),
            dictionary! {"BaseEncoding"=>"MacExpertEncoding"}.into(),
            dictionary! {"Differences"=>Vec::<Object>::new()}.into(),
            dictionary! {"BaseEncoding"=>"WinAnsiEncoding","Differences"=>Object::Null}.into(),
            dictionary! {"BaseEncoding"=>"WinAnsiEncoding","Type"=>"Font"}.into(),
        ] { assert!(name(&pdf, &value).is_none()); }
    }
    #[test]
    fn literal_accented_names_retain_each_explicit_base() {
        let pdf = Document::new();
        for (base, code, glyph) in [
            ("WinAnsiEncoding", 233, "eacute"),
            ("WinAnsiEncoding", 127, "bullet"),
            ("WinAnsiEncoding", 128, "Euro"),
            ("WinAnsiEncoding", 201, "Eacute"),
            ("MacRomanEncoding", 142, "eacute"),
            ("MacRomanEncoding", 128, "Adieresis"),
            ("MacRomanEncoding", 174, "AE"),
        ] {
            let value = encoding(base, vec![code.into(), Object::Name(glyph.as_bytes().to_vec())]);
            assert_eq!(name(&pdf, &value), Some(base.as_bytes()), "{base}/{code}/{glyph}");
        }
    }
    #[test]
    fn accented_names_do_not_authorize_wrong_base_unicode_aliases_or_duplicates() {
        let pdf = Document::new();
        for (base, values) in [
            ("WinAnsiEncoding", vec![142.into(), Object::Name(b"eacute".to_vec())]),
            ("MacRomanEncoding", vec![233.into(), Object::Name(b"eacute".to_vec())]),
            ("WinAnsiEncoding", vec![233.into(), Object::Name(b"uni00E9".to_vec())]),
            ("WinAnsiEncoding", vec![233.into(), Object::Name(b"egrave".to_vec())]),
            ("WinAnsiEncoding", vec![233.into(), Object::Name(b"eacute".to_vec()), 233.into(), Object::Name(b"eacute".to_vec())]),
            ("MacRomanEncoding", vec![240.into(), Object::Name(b"uniF8FF".to_vec())]),
        ] { assert!(name(&pdf, &encoding(base, values)).is_none(), "{base}"); }
    }
    #[test]
    fn indirect_members_are_bounded_and_source_objects_are_unchanged() {
        let mut pdf = Document::new();
        let base = pdf.add_object(Object::Name(b"WinAnsiEncoding".to_vec()));
        let a = pdf.add_object(Object::Name(b"A".to_vec()));
        let values = pdf.add_object(vec![65.into(), a.into()]);
        let encoding = pdf.add_object(dictionary! {"BaseEncoding"=>base,"Differences"=>values});
        let before = pdf.objects.clone();
        assert_eq!(name(&pdf, &encoding.into()), Some(b"WinAnsiEncoding".as_slice()));
        assert_eq!(pdf.objects, before);
        let cycle = pdf.new_object_id();
        pdf.objects.insert(cycle, cycle.into());
        assert_eq!(name(&pdf, &cycle.into()), None);
        let broken = dictionary! {"BaseEncoding"=>cycle}.into();
        assert_eq!(name(&pdf, &broken), None);
    }
}
