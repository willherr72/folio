use super::*;
use crate::redaction_ranges::inspect_standard_glyphs;
use lopdf::{dictionary, Document, Object, Stream};

fn runtime() -> WorkerRuntime {
    WorkerRuntime::new(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
        Arc::new(FontRegistry::new()),
    )
    .unwrap()
}
fn source(rt: &mut WorkerRuntime, program: &str, win: bool) -> DocumentInfo {
    let mut d = Document::with_version("1.7");
    let tree = d.new_object_id();
    let mut f = dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"};
    if win {
        f.set("Encoding", "WinAnsiEncoding");
    }
    let font = d.add_object(f);
    let content = d.add_object(Stream::new(dictionary! {}, program.as_bytes().to_vec()));
    let page=d.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),500.into(),300.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});
    d.objects.insert(
        tree,
        Object::Dictionary(dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}),
    );
    let root = d.add_object(dictionary! {"Type"=>"Catalog","Pages"=>tree});
    d.trailer.set("Root", root);
    let mut bytes = vec![];
    d.save_to(&mut bytes).unwrap();
    rt.register_generated_document(bytes, "Mark source.pdf")
        .unwrap()
}
fn enclosing(glyphs: impl Iterator<Item = TextMarkRect>) -> TextMarkRect {
    let mut x = f64::INFINITY;
    let mut y = f64::INFINITY;
    let mut right = f64::NEG_INFINITY;
    let mut bottom = f64::NEG_INFINITY;
    for b in glyphs {
        x = x.min(b.x);
        y = y.min(b.y);
        right = right.max(b.x + b.width);
        bottom = bottom.max(b.y + b.height);
    }
    TextMarkRect {
        x,
        y,
        width: right - x,
        height: bottom - y,
    }
}
#[test]
fn redaction_selection_owned_mark_removes_exact_text_without_changing_source() {
    let mut rt = runtime();
    let info = source(
        &mut rt,
        "BT /F1 12 Tf 20 240 Td (Left Confidential-84912 Right) Tj 0 -20 Td (Next line.) Tj ET",
        true,
    );
    let original = rt.document(&info.id).unwrap().bytes().unwrap();
    let selected = rt.inspect_redaction_text_page(&info.id, 0).unwrap();
    let rect = enclosing(
        selected
            .glyphs()
            .iter()
            .filter(|g| (5..23).contains(&g.offset))
            .map(|g| g.bounds),
    );
    let mark = selected.mark(rect).unwrap();
    assert_eq!(mark.affected_glyphs().len(), 18);
    let out = rt.redact_text_mark(&mark).unwrap();
    assert_eq!(out.removed_characters, 18);
    let saved = Document::load_mem(&out.bytes).unwrap();
    let b = inspect_standard_glyphs(&saved).unwrap();
    assert_eq!(b[0].source.text, "Left  RightNext line.");
    assert_eq!(&*original, &*rt.document(&info.id).unwrap().bytes().unwrap());
    assert_eq!(rt.documents.len(), 1);
}
#[test]
fn redaction_selection_uses_actual_encoding_and_ignores_generated_separators() {
    let mut rt = runtime();
    for win in [false, true] {
        let info = source(
            &mut rt,
            "BT /F1 12 Tf 20 240 Td (`A') Tj 0 -20 Td (B) Tj ET",
            win,
        );
        let selected = rt.inspect_redaction_text_page(&info.id, 0).unwrap();
        let codes: Vec<_> = selected.glyphs().iter().map(|g| g.unicode).collect();
        assert_eq!(
            codes,
            if win {
                vec![96, 65, 39, 66]
            } else {
                vec![0x2018, 65, 0x2019, 66]
            }
        );
        assert_eq!(selected.glyphs()[3].offset, 3);
    }
}
#[test]
fn redaction_selection_intervening_spaces_are_removed_but_not_unselected_letters() {
    let mut rt = runtime();
    let info = source(&mut rt, "BT /F1 12 Tf 20 240 Td (A  B C) Tj ET", true);
    let selected = rt.inspect_redaction_text_page(&info.id, 0).unwrap();
    let mark = selected
        .mark(enclosing(selected.glyphs()[..2].iter().map(|g| g.bounds)))
        .unwrap();
    let out = rt.redact_text_mark(&mark).unwrap();
    assert_eq!(out.removed_characters, 4);
    assert_eq!(
        inspect_standard_glyphs(&Document::load_mem(&out.bytes).unwrap()).unwrap()[0]
            .source
            .text,
        " C"
    );
}
#[test]
fn redaction_selection_invalid_stale_closed_and_print_only_marks_fail() {
    let mut rt = runtime();
    let info = source(&mut rt, "BT /F1 12 Tf 20 240 Td (ABC) Tj ET", true);
    let selected = rt.inspect_redaction_text_page(&info.id, 0).unwrap();
    for rect in [
        TextMarkRect {
            x: f64::NAN,
            y: 1.,
            width: 1.,
            height: 1.,
        },
        TextMarkRect {
            x: 0.,
            y: 0.,
            width: 0.,
            height: 1.,
        },
        TextMarkRect {
            x: 499.,
            y: 0.,
            width: 2.,
            height: 1.,
        },
        TextMarkRect {
            x: 0.,
            y: 0.,
            width: 1.,
            height: 1.,
        },
    ] {
        assert!(selected.mark(rect).is_err());
    }
    let mark = selected.mark(selected.glyphs()[0].bounds).unwrap();
    rt.documents.get_mut(&info.id).unwrap().for_printing = true;
    assert!(rt.inspect_redaction_text_page(&info.id, 0).is_err());
    assert!(rt.redact_text_mark(&mark).is_err());
    rt.documents.get_mut(&info.id).unwrap().for_printing = false;
    let mut changed = rt.document(&info.id).unwrap().bytes().unwrap().to_vec();
    changed.extend_from_slice(b"\n%changed");
    rt.documents.get_mut(&info.id).unwrap().source_bytes = crate::source_storage::SourceStore::new().capture(changed.into()).unwrap();
    assert!(rt.redact_text_mark(&mark).is_err());
    rt.close_document(&info.id).unwrap();
    assert!(rt.redact_text_mark(&mark).is_err());
    assert_eq!(rt.documents.len(), 0);
}
#[test]
fn redaction_selection_coincident_mapping_is_not_guessed() {
    let mut rt = runtime();
    let info = source(
        &mut rt,
        "BT /F1 12 Tf 1 0 0 1 20 240 Tm (A) Tj 1 0 0 1 20 240 Tm (A) Tj ET",
        true,
    );
    assert!(rt.inspect_redaction_text_page(&info.id, 0).is_err());
    assert_eq!(rt.documents.len(), 1);
}

