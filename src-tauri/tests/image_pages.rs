use folio_engine::{ExportRequest, ImagePageOptions, PagePlan, PdfEngine, RecoveryStore};
use image::{Rgb, RgbImage, Rgba, RgbaImage};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
};

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}
fn options() -> ImagePageOptions {
    ImagePageOptions {
        width: 200.,
        height: 300.,
        margin: 20.,
        auto_orientation: false,
    }
}
fn plans(info: &folio_engine::DocumentInfo) -> Vec<PagePlan> {
    info.pages
        .iter()
        .enumerate()
        .map(|(i, p)| PagePlan {
            crop: None,
            id: format!("p{i}"),
            source_id: info.id.clone(),
            page_index: i,
            width: p.width,
            height: p.height,
            rotation: 0,
            overlays: vec![],
        })
        .collect()
}
fn raster(engine: &PdfEngine, id: &str, i: usize) -> RgbImage {
    image::load_from_memory(&engine.render_page(id, i, 200).unwrap())
        .unwrap()
        .into_rgb8()
}
fn near(actual: Rgb<u8>, expected: [u8; 3]) {
    assert!(
        actual
            .0
            .iter()
            .zip(expected)
            .all(|(a, b)| a.abs_diff(b) < 8),
        "{actual:?} != {expected:?}"
    );
}
fn jpeg(path: &Path, orientation: u8) {
    let img = RgbImage::from_fn(80, 40, |x, y| {
        if x < 40 {
            if y < 20 {
                Rgb([255, 0, 0])
            } else {
                Rgb([0, 0, 255])
            }
        } else {
            if y < 20 {
                Rgb([0, 255, 0])
            } else {
                Rgb([255, 255, 0])
            }
        }
    });
    let mut bytes = vec![];
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 95)
        .encode_image(&img)
        .unwrap();
    // Little-endian TIFF IFD with one Orientation SHORT, embedded in APP1.
    let mut exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0".to_vec();
    exif.extend_from_slice(&[orientation, 0, 0, 0, 0, 0, 0, 0]);
    let mut tagged = vec![255, 216, 255, 225];
    tagged.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
    tagged.extend(exif);
    tagged.extend_from_slice(&bytes[2..]);
    fs::write(path, tagged).unwrap();
}

#[test]
fn ordered_images_fit_and_alpha_is_white_with_export_and_recovery() {
    let temp = tempfile::tempdir().unwrap();
    let a = temp.path().join("alpha.png");
    let b = temp.path().join("blue.png");
    RgbaImage::from_pixel(80, 40, Rgba([255, 0, 0, 128]))
        .save(&a)
        .unwrap();
    RgbImage::from_pixel(20, 60, Rgb([0, 0, 255]))
        .save(&b)
        .unwrap();
    let before = fs::read(&a).unwrap();
    let engine = engine();
    let doc = engine
        .create_image_document(vec![a.clone(), b.clone()], options())
        .unwrap();
    assert_eq!(doc.pages.len(), 2);
    let p = raster(&engine, &doc.id, 0);
    near(*p.get_pixel(100, 150), [255, 127, 127]);
    near(*p.get_pixel(10, 150), [255, 255, 255]);
    near(*p.get_pixel(100, 100), [255, 255, 255]);
    let p = raster(&engine, &doc.id, 1);
    near(*p.get_pixel(100, 25), [0, 0, 255]);
    near(*p.get_pixel(50, 150), [255, 255, 255]);
    let output = temp.path().join("copy.pdf");
    engine
        .export_pdf(
            ExportRequest {
                pages: plans(&doc),
                flatten: false,
            },
            &output,
        )
        .unwrap();
    let reopened = engine.open_document(&output).unwrap();
    assert_eq!(
        raster(&engine, &doc.id, 0),
        raster(&engine, &reopened.id, 0)
    );
    assert_eq!(fs::read(&a).unwrap(), before);
    let store = RecoveryStore::new(temp.path().join("recovery")).unwrap();
    store.save(&engine,json!({"version":1,"activeId":"a","tabs":[{"id":"a","savedDigest":"","dirty":true,"zoom":100,"scrollPosition":{"top":0,"left":0},"document":{"name":"Images.pdf","selectedPageId":"p0","selectedOverlayId":null,"pages":plans(&doc)}}]})).unwrap();
    fs::remove_file(a).unwrap();
    fs::remove_file(b).unwrap();
    engine.close_document(&doc.id).unwrap();
    let recovered = store.load(&engine).unwrap().unwrap();
    let id = recovered["tabs"][0]["document"]["pages"][0]["sourceId"]
        .as_str()
        .unwrap();
    assert_eq!(raster(&engine, id, 1), raster(&engine, &reopened.id, 1));
}

