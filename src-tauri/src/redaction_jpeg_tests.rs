use super::*;

fn fixtures() -> [(&'static str, &'static [u8]); 8] {
    [
        (
            "gray-sub0-progressive0",
            include_bytes!("../tests/fixtures/redaction-jpeg/gray-sub0-progressive0.jpg"),
        ),
        (
            "gray-sub0-progressive1",
            include_bytes!("../tests/fixtures/redaction-jpeg/gray-sub0-progressive1.jpg"),
        ),
        (
            "rgb-sub0-progressive0",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub0-progressive0.jpg"),
        ),
        (
            "rgb-sub0-progressive1",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub0-progressive1.jpg"),
        ),
        (
            "rgb-sub1-progressive0",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub1-progressive0.jpg"),
        ),
        (
            "rgb-sub1-progressive1",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub1-progressive1.jpg"),
        ),
        (
            "rgb-sub2-progressive0",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub2-progressive0.jpg"),
        ),
        (
            "rgb-sub2-progressive1",
            include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub2-progressive1.jpg"),
        ),
    ]
}
fn expected(name: &str) -> (u32, u32, u32) {
    (64, 48, if name.starts_with("gray") { 1 } else { 3 })
}
fn selected(h: &Header) -> Vec<u8> {
    let mut mask = vec![0; (h.columns * h.rows) as usize];
    for y in 16 / h.mcu_height..32 / h.mcu_height {
        for x in 16 / h.mcu_width..32 / h.mcu_width {
            mask[(y * h.columns + x) as usize] = 1;
        }
    }
    mask
}
#[test]
fn redaction_jpeg_inspects_baseline_progressive_and_sampling() {
    for (name, bytes) in fixtures() {
        let h = inspect(bytes, expected(name), Transform::Default).unwrap();
        assert_eq!((h.width, h.height, h.components), expected(name));
        assert_eq!(h.mcu_width, if name.contains("sub0") { 8 } else { 16 });
        assert_eq!(h.mcu_height, if name.contains("sub2") { 16 } else { 8 });
        assert_eq!(h.scans > 1, name.ends_with("progressive1"));
        assert_eq!(h.transformed, !name.starts_with("gray"));
    }
}
#[test]
fn redaction_jpeg_rewrites_owned_mask_without_changing_input() {
    for (name, input) in fixtures() {
        let source = input.to_vec();
        let h = inspect(&source, expected(name), Transform::Default).unwrap();
        let output = rewrite(
            &source,
            expected(name),
            Transform::Default,
            &selected(&h),
            Limits::default(),
        )
        .unwrap();
        assert_eq!(source, input);
        assert_ne!(output.bytes, input);
        assert_eq!(
            (
                output.header.width,
                output.header.height,
                output.header.components
            ),
            expected(name)
        );
        assert!(output.peak_memory > 0 && output.peak_memory <= Limits::default().memory_bytes);
        let again = rewrite(
            &output.bytes,
            expected(name),
            Transform::Default,
            &selected(&h),
            Limits::default(),
        )
        .unwrap();
        assert_eq!(again.bytes, output.bytes, "canonical re-edit {name}");
        if let Some(root) = std::env::var_os("FOLIO_JPEG_OUTPUT") {
            use std::io::Write;
            for (suffix, bytes) in [("source", input), ("actual", output.bytes.as_slice())] {
                std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(std::path::PathBuf::from(&root).join(format!("{name}-{suffix}.jpg")))
                    .unwrap()
                    .write_all(bytes)
                    .unwrap();
            }
        }
    }
}
#[test]
fn redaction_jpeg_rejects_shape_mask_and_resource_errors_then_recovers() {
    let (name, data) = fixtures()[7];
    let h = inspect(data, expected(name), Transform::Default).unwrap();
    let mask = selected(&h);
    for shape in [(63, 48, 3), (64, 47, 3), (64, 48, 1)] {
        assert!(inspect(data, shape, Transform::Default).is_err());
    }
    for bad in [vec![], vec![0; mask.len() + 1], vec![2; mask.len()]] {
        assert!(rewrite(
            data,
            expected(name),
            Transform::Default,
            &bad,
            Limits::default()
        )
        .is_err());
    }
    for limits in [
        Limits {
            memory_bytes: 1,
            ..Limits::default()
        },
        Limits {
            output_bytes: 1,
            ..Limits::default()
        },
        Limits {
            coefficient_work: 1,
            ..Limits::default()
        },
    ] {
        for _ in 0..12 {
            assert!(rewrite(data, expected(name), Transform::Default, &mask, limits).is_err());
        }
    }
    assert!(rewrite(
        data,
        expected(name),
        Transform::Default,
        &mask,
        Limits::default()
    )
    .is_ok());
}