#[test]
fn redaction_selection_mapping_is_order_independent_and_never_consumes_ambiguity() {
    let e = |offset, x| ExpectedGlyph {
        block: 0,
        offset,
        unicode: 65,
        origin: [x, 10.],
    };
    let n = |x| NativeGlyph {
        unicode: 65,
        origin: [x, 10.],
        bounds: TextMarkRect {
            x,
            y: 20.,
            width: 1.,
            height: 1.,
        },
    };
    let mapped = bind_glyphs(vec![e(0, 20.), e(1, 40.)], vec![n(40.), n(20.)]).unwrap();
    assert_eq!(mapped[0].offset, 0);
    assert_eq!(mapped[0].bounds.x, 20.);
    assert_eq!(mapped[1].offset, 1);
    assert_eq!(mapped[1].bounds.x, 40.);
    assert!(
        bind_glyphs(vec![e(0, 0.), e(1, 0.003)], vec![n(0.), n(0.0015)]).is_err(),
        "consuming the first glyph cannot disambiguate the second"
    );
    assert!(bind_glyphs(
        (0..33).map(|i| e(i, i as f64 * 0.00001)).collect(),
        (0..33).map(|i| n(i as f64 * 0.00001)).collect()
    )
    .is_err());
    assert!(bind_glyphs(vec![e(0, 20.)], vec![n(f64::NAN)]).is_err());
}

