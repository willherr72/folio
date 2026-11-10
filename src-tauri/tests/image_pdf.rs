use folio_engine::{prepare_pdf_image, ImageAsset, PdfEngine};
use image::{DynamicImage, ImageBuffer, ImageEncoder, ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};
use lopdf::{dictionary, Document, Stream};
use std::{io::Cursor, path::PathBuf};

fn png(image: DynamicImage) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    image.write_to(&mut out, ImageFormat::Png).unwrap();
    out.into_inner()
}
fn pixels(form: &Stream) -> &Stream {
    form.dict
        .get(b"Resources")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"XObject")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(b"Im0")
        .unwrap()
        .as_stream()
        .unwrap()
}
fn decoded(stream: &Stream) -> Vec<u8> {
    if stream.dict.has(b"Filter") {
        stream.decompressed_content().unwrap()
    } else {
        stream.content.clone()
    }
}
fn document(image: Option<folio_engine::PreparedPdfImage>) -> Vec<u8> {
    let mut pdf = Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let font =
        pdf.add_object(dictionary! {"Type"=>"Font", "Subtype"=>"Type1", "BaseFont"=>"Helvetica"});
    let mut resources = dictionary! {"Font"=>dictionary! {"F1"=>font}};
    let mut contents = b"q 0 0 1 rg 0 0 200 300 re f Q\nBT /F1 12 Tf 1 1 1 rg 20 250 Td (Untouched original text) Tj ET\n".to_vec();
    if let Some(image) = image {
        let id = image.install(&mut pdf);
        resources.set("XObject", dictionary! {"Placed"=>id});
        contents.extend_from_slice(b"q 90 0 0 30 50 100 cm /Placed Do Q\n");
    }
    let content = pdf.add_object(Stream::new(dictionary! {}, contents));
    let page = pdf.add_object(dictionary! {"Type"=>"Page", "Parent"=>pages, "MediaBox"=>vec![0.into(),0.into(),200.into(),300.into()], "Resources"=>resources, "Contents"=>content});
    pdf.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages", "Kids"=>vec![page.into()], "Count"=>1}.into(),
    );
    let catalog = pdf.add_object(dictionary! {"Type"=>"Catalog", "Pages"=>pages});
    pdf.trailer.set("Root", catalog);
    let mut bytes = Vec::new();
    pdf.save_to(&mut bytes).unwrap();
    bytes
}
fn artifact(name: &str, bytes: &[u8]) {
    if let Some(root) = std::env::var_os("FOLIO_IMAGE_PDF_OUTPUT") {
        let root = PathBuf::from(root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join(name), bytes).unwrap();
    }
}
fn render(bytes: &[u8]) -> RgbImage {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("image.pdf");
    std::fs::write(&path, bytes).unwrap();
    let engine = PdfEngine::start(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
    )
    .unwrap();
    let opened = engine.open_document(&path).unwrap();
    image::load_from_memory(&engine.render_page(&opened.id, 0, 200).unwrap())
        .unwrap()
        .into_rgb8()
}
fn near(actual: [u8; 3], expected: [u8; 3]) {
    assert!(
        actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 4),
        "{actual:?} != {expected:?}"
    );
}

