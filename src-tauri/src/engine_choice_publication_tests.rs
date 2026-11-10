use super::*;
use crate::form_choice_appearance::{geometry, verify_unchanged, AppearanceBudget};
use crate::form_choice_text::ChoiceTextBlock;
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream};

fn runtime() -> WorkerRuntime {
    WorkerRuntime::new(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
        Arc::new(FontRegistry::new()),
    ).unwrap()
}
fn normal(doc: &Document, widget: ObjectId) -> Stream {
    let w = doc.get_dictionary(widget).unwrap();
    let ap = doc.dereference(w.get(b"AP").unwrap()).unwrap().1.as_dict().unwrap();
    doc.dereference(ap.get(b"N").unwrap()).unwrap().1.as_stream().unwrap().clone()
}
fn array(values: &[f32]) -> Object {
    Object::Array(values.iter().copied().map(Object::Real).collect())
}
fn appearance(bbox: &[f32], matrix: &[f32]) -> Stream {
    Stream::new(dictionary! {"BBox"=>array(bbox), "Matrix"=>array(matrix)}, vec![])
}

#[test]
fn choice_geometry_maps_local_bounds_without_canceling_the_original_matrix() {
    let d = Document::new();
    let source = appearance(&[7.,11.,207.,71.], &[1.,0.2,0.15,1.,7.,11.]);
    let generated = appearance(&[0.,0.,100.,20.], &[0.,1.,-1.,0.,20.,0.]);
    assert_eq!(geometry(&d,&source,&d,&generated).unwrap(), [2.,0.,0.,3.,7.,11.]);
    assert_eq!(source.dict.get(b"Matrix").unwrap(), &array(&[1.,0.2,0.15,1.,7.,11.]));
}

#[test]
fn choice_indirect_geometry_is_resolved_and_invalid_geometry_refused() {
    let mut d = Document::new();
    let zero = d.add_object(0);
    let bbox = d.add_object(Object::Array(vec![zero.into(),zero.into(),100.into(),20.into()]));
    let matrix = d.add_object(array(&[1.,0.,0.,1.,0.,0.]));
    let source = Stream::new(dictionary! {"BBox"=>bbox,"Matrix"=>matrix},vec![]);
    assert_eq!(geometry(&d,&source,&d,&source).unwrap(), [1.,0.,0.,1.,0.,0.]);
    for bad_box in [vec![],vec![0.,0.,0.,20.],vec![0.,0.,100.,-20.],vec![0.,0.,f32::NAN,20.]] {
        let mut invalid = source.clone(); invalid.dict.set("BBox",array(&bad_box));
        assert!(geometry(&d,&invalid,&d,&source).is_err());
        assert!(geometry(&d,&source,&d,&invalid).is_err());
    }
    for bad_matrix in [vec![],vec![0.,0.,0.,1.,0.,0.],vec![1.,2.,2.,4.,0.,0.],vec![1.,0.,0.,1.,f32::INFINITY,0.]] {
        let mut invalid = source.clone(); invalid.dict.set("Matrix",array(&bad_matrix));
        assert!(geometry(&d,&invalid,&d,&source).is_err());
        assert!(geometry(&d,&source,&d,&invalid).is_err());
    }
    let tiny = appearance(&[0.,0.,1e-20,20.],&[1.,0.,0.,1.,0.,0.]);
    assert!(geometry(&d,&source,&d,&tiny).is_err());
}

