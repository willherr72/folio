use super::*;
use lopdf::{dictionary, Dictionary, Object};

#[test]
fn review_inherited_logo_must_not_be_shadowed_by_a_new_local_xobject_category() {
    for font_only in [false, true] {
        let mut source = Document::new();
        let mut native = Document::new();
        let mut original = stream(
            &mut source,
            "Helvetica",
            b"/Tx BMC q BT /F1 12 Tf (Old) Tj ET Q EMC /Logo Do",
        );
        if !font_only {
            original.dict.remove(b"Resources");
        }
        let generated = stream(
            &mut native,
            "Helvetica",
            b"/Tx BMC q BT /F1 12 Tf (New) Tj ET Q EMC",
        );
        let mut staging = Document::new();
        let mut budget = FillAppearanceBudget::new(&source, &native, &mut staging).unwrap();
        assert!(
            transplant_answer(
                &source,
                &original,
                &native,
                &generated,
                AnswerKind::Text,
                &mut budget
            )
            .is_err(),
            "unproved inherited Logo was shadowed, font_only={font_only}"
        );
    }
}

#[test]
fn review_empty_value_does_not_authorize_deleting_an_unmarked_caption() {
    let mut source = Document::new();
    let mut native = Document::new();
    let original = stream(
        &mut source,
        "Helvetica",
        b"q 0 0 100 100 re S Q BT /F1 8 Tf 5 80 Td (Account number:) Tj ET",
    );
    let generated = stream(
        &mut native,
        "Helvetica",
        b"/Tx BMC q BT /F1 12 Tf (12345) Tj ET Q EMC",
    );
    let mut staging = Document::new();
    let mut budget = FillAppearanceBudget::new(&source, &native, &mut staging).unwrap();
    assert!(
        transplant_answer_allow_empty(
            &source,
            &original,
            &native,
            &generated,
            AnswerKind::Text,
            &mut budget,
            false,
            true
        )
        .is_err(),
        "empty logical value authorized deleting a caption"
    );
}

