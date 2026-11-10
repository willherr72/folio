use super::*;

type Fixture = (&'static str, &'static [u8], (u32, u32, u32));

#[test]
fn redaction_jpeg_source_envelope_corpus_uses_actual_codec_and_immutable_coverage() {
    use crate::redaction_jpeg_coverage::{Coverage, CoverageBudget};
    let destination = std::env::var_os("FOLIO_JPEG_DECODER_OUTPUT")
        .map(|p| std::path::PathBuf::from(p).join("source-envelopes"));
    if let Some(root) = &destination {
        std::fs::create_dir_all(root).unwrap();
    }
    let mut all = fixtures();
    all.push((
        "rgb-large-owned",
        include_bytes!("../tests/fixtures/redaction-jpeg/rgb-large-owned.jpg"),
        (256, 192, 3),
    ));
    let mut table =
        String::from("name\tmaximum\twidth\theight\tmcu_width\tmcu_height\tfirst\tlast\tcells\n");
    let mut sizing = String::from("name\treader\tinput_width\tinput_height\tmaximum\n");
    let mut stretch = String::from("name\tmaximum\twidth\theight\tpitch_x\tpitch_y\tregions\n");
    for (name, input, shape) in all {
        let h = inspect(input, shape, Transform::Default).unwrap();
        let first = if h.columns > 2 && h.rows > 2 {
            (h.columns + 1) as usize
        } else {
            0
        };
        let last = (h.columns * h.rows - 1) as usize;
        let mut mask = vec![0; last + 1];
        mask[first] = 1;
        mask[last] = 1;
        let output = rewrite(input, shape, Transform::Default, &mask, Limits::default()).unwrap();
        let coverage = Coverage::from_mask(&h, &mask, &mut CoverageBudget::default()).unwrap();
        let binding = coverage.binding(&[9; 32]);
        for divisor in [1, 2, 4, 8] {
            let device = [(h.width / divisor).max(1), (h.height / divisor).max(1)];
            let maximum = crate::redaction_render_sizing::pdfium_maximum(
                [h.width, h.height],
                [h.mcu_width, h.mcu_height],
                device,
            )
            .unwrap();
            sizing.push_str(&format!(
                "{name}\tpdfium\t{}\t{}\t{maximum}\n",
                device[0], device[1]
            ));
        }
        for divisor in [1, 2, 4, 8, 16, 32, 64] {
            let lower = if divisor == 1 {
                [h.width, h.height]
            } else {
                [
                    (h.width / divisor).saturating_sub(2),
                    (h.height / divisor).saturating_sub(2),
                ]
            };
            let maximum =
                crate::redaction_render_sizing::mupdf_maximum([h.width, h.height], lower).unwrap();
            sizing.push_str(&format!(
                "{name}\tmupdf\t{}\t{}\t{maximum}\n",
                lower[0], lower[1]
            ));
        }
        for maximum in [1, 2, 4, 8, 16, 32, 64] {
            let envelope = coverage
                .source_envelope(maximum, &mut CoverageBudget::default())
                .unwrap();
            assert_eq!(envelope.dimensions(), (h.width, h.height));
            let cells = envelope
                .cells()
                .iter()
                .map(|[l, t, r, b]| format!("{l},{t},{r},{b}"))
                .collect::<Vec<_>>()
                .join(";");
            table.push_str(&format!(
                "{name}\t{maximum}\t{}\t{}\t{}\t{}\t{first}\t{last}\t{cells}\n",
                h.width, h.height, h.mcu_width, h.mcu_height
            ));
            for (width, height) in [(3, 2), (19, 17), (129, 97)] {
                let pitch = [
                    (f64::from(h.width) / f64::from(width))
                        .next_up()
                        .min(f64::from(h.width)),
                    (f64::from(h.height) / f64::from(height))
                        .next_up()
                        .min(f64::from(h.height)),
                ];
                let regions = coverage
                    .source_envelope(maximum, &mut CoverageBudget::default())
                    .unwrap()
                    .into_stretch_envelope(maximum, pitch)
                    .unwrap()
                    .into_iter()
                    .map(|r| format!("{},{},{},{}", r.left, r.bottom, r.right, r.top))
                    .collect::<Vec<_>>()
                    .join(";");
                stretch.push_str(&format!(
                    "{name}\t{maximum}\t{width}\t{height}\t{}\t{}\t{regions}\n",
                    pitch[0], pitch[1]
                ));
            }
        }
        assert_eq!(coverage.mask(), mask);
        assert_eq!(coverage.binding(&[9; 32]), binding);
        if let Some(root) = &destination {
            std::fs::write(root.join(format!("{name}-source.jpg")), input).unwrap();
            std::fs::write(root.join(format!("{name}-actual.jpg")), &output.bytes).unwrap();
        }
    }
    if let Some(root) = &destination {
        std::fs::write(root.join("envelopes.tsv"), table).unwrap();
        std::fs::write(root.join("sizing.tsv"), sizing).unwrap();
        std::fs::write(root.join("stretch.tsv"), stretch).unwrap();
    }
}

