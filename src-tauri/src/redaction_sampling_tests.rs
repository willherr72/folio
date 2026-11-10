use super::*;

#[test]
fn redaction_sampling_stretch_envelope_includes_filter_and_full_destination_cells() {
    let r = SampleRegions::new(64, 48, vec![[32, 20, 48, 36]])
        .unwrap()
        .into_stretch_envelope(4, [2., 2.])
        .unwrap()[0];
    for (value, exact, lower) in [
        (r.left, 25. / 64., true),
        (r.bottom, 5. / 48., true),
        (r.right, 55. / 64., false),
        (r.top, 35. / 48., false),
    ] {
        assert!(if lower {
            value <= exact
        } else {
            value >= exact
        });
        assert!((value - exact).abs() < 1e-14);
    }
    let r = SampleRegions::new(64, 48, vec![[0, 0, 64, 48]])
        .unwrap()
        .into_stretch_envelope(64, [64., 48.])
        .unwrap()[0];
    assert_eq!((r.left, r.bottom, r.right, r.top), (0., 0., 1., 1.));
}

#[test]
fn redaction_sampling_stretch_envelope_rejects_unproved_parameters_even_empty() {
    for (t, p) in [
        (0, [1., 1.]),
        (3, [1., 1.]),
        (128, [1., 1.]),
        (1, [0., 1.]),
        (1, [f64::NAN, 1.]),
        (1, [f64::INFINITY, 1.]),
        (1, [65., 1.]),
        (1, [1., 49.]),
        (1, [f64::from_bits(1), 1.]),
    ] {
        assert!(SampleRegions::new(64, 48, vec![])
            .unwrap()
            .into_stretch_envelope(t, p)
            .is_err());
    }
}

#[test]
fn redaction_sampling_source_envelope_covers_crop_deficit_and_box_overlap() {
    let grid = SampleRegions::new(33, 25, vec![[7, 3, 17, 9], [32, 24, 33, 25]])
        .unwrap()
        .source_envelope([65, 49], 2, 4)
        .unwrap();
    assert_eq!(grid.dimensions(), (65, 49));
    assert_eq!(grid.cells(), &[[1, 0, 40, 24], [57, 41, 65, 49]]);
    let grid = SampleRegions::new(33, 25, vec![[32, 24, 33, 25]])
        .unwrap()
        .source_envelope([65, 49], 2, 1)
        .unwrap();
    assert_eq!(grid.cells(), &[[63, 47, 65, 49]]);
}

#[test]
fn redaction_sampling_source_envelope_validates_empty_and_mismatched_grids() {
    for (source, d, f) in [
        ([65, 49], 0, 1),
        ([65, 49], 3, 1),
        ([65, 49], 2, 0),
        ([65, 49], 2, 3),
        ([65, 49], 2, 64),
        ([0, 49], 2, 1),
        ([66, 50], 4, 1),
        ([u32::MAX, 49], 2, 1),
    ] {
        assert!(SampleRegions::new(33, 25, vec![])
            .unwrap()
            .source_envelope(source, d, f)
            .is_err());
    }
    let grid = SampleRegions::new(4096, 4096, vec![[4095, 4095, 4096, 4096]])
        .unwrap()
        .source_envelope([32768, 32768], 8, 8)
        .unwrap();
    assert_eq!(grid.dimensions(), (32768, 32768));
    assert_eq!(grid.cells()[0][2..], [32768, 32768]);
}

