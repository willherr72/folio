//! Measurement-only Unicode mapping; never returned or published as an edit.
use super::{invalid, parse, require, EngineResult, MAX_BYTES};
use lopdf::{dictionary, Document, Object, Stream};
use std::io::{self, Write};

// Only the already admitted twelve unembedded standard Latin fonts qualify.
// Their rendering encoding is independent of ToUnicode. Embedded fonts and
// custom encodings remain unsupported here rather than guessing glyph identity.
pub(super) fn build(source: &Document, page_index: usize) -> EngineResult<Vec<u8>> {
    build_with_context(source, page_index, &mut Default::default())
}
pub(super) fn build_with_context(
    source: &Document,
    page_index: usize,
    images: &mut crate::redaction_images::ImageContext,
) -> EngineResult<Vec<u8>> {
    let mut prepared = crate::redaction_core::prepare_area_with_context(source, images)?;
    require(page_index < prepared.pages.len())?;
    let page = prepared.pages.remove(page_index);
    require(
        page.fonts
            .iter()
            .all(|font| font.supports_space_measurement_probe()),
    )?;
    let bytes = crate::redaction_core::publish(vec![page])?;
    let mut probe = parse(&bytes)?;
    let page_id = *probe.get_pages().values().next().ok_or_else(invalid)?;
    let fonts: Vec<_> = probe
        .get_object(page_id)
        .and_then(Object::as_dict)
        .and_then(|d| d.get(b"Resources"))
        .and_then(Object::as_dict)
        .and_then(|d| d.get(b"Font"))
        .and_then(Object::as_dict)
        .map_err(|_| invalid())?
        .iter()
        .map(|(_, f)| f.as_reference().map_err(|_| invalid()))
        .collect::<EngineResult<_>>()?;
    let mapping = probe.add_object(Stream::new(dictionary!{},
        b"/CIDInit /ProcSet findresource begin 12 dict begin begincmap /CIDSystemInfo << /Registry (FolioProbe) /Ordering (Space) /Supplement 0 >> def /CMapName /FolioSpace def /CMapType 2 def 1 begincodespacerange <00> <FF> endcodespacerange 1 beginbfchar <20> <E000> endbfchar endcmap CMapName currentdict /CMap defineresource pop end end".to_vec()));
    for font in fonts {
        let dictionary = probe
            .get_object_mut(font)
            .and_then(Object::as_dict_mut)
            .map_err(|_| invalid())?;
        require(!dictionary.has(b"ToUnicode"))?;
        dictionary.set("ToUnicode", mapping);
    }
    struct Bounded(Vec<u8>);
    impl Write for Bounded {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes.len() > MAX_BYTES.saturating_sub(self.0.len()) {
                return Err(io::Error::other(
                    "Space measurement probe exceeds byte limit",
                ));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut out = Bounded(vec![]);
    probe.save_to(&mut out).map_err(|_| invalid())?;
    Ok(out.0)
}