fn fixtures() -> Vec<Fixture> {
    vec![
        (
            "partial-65x49-gray-sub0-progressive0",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-65x49-gray-sub0-progressive0.jpg"
            ),
            (65, 49, 1),
        ),
        (
            "partial-65x49-gray-sub0-progressive1",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-65x49-gray-sub0-progressive1.jpg"
            ),
            (65, 49, 1),
        ),
        (
            "partial-65x49-rgb-sub0-progressive0",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-65x49-rgb-sub0-progressive0.jpg"
            ),
            (65, 49, 3),
        ),
        (
            "partial-65x49-rgb-sub0-progressive1",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-65x49-rgb-sub0-progressive1.jpg"
            ),
            (65, 49, 3),
        ),
        (
            "partial-65x49-rgb-sub1-progressive0",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-65x49-rgb-sub1-progressive0.jpg"
            ),
            (65, 49, 3),
        ),
        (
            "partial-65x49-rgb-sub1-progressive1",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-65x49-rgb-sub1-progressive1.jpg"
            ),
            (65, 49, 3),
        ),
        (
            "partial-65x49-rgb-sub2-progressive0",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-65x49-rgb-sub2-progressive0.jpg"
            ),
            (65, 49, 3),
        ),
        (
            "partial-65x49-rgb-sub2-progressive1",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-65x49-rgb-sub2-progressive1.jpg"
            ),
            (65, 49, 3),
        ),
        (
            "partial-65x49-rgb-subv-progressive0",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-65x49-rgb-subv-progressive0.jpg"
            ),
            (65, 49, 3),
        ),
        (
            "partial-65x49-rgb-subv-progressive1",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-65x49-rgb-subv-progressive1.jpg"
            ),
            (65, 49, 3),
        ),
        (
            "partial-19x17-gray-sub0-progressive0",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-19x17-gray-sub0-progressive0.jpg"
            ),
            (19, 17, 1),
        ),
        (
            "partial-19x17-gray-sub0-progressive1",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-19x17-gray-sub0-progressive1.jpg"
            ),
            (19, 17, 1),
        ),
        (
            "partial-19x17-rgb-sub0-progressive0",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-19x17-rgb-sub0-progressive0.jpg"
            ),
            (19, 17, 3),
        ),
        (
            "partial-19x17-rgb-sub0-progressive1",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-19x17-rgb-sub0-progressive1.jpg"
            ),
            (19, 17, 3),
        ),
        (
            "partial-19x17-rgb-sub1-progressive0",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-19x17-rgb-sub1-progressive0.jpg"
            ),
            (19, 17, 3),
        ),
        (
            "partial-19x17-rgb-sub1-progressive1",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-19x17-rgb-sub1-progressive1.jpg"
            ),
            (19, 17, 3),
        ),
        (
            "partial-19x17-rgb-sub2-progressive0",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-19x17-rgb-sub2-progressive0.jpg"
            ),
            (19, 17, 3),
        ),
        (
            "partial-19x17-rgb-sub2-progressive1",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-19x17-rgb-sub2-progressive1.jpg"
            ),
            (19, 17, 3),
        ),
        (
            "partial-19x17-rgb-subv-progressive0",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-19x17-rgb-subv-progressive0.jpg"
            ),
            (19, 17, 3),
        ),
        (
            "partial-19x17-rgb-subv-progressive1",
            include_bytes!(
                "../tests/fixtures/redaction-jpeg/partial-19x17-rgb-subv-progressive1.jpg"
            ),
            (19, 17, 3),
        ),
        (
            "gray-sub0-progressive0",
            include_bytes!("../tests/fixtures/redaction-jpeg/gray-sub0-progressive0.jpg"),
            (64, 48, 1),
        ),
        (
            "gray-sub0-progressive1",
            include_bytes!("../tests/fixtures/redaction-jpeg/gray-sub0-progressive1.jpg"),
            (64, 48, 1),
        ),
        (
            "rgb-sub0-progressive0",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub0-progressive0.jpg"),
            (64, 48, 3),
        ),
        (
            "rgb-sub0-progressive1",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub0-progressive1.jpg"),
            (64, 48, 3),
        ),
        (
            "rgb-sub1-progressive0",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub1-progressive0.jpg"),
            (64, 48, 3),
        ),
        (
            "rgb-sub1-progressive1",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub1-progressive1.jpg"),
            (64, 48, 3),
        ),
        (
            "rgb-sub2-progressive0",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub2-progressive0.jpg"),
            (64, 48, 3),
        ),
        (
            "rgb-sub2-progressive1",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub2-progressive1.jpg"),
            (64, 48, 3),
        ),
    ]
}

