use super::*;
use crate::redaction_content::{canonicalize, ContentBudget, Program, TextFonts};
use lopdf::content::{Content, Operation};

struct Fonts;
impl TextFonts for Fonts {
    fn select(&mut self, _: &[u8]) -> EngineResult<usize> {
        Ok(0)
    }
    fn decode(&self, _: usize, bytes: &[u8]) -> EngineResult<String> {
        String::from_utf8(bytes.to_vec()).map_err(|_| invalid())
    }
}
fn fixture() -> (Document, Dictionary, Arc<Image>, ImageContext) {
    let (mut pdf, value, _) = super::tests::jpeg_fixture(true, Object::Null);
    let mut context = ImageContext::for_jpeg_validation();
    let original = context.begin_document().load(&pdf, &value).unwrap();
    let header = original.jpeg_header().unwrap();
    let mut mask = vec![0; (header.columns * header.rows) as usize];
    mask[0] = 1;
    let selected = Coverage::from_mask(header, &mask, &mut CoverageBudget::default()).unwrap();
    let owned = context.redact_jpeg(&original, &selected).unwrap();
    let id = pdf.add_object(owned.stream().unwrap());
    (
        pdf,
        dictionary! {"XObject"=>dictionary!{"First"=>id,"Again"=>id}},
        owned,
        context,
    )
}
fn occurrence(alias: &str, image: &Image) -> Vec<Operation> {
    let mut operations =
        Content::decode_strict(format!("q 128 0 0 96 20 24 cm /{alias} Do").as_bytes())
            .unwrap()
            .operations;
    operations.extend_from_slice(
        image
            .jpeg_coverage()
            .unwrap()
            .paint(&mut CoverageBudget::default())
            .unwrap()
            .operations(),
    );
    operations.extend(
        Content::decode_strict(b"Q BT /F1 12 Tf 1 0 0 1 20 160 Tm (After image) Tj ET")
            .unwrap()
            .operations,
    );
    operations
}
fn parse(
    pdf: &Document,
    resources: &Dictionary,
    operations: Vec<Operation>,
    context: &mut ImageContext,
) -> EngineResult<Program> {
    let bytes = Content { operations }.encode().unwrap();
    let mut graphics = crate::redaction_graphics::Registry::default();
    let mut states =
        crate::redaction_graphics::PageStates::new(pdf, Some(resources), &mut graphics);
    let mut images = PageImages::new(pdf, Some(resources), context.begin_document(), true);
    canonicalize(
        &bytes,
        &mut Fonts,
        &mut states,
        &mut images,
        &mut ContentBudget::default(),
    )
}
#[test]
fn redaction_image_source_canonicalization_strips_verified_paint_before_text_indices() {
    let (pdf, resources, image, mut context) = fixture();
    let mut ops = occurrence("First", &image);
    ops.extend(occurrence("Again", &image));
    assert!(
        parse(&pdf, &resources, ops.clone(), &mut ImageContext::default()).is_err(),
        "ordinary admission must stay gated"
    );
    let program = parse(&pdf, &resources, ops.clone(), &mut context).unwrap();
    assert_eq!(program.operations.len(), 18);
    assert_eq!(program.blocks.len(), 2);
    for (block, index) in program.blocks.iter().zip([7, 16]) {
        assert_eq!(block.shows, vec![index]);
        assert_eq!(block.text, "After image");
        assert_eq!(program.operations[index].operator, "Tj");
    }
    let again = parse(&pdf, &resources, ops, &mut context).unwrap();
    assert_eq!(
        Content {
            operations: again.operations
        }
        .encode()
        .unwrap(),
        Content {
            operations: program.operations
        }
        .encode()
        .unwrap()
    );
}
#[test]
fn redaction_image_source_cached_content_never_authorizes_bad_occurrence_or_context() {
    let (pdf, resources, image, mut context) = fixture();
    let good = occurrence("First", &image);
    parse(&pdf, &resources, good.clone(), &mut context).unwrap();
    for mode in 0..5 {
        let mut bad = occurrence("Again", &image);
        match mode {
            0 => bad[4].operands[0] = 1.into(), // owned fill changed from black
            1 => {
                bad.remove(3);
            } // missing saved graphics state in owned paint
            2 => {
                bad.insert(0, Operation::new("m", vec![0.into(), 0.into()]));
            }
            3 => {
                bad.splice(
                    0..0,
                    Content::decode_strict(b"0 0 50 50 re W n")
                        .unwrap()
                        .operations,
                );
            }
            _ => bad.push(Operation::new("Unknown", vec![])),
        }
        let mut both = good.clone();
        both.extend(bad);
        assert!(
            parse(&pdf, &resources, both, &mut context).is_err(),
            "mode={mode}"
        );
        assert!(
            parse(&pdf, &resources, good.clone(), &mut context).is_ok(),
            "refusal poisoned cache"
        );
    }
}
