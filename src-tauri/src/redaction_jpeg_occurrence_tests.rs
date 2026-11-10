use super::*;
use crate::{redaction_content::Program, redaction_images::ImageContext};
use lopdf::{content::Content, dictionary, Document, Stream};
fn source(context: &mut ImageContext) -> Arc<Image> {
    let mut pdf = Document::new();
    let value=Object::Reference(pdf.add_object(Stream::new(dictionary!{"Subtype"=>"Image","Width"=>64,"Height"=>48,"BitsPerComponent"=>8,"ColorSpace"=>"DeviceRGB","Filter"=>"DCTDecode"},include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub2-progressive1.jpg").to_vec())));
    context.begin_document().load(&pdf, &value).unwrap()
}
fn page(image: Arc<Image>, data: &[u8]) -> Page {
    Page {
        width: 400.into(),
        height: 200.into(),
        fonts: vec![],
        graphics: vec![],
        images: vec![image],
        program: Program {
            operations: Content::decode_strict(data).unwrap().operations,
            blocks: vec![],
        },
    }
}
const OPS: &[u8] = b"q 128 0 0 96 20 24 cm /I0 Do Q q 128 0 0 96 200 24 cm /I0 Do Q";
#[test]
fn redaction_jpeg_occurrence_reedit_includes_prior_coalesced_paint_edge() {
    let mut context = ImageContext::for_jpeg_validation();
    let image = source(&mut context);
    let mut p = page(image, b"q 0 128 -96 0 116 16 cm /I0 Do Q");
    p.width = Object::Real(192.125);
    p.height = Object::Real(160.25);
    for bottom in [64., 96.] {
        let selected = ImageWork::default()
            .page(
                &mut p,
                0,
                &[AreaRect {
                    left: 68.,
                    bottom,
                    right: 68.001,
                    top: bottom + 0.001,
                }],
                &mut context,
            )
            .unwrap();
        let jpeg = selected[0].jpeg.as_ref().unwrap();
        assert_eq!(jpeg.coverage.mask().iter().filter(|&&v| v == 1).count(), 1);
        if bottom == 96. {
            let regions = jpeg
                .page_influence(
                    [64, 53],
                    &mut crate::redaction_jpeg_coverage::CoverageBudget::default(),
                )
                .unwrap();
            // Installed PDFium changes the previous rectangle's far edge when
            // the combined width changes its snapped rectangle-fill branch.
            for x in 17..=27 {
                assert!(
                    regions[0]
                        .iter()
                        .any(|r| r[0] <= x && x < r[2] && r[1] <= 37 && 37 < r[3]),
                    "prior edge ({x},37) omitted"
                );
            }
            assert_eq!(
                jpeg.coverage.mask()[5],
                0,
                "repaint must not authorize old source removal"
            );
            assert_eq!(jpeg.coverage.mask()[6], 1);
        }
    }
}
fn mark(left: f64) -> AreaRect {
    AreaRect {
        left,
        bottom: 68.25,
        right: left + 1.,
        top: 69.25,
    }
}
#[test]
fn redaction_jpeg_occurrence_pdfium_influence_uses_its_source_matrix() {
    use crate::{
        redaction_jpeg_coverage::CoverageBudget, redaction_render_transform::RendererTransform,
    };
    let mut context = ImageContext::for_jpeg_validation();
    let image = source(&mut context);
    let mut p = page(image, OPS);
    let selections = ImageWork::default()
        .page(&mut p, 0, &[mark(55.25), mark(235.25)], &mut context)
        .unwrap();
    let to_device = RendererTransform::known([0.5, 0., 0., -0.5, 0., 100.]).unwrap();
    for (index, selection) in selections.iter().enumerate() {
        let selected = selection.jpeg.as_ref().unwrap();
        let bound = selected
            .pdfium_influence(
                to_device,
                [200, 100],
                [0, 0, 200, 100],
                &mut CoverageBudget::default(),
            )
            .unwrap();
        let expected =
            RendererTransform::known([64., 0., 0., -48., if index == 0 { 10. } else { 100. }, 88.])
                .unwrap();
        assert_eq!(
            bound,
            selected
                .coverage
                .pdfium_influence(
                    expected,
                    [200, 100],
                    [0, 0, 200, 100],
                    &mut CoverageBudget::default()
                )
                .unwrap()
        );
        assert!(!bound.is_empty());
    }
}

#[test]
fn redaction_jpeg_occurrence_page_influence_owns_dimensions_and_preserves_mask() {
    use crate::redaction_jpeg_coverage::CoverageBudget;
    let mut context = ImageContext::for_jpeg_validation();
    let original = source(&mut context);
    let mut p = page(original, OPS);
    let changes = ImageWork::default()
        .page(&mut p, 0, &[mark(55.25)], &mut context)
        .unwrap();
    let selected = changes[0].jpeg.as_ref().unwrap();
    let before = selected.coverage.mask().to_vec();
    for device in [[64, 32], [200, 100], [937, 469]] {
        let regions = selected
            .page_influence(device, &mut CoverageBudget::default())
            .unwrap();
        assert!(regions.iter().all(|v| !v.is_empty()));
        assert!(regions.iter().flatten().all(|r| r[0] >= 0
            && r[1] >= 0
            && r[2] <= device[0] as i32
            && r[3] <= device[1] as i32));
    }
    assert_eq!(selected.coverage.mask(), before);
    assert!(selected
        .page_influence([0, 32], &mut CoverageBudget::default())
        .is_err());
}

#[test]
fn redaction_jpeg_occurrence_keeps_native_rounding_and_restores_matrix_stack() {
    use crate::redaction_render_transform::RendererTransform;
    let mut context = ImageContext::for_jpeg_validation();
    let image = source(&mut context);
    let ops=b"q 1.1 .2 .3 .9 100 50 cm q .7 .1 .2 1.3 4 5 cm q 64 0 0 48 0 0 cm /I0 Do Q Q Q q 64 0 0 48 200 24 cm /I0 Do Q";
    let mut p = page(image, ops);
    let all = AreaRect {
        left: 0.,
        bottom: 0.,
        right: 400.,
        top: 200.,
    };
    let changes = ImageWork::default()
        .page(&mut p, 0, &[all], &mut context)
        .unwrap();
    assert_eq!(changes.len(), 2);
    let expected = RendererTransform::known([1.1, 0.2, 0.3, 0.9, 100., 50.])
        .unwrap()
        .concat(RendererTransform::known([0.7, 0.1, 0.2, 1.3, 4., 5.]).unwrap())
        .unwrap()
        .concat(RendererTransform::known([64., 0., 0., 48., 0., 0.]).unwrap())
        .unwrap();
    let coefficients = changes[0]
        .jpeg
        .as_ref()
        .unwrap()
        .renderer_matrix
        .coefficients();
    assert_eq!(coefficients, expected.coefficients());
    assert!(
        coefficients.iter().any(|v| v[0] < v[1]),
        "Must exercise intermediate rounding uncertainty"
    );
    assert_eq!(
        changes[1]
            .jpeg
            .as_ref()
            .unwrap()
            .renderer_matrix
            .coefficients(),
        RendererTransform::known([64., 0., 0., 48., 200., 24.])
            .unwrap()
            .coefficients()
    );
    let initial = RendererTransform::known([0.7, 0.1, -0.2, -0.8, 4., 210.]).unwrap();
    let flip = RendererTransform::known([1., 0., 0., -1., 0., 1.]).unwrap();
    let steps = [
        vec![
            [1.1, 0.2, 0.3, 0.9, 100., 50.],
            [0.7, 0.1, 0.2, 1.3, 4., 5.],
            [64., 0., 0., 48., 0., 0.],
        ],
        vec![[64., 0., 0., 48., 200., 24.]],
    ];
    for (change, steps) in changes.iter().zip(steps) {
        let selected = change.jpeg.as_ref().unwrap();
        let mut expected = initial;
        for step in steps {
            expected = expected
                .concat(RendererTransform::known(step).unwrap())
                .unwrap();
        }
        expected = expected.concat(flip).unwrap();
        let actual = selected
            .mupdf_influence(
                initial,
                [400, 240],
                [0, 0, 400, 240],
                crate::redaction_render_transform::GridFit::Outward,
                &mut crate::redaction_jpeg_coverage::CoverageBudget::default(),
            )
            .unwrap();
        assert_eq!(
            actual,
            selected
                .coverage
                .mupdf_influence(
                    expected,
                    [400, 240],
                    [0, 0, 400, 240],
                    crate::redaction_render_transform::GridFit::Outward,
                    &mut crate::redaction_jpeg_coverage::CoverageBudget::default()
                )
                .unwrap()
        );
        assert!(!actual.is_empty());
    }
}
#[test]
fn redaction_jpeg_occurrence_expands_one_mcu_preserves_alias_and_accumulates_prior_coverage() {
    let mut context = ImageContext::for_jpeg_validation();
    let original = source(&mut context);
    let before = original.stream().unwrap().content;
    let mut page = page(original.clone(), OPS);
    let changes = ImageWork::default()
        .page(&mut page, 0, &[mark(55.25)], &mut context)
        .unwrap();
    assert_eq!(changes.len(), 1);
    let changed = &changes[0];
    assert_eq!(changed.occurrence_index, 0);
    assert_eq!(changed.pixels, 256);
    assert!(changed.extends_beyond_mark);
    assert!(
        changed.bounds.left <= 52.
            && changed.bounds.right >= 84.
            && changed.bounds.bottom <= 56.
            && changed.bounds.top >= 88.
    );
    let selected = changed.jpeg.as_ref().unwrap();
    assert_eq!(
        selected.coverage.mask(),
        &[0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0]
    );
    assert!(Arc::ptr_eq(&page.images[1], &original));
    assert_eq!(original.stream().unwrap().content, before);
    let first = page.images[0].clone();
    let second = ImageWork::default()
        .page(&mut page, 0, &[mark(87.25)], &mut context)
        .unwrap();
    assert_eq!(second.len(), 1);
    assert_eq!(
        page.images[0].jpeg_coverage().unwrap().mask(),
        &[0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0]
    );
    assert!(Arc::ptr_eq(&page.images[1], &original));
    assert_eq!(
        first.jpeg_coverage().unwrap().mask(),
        selected.coverage.mask()
    );
}
#[test]
fn redaction_jpeg_occurrence_equal_requests_share_variant_and_unmarked_pages_keep_original() {
    let mut context = ImageContext::for_jpeg_validation();
    let original = source(&mut context);
    let mut a = page(original.clone(), OPS);
    let mut work = ImageWork::default();
    let changed = work
        .page(&mut a, 0, &[mark(55.25), mark(235.25)], &mut context)
        .unwrap();
    assert_eq!(changed.len(), 2);
    assert_eq!(a.images.len(), 1);
    let mut b = page(original.clone(), OPS);
    assert!(work.page(&mut b, 1, &[], &mut context).unwrap().is_empty());
    assert!(Arc::ptr_eq(&b.images[0], &original));
    let mut c = page(original, OPS);
    work.page(&mut c, 2, &[mark(55.25), mark(235.25)], &mut context)
        .unwrap();
    assert!(Arc::ptr_eq(&a.images[0], &c.images[0]));
}
