use crate::redaction_ranges::*;
use lopdf::{dictionary, Document, Object, Stream};
const SECRET: &str = "Confidential-84912";
fn fixture(program: &str) -> Document {
    let mut d = Document::with_version("1.7");
    let tree = d.new_object_id();
    let font=d.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica","Encoding"=>"WinAnsiEncoding"});
    let content = d.add_object(Stream::new(dictionary! {}, program.as_bytes().to_vec()));
    let page=d.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),500.into(),300.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});
    d.objects.insert(
        tree,
        Object::Dictionary(dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}),
    );
    let root = d.add_object(dictionary! {"Type"=>"Catalog","Pages"=>tree});
    d.trailer.set("Root", root);
    d
}
fn near(a: [f64; 2], b: [f64; 2]) {
    assert!(
        (a[0] - b[0]).abs() < 0.002 && (a[1] - b[1]).abs() < 0.002,
        "{a:?} != {b:?}"
    );
}
#[test]
fn redaction_ranges_trace_spacing_scaling_and_rise() {
    let d = fixture("BT /F1 10 Tf 1 Tc 2 Tw 80 Tz 3 Ts 1 0 0 1 20 150 Tm (A B) Tj ET");
    let b = inspect_standard_glyphs(&d).unwrap();
    assert_eq!(b[0].source.text, "A B");
    near(b[0].glyphs[0].origin, [20., 153.]);
    near(b[0].glyphs[0].end, [26.136, 153.]);
    near(b[0].glyphs[2].origin, [30.76, 153.]);
    assert_eq!(b[0].glyphs[2].offset, 2);
}
#[test]
fn redaction_ranges_trace_native_saved_text_and_line_matrices() {
    let d = fixture(
        "BT /F1 12 Tf 14 TL 1 0 0 1 20 150 Tm (A) Tj q 0 -20 Td (B) Tj Q (C) Tj T* (D) Tj ET",
    );
    let b = inspect_standard_glyphs(&d).unwrap();
    let g = &b[0].glyphs;
    near(g[1].origin, [20., 130.]);
    near(g[2].origin, [28.004, 150.]);
    near(g[3].origin, [20., 136.]);
}
#[test]
fn redaction_ranges_remove_middle_and_keep_every_retained_origin() {
    let d = fixture(&format!(
        "BT /F1 12 Tf 1 0 0 1 20 200 Tm (Left {SECRET} Right) Tj 0 -20 Td (Next line.) Tj ET"
    ));
    let before = inspect_standard_glyphs(&d).unwrap();
    let result = remove_standard_text_ranges(
        &d,
        &[TextRange {
            selection_id: before[0].source.selection_id.clone(),
            start: 5,
            end: 5 + SECRET.len(),
        }],
    )
    .unwrap();
    assert_eq!(result.removed_characters, SECRET.len());
    assert_eq!(result.affected_blocks, 1);
    assert!(!result.omitted_metadata.is_empty());
    let out = Document::load_mem(&result.bytes).unwrap();
    let after = inspect_standard_glyphs(&out).unwrap();
    assert_eq!(after[0].source.text, "Left  RightNext line.");
    let kept: Vec<_> = before[0]
        .glyphs
        .iter()
        .filter(|g| g.offset < 5 || g.offset >= 5 + SECRET.len())
        .collect();
    assert_eq!(kept.len(), after[0].glyphs.len());
    for (a, b) in kept.into_iter().zip(&after[0].glyphs) {
        assert_eq!(a.code, b.code);
        near(a.origin, b.origin);
        near(a.end, b.end);
    }
    assert!(!result
        .bytes
        .windows(SECRET.len())
        .any(|v| v == SECRET.as_bytes()));
}
#[test]
fn redaction_ranges_reject_overlaps_empty_stale_and_outside_offsets() {
    let d = fixture("BT /F1 12 Tf (ABCDE) Tj ET");
    let id = inspect_standard_glyphs(&d).unwrap()[0]
        .source
        .selection_id
        .clone();
    let r = |a, b| TextRange {
        selection_id: id.clone(),
        start: a,
        end: b,
    };
    assert!(remove_standard_text_ranges(&d, &[]).is_err());
    for ranges in [
        vec![r(1, 1)],
        vec![r(2, 1)],
        vec![r(0, 6)],
        vec![r(0, 2), r(1, 3)],
        vec![r(0, 2), r(0, 2)],
    ] {
        assert!(remove_standard_text_ranges(&d, &ranges).is_err());
    }
    assert!(remove_standard_text_ranges(
        &d,
        &[TextRange {
            selection_id: "0".repeat(64),
            start: 0,
            end: 1
        }]
    )
    .is_err());
    let result = remove_standard_text_ranges(&d, &[r(0, 1), r(1, 2)]).unwrap();
    assert_eq!(result.removed_characters, 2);
}