fn marker(data: &[u8], kind: u8) -> usize {
    data.windows(2).position(|w| w == [0xff, kind]).unwrap()
}
fn segment(kind: u8, payload: &[u8]) -> Vec<u8> {
    let mut v = vec![0xff, kind];
    v.extend_from_slice(&u16::try_from(payload.len() + 2).unwrap().to_be_bytes());
    v.extend_from_slice(payload);
    v
}
fn prepend(data: &[u8], kind: u8, payload: &[u8]) -> Vec<u8> {
    let mut v = data[..2].to_vec();
    v.extend(segment(kind, payload));
    v.extend_from_slice(&data[2..]);
    v
}
#[test]
fn redaction_jpeg_strict_framing_tables_and_profiles() {
    let (name, source) = fixtures()[2];
    let shape = expected(name);
    for end in [0, 1, 2, 20, source.len() - 2, source.len() - 1] {
        assert!(inspect(&source[..end], shape, Transform::Default).is_err());
    }
    for tail in [&[0][..], source] {
        let mut v = source.to_vec();
        v.extend_from_slice(tail);
        assert!(inspect(&v, shape, Transform::Default).is_err());
    }
    let sof = marker(source, 0xc0);
    let dqt = marker(source, 0xdb);
    for (offset, value) in [
        (sof + 1, 0xc9),
        (sof + 4, 12),
        (sof + 9, 4),
        (sof + 11, 0),
        (dqt + 4, 0x10),
        (dqt + 5, 0),
    ] {
        let mut v = source.to_vec();
        v[offset] = value;
        assert!(
            inspect(&v, shape, Transform::Default).is_err(),
            "offset {offset}"
        );
    }
    assert!(inspect(
        &prepend(source, 0xe2, b"ICC_PROFILE\0\x01\x01profile"),
        shape,
        Transform::Default
    )
    .is_err());
    let table_len = u16::from_be_bytes([source[dqt + 2], source[dqt + 3]]) as usize;
    let mut table = source[dqt + 4..dqt + 2 + table_len].to_vec();
    table[1] = table[1].saturating_add(1);
    assert!(inspect(&prepend(source, 0xdb, &table), shape, Transform::Default).is_err());
    let mut many = source[..2].to_vec();
    for _ in 0..4096 {
        many.extend(segment(0xfe, b""));
    }
    many.extend_from_slice(&source[2..]);
    assert!(inspect(&many, shape, Transform::Default).is_err());
}
#[test]
fn redaction_jpeg_strips_metadata_and_zero_mask_preserves_coefficients() {
    for (name, source) in fixtures() {
        let with_metadata = prepend(
            &prepend(source, 0xe1, b"Exif\0\0PRIVATE-LOCATION"),
            0xfe,
            b"PRIVATE-COMMENT",
        );
        let h = inspect(&with_metadata, expected(name), Transform::Default).unwrap();
        let zero = vec![0; (h.columns * h.rows) as usize];
        let output = rewrite(
            &with_metadata,
            expected(name),
            Transform::Default,
            &zero,
            Limits::default(),
        )
        .unwrap();
        assert!(!output.bytes.windows(7).any(|v| v == b"PRIVATE"));
        let again = rewrite(
            &output.bytes,
            expected(name),
            Transform::Default,
            &zero,
            Limits::default(),
        )
        .unwrap();
        assert_eq!(again.bytes, output.bytes);
        if let Some(root) = std::env::var_os("FOLIO_JPEG_OUTPUT") {
            use std::io::Write;
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(std::path::PathBuf::from(root).join(format!("{name}-zero.jpg")))
                .unwrap()
                .write_all(&output.bytes)
                .unwrap();
        }
    }
}
#[test]
fn redaction_jpeg_respects_effective_pdf_color_and_marker_presence() {
    let (name, source) = fixtures()[2];
    let shape = expected(name);
    let h = inspect(source, shape, Transform::Rgb).unwrap();
    assert!(!h.transformed);
    let output = rewrite(
        source,
        shape,
        Transform::Rgb,
        &selected(&h),
        Limits::default(),
    )
    .unwrap();
    assert!(!output.header.transformed);
    assert!(!output.bytes.windows(5).any(|w| w == b"Adobe"));
    let jfif = marker(source, 0xe0);
    let len = u16::from_be_bytes([source[jfif + 2], source[jfif + 3]]) as usize;
    let mut no_jfif = source.to_vec();
    no_jfif.drain(jfif..jfif + len + 2);
    for (adobe, transformed) in [(0, false), (1, true)] {
        let mut payload = b"Adobe\0d\0\0\0\0\0".to_vec();
        payload[11] = adobe;
        let with = prepend(&no_jfif, 0xee, &payload);
        for transform in [Transform::Default, Transform::Rgb, Transform::YCbCr] {
            let h = inspect(&with, shape, transform).unwrap();
            assert_eq!(h.transformed, transformed);
            let output =
                rewrite(&with, shape, transform, &selected(&h), Limits::default()).unwrap();
            assert_eq!(output.header.transformed, transformed);
        }
        if adobe == 0 {
            assert!(inspect(&prepend(source, 0xee, &payload), shape, Transform::Default).is_err());
        }
    }
    let sof = marker(&no_jfif, 0xc0);
    let sos = marker(&no_jfif, 0xda);
    for (i, id) in b"RGB".iter().enumerate() {
        no_jfif[sof + 10 + 3 * i] = *id;
        no_jfif[sos + 5 + 2 * i] = *id;
    }
    // PDF requires transformation by default, but some readers infer RGB from
    // these IDs. Refuse that disagreement rather than select one interpretation.
    for t in [Transform::Default, Transform::YCbCr] {
        assert!(inspect(&no_jfif, shape, t).is_err());
    }
    for t in [Transform::Rgb] {
        let h = inspect(&no_jfif, shape, t).unwrap();
        assert!(!h.transformed);
        let output = rewrite(&no_jfif, shape, t, &selected(&h), Limits::default()).unwrap();
        assert!(!output.header.transformed);
        assert!(!output.bytes.windows(5).any(|w| w == b"Adobe"));
    }
}

