use super::*;
const SPACE: &str = "1 begincodespacerange <0000> <ffff> endcodespacerange ";
fn source(body: &str) -> EngineResult<BTreeMap<u16, SourceMapping>> {
    source_cmap(format!("{SPACE}{body}").as_bytes(), 2)
}

#[test]
fn source_cmap_preserves_fi_ffi_without_changing_scalar_or_complete_policies() {
    let data = format!(
        "{SPACE}3 beginbfchar <0001> <004f> <0002> <00660069> <0003> <006600660069> endbfchar"
    );
    let map = source_cmap(data.as_bytes(), 2).unwrap();
    assert_eq!(map[&1], SourceMapping::Scalar('O'));
    assert_eq!(map[&2], SourceMapping::Sequence("fi".into()));
    assert_eq!(map[&3], SourceMapping::Sequence("ffi".into()));
    let old = cmap_with_policy(data.as_bytes(), 2, CMapPolicy::LatinEditing).unwrap();
    assert_eq!(old[&1], Some('O'));
    assert_eq!(old[&2], None);
    assert_eq!(old[&3], None);
    assert!(cmap_with_policy(data.as_bytes(), 2, CMapPolicy::Complete).is_err());
}

#[test]
fn source_cmap_ranges_arrays_and_supplementary_sequences_follow_encoded_bytes() {
    let map=source("3 beginbfrange <0001> <0002> <00660069> <0003> <0004> [<006600660069> <0066006c>] <0005> <0006> <006600ff> endbfrange 1 beginbfchar <0007> <d83edd800061> endbfchar").unwrap();
    assert_eq!(map[&1], SourceMapping::Sequence("fi".into()));
    assert_eq!(map[&2], SourceMapping::Sequence("fj".into()));
    assert_eq!(map[&3], SourceMapping::Sequence("ffi".into()));
    assert_eq!(map[&4], SourceMapping::Sequence("fl".into()));
    assert_eq!(map[&5], SourceMapping::Sequence("fÿ".into()));
    assert_eq!(map[&6], SourceMapping::Unavailable); // never carry out of final byte.
    assert_eq!(map[&7], SourceMapping::Sequence("🦀a".into()));
}

#[test]
fn source_cmap_unavailable_destinations_still_participate_in_structure_checks() {
    let map=source("5 beginbfchar <0001> <d800> <0002> <> <0003> <00> <0004> <006600d800> <0005> <006600660069> endbfchar").unwrap();
    for code in 1..=4 {
        assert_eq!(map[&code], SourceMapping::Unavailable);
    }
    assert_eq!(map[&5], SourceMapping::Sequence("ffi".into()));
    for body in [
        "2 beginbfchar <0001> <d800> <0001> <00660069> endbfchar",
        "1 beginbfchar <0001> <00660069> endbfrange",
        "1 beginbfchar <0001> 12 endbfchar",
        "/Inherited usecmap 1 beginbfchar <0001> <00660069> endbfchar",
        "1 beginbfrange <0001> <0002> [<00660069>] endbfrange",
    ] {
        assert!(source(body).is_err(), "{body}");
    }
    assert!(source_cmap(b"1 begincodespacerange <0000> <0001> endcodespacerange 1 beginbfchar <0002> <00660069> endbfchar",2).is_err());
}

#[test]
fn source_cmap_sequence_budget_excludes_all_sequences_independent_of_order() {
    for reverse in [false, true] {
        let mut entries: Vec<_> = (1u16..=513)
            .map(|code| format!("<{code:04x}> <0061{:04x}>", 0x400 + code))
            .collect();
        if reverse {
            entries.reverse();
        }
        let map = source(&format!(
            "514 beginbfchar {} <ffff> <004f> endbfchar",
            entries.join(" ")
        ))
        .unwrap();
        assert_eq!(map[&65535], SourceMapping::Scalar('O'));
        assert!(map
            .iter()
            .filter(|(code, _)| **code != 65535)
            .all(|(_, value)| *value == SourceMapping::Unavailable));
    }
    let entries: Vec<_> = (1u16..=512)
        .map(|code| format!("<{code:04x}> <0061{:04x}>", 0x400 + code))
        .collect();
    let map = source(&format!("512 beginbfchar {} endbfchar", entries.join(" "))).unwrap();
    assert!(map
        .values()
        .all(|value| matches!(value, SourceMapping::Sequence(_))));
}

#[test]
fn source_cmap_sequence_length_is_bounded_before_retention() {
    let map = source(&format!(
        "3 beginbfchar <0001> <{}> <0002> <{}> <0003> <{}> endbfchar",
        "0061".repeat(16),
        "0061".repeat(17),
        "d83edd80".repeat(16)
    ))
    .unwrap();
    assert_eq!(map[&1], SourceMapping::Sequence("a".repeat(16)));
    assert_eq!(map[&2], SourceMapping::Unavailable);
    assert_eq!(map[&3], SourceMapping::Sequence("🦀".repeat(16)));
}