#[test]
fn redaction_ranges_split_large_generated_arrays_and_reject_serialized_drift() {
    let d = fixture(&format!("BT /F1 0.1 Tf ({}Z) Tj ET", "A".repeat(5000)));
    let b = inspect_standard_glyphs(&d).unwrap();
    let out = remove_standard_text_ranges(
        &d,
        &[TextRange {
            selection_id: b[0].source.selection_id.clone(),
            start: 0,
            end: 5000,
        }],
    )
    .unwrap();
    let saved = Document::load_mem(&out.bytes).unwrap();
    let after = inspect_standard_glyphs(&saved).unwrap();
    assert_eq!(after[0].source.text, "Z");
    near(after[0].glyphs[0].origin, b[0].glyphs[5000].origin);
    let d = fixture("1000 0 0 1000 0 0 cm BT /F1 100 Tf 0.123459 Tc (AZ) Tj ET");
    let b = inspect_standard_glyphs(&d).unwrap();
    assert!(
        remove_standard_text_ranges(
            &d,
            &[TextRange {
                selection_id: b[0].source.selection_id.clone(),
                start: 0,
                end: 1
            }]
        )
        .is_err(),
        "serialized f32 compensation drifts beyond 0.002 pt"
    );
    let d = fixture("BT /F1 0.01 Tf 1000 Tc (AZ) Tj ET");
    let b = inspect_standard_glyphs(&d).unwrap();
    assert!(
        remove_standard_text_ranges(
            &d,
            &[TextRange {
                selection_id: b[0].source.selection_id.clone(),
                start: 0,
                end: 1
            }]
        )
        .is_err(),
        "unrepresentable compensation must refuse"
    );
}

#[test]
fn redaction_ranges_prefix_suffix_and_space_displacements_keep_remaining_text() {
    for text in [
        format!("{SECRET} Right"),
        format!("Left {SECRET}"),
        format!("Left {SECRET} Right"),
    ] {
        let d=fixture(&format!("BT /F1 13.25 Tf 17 TL 85 Tz 5 Ts 1 0 0 1 20 200 Tm 2 0.4 ({text}) \" ( Following) Tj T* (Next line.) Tj ET"));
        let before = inspect_standard_glyphs(&d).unwrap();
        let start = text.find(SECRET).unwrap();
        let result = remove_standard_text_ranges(
            &d,
            &[TextRange {
                selection_id: before[0].source.selection_id.clone(),
                start,
                end: start + SECRET.len(),
            }],
        )
        .unwrap();
        let saved = Document::load_mem(&result.bytes).unwrap();
        let after = inspect_standard_glyphs(&saved).unwrap();
        assert_eq!(
            after[0].source.text,
            format!("{} FollowingNext line.", text.replace(SECRET, ""))
        );
    }
    let d = fixture("BT /F1 12 Tf 3 Tw 2 Tc (A B) Tj ET");
    let b = inspect_standard_glyphs(&d).unwrap();
    let out = remove_standard_text_ranges(
        &d,
        &[TextRange {
            selection_id: b[0].source.selection_id.clone(),
            start: 1,
            end: 2,
        }],
    )
    .unwrap();
    let saved = Document::load_mem(&out.bytes).unwrap();
    let after = inspect_standard_glyphs(&saved).unwrap();
    near(after[0].glyphs[1].origin, b[0].glyphs[2].origin);
}

