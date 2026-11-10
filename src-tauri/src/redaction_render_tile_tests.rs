use super::*;

#[test]
fn redaction_render_tile_full_frame_keeps_exact_interval_identity() {
    let m = RendererTransform::known([65., 0.1, -0.1, 49., 0.25, -0.75]).unwrap();
    assert_eq!(
        m.mupdf_tile_frame([65, 49], [0, 0, 65, 49])
            .unwrap()
            .coefficients(),
        m.coefficients()
    );
}

#[test]
fn redaction_render_tile_partial_frame_precomposes_in_native_row_coordinates() {
    let m = RendererTransform::known([100., 25., 50., 75., 30., 20.]).unwrap();
    assert_eq!(
        m.mupdf_tile_frame([64, 48], [16, 12, 48, 36])
            .unwrap()
            .coefficients(),
        [50., 12.5, 25., 37.5, 67.5, 45.].map(|v| [v, v])
    );
    let m = RendererTransform::known([65., 0., 0., 49., 0.25, -0.75]).unwrap();
    let result = m.mupdf_tile_frame([65, 49], [21, 16, 65, 49]).unwrap();
    for (bound, ideal) in result
        .coefficients()
        .into_iter()
        .zip([44., 0., 0., 33., 21.25, 15.25])
    {
        assert!(
            bound[0] <= ideal && ideal <= bound[1],
            "{bound:?} misses {ideal}"
        );
    }
    assert_ne!(
        result.coefficients(),
        m.coefficients(),
        "A full cached image cannot use requested crop placement"
    );
}

#[test]
fn redaction_render_tile_invalid_dimensions_and_windows_refuse() {
    let m = RendererTransform::default();
    for source in [[0, 49], [65, 0], [32769, 49]] {
        assert!(m.mupdf_tile_frame(source, [0, 0, 1, 1]).is_err());
    }
    for window in [
        [0, 0, 66, 49],
        [0, 0, 65, 50],
        [21, 16, 21, 49],
        [21, 16, 65, 16],
        [22, 16, 21, 49],
    ] {
        assert!(m.mupdf_tile_frame([65, 49], window).is_err());
    }
}

pub(crate) fn tile_case_header() -> String {
    use std::fmt::Write;
    let mut text=String::from("name\tdenominator\tcrop\tvariant\tgrid\tsw\tsh\twindow\ttw\tth\tcells\tlayout\ta\tb\tc\td\te\tf");
    for prefix in ["tile", "fitted"] {
        for k in ['a', 'b', 'c', 'd', 'e', 'f'] {
            write!(text, "\t{prefix}_{k}0\t{prefix}_{k}1").unwrap();
        }
    }
    text.push_str("\tbounds\n");
    text
}

pub(crate) fn record_tile_case(
    text: &mut String,
    name: &str,
    source: [u32; 2],
    denominator: u32,
    crop: &str,
    window: [u32; 4],
    samples: &crate::redaction_sampling::SampleRegions,
) {
    use std::fmt::Write;
    let size = if denominator == 1 {
        source
    } else {
        source.map(|v| (v / denominator).saturating_sub(2))
    };
    if size.contains(&0) {
        return;
    }
    let [sx, sy] = size.map(|v| v as f32);
    let (tw, th) = samples.dimensions();
    let cells = samples
        .cells()
        .iter()
        .map(|[l, t, r, b]| format!("{l},{t},{r},{b}"))
        .collect::<Vec<_>>()
        .join(";");
    let frame = format!("{},{},{},{}", window[0], window[1], window[2], window[3]);
    for (variant, layout, values) in [
        ("axis", "axis", [sx, 0., 0., sy, 0.25, -0.75]),
        ("reflection", "axis", [-sx, 0., 0., -sy, 0.25, -0.75]),
        ("quarter", "quarter", [0., sx, -sy, 0., 0.25, -0.75]),
        (
            "affine",
            "affine",
            [sx, sx / 16., sy / 16., sy, 0.25, -0.75],
        ),
    ] {
        let tile = RendererTransform::known(values)
            .unwrap()
            .mupdf_tile_frame(source, window)
            .unwrap();
        for (label, mode) in [
            ("disabled", GridFit::Disabled),
            ("outward", GridFit::Outward),
            ("tiled", GridFit::Tiled),
        ] {
            let fitted = tile.mupdf_sizing(source, mode).unwrap().fitted;
            let k = fitted.coefficients();
            let bounds = match layout {
                "axis" => mupdf_scale_bounds(samples, [k[4], k[5]], [k[0], k[3]], None).unwrap(),
                "quarter" => mupdf_scale_bounds(samples, [k[5], k[4]], [k[1], k[2]], None).unwrap(),
                _ => Vec::new(),
            };
            write!(text,"{name}\t{denominator}\t{crop}\t{variant}\t{label}\t{}\t{}\t{frame}\t{tw}\t{th}\t{cells}\t{layout}",source[0],source[1]).unwrap();
            for value in values {
                write!(text, "\t{}", f64::from(value)).unwrap();
            }
            for matrix in [tile, fitted] {
                for bound in matrix.coefficients() {
                    for value in bound {
                        write!(text, "\t{value}").unwrap();
                    }
                }
            }
            let bounds = bounds
                .into_iter()
                .map(|[l, t, r, b]| format!("{l},{t},{r},{b}"))
                .collect::<Vec<_>>()
                .join(";");
            writeln!(text, "\t{bounds}").unwrap();
        }
    }
}