#[test]
fn png_alpha_eight_and_sixteen_bit_samples_remain_exact_and_render_over_native_background() {
    artifact("background.pdf", &document(None));
    for bits in [8, 16] {
        let image = if bits == 8 {
            DynamicImage::ImageRgba8(RgbaImage::from_fn(3, 1, |x, _| {
                Rgba([255, 0, 0, [0, 128, 255][x as usize]])
            }))
        } else {
            DynamicImage::ImageRgba16(ImageBuffer::from_fn(3, 1, |x, _| {
                Rgba([65535, 0, 0, [0, 32769, 65535][x as usize]])
            }))
        };
        let original = png(image);
        let asset = ImageAsset::parse(original.clone()).unwrap();
        let prepared = prepare_pdf_image(&asset).unwrap();
        let image = pixels(prepared.form());
        assert_eq!(
            image
                .dict
                .get(b"BitsPerComponent")
                .unwrap()
                .as_i64()
                .unwrap(),
            bits
        );
        assert_eq!(
            image.dict.get(b"ColorSpace").unwrap().as_name().unwrap(),
            b"DeviceRGB"
        );
        let expected = if bits == 8 {
            vec![255, 0, 0, 255, 0, 0, 255, 0, 0]
        } else {
            [65535u16, 0, 0, 65535, 0, 0, 65535, 0, 0]
                .into_iter()
                .flat_map(u16::to_be_bytes)
                .collect()
        };
        assert_eq!(decoded(image), expected);
        let mask = image.dict.get(b"SMask").unwrap().as_stream().unwrap();
        assert_eq!(
            decoded(mask),
            if bits == 8 {
                vec![0, 128, 255]
            } else {
                [0u16, 32769, 65535]
                    .into_iter()
                    .flat_map(u16::to_be_bytes)
                    .collect()
            }
        );
        let output = document(Some(prepared));
        artifact(&format!("alpha{bits}.pdf"), &output);
        let raster = render(&output);
        near(raster.get_pixel(65, 185).0, [0, 0, 255]);
        near(raster.get_pixel(95, 185).0, [128, 0, 127]);
        near(raster.get_pixel(125, 185).0, [255, 0, 0]);
        assert_eq!(asset.bytes(), original);
    }
}

fn jpeg(orientation: u8) -> Vec<u8> {
    let image = RgbImage::from_fn(80, 40, |x, y| {
        Rgb(match (x < 40, y < 20) {
            (true, true) => [255, 0, 0],
            (false, true) => [0, 255, 0],
            (true, false) => [0, 0, 255],
            _ => [255, 255, 0],
        })
    });
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 95)
        .encode_image(&image)
        .unwrap();
    let mut exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0".to_vec();
    exif.extend_from_slice(&[orientation, 0, 0, 0, 0, 0, 0, 0]);
    let mut tagged = vec![255, 216, 255, 225];
    tagged.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
    tagged.extend(exif);
    tagged.extend_from_slice(&bytes[2..]);
    tagged
}

#[test]
fn jpeg_dct_bytes_and_all_eight_exif_orientations_survive_pdf_rendering() {
    let expected = [
        [255, 0, 0],
        [0, 255, 0],
        [255, 255, 0],
        [0, 0, 255],
        [255, 0, 0],
        [0, 0, 255],
        [255, 255, 0],
        [0, 255, 0],
    ];
    for orientation in 1..=8 {
        let bytes = jpeg(orientation);
        let asset = ImageAsset::parse(bytes.clone()).unwrap();
        let prepared = prepare_pdf_image(&asset).unwrap();
        let image = pixels(prepared.form());
        assert_eq!(
            image.dict.get(b"Filter").unwrap().as_name().unwrap(),
            b"DCTDecode"
        );
        assert_eq!(image.content, bytes);
        let output = document(Some(prepared));
        artifact(&format!("orientation{orientation}.pdf"), &output);
        near(
            render(&output).get_pixel(65, 176).0,
            expected[orientation as usize - 1],
        );
    }
}

fn chunk(mut bytes: Vec<u8>, name: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut inserted = Vec::new();
    inserted.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    inserted.extend_from_slice(name);
    inserted.extend_from_slice(payload);
    inserted.extend_from_slice(&crc32fast::hash(&inserted[4..]).to_be_bytes());
    bytes.splice(33..33, inserted);
    bytes
}

#[test]
fn png_gamma_is_calibrated_srgb_takes_precedence_and_hdr_is_refused() {
    let original = png(DynamicImage::ImageRgb8(RgbImage::from_pixel(
        3,
        1,
        Rgb([128, 128, 128]),
    )));
    let gamma = chunk(original.clone(), b"gAMA", &100000u32.to_be_bytes());
    let prepared = prepare_pdf_image(&ImageAsset::parse(gamma.clone()).unwrap()).unwrap();
    let color = pixels(prepared.form())
        .dict
        .get(b"ColorSpace")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(color[0].as_name().unwrap(), b"ICCBased");
    let output = document(Some(prepared));
    artifact("gamma1.pdf", &output);
    near(render(&output).get_pixel(95, 185).0, [188, 188, 188]);
    let srgb = chunk(gamma, b"sRGB", &[0]);
    let prepared = prepare_pdf_image(&ImageAsset::parse(srgb).unwrap()).unwrap();
    assert_eq!(
        pixels(prepared.form())
            .dict
            .get(b"ColorSpace")
            .unwrap()
            .as_name()
            .unwrap(),
        b"DeviceRGB"
    );
    for hdr in [
        chunk(original.clone(), b"cICP", &[9, 16, 0, 1]),
        chunk(original, b"cLLI", &[0; 8]),
    ] {
        let asset = ImageAsset::parse(hdr).unwrap();
        assert!(prepare_pdf_image(&asset).is_err());
    }
}

