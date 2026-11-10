use super::*;

#[path = "../tests/support/affine_forms.rs"]
#[allow(dead_code)]
mod affine_forms;

#[test]
fn inherited_form_paragraph_promotion_preserves_pixels_and_complete_glyphs() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("inherited.pdf");
    affine_forms::fixture(&input, "standard", "plain", 0, "Cafe old");
    let original_bytes = std::fs::read(&input).unwrap();
    let renderer = std::env::var_os("FOLIO_PARAGRAPH_PROJECTION_DLL").map(PathBuf::from)
        .unwrap_or_else(||PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"));
    let mut rt = WorkerRuntime::new(
        &renderer,
        Arc::new(FontRegistry::new()),
    ).unwrap();
    let source = rt.register_generated_document(original_bytes.clone(), "Inherited.pdf").unwrap();
    rt.with_text_projection(&source.id, 0, |rt, baseline_id| {
        let source = rt.editing_document(baseline_id)?;
        let model = Model::read_mode(source.bytes().unwrap().as_ref(), true)?;
        let selected = model.occurrences.iter().find(|o| o.path.len() > 1).unwrap();
        let promoted = model.promote_paragraph(&selected.path)?;
        let document = rt.pdfium.load_pdf_from_byte_slice(&promoted.bytes, None)?;
        let before = source.pdf().unwrap().pages().get(0)?;
        let after = document.pages().get(0)?;
        let a = page_geometry(&before)?;
        let b = page_geometry(&after)?;
        let geometry_equal = [a.left,a.bottom,a.right,a.top] == [b.left,b.bottom,b.right,b.top] && a.rotation == b.rotation;
        let pixels_a = raster(&before)?;
        let pixels_b = raster(&after)?;
        let glyphs_a = glyph_snapshot(&before)?;
        let glyphs_b = glyph_snapshot(&after)?;
        let physical_equal = same_glyphs(&physical_glyphs(&before)?, &physical_glyphs(&after)?);
        eprintln!("geometry={geometry_equal} pixels={} physical={physical_equal} complete={} glyph_counts=({}, {}) first_difference={:?}", pixels_a==pixels_b, same_glyphs(&glyphs_a,&glyphs_b), glyphs_a.len(), glyphs_b.len(), glyphs_a.iter().zip(&glyphs_b).enumerate().find(|(_, (a,b))| a!=b));
        if let Some(output) = std::env::var_os("FOLIO_PARAGRAPH_PROJECTION_DIAGNOSTIC") {
            let output = PathBuf::from(output);
            std::fs::create_dir_all(&output).unwrap();
            std::fs::write(output.join("source.pdf"), source.bytes().unwrap().as_ref()).unwrap();
            std::fs::write(output.join("promoted.pdf"), &promoted.bytes).unwrap();
            pixels_a.save(output.join("source.png")).unwrap();
            pixels_b.save(output.join("promoted.png")).unwrap();
            std::fs::write(output.join("glyphs.txt"), format!("BEFORE\n{glyphs_a:#?}\nAFTER\n{glyphs_b:#?}")).unwrap();
        }
        assert!(geometry_equal, "promotion changed page geometry");
        assert!(pixels_a == pixels_b, "promotion changed page pixels");
        assert!(physical_equal, "promotion changed actual glyphs");
        assert!(same_glyphs(&glyphs_a,&glyphs_b), "promotion changed the complete extracted glyph sequence");
        Ok(())
    }).unwrap();
    assert_eq!(std::fs::read(&input).unwrap(), original_bytes);
}