#[test]
fn redaction_jpeg_decoder_corpus_preserves_partial_edges_and_reedit_determinism() {
    let destination = std::env::var_os("FOLIO_JPEG_DECODER_OUTPUT").map(std::path::PathBuf::from);
    if let Some(root) = &destination {
        std::fs::create_dir_all(root).unwrap();
    }
    let mut manifest = String::from(
        "name\twidth\theight\tcomponents\tmcu_width\tmcu_height\tcolumns\trows\tfirst\tlast\n",
    );
    let mut geometry =
        String::from("name\tlabel\tdenominator\thalo_x\thalo_y\tleft\tbottom\tright\ttop\n");
    let mut sampling=String::from("name\tdenominator\thalo_x\thalo_y\tfactor\tstage\twidth\theight\tleft\ttop\tright\tbottom\n");
    let mut crops=String::from("name\tdenominator\tcrop\tsource_left\tsource_top\tsource_right\tsource_bottom\twidth\theight\tcells\tregions\n");
    let mut tiles = crate::redaction_render_transform::tile_case_header();
    for (name, fixture, shape) in fixtures() {
        let input = fixture.to_vec();
        let h = inspect(&input, shape, Transform::Default).unwrap();
        let empty = vec![0; (h.columns * h.rows) as usize];
        let zero = rewrite(&input, shape, Transform::Default, &empty, Limits::default()).unwrap();
        assert_eq!(
            rewrite(
                &zero.bytes,
                shape,
                Transform::Default,
                &empty,
                Limits::default()
            )
            .unwrap()
            .bytes,
            zero.bytes,
            "zero-mask idempotence: {name}"
        );
        let first_index = if h.columns > 2 && h.rows > 2 {
            (h.columns + 1) as usize
        } else {
            0
        };
        let last_index = empty.len() - 1;
        assert_ne!(first_index, last_index);
        let mut selected = empty;
        selected[first_index] = 1;
        let first = rewrite(
            &input,
            shape,
            Transform::Default,
            &selected,
            Limits::default(),
        )
        .unwrap();
        selected[last_index] = 1;
        let second = rewrite(
            &first.bytes,
            shape,
            Transform::Default,
            &selected,
            Limits::default(),
        )
        .unwrap();
        let direct = rewrite(
            &input,
            shape,
            Transform::Default,
            &selected,
            Limits::default(),
        )
        .unwrap();
        assert_eq!(
            second.bytes, direct.bytes,
            "re-edit matches direct union: {name}"
        );
        assert_eq!(input, fixture, "source immutable: {name}");
        for (label, indices) in [
            ("first", vec![first_index]),
            ("second", vec![first_index, last_index]),
            ("new_last", vec![last_index]),
        ] {
            use crate::redaction_jpeg_coverage::{Coverage, CoverageBudget};
            let mut mask = vec![0; selected.len()];
            for index in indices {
                mask[index] = 1;
            }
            let coverage = Coverage::from_mask(&h, &mask, &mut CoverageBudget::default()).unwrap();
            for denominator in [1, 2, 4, 8] {
                if label == "second" {
                    let align = |n: u32| n / denominator * denominator;
                    let x = align(h.width / 3).max(denominator);
                    let y = align(h.height / 3).max(denominator);
                    for (crop, window) in [
                        ("full", [0, 0, h.width, h.height]),
                        ("tail", [x, y, h.width, h.height]),
                        ("head", [0, 0, align(h.width / 2), align(h.height / 2)]),
                        (
                            "center",
                            [x, y, align(2 * h.width / 3), align(2 * h.height / 3)],
                        ),
                    ] {
                        let [left, top, right, bottom] = window;
                        if left >= right || top >= bottom {
                            continue;
                        }
                        let grid = coverage
                            .decoded_window_samples(
                                denominator,
                                [1, 1],
                                window,
                                &mut CoverageBudget::default(),
                            )
                            .unwrap();
                        let (width, height) = grid.dimensions();
                        crate::redaction_render_transform::record_tile_case(
                            &mut tiles,
                            name,
                            [h.width, h.height],
                            denominator,
                            crop,
                            window,
                            &grid,
                        );
                        let cells = grid
                            .cells()
                            .iter()
                            .map(|[l, t, r, b]| format!("{l},{t},{r},{b}"))
                            .collect::<Vec<_>>()
                            .join(";");
                        let regions = grid
                            .into_source_regions([h.width, h.height], window)
                            .unwrap()
                            .into_iter()
                            .map(|r| format!("{},{},{},{}", r.left, r.bottom, r.right, r.top))
                            .collect::<Vec<_>>()
                            .join(";");
                        crops.push_str(&format!("{name}\t{denominator}\t{crop}\t{left}\t{top}\t{right}\t{bottom}\t{width}\t{height}\t{cells}\t{regions}\n"));
                    }
                }
                for halo in [[0, 0], [1, 0], [0, 1], [1, 1]] {
                    let regions = coverage
                        .decoded_unit_regions(denominator, halo, &mut CoverageBudget::default())
                        .unwrap();
                    assert!(!regions.is_empty());
                    for region in regions {
                        geometry.push_str(&format!(
                            "{name}\t{label}\t{denominator}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                            halo[0], halo[1], region.left, region.bottom, region.right, region.top
                        ));
                    }
                    if label == "second" && matches!(denominator, 1 | 4) {
                        for factor in [1, 4] {
                            let reduced = coverage
                                .decoded_samples(denominator, halo, &mut CoverageBudget::default())
                                .unwrap()
                                .box_reduce(factor)
                                .unwrap();
                            let (width, height) = reduced.dimensions();
                            for [left, top, right, bottom] in reduced.cells() {
                                sampling.push_str(&format!("{name}\t{denominator}\t{}\t{}\t{factor}\tbox\t{width}\t{height}\t{left}\t{top}\t{right}\t{bottom}\n",halo[0],halo[1]));
                            }
                            for (width, height) in [(3, 2), (19, 17), (129, 97)] {
                                let scaled = coverage
                                    .decoded_samples(
                                        denominator,
                                        halo,
                                        &mut CoverageBudget::default(),
                                    )
                                    .unwrap()
                                    .box_reduce(factor)
                                    .unwrap()
                                    .stretch(width, height)
                                    .unwrap();
                                for [left, top, right, bottom] in scaled.cells() {
                                    sampling.push_str(&format!("{name}\t{denominator}\t{}\t{}\t{factor}\tscale\t{width}\t{height}\t{left}\t{top}\t{right}\t{bottom}\n",halo[0],halo[1]));
                                }
                            }
                        }
                    }
                }
            }
            assert_eq!(coverage.mask(), mask);
        }
        if let Some(root) = &destination {
            for (label, data) in [
                ("source", input.as_slice()),
                ("zero", zero.bytes.as_slice()),
                ("first", first.bytes.as_slice()),
                ("second", second.bytes.as_slice()),
            ] {
                std::fs::write(root.join(format!("{name}-{label}.jpg")), data).unwrap();
            }
        }
        manifest.push_str(&format!(
            "{name}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{first_index}\t{last_index}\n",
            shape.0, shape.1, shape.2, h.mcu_width, h.mcu_height, h.columns, h.rows
        ));
    }
    if let Some(root) = &destination {
        std::fs::write(root.join("manifest.tsv"), manifest).unwrap();
        std::fs::write(root.join("geometry.tsv"), geometry).unwrap();
        std::fs::write(root.join("sampling.tsv"), sampling).unwrap();
        std::fs::write(root.join("crops.tsv"), crops).unwrap();
        std::fs::write(root.join("tile-frames.tsv"), tiles).unwrap();
    }
}