#[test]
fn invalid_or_wrong_component_icc_profiles_are_not_silently_discarded() {
    for profile in [b"bad profile".to_vec(), minimal_profile(b"CMYK")] {
        let mut bytes = Vec::new();
        let mut encoder = image::codecs::png::PngEncoder::new(&mut bytes);
        encoder.set_icc_profile(profile).unwrap();
        encoder
            .write_image(&[10, 20, 30], 1, 1, image::ExtendedColorType::Rgb8)
            .unwrap();
        let asset = ImageAsset::parse(bytes).unwrap();
        assert!(prepare_pdf_image(&asset).is_err());
    }
}
fn minimal_profile(color: &[u8; 4]) -> Vec<u8> {
    let mut bytes = vec![0; 132];
    bytes[..4].copy_from_slice(&132u32.to_be_bytes());
    bytes[8] = 4;
    bytes[12..16].copy_from_slice(b"mntr");
    bytes[16..20].copy_from_slice(color);
    bytes[20..24].copy_from_slice(b"XYZ ");
    bytes[36..40].copy_from_slice(b"acsp");
    bytes
}

#[test]
fn embedded_rgb_and_gray_icc_profiles_retain_original_samples_and_profile_bytes() {
    for (name, profile, kind, channels) in [
        (
            "rgb-profile",
            moxcms::ColorProfile::new_display_p3().encode().unwrap(),
            image::ExtendedColorType::Rgb8,
            vec![180, 80, 40],
        ),
        (
            "gray-profile",
            moxcms::ColorProfile::new_gray_with_gamma(1.)
                .encode()
                .unwrap(),
            image::ExtendedColorType::L8,
            vec![128],
        ),
    ] {
        let mut bytes = Vec::new();
        let mut encoder = image::codecs::png::PngEncoder::new(&mut bytes);
        encoder.set_icc_profile(profile.clone()).unwrap();
        encoder.write_image(&channels, 1, 1, kind).unwrap();
        artifact(&format!("{name}.icc"), &profile);
        let asset = ImageAsset::parse(bytes.clone()).unwrap();
        let prepared = prepare_pdf_image(&asset).unwrap_or_else(|e| panic!("{name}: {e}"));
        let image = pixels(prepared.form());
        assert_eq!(decoded(image), channels);
        let space = image.dict.get(b"ColorSpace").unwrap().as_array().unwrap();
        assert_eq!(space[0].as_name().unwrap(), b"ICCBased");
        assert_eq!(decoded(space[1].as_stream().unwrap()), profile);
        artifact(&format!("{name}.png"), &bytes);
        let output = document(Some(prepared));
        artifact(&format!("{name}.pdf"), &output);
        let raster = render(&output);
        if name == "gray-profile" {
            near(raster.get_pixel(95, 185).0, [188, 188, 188]);
        }
        let mut raster_bytes = Cursor::new(Vec::new());
        raster
            .write_to(&mut raster_bytes, ImageFormat::Png)
            .unwrap();
        artifact(&format!("{name}-pdfium.png"), &raster_bytes.into_inner());
    }
}