#[test]
fn jpeg_exif_rotation_and_flip_precede_auto_page_orientation() {
    let temp = tempfile::tempdir().unwrap();
    let engine = engine();
    for (orientation, top_left) in [
        (1, [255, 0, 0]),
        (2, [0, 255, 0]),
        (3, [255, 255, 0]),
        (4, [0, 0, 255]),
        (5, [255, 0, 0]),
        (6, [0, 0, 255]),
        (7, [255, 255, 0]),
        (8, [0, 255, 0]),
    ] {
        let path = temp.path().join(format!("exif{orientation}.jpg"));
        jpeg(&path, orientation);
        let mut opts = options();
        opts.auto_orientation = true;
        opts.margin = 0.;
        let doc = engine.create_image_document(vec![path], opts).unwrap();
        let p = &doc.pages[0];
        assert_eq!(
            (p.width, p.height),
            if orientation < 5 {
                (300., 200.)
            } else {
                (200., 300.)
            }
        );
        let png = raster(&engine, &doc.id, 0);
        near(*png.get_pixel(png.width() / 4, png.height() / 4), top_left);
        engine.close_document(&doc.id).unwrap();
    }
}

#[test]
fn rejects_invalid_settings_files_and_mixed_lists_without_touching_inputs() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("good.png");
    RgbImage::from_pixel(10, 10, Rgb([10, 20, 30]))
        .save(&path)
        .unwrap();
    let before = fs::read(&path).unwrap();
    let engine = engine();
    assert!(engine
        .create_image_document(vec![], options())
        .unwrap_err()
        .to_string()
        .contains("32"));
    assert!(engine
        .create_image_document(vec![path.clone(); 33], options())
        .is_err());
    for (w, h, m) in [
        (f32::NAN, 300., 20.),
        (100., 300., 20.),
        (200., 300., 100.),
        (200., 300., -1.),
    ] {
        let mut opts = options();
        opts.width = w;
        opts.height = h;
        opts.margin = m;
        assert!(engine
            .create_image_document(vec![path.clone()], opts)
            .is_err());
    }
    let bad = temp.path().join("broken.jpg");
    fs::write(&bad, b"not an image").unwrap();
    assert!(engine
        .create_image_document(vec![path.clone(), bad], options())
        .unwrap_err()
        .to_string()
        .contains("broken.jpg"));
    assert!(engine
        .create_image_document(vec![temp.path().to_path_buf()], options())
        .is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    assert!(engine.create_image_document(vec![path], options()).is_ok());
}

#[test]
fn huge_declared_dimensions_and_compressed_files_are_rejected_before_full_decode() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("huge.png");
    RgbImage::new(1, 1).save(&path).unwrap();
    let mut bytes = fs::read(&path).unwrap();
    bytes[16..20].copy_from_slice(&6000_u32.to_be_bytes());
    bytes[20..24].copy_from_slice(&6000_u32.to_be_bytes());
    let mut crc = 0xffff_ffff_u32;
    for byte in &bytes[12..29] {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    bytes[29..33].copy_from_slice(&(!crc).to_be_bytes());
    fs::write(&path, bytes).unwrap();
    let engine = engine();
    let error = engine
        .create_image_document(vec![path], options())
        .unwrap_err()
        .to_string();
    assert!(error.contains("32 million"), "{error}");
    let large = temp.path().join("large.jpg");
    fs::File::create(&large)
        .unwrap()
        .set_len(64 * 1024 * 1024 + 1)
        .unwrap();
    assert!(engine
        .create_image_document(vec![large], options())
        .unwrap_err()
        .to_string()
        .contains("64 MiB"));
}