#[test]
fn redaction_jpeg_work_budget_includes_initialization_replacement_and_encoding() {
    let (name, bytes) = fixtures()[0];
    let shape = expected(name);
    let parsed = format::parse(bytes, shape, Transform::Default).unwrap();
    // 64x48 gray = 48 padded 8x8 blocks; one entropy scan plus three
    // conservative passes for initialization, replacement and encoding.
    assert!(rewrite(
        bytes,
        shape,
        Transform::Default,
        &selected(&parsed.header),
        Limits {
            coefficient_work: 48 * 64,
            ..Limits::default()
        }
    )
    .is_err());
    assert_eq!(parsed.header.coefficient_work, 48 * 64 * 4);
    assert_eq!(
        native_attempt(
            bytes,
            &parsed.header,
            Limits {
                coefficient_work: 48 * 64,
                ..Limits::default()
            }
        )
        .0,
        1
    );
}
#[test]
fn redaction_jpeg_exact_output_and_work_limits() {
    let (name, source) = fixtures()[7];
    let shape = expected(name);
    let parsed = format::parse(source, shape, Transform::Default).unwrap();
    let mask = selected(&parsed.header);
    let output = rewrite(source, shape, Transform::Default, &mask, Limits::default()).unwrap();
    let limits = Limits {
        output_bytes: output.bytes.len(),
        coefficient_work: parsed.header.coefficient_work,
        ..Limits::default()
    };
    assert_eq!(
        rewrite(source, shape, Transform::Default, &mask, limits)
            .unwrap()
            .bytes,
        output.bytes
    );
    assert!(rewrite(
        source,
        shape,
        Transform::Default,
        &mask,
        Limits {
            output_bytes: limits.output_bytes - 1,
            ..limits
        }
    )
    .is_err());
    assert!(rewrite(
        source,
        shape,
        Transform::Default,
        &mask,
        Limits {
            coefficient_work: limits.coefficient_work - 1,
            ..limits
        }
    )
    .is_err());
    assert!(rewrite(
        source,
        shape,
        Transform::Default,
        &mask,
        Limits {
            memory_bytes: output.peak_memory,
            ..Limits::default()
        }
    )
    .is_ok());
}
fn native_attempt(bytes: &[u8], h: &Header, limits: Limits) -> (i32, usize) {
    let mask = selected(h);
    let request = Request {
        width: h.width,
        height: h.height,
        components: h.components,
        hmax: h.mcu_width / 8,
        vmax: h.mcu_height / 8,
        transformed: u32::from(h.transformed),
        scans: h.scans,
        memory_limit: limits.memory_bytes,
        output_limit: limits.output_bytes,
        work_limit: limits.coefficient_work,
    };
    let mut result = NativeResult {
        data: std::ptr::null_mut(),
        length: 0,
        peak_memory: 0,
        retained_memory: 0,
    };
    let code = unsafe {
        folio_jpeg_rewrite(
            bytes.as_ptr(),
            bytes.len(),
            &request,
            mask.as_ptr(),
            mask.len(),
            &mut result,
        )
    };
    let _owned = OwnedNative(result.data);
    assert_eq!(result.retained_memory, 0);
    assert!(result.peak_memory <= limits.memory_bytes);
    if code != 0 {
        assert!(result.data.is_null());
        assert_eq!(result.length, 0);
    }
    (code, result.peak_memory)
}