#[test]
fn calibrated_profile_is_deterministic_and_custom_whitepoint_and_gray_are_supported() {
    for (name, image, xy) in [
        (
            "custom-white",
            DynamicImage::ImageRgb8(RgbImage::from_pixel(1, 1, Rgb([128, 128, 128]))),
            [34570u32, 35850, 64000, 33000, 30000, 60000, 15000, 6000],
        ),
        (
            "gray-gamma",
            DynamicImage::ImageLuma8(image::GrayImage::from_pixel(1, 1, image::Luma([128]))),
            [31270, 32900, 64000, 33000, 30000, 60000, 15000, 6000],
        ),
    ] {
        let data = chunk(
            chunk(png(image), b"gAMA", &100000u32.to_be_bytes()),
            b"cHRM",
            &xy.into_iter()
                .flat_map(u32::to_be_bytes)
                .collect::<Vec<_>>(),
        );
        let asset = ImageAsset::parse(data.clone()).unwrap();
        let first = prepare_pdf_image(&asset).unwrap();
        let second = prepare_pdf_image(&asset).unwrap();
        assert_eq!(first.form().content, second.form().content);
        assert_eq!(
            format!("{:?}", first.form().dict),
            format!("{:?}", second.form().dict)
        );
        let space = pixels(first.form())
            .dict
            .get(b"ColorSpace")
            .unwrap()
            .as_array()
            .unwrap();
        let profile = decoded(space[1].as_stream().unwrap());
        assert_eq!(&profile[24..36], &[7, 208, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0]);
        let output = document(Some(first));
        artifact(&format!("{name}.pdf"), &output);
        artifact(&format!("{name}.png"), &data);
        near(render(&output).get_pixel(95, 185).0, [188, 188, 188]);
    }
}

#[test]
fn jpeg_baseline_progressive_gray_and_cmyk_use_original_resources() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/image-assets");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("manifest.json")).unwrap()).unwrap();
    for case in manifest["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let bytes = std::fs::read(root.join(format!("{name}.jpg"))).unwrap();
        let asset = ImageAsset::parse(bytes.clone()).unwrap();
        let prepared = prepare_pdf_image(&asset).unwrap();
        let image = pixels(prepared.form());
        assert_eq!(image.content, bytes);
        let expected: &[u8] = if name.starts_with("cmyk") {
            b"DeviceCMYK"
        } else if name.starts_with("l-") {
            b"DeviceGray"
        } else {
            b"DeviceRGB"
        };
        assert_eq!(
            image.dict.get(b"ColorSpace").unwrap().as_name().unwrap(),
            expected
        );
        if name.starts_with("cmyk") {
            assert_eq!(
                image.dict.get(b"Decode").unwrap().as_array().unwrap().len(),
                8
            );
        }
        let output = document(Some(prepared));
        artifact(&format!("corpus-{name}.pdf"), &output);
        render(&output);
    }
}

fn cmyk_profile() -> Vec<u8> {
    use moxcms::{
        ColorProfile, DataColorSpace, LutDataType, LutStore, LutType, LutWarehouse, Matrix3d,
        ProfileClass, WHITE_POINT_D50,
    };
    // Original synthetic four-input XYZ profile. Different C/M/Y/K values map to
    // neutral tones, making incorrect RGB attachment or inverted polarity visible.
    let mut clut = Vec::new();
    for c in 0..2 {
        for m in 0..2 {
            for y in 0..2 {
                for k in 0..2 {
                    let light =
                        (1. - 0.4 * c as f32 - 0.3 * m as f32 - 0.2 * y as f32) * (1. - k as f32);
                    clut.extend(
                        [0.9642, 1., 0.8249].map(|white| (light * white * 32768.).round() as u16),
                    );
                }
            }
        }
    }
    let identity = vec![0u16, 65535];
    let mut profile = ColorProfile::default();
    profile.color_space = DataColorSpace::Cmyk;
    profile.pcs = DataColorSpace::Xyz;
    profile.profile_class = ProfileClass::OutputDevice;
    profile.white_point = WHITE_POINT_D50.to_xyzd();
    profile.media_white_point = Some(WHITE_POINT_D50.to_xyzd());
    profile.lut_a_to_b_perceptual = Some(LutWarehouse::Lut(LutDataType {
        num_input_channels: 4,
        num_output_channels: 3,
        num_clut_grid_points: 2,
        matrix: Matrix3d {
            v: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
        },
        num_input_table_entries: 2,
        num_output_table_entries: 2,
        input_table: LutStore::Store16(identity.repeat(4)),
        clut_table: LutStore::Store16(clut),
        output_table: LutStore::Store16(identity.repeat(3)),
        lut_type: LutType::Lut16,
    }));
    profile.encode().unwrap()
}