#[test]
fn redaction_ranges_exports_independent_corpus() {
    let Some(root) = std::env::var_os("FOLIO_REDACTION_RANGE_FIXTURES") else {
        return;
    };
    export_corpus(root, &["plain", "spaced", "saved", "affine"]);
}
#[test]
fn redaction_ranges_exports_positioned_corpus() {
    let Some(root) = std::env::var_os("FOLIO_REDACTION_POSITIONED_FIXTURES") else {
        return;
    };
    export_corpus(
        root,
        &[
            "tj-middle",
            "tj-spaced",
            "tj-affine",
            "tj-prefix",
            "tj-suffix",
            "tj-adjacent",
        ],
    );
}
fn export_corpus(root: std::ffi::OsString, layouts: &[&str]) {
    let root = std::path::PathBuf::from(root);
    std::fs::create_dir_all(&root).unwrap();
    let fonts = [
        "Helvetica",
        "Helvetica-Bold",
        "Helvetica-Oblique",
        "Helvetica-BoldOblique",
        "Times-Roman",
        "Times-Bold",
        "Times-Italic",
        "Times-BoldItalic",
        "Courier",
        "Courier-Bold",
        "Courier-Oblique",
        "Courier-BoldOblique",
    ];
    let mut manifest = vec![];
    for (fi, face) in fonts.iter().enumerate() {
        for win in [false, true] {
            for &layout in layouts {
                let program=match layout {
            "plain"=>format!("BT /F1 12 Tf 16 TL 1 0 0 1 20 240 Tm (`Left' {SECRET} Right) Tj ( Following.) Tj T* (Next line.) Tj ET"),
            "spaced"=>format!("BT /F1 13.25 Tf 18 TL 85 Tz 3 Ts 1 0 0 1 20 240 Tm 2 0.4 (`Left' {SECRET} Right) \" [( Following) 12 (.)] TJ T* (Next line.) Tj ET"),
            "saved"=>format!("BT /F1 12 Tf 16 TL 1 0 0 1 20 240 Tm (Before.) Tj q 0 -25 Td (`Left' {SECRET} Right) Tj Q ( Following.) Tj T* (Next line.) Tj ET"),
            "tj-middle"=>"BT /F1 12 Tf 16 TL 1 0 0 1 20 240 Tm [(`Left' Confi) 12 (dential) -20 (-84912 Right)] TJ ( Following.) Tj T* (Next line.) Tj ET".to_string(),
            "tj-spaced"=>"BT /F1 13.25 Tf 18 TL 85 Tz 3 Ts 2 Tw 0.4 Tc 1 0 0 1 20 222 Tm [(`Left' Confi) 12 (dential) -20 (-84912 Right)] TJ [( Following) 12 (.)] TJ T* (Next line.) Tj ET".to_string(),
            "tj-affine"=>"q 0.9 0.2 -0.15 0.9 30 10 cm BT /F1 11.5 Tf 16 TL 0.98 0.1 -0.1 0.98 20 190 Tm [(`Left' Confi) 12 (dential) -20 (-84912 Right)] TJ [( Following) -20 (.)] TJ T* (Next line.) Tj ET Q".to_string(),
            "tj-prefix"=>"BT /F1 12 Tf 16 TL 1 0 0 1 20 240 Tm [(Confi) 12 (dential) -20 (-84912 Right)] TJ ( Following.) Tj T* (Next line.) Tj ET".to_string(),
            "tj-suffix"=>"BT /F1 12 Tf 16 TL 1 0 0 1 20 240 Tm [(`Left' Confi) 12 (dential) -20 (-84912)] TJ ( Following.) Tj T* (Next line.) Tj ET".to_string(),
            "tj-adjacent"=>"BT /F1 12 Tf 16 TL 1 0 0 1 20 240 Tm [7 5 (`Left' Confi) 10 2 (dential) -9 -11 (-84912 Right) 6 -3] TJ ( Following.) Tj T* (Next line.) Tj ET".to_string(),
            _=>format!("q 0.9 0.2 -0.15 0.9 30 10 cm BT /F1 11.5 Tf 16 TL 0.98 0.1 -0.1 0.98 20 190 Tm (`Left' {SECRET} Right) Tj [( Following) -20 (.)] TJ T* (Next line.) Tj ET Q"),
        };
                let mut d = fixture(&program);
                let page = d.get_pages()[&1];
                let font = d
                    .get_dictionary(page)
                    .unwrap()
                    .get(b"Resources")
                    .unwrap()
                    .as_dict()
                    .unwrap()
                    .get(b"Font")
                    .unwrap()
                    .as_dict()
                    .unwrap()
                    .get(b"F1")
                    .unwrap()
                    .as_reference()
                    .unwrap();
                let f = d.get_dictionary_mut(font).unwrap();
                f.set("BaseFont", *face);
                if !win {
                    f.remove(b"Encoding");
                }
                let info = d.add_object(dictionary! {"Subject"=>Object::string_literal(SECRET)});
                d.trailer.set("Info", info);
                let inspected = inspect_standard_glyphs(&d).unwrap();
                let start = inspected[0].source.text.find(SECRET).unwrap();
                let end = start + SECRET.len();
                let result = remove_standard_text_ranges(
                    &d,
                    &[TextRange {
                        selection_id: inspected[0].source.selection_id.clone(),
                        start,
                        end,
                    }],
                )
                .unwrap();
                let id = format!("{fi}-{}-{layout}", u8::from(win));
                let before = format!("{id}-before.pdf");
                let after = format!("{id}-after.pdf");
                let mut bytes = vec![];
                d.save_to(&mut bytes).unwrap();
                std::fs::write(root.join(&before), bytes).unwrap();
                std::fs::write(root.join(&after), result.bytes).unwrap();
                let glyphs:Vec<_>=inspected[0].glyphs.iter().map(|g|serde_json::json!({"offset":g.offset,"code":g.code,"origin":g.origin,"end":g.end})).collect();
                manifest.push(serde_json::json!({"id":id,"face":face,"winAnsi":win,"layout":layout,"before":before,"after":after,"start":start,"end":end,"glyphs":glyphs}));
            }
        }
    }
    std::fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
}

