use super::*;
use lopdf::Object;

fn invocation() -> Vec<Operation> {
    vec![
        Operation::new("q", vec![]),
        Operation::new("Do", vec![Object::Name(b"Answer".to_vec())]),
        Operation::new("Q", vec![]),
    ]
}
fn edited(bytes: &[u8], kind: AnswerKind) -> Vec<Operation> {
    let slot = AnswerSlot::parse(bytes, kind).unwrap();
    Content::decode_strict(&slot.splice(&invocation()).unwrap())
        .unwrap()
        .operations
}
fn strings(ops: &[Operation]) -> Vec<Vec<u8>> {
    ops.iter()
        .filter(|op| op.operator == "Tj")
        .map(|op| op.operands[0].as_str().unwrap().to_vec())
        .collect()
}
#[test]
fn compound_pymupdf_frame_and_inset_answer_are_isolated_without_border_loss() {
    let bytes=b"/Tx BMC q 1 w 0 0 0 RG .5 .5 249 29 re s 1 1 248 28 re W n BT 0 0 0 rg 2 10.799999 Td /Helv 14 Tf (Original answer) Tj ET Q EMC";
    let slot=AnswerSlot::parse_bounds(bytes,AnswerKind::Text,Some([0.,0.,250.,30.])).unwrap();
    let changed=slot.splice(&invocation()).unwrap();
    let ops=Content::decode_strict(&changed).unwrap().operations;
    assert!(strings(&ops).is_empty());
    assert_eq!(ops.iter().filter(|o|o.operator=="BMC").count(),1);
    assert_eq!(ops.iter().filter(|o|o.operator=="s").count(),1);
    assert_eq!(ops.iter().filter(|o|o.operator=="W").count(),1);
    assert!(ops.iter().position(|o|o.operator=="s").unwrap()<ops.iter().position(|o|o.operator=="Do").unwrap());
    assert!(AnswerSlot::parse_bounds(&changed,AnswerKind::Text,Some([0.,0.,250.,30.])).is_ok());
}
#[test]
fn compound_frame_clear_refill_keeps_one_bounded_slot_and_original_clip() {
    let source=b"/Tx BMC q .9 g 0 0 180 30 re f 0 G 1 w .5 .5 179 29 re s 1 1 178 28 re W n BT /F1 12 Tf 2 10 Td (First) Tj ET BT /F1 12 Tf 2 20 Td (Second) Tj ET Q EMC";
    let mut bytes=source.to_vec();let mut counts=None;let replacement=invocation();
    for i in 0..20 {
        let slot=AnswerSlot::parse_bounds(&bytes,AnswerKind::Text,Some([0.,0.,180.,30.])).unwrap();
        bytes=slot.splice(if i%2==0 {&[]} else {&replacement}).unwrap();
        let ops=Content::decode_strict(&bytes).unwrap().operations;
        assert_eq!(ops.iter().filter(|o|o.operator=="BMC").count(),1);
        assert_eq!(ops.iter().filter(|o|o.operator=="s").count(),1);
        assert_eq!(ops.iter().filter(|o|o.operator=="f").count(),1);
        assert_eq!(ops.iter().filter(|o|o.operator=="W").count(),1);
        assert!(strings(&ops).is_empty());
        if i%2==0 {if let Some(n)=counts {assert_eq!(ops.len(),n);} else {counts=Some(ops.len());}}
    }
}
#[test]
fn compound_answer_does_not_admit_unknown_art_or_merge_separate_clips() {
    for source in [
        b"/Tx BMC q 0 G 1 w .5 .5 179 29 re s /Logo Do 1 1 178 28 re W n BT /F1 12 Tf (Old) Tj ET Q EMC".as_slice(),
        b"/Tx BMC q 0 G 1 w .5 .5 179 29 re s 1 1 178 28 re W n BT /F1 12 Tf (One) Tj ET Q q 20 5 40 10 re W n BT /F1 12 Tf (Two) Tj ET Q EMC",
        b"/Tx BMC q 0 G 1 w .5 .5 179 29 re s 1 1 178 28 re W n BT /F1 12 Tf (One) Tj ET 1 0 0 rg 20 5 40 10 re f BT /F1 12 Tf (Two) Tj ET Q EMC",
    ] {assert!(AnswerSlot::parse_bounds(source,AnswerKind::Text,Some([0.,0.,180.,30.])).is_err());}
}
#[test]
fn compound_empty_text_keeps_frame_and_replays_text_state_for_following_paint() {
    let bytes=b"/Tx BMC q 0 G 1 w .5 .5 179 29 re s 1 1 178 28 re W n BT /F1 12 Tf 0 -7 TD 2 3 () \" ET Q EMC";
    let slot=AnswerSlot::parse_bounds(bytes,AnswerKind::Text,Some([0.,0.,180.,30.])).unwrap();
    let ops=Content::decode_strict(&slot.splice(&invocation()).unwrap()).unwrap().operations;
    assert!(strings(&ops).is_empty());
    let end=ops.iter().position(|op|op.operator=="EMC").unwrap();
    let replay=&ops[end+1..];
    for (operator,value) in [("TL",7.),("Tw",2.),("Tc",3.)] {
        assert!(replay.iter().any(|op|op.operator==operator && op.operands[0].as_float().unwrap()==value));
    }
    assert_eq!(ops.iter().filter(|op|op.operator=="s").count(),1);
    assert_eq!(ops.iter().filter(|op|op.operator=="W").count(),1);
}
#[test]
fn compound_text_cannot_escape_and_reenter_an_equal_depth_graphics_frame() {
    // Net depth is zero and both branches inherit the same clip, but the text
    // interval still escapes its entry state. Canonicalization must refuse it.
    let bytes=b"/Tx BMC q 0 G 1 w .5 .5 179 29 re s 1 1 178 28 re W n q BT /F1 12 Tf (One) Tj ET Q q BT /F1 12 Tf (Two) Tj ET Q Q EMC";
    assert!(AnswerSlot::parse_bounds(bytes,AnswerKind::Text,Some([0.,0.,180.,30.])).is_err());
}
#[test]
fn marked_answer_preserves_caption_and_foreground_order() {
    let bytes=b"BT /F1 10 Tf (Caption) Tj ET /Tx BMC q BT /F1 12 Tf (Old answer) Tj ET Q EMC 1 0 0 rg 80 5 5 5 re f";
    let ops = edited(bytes, AnswerKind::Text);
    assert_eq!(strings(&ops), vec![b"Caption".to_vec()]);
    assert!(
        ops.iter().position(|op| op.operator == "Do").unwrap()
            < ops.iter().position(|op| op.operator == "re").unwrap()
    );
    assert_eq!(ops.iter().filter(|op| op.operator == "rg").count(), 1);
}
#[test]
fn empty_marked_answer_keeps_its_insertion_layer_for_refill() {
    let bytes = b"/Tx BMC q Q EMC 1 0 0 rg 80 5 5 5 re f";
    let slot = AnswerSlot::parse(bytes, AnswerKind::Text).unwrap();
    let cleared = slot.splice(&[]).unwrap();
    let ops = edited(&cleared, AnswerKind::Text);
    assert!(
        ops.iter().position(|op| op.operator == "Do").unwrap()
            < ops.iter().position(|op| op.operator == "re").unwrap()
    );
}
#[test]
fn unmarked_text_preserves_following_paint_state_and_removes_quote_strings() {
    let bytes =
        b"0 0 180 30 re S BT /F1 12 Tf 5 10 Td 1 0 0 rg (Old) Tj 2 3 (answer) \" ET 80 5 5 5 re f";
    let ops = edited(bytes, AnswerKind::Text);
    assert!(strings(&ops).is_empty());
    assert!(!ops.iter().any(|op| op.operator == "\""));
    assert!(ops
        .iter()
        .any(|op| op.operator == "Tw" && op.operands == vec![Object::Integer(2)]));
    assert!(ops
        .iter()
        .any(|op| op.operator == "Tc" && op.operands == vec![Object::Integer(3)]));
    let color = ops.iter().position(|op| op.operator == "rg").unwrap();
    let art = ops.iter().rposition(|op| op.operator == "re").unwrap();
    assert!(color < art);
    assert!(AnswerSlot::parse(
        &Content { operations: ops }.encode().unwrap(),
        AnswerKind::Text
    )
    .is_ok());
}
#[test]
fn ordinary_unmarked_choice_fixture_remains_supported() {
    for kind in [AnswerKind::Combo, AnswerKind::List] {
        let ops = edited(
            b"0 0 180 30 re S BT /F1 12 Tf 5 10 Td (Original name) Tj ET",
            kind,
        );
        assert!(strings(&ops).is_empty());
        assert!(ops.iter().any(|op| op.operator == "S"));
    }
}
#[test]
fn marked_list_selection_paint_is_replaced_with_the_answer() {
    let ops=edited(b"q 0 0 180 30 re S Q /Tx BMC q 1 1 178 28 re W n 0 0 1 rg 1 1 178 12 re f BT /F1 12 Tf (Selected) Tj ET Q EMC",AnswerKind::List);
    assert!(!ops
        .iter()
        .any(|op| op.operator == "f" || op.operator == "rg"));
    assert!(ops.iter().any(|op| op.operator == "S"));
}
#[test]
fn nested_balanced_graphics_and_clips_are_accepted() {
    let bytes=b"q 2 0 0 2 4 8 cm 0 0 100 30 re W n /Tx BMC q q 1 1 98 28 re W n BT /F1 12 Tf (Old) Tj ET Q Q EMC Q";
    let ops = edited(bytes, AnswerKind::Text);
    assert!(ops.iter().any(|op| op.operator == "cm"));
}
#[test]
fn ambiguous_or_state_leaking_answers_are_refused() {
    for bytes in [
        b"BT /F1 12 Tf (Old) Tj ET BT /F1 12 Tf (Caption) Tj ET".as_slice(),
        b"BT /F1 12 Tf 7 Tr (Old) Tj ET",
        b"/Tx BMC BT /F1 12 Tf (Old) Tj ET EMC",
        b"/Tx BMC q BT /F1 12 Tf (Old) Tj ET EMC Q",
        b"0 0 m /Tx BMC q Q EMC 10 10 l S",
        b"/Tx BMC q Q EMC /Tx BMC q Q EMC",
        b"/Tx BMC q BT /F1 12 Tf (Old) Tj ET Q EMC trailing",
    ] {
        assert!(
            AnswerSlot::parse(bytes, AnswerKind::Text).is_err(),
            "accepted {}",
            String::from_utf8_lossy(bytes)
        );
    }
    assert!(
        AnswerSlot::parse(b"0 0 10 10 re f BT /F1 12 Tf (Old) Tj ET", AnswerKind::List).is_err()
    );
}
#[test]
fn byte_operation_operand_and_nesting_limits_precede_expansion() {
    assert!(AnswerSlot::parse(&vec![b' '; MAX_BYTES + 1], AnswerKind::Text).is_err());
    let mut ops = b"q Q ".repeat(MAX_OPS / 2 + 1);
    ops.extend_from_slice(b"/Tx BMC q Q EMC");
    assert!(AnswerSlot::parse(&ops, AnswerKind::Text).is_err());
    let mut deep = b"[".repeat(65);
    deep.extend_from_slice(b"0");
    deep.extend(b"]".repeat(65));
    deep.extend_from_slice(b" TJ");
    assert!(AnswerSlot::parse(&deep, AnswerKind::Text).is_err());
    let mut operands = b"0 ".repeat(100001);
    operands.extend_from_slice(b"TJ");
    assert!(AnswerSlot::parse(&operands, AnswerKind::Text).is_err());
}