#[test]
fn cmyk_icc_is_attached_to_original_four_component_jpeg_not_rgb_preview() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/image-assets");
    let original = std::fs::read(root.join("cmyk-odd.jpg")).unwrap();
    let profile = cmyk_profile();
    let mut payload = b"ICC_PROFILE\0\x01\x01".to_vec();
    payload.extend_from_slice(&profile);
    let mut bytes = vec![255, 216, 255, 226];
    bytes.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
    bytes.extend(payload);
    bytes.extend_from_slice(&original[2..]);
    let asset = ImageAsset::parse(bytes.clone()).unwrap();
    let prepared = prepare_pdf_image(&asset).unwrap();
    let image = pixels(prepared.form());
    assert_eq!(image.content, bytes);
    let space = image.dict.get(b"ColorSpace").unwrap().as_array().unwrap();
    let embedded = space[1].as_stream().unwrap();
    assert_eq!(embedded.dict.get(b"N").unwrap().as_i64().unwrap(), 4);
    assert_eq!(decoded(embedded), profile);
    let output = document(Some(prepared));
    artifact("cmyk-profile.pdf", &output);
    artifact("cmyk-profile.jpg", &bytes);
    let raster = render(&output);
    let mut rendered = Cursor::new(Vec::new());
    raster.write_to(&mut rendered, ImageFormat::Png).unwrap();
    artifact("cmyk-profile-pdfium.png", &rendered.into_inner());
}

#[test]
fn unusable_declared_jpeg_profiles_are_refused() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/image-assets");
    let original = std::fs::read(root.join("cmyk-odd.jpg")).unwrap();
    for sequences in [vec![(1, 2)], vec![(1, 2), (1, 2)], vec![(0, 1)]] {
        let mut bytes = original[..2].to_vec();
        for (sequence, count) in sequences {
            let mut payload = b"ICC_PROFILE\0".to_vec();
            payload.extend([sequence, count]);
            payload.extend(cmyk_profile());
            bytes.extend([255, 226]);
            bytes.extend(((payload.len() + 2) as u16).to_be_bytes());
            bytes.extend(payload);
        }
        bytes.extend_from_slice(&original[2..]);
        let asset = ImageAsset::parse(bytes).unwrap();
        assert!(
            prepare_pdf_image(&asset).is_err(),
            "Unusable declared JPEG profile must not select device color"
        );
    }
}

#[test]
fn unusable_declared_png_profiles_are_refused_even_with_srgb_cicp() {
    for payload in [
        b"profile\0\x01bad".as_slice(),
        b"profile\0\x00bad".as_slice(),
    ] {
        for cicp in [false, true] {
            let mut bytes = chunk(
                png(DynamicImage::ImageRgb8(RgbImage::new(1, 1))),
                b"iCCP",
                payload,
            );
            if cicp {
                bytes = chunk(bytes, b"cICP", &[1, 13, 0, 1]);
            }
            let asset = ImageAsset::parse(bytes).unwrap();
            assert!(
                prepare_pdf_image(&asset).is_err(),
                "Declared malformed iCCP must not disappear"
            );
        }
    }
}

#[test]
fn standard_png_mastering_display_chunk_is_refused() {
    let bytes = chunk(
        png(DynamicImage::ImageRgb8(RgbImage::new(1, 1))),
        b"mDCV",
        &[0; 24],
    );
    let asset = ImageAsset::parse(bytes).unwrap();
    assert!(
        prepare_pdf_image(&asset).is_err(),
        "HDR mastering metadata must not be silently ignored"
    );
}