#[test]
fn redaction_ranges_selected_tj_fragments_keep_numbers_and_following_positions() {
    let d = fixture("BT /F1 13.25 Tf 2 Tw 0.4 Tc 85 Tz 3 Ts 1 0 0 1 20 222 Tm [(Left Confi) 10 2 (dential) -20 (-84912 Right) 6 -3] TJ ( Following.) Tj ET");
    let before = inspect_standard_glyphs(&d).unwrap().remove(0);
    let start = before.source.text.find(SECRET).unwrap();
    let end = start + SECRET.len();
    let result = remove_standard_text_ranges(
        &d,
        &[TextRange {
            selection_id: before.source.selection_id.clone(),
            start,
            end,
        }],
    )
    .unwrap();
    let saved = Document::load_mem(&result.bytes).unwrap();
    let after = inspect_standard_glyphs(&saved).unwrap().remove(0);
    assert_eq!(after.source.text, "Left  Right Following.");
    let kept: Vec<_> = before
        .glyphs
        .iter()
        .filter(|g| g.offset < start || g.offset >= end)
        .collect();
    assert_eq!(kept.len(), after.glyphs.len());
    for (a, b) in kept.iter().zip(&after.glyphs) {
        assert_eq!(a.code, b.code);
        near(a.origin, b.origin);
        near(a.end, b.end);
    }
}
