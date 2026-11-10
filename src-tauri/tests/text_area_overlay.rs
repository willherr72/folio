#![cfg(feature = "shaped-text")]
use folio_engine::*;
use serde_json::{json, Value};

fn font() -> FontAsset {
    FontAsset::parse(include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf").to_vec())
        .unwrap()
}
fn overlay_value(font: &FontAsset, text: &str) -> Value {
    json!({"id":"area","x":20,"y":30,"text":text,"fontSize":18,"fontId":font.info.id,
        "shaping":{"version":1,"direction":"ltr","ligatures":true},"color":"#123456","rotation":0,
        "area":{"version":1,"width":180,"height":240,"inset":5,"lineSpacing":1.2,"alignment":"center"}})
}
#[test]
fn area_settings_survive_schema_round_trip_and_unknown_versions_are_rejected() {
    let value = overlay_value(&font(), "Hello");
    let overlay: TextOverlay = serde_json::from_value(value.clone()).unwrap();
    let saved = serde_json::to_value(&overlay).unwrap();
    assert!(saved.get("area").is_some());
    let expected: TextAreaSettings = serde_json::from_value(value["area"].clone()).unwrap();
    assert_eq!(overlay.area.as_ref(), Some(&expected));
    assert_eq!(
        serde_json::from_value::<TextOverlay>(saved).unwrap(),
        overlay
    );
    for field in ["version", "unknown"] {
        let mut bad = value.clone();
        bad["area"][field] = json!(99);
        assert!(serde_json::from_value::<TextOverlay>(bad).is_err());
    }
    let mut legacy = value;
    legacy.as_object_mut().unwrap().remove("area");
    let overlay: TextOverlay = serde_json::from_value(legacy).unwrap();
    assert!(serde_json::to_value(overlay).unwrap().get("area").is_none());
}
#[test]
fn prepared_area_preserves_exact_source_and_full_rectangle() {
    let font = font();
    let source = "office  café and more words\r\n\r\nA\u{a0}B  \n";
    let overlay: TextOverlay = serde_json::from_value(overlay_value(&font, source)).unwrap();
    let prepared = prepare_text_overlay(&font, &overlay).unwrap();
    assert_eq!(prepared.preview.text, source);
    assert_eq!(
        prepared
            .characters
            .iter()
            .map(|c| c.text.as_str())
            .collect::<String>(),
        source
    );
    assert_eq!(
        prepared.bounds,
        AnnotationRect {
            x: 0.,
            y: 0.,
            width: 180.,
            height: 240.
        }
    );
    assert_eq!(prepared.origin, [0., 222.]);
    assert!(!prepared.preview.glyphs.is_empty());
    assert!(prepared
        .characters
        .iter()
        .all(|c| c.x.is_finite() && c.y.is_finite()));
}
#[test]
fn areas_refuse_overflow_invalid_geometry_and_non_ltr_shaping() {
    let font = font();
    let base = overlay_value(&font, "A whole paragraph needs more than a tiny box");
    for (key, value) in [
        ("height", json!(5)),
        ("width", json!(0)),
        ("inset", json!(-1)),
        ("lineSpacing", json!(4)),
    ] {
        let mut bad = base.clone();
        bad["area"][key] = value;
        let overlay = serde_json::from_value(bad).unwrap();
        assert!(prepare_text_overlay(&font, &overlay).is_err(), "{key}");
    }
    let mut bad = base;
    bad["shaping"]["direction"] = json!("auto");
    assert!(prepare_text_overlay(&font, &serde_json::from_value(bad).unwrap()).is_err());
}
#[test]
fn empty_and_blank_areas_keep_their_original_text() {
    let font = font();
    for source in ["", "  \r\n\n "] {
        let overlay = serde_json::from_value(overlay_value(&font, source)).unwrap();
        let prepared = prepare_text_overlay(&font, &overlay).unwrap();
        assert_eq!(prepared.preview.text, source);
        assert_eq!(
            prepared
                .characters
                .iter()
                .map(|c| c.text.as_str())
                .collect::<String>(),
            source
        );
        assert_eq!(prepared.bounds.width, 180.);
    }
}