#[test]
fn redaction_jpeg_destination_growth_and_repeated_entropy_mutations() {
    let input = include_bytes!("../tests/fixtures/redaction-jpeg/rgb-large-owned.jpg");
    let shape = (256, 192, 3);
    let h = inspect(input, shape, Transform::Default).unwrap();
    let mask = selected(&h);
    let output = rewrite(input, shape, Transform::Default, &mask, Limits::default()).unwrap();
    assert!(output.bytes.len() > 32768);
    for limit in [4096, 8192, 16384, output.bytes.len() - 1] {
        let (code, peak) = native_attempt(
            input,
            &h,
            Limits {
                output_bytes: limit,
                ..Limits::default()
            },
        );
        assert_eq!(code, 3);
        assert!(peak > limit);
    }
    let exact = rewrite(
        input,
        shape,
        Transform::Default,
        &mask,
        Limits {
            output_bytes: output.bytes.len(),
            ..Limits::default()
        },
    )
    .unwrap();
    assert_eq!(exact.bytes, output.bytes);
    let sos = marker(input, 0xda);
    let len = u16::from_be_bytes([input[sos + 2], input[sos + 3]]) as usize;
    let start = sos + 2 + len;
    let mut refusals = 0;
    for n in 0..128 {
        let mut bytes = input.to_vec();
        let offset = start + (n * 487) % (input.len() - start - 2);
        bytes[offset] ^= 1 << (n % 8);
        let copy = bytes.clone();
        if rewrite(&bytes, shape, Transform::Default, &mask, Limits::default()).is_err() {
            refusals += 1;
        }
        assert_eq!(bytes, copy);
    }
    assert!(refusals > 0);
    assert!(rewrite(input, shape, Transform::Default, &mask, Limits::default()).is_ok());
}