#[test]
fn redaction_selection_marks_do_not_bridge_unselected_letters_and_bound_fragmentation() {
    let make = |count| PageTextSelection {
        complete_area: false,
        page_index: 0,
        source_id: "test-owned".into(),
        source_hash: [0; 32],
        width: 500.,
        height: 300.,
        blocks: HashMap::from([(
            0,
            SelectionBlock {
                id: "f".repeat(64),
                text: "AB".repeat(count).chars().collect(),
            },
        )]),
        glyphs: (0..count * 2)
            .map(|i| MeasuredGlyph {
                block_index: 0,
                offset: i,
                unicode: if i % 2 == 0 { 65 } else { 66 },
                bounds: TextMarkRect {
                    x: 20.,
                    y: if i % 2 == 0 { 20. } else { 40. },
                    width: 1.,
                    height: 1.,
                },
            })
            .collect(),
    };
    let rect = TextMarkRect {
        x: 20.,
        y: 20.,
        width: 1.,
        height: 1.,
    };
    let plan = make(4096).mark(rect).unwrap();
    assert_eq!(plan.ranges.len(), 4096);
    assert!(plan.ranges.iter().all(|r| r.end == r.start + 1));
    assert!(make(4097).mark(rect).is_err());
    let mut complete = make(4096);
    complete.complete_area = true;
    assert_eq!(
        complete
            .area_ranges("test-owned", [0; 32], &[rect])
            .unwrap()
            .ranges
            .len(),
        4096
    );
    let mut fragmented = make(4097);
    fragmented.complete_area = true;
    assert!(fragmented
        .area_ranges("test-owned", [0; 32], &[rect])
        .is_err());
    assert!(
        make(1)
            .mark(TextMarkRect {
                x: 21.,
                y: 20.,
                width: 1.,
                height: 1.
            })
            .is_err(),
        "touching edges do not select"
    );
}

#[test]
fn redaction_area_selection_requires_complete_measurement_and_exact_source_identity() {
    let mut rt = runtime();
    let info = source(&mut rt, "BT /F1 12 Tf 20 240 Td (A B) Tj ET", true);
    let hash: [u8; 32] = Sha256::digest(rt.document(&info.id).unwrap().bytes().unwrap().as_ref()).into();
    let text_only = rt.inspect_redaction_text_page(&info.id, 0).unwrap();
    let rect = enclosing(text_only.glyphs().iter().map(|g| g.bounds));
    assert!(text_only.area_ranges(&info.id, hash, &[rect]).is_err());
    let full = rt.inspect_redaction_area_page(&info.id, 0).unwrap();
    assert_eq!(full.glyphs().len(), 3);
    assert!(full.area_ranges("another-source", hash, &[rect]).is_err());
    assert!(full.area_ranges(&info.id, [0; 32], &[rect]).is_err());
    assert_eq!(
        full.area_ranges(&info.id, hash, &[rect])
            .unwrap()
            .bounds
            .len(),
        3
    );
}

