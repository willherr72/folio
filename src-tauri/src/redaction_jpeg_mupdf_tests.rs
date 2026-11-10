use super::*;
#[path = "redaction_jpeg_mupdf_corpus_tests.rs"]
mod corpus;
use crate::redaction_render_transform::{GridFit, RendererTransform};
fn selected() -> Coverage {
    let input = include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub2-progressive1.jpg");
    let h = crate::redaction_jpeg::inspect(
        input,
        (64, 48, 3),
        crate::redaction_jpeg::Transform::Default,
    )
    .unwrap();
    let mut mask = vec![0; (h.columns * h.rows) as usize];
    mask[5] = 1;
    Coverage::from_mask(&h, &mask, &mut CoverageBudget::default()).unwrap()
}
#[test]
fn redaction_jpeg_mupdf_composed_influence_keeps_source_and_all_tiles() {
    let c = selected();
    let before = c.mask().to_vec();
    let binding = c.binding(&[9; 32]);
    let matrix = RendererTransform::known([10., 0., 0., 8., 1., 1.]).unwrap();
    let mut budget = CoverageBudget::default();
    let clip = [0, 0, 12, 10];
    let result = c
        .mupdf_influence(matrix, [12, 10], clip, GridFit::Outward, &mut budget)
        .unwrap();
    assert!(!result.is_empty());
    assert!(budget.render_work > 0);
    for tile in matrix
        .mupdf_tiles([64, 48], clip, GridFit::Outward)
        .unwrap()
    {
        let samples = c
            .decoded_window_samples(
                tile.denominator,
                [1, 1],
                tile.window,
                &mut CoverageBudget::default(),
            )
            .unwrap()
            .box_reduce(tile.extra)
            .unwrap();
        for r in tile
            .paint_bounds(&samples, clip, GridFit::Outward, &mut 65536)
            .unwrap()
        {
            for y in r[1]..r[3] {
                for x in r[0]..r[2] {
                    assert!(result
                        .iter()
                        .any(|r| r[0] <= x && x < r[2] && r[1] <= y && y < r[3]));
                }
            }
        }
    }
    assert_eq!(c.mask(), before);
    assert_eq!(c.binding(&[9; 32]), binding);
}
#[test]
fn redaction_jpeg_mupdf_failure_is_charged_and_invalid_devices_refuse() {
    let c = selected();
    let m = RendererTransform::known([10., 0., 0., 8., 1., 1.]).unwrap();
    let mut b = CoverageBudget::default();
    b.render_work = 1_048_575;
    assert!(c
        .mupdf_influence(m, [12, 10], [0, 0, 12, 10], GridFit::Outward, &mut b)
        .is_err());
    assert_eq!(b.render_work, 1_048_576);
    for (size, clip) in [
        ([0, 10], [0, 0, 12, 10]),
        ([12, 10], [-1, 0, 12, 10]),
        ([12, 10], [0, 0, 13, 10]),
    ] {
        assert!(c
            .mupdf_influence(
                m,
                size,
                clip,
                GridFit::Disabled,
                &mut CoverageBudget::default()
            )
            .is_err());
    }
}
