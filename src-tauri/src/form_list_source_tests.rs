use super::*;
use lopdf::{dictionary, Document};

fn source(name: &str) -> (Document, Vec<u8>, Dictionary) {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(format!("tests/fixtures/form-compound-list-{name}.pdf"));
    let doc = Document::load(&path).unwrap();
    let page = doc.get_dictionary(doc.get_pages()[&1]).unwrap();
    let widget = resolve(&doc, &page.get(b"Annots").unwrap().as_array().unwrap()[0])
        .unwrap().as_dict().unwrap();
    let ap = resolve(&doc, widget.get(b"AP").unwrap()).unwrap().as_dict().unwrap();
    let appearance = resolve(&doc, ap.get(b"N").unwrap()).unwrap().as_stream().unwrap();
    let data = appearance.get_plain_content_with_limit(MAX_BYTES).unwrap();
    let resources = resolve(&doc, appearance.dict.get(b"Resources").unwrap())
        .unwrap().as_dict().unwrap().clone();
    (doc, data, resources)
}

fn model(selected: &[usize]) -> ListModel {
    ListModel {
        options: vec!["Red".into(), "Green".into(), "Blue".into()],
        selected: selected.to_vec(),
        multiple: selected.len() > 1,
    }
}

#[test]
fn list_source_structure_refuses_mixed_paths_even_across_graphics_restore() {
    let(doc,bytes,resources)=source("single-selection-paint");
    let body=String::from_utf8(bytes).unwrap();
    let paint="q 0.7 0.8 1 rg 1 49.000000 248 16 re f Q";
    for (variant,replacement) in [
        "q 0.7 0.8 1 rg 180 15 m 195 15 l 195 30 l h 1 49 248 16 re f n Q",
        "180 15 m 195 15 l 195 30 l h q 0.7 0.8 1 rg 1 49 248 16 re f n Q",
        "q 180 15 m 195 15 l 195 30 l h Q q 0.7 0.8 1 rg 1 49 248 16 re f n Q",
        "q 0.7 0.8 1 rg 1 49 248 16 re 180 15 m 195 15 l 195 30 l h f n Q",
    ].into_iter().enumerate() {
        let changed=body.replace(paint,replacement);assert_ne!(changed,body);
        let error=analyse(&doc,&resources,changed.as_bytes(),[0.,0.,250.,70.],&model(&[0]),0)
            .err().expect("mixed subpaths authorized removal of unrelated source artwork");
        let message=error.to_string();
        assert!(matches!(error,crate::EngineError::InvalidRequest(_)),"{message}");
        if variant==0 {assert!(message.contains("unidentified paint"),"{message}");}
    }
    assert!(analyse(&doc,&resources,body.as_bytes(),[0.,0.,250.,70.],&model(&[0]),0).is_ok());
}

#[test]
fn list_source_structure_refuses_selection_paint_after_its_row_text() {
    let(doc,bytes,resources)=source("single-selection-paint");let body=String::from_utf8(bytes).unwrap();
    let paint="q 0.7 0.8 1 rg 1 49.000000 248 16 re f Q";
    let changed=body.replace(paint,"").replace("ET\nQ",&format!("ET\n{paint}\nQ"));assert_ne!(changed,body);
    let error=analyse(&doc,&resources,changed.as_bytes(),[0.,0.,250.,70.],&model(&[0]),0)
        .err().expect("occluding foreground was treated as a visible row background").to_string();
    assert!(error.contains("precedes"),"{error}");
    assert!(analyse(&doc,&resources,body.as_bytes(),[0.,0.,250.,70.],&model(&[0]),0).is_ok());
}

#[test]
fn list_source_structure_observed_pitch_bounds_dormant_positioning_jumps() {
    let(doc,bytes,resources)=source("single-selection-paint");let body=String::from_utf8(bytes).unwrap();
    let shifted=body.replace("2 68 Td\n0 -15.624001 Td","2 1052.3759765625 Td\n0 -1000 Td");assert_ne!(shifted,body);
    for tall in [false,true] {
        let mut changed=shifted.replace("1 49.000000 248 16 re","1 48 248 21 re");
        let height=if tall {changed=changed.replace(".5 .5 249 69 re",".5 .5 249 249 re")
            .replace("1 1 248 68 re","1 1 248 248 re").replace("1 48 248 21 re","1 48 248 201 re");250.}else{70.};
        let error=analyse(&doc,&resources,changed.as_bytes(),[0.,0.,250.,height],&model(&[0]),0)
            .err().expect("raw Td displacement authorized a background beyond its observed row band").to_string();
        assert!(error.contains("exceeds its row band"),"{error}");
    }
}

