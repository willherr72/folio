use super::*;
use lopdf::{dictionary, Object};

#[test]
fn bs_precedence_and_fractional_width_are_explicit() {
    let doc = Document::new();
    let widget = dictionary! {"BS"=>dictionary!{"W"=>0.5,"S"=>"D","D"=>vec![3.into(),2.into()]},"Border"=>vec![0.into(),0.into(),7.into()]};
    let m = metadata(&doc, &widget).unwrap();
    assert_eq!(m.width, 0.5);
    assert_eq!(m.style, FrameStyle::Dashed);
    assert_eq!(m.dash, vec![3., 2.]);
}
#[test]
fn legacy_border_and_absent_defaults_are_distinct_from_transparency() {
    let doc = Document::new();
    let m = metadata(&doc, &dictionary! {}).unwrap();
    assert_eq!(m.width, 1.);
    assert_eq!(m.border, FramePalette::Missing);
    let m=metadata(&doc,&dictionary!{"Border"=>vec![0.into(),0.into(),0.into()],"MK"=>dictionary!{"BC"=>Vec::<Object>::new(),"BG"=>Vec::<Object>::new()}}).unwrap();
    assert_eq!(m.width, 0.);
    assert_eq!(m.border, FramePalette::Transparent);
    assert_eq!(m.background, FramePalette::Transparent);
    let m=metadata(&doc,&dictionary!{"Border"=>vec![0.into(),0.into(),2.into(),Object::Array(vec![4.into(),1.into()])]}).unwrap();
    assert_eq!(m.style, FrameStyle::Dashed);
    assert_eq!(m.dash, vec![4., 1.]);
}
#[test]
fn gray_cmyk_and_indirect_numeric_values_remain_exact() {
    let mut doc = Document::new();
    let gray = doc.add_object(Object::Real(0.25));
    let width = doc.add_object(Object::Real(0.75));
    let color = doc.add_object(vec![0.into(), 1.into(), 0.into(), 0.into()]);
    let mk = doc.add_object(dictionary! {"BC"=>vec![gray.into()],"BG"=>color});
    let bs = doc.add_object(dictionary! {"W"=>width,"S"=>"I"});
    let m = metadata(&doc, &dictionary! {"MK"=>mk,"BS"=>bs}).unwrap();
    assert_eq!(m.width, 0.75);
    assert_eq!(m.style, FrameStyle::Inset);
    assert_eq!(m.border, FramePalette::Color(PaintColor::Gray(0.25)));
    assert_eq!(
        m.background,
        FramePalette::Color(PaintColor::Cmyk([0., 1., 0., 0.]))
    );
}
#[test]
fn unknown_malformed_rounded_and_excessive_settings_refuse() {
    let doc = Document::new();
    for w in [
        dictionary! {"BS"=>dictionary!{"W"=>Object::Real(f32::INFINITY)}},
        dictionary! {"BS"=>dictionary!{"W"=>-1}},
        dictionary! {"BS"=>dictionary!{"W"=>i64::MAX}},
        dictionary! {"BS"=>dictionary!{"S"=>"Unknown"}},
        dictionary! {"Border"=>vec![1.into(),0.into(),1.into()]},
        dictionary! {"Border"=>vec![0.into(),0.into()]},
        dictionary! {"BS"=>dictionary!{"S"=>"D","D"=>vec![0.into(),0.into()]}},
        dictionary! {"MK"=>dictionary!{"BC"=>vec![0.into(),0.into()]}},
        dictionary! {"MK"=>dictionary!{"BG"=>vec![2.into()]}},
        dictionary! {"BS"=>dictionary!{"S"=>"D","D"=>vec![1.into();65]}},
    ] {
        assert!(metadata(&doc, &w).is_err(), "accepted {w:?}");
    }
}
#[test]
fn inactive_malformed_legacy_border_is_not_a_future_fill_trap() {
    let doc = Document::new();
    assert!(metadata(&doc,&dictionary!{"BS"=>dictionary!{"W"=>1},"Border"=>vec![0.into(),0.into(),Object::Real(f32::INFINITY)]}).is_err());
}
#[test]
fn bounded_references_and_null_optional_entries() {
    let mut doc = Document::new();
    let id = doc.new_object_id();
    doc.objects.insert(id, id.into());
    assert!(metadata(&doc, &dictionary! {"BS"=>id}).is_err());
    let m = metadata(
        &doc,
        &dictionary! {"BS"=>Object::Null,"MK"=>Object::Null,"Border"=>Object::Null},
    )
    .unwrap();
    assert_eq!(m.width, 1.);
    assert_eq!(m.style, FrameStyle::Solid);
}
fn request(changes: serde_json::Value) -> SetFormFrameRequest {
    serde_json::from_value(serde_json::json!({"annotationIndex":0,"expectedBounds":{"x":0,"y":0,"width":100,"height":30},"expectedValue":{"kind":"text","text":"Answer"},"expectedStyleKey":"a".repeat(64),"changes":changes})).unwrap()
}
#[test]
fn physical_request_endpoints_and_color_modes_validate() {
    for width in [0., 0.25, 32.] {
        assert!(request(serde_json::json!({"width":width}))
            .validate()
            .is_ok());
    }
    for width in [-0.1, 32.01] {
        assert!(request(serde_json::json!({"width":width}))
            .validate()
            .is_err());
    }
    for color in [
        serde_json::json!({"mode":"transparent"}),
        serde_json::json!({"mode":"rgb","color":"#aB09ff"}),
    ] {
        assert!(request(serde_json::json!({"border":color}))
            .validate()
            .is_ok());
    }
    for color in ["red", "#123", "#zz9999", "#FFFFFF00"] {
        assert!(
            request(serde_json::json!({"background":{"mode":"rgb","color":color}}))
                .validate()
                .is_err()
        );
    }
    assert!(request(serde_json::json!({})).validate().is_err());
    let mut r = request(serde_json::json!({"width":1}));
    r.changes.width = Some(f32::NAN);
    assert!(r.validate().is_err());
    let mut r = request(serde_json::json!({"width":1}));
    r.expected_bounds.x = f32::MAX;
    r.expected_bounds.width = f32::MAX;
    assert!(r.validate().is_err());
    let mut r = request(serde_json::json!({"width":1}));
    r.expected_style_key = "z".repeat(64);
    assert!(r.validate().is_err());
}