fn stream(doc: &mut Document, font: &str, content: &[u8]) -> Stream {
    let font = doc.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>font});
    Stream::new(
        dictionary! {"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),100.into(),100.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}},
        content.to_vec(),
    )
}
fn page(mut doc: Document, appearance: Stream) -> Vec<u8> {
    let pages = doc.new_object_id();
    let ap = doc.add_object(appearance);
    let content = doc.add_object(Stream::new(Dictionary::new(), b"/Appearance Do".to_vec()));
    let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),100.into(),100.into()],"Resources"=>dictionary!{"XObject"=>dictionary!{"Appearance"=>ap}},"Contents"=>content});
    doc.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]}.into(),
    );
    let root = doc.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    doc.trailer.set("Root", root);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    bytes
}
fn render_pair(actual: Vec<u8>, expected: Vec<u8>) {
    let e = crate::PdfEngine::start(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut pictures = Vec::new();
    for (name, bytes) in [("actual", actual), ("expected", expected)] {
        let path = dir.path().join(format!("{name}.pdf"));
        std::fs::write(&path, bytes).unwrap();
        let d = e.open_document(&path).unwrap();
        pictures.push(e.render_page(&d.id, 0, 400).unwrap());
        e.close_document(&d.id).unwrap();
    }
    assert!(
        pictures[0] == pictures[1],
        "native output differs from independently constructed expected appearance"
    );
}
fn with_user_unit(bytes: &[u8], unit: f32) -> Vec<u8> {
    let mut doc = Document::load_mem(bytes).unwrap();
    let page = *doc.get_pages().get(&1).unwrap();
    doc.get_dictionary_mut(page).unwrap().set("UserUnit", unit);
    let mut output = Vec::new();
    doc.save_to(&mut output).unwrap();
    output
}

#[test]
fn conflicting_font_names_keep_source_caption_and_new_answer_fonts_and_foreground() {
    let mut source = Document::new();
    let mut native = Document::new();
    let original=stream(&mut source,"Helvetica",b"BT /F1 10 Tf 5 75 Td (Caption) Tj ET /Tx BMC q 0 0 100 60 re W n BT /F1 12 Tf 5 30 Td (Old) Tj ET Q EMC 1 0 0 rg 10 25 8 15 re f");
    let generated=stream(&mut native,"Courier",b"q 1 g 0 0 100 100 re f Q /Tx BMC q 0 0 100 60 re W n BT /F1 12 Tf 5 30 Td (New answer) Tj ET Q EMC");
    let mut staging = Document::new();
    let mut budget = FillAppearanceBudget::new(&source, &native, &mut staging).unwrap();
    let changed = transplant_answer(
        &source,
        &original,
        &native,
        &generated,
        AnswerKind::Text,
        &mut budget,
    )
    .unwrap();
    drop(budget);
    let mut actual = source.clone();
    actual.objects.extend(staging.objects);
    actual.max_id = staging.max_id;
    let mut expected = source;
    let courier =
        expected.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier"});
    let mut expected_ap = original;
    expected_ap
        .dict
        .get_mut(b"Resources")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .get_mut(b"Font")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set("F2", courier);
    expected_ap.set_plain_content(b"BT /F1 10 Tf 5 75 Td (Caption) Tj ET /Tx BMC q 0 0 100 60 re W n BT /F2 12 Tf 5 30 Td (New answer) Tj ET Q EMC 1 0 0 rg 10 25 8 15 re f".to_vec());
    render_pair(
        page(actual, changed.appearance),
        page(expected, expected_ap),
    );
}

#[test]
fn source_text_spacing_does_not_leak_into_the_generated_answer() {
    let mut source = Document::new();
    let mut native = Document::new();
    let original=stream(&mut source,"Helvetica",b"BT 5 Tc 7 Tw 150 Tz 4 Ts ET /Tx BMC q BT /F1 12 Tf 5 30 Td (Old) Tj ET Q EMC 1 0 0 rg 80 80 5 5 re f");
    let generated = stream(
        &mut native,
        "Courier",
        b"/Tx BMC q BT /F1 12 Tf 5 30 Td (New answer) Tj ET Q EMC",
    );
    let mut staging = Document::new();
    let mut budget = FillAppearanceBudget::new(&source, &native, &mut staging).unwrap();
    let changed = transplant_answer(
        &source,
        &original,
        &native,
        &generated,
        AnswerKind::Text,
        &mut budget,
    )
    .unwrap();
    drop(budget);
    let mut actual = source;
    actual.objects.extend(staging.objects);
    actual.max_id = staging.max_id;
    let mut expected = native;
    let expected_ap = stream(
        &mut expected,
        "Courier",
        b"/Tx BMC q BT /F1 12 Tf 5 30 Td (New answer) Tj ET Q EMC 1 0 0 rg 80 80 5 5 re f",
    );
    render_pair(
        page(actual, changed.appearance),
        page(expected, expected_ap),
    );
}

#[test]
fn repeated_linked_imports_reuse_one_native_font_graph() {
    let mut source = Document::new();
    let mut native = Document::new();
    let original = stream(
        &mut source,
        "Helvetica",
        b"/Tx BMC q BT /F1 12 Tf (Old) Tj ET Q EMC",
    );
    let generated = stream(
        &mut native,
        "Courier",
        b"/Tx BMC q BT /F1 12 Tf (New) Tj ET Q EMC",
    );
    let mut staging = Document::new();
    let mut budget = FillAppearanceBudget::new(&source, &native, &mut staging).unwrap();
    for _ in 0..30 {
        transplant_answer(
            &source,
            &original,
            &native,
            &generated,
            AnswerKind::Text,
            &mut budget,
        )
        .unwrap();
    }
    drop(budget);
    assert_eq!(
        staging
            .objects
            .values()
            .filter(|v| v
                .as_dict()
                .ok()
                .and_then(|d| d.get(b"Type").ok())
                .and_then(|o| o.as_name().ok())
                == Some(b"Font"))
            .count(),
        1
    );
    assert_eq!(source.objects.len(), 1);
    assert_eq!(native.objects.len(), 1);
}

#[test]
fn implicit_default_color_spaces_stay_bound_to_their_own_paint() {
    for source_default in [true, false] {
        let mut source = Document::new();
        let mut native = Document::new();
        let mut original = stream(
            &mut source,
            "Helvetica",
            b"q 0.5 0.2 0.2 rg 0 0 100 100 re f Q /Tx BMC q BT /F1 12 Tf 5 30 Td (Old) Tj ET Q EMC",
        );
        let mut generated = stream(
            &mut native,
            "Helvetica",
            b"/Tx BMC q BT /F1 12 Tf 5 30 Td (New) Tj ET Q EMC",
        );
        let ap = if source_default {
            &mut original
        } else {
            &mut generated
        };
        ap.dict.get_mut(b"Resources").unwrap().as_dict_mut().unwrap().set("ColorSpace",dictionary!{"DefaultRGB"=>vec![Object::Name(b"CalRGB".to_vec()),dictionary!{"WhitePoint"=>vec![Object::Real(0.95),Object::Integer(1),Object::Real(1.09)],"Gamma"=>vec![Object::Real(2.2),Object::Integer(1),Object::Integer(1)]}.into()]});
        let mut staging = Document::new();
        let mut budget = FillAppearanceBudget::new(&source, &native, &mut staging).unwrap();
        let changed = transplant_answer(
            &source,
            &original,
            &native,
            &generated,
            AnswerKind::Text,
            &mut budget,
        )
        .unwrap();
        drop(budget);
        let outer = changed
            .appearance
            .dict
            .get(b"Resources")
            .unwrap()
            .as_dict()
            .unwrap();
        assert_eq!(
            outer.has(b"ColorSpace"),
            source_default,
            "default color-space binding changed for retained frame paint"
        );
        let nested = outer
            .get(b"XObject")
            .unwrap()
            .as_dict()
            .unwrap()
            .iter()
            .find(|(name, _)| name.starts_with(b"FolioAnswer_"))
            .unwrap()
            .1;
        let nested = staging
            .get_object(nested.as_reference().unwrap())
            .unwrap()
            .as_stream()
            .unwrap();
        assert_eq!(
            nested
                .dict
                .get(b"Resources")
                .unwrap()
                .as_dict()
                .unwrap()
                .has(b"ColorSpace"),
            !source_default,
            "default color-space binding changed for generated answer paint"
        );
    }
}

#[test]
fn compressed_appearance_expansion_is_refused_before_transplantation() {
    let mut source = Document::new();
    let mut native = Document::new();
    let original = stream(&mut source, "Helvetica", b"/Tx BMC q Q EMC");
    let mut generated = stream(&mut native, "Courier", &vec![b' '; MAX_BYTES + 1]);
    generated.compress().unwrap();
    let mut staging = Document::new();
    let mut budget = FillAppearanceBudget::new(&source, &native, &mut staging).unwrap();
    assert!(transplant_answer(
        &source,
        &original,
        &native,
        &generated,
        AnswerKind::Text,
        &mut budget
    )
    .is_err());
    let valid = stream(&mut Document::new(), "Courier", b"/Tx BMC q Q EMC");
    assert!(transplant_answer(
        &source,
        &original,
        &native,
        &valid,
        AnswerKind::Text,
        &mut budget
    )
    .is_err());
    drop(budget);
    assert!(staging.objects.is_empty());
}

#[test]
fn generated_resource_names_must_resolve_locally() {
    for operation in [
        "BT /Missing 12 Tf (New) Tj ET",
        "/Missing gs",
        "/Missing Do",
        "/Missing sh",
    ] {
        let mut source = Document::new();
        let mut native = Document::new();
        let original = stream(&mut source, "Helvetica", b"/Tx BMC q Q EMC");
        let mut generated = stream(
            &mut native,
            "Courier",
            format!("/Tx BMC q {operation} Q EMC").as_bytes(),
        );
        let resources = generated
            .dict
            .get_mut(b"Resources")
            .unwrap()
            .as_dict_mut()
            .unwrap();
        for category in ["ExtGState", "XObject", "Shading"] {
            resources.set(category, Dictionary::new());
        }
        let mut staging = Document::new();
        let mut budget = FillAppearanceBudget::new(&source, &native, &mut staging).unwrap();
        assert!(
            transplant_answer(
                &source,
                &original,
                &native,
                &generated,
                AnswerKind::Text,
                &mut budget
            )
            .is_err(),
            "unbound instruction {operation} accepted"
        );
    }
}

#[test]
fn rotated_nonzero_bounds_map_generated_answer_into_the_original_appearance() {
    for (matrix, mapping) in [
        ([1, 0, 0, 1, -10, -20], "0.5 0 0 1 10 20"),
        ([0, 1, -1, 0, 120, -10], "0 -0.5 1 0 10 120"),
        ([-1, 0, 0, -1, 110, 120], "-0.5 0 0 -1 110 120"),
        ([0, -1, 1, 0, -20, 110], "0 0.5 -1 0 110 20"),
    ] {
        let mut source = Document::new();
        let mut native = Document::new();
        let mut original = stream(
            &mut source,
            "Helvetica",
            b"/Tx BMC q 0 g 20 30 10 10 re f Q EMC 1 0 0 rg 90 100 8 8 re f",
        );
        let bbox = source.add_object(Object::Array(vec![
            10.into(),
            20.into(),
            110.into(),
            120.into(),
        ]));
        let matrix = source.add_object(Object::Array(
            matrix.into_iter().map(Object::Integer).collect(),
        ));
        original.dict.set("BBox", bbox);
        original.dict.set("Matrix", matrix);
        let mut generated = stream(
            &mut native,
            "Courier",
            b"/Tx BMC q 0 g 20 20 40 20 re f Q EMC",
        );
        generated
            .dict
            .set("BBox", vec![0.into(), 0.into(), 200.into(), 100.into()]);
        let mut staging = Document::new();
        let mut budget = FillAppearanceBudget::new(&source, &native, &mut staging).unwrap();
        let changed = transplant_answer(
            &source,
            &original,
            &native,
            &generated,
            AnswerKind::Text,
            &mut budget,
        )
        .unwrap();
        drop(budget);
        assert_eq!(
            changed.appearance.dict.get(b"BBox").unwrap(),
            original.dict.get(b"BBox").unwrap()
        );
        assert_eq!(
            changed.appearance.dict.get(b"Matrix").unwrap(),
            original.dict.get(b"Matrix").unwrap()
        );
        let mut actual = source.clone();
        actual.objects.extend(staging.objects);
        actual.max_id = staging.max_id;
        let mut expected_ap = original;
        expected_ap.set_plain_content(
            format!("/Tx BMC q {mapping} cm 0 g 20 20 40 20 re f Q EMC 1 0 0 rg 90 100 8 8 re f")
                .into_bytes(),
        );
        let actual = page(actual, changed.appearance);
        let expected = page(source, expected_ap);
        for unit in [1., 1.7] {
            render_pair(
                with_user_unit(&actual, unit),
                with_user_unit(&expected, unit),
            );
        }
    }
}

#[test]
fn inherited_transparency_is_reset_for_answer_and_retained_for_artwork() {
    let mut source = Document::new();
    let mut native = Document::new();
    let mut original = stream(
        &mut source,
        "Helvetica",
        b"/Fade gs /Tx BMC q BT /F1 12 Tf 5 30 Td (Old) Tj ET Q EMC 1 0 0 rg 80 80 5 5 re f",
    );
    original
        .dict
        .get_mut(b"Resources")
        .unwrap()
        .as_dict_mut()
        .unwrap()
        .set(
            "ExtGState",
            dictionary! {"Fade"=>dictionary!{"Type"=>"ExtGState","ca"=>0.25,"CA"=>0.25}},
        );
    let generated = stream(
        &mut native,
        "Helvetica",
        b"/Tx BMC q BT /F1 12 Tf 5 30 Td (New) Tj ET Q EMC",
    );
    let mut staging = Document::new();
    let mut budget = FillAppearanceBudget::new(&source, &native, &mut staging).unwrap();
    let changed = transplant_answer(
        &source,
        &original,
        &native,
        &generated,
        AnswerKind::Text,
        &mut budget,
    )
    .unwrap();
    drop(budget);
    let mut actual = source.clone();
    actual.objects.extend(staging.objects);
    actual.max_id = staging.max_id;
    let mut expected = original;
    expected.set_plain_content(
        b"/Tx BMC q BT /F1 12 Tf 5 30 Td (New) Tj ET Q EMC /Fade gs 1 0 0 rg 80 80 5 5 re f"
            .to_vec(),
    );
    render_pair(page(actual, changed.appearance), page(source, expected));
}

#[test]
fn answer_budget_rejects_reuse_for_a_different_document() {
    let mut source = Document::new();
    let mut native = Document::new();
    let original = stream(&mut source, "Helvetica", b"/Tx BMC q Q EMC");
    let generated = stream(&mut native, "Courier", b"/Tx BMC q Q EMC");
    let other = native.clone();
    let mut staging = Document::new();
    let mut budget = FillAppearanceBudget::new(&source, &native, &mut staging).unwrap();
    assert!(transplant_answer(
        &source,
        &original,
        &other,
        &generated,
        AnswerKind::Text,
        &mut budget
    )
    .is_err());
}

#[test]
fn ordinary_unmarked_choice_background_survives_answer_replacement() {
    for kind in [AnswerKind::Combo, AnswerKind::List] {
        let mut source = Document::new();
        let mut native = Document::new();
        let original = stream(
            &mut source,
            "Helvetica",
            b"q 0.8 g 0 0 100 100 re f Q 0 0 100 100 re S BT /F1 12 Tf 5 30 Td (Old) Tj ET",
        );
        let generated = stream(
            &mut native,
            "Helvetica",
            b"/Tx BMC q 0 0 1 rg 2 25 96 16 re f 0 g BT /F1 12 Tf 5 30 Td (New) Tj ET Q EMC",
        );
        let mut staging = Document::new();
        let mut budget = FillAppearanceBudget::new(&source, &native, &mut staging).unwrap();
        let changed =
            transplant_answer(&source, &original, &native, &generated, kind, &mut budget).unwrap();
        drop(budget);
        let mut actual = source.clone();
        actual.objects.extend(staging.objects);
        actual.max_id = staging.max_id;
        let mut expected = original;
        expected.set_plain_content(b"q 0.8 g 0 0 100 100 re f Q 0 0 100 100 re S /Tx BMC q 0 0 1 rg 2 25 96 16 re f 0 g BT /F1 12 Tf 5 30 Td (New) Tj ET Q EMC".to_vec());
        render_pair(page(actual, changed.appearance), page(source, expected));
    }
}

#[test]
fn partial_or_clipped_unmarked_choice_paint_is_not_assumed_to_be_background() {
    for prefix in [
        "0.8 g 0 0 100 16 re f",
        "q 0 0 100 16 re W n 0.8 g 0 0 100 100 re f Q",
    ] {
        let mut source = Document::new();
        let mut native = Document::new();
        let original = stream(
            &mut source,
            "Helvetica",
            format!("{prefix} BT /F1 12 Tf 5 30 Td (Old) Tj ET").as_bytes(),
        );
        let generated = stream(
            &mut native,
            "Helvetica",
            b"/Tx BMC q BT /F1 12 Tf (New) Tj ET Q EMC",
        );
        let mut staging = Document::new();
        let mut budget = FillAppearanceBudget::new(&source, &native, &mut staging).unwrap();
        assert!(transplant_answer(
            &source,
            &original,
            &native,
            &generated,
            AnswerKind::List,
            &mut budget
        )
        .is_err());
    }
}

#[test]
fn linked_appearance_imports_cannot_exceed_the_aggregate_budget() {
    let mut source = Document::new();
    let mut native = Document::new();
    let original = stream(&mut source, "Helvetica", b"/Tx BMC q Q EMC");
    let mut generated = Vec::new();
    for _ in 0..4 {
        let program = native.add_object(Stream::new(Dictionary::new(), vec![0; 20 * 1024 * 1024]));
        let descriptor =
            native.add_object(dictionary! {"Type"=>"FontDescriptor","FontFile2"=>program});
        let font = native.add_object(
            dictionary! {"Type"=>"Font","Subtype"=>"TrueType","FontDescriptor"=>descriptor},
        );
        generated.push(Stream::new(dictionary!{"BBox"=>vec![0.into(),0.into(),100.into(),100.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}},b"/Tx BMC q BT /F1 12 Tf (New) Tj ET Q EMC".to_vec()));
    }
    let mut staging = Document::new();
    let mut budget = FillAppearanceBudget::new(&source, &native, &mut staging).unwrap();
    for appearance in &generated[..3] {
        transplant_answer(
            &source,
            &original,
            &native,
            appearance,
            AnswerKind::Text,
            &mut budget,
        )
        .unwrap();
    }
    assert!(transplant_answer(
        &source,
        &original,
        &native,
        &generated[3],
        AnswerKind::Text,
        &mut budget
    )
    .is_err());
    assert!(transplant_answer(
        &source,
        &original,
        &native,
        &generated[0],
        AnswerKind::Text,
        &mut budget
    )
    .is_err());
    drop(budget);
    let retained: usize = staging
        .objects
        .values()
        .filter_map(|v| v.as_stream().ok())
        .map(|s| s.content.len())
        .sum();
    assert!(retained < 64 * 1024 * 1024);
}

#[test]
fn empty_generated_answer_does_not_require_unused_font_resources() {
    let mut source = Document::new();
    let native = Document::new();
    let original = stream(
        &mut source,
        "Helvetica",
        b"/Tx BMC q BT /F1 12 Tf (Old) Tj ET Q EMC 1 0 0 rg 80 80 5 5 re f",
    );
    let generated = Stream::new(
        dictionary! {"BBox"=>vec![0.into(),0.into(),100.into(),100.into()]},
        b"/Tx BMC q Q EMC".to_vec(),
    );
    let mut staging = Document::new();
    let mut budget = FillAppearanceBudget::new(&source, &native, &mut staging).unwrap();
    let changed = transplant_answer(
        &source,
        &original,
        &native,
        &generated,
        AnswerKind::Text,
        &mut budget,
    )
    .unwrap();
    drop(budget);
    let mut actual = source.clone();
    actual.objects.extend(staging.objects);
    actual.max_id = staging.max_id;
    let mut expected = original;
    expected.set_plain_content(b"/Tx BMC q Q EMC 1 0 0 rg 80 80 5 5 re f".to_vec());
    render_pair(page(actual, changed.appearance), page(source, expected));
}

#[test]
fn oversized_encoded_operands_are_refused_before_serialization() {
    let op = Operation::new(
        "Tj",
        vec![Object::string_literal(vec![b'\\'; MAX_BYTES / 2])],
    );
    assert!(encoded_bound(&[op]).is_err());
    let mut source = Document::new();
    let mut native = Document::new();
    let original = stream(&mut source, "Helvetica", b"/Tx BMC q Q EMC");
    let generated = stream(&mut native, "Courier", b"/Tx BMC q Q EMC");
    let mut staging = Document::new();
    staging.add_object(Object::Integer(1));
    assert!(
        FillAppearanceBudget::new(&source, &native, &mut staging).is_err(),
        "preexisting object graph was adopted"
    );
    assert_eq!(original.content, b"/Tx BMC q Q EMC");
    assert_eq!(generated.content, original.content);
}

#[test]
fn unmarked_text_keeps_color_effects_for_following_artwork_in_native_pixels() {
    let mut source = Document::new();
    let mut native = Document::new();
    let original = stream(
        &mut source,
        "Helvetica",
        b"BT /F1 12 Tf 1 0 0 rg 5 30 Td (Old) Tj ET 80 80 5 5 re f",
    );
    let generated = stream(
        &mut native,
        "Helvetica",
        b"/Tx BMC q BT /F1 12 Tf 5 30 Td (New) Tj ET Q EMC",
    );
    let mut staging = Document::new();
    let mut budget = FillAppearanceBudget::new(&source, &native, &mut staging).unwrap();
    let changed = transplant_answer(
        &source,
        &original,
        &native,
        &generated,
        AnswerKind::Text,
        &mut budget,
    )
    .unwrap();
    drop(budget);
    let mut actual = source.clone();
    actual.objects.extend(staging.objects);
    actual.max_id = staging.max_id;
    let mut expected = original;
    expected.set_plain_content(
        b"/Tx BMC q BT /F1 12 Tf 5 30 Td (New) Tj ET Q EMC 1 0 0 rg 80 80 5 5 re f".to_vec(),
    );
    render_pair(page(actual, changed.appearance), page(source, expected));
}