#[test]
fn list_source_structure_lone_visible_row_requires_observed_band_authority() {
    let(doc,bytes,resources)=source("single-selection-paint");let body=String::from_utf8(bytes).unwrap();
    let body=body.replace("0 -15.624001 Td\n/Helv 14 Tf\n(Green) Tj\n","")
        .replace("0 -15.624001 Td\n/Helv 14 Tf\n(Blue) Tj\n","");
    let mut one=model(&[0]);one.options.truncate(1);
    for changed in [body.clone(),body.replace("2 68 Td\n0 -15.624001 Td","2 1052.3759765625 Td\n0 -1000 Td")
        .replace("1 49.000000 248 16 re","1 48 248 21 re")] {
        let error=analyse(&doc,&resources,changed.as_bytes(),[0.,0.,250.,70.],&one,0)
            .err().expect("a lone positioning move supplied unproved row-band authority").to_string();
        assert!(error.contains("row height cannot be proved"),"{error}");
    }
}

#[test]
fn list_source_structure_scaled_text_uses_actual_baseline_pitch() {
    let(doc,bytes,resources)=source("single-selection-paint");let body=String::from_utf8(bytes).unwrap()
        .replace("2 68 Td","2 0 0 2 0 0 Tm 1 34 Td")
        .replace("0 -15.624001 Td","0 -7.8120005 Td").replace("/Helv 14 Tf","/Helv 7 Tf");
    let layout=analyse(&doc,&resources,body.as_bytes(),[0.,0.,250.,70.],&model(&[0]),0).unwrap();
    assert!((layout.rows[0].origin[1]-layout.rows[1].origin[1]-15.624001).abs()<0.0001);
    assert_eq!(layout.paints.len(),1);
}

fn inspect(name: &str, model: &ListModel, top: usize) -> EngineResult<Layout> {
    let (doc, bytes, resources) = source(name);
    analyse(&doc, &resources, &bytes, [0., 0., 250., 70.], model, top)
}

#[test]
fn list_source_structure_binds_row_order_and_selection_without_palette_guess() {
    for (name, indices, count) in [
        ("single-selection-paint", vec![0], 1),
        ("multi-selection-paint", vec![0, 2], 2),
    ] {
        let layout = inspect(name, &model(&indices), 0).unwrap();
        assert_eq!(layout.rows.iter().map(|row| row.index).collect::<Vec<_>>(), vec![0, 1, 2]);
        assert_eq!(layout.rows.iter().map(|row| row.label.as_str()).collect::<Vec<_>>(), vec!["Red", "Green", "Blue"]);
        assert_eq!(layout.paints.len(), count);
        assert_eq!(layout.clip, [1., 1., 249., 69.]);
    }
}

#[test]
fn list_source_structure_binds_scrolled_rows_by_index() {
    let model = ListModel {
        options: (1..=15).map(|index| format!("Option {index:02}")).collect(),
        selected: vec![5], multiple: false,
    };
    let layout = inspect("scrolled-selection-paint", &model, 5).unwrap();
    assert_eq!(layout.rows.iter().map(|row| row.index).collect::<Vec<_>>(), vec![5, 6, 7, 8]);
    assert!(inspect("scrolled-selection-paint", &model, 0).is_err());
    assert!(inspect("scrolled-selection-paint", &model, 15).is_err());
}

#[test]
fn list_source_structure_refuses_unmatched_artwork_labels_and_selection() {
    assert!(inspect("ambiguous-artwork", &model(&[0]), 0).is_err());
    assert!(inspect("single-selection-paint", &model(&[]), 0).is_err());
    assert!(inspect("single-selection-paint", &model(&[1]), 0).is_err());
    let mut wrong = model(&[0]);
    wrong.options[0] = "Other label".into();
    assert!(inspect("single-selection-paint", &wrong, 0).is_err());
}

#[test]
fn list_source_structure_does_not_infer_index_from_duplicate_label() {
    let (doc, bytes, resources) = source("single-selection-paint");
    let bytes = String::from_utf8(bytes).unwrap().replace("(Green)", "(Red)");
    let mut model = model(&[0]);
    model.options[1] = "Red".into();
    let layout = analyse(&doc, &resources, bytes.as_bytes(), [0., 0., 250., 70.], &model, 0).unwrap();
    assert_eq!(layout.rows[1].index, 1);
    model.selected = vec![1];
    assert!(analyse(&doc, &resources, bytes.as_bytes(), [0., 0., 250., 70.], &model, 0).is_err());
}