#[test]
fn redaction_selection_rechecks_edit_copy_and_protected_policy_before_output() {
    let mut rt = runtime();
    let info = source(&mut rt, "BT /F1 12 Tf 20 240 Td (ABC) Tj ET", true);
    let selection = rt.inspect_redaction_text_page(&info.id, 0).unwrap();
    let plan = selection.mark(selection.glyphs()[0].bounds).unwrap();
    let original = rt.document(&info.id).unwrap().bytes().unwrap();
    for (edit, copy) in [(false, true), (true, false), (true, true)] {
        let change:crate::protection::ProtectionChange=serde_json::from_value(serde_json::json!({"mode":"passwords","userPassword":"test-reader","ownerPassword":"test-owner","permissions":{"print":true,"printHighQuality":true,"copyContent":copy,"editContent":edit,"annotate":false,"fillForms":false,"assemble":true}})).unwrap();
        let encrypted = change.apply(&original).unwrap();
        let unlocked = crate::protection::unlock_pdf(&encrypted, Some("test-reader")).unwrap();
        let policy = SourcePolicy::new(unlocked.protection.unwrap(), &info.pages, &Ok(vec![]));
        rt.documents.get_mut(&info.id).unwrap().policy = Some(policy);
        let message = match rt.redact_text_mark(&plan) {
            Err(e) => e.to_string(),
            Ok(_) => panic!("policy must refuse"),
        };
        assert!(
            message.contains(if !edit {
                "editing existing text"
            } else if !copy {
                "copying text"
            } else {
                "Protected PDF text redaction"
            }),
            "{message}"
        );
        assert!(rt.inspect_redaction_text_page(&info.id, 0).is_err());
        assert_eq!(&*original, &*rt.document(&info.id).unwrap().bytes().unwrap());
    }
    assert_eq!(rt.documents.len(), 1);
}

#[test]
fn redaction_selection_exports_source_mapping_and_mark_corpus() {
    let Some(out) = std::env::var_os("FOLIO_REDACTION_SELECTION_FIXTURES") else {
        return;
    };
    let out = PathBuf::from(out);
    std::fs::create_dir(&out).unwrap();
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let inputs = [
        repo.join("artifacts/redaction-ranges-20261029/fixtures-fix-1"),
        repo.join("artifacts/redaction-positioned-20261029/fixtures-1"),
    ];
    let mut rt = runtime();
    let mut receipts = vec![];
    let mut outputs = vec![];
    for directory in inputs {
        let cases: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(directory.join("manifest.json")).unwrap())
                .unwrap();
        for mut case in cases {
            let path = directory.join(case["before"].as_str().unwrap());
            let bytes = std::fs::read(&path).unwrap();
            let hash = Sha256::digest(&bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            let info = rt
                .register_generated_document(bytes.clone(), "Selection corpus.pdf")
                .unwrap();
            let selected = rt
                .inspect_redaction_text_page(&info.id, 0)
                .unwrap_or_else(|e| panic!("{}: {e}", case["id"]));
            let native:Vec<_>=selected.glyphs().iter().map(|g|serde_json::json!({"block":g.block_index,"offset":g.offset,"unicode":g.unicode,"bounds":[g.bounds.x,g.bounds.y,g.bounds.width,g.bounds.height]})).collect();
            receipts.push(serde_json::json!({"input":path.to_string_lossy(),"inputSha256":hash,"sourceId":info.id,"glyphs":native}));
            let layout = case["layout"].as_str().unwrap();
            if layout == "plain" || layout == "tj-middle" {
                let start = case["start"].as_u64().unwrap() as usize;
                let end = case["end"].as_u64().unwrap() as usize;
                let rect = enclosing(
                    selected
                        .glyphs()
                        .iter()
                        .filter(|g| (start..end).contains(&g.offset))
                        .map(|g| g.bounds),
                );
                let mark = selected.mark(rect).unwrap();
                assert_eq!(mark.ranges.len(), 1);
                assert_eq!(
                    (mark.ranges[0].start, mark.ranges[0].end),
                    (start, end),
                    "{}",
                    case["id"]
                );
                let removed = rt.redact_text_mark(&mark).unwrap();
                assert_eq!(removed.removed_characters, end - start);
                let id = format!("select-{}", case["id"].as_str().unwrap());
                let before = format!("{id}-before.pdf");
                let after = format!("{id}-after.pdf");
                std::fs::write(out.join(&before), &bytes).unwrap();
                std::fs::write(out.join(&after), removed.bytes).unwrap();
                case["id"] = id.into();
                case["before"] = before.into();
                case["after"] = after.into();
                case["mark"] = serde_json::json!([rect.x, rect.y, rect.width, rect.height]);
                outputs.push(case);
            }
            assert_eq!(&bytes, &*rt.document(&info.id).unwrap().bytes().unwrap());
            rt.close_document(&info.id).unwrap();
            assert!(rt.documents.is_empty());
        }
    }
    assert_eq!(receipts.len(), 240);
    assert_eq!(outputs.len(), 48);
    std::fs::write(
        out.join("mapping.json"),
        serde_json::to_vec_pretty(&receipts).unwrap(),
    )
    .unwrap();
    std::fs::write(
        out.join("manifest.json"),
        serde_json::to_vec_pretty(&outputs).unwrap(),
    )
    .unwrap();
}

