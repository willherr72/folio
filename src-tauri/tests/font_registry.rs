use folio_engine::{FontAsset, FontRegistry};

fn font() -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(&std::env::var_os("WINDIR").unwrap_or_else(|| "C:/Windows".into()))
            .join("Fonts/arial.ttf"),
    )
    .unwrap()
}

fn set_permissions(bytes: &mut [u8], flags: u16) {
    let count = u16::from_be_bytes(bytes[4..6].try_into().unwrap()) as usize;
    let record = (0..count)
        .map(|index| 12 + index * 16)
        .find(|&offset| &bytes[offset..offset + 4] == b"OS/2")
        .unwrap();
    let offset = u32::from_be_bytes(bytes[record + 8..record + 12].try_into().unwrap()) as usize;
    bytes[offset + 8..offset + 10].copy_from_slice(&flags.to_be_bytes());
}

#[test]
fn registers_exact_bytes_deduplicates_and_validates_unicode_metrics() {
    let registry = FontRegistry::new();
    let bytes = font();
    let info = registry.register(bytes.clone()).unwrap();
    assert!(info.name.contains("Arial"));
    assert_eq!(info.weight, 400);
    assert!(!info.italic);
    assert_eq!(info.id.len(), 64);
    let asset = registry.get(&info.id).unwrap();
    assert_eq!(asset.bytes.as_ref(), bytes.as_slice());
    assert_eq!(registry.register(bytes).unwrap(), info);
    assert!(std::sync::Arc::ptr_eq(
        &asset,
        &registry.get(&info.id).unwrap()
    ));
    asset.validate_text("Café\nΩ Ж").unwrap();
    assert!(asset.glyph_id('Ω').unwrap() > 0);
    assert!(asset.advance('W').unwrap() > asset.advance('i').unwrap());
    assert!(asset.advance('W').unwrap() < 2.0);
    for character in ['é', 'Ω', 'Ж'] {
        assert!(info
            .coverage
            .iter()
            .any(|range| range[0] <= character as u32 && character as u32 <= range[1]));
    }
    for text in [
        "e\u{301}",
        "مرحبا",
        "\u{202e}",
        "😀",
        "\t",
        "\u{0378}",
        "\u{ad}",
        "\u{a670}",
    ] {
        assert!(asset.validate_text(text).is_err(), "{text:?}");
    }
    registry.remove(&info.id).unwrap();
    registry.remove(&info.id).unwrap();
    assert!(registry.get(&info.id).is_err());
    assert_eq!(asset.info, info); // Existing operation keeps its immutable asset.
}

#[test]
fn rejects_malformed_collections_and_unsafe_embedding_permissions() {
    let registry = FontRegistry::new();
    for bytes in [
        vec![],
        b"not a font".to_vec(),
        b"ttcf\0\x01\0\0".to_vec(),
        font()[..128].to_vec(),
    ] {
        assert!(registry.register(bytes).is_err());
    }
    for flags in [0x0002, 0x0004, 0x0006, 0x000c, 0x0200, 0x0208, 0x0010] {
        let mut bytes = font();
        set_permissions(&mut bytes, flags);
        let error = registry.register(bytes).unwrap_err().to_string();
        assert!(error.contains("embedding"), "{flags:x}: {error}");
    }
    for flags in [0, 8, 0x0100, 0x0108] {
        let mut bytes = font();
        set_permissions(&mut bytes, flags);
        let asset = FontAsset::parse(bytes.clone()).unwrap();
        assert_eq!(asset.bytes.as_ref(), bytes.as_slice()); // No-subsetting preserves entire program.
    }
}

#[test]
fn explains_unsupported_outline_variable_and_color_fonts_and_reports_style() {
    for (tag, reason) in [(b"CFF ", "CFF"), (b"fvar", "Variable"), (b"COLR", "Color")] {
        let mut bytes = font();
        let count = u16::from_be_bytes(bytes[4..6].try_into().unwrap()) as usize;
        let mut records: Vec<[u8; 16]> = bytes[12..12 + count * 16]
            .chunks_exact(16)
            .map(|record| record.try_into().unwrap())
            .collect();
        let optional = records
            .iter_mut()
            .find(|record| {
                matches!(
                    &record[..4],
                    b"DSIG" | b"GPOS" | b"GSUB" | b"GDEF" | b"kern"
                )
            })
            .unwrap();
        optional[..4].copy_from_slice(tag);
        records.sort_by_key(|record| [record[0], record[1], record[2], record[3]]);
        for (index, record) in records.iter().enumerate() {
            bytes[12 + index * 16..28 + index * 16].copy_from_slice(record);
        }
        let error = FontAsset::parse(bytes).unwrap_err().to_string();
        assert!(error.contains(reason), "{error}");
    }
    let path =
        std::path::PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| "C:/Windows".into()))
            .join("Fonts/arialbi.ttf");
    let asset = FontAsset::parse(std::fs::read(path).unwrap()).unwrap();
    assert_eq!(asset.info.weight, 700);
    assert!(asset.info.italic);
}

#[test]
fn caps_individual_font_size_count_and_total_bytes_without_losing_existing_assets() {
    let registry = FontRegistry::new();
    assert!(registry
        .register(vec![0; 16 * 1024 * 1024 + 1])
        .unwrap_err()
        .to_string()
        .contains("16 MiB"));
    let bytes = font();
    let mut first = None;
    for index in 0..64_u8 {
        let mut unique = bytes.clone();
        unique.push(index);
        let info = registry.register(unique).unwrap();
        first.get_or_insert(info);
    }
    let mut overflow = bytes.clone();
    overflow.push(64);
    assert!(registry
        .register(overflow)
        .unwrap_err()
        .to_string()
        .contains("64"));
    let mut duplicate = bytes;
    duplicate.push(0);
    assert_eq!(
        registry.register(duplicate).unwrap(),
        first.clone().unwrap()
    );
    registry.remove(&first.unwrap().id).unwrap();
    let registry = FontRegistry::new();
    let mut bytes = font();
    bytes.resize(16 * 1024 * 1024, 0);
    let mut ids = Vec::new();
    for index in 0..8_u8 {
        *bytes.last_mut().unwrap() = index;
        ids.push(registry.register(bytes.clone()).unwrap().id);
    }
    *bytes.last_mut().unwrap() = 8;
    assert!(registry
        .register(bytes.clone())
        .unwrap_err()
        .to_string()
        .contains("128 MiB"));
    assert!(ids.iter().all(|id| registry.get(id).is_ok()));
    registry.remove(&ids[0]).unwrap();
    registry.register(bytes).unwrap();
}

#[test]
fn installed_catalog_uses_bound_opaque_ids_and_loads_only_selected_fonts() {
    let registry = FontRegistry::new();
    let catalog = registry.list_installed().unwrap();
    assert!(!catalog.is_empty());
    assert!(registry.ids().unwrap().is_empty());
    let entry = catalog
        .iter()
        .find(|entry| entry.supported && entry.name == "Arial")
        .unwrap();
    assert!(!entry.id.contains(':') && !entry.id.contains('\\'));
    assert!(registry.get(&entry.id).is_err());
    let info = registry.load_installed(&entry.id).unwrap();
    assert_eq!(info.name, "Arial");
    assert!(registry.get(&info.id).is_ok());
    assert!(registry
        .load_installed("C:/Windows/Fonts/arial.ttf")
        .is_err());
    assert!(catalog
        .iter()
        .all(|entry| entry.supported == entry.reason.is_none()));
}
