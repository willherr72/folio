use folio_engine::{ImageAsset, ImageRegistry};
use image::{DynamicImage, ImageBuffer, ImageEncoder, ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};
use std::{
    io::Cursor,
    sync::{Arc, Barrier},
};

fn png(color: [u8; 4]) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(RgbaImage::from_pixel(3, 2, Rgba(color)))
        .write_to(&mut bytes, ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}

#[test]
fn png_alpha_original_bytes_and_content_identity_survive_decode() {
    let bytes = png([20, 100, 240, 128]);
    let asset = ImageAsset::parse(bytes.clone()).unwrap();
    assert_eq!(asset.bytes(), bytes);
    assert_eq!(asset.info().width, 3);
    assert_eq!(asset.info().height, 2);
    assert!(asset.info().has_alpha);
    assert_eq!(asset.info().bits_per_channel, 8);
    assert_eq!(asset.info().mime_type, "image/png");
    assert_eq!(asset.info().byte_length, bytes.len());
    assert_eq!(
        asset
            .decode()
            .unwrap()
            .pixels
            .into_rgba8()
            .get_pixel(1, 1)
            .0,
        [20, 100, 240, 128]
    );
    assert_eq!(asset.info().id, ImageAsset::parse(bytes).unwrap().info().id);
    assert_ne!(
        asset.info().id,
        ImageAsset::parse(png([20, 100, 240, 127]))
            .unwrap()
            .info()
            .id
    );
}

#[test]
fn sixteen_bit_png_and_profile_metadata_are_retained() {
    let pixels = ImageBuffer::<Rgba<u16>, Vec<u16>>::from_pixel(2, 2, Rgba([1, 257, 65534, 32769]));
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::ImageRgba16(pixels.clone())
        .write_to(&mut bytes, ImageFormat::Png)
        .unwrap();
    let asset = ImageAsset::parse(bytes.into_inner()).unwrap();
    assert_eq!(asset.info().bits_per_channel, 16);
    assert_eq!(asset.decode().unwrap().pixels.into_rgba16(), pixels);
    // This is a metadata-retention test, not an ICC color-validity claim.
    let profile = b"retained profile payload".to_vec();
    let mut encoded = Vec::new();
    let mut encoder = image::codecs::png::PngEncoder::new(&mut encoded);
    encoder.set_icc_profile(profile.clone()).unwrap();
    encoder
        .write_image(&[10, 20, 30, 40], 1, 1, image::ExtendedColorType::Rgba8)
        .unwrap();
    let asset = ImageAsset::parse(encoded.clone()).unwrap();
    assert_eq!(asset.bytes(), encoded);
    assert_eq!(asset.decode().unwrap().icc_profile, Some(profile));
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
fn all_jpeg_exif_orientations_preserve_bytes_and_displayed_dimensions() {
    let red = [255, 0, 0];
    let green = [0, 255, 0];
    let blue = [0, 0, 255];
    let yellow = [255, 255, 0];
    for (orientation, expected) in (1..=8).zip([red, green, yellow, blue, red, blue, yellow, green])
    {
        let bytes = jpeg(orientation);
        let asset = ImageAsset::parse(bytes.clone()).unwrap();
        assert_eq!(asset.bytes(), bytes);
        assert_eq!(asset.info().mime_type, "image/jpeg");
        assert!(!asset.info().has_alpha);
        let expected_size = if orientation <= 4 { (80, 40) } else { (40, 80) };
        assert_eq!((asset.info().width, asset.info().height), expected_size);
        let pixels = asset.decode().unwrap().pixels.into_rgb8();
        assert_eq!(pixels.dimensions(), expected_size);
        let actual = pixels.get_pixel(10, 10).0;
        assert!(
            actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) < 10),
            "orientation{orientation}: {actual:?}"
        );
    }
}

fn oversized_header(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = png([1, 2, 3, 4]);
    bytes[16..20].copy_from_slice(&width.to_be_bytes());
    bytes[20..24].copy_from_slice(&height.to_be_bytes());
    let mut crc = 0xffff_ffffu32;
    for byte in &bytes[12..29] {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ if crc & 1 != 0 { 0xedb8_8320 } else { 0 };
        }
    }
    bytes[29..33].copy_from_slice(&(!crc).to_be_bytes());
    bytes
}

#[test]
fn animated_png_requires_an_explicit_still_frame_and_oversized_files_are_not_read() {
    let original = png([1, 2, 3, 4]);
    let mut chunk = b"acTL".to_vec();
    chunk.extend_from_slice(&1u32.to_be_bytes());
    chunk.extend_from_slice(&0u32.to_be_bytes());
    let mut crc = 0xffff_ffffu32;
    for byte in &chunk {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ if crc & 1 != 0 { 0xedb8_8320 } else { 0 };
        }
    }
    let mut bytes = original[..33].to_vec();
    bytes.extend_from_slice(&8u32.to_be_bytes());
    bytes.extend(chunk);
    bytes.extend_from_slice(&(!crc).to_be_bytes());
    bytes.extend_from_slice(&original[33..]);
    assert!(ImageAsset::parse(bytes)
        .unwrap_err()
        .to_string()
        .contains("Animated PNG"));
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("oversized.png");
    std::fs::File::create(&path)
        .unwrap()
        .set_len(64 * 1024 * 1024 + 1)
        .unwrap();
    assert!(ImageAsset::read(&path)
        .unwrap_err()
        .to_string()
        .contains("64 MiB"));
}