#[test]
fn unusable_required_icc_tags_are_refused() {
    for tag in [b"kTRC", b"wtpt"] {
        let mut profile = moxcms::ColorProfile::new_gray_with_gamma(1.)
            .encode()
            .unwrap();
        let count = u32::from_be_bytes(profile[128..132].try_into().unwrap()) as usize;
        let entry = (0..count)
            .map(|i| 132 + i * 12)
            .find(|&at| &profile[at..at + 4] == tag)
            .unwrap();
        if tag == b"kTRC" {
            profile[entry + 8..entry + 12].copy_from_slice(&8u32.to_be_bytes());
        } else {
            let offset =
                u32::from_be_bytes(profile[entry + 4..entry + 8].try_into().unwrap()) as usize;
            profile[offset..offset + 4].copy_from_slice(b"oops");
        }
        let mut bytes = Vec::new();
        let mut encoder = image::codecs::png::PngEncoder::new(&mut bytes);
        encoder.set_icc_profile(profile).unwrap();
        encoder
            .write_image(&[128], 1, 1, image::ExtendedColorType::L8)
            .unwrap();
        let asset = ImageAsset::parse(bytes).unwrap();
        assert!(
            prepare_pdf_image(&asset).is_err(),
            "Invalid required tag {tag:?} must not be embedded"
        );
    }
}

#[test]
fn icc_lut_channels_must_match_source_and_pcs() {
    // Header and PNG both say RGB, but the otherwise valid LUT consumes CMYK.
    let mut profile = cmyk_profile();
    profile[16..20].copy_from_slice(b"RGB ");
    let mut bytes = Vec::new();
    let mut encoder = image::codecs::png::PngEncoder::new(&mut bytes);
    encoder.set_icc_profile(profile).unwrap();
    encoder
        .write_image(&[128, 128, 128], 1, 1, image::ExtendedColorType::Rgb8)
        .unwrap();
    let asset = ImageAsset::parse(bytes).unwrap();
    assert!(
        prepare_pdf_image(&asset).is_err(),
        "A four-input LUT cannot describe RGB samples"
    );
}

#[test]
fn jpeg_profiles_between_progressive_scans_are_checked() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/image-assets");
    let original = std::fs::read(root.join("rgb-progressive.jpg")).unwrap();
    let at = original
        .windows(2)
        .enumerate()
        .filter(|(_, pair)| *pair == [255, 218])
        .nth(1)
        .unwrap()
        .0;
    let profile = moxcms::ColorProfile::new_srgb().encode().unwrap();
    for count in [1, 2] {
        let mut payload = b"ICC_PROFILE\0".to_vec();
        payload.extend([1, count]);
        payload.extend_from_slice(&profile);
        let mut bytes = original[..at].to_vec();
        bytes.extend([255, 226]);
        bytes.extend(((payload.len() + 2) as u16).to_be_bytes());
        bytes.extend(payload);
        bytes.extend_from_slice(&original[at..]);
        let asset = ImageAsset::parse(bytes.clone()).unwrap();
        let result = prepare_pdf_image(&asset);
        if count == 1 {
            let prepared = result.unwrap();
            let image = pixels(prepared.form());
            assert_eq!(image.content, bytes);
            let space = image.dict.get(b"ColorSpace").unwrap().as_array().unwrap();
            assert_eq!(decoded(space[1].as_stream().unwrap()), profile);
        } else {
            assert!(
                result.is_err(),
                "Incomplete ICC between scans must not disappear"
            );
        }
    }
}

#[test]
fn duplicate_png_profiles_cannot_hide_a_bad_declaration() {
    let mut bytes = Vec::new();
    let mut encoder = image::codecs::png::PngEncoder::new(&mut bytes);
    encoder
        .set_icc_profile(moxcms::ColorProfile::new_srgb().encode().unwrap())
        .unwrap();
    encoder
        .write_image(&[128, 128, 128], 1, 1, image::ExtendedColorType::Rgb8)
        .unwrap();
    assert_eq!(&bytes[37..41], b"iCCP");
    let valid_chunk_len = u32::from_be_bytes(bytes[33..37].try_into().unwrap()) as usize + 12;
    let payload = b"profile\0\x00bad";
    let bytes = chunk(bytes, b"iCCP", payload);
    for bad_first in [true, false] {
        let mut data = bytes.clone();
        if !bad_first {
            data[33..33 + payload.len() + 12 + valid_chunk_len].rotate_left(payload.len() + 12);
        }
        let asset = ImageAsset::parse(data).unwrap();
        assert!(
            prepare_pdf_image(&asset).is_err(),
            "Duplicate profile declaration must not be ignored (bad_first={bad_first})"
        );
    }
}