#[test]
fn redaction_sampling_source_envelope_contains_exact_small_grid_crop_box_chains() {
    let mut checked = 0u64;
    let mut naive_mapping_failed = false;
    for source in 1u32..=65 {
        for d in [1, 2, 4, 8] {
            let decoded = source.div_ceil(d);
            for f in [1, 2, 4, 8, 16, 32, 64].into_iter().filter(|f| d * f <= 64) {
                let total = d * f;
                for selected in 0..decoded {
                    let original = || {
                        SampleRegions::new(decoded, 1, vec![[selected, 0, selected + 1, 1]])
                            .unwrap()
                    };
                    let envelope = original()
                        .source_envelope([source, 1], d, f)
                        .unwrap()
                        .cells()[0];
                    for start in (0..source).step_by(total as usize) {
                        let ends = (start + total..source)
                            .step_by(total as usize)
                            .chain(std::iter::once(source));
                        for end in ends {
                            let actual = original()
                                .crop([start / d, 0, end.div_ceil(d), 1])
                                .unwrap()
                                .box_reduce(f)
                                .unwrap();
                            let (count, _) = actual.dimensions();
                            for [l, _, r, _] in actual.cells() {
                                let left = u64::from(start) * u64::from(count)
                                    + u64::from(*l) * u64::from(end - start);
                                let right = u64::from(start) * u64::from(count)
                                    + u64::from(*r) * u64::from(end - start);
                                assert!(u64::from(envelope[0]) * u64::from(count) <= left);
                                assert!(u64::from(envelope[2]) * u64::from(count) >= right);
                                if f == 1 && left < u64::from(selected * d) * u64::from(count) {
                                    naive_mapping_failed = true;
                                }
                                checked += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(checked > 100_000);
    assert!(naive_mapping_failed);
    if let Some(root) = std::env::var_os("FOLIO_JPEG_DECODER_OUTPUT") {
        let root = std::path::PathBuf::from(root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("source-envelope-exact.txt"),
            format!("{checked}\nnaive-source-grid-rejected\n"),
        )
        .unwrap();
    }
}

#[test]
fn redaction_sampling_crop_intersects_before_translating_and_keeps_empty() {
    let grid = SampleRegions::new(33, 25, vec![[7, 3, 17, 9], [32, 24, 33, 25], [0, 0, 8, 4]])
        .unwrap()
        .crop([8, 4, 33, 25])
        .unwrap();
    assert_eq!(grid.dimensions(), (25, 21));
    assert_eq!(grid.cells(), &[[0, 0, 9, 5], [24, 20, 25, 21]]);
    let empty = SampleRegions::new(33, 25, vec![[0, 0, 8, 4]])
        .unwrap()
        .crop([8, 4, 33, 25])
        .unwrap()
        .box_reduce(4)
        .unwrap();
    assert!(empty.cells().is_empty());
    assert_eq!(empty.dimensions(), (7, 6));
}

#[test]
fn redaction_sampling_crop_source_frame_keeps_partial_grid_rational_coordinates() {
    let regions = SampleRegions::new(25, 21, vec![[24, 20, 25, 21]])
        .unwrap()
        .into_source_regions([65, 49], [16, 8, 65, 49])
        .unwrap();
    let r = regions[0];
    // Left = (16*25+24*49)/(65*25); bottom = 0.
    assert!(r.left <= 1576. / 1625. && r.left >= (1576_f64 / 1625.).next_down());
    assert_eq!((r.bottom, r.right), (0., 1.));
    assert!(r.top >= 41. / 1029. && r.top <= (41_f64 / 1029.).next_up());
    assert!((r.left - 32. / 33.).abs() > 0.0001);
    let r = SampleRegions::new(1 << 24, 1 << 24, vec![[0, 0, 1 << 24, 1 << 24]])
        .unwrap()
        .into_source_regions([1 << 24, 1 << 24], [0, 0, 1 << 24, 1 << 24])
        .unwrap()[0];
    assert_eq!((r.left, r.bottom, r.right, r.top), (0., 0., 1., 1.));
}

#[test]
fn redaction_sampling_invalid_crop_or_frame_refuses_even_without_cells() {
    for window in [[0, 0, 0, 1], [3, 1, 2, 2], [0, 0, 34, 25], [0, 0, 33, 26]] {
        assert!(SampleRegions::new(33, 25, vec![])
            .unwrap()
            .crop(window)
            .is_err());
        assert!(SampleRegions::new(33, 25, vec![])
            .unwrap()
            .into_source_regions([33, 25], window)
            .is_err());
    }
    for size in [[0, 25], [33, 0], [u32::MAX, 25]] {
        assert!(SampleRegions::new(33, 25, vec![])
            .unwrap()
            .into_source_regions(size, [0, 0, 1, 1])
            .is_err());
    }
}

#[test]
fn redaction_sampling_dimension_endpoints_keep_bounded_nonempty_cells() {
    let grid = SampleRegions::new(32768, 1, vec![[32767, 0, 32768, 1]])
        .unwrap()
        .stretch(1 << 24, 1)
        .unwrap();
    let r = grid.cells()[0];
    assert!(r[0] < r[2]);
    assert_eq!((r[1], r[2], r[3]), (0, 1 << 24, 1));
    let grid = SampleRegions::new(1, 1, vec![[0, 0, 1, 1]])
        .unwrap()
        .box_reduce(64)
        .unwrap()
        .stretch(1, 1)
        .unwrap();
    assert_eq!(grid.cells(), &[[0, 0, 1, 1]]);
}

#[test]
fn redaction_sampling_box_includes_distributed_partial_group_overlap() {
    let grid = SampleRegions::new(17, 17, vec![[13, 13, 14, 14]])
        .unwrap()
        .box_reduce(4)
        .unwrap();
    assert_eq!(grid.dimensions(), (5, 5));
    assert_eq!(grid.cells(), &[[3, 3, 5, 5]]);
    if let Some(root) = std::env::var_os("FOLIO_JPEG_DECODER_OUTPUT") {
        let root = std::path::PathBuf::from(root);
        std::fs::create_dir_all(&root).unwrap();
        let (w, h) = grid.dimensions();
        let [l, t, r, b] = grid.cells()[0];
        std::fs::write(
            root.join("box-control.tsv"),
            format!("{w}\t{h}\t{l}\t{t}\t{r}\t{b}\n"),
        )
        .unwrap();
    }
    let grid = SampleRegions::new(16, 16, vec![[13, 13, 14, 14]])
        .unwrap()
        .box_reduce(4)
        .unwrap();
    assert_eq!(grid.dimensions(), (4, 4));
    assert_eq!(grid.cells(), &[[3, 3, 4, 4]]);
}

#[test]
fn redaction_sampling_stretch_includes_filter_window_and_complete_output_cells() {
    let grid = SampleRegions::new(32, 32, vec![[8, 8, 16, 16]])
        .unwrap()
        .stretch(8, 8)
        .unwrap();
    assert_eq!(grid.dimensions(), (8, 8));
    assert_eq!(grid.cells(), &[[1, 1, 5, 5]]);
    let grid = SampleRegions::new(32, 32, vec![[8, 8, 16, 16]])
        .unwrap()
        .stretch(128, 128)
        .unwrap();
    assert_eq!(grid.cells(), &[[26, 26, 70, 70]]);
}

#[test]
fn redaction_sampling_chain_keeps_partial_cell_and_disjoint_regions() {
    let grid = SampleRegions::new(65, 49, vec![[0, 0, 1, 1], [64, 48, 65, 49]])
        .unwrap()
        .box_reduce(8)
        .unwrap()
        .stretch(18, 14)
        .unwrap();
    assert_eq!(grid.dimensions(), (18, 14));
    assert_eq!(grid.cells().len(), 2);
    assert!(grid.cells()[0][2] < grid.cells()[1][0]);
    let regions = grid.into_unit_regions();
    assert_eq!((regions[0].left, regions[0].top), (0., 1.));
    assert_eq!((regions[1].right, regions[1].bottom), (1., 0.));
}

#[test]
fn redaction_sampling_invalid_parameters_refuse_including_empty_sets() {
    for factor in [0, 3, 128, u32::MAX] {
        assert!(SampleRegions::new(17, 17, vec![])
            .unwrap()
            .box_reduce(factor)
            .is_err());
    }
    for dimensions in [(0, 1), (1, 0), ((1 << 24) + 1, 1), (1, u32::MAX)] {
        assert!(SampleRegions::new(17, 17, vec![])
            .unwrap()
            .stretch(dimensions.0, dimensions.1)
            .is_err());
    }
    assert!(SampleRegions::new(32769, 1, vec![])
        .unwrap()
        .stretch(2, 2)
        .is_err());
    assert!(SampleRegions::new(1, 1, vec![[0, 0, 1, 1]; 4097]).is_err());
    assert!(SampleRegions::new(1, 1, vec![[0, 0, 2, 1]]).is_err());
}
