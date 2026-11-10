use super::*;
#[test]
fn redaction_pdfium_stretch_signed_axes_and_clipped_frame_keep_changed_cells() {
    let s = SampleRegions::new(8, 8, vec![[1, 2, 2, 3]]).unwrap();
    let normal = pdfium_stretch_bounds(&s, [8, 8], [0, 0, 8, 8]).unwrap();
    assert_eq!(normal.dimensions(), (8, 8));
    assert!(normal
        .cells()
        .iter()
        .any(|r| r[0] <= 1 && r[2] > 1 && r[1] <= 2 && r[3] > 2));
    let flipped = pdfium_stretch_bounds(&s, [-8, -8], [4, 4, 8, 8]).unwrap();
    assert_eq!(flipped.dimensions(), (4, 4));
    assert!(flipped
        .cells()
        .iter()
        .any(|r| r[0] <= 2 && r[2] > 2 && r[1] <= 1 && r[3] > 1));
    assert!(pdfium_stretch_bounds(&s, [8, 8], [6, 6, 8, 8])
        .unwrap()
        .cells()
        .is_empty());
}
#[test]
fn redaction_pdfium_stretch_upscale_includes_interpolation_neighbors() {
    let s = SampleRegions::new(4, 4, vec![[1, 1, 2, 2]]).unwrap();
    let out = pdfium_stretch_bounds(&s, [8, 8], [0, 0, 8, 8]).unwrap();
    assert!(out
        .cells()
        .iter()
        .any(|r| r[0] <= 1 && r[2] >= 5 && r[1] <= 1 && r[3] >= 5));
}
#[test]
fn redaction_pdfium_stretch_invalid_stage_parameters_refuse_even_empty() {
    let s = SampleRegions::new(8, 8, vec![]).unwrap();
    for (size, clip) in [
        ([0, 8], [0, 0, 1, 1]),
        ([8, 8], [0, 0, 9, 8]),
        ([8, 8], [1, 1, 1, 2]),
        ([i32::MIN, 8], [0, 0, 1, 1]),
    ] {
        assert!(pdfium_stretch_bounds(&s, size, clip).is_err());
    }
}

#[test]
fn redaction_pdfium_stretch_uncertain_tiny_source_window_refuses() {
    let s = SampleRegions::new(1, 1, vec![[0, 0, 1, 1]]).unwrap();
    assert!(
        pdfium_stretch_bounds(&s, [(1 << 24) - 1, 1], [(1 << 24) - 2, 0, (1 << 24) - 1, 1])
            .is_err()
    );
}

#[test]
fn redaction_pdfium_stretch_owned_stage_cases_export() {
    use std::fmt::Write;
    let mut text = String::from("case\tsw\tsh\tdw\tdh\tclip\tcells\tbounds\n");
    let mut count = 0;
    let list = |items: &[[u32; 4]]| {
        items
            .iter()
            .map(|r| format!("{},{},{},{}", r[0], r[1], r[2], r[3]))
            .collect::<Vec<_>>()
            .join(";")
    };
    for (sw, sh) in [(1_u32, 1_u32), (8, 8), (19, 17), (65, 49)] {
        for (width, height) in [
            (1, 1),
            (sw, sh),
            (sw / 2 + 1, sh / 2 + 1),
            (sw * 2 + 1, sh * 2 + 1),
        ] {
            for sx in [-1, 1] {
                for sy in [-1, 1] {
                    let destination = [width as i32 * sx, height as i32 * sy];
                    for clip in [
                        [0, 0, width, height],
                        [width / 2, height / 2, width, height],
                        [width - 1, 0, width, height],
                    ] {
                        for cells in [
                            vec![[0, 0, 1, 1]],
                            vec![[sw / 2, sh / 2, (sw / 2 + 2).min(sw), (sh / 2 + 2).min(sh)]],
                            vec![[sw - 1, sh - 1, sw, sh]],
                            vec![[0, 0, 1, sh], [sw - 1, 0, sw, sh]],
                        ] {
                            let samples = SampleRegions::new(sw, sh, cells).unwrap();
                            let bounds=pdfium_stretch_bounds(&samples,destination,clip).unwrap_or_else(|e|panic!("source {sw}x{sh}, size {destination:?}, clip {clip:?}: {e:?}"));
                            assert_eq!(bounds.dimensions(), (clip[2] - clip[0], clip[3] - clip[1]));
                            writeln!(
                                &mut text,
                                "{count}\t{sw}\t{sh}\t{}\t{}\t{},{},{},{}\t{}\t{}",
                                destination[0],
                                destination[1],
                                clip[0],
                                clip[1],
                                clip[2],
                                clip[3],
                                list(samples.cells()),
                                list(bounds.cells())
                            )
                            .unwrap();
                            count += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(count, 768);
    if let Some(root) = std::env::var_os("FOLIO_RENDER_TRANSFORM_OUTPUT") {
        let root = std::path::PathBuf::from(root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("pdfium-stretch.tsv"), text).unwrap();
    }
}
