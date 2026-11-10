use super::*;
use crate::form_fill_identity::{
    tests::{fixture, request},
    NodeBinding,
};
use lopdf::{dictionary, Stream};

#[test]
fn review_real_pattern_type_cannot_bypass_program_bounds() {
    let mut native = Document::new();
    let mut large = oversized(true);
    large.dict.remove(b"Subtype");
    large.dict.set("PatternType", Object::Real(1.0));
    let pattern = native.add_object(large);
    let root = native.add_object(Stream::new(
        dictionary! {"Resources"=>dictionary!{"Pattern"=>dictionary!{"P"=>pattern}}},
        b"q Q".to_vec(),
    ));
    assert!(
        NormalAppearanceAdmission::new(&native)
            .admit(&root.into())
            .is_err(),
        "real-valued pattern type bypassed admission"
    );
}

fn oversized(operations: bool) -> Stream {
    let bytes = if operations {
        b"q Q\n".repeat(131073)
    } else {
        vec![b' '; 4 * 1024 * 1024 + 1]
    };
    let mut stream = Stream::new(
        dictionary! {"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),50.into(),20.into()]},
        bytes,
    );
    stream.compress().unwrap();
    assert!(stream.content.len() < 10000);
    stream
}

#[test]
fn review_global_baseline_import_enforces_decoded_and_operation_limits() {
    for operations in [false, true] {
        let f = fixture();
        let mut identity = FillIdentity::prepare(&f.bytes, &request()).unwrap();
        let mut baseline = Document::load_mem(&identity.probe_bytes().unwrap()).unwrap();
        let bound = identity.bind_native(&baseline).unwrap();
        let stream = baseline.add_object(oversized(operations));
        for node in bound.nodes.iter().filter(|n| n.widget) {
            baseline
                .get_dictionary_mut(node.native)
                .unwrap()
                .set("AP", dictionary! {"N"=>stream});
        }
        assert!(
            materialize_baseline(&mut identity, &baseline, &bound, true).is_err(),
            "oversized baseline imported: operations={operations}"
        );
    }
}

#[test]
fn review_missing_button_state_import_enforces_decoded_and_operation_limits() {
    for operations in [false, true] {
        let mut source = Document::new();
        let original = source.add_object(dictionary! {});
        let mut native = Document::new();
        let state = native.add_object(oversized(operations));
        let widget = native.add_object(
            dictionary! {"AS"=>"Yes","AP"=>dictionary!{"N"=>dictionary!{"Yes"=>state}}},
        );
        let bound = BoundFill {
            nodes: vec![NodeBinding {
                original,
                native: widget,
                widget: true,
                selected: true,
                placements: vec![(0, 0)],
            }],
            changes: vec![],
        };
        assert!(
            stage_buttons(&source, &native, &bound, LIMIT).is_err(),
            "oversized button state imported: operations={operations}"
        );
    }
}

#[test]
fn review_missing_text_normal_and_nested_appearance_imports_are_bounded() {
    for nested in [false, true] {
        for operations in [false, true] {
            let source = Document::new();
            let mut native = Document::new();
            let large = native.add_object(oversized(operations));
            let normal = if nested {
                native.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),50.into(),20.into()],"Resources"=>dictionary!{"XObject"=>dictionary!{"Child"=>large}}},b"/Child Do".to_vec()))
            } else {
                large
            };
            let mut staging = Document::new();
            let mut budget = FillAppearanceBudget::new(&source, &native, &mut staging).unwrap();
            assert!(
                budget.copy_native(&normal.into()).is_err(),
                "oversized text normal imported: nested={nested}, operations={operations}"
            );
            assert!(
                budget.copy_native(&Object::Integer(1)).is_err(),
                "failed admission did not poison importer"
            );
        }
    }
}

#[test]
fn imported_custom_artwork_and_large_font_payloads_remain_supported_and_memoized() {
    let source = Document::new();
    let mut native = Document::new();
    let program = native.add_object(Stream::new(
        dictionary! {"Length1"=>6*1024*1024},
        vec![42; 6 * 1024 * 1024],
    ));
    let descriptor = native.add_object(dictionary! {"Type"=>"FontDescriptor","FontFile"=>program});
    let font = native
        .add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","FontDescriptor"=>descriptor});
    let bytes=b"/Artwork << /Label (not q Q instructions) /Nested [1 << /X 2 >>] >> BDC q 1 0 0 rg 2 2 8 8 re f Q EMC BT /F1 12 Tf (Answer) Tj ET".to_vec();
    let normal=native.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),50.into(),20.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}},bytes.clone()));
    let mut staging = Document::new();
    let mut budget = FillAppearanceBudget::new(&source, &native, &mut staging).unwrap();
    let copied = budget.copy_native(&normal.into()).unwrap();
    for _ in 0..20 {
        assert_eq!(budget.copy_native(&normal.into()).unwrap(), copied);
    }
    drop(budget);
    assert_eq!(staging.objects.len(), 4);
    assert_eq!(
        staging
            .get_object(copied.as_reference().unwrap())
            .unwrap()
            .as_stream()
            .unwrap()
            .get_plain_content()
            .unwrap(),
        bytes
    );
}

#[test]
fn normal_admission_checks_type3_mask_and_pattern_programs_and_aggregate_work() {
    for kind in ["type3", "mask", "pattern"] {
        let mut doc = Document::new();
        let mut large = oversized(true);
        if kind == "pattern" {
            large.dict.remove(b"Subtype");
            large.dict.set("PatternType", 1);
        }
        let child = doc.add_object(large);
        let resource = match kind {
            "type3" => {
                dictionary! {"Font"=>dictionary!{"F1"=>dictionary!{"Subtype"=>"Type3","CharProcs"=>dictionary!{"a"=>child}}}}
            }
            "mask" => {
                dictionary! {"ExtGState"=>dictionary!{"GS"=>dictionary!{"SMask"=>dictionary!{"S"=>"Alpha","G"=>child}}}}
            }
            _ => dictionary! {"Pattern"=>dictionary!{"P"=>child}},
        };
        let root = doc.add_object(Stream::new(
            dictionary! {"Resources"=>resource},
            b"q Q".to_vec(),
        ));
        assert!(
            NormalAppearanceAdmission::new(&doc)
                .admit(&root.into())
                .is_err(),
            "unbounded {kind} program"
        );
    }
    let mut doc = Document::new();
    let roots: Vec<_> = (0..3)
        .map(|_| doc.add_object(Stream::new(dictionary! {}, b"q Q\n".repeat(95000))))
        .collect();
    let mut admission = NormalAppearanceAdmission::new(&doc);
    admission.admit(&roots[0].into()).unwrap();
    for _ in 0..100 {
        admission.admit(&roots[0].into()).unwrap();
    }
    admission.admit(&roots[1].into()).unwrap();
    assert!(
        admission.admit(&roots[2].into()).is_err(),
        "aggregate appearance working budget reset between imports"
    );
    assert!(
        admission.admit(&roots[0].into()).is_err(),
        "failed admission was reusable"
    );
}