#[test]
fn redaction_selection_many_marks_union_and_every_mark_must_hit() {
    let mut rt = runtime();
    let info = source(&mut rt, "BT /F1 12 Tf 20 240 Td (ABCDE) Tj ET", true);
    let selected = rt.inspect_redaction_text_page(&info.id, 0).unwrap();
    let a = selected.glyphs()[0].bounds;
    let c = selected.glyphs()[2].bounds;
    let plan = selected.mark_many(&[a, c, a]).unwrap();
    assert_eq!(plan.affected_glyphs().len(), 2);
    assert_eq!(plan.ranges.len(), 2);
    let out = rt.redact_text_marks(&[plan]).unwrap();
    assert_eq!(out.removed_characters, 2);
    assert_eq!(
        inspect_standard_glyphs(&Document::load_mem(&out.bytes).unwrap()).unwrap()[0]
            .source
            .text,
        "BDE"
    );
    assert!(selected.mark_many(&[]).is_err());
    assert!(selected.mark_many(&vec![a; 65]).is_err());
    assert!(selected
        .mark_many(&[
            a,
            TextMarkRect {
                x: 0.,
                y: 0.,
                width: 1.,
                height: 1.
            }
        ])
        .is_err());
}
#[test]
fn redaction_selection_batch_is_page_and_source_bound() {
    let mut rt = runtime();
    let info = source(&mut rt, "BT /F1 12 Tf 20 240 Td (ABC) Tj ET", true);
    let mut doc = Document::load_mem(rt.document(&info.id).unwrap().bytes().unwrap().as_ref()).unwrap();
    let original = doc.get_pages()[&1];
    let page = doc.get_object(original).unwrap().clone();
    let parent = page
        .as_dict()
        .unwrap()
        .get(b"Parent")
        .unwrap()
        .as_reference()
        .unwrap();
    let second = doc.add_object(page);
    let tree = doc.get_object_mut(parent).unwrap().as_dict_mut().unwrap();
    tree.set(
        "Kids",
        vec![Object::Reference(original), Object::Reference(second)],
    );
    tree.set("Count", 2);
    let mut bytes = vec![];
    doc.save_to(&mut bytes).unwrap();
    let multi = rt.register_generated_document(bytes, "multi.pdf").unwrap();
    let first = rt.inspect_redaction_text_page(&multi.id, 0).unwrap();
    let second = rt.inspect_redaction_text_page(&multi.id, 1).unwrap();
    let p = first.mark(first.glyphs()[0].bounds).unwrap();
    let q = second.mark(second.glyphs()[1].bounds).unwrap();
    let out = rt.redact_text_marks(&[p, q]).unwrap();
    let got = inspect_standard_glyphs(&Document::load_mem(&out.bytes).unwrap()).unwrap();
    assert_eq!(
        got.iter()
            .map(|g| g.source.text.as_str())
            .collect::<Vec<_>>(),
        vec!["BC", "AC"]
    );
    assert!(rt.redact_text_marks(&[]).is_err());
    assert!(rt
        .redact_text_marks(&[
            first.mark(first.glyphs()[0].bounds).unwrap(),
            first.mark(first.glyphs()[1].bounds).unwrap()
        ])
        .is_err());
    let other = rt.inspect_redaction_text_page(&info.id, 0).unwrap();
    assert!(rt
        .redact_text_marks(&[
            other.mark(other.glyphs()[0].bounds).unwrap(),
            second.mark(second.glyphs()[1].bounds).unwrap()
        ])
        .is_err());
}