#[test]
fn redaction_jpeg_quantization_endpoints_and_changed_progressive_tables() {
    for (name, source) in [fixtures()[0], fixtures()[3]] {
        let q = marker(source, 0xdb);
        let shape = expected(name);
        for dc in [1, 255] {
            let mut bytes = source.to_vec();
            bytes[q + 5] = dc;
            let h = inspect(&bytes, shape, Transform::Default).unwrap();
            let output = rewrite(
                &bytes,
                shape,
                Transform::Default,
                &selected(&h),
                Limits::default(),
            )
            .unwrap();
            if let Some(root) = std::env::var_os("FOLIO_JPEG_OUTPUT") {
                use std::io::Write;
                let dir = std::path::PathBuf::from(root).join("quantization");
                std::fs::create_dir_all(&dir).unwrap();
                for (suffix, data) in [
                    ("source", bytes.as_slice()),
                    ("actual", output.bytes.as_slice()),
                ] {
                    std::fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(dir.join(format!("{name}-q{dc}-{suffix}.jpg")))
                        .unwrap()
                        .write_all(data)
                        .unwrap();
                }
            }
        }
        let len = u16::from_be_bytes([source[q + 2], source[q + 3]]) as usize;
        let mut changed = source[q + 4..q + 2 + len].to_vec();
        changed[1] = changed[1].saturating_add(1);
        let mut bytes = source[..source.len() - 2].to_vec();
        bytes.extend(segment(0xdb, &changed));
        bytes.extend_from_slice(&[0xff, 0xd9]);
        assert!(inspect(&bytes, shape, Transform::Default).is_err());
    }
}
#[test]
fn redaction_jpeg_native_partial_allocation_and_entropy_failure_cleanup() {
    let (name, source) = fixtures()[7];
    let h = inspect(source, expected(name), Transform::Default).unwrap();
    let (_, peak) = native_attempt(source, &h, Limits::default());
    let mut allocated_failures = 0;
    for cap in (1024..peak).step_by(1024) {
        for _ in 0..3 {
            let (code, used) = native_attempt(
                source,
                &h,
                Limits {
                    memory_bytes: cap,
                    ..Limits::default()
                },
            );
            if code != 0 && used > 0 {
                allocated_failures += 1;
            }
        }
    }
    assert!(allocated_failures > 3);
    let (name, baseline) = fixtures()[2];
    let sos = marker(baseline, 0xda);
    let len = u16::from_be_bytes([baseline[sos + 2], baseline[sos + 3]]) as usize;
    let mut no_entropy = baseline[..sos + 2 + len].to_vec();
    no_entropy.extend_from_slice(&[0xff, 0xd9]);
    let h = inspect(&no_entropy, expected(name), Transform::Default).unwrap();
    for _ in 0..12 {
        let (code, peak) = native_attempt(&no_entropy, &h, Limits::default());
        assert_ne!(code, 0);
        assert!(peak > 0);
    }
    let h = inspect(baseline, expected(name), Transform::Default).unwrap();
    assert_eq!(native_attempt(baseline, &h, Limits::default()).0, 0);
}

#[test]
fn redaction_jpeg_refuses_late_color_markers_and_strips_between_scan_metadata() {
    let (name, baseline) = fixtures()[2];
    let shape = expected(name);
    let jfif = marker(baseline, 0xe0);
    let length = u16::from_be_bytes([baseline[jfif + 2], baseline[jfif + 3]]) as usize;
    let jfif_segment = baseline[jfif..jfif + length + 2].to_vec();
    let mut rgb = baseline.to_vec();
    rgb.drain(jfif..jfif + length + 2);
    let sof = marker(&rgb, 0xc0);
    let sos = marker(&rgb, 0xda);
    for (i, id) in b"RGB".iter().enumerate() {
        rgb[sof + 10 + 3 * i] = *id;
        rgb[sos + 5 + 2 * i] = *id;
    }
    for color in [jfif_segment, segment(0xee, b"Adobe\0d\0\0\0\0\x01")] {
        let mut late = rgb[..rgb.len() - 2].to_vec();
        late.extend(color);
        late.extend_from_slice(&[0xff, 0xd9]);
        assert!(inspect(&late, shape, Transform::Default).is_err());
    }
    let (name, progressive) = fixtures()[7];
    let shape = expected(name);
    let second_scan = progressive
        .windows(2)
        .enumerate()
        .filter(|(_, v)| *v == [0xff, 0xda])
        .nth(1)
        .unwrap()
        .0;
    let mut modified = progressive[..second_scan].to_vec();
    modified.extend(segment(0xe1, b"Exif\0\0BETWEEN-SCAN-PRIVATE"));
    modified.extend(segment(0xfe, b"BETWEEN-SCAN-COMMENT"));
    modified.extend_from_slice(&progressive[second_scan..]);
    let h = inspect(progressive, shape, Transform::Default).unwrap();
    let mask = selected(&h);
    let expected = rewrite(
        progressive,
        shape,
        Transform::Default,
        &mask,
        Limits::default(),
    )
    .unwrap();
    let actual = rewrite(
        &modified,
        shape,
        Transform::Default,
        &mask,
        Limits::default(),
    )
    .unwrap();
    assert_eq!(actual.bytes, expected.bytes);
    assert!(!actual.bytes.windows(7).any(|v| v == b"PRIVATE"));
}