#[test]
fn malformed_unsupported_and_oversized_images_are_rejected_before_publication() {
    let registry = ImageRegistry::new();
    for bytes in [
        b"not an image".to_vec(),
        b"GIF89a".to_vec(),
        png([1, 2, 3, 4])[..25].to_vec(),
        oversized_header(32769, 1),
        oversized_header(8000, 5000),
    ] {
        assert!(registry.register(bytes).is_err());
        assert!(registry.ids().unwrap().is_empty());
    }
    let error = ImageAsset::parse(oversized_header(8000, 5000))
        .unwrap_err()
        .to_string();
    assert!(error.contains("32 million"), "{error}");
    let temp = tempfile::tempdir().unwrap();
    assert!(ImageAsset::read(temp.path()).is_err());
    let path = temp.path().join("photo.png");
    let bytes = png([20, 30, 40, 50]);
    std::fs::write(&path, &bytes).unwrap();
    let asset = ImageAsset::read(&path).unwrap();
    assert_eq!(asset.bytes(), bytes);
    assert_eq!(std::fs::read(path).unwrap(), bytes);
}

#[test]
fn truncated_jpeg_entropy_is_rejected_before_publication() {
    let bytes = jpeg(1);
    let sos = bytes
        .windows(2)
        .position(|pair| pair == [0xff, 0xda])
        .unwrap();
    let length = usize::from(u16::from_be_bytes([bytes[sos + 2], bytes[sos + 3]]));
    let mut truncated = bytes[..sos + 2 + length + 1].to_vec();
    truncated.extend_from_slice(&[0xff, 0xd9]);
    let registry = ImageRegistry::new();
    assert!(
        registry.register(truncated).is_err(),
        "partial JPEG pixels must not be published"
    );
    assert!(registry.ids().unwrap().is_empty());
}

#[test]
fn missing_or_corrupt_png_terminal_chunks_are_rejected_before_publication() {
    let bytes = png([1, 2, 3, 4]);
    let mut corrupt = bytes.clone();
    *corrupt.last_mut().unwrap() ^= 1;
    for invalid in [bytes[..bytes.len() - 12].to_vec(), corrupt] {
        let registry = ImageRegistry::new();
        assert!(
            registry.register(invalid).is_err(),
            "incomplete PNG must not be published"
        );
        assert!(registry.ids().unwrap().is_empty());
    }
}

#[test]
fn jpeg_baseline_progressive_grayscale_cmyk_and_restart_samples_match_independent_reference() {
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/image-assets");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("manifest.json")).unwrap()).unwrap();
    for case in manifest["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let bytes = std::fs::read(root.join(format!("{name}.jpg"))).unwrap();
        let asset =
            ImageAsset::parse(bytes.clone()).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(asset.bytes(), bytes);
        assert_eq!(asset.info().id, case["jpegSha256"].as_str().unwrap());
        let actual = asset.decode().unwrap().pixels.into_rgb8();
        let expected = image::open(root.join(format!("{name}.png")))
            .unwrap()
            .into_rgb8();
        assert_eq!(actual.dimensions(), expected.dimensions());
        let max_difference = actual
            .as_raw()
            .iter()
            .zip(expected.as_raw())
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap();
        assert!(
            max_difference <= 3,
            "{name}: max channel difference {max_difference}"
        );
    }
}

#[test]
fn jpeg_missing_end_marker_and_multiple_entropy_truncations_are_rejected() {
    let bytes = jpeg(1);
    let sos = bytes
        .windows(2)
        .position(|pair| pair == [0xff, 0xda])
        .unwrap();
    let length = usize::from(u16::from_be_bytes([bytes[sos + 2], bytes[sos + 3]]));
    let start = sos + 2 + length;
    for length in [0, 1, 4, 16, (bytes.len() - start) / 2] {
        let mut truncated = bytes[..start + length].to_vec();
        truncated.extend_from_slice(&[0xff, 0xd9]);
        assert!(
            ImageAsset::parse(truncated).is_err(),
            "accepted {length} entropy bytes"
        );
    }
    assert!(ImageAsset::parse(bytes[..bytes.len() - 2].to_vec()).is_err());
}