#[test]
fn list_source_structure_accepts_split_zero_adjustment_text_but_refuses_unknown_advance() {
    let (doc, bytes, resources) = source("single-selection-paint");
    let text = String::from_utf8(bytes).unwrap();
    for (replacement, accepted) in [("[(R) 0 (ed)] TJ", true), ("[(R) 300 (ed)] TJ", false)] {
        let bytes = text.replace("(Red) Tj", replacement);
        assert_eq!(analyse(&doc, &resources, bytes.as_bytes(), [0., 0., 250., 70.], &model(&[0]), 0).is_ok(), accepted);
    }
}

#[test]
fn list_source_structure_refuses_hidden_text_and_unproved_font_mapping() {
    let (doc, bytes, mut resources) = source("single-selection-paint");
    let hidden = String::from_utf8(bytes.clone()).unwrap().replace("/Helv 14 Tf", "/Helv 14 Tf 3 Tr");
    assert!(analyse(&doc, &resources, hidden.as_bytes(), [0., 0., 250., 70.], &model(&[0]), 0).is_err());
    resources.set("Font", dictionary! {"Helv"=>dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Symbol"}});
    assert!(analyse(&doc, &resources, &bytes, [0., 0., 250., 70.], &model(&[0]), 0).is_err());
}

#[test]
fn list_source_structure_refuses_clip_branch_and_graphics_escape() {
    let (doc, bytes, resources) = source("single-selection-paint");
    let text = String::from_utf8(bytes).unwrap();
    for replacement in [
        "ET Q q BT /Helv 14 Tf (Green) Tj",
        "ET q 20 5 40 10 re W n BT /Helv 14 Tf (Green) Tj ET Q BT /Helv 14 Tf",
    ] {
        let bytes = text.replace("(Green) Tj", replacement);
        assert!(analyse(&doc, &resources, bytes.as_bytes(), [0., 0., 250., 70.], &model(&[0]), 0).is_err());
    }
}

#[test]
fn list_source_structure_requires_paint_to_span_source_clip_column() {
    let (doc, bytes, resources) = source("single-selection-paint");
    let text = String::from_utf8(bytes).unwrap();
    for rectangle in ["2 49.000000 247 16 re", "1 49.000000 100 16 re", "1 1 248 68 re"] {
        let bytes = text.replace("1 49.000000 248 16 re", rectangle);
        assert!(analyse(&doc, &resources, bytes.as_bytes(), [0., 0., 250., 70.], &model(&[0]), 0).is_err());
    }
}

#[test]
fn list_source_structure_refuses_byte_operation_and_glyph_work_overflow() {
    let(doc,bytes,resources)=source("single-selection-paint");
    let error=analyse(&doc,&resources,&vec![b' ';MAX_BYTES+1],[0.,0.,250.,70.],&model(&[0]),0).err().expect("byte limit missing").to_string();
    assert!(error.contains("appearance exceeds 4 MiB"),"{error}");
    let too_many="q Q ".repeat(MAX_OPS/2+1);
    let error=analyse(&doc,&resources,too_many.as_bytes(),[0.,0.,250.,70.],&model(&[0]),0).err().expect("operation limit missing").to_string();
    assert!(error.contains("appearance parser working set exceeds"),"{error}");
    let text=String::from_utf8(bytes).unwrap();let start=text.find("BT").unwrap();let end=text.rfind("ET").unwrap()+2;
    let label="R".repeat(256);
    let mut body=String::from("BT /Helv 14 Tf 2 52.376 Td ");
    for _ in 0..2000{body.push_str(&format!("0 -1 Td ({label}) Tj \n"));}body.push_str("ET");
    let program=format!("{}{}{}",&text[..start],body,&text[end..]);
    let model=ListModel{options:vec![label;2000],selected:vec![0],multiple:false};
    let error=analyse(&doc,&resources,program.as_bytes(),[0.,0.,250.,70.],&model,0).err().expect("unbounded native glyph work").to_string();
    assert!(error.contains("glyph work exceeds"),"{error}");
}

#[test]
fn list_source_structure_accepts_separate_text_objects_with_explicit_row_origins() {
    let(doc,bytes,resources)=source("single-selection-paint");
    let text=String::from_utf8(bytes).unwrap();let start=text.find("BT").unwrap();let end=text.rfind("ET").unwrap()+2;
    let mut body=String::new();
    for(index,label)in ["Red","Green","Blue"].iter().enumerate(){
        body.push_str(&format!("BT /Helv 14 Tf 2 {} Td ({label}) Tj ET\n",52.375999-15.624001*index as f64));
    }
    let program=format!("{}{}{}",&text[..start],body,&text[end..]);
    assert_eq!(analyse(&doc,&resources,program.as_bytes(),[0.,0.,250.,70.],&model(&[0]),0).unwrap().rows.len(),3);
}

