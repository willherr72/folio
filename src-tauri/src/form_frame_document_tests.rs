use super::*;
use lopdf::{dictionary, ObjectId, Stream};

const PROGRAM:&str="q .85 g 0 0 100 30 re f Q q 0 g 0 0 100 30 re 1 1 98 28 re f* Q /Tx BMC q BT /F1 12 Tf 1 0 0 1 4 12 Tm (Answer) Tj ET Q EMC q 1 0 0 rg 80 10 4 4 re f Q";
fn fixture(program: &str) -> (Document, [ObjectId; 2]) {
    let mut d = Document::with_version("1.7");
    let pages = d.new_object_id();
    let page = d.new_object_id();
    let parent = d.new_object_id();
    let font =
        d.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let stream=d.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),100.into(),30.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}},program.as_bytes().to_vec()));
    let ap = d.add_object(dictionary! {"N"=>stream});
    let mk = d.add_object(dictionary! {"BC"=>vec![0.into()],"BG"=>vec![0.85.into()]});
    let bs = d.add_object(dictionary! {"W"=>1,"S"=>"S"});
    let ids = std::array::from_fn(|i| {
        d.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","Parent"=>parent,"P"=>page,"F"=>4,"Rect"=>vec![20.into(),((40+i*60) as i64).into(),120.into(),((70+i*60) as i64).into()],"AP"=>ap,"MK"=>mk,"BS"=>bs})
    });
    d.objects.insert(parent,dictionary!{"FT"=>"Tx","T"=>Object::string_literal("shared"),"V"=>Object::string_literal("Answer"),"DV"=>Object::string_literal("Default"),"DA"=>Object::string_literal("/F1 12 Tf 0 g"),"Kids"=>ids.iter().map(|id|Object::Reference(*id)).collect::<Vec<_>>()}.into());
    d.objects.insert(page,dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),200.into(),200.into()],"Annots"=>ids.iter().map(|id|Object::Reference(*id)).collect::<Vec<_>>()}.into());
    d.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into(),
    );
    let root=d.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages,"AcroForm"=>dictionary!{"Fields"=>vec![parent.into()],"NeedAppearances"=>false,"DR"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}}});
    d.trailer.set("Root", root);
    (d, ids)
}
fn save(mut d: Document) -> Vec<u8> {
    crate::pdf_output::save(&mut d).unwrap()
}
fn changed() -> FormFrameChanges {
    FormFrameChanges {
        width: Some(2.25),
        border: Some(FrameColorChange::Rgb("#0033CC".into())),
        ..Default::default()
    }
}
fn normal<'a>(d: &'a Document, id: ObjectId) -> &'a Stream {
    let w = d.get_dictionary(id).unwrap();
    let ap = resolve(d, w.get(b"AP").unwrap())
        .unwrap()
        .as_dict()
        .unwrap();
    resolve(d, ap.get(b"N").unwrap())
        .unwrap()
        .as_stream()
        .unwrap()
}
#[test]
fn selected_widget_aliases_and_shared_answer_are_preserved() {
    let (d, ids) = fixture(PROGRAM);
    let source = save(d);
    let info = inspect(&source, 0, 0).unwrap();
    assert_eq!(info.width, 1.);
    assert_eq!(info.style, Some(FrameStyle::Solid));
    let output = change(&source, 0, 0, &changed()).unwrap();
    let before = crate::pdf_output::load(&source).unwrap();
    let after = crate::pdf_output::load(&output).unwrap();
    assert_eq!(
        after.get_dictionary(ids[1]).unwrap(),
        before.get_dictionary(ids[1]).unwrap()
    );
    assert_eq!(
        normal(&after, ids[1]).content,
        normal(&before, ids[1]).content
    );
    let parent = before
        .get_dictionary(ids[0])
        .unwrap()
        .get(b"Parent")
        .unwrap()
        .as_reference()
        .unwrap();
    assert_eq!(
        before.get_dictionary(parent).unwrap(),
        after.get_dictionary(parent).unwrap()
    );
    assert_eq!(inspect(&output, 0, 0).unwrap().width, 2.25);
    assert_eq!(inspect(&output, 0, 1).unwrap().width, 1.);
    assert!(
        String::from_utf8(normal(&after, ids[0]).get_plain_content().unwrap())
            .unwrap()
            .contains("(Answer) Tj")
    );
    assert_ne!(inspect(&output, 0, 0).unwrap().style_key, info.style_key);
}
#[test]
fn actual_paint_width_survives_absent_or_conflicting_defaults() {
    let (mut d, ids) = fixture(PROGRAM);
    d.get_dictionary_mut(ids[0])
        .unwrap()
        .set("BS", dictionary! {"W"=>7,"S"=>"U"});
    let source = save(d);
    let info = inspect(&source, 0, 0).unwrap();
    assert_eq!(info.width, 1.);
    assert_eq!(info.style, Some(FrameStyle::Solid));
    let output = change(
        &source,
        0,
        0,
        &FormFrameChanges {
            background: Some(FrameColorChange::Rgb("#FFF0CC".into())),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(inspect(&output, 0, 0).unwrap().width, 1.);
    assert_eq!(
        inspect(&output, 0, 0).unwrap().style,
        Some(FrameStyle::Solid)
    );
    let d = crate::pdf_output::load(&output).unwrap();
    assert_eq!(
        resolve(&d, d.get_dictionary(ids[0]).unwrap().get(b"BS").unwrap())
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"W")
            .unwrap()
            .as_i64()
            .unwrap(),
        7
    );
}
#[test]
fn legacy_border_is_reconciled_only_for_explicit_border_changes() {
    let (mut d, ids) = fixture(PROGRAM);
    d.get_dictionary_mut(ids[0])
        .unwrap()
        .set("Border", vec![0.into(), 0.into(), 7.into()]);
    let source = save(d);
    let output = change(&source, 0, 0, &changed()).unwrap();
    let d = crate::pdf_output::load(&output).unwrap();
    assert!(!d.get_dictionary(ids[0]).unwrap().has(b"Border"));
    assert_eq!(inspect(&output, 0, 0).unwrap().width, 2.25);
}
#[test]
fn missing_global_and_malformed_appearances_refuse_without_mutation() {
    for case in 0..3 {
        let (mut d, ids) = fixture(PROGRAM);
        if case == 0 {
            d.get_dictionary_mut(ids[0]).unwrap().remove(b"AP");
        }
        if case == 1 {
            let root = d.trailer.get(b"Root").unwrap().as_reference().unwrap();
            d.get_dictionary_mut(root)
                .unwrap()
                .get_mut(b"AcroForm")
                .unwrap()
                .as_dict_mut()
                .unwrap()
                .set("NeedAppearances", true);
        }
        if case == 2 {
            let ap = d
                .get_dictionary(ids[0])
                .unwrap()
                .get(b"AP")
                .unwrap()
                .as_reference()
                .unwrap();
            d.get_dictionary_mut(ap)
                .unwrap()
                .set("D", dictionary! {"Bad"=>Object::Null});
        }
        let source = save(d);
        let copy = source.clone();
        assert!(change(&source, 0, 0, &changed()).is_err());
        assert_eq!(source, copy);
    }
}
#[test]
fn appearance_scaling_and_quarter_turns_use_raw_page_width() {
    for rotated in [false, true] {
        let (mut d, ids) = fixture(PROGRAM);
        let ap = d
            .get_dictionary(ids[0])
            .unwrap()
            .get(b"AP")
            .unwrap()
            .as_reference()
            .unwrap();
        let stream = d
            .get_dictionary(ap)
            .unwrap()
            .get(b"N")
            .unwrap()
            .as_reference()
            .unwrap();
        d.get_dictionary_mut(ids[0]).unwrap().set(
            "Rect",
            if rotated {
                vec![0.into(), 0.into(), 60.into(), 200.into()]
            } else {
                vec![0.into(), 0.into(), 200.into(), 60.into()]
            },
        );
        if rotated {
            d.get_object_mut(stream)
                .unwrap()
                .as_stream_mut()
                .unwrap()
                .dict
                .set(
                    "Matrix",
                    vec![
                        0.into(),
                        1.into(),
                        (-1).into(),
                        0.into(),
                        30.into(),
                        0.into(),
                    ],
                );
        }
        let source = save(d);
        assert_eq!(inspect(&source, 0, 0).unwrap().width, 2.);
        let output = change(
            &source,
            0,
            0,
            &FormFrameChanges {
                width: Some(3.),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(inspect(&output, 0, 0).unwrap().width, 3.);
        let after = crate::pdf_output::load(&output).unwrap();
        let s = normal(&after, ids[0]);
        let proof = crate::form_frame_paint::FramePaint::inspect(
            &s.get_plain_content().unwrap(),
            [0., 0., 100., 30.],
            false,
        )
        .unwrap();
        assert_eq!(proof.width, 1.5);
    }
}
#[test]
fn originally_borderless_text_can_gain_and_remove_a_frame() {
    let program = "q 0 0 100 30 re W n /Tx BMC BT /F1 12 Tf (Answer) Tj ET EMC Q";
    let (mut d, ids) = fixture(program);
    d.get_dictionary_mut(ids[0])
        .unwrap()
        .set("BS", dictionary! {"W"=>0});
    d.get_dictionary_mut(ids[0])
        .unwrap()
        .set("MK", dictionary! {});
    let source = save(d);
    assert_eq!(inspect(&source, 0, 0).unwrap().width, 0.);
    let output = change(&source, 0, 0, &changed()).unwrap();
    assert_eq!(inspect(&output, 0, 0).unwrap().width, 2.25);
    let output = change(
        &output,
        0,
        0,
        &FormFrameChanges {
            border: Some(FrameColorChange::Transparent),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        inspect(&output, 0, 0).unwrap().border.color_space,
        "transparent"
    );
}
#[test]
fn repeated_publication_prunes_only_unreachable_old_appearances() {
    let (d, ids) = fixture(PROGRAM);
    let mut bytes = change(&save(d), 0, 0, &changed()).unwrap();
    let count = crate::pdf_output::load(&bytes).unwrap().objects.len();
    for i in 0..12 {
        bytes = change(
            &bytes,
            0,
            0,
            &FormFrameChanges {
                width: Some(1. + (i % 3) as f32 / 4.),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            crate::pdf_output::load(&bytes).unwrap().objects.len(),
            count
        );
    }
    let d = crate::pdf_output::load(&bytes).unwrap();
    assert_eq!(
        normal(&d, ids[1]).get_plain_content().unwrap(),
        PROGRAM.as_bytes()
    );
}

#[test]
fn unchanged_visible_settings_return_original_bytes_even_with_conflicting_defaults() {
    let (mut d, ids) = fixture(PROGRAM);
    d.get_dictionary_mut(ids[0])
        .unwrap()
        .set("BS", dictionary! {"W"=>7,"S"=>"U"});
    let source = save(d);
    let output = change(
        &source,
        0,
        0,
        &FormFrameChanges {
            width: Some(1.),
            style: Some(FrameStyle::Solid),
            border: Some(FrameColorChange::Rgb("#000000".into())),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        output == source,
        "unchanged visible settings must not rewrite the document"
    );
}

#[test]
fn dashed_width_edits_preserve_nonzero_source_phase() {
    let program = "q .85 g 0 0 100 30 re f Q q 0 G 1 w [5 2] 1.5 d .5 .5 99 29 re S Q BT /F1 12 Tf (Answer) Tj ET";
    let (d, ids) = fixture(program);
    let source = save(d);
    let output = change(
        &source,
        0,
        0,
        &FormFrameChanges {
            width: Some(2.),
            ..Default::default()
        },
    )
    .unwrap();
    let d = crate::pdf_output::load(&output).unwrap();
    let p = FramePaint::inspect(
        &normal(&d, ids[0]).get_plain_content().unwrap(),
        [0., 0., 100., 30.],
        false,
    )
    .unwrap();
    assert_eq!(p.style, PaintStyle::Dashed(vec![5., 2.], 1.5));
    assert_eq!(p.width, 2.);
}

#[test]
fn ambiguous_compound_style_keeps_original_edge_palette_on_background_edit() {
    let samples: serde_json::Value = serde_json::from_str(include_str!(
        "../tests/fixtures/form-frame-compound-programs.json"
    ))
    .unwrap();
    let program = samples[0]["program"]
        .as_str()
        .unwrap()
        .replace("1 g", ".2 g")
        .replace("0.425 g", ".7 g");
    let (mut d, ids) = fixture(&program);
    let ap = d
        .get_dictionary(ids[0])
        .unwrap()
        .get(b"AP")
        .unwrap()
        .as_reference()
        .unwrap();
    let sid = d
        .get_dictionary(ap)
        .unwrap()
        .get(b"N")
        .unwrap()
        .as_reference()
        .unwrap();
    d.get_object_mut(sid)
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .dict
        .set("BBox", vec![0.into(), 0.into(), 280.into(), 50.into()]);
    d.get_dictionary_mut(ids[0])
        .unwrap()
        .set("Rect", vec![0.into(), 0.into(), 280.into(), 50.into()]);
    let source = save(d);
    assert_eq!(inspect(&source, 0, 0).unwrap().style, None);
    let output = change(
        &source,
        0,
        0,
        &FormFrameChanges {
            background: Some(FrameColorChange::Rgb("#CCCCCC".into())),
            ..Default::default()
        },
    )
    .unwrap();
    let d = crate::pdf_output::load(&output).unwrap();
    let p = FramePaint::inspect(
        &normal(&d, ids[0]).get_plain_content().unwrap(),
        [0., 0., 280., 50.],
        false,
    )
    .unwrap();
    let before = FramePaint::inspect(program.as_bytes(), [0., 0., 280., 50.], false).unwrap();
    assert_eq!(p.edge_colors, before.edge_colors);
    assert_eq!(inspect(&output, 0, 0).unwrap().style, None);
}

fn checkbox_fixture(style: &str) -> (Document, [ObjectId; 2]) {
    let samples: serde_json::Value = serde_json::from_str(include_str!(
        "../tests/fixtures/form-frame-compound-programs.json"
    ))
    .unwrap();
    let (mut d, ids) = fixture(PROGRAM);
    let ap = d
        .get_dictionary(ids[0])
        .unwrap()
        .get(b"AP")
        .unwrap()
        .as_reference()
        .unwrap();
    let parent = d
        .get_dictionary(ids[0])
        .unwrap()
        .get(b"Parent")
        .unwrap()
        .as_reference()
        .unwrap();
    let template = normal(&d, ids[0]).dict.clone();
    let mut modes = Dictionary::new();
    for mode in ["N", "D"] {
        let mut states = Dictionary::new();
        for name in ["Off", "Yes"] {
            let key = format!("checkbox-{style}/{mode}/{name}");
            let sample = samples
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["case"] == key)
                .unwrap();
            let mut dict = template.clone();
            dict.set("BBox", vec![0.into(), 0.into(), 280.into(), 50.into()]);
            let sid = d.add_object(Stream::new(
                dict,
                sample["program"].as_str().unwrap().as_bytes().to_vec(),
            ));
            states.set(name, sid);
        }
        modes.set(mode, states);
    }
    // R aliases N, and both widgets alias the entire AP dictionary.
    modes.set("R", modes.get(b"N").unwrap().clone());
    d.objects.insert(ap, Object::Dictionary(modes));
    let p = d.get_dictionary_mut(parent).unwrap();
    p.set("FT", "Btn");
    p.set("V", "Yes");
    p.set("DV", "Off");
    for (i, id) in ids.iter().enumerate() {
        let w = d.get_dictionary_mut(*id).unwrap();
        w.set("AS", "Yes");
        w.set(
            "Rect",
            vec![
                0.into(),
                ((i * 60) as i64).into(),
                280.into(),
                ((i * 60 + 50) as i64).into(),
            ],
        );
        w.set("BS", dictionary! {"W"=>1.25,"S"=>style});
    }
    (d, ids)
}
fn state_stream<'a>(d: &'a Document, id: ObjectId, mode: &[u8], name: &[u8]) -> &'a Stream {
    let ap = dictionary(d, d.get_dictionary(id).unwrap(), b"AP")
        .unwrap()
        .unwrap();
    let states = dictionary(d, ap, mode).unwrap().unwrap();
    resolve(d, states.get(name).unwrap())
        .unwrap()
        .as_stream()
        .unwrap()
}
#[test]
fn all_checkbox_states_keep_pressed_shading_relationship_and_shared_widget() {
    for style in ["B", "I"] {
        let (d, ids) = checkbox_fixture(style);
        let source = save(d);
        let output = change(
            &source,
            0,
            0,
            &FormFrameChanges {
                width: Some(2.),
                background: Some(FrameColorChange::Rgb("#CCCCCC".into())),
                ..Default::default()
            },
        )
        .unwrap();
        let before = crate::pdf_output::load(&source).unwrap();
        let after = crate::pdf_output::load(&output).unwrap();
        assert_eq!(
            before.get_dictionary(ids[1]).unwrap(),
            after.get_dictionary(ids[1]).unwrap()
        );
        for mode in [b"N", b"R", b"D"] {
            for name in [b"Off".as_slice(), b"Yes"] {
                let old = state_stream(&before, ids[0], mode, name)
                    .get_plain_content()
                    .unwrap();
                let new = state_stream(&after, ids[0], mode, name)
                    .get_plain_content()
                    .unwrap();
                assert_eq!(
                    old,
                    state_stream(&after, ids[1], mode, name)
                        .get_plain_content()
                        .unwrap()
                );
                let old = FramePaint::inspect(&old, [0., 0., 280., 50.], false).unwrap();
                let new = FramePaint::inspect(&new, [0., 0., 280., 50.], false).unwrap();
                assert_eq!(new.width, 2.);
                assert!(same_color(
                    new.background_color.as_ref(),
                    Some(&PaintColor::Rgb([if mode == b"D" { 0.55 } else { 0.8 }; 3]))
                ));
                let shades = if style == "B" {
                    if mode == b"D" {
                        [0.4, 1.]
                    } else {
                        [1., 0.4]
                    }
                } else if mode == b"D" {
                    [0., 1.]
                } else {
                    [0.5, 0.75]
                };
                for (color, expected) in new.edge_colors.as_ref().unwrap().iter().zip(shades) {
                    assert!(
                        same_color(Some(color), Some(&PaintColor::Gray(expected))),
                        "{style} {mode:?}: {color:?}"
                    );
                }
                // Retain the entire answer/decoration suffix after frame paint.
                let tail = |p: &FramePaint| {
                    lopdf::content::Content {
                        operations: p.content.operations[p.border + 1..].to_vec(),
                    }
                    .encode()
                    .unwrap()
                };
                assert_eq!(tail(&old), tail(&new));
            }
        }
        let ap = dictionary(&after, after.get_dictionary(ids[0]).unwrap(), b"AP")
            .unwrap()
            .unwrap();
        assert_eq!(
            ap.get(b"N").unwrap(),
            ap.get(b"R").unwrap(),
            "equivalent aliases should stay shared"
        );
    }
}
#[test]
fn unknown_pressed_palette_refuses_color_change_but_preserves_width_only_edit() {
    let (mut d, ids) = checkbox_fixture("B");
    let ap = dictionary(&d, d.get_dictionary(ids[0]).unwrap(), b"AP")
        .unwrap()
        .unwrap();
    let down = dictionary(&d, ap, b"D").unwrap().unwrap();
    let sids = down
        .iter()
        .map(|(_, v)| v.as_reference().unwrap())
        .collect::<Vec<_>>();
    for sid in sids {
        let s = d.get_object_mut(sid).unwrap().as_stream_mut().unwrap();
        s.set_content(
            String::from_utf8(s.content.clone())
                .unwrap()
                .replace("0.6 g", "0.3 g")
                .into_bytes(),
        );
    }
    let source = save(d);
    assert!(change(
        &source,
        0,
        0,
        &FormFrameChanges {
            background: Some(FrameColorChange::Rgb("#CCCCCC".into())),
            ..Default::default()
        }
    )
    .is_err());
    let output = change(
        &source,
        0,
        0,
        &FormFrameChanges {
            width: Some(2.),
            ..Default::default()
        },
    )
    .unwrap();
    let after = crate::pdf_output::load(&output).unwrap();
    let p = FramePaint::inspect(
        &state_stream(&after, ids[0], b"D", b"Yes")
            .get_plain_content()
            .unwrap(),
        [0., 0., 280., 50.],
        false,
    )
    .unwrap();
    assert!(same_color(
        p.background_color.as_ref(),
        Some(&PaintColor::Gray(0.3))
    ));
}

#[test]
fn temporarily_hidden_dashes_keep_pattern_and_phase_when_restored() {
    let program = "q 0 G 1 w [5 2] 1.5 d .5 .5 99 29 re S Q BT /F1 12 Tf (Answer) Tj ET";
    let (d, ids) = fixture(program);
    let source = save(d);
    let hidden = change(
        &source,
        0,
        0,
        &FormFrameChanges {
            width: Some(0.),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(inspect(&hidden, 0, 0).unwrap().width, 0.);
    let restored = change(
        &hidden,
        0,
        0,
        &FormFrameChanges {
            width: Some(2.),
            ..Default::default()
        },
    )
    .unwrap();
    let d = crate::pdf_output::load(&restored).unwrap();
    let p = FramePaint::inspect(
        &normal(&d, ids[0]).get_plain_content().unwrap(),
        [0., 0., 100., 30.],
        false,
    )
    .unwrap();
    assert_eq!(p.style, PaintStyle::Dashed(vec![5., 2.], 1.5));
}
#[test]
fn original_zero_width_dashed_metadata_survives_border_creation() {
    let (mut d, ids) = fixture("BT /F1 12 Tf (Answer) Tj ET");
    d.get_dictionary_mut(ids[0]).unwrap().set(
        "BS",
        dictionary! {"W"=>0,"S"=>"D","D"=>vec![7.into(),3.into()]},
    );
    let source = save(d);
    let output = change(
        &source,
        0,
        0,
        &FormFrameChanges {
            width: Some(2.),
            ..Default::default()
        },
    )
    .unwrap();
    let d = crate::pdf_output::load(&output).unwrap();
    let p = FramePaint::inspect(
        &normal(&d, ids[0]).get_plain_content().unwrap(),
        [0., 0., 100., 30.],
        false,
    )
    .unwrap();
    assert_eq!(p.style, PaintStyle::Dashed(vec![7., 3.], 0.));
}

#[test]
fn thousands_of_state_aliases_reuse_bounded_programs() {
    let program = format!("%{}\n{PROGRAM}", "a".repeat(128 * 1024));
    let (mut d, ids) = fixture(&program);
    let ap = d
        .get_dictionary(ids[0])
        .unwrap()
        .get(b"AP")
        .unwrap()
        .as_reference()
        .unwrap();
    let sid = d
        .get_dictionary(ap)
        .unwrap()
        .get(b"N")
        .unwrap()
        .as_reference()
        .unwrap();
    let mut states = Dictionary::new();
    for i in 0..3000 {
        states.set(format!("S{i}"), sid);
    }
    d.get_dictionary_mut(ap).unwrap().set("N", states);
    for id in ids {
        d.get_dictionary_mut(id).unwrap().set("AS", "S0");
    }
    let source = save(d);
    let output = change(&source, 0, 0, &changed()).unwrap();
    let after = crate::pdf_output::load(&output).unwrap();
    let ap = dictionary(&after, after.get_dictionary(ids[0]).unwrap(), b"AP")
        .unwrap()
        .unwrap();
    let states = dictionary(&after, ap, b"N").unwrap().unwrap();
    assert_eq!(states.len(), 3000);
    let first = states.get(b"S0").unwrap();
    assert!(states.iter().all(|(_, v)| v == first));
    assert_eq!(
        state_stream(&after, ids[1], b"N", b"S0")
            .get_plain_content()
            .unwrap(),
        program.as_bytes()
    );
}
#[test]
fn unique_large_appearance_states_share_one_retained_byte_budget() {
    let program = format!("%{}\n{PROGRAM}", "a".repeat(3 * 1024 * 1024));
    let (mut d, ids) = fixture(&program);
    let ap = d
        .get_dictionary(ids[0])
        .unwrap()
        .get(b"AP")
        .unwrap()
        .as_reference()
        .unwrap();
    let template = normal(&d, ids[0]).clone();
    let mut states = Dictionary::new();
    for i in 0..12 {
        states.set(format!("S{i}"), d.add_object(template.clone()));
    }
    d.get_dictionary_mut(ap).unwrap().set("N", states);
    for id in ids {
        d.get_dictionary_mut(id).unwrap().set("AS", "S0");
    }
    let source = save(d);
    let error = change(&source, 0, 0, &changed()).unwrap_err().to_string();
    assert!(
        error.contains("budget") || error.contains("67108864"),
        "{error}"
    );
}

#[test]
fn calibrated_source_color_is_not_mistaken_for_an_unchanged_rgb_request() {
    let (mut d, ids) = fixture(PROGRAM);
    let ap = d
        .get_dictionary(ids[0])
        .unwrap()
        .get(b"AP")
        .unwrap()
        .as_reference()
        .unwrap();
    let sid = d
        .get_dictionary(ap)
        .unwrap()
        .get(b"N")
        .unwrap()
        .as_reference()
        .unwrap();
    let resources = d
        .get_object_mut(sid)
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .dict
        .get_mut(b"Resources")
        .unwrap()
        .as_dict_mut()
        .unwrap();
    resources.set("ColorSpace",dictionary!{"DefaultGray"=>vec![Object::Name(b"CalGray".to_vec()),Object::Dictionary(dictionary!{"WhitePoint"=>vec![1.into(),1.into(),1.into()]})]});
    let source = save(d);
    assert!(change(
        &source,
        0,
        0,
        &FormFrameChanges {
            border: Some(FrameColorChange::Rgb("#000000".into())),
            ..Default::default()
        }
    )
    .is_err());
    let output = change(
        &source,
        0,
        0,
        &FormFrameChanges {
            width: Some(2.),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(inspect(&output, 0, 0).unwrap().border.color_space, "gray");
}

#[test]
fn invalid_rollover_stream_subtype_and_geometry_refuse_atomically() {
    for case in 0..3 {
        let (mut d, ids) = fixture(PROGRAM);
        let ap = d
            .get_dictionary(ids[0])
            .unwrap()
            .get(b"AP")
            .unwrap()
            .as_reference()
            .unwrap();
        let mut rollover = normal(&d, ids[0]).clone();
        match case {
            0 => rollover.dict.set("Subtype", "Image"),
            1 => rollover.dict.set(
                "Matrix",
                vec![1.into(), 0.5.into(), 0.into(), 1.into(), 0.into(), 0.into()],
            ),
            _ => rollover
                .dict
                .set("BBox", vec![0.into(), 0.into(), 200.into(), 30.into()]),
        }
        let sid = d.add_object(rollover);
        d.get_dictionary_mut(ap).unwrap().set("R", sid);
        let source = save(d);
        assert!(change(&source, 0, 0, &changed()).is_err(), "case {case}");
    }
}