#[test]
fn jpeg_profile_chunks_survive_and_oversized_headers_are_rejected_before_entropy_decode() {
    // Metadata retention only: validity/color conversion of an ICC is a later layer.
    let profile = b"original JPEG profile payload";
    let original = jpeg(6);
    let mut bytes = original[..2].to_vec();
    for (index, payload) in [(2, &profile[8..]), (1, &profile[..8])] {
        let mut chunk = b"ICC_PROFILE\0".to_vec();
        chunk.extend_from_slice(&[index, 2]);
        chunk.extend_from_slice(payload);
        bytes.extend_from_slice(&[0xff, 0xe2]);
        bytes.extend_from_slice(&((chunk.len() + 2) as u16).to_be_bytes());
        bytes.extend(chunk);
    }
    bytes.extend_from_slice(&original[2..]);
    let asset = ImageAsset::parse(bytes.clone()).unwrap();
    assert_eq!(asset.bytes(), bytes);
    assert_eq!((asset.info().width, asset.info().height), (40, 80));
    assert_eq!(
        asset.decode().unwrap().icc_profile.as_deref(),
        Some(profile.as_slice())
    );

    let sof = original
        .windows(2)
        .position(|pair| pair == [0xff, 0xc0])
        .unwrap();
    for (width, height, message) in [(32769u16, 1u16, "32,768"), (8000, 5000, "32 million")] {
        let mut huge = original.clone();
        huge[sof + 5..sof + 7].copy_from_slice(&height.to_be_bytes());
        huge[sof + 7..sof + 9].copy_from_slice(&width.to_be_bytes());
        let error = ImageAsset::parse(huge).unwrap_err().to_string();
        assert!(error.contains(message), "{error}");
    }
}

#[test]
fn independent_leases_deduplicate_and_stale_release_cannot_remove_other_owners() {
    let registry = ImageRegistry::new();
    let bytes = png([10, 20, 30, 128]);
    let a = registry.register(bytes.clone()).unwrap();
    let b = registry.register(bytes).unwrap();
    assert_eq!(a.image.id, b.image.id);
    assert_ne!(a.token, b.token);
    assert_eq!(registry.ids().unwrap().len(), 1);
    let borrowed = registry.get(&a.image.id).unwrap();
    registry.release(&a.token).unwrap();
    registry.release(&a.token).unwrap();
    let c = registry.acquire(&b.image.id).unwrap();
    registry.release(&b.token).unwrap();
    assert!(registry.get(&c.image.id).is_ok());
    registry.release(&c.token).unwrap();
    assert!(registry.get(&c.image.id).is_err());
    assert_eq!(
        borrowed
            .decode()
            .unwrap()
            .pixels
            .into_rgba8()
            .get_pixel(0, 0)
            .0,
        [10, 20, 30, 128]
    );
    let d = registry.register(png([10, 20, 30, 128])).unwrap();
    registry.release(&a.token).unwrap();
    registry.release(&c.token).unwrap();
    assert!(registry.get(&d.image.id).is_ok());
    registry.release_all(&[d]).unwrap();
    assert!(registry.ids().unwrap().is_empty());
}

#[test]
fn failed_batch_preserves_existing_owners_and_successful_batch_has_independent_leases() {
    let registry = ImageRegistry::new();
    let owned = registry.register(png([255, 0, 0, 255])).unwrap();
    let excessive = (0..64)
        .map(|n| Arc::new(ImageAsset::parse(png([n, 1, 2, 255])).unwrap()))
        .collect::<Vec<_>>();
    assert!(registry.register_assets(&excessive).is_err());
    assert_eq!(
        registry.ids().unwrap().into_iter().collect::<Vec<_>>(),
        vec![owned.image.id.clone()]
    );
    let asset = registry.get(&owned.image.id).unwrap();
    let batch = registry.register_assets(&[asset.clone(), asset]).unwrap();
    assert_eq!(batch.len(), 2);
    assert_ne!(batch[0].token, batch[1].token);
    registry.release_all(&batch).unwrap();
    assert!(registry.get(&owned.image.id).is_ok());
    registry.release(&owned.token).unwrap();
    assert!(registry.ids().unwrap().is_empty());
}

#[test]
fn concurrent_imports_share_one_asset_and_each_owner_survives_other_releases() {
    let registry = Arc::new(ImageRegistry::new());
    let ready = Arc::new(Barrier::new(9));
    let release = Arc::new(Barrier::new(9));
    let threads = (0..8)
        .map(|_| {
            let registry = registry.clone();
            let ready = ready.clone();
            let release = release.clone();
            std::thread::spawn(move || {
                let owner = registry.register(png([20, 30, 40, 200])).unwrap();
                ready.wait();
                assert!(registry.get(&owner.image.id).is_ok());
                release.wait();
                registry.release(&owner.token).unwrap();
            })
        })
        .collect::<Vec<_>>();
    ready.wait();
    assert_eq!(registry.ids().unwrap().len(), 1);
    release.wait();
    for thread in threads {
        thread.join().unwrap();
    }
    assert!(registry.ids().unwrap().is_empty());
}