#[test]
fn choice_shifted_scaled_rotated_and_affine_sources_keep_native_noop_pixels() {
    let r = runtime();
    for kind in ["combo","list"] {
        for rotation in [0,90] {
            for variant in ["canonical","shift","scale","affine-matrix","stale-color"] {
                let name = format!("{kind}-{rotation}-{variant}");
                let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("../tests/fixtures/choice-appearance/{name}-source.pdf"));
                let bytes = std::fs::read(path).unwrap();
                let ctx = crate::form_properties::load(&bytes,0,0).unwrap();
                let page = *ctx.doc.get_pages().values().next().unwrap();
                let annots = ctx.doc.dereference(ctx.doc.get_dictionary(page).unwrap().get(b"Annots").unwrap()).unwrap().1.as_array().unwrap();
                let widget = annots[0].as_reference().unwrap();
                assert!(ctx.widgets.contains(&widget));
                let original = normal(&ctx.doc,widget);
                let generated = crate::form_choice_probe::generate_choice_appearance(r.bindings,&ctx,widget).unwrap();
                let old = ChoiceTextBlock::parse(&original.get_plain_content().unwrap()).unwrap();
                let new = ChoiceTextBlock::parse(&generated.appearance.get_plain_content().unwrap()).unwrap();
                let mapping = geometry(&ctx.doc,&original,&generated.document,&generated.appearance).unwrap();
                let content = old.transplant_mapped(&new,&generated.font_names,mapping).unwrap();
                let result = verify_unchanged(&r.pdfium,&ctx.doc,widget,page,&content,&mut AppearanceBudget::default());
                assert_eq!(result.is_ok(),variant!="stale-color","{name}: {result:?}");
                assert_eq!(normal(&ctx.doc,widget).dict,original.dict);
                assert!(normal(&ctx.doc,widget).content==original.content,"source modified: {name}");
            }
        }
    }
}

