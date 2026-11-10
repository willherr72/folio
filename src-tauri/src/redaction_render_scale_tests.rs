use super::*;

#[test]
fn redaction_render_scale_identity_fraction_and_flip_keep_candidate_pixels() {
    let samples = SampleRegions::new(8, 8, vec![[3, 3, 4, 4]]).unwrap();
    let known = |v: [f64; 2]| v.map(|n| [n, n]);
    assert_eq!(
        mupdf_scale_bounds(&samples, known([0., 0.]), known([8., 8.]), None).unwrap(),
        vec![[2, 2, 5, 5]]
    );
    assert_eq!(
        mupdf_scale_bounds(&samples, known([0.25, 0.25]), known([8., 8.]), None).unwrap(),
        vec![[3, 3, 5, 5]]
    );
    assert_eq!(
        mupdf_scale_bounds(&samples, known([8., 8.]), known([-8., -8.]), None).unwrap(),
        vec![[3, 3, 6, 6]]
    );
}

#[test]
fn redaction_render_scale_clip_empty_and_single_pixel_paths_are_bounded() {
    let samples = SampleRegions::new(8, 8, vec![[3, 3, 4, 4]]).unwrap();
    let origin = [[0.; 2]; 2];
    let extent = [[8.; 2]; 2];
    assert_eq!(
        mupdf_scale_bounds(&samples, origin, extent, Some([3, 3, 4, 4])).unwrap(),
        vec![[3, 3, 4, 4]]
    );
    assert!(
        mupdf_scale_bounds(&samples, origin, extent, Some([8, 8, 9, 9]))
            .unwrap()
            .is_empty()
    );
    let single = SampleRegions::new(1, 1, vec![[0, 0, 1, 1]]).unwrap();
    assert_eq!(
        mupdf_scale_bounds(
            &single,
            [[-0.25; 2], [0.25; 2]],
            [[0.5; 2], [-0.5; 2]],
            None
        )
        .unwrap(),
        vec![[-1, -1, 1, 1]]
    );
}

#[test]
fn redaction_render_scale_interval_uncertainty_and_large_grid_remain_bounded() {
    let samples = SampleRegions::new(8, 8, vec![[3, 3, 4, 4]]).unwrap();
    let bound =
        mupdf_scale_bounds(&samples, [[-0.25, 0.75]; 2], [[7.75, 8.25]; 2], None).unwrap()[0];
    for o in [-0.25, 0., 0.25, 0.75] {
        for w in [7.75, 8., 8.25] {
            let p = mupdf_scale_bounds(&samples, [[o; 2]; 2], [[w; 2]; 2], None).unwrap()[0];
            assert!(bound[0] <= p[0] && bound[1] <= p[1] && bound[2] >= p[2] && bound[3] >= p[3]);
        }
    }
    let result = mupdf_scale_bounds(
        &samples,
        [[0.; 2]; 2],
        [[16777216.; 2]; 2],
        Some([0, 0, 100, 100]),
    )
    .unwrap();
    assert!(result.is_empty());
}

#[test]
fn redaction_render_scale_invalid_intervals_refuse_even_empty_support() {
    let empty = SampleRegions::new(8, 8, vec![]).unwrap();
    for v in [[f64::NAN, 1.], [2., 1.], [-16777218., 0.], [0.1, 0.1]] {
        assert!(mupdf_scale_bounds(&empty, [v, [0.; 2]], [[8.; 2]; 2], None).is_err());
        assert!(mupdf_scale_bounds(&empty, [[0.; 2]; 2], [v, [8.; 2]], None).is_err());
    }
    assert!(mupdf_scale_bounds(&empty, [[0.; 2]; 2], [[8.; 2]; 2], Some([1, 0, 0, 1])).is_err());
}

#[test]
fn redaction_render_scale_owned_fractional_cases_export() {
    use std::fmt::Write;
    let mut output =
        String::from("case\tsw\tsh\tl\tt\tr\tb\tx0\tx1\ty0\ty1\tw0\tw1\th0\th1\tclip\tbounds\n");
    let mut index = 0;
    let mut record = |size: [u32; 2],
                      cell: [u32; 4],
                      origin: [[f64; 2]; 2],
                      extent: [[f64; 2]; 2],
                      clip: Option<[i32; 4]>| {
        let regions = SampleRegions::new(size[0], size[1], vec![cell]).unwrap();
        let bounds = mupdf_scale_bounds(&regions, origin, extent, clip).unwrap();
        write!(
            output,
            "{index}\t{}\t{}\t{}\t{}\t{}\t{}",
            size[0], size[1], cell[0], cell[1], cell[2], cell[3]
        )
        .unwrap();
        for v in origin.into_iter().chain(extent) {
            for x in v {
                write!(output, "\t{x}").unwrap();
            }
        }
        let format = |r: [i32; 4]| format!("{},{},{},{}", r[0], r[1], r[2], r[3]);
        let clip = clip.map(format).unwrap_or_default();
        let bounds = bounds.into_iter().map(format).collect::<Vec<_>>().join(";");
        writeln!(output, "\t{clip}\t{bounds}").unwrap();
        index += 1;
    };
    for [sw, sh] in [[19, 17], [1, 17], [19, 1], [1, 1], [64, 48]] {
        for [x, y] in [[0, 0], [sw / 2, sh / 2], [sw - 1, sh - 1]] {
            for [w, h] in [
                [0.25, 0.75],
                [1.25, 1.75],
                [3.5, 4.25],
                [8.75, 12.5],
                [32.75, 24.25],
            ] {
                for [sx, sy] in [[1., 1.], [-1., 1.], [1., -1.], [-1., -1.]] {
                    for [ox, oy] in [[-1.25, 0.25], [0.25, -0.75], [0.9990234375, -1.0009765625]] {
                        for clip in [None, Some([-2, -2, 4, 4])] {
                            record(
                                [sw, sh],
                                [x, y, x + 1, y + 1],
                                [[ox; 2], [oy; 2]],
                                [[sx * w; 2], [sy * h; 2]],
                                clip,
                            );
                        }
                    }
                }
            }
        }
    }
    for cell in [[8191, 0, 8192, 1], [32767, 0, 32768, 1], [0, 0, 1, 1]] {
        for ox in [0., 0.25, 0.75] {
            for sx in [1., -1.] {
                record(
                    [32768, 1],
                    cell,
                    [[ox; 2], [0.; 2]],
                    [[sx; 2], [1.; 2]],
                    None,
                );
            }
        }
    }
    for extent in [
        [[4.25, 5.75], [2.25, 3.75]],
        [[-5.75, -4.25], [-3.75, -2.25]],
        [[-0.75, 0.75], [-0.75, 0.75]],
    ] {
        for cell in [[0, 0, 1, 1], [9, 8, 10, 9], [18, 16, 19, 17]] {
            for clip in [None, Some([-2, -2, 4, 4])] {
                record([19, 17], cell, [[-0.25, 0.75], [-0.75, 0.25]], extent, clip);
            }
        }
    }
    for clip in [[8, 8, 9, 9], [-2, -2, -1, -1], [0, 0, 0, 8], [0, 0, 8, 0]] {
        record([8, 8], [0, 0, 8, 8], [[0.; 2]; 2], [[8.; 2]; 2], Some(clip));
    }
    if let Some(path) = std::env::var_os("FOLIO_FRACTIONAL_OUTPUT") {
        let path = std::path::PathBuf::from(path);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("bounds.tsv"), output).unwrap();
    }
}
