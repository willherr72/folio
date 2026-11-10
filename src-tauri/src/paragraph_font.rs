//! Explicit paragraph substitutes share the source-text full-font PDF format.
use super::*;

pub(super) fn install(
    pdf: &mut Document,
    page_id: lopdf::ObjectId,
    resources: &lopdf::Dictionary,
    asset: &FontAsset,
) -> EngineResult<Vec<u8>> {
    install_with_policy(pdf,page_id,resources,asset,false)
}

pub(super) fn install_integral(pdf:&mut Document,page_id:lopdf::ObjectId,resources:&lopdf::Dictionary,asset:&FontAsset)->EngineResult<Vec<u8>> {
    install_with_policy(pdf,page_id,resources,asset,true)
}

pub(super) fn integral_advance(asset:&FontAsset,ch:char)->EngineResult<f32> {
    Ok((asset.advance(ch)?*1000.).round()/1000.)
}

fn install_with_policy(pdf:&mut Document,page_id:lopdf::ObjectId,resources:&lopdf::Dictionary,asset:&FontAsset,integral:bool)->EngineResult<Vec<u8>> {
    let mut resources=resources.clone();
    let mut fonts=pdf.dereference(resources.get(b"Font").map_err(|_|invalid("Missing paragraph fonts."))?)
        .and_then(|(_,value)|value.as_dict()).map_err(|_|invalid("Malformed paragraph fonts."))?.clone();
    let mut name=b"FolioParagraph".to_vec();
    while fonts.has(&name) { name.push(b'_'); }
    let font=if integral {persistence::install_paragraph_font_resource(pdf,asset)?}
        else {persistence::install_font_resource(pdf,asset)?};
    fonts.set(name.clone(),font);
    resources.set("Font",fonts);
    pdf.get_dictionary_mut(page_id).map_err(|_|invalid("Missing paragraph page."))?.set("Resources",resources);
    Ok(name)
}

pub(super) fn metrics(asset: &FontAsset, text: &str, size: f32) -> EngineResult<HashMap<char,[f32;3]>> {
    if text.len()>4096 { return Err(invalid("Use at most 4096 UTF-8 bytes for a paragraph.")); }
    asset.validate_text(text)?;
    let face=asset.face()?;let scale=size/face.units_per_em() as f32;
    let mut metrics=HashMap::new();
    for c in text.chars().filter(|c|*c!='\n') {
        if metrics.contains_key(&c) { continue; }
        let glyph=asset.glyph_id(c)?;
        let ink=face.glyph_bounding_box(ttf_parser::GlyphId(glyph));
        let value=[asset.advance(c)?*size,ink.map_or(0.,|b|b.x_min as f32*scale),ink.map_or(0.,|b|b.x_max as f32*scale)];
        if value.iter().any(|v|!v.is_finite()||v.abs()>100_000.) { return Err(invalid("The paragraph glyph metrics exceed the limit.")); }
        metrics.insert(c,value);
    }
    Ok(metrics)
}

pub(super) fn encode(text: &str) -> Vec<u8> {
    // Registry validation accepts only unshaped BMP characters; CIDs are Unicode.
    text.encode_utf16().flat_map(u16::to_be_bytes).collect()
}
