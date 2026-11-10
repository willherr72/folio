use super::*;
use crate::redaction_jpeg::{inspect, rewrite, Limits, Transform};
use std::fmt::Write;
#[test]
fn redaction_jpeg_mupdf_owned_corpus_exports_complete_influence() {
    let fixtures: &[(&str, &[u8], (u32, u32, u32))] = &[
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
            "rgb-large-owned",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-large-owned.jpg"),
            (256, 192, 3),
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
    ];

    export(fixtures);
}

fn export(fixtures: &[(&str, &[u8], (u32, u32, u32))]) {
    let root = std::env::var_os("FOLIO_JPEG_DECODER_OUTPUT")
        .map(|p| std::path::PathBuf::from(p).join("mupdf-influence"));
    if let Some(root) = &root {
        std::fs::create_dir_all(root).unwrap();
    }
    let rects = |v: &[[i32; 4]]| {
        v.iter()
            .map(|r| {
                r.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .collect::<Vec<_>>()
            .join(";")
    };
    let mut cases = String::from(
        "case\tname\tkind\tsw\tsh\tcomponents\tdw\tdh\tgrid\tmatrixBits\tclip\tcells\tbounds\n",
    );
    let mut tiles = String::from("case\twindow\tdenominator\textra\textents\tframe\n");
    let mut count = 0;
    for &(name, input, shape) in fixtures {
        let h = inspect(input, shape, Transform::Default).unwrap();
        let mut mask = vec![0; (h.columns * h.rows) as usize];
        let first = if h.columns > 2 && h.rows > 2 {
            (h.columns + 1) as usize
        } else {
            0
        };
        mask[first] = 1;
        *mask.last_mut().unwrap() = 1;
        let c = Coverage::from_mask(&h, &mask, &mut CoverageBudget::default()).unwrap();
        let binding = c.binding(&[9; 32]);
        let rewritten =
            rewrite(input, shape, Transform::Default, &mask, Limits::default()).unwrap();
        if let Some(root) = &root {
            std::fs::write(root.join(format!("{name}-source.jpg")), input).unwrap();
            std::fs::write(root.join(format!("{name}-actual.jpg")), rewritten.bytes).unwrap();
        }
        let selected = c
            .rectangles(&mut CoverageBudget::default())
            .unwrap()
            .iter()
            .map(|r| r.map(|v| v as i32))
            .collect::<Vec<_>>();
        for [dw, dh] in [[8_u32, 6_u32], [16, 12], [65, 49], [129, 97]] {
            let w = dw as f32;
            let y = dh as f32;
            for (kind, matrix) in [
                ("normal", [w, 0., 0., y, 0., 0.]),
                (
                    "fractional",
                    [w * 0.75, 0., 0., y * 0.75, w * 0.125, y * 0.125],
                ),
                (
                    "quarter",
                    [0., y * 0.75, -w * 0.75, 0., w * 0.875, y * 0.125],
                ),
                (
                    "general",
                    [w * 0.75, y * 0.25, -w * 0.25, y * 0.75, w * 0.25, 0.],
                ),
                (
                    "reflected",
                    [-w * 0.75, 0., 0., -y * 0.75, w * 0.875, y * 0.875],
                ),
                ("small", [w * 0.125, 0., 0., y * 0.125, w * 0.5, y * 0.25]),
            ] {
                let frame = RendererTransform::known([1., 0., 0., -1., 0., y])
                    .unwrap()
                    .concat(RendererTransform::known(matrix).unwrap())
                    .unwrap()
                    .concat(RendererTransform::known([1., 0., 0., -1., 0., 1.]).unwrap())
                    .unwrap();
                for clip in [
                    [0, 0, dw as i32, dh as i32],
                    [
                        (dw / 3) as i32,
                        (dh / 3) as i32,
                        (dw - dw / 4) as i32,
                        (dh - dh / 4) as i32,
                    ],
                ] {
                    for (grid, mode) in [
                        (0, GridFit::Disabled),
                        (1, GridFit::Outward),
                        (2, GridFit::Tiled),
                    ] {
                        let bound = c
                            .mupdf_influence(
                                frame,
                                [dw, dh],
                                clip,
                                mode,
                                &mut CoverageBudget::default(),
                            )
                            .unwrap_or_else(|e| {
                                panic!("{name} {kind} {dw}x{dh} {grid} {clip:?}: {e:?}")
                            });
                        writeln!(&mut cases,"{count}\t{name}\t{kind}\t{}\t{}\t{}\t{dw}\t{dh}\t{grid}\t{}\t{}\t{}\t{}",shape.0,shape.1,shape.2,matrix.map(f32::to_bits).map(|v|format!("{v:08x}")).join(","),clip.iter().map(ToString::to_string).collect::<Vec<_>>().join(","),rects(&selected),rects(&bound)).unwrap();
                        for tile in frame.mupdf_tiles([h.width, h.height], clip, mode).unwrap() {
                            writeln!(
                                &mut tiles,
                                "{count}\t{}\t{}\t{}\t{}\t{}",
                                tile.window
                                    .iter()
                                    .map(ToString::to_string)
                                    .collect::<Vec<_>>()
                                    .join(","),
                                tile.denominator,
                                tile.extra,
                                tile.extents
                                    .iter()
                                    .flatten()
                                    .map(ToString::to_string)
                                    .collect::<Vec<_>>()
                                    .join(","),
                                tile.frame
                                    .coefficients()
                                    .iter()
                                    .flatten()
                                    .map(ToString::to_string)
                                    .collect::<Vec<_>>()
                                    .join(",")
                            )
                            .unwrap();
                        }
                        count += 1;
                    }
                }
            }
        }
        assert_eq!(c.mask(), mask);
        assert_eq!(c.binding(&[9; 32]), binding);
    }
    assert_eq!(count, 4176);
    if let Some(root) = root {
        std::fs::write(root.join("cases.tsv"), cases).unwrap();
        std::fs::write(root.join("tiles.tsv"), tiles).unwrap();
    }
}