fn simple_source(text: &[u8], width: f32, height: f32) -> (Document,ObjectId,ObjectId) {
    let mut d = Document::with_version("1.7");
    let pages = d.new_object_id();
    let page = d.new_object_id();
    let font = d.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let ap = d.add_object(Stream::new(dictionary! {"Type"=>"XObject","Subtype"=>"Form",
        "BBox"=>array(&[0.,0.,width,height]),"Resources"=>dictionary! {"Font"=>dictionary! {"F1"=>font}}},text.to_vec()));
    let widget = d.add_object(dictionary! {"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Ch",
        "Rect"=>array(&[0.,0.,width,height]),"P"=>page,"AP"=>dictionary! {"N"=>ap}});
    d.objects.insert(page,dictionary! {"Type"=>"Page","Parent"=>pages,"MediaBox"=>array(&[0.,0.,width,height]),
        "Annots"=>vec![widget.into()],"Resources"=>Dictionary::new()}.into());
    d.objects.insert(pages,dictionary! {"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());
    let catalog = d.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    d.trailer.set("Root",catalog);
    (d,widget,page)
}

#[test]
fn choice_transparent_proof_detects_white_ink_removal_and_accepts_equivalent_content() {
    let r = runtime();
    let original = b"/Tx BMC q BT /F1 20 Tf 1 g 10 30 Td (Answer) Tj ET Q EMC";
    let equivalent = b"/Tx BMC q BT /F1 20.0 Tf 1.0 g 10.0 30.0 Td <416e73776572> Tj ET Q EMC";
    let (doc,widget,page) = simple_source(original,200.,80.);
    assert!(verify_unchanged(&r.pdfium,&doc,widget,page,equivalent,&mut AppearanceBudget::default()).is_ok());
    let error = verify_unchanged(&r.pdfium,&doc,widget,page,b"/Tx BMC q Q EMC",&mut AppearanceBudget::default()).unwrap_err();
    assert!(error.to_string().contains("appearance"),"{error}");
}

#[test]
fn choice_physical_scale_prevents_tiny_raw_glyphs_from_becoming_false_noops() {
    let r = runtime();
    let a = b"/Tx BMC q BT /F1 0.02 Tf 0 g 0.1 0.5 Td (A) Tj ET Q EMC";
    let b = b"/Tx BMC q BT /F1 0.02 Tf 0 g 0.1 0.5 Td (B) Tj ET Q EMC";
    let (mut doc,widget,page) = simple_source(a,1.,1.);
    doc.get_dictionary_mut(page).unwrap().set("UserUnit",128);
    assert!(verify_unchanged(&r.pdfium,&doc,widget,page,a,&mut AppearanceBudget::default()).is_ok());
    assert!(verify_unchanged(&r.pdfium,&doc,widget,page,b,&mut AppearanceBudget::default()).is_err());
}

#[test]
fn choice_local_and_indirect_units_work_and_oversized_proofs_are_refused() {
    let r = runtime();
    let text = b"/Tx BMC q BT /F1 12 Tf 0 g 10 20 Td (Answer) Tj ET Q EMC";
    let (mut doc,widget,page) = simple_source(text,100.,40.);
    for unit in [0.5,1.7,8.] {
        let value = doc.add_object(Object::Real(unit));
        doc.get_dictionary_mut(page).unwrap().set("UserUnit",value);
        assert!(verify_unchanged(&r.pdfium,&doc,widget,page,text,&mut AppearanceBudget::default()).is_ok(),"{unit}");
    }
    for unit in [0.,-1.,f32::NAN,75001.] {
        doc.get_dictionary_mut(page).unwrap().set("UserUnit",Object::Real(unit));
        assert!(verify_unchanged(&r.pdfium,&doc,widget,page,text,&mut AppearanceBudget::default()).is_err(),"{unit}");
    }
    doc.get_dictionary_mut(page).unwrap().set("UserUnit",75000);
    let error=verify_unchanged(&r.pdfium,&doc,widget,page,text,&mut AppearanceBudget::default()).unwrap_err();
    assert!(error.to_string().contains("budget") || error.to_string().contains("limit"),"{error}");
    doc.get_dictionary_mut(page).unwrap().remove(b"UserUnit");
    let parent=doc.get_dictionary(page).unwrap().get(b"Parent").unwrap().as_reference().unwrap();
    doc.get_dictionary_mut(parent).unwrap().set("UserUnit",75000);
    assert!(verify_unchanged(&r.pdfium,&doc,widget,page,text,&mut AppearanceBudget::default()).is_ok());
}

#[test]
fn choice_missing_local_font_cannot_authorize_two_blank_renders() {
    let r = runtime();
    let text = b"/Tx BMC q BT /F1 12 Tf 0 g 10 20 Td (Answer) Tj ET Q EMC";
    let (mut doc,widget,page) = simple_source(text,100.,40.);
    let mut saved = normal(&doc,widget);
    saved.dict.set("Resources",Dictionary::new());
    let ap = doc.add_object(saved);
    doc.get_dictionary_mut(widget).unwrap().set("AP",dictionary! {"N"=>ap});
    let error=verify_unchanged(&r.pdfium,&doc,widget,page,text,&mut AppearanceBudget::default()).unwrap_err();
    assert!(error.to_string().contains("font") || error.to_string().contains("resource"),"{error}");
}

#[test]
fn choice_stale_size_color_placement_or_answer_cannot_be_silently_repainted() {
    let r = runtime();
    let text = "/Tx BMC q BT /F1 12 Tf 0 g 10 20 Td (Answer) Tj ET Q EMC";
    let (doc,widget,page) = simple_source(text.as_bytes(),100.,40.);
    for candidate in [text.replace("12 Tf","14 Tf"),text.replace("0 g","1 g"),
        text.replace("10 20 Td","20 20 Td"),text.replace("Answer","Other")] {
        assert!(verify_unchanged(&r.pdfium,&doc,widget,page,candidate.as_bytes(),&mut AppearanceBudget::default()).is_err(),"{candidate}");
    }
}

#[test]
fn choice_linked_appearance_work_is_bounded_across_successful_proofs() {
    let r = runtime();
    let empty = b"q Q";
    let (doc,widget,page) = simple_source(empty,1000.,1000.);
    let mut budget = AppearanceBudget::default();
    verify_unchanged(&r.pdfium,&doc,widget,page,empty,&mut budget).unwrap();
    // Each bitmap is admissible, but another full three-scale comparison
    // exceeds the transaction-wide work budget (both images must be charged).
    let error=verify_unchanged(&r.pdfium,&doc,widget,page,empty,&mut budget).unwrap_err();
    assert!(error.to_string().contains("budget"),"{error}");
    // A rejected reservation cannot consume work which was never performed.
    let (small,widget,page)=simple_source(empty,1.,1.);
    verify_unchanged(&r.pdfium,&small,widget,page,empty,&mut budget).unwrap();
}
