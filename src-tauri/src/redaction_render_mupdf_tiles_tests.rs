use super::*;
use crate::redaction_sampling::SampleRegions;

#[test]
fn redaction_mupdf_tiles_paint_includes_native_scaler_and_affine_routes() {
    let samples = SampleRegions::new(64, 48, vec![[16, 16, 32, 32]]).unwrap();
    for (matrix, extents) in [
        ([8.5, 0., 0., 6.25, 10.25, 10.75], [[8, 8], [6, 6]]),
        ([0., -8.5, 6.25, 0., 10.75, 20.25], [[8, 8], [6, 6]]),
        ([-8.5, 1., 2., 6.25, 20.25, 10.75], [[8, 8], [6, 6]]),
    ] {
        let tile = MuPdfTile {
            window: [0, 0, 64, 48],
            denominator: 1,
            extra: 1,
            frame: RendererTransform::known(matrix).unwrap(),
            extents,
        };
        let mut remaining = 65536;
        let out = tile
            .paint_bounds(&samples, [0, 0, 32, 32], GridFit::Disabled, &mut remaining)
            .unwrap();
        assert!(!out.is_empty());
        assert!(remaining < 65536);
        for r in tile
            .frame
            .mupdf_affine_bounds(&samples, [0, 0, 32, 32])
            .unwrap()
        {
            for y in r[1]..r[3] {
                for x in r[0]..r[2] {
                    assert!(out
                        .iter()
                        .any(|r| r[0] <= x && x < r[2] && r[1] <= y && y < r[3]));
                }
            }
        }
        let mut exhausted = 0;
        assert!(tile
            .paint_bounds(&samples, [0, 0, 32, 32], GridFit::Disabled, &mut exhausted)
            .is_err());
    }
}
#[test]
fn redaction_mupdf_tiles_include_cropped_and_full_cache_frames() {
    let m = RendererTransform::known([256., 0., 0., 192., 0., 0.]).unwrap();
    let plans = m
        .mupdf_tiles([256, 192], [100, 70, 120, 90], GridFit::Disabled)
        .unwrap();
    assert!(plans
        .iter()
        .any(|p| p.window == [96, 66, 124, 94] && p.denominator == 1 && p.extra == 1));
    assert!(plans
        .iter()
        .any(|p| p.window == [0, 0, 256, 192] && p.denominator == 1 && p.extra == 1));
    for p in plans {
        assert_eq!(
            p.frame.coefficients(),
            m.mupdf_tile_frame([256, 192], p.window)
                .unwrap()
                .coefficients()
        );
    }
}
#[test]
fn redaction_mupdf_tiles_include_all_reduction_and_cache_choices() {
    let m = RendererTransform::known([14., 0., 0., 10., 0., 0.]).unwrap();
    let plans = m
        .mupdf_tiles([256, 192], [0, 0, 14, 10], GridFit::Disabled)
        .unwrap();
    for total in [1, 2, 4, 8, 16] {
        assert!(plans
            .iter()
            .any(|p| p.window == [0, 0, 256, 192] && p.denominator * p.extra == total));
    }
}
#[test]
fn redaction_mupdf_tiles_invalid_geometry_refuses() {
    let m = RendererTransform::default();
    assert!(m
        .mupdf_tiles([0, 192], [0, 0, 1, 1], GridFit::Disabled)
        .is_err());
    assert!(m
        .mupdf_tiles([256, 192], [2, 0, 1, 1], GridFit::Disabled)
        .is_err());
    let singular = RendererTransform::known([1., 1., 1., 1., 0., 0.]).unwrap();
    assert!(singular
        .mupdf_tiles([256, 192], [0, 0, 2, 2], GridFit::Disabled)
        .is_err());
}

#[test]
fn redaction_mupdf_tiles_empty_clip_and_unseen_source_remain_empty() {
    let m = RendererTransform::known([256., 0., 0., 192., 0., 0.]).unwrap();
    assert!(m
        .mupdf_tiles([256, 192], [0, 0, 0, 20], GridFit::Outward)
        .unwrap()
        .is_empty());
    assert!(m
        .mupdf_tiles([256, 192], [300, 300, 310, 310], GridFit::Outward)
        .unwrap()
        .is_empty());
    let s = SampleRegions::new(256, 192, vec![[16, 16, 32, 32]]).unwrap();
    for (denominator, extra, window, extents) in [
        (0, 1, [0, 0, 256, 192], [[256, 256], [192, 192]]),
        (1, 0, [0, 0, 256, 192], [[256, 256], [192, 192]]),
        (1, 1, [257, 0, 256, 192], [[256, 256], [192, 192]]),
        (1, 1, [0, 0, 256, 192], [[257, 256], [192, 192]]),
    ] {
        let tile = MuPdfTile {
            denominator,
            extra,
            window,
            extents,
            frame: m,
        };
        assert!(tile
            .paint_bounds(&s, [0, 0, 256, 192], GridFit::Disabled, &mut 65536)
            .is_err());
    }
}