fn page_pdf(content: Vec<u8>) -> Vec<u8> {
    use lopdf::{dictionary, Dictionary, Document, Stream};
    let mut d = Document::new();
    let pages = d.new_object_id();
    let content = d.add_object(Stream::new(Dictionary::new(), content));
    let page=d.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),120.into(),120.into()],"Resources"=>Dictionary::new(),"Contents"=>content});
    d.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]}.into(),
    );
    let root = d.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    d.trailer.set("Root", root);
    let mut bytes = Vec::new();
    d.save_to(&mut bytes).unwrap();
    bytes
}
#[test]
fn replacement_keeps_source_clip_and_cancels_entry_transform_in_native_pixels() {
    let e = crate::PdfEngine::start(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let paint = Content::decode_strict(b"0 0 1 rg -100 -100 400 400 re f")
        .unwrap()
        .operations;
    for (case, matrix) in ["2 0 0 2 10 10", "0 1 -1 0 100 10", "1 0.25 0.5 1 10 10"]
        .iter()
        .enumerate()
    {
        let source =
            format!("q {matrix} cm /Tx BMC q 5 5 20 10 re W n 1 0 0 rg 0 0 100 100 re f Q EMC Q");
        let slot = AnswerSlot::parse(source.as_bytes(), AnswerKind::Text).unwrap();
        let edited = slot.splice(&paint).unwrap();
        let expected = source.replace("1 0 0 rg", "0 0 1 rg").into_bytes();
        let mut renders = Vec::new();
        for (name, content) in [("edited", edited), ("expected", expected)] {
            let path = dir.path().join(format!("{case}-{name}.pdf"));
            std::fs::write(&path, page_pdf(content)).unwrap();
            let doc = e.open_document(&path).unwrap();
            renders.push(e.render_page(&doc.id, 0, 360).unwrap());
            e.close_document(&doc.id).unwrap();
        }
        assert!(
            renders[0] == renders[1],
            "clip or transform changed for {matrix}"
        );
    }
}
#[test]
fn different_answer_clip_regions_are_not_merged_into_a_larger_clip() {
    let bytes=b"/Tx BMC q 0 0 10 10 re W n BT /F1 12 Tf (One) Tj ET Q q 20 20 10 10 re W n BT /F1 12 Tf (Two) Tj ET Q EMC";
    assert!(AnswerSlot::parse(bytes, AnswerKind::Text).is_err());
}

#[test]
fn empty_answer_cannot_pick_one_of_several_unrelated_clip_branches() {
    let bytes = b"/Tx BMC q 0 0 10 10 re W n Q q 20 20 10 10 re W n Q EMC";
    assert!(AnswerSlot::parse(bytes, AnswerKind::Text).is_err());
}

#[test]
fn repeated_replacement_preserves_an_empty_clipped_slot_without_growth() {
    let original = b"q 2 0 0 2 10 10 cm 0 0 30 30 re W n /Tx BMC q 5 5 20 10 re W n Q EMC Q";
    let mut bytes = original.to_vec();
    let mut size = None;
    for _ in 0..20 {
        bytes = AnswerSlot::parse(&bytes, AnswerKind::Text)
            .unwrap()
            .splice(&[])
            .unwrap();
        let count = Content::decode_strict(&bytes).unwrap().operations.len();
        if let Some(expected) = size {
            assert_eq!(count, expected, "clip program grew on re-edit");
        } else {
            size = Some(count);
        }
        assert_eq!(
            Content::decode_strict(&bytes)
                .unwrap()
                .operations
                .iter()
                .filter(|op| op.operator == "W")
                .count(),
            2
        );
    }
}

#[test]
fn strings_and_comments_do_not_consume_the_operator_budget() {
    let mut bytes = b"% /Tx BMC Q ET BI\n BT /F1 12 Tf (".to_vec();
    bytes.extend(b"q Q ".repeat(MAX_OPS / 2 + 1));
    bytes.extend_from_slice(b") Tj ET");
    assert!(AnswerSlot::parse(&bytes, AnswerKind::Text).is_ok());
}

#[test]
fn singular_and_nonfinite_transforms_are_refused() {
    for prefix in [
        "0 0 0 0 0 0",
        "1 1 1 1 0 0",
        "1000000000000000000000000000000000000000000000 0 0 1 0 0",
    ] {
        assert!(AnswerSlot::parse(
            format!("q {prefix} cm /Tx BMC q Q EMC Q").as_bytes(),
            AnswerKind::Text
        )
        .is_err());
    }
}

#[test]
fn empty_choice_inset_borders_are_distinguished_from_row_selection_paint() {
    let bounds = [0., 0., 280., 50.];
    for kind in [AnswerKind::Combo, AnswerKind::List] {
        assert!(AnswerSlot::parse_empty_source(
            b"0 g 0 0 280 50 re 0.5 0.5 279 49 re f*",
            kind,
            bounds
        )
        .is_ok());
        for paint in [
            "0 0 280 50 re 0.5 0.5 279 49 re f", // non-zero fill has no hole
            "0 0 280 50 re 0 5 280 30 re f*",    // selection strip
            "0 0 280 50 re 1 2 278 46 re f*",    // asymmetric inset
            "0 0 280 50 re 1 1 278 48 re 2 2 276 46 re f*",
            "0 0 280 50 re 0 0 m 1 1 l f*",
        ] {
            assert!(
                AnswerSlot::parse_empty_source(paint.as_bytes(), kind, bounds).is_err(),
                "{paint}"
            );
        }
    }
}
