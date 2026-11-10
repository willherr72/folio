//! Nominal Latin Type1 mappings. Authority comes from the selected resource,
//! never a renderer font name shared by potentially different encodings.
use super::*;
use lopdf::{Dictionary, Document};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Encoding { Standard, WinAnsi, MacRoman }

#[derive(Debug, Clone, PartialEq)]
pub(super) struct StandardFont {
    font: TextFont,
    encoding: Encoding,
}

impl StandardFont {
    pub(super) fn read(pdf: &Document, resource: &Dictionary) -> EngineResult<Self> {
        let bad = || invalid("The standard font has no proved nominal encoding and metrics.");
        let name = |key| resource.get(key).ok().and_then(|v| pdf.dereference(v).ok())
            .and_then(|(_, v)| v.as_name().ok());
        if name(b"Subtype") != Some(b"Type1".as_slice())
            || [b"Widths".as_slice(), b"FirstChar", b"LastChar", b"ToUnicode", b"FontDescriptor"]
                .iter().any(|key| resource.has(key)) {
            return Err(bad());
        }
        let base = std::str::from_utf8(name(b"BaseFont").ok_or_else(bad)?).map_err(|_| bad())?;
        if !base_font(base) { return Err(bad()); }
        let font = serde_json::from_value(serde_json::Value::String(base.to_owned())).map_err(|_| bad())?;
        let encoding = match resource.get(b"Encoding") {
            Ok(value) => match equivalent_encoding::name(pdf, value) {
                Some(b"StandardEncoding") => Encoding::Standard,
                Some(b"WinAnsiEncoding") => Encoding::WinAnsi,
                Some(b"MacRomanEncoding") => Encoding::MacRoman,
                _ => return Err(bad()),
            },
            Err(_) => Encoding::Standard,
        };
        Ok(Self { font, encoding })
    }

    fn table(&self) -> &'static [Option<char>; 256] {
        use std::sync::OnceLock;
        static STANDARD: OnceLock<[Option<char>; 256]> = OnceLock::new();
        static WIN_ANSI: OnceLock<[Option<char>; 256]> = OnceLock::new();
        static MAC_ROMAN: OnceLock<[Option<char>; 256]> = OnceLock::new();
        let (cache, name) = match self.encoding {
            Encoding::Standard => (&STANDARD, "StandardEncoding"),
            Encoding::WinAnsi => (&WIN_ANSI, "WinAnsiEncoding"),
            Encoding::MacRoman => (&MAC_ROMAN, "MacRomanEncoding"),
        };
        cache.get_or_init(|| {
            // Only fixed trusted dictionaries reach the dependency's public
            // decoder. Never let its permissive fallback interpret user input.
            let mut nominal = Dictionary::new();
            nominal.set("Type", "Font"); nominal.set("Encoding", name);
            let pdf = Document::new();
            match nominal.get_font_encoding(&pdf) {
                Ok(lopdf::Encoding::OneByteEncoding(table)) => table.map(|g|
                    g.and_then(|g| char::from_u32(u32::from(g.utf16_code_unit())))),
                _ => [None; 256],
            }
        })
    }

    fn character(&self, code: u8) -> EngineResult<char> {
        let c = self.table()[usize::from(code)]
            .ok_or_else(|| invalid("The standard font character code is undefined."))?;
        if !embedded_font::latin(&c.to_string()) && c != ' ' {
            return Err(invalid("The standard font character is outside the supported Latin repertoire."));
        }
        self.width(c)?;
        Ok(c)
    }

    fn code(&self, c: char) -> EngineResult<u8> {
        // WinAnsi has alternate bullet/space slots. New text uses the nominal
        // code; source admission separately requires exact byte roundtrip.
        let preferred = if c == ' ' { Some(32) }
            else if c == '\u{2022}' && self.encoding == Encoding::WinAnsi { Some(149) }
            else { None };
        let code = preferred.or_else(|| self.table().iter().position(|g| *g == Some(c)).map(|i| i as u8))
            .ok_or_else(|| invalid("The standard font cannot encode this character. Choose an explicit substitute font."))?;
        if self.character(code)? != c {
            return Err(invalid("The standard font character has no exact nominal mapping."));
        }
        Ok(code)
    }

    pub(super) fn encode(&self, value: &str) -> EngineResult<Vec<u8>> {
        if value.len() > 4096 { return Err(invalid("The standard font text exceeds 4096 UTF-8 bytes.")); }
        value.chars().map(|c| self.code(c)).collect()
    }

    pub(super) fn decode(&self, bytes: &[u8]) -> EngineResult<String> {
        if bytes.len() > 4096 { return Err(invalid("The standard font operand exceeds the text limit.")); }
        let mut text = String::new();
        for &code in bytes {
            let c = self.character(code)?;
            if text.len() + c.len_utf8() > 4096 {
                return Err(invalid("The standard font text exceeds 4096 UTF-8 bytes."));
            }
            text.push(c);
        }
        Ok(text)
    }

    pub(super) fn advance(&self, value: &str) -> EngineResult<f32> {
        let encoded = self.encode(value)?;
        // Preserve the existing exact Courier accumulation policy. Repeatedly
        // adding 0.6 rounds differently and can shift a following show enough
        // to change antialias pixels after paragraph cursor restoration.
        if matches!(self.font, TextFont::Courier | TextFont::CourierBold
            | TextFont::CourierOblique | TextFont::CourierBoldOblique) {
            return Ok(encoded.len() as f32 * 0.6);
        }
        value.chars().try_fold(0., |sum, c| self.width(c).map(|width| sum + width))
    }

    fn width(&self, c: char) -> EngineResult<f32> {
        // Additional Standard/MacRoman glyphs absent from WinAnsi. Adapted from
        // ReportLab 4.5.1 under the existing notice/license in THIRD_PARTY_NOTICES.md.
        // All widths are in 1000 em; oblique Helvetica shares upright advances.
        use TextFont::*;
        let courier = matches!(self.font, Courier | CourierBold | CourierOblique | CourierBoldOblique);
        let extra = match c {
            '\u{0131}' => Some(278),
            '\u{0141}' => Some(match self.font {
                Helvetica | HelveticaOblique | TimesItalic => 556,
                TimesBold => 667,
                _ => 611,
            }),
            '\u{0142}' => Some(if matches!(self.font, Helvetica | HelveticaOblique) {222} else {278}),
            '\u{02c7}' | '\u{02d8}' | '\u{02d9}' | '\u{02da}' | '\u{02db}' | '\u{02dd}' => Some(333),
            '\u{2044}' => Some(167),
            _ => None,
        };
        if let Some(width) = extra { return Ok(if courier {0.6} else {width as f32 / 1000.}); }
        persistence::text_advance(&c.to_string(), self.font)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Object};
    fn resource(base: &str, encoding: &str) -> Dictionary {
        dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>base,"Encoding"=>encoding}
    }
    #[test]
    fn encodings_distinguish_identically_named_fonts_and_roundtrip_accented_bytes() {
        let pdf=Document::new();
        let win=StandardFont::read(&pdf,&resource("Helvetica","WinAnsiEncoding")).unwrap();
        let mac=StandardFont::read(&pdf,&resource("Helvetica","MacRomanEncoding")).unwrap();
        assert_ne!(win,mac);
        assert_eq!(win.encode("Résumé café €—").unwrap(),b"R\xe9sum\xe9 caf\xe9 \x80\x97");
        assert_eq!(mac.encode("Résumé café").unwrap(),b"R\x8esum\x8e caf\x8e");
        assert_eq!(win.decode(b"R\xe9sum\xe9").unwrap(),"Résumé");
        assert_eq!(mac.decode(b"R\x8esum\x8e").unwrap(),"Résumé");
        assert!((win.advance("Résumé café").unwrap()-mac.advance("Résumé café").unwrap()).abs()<0.00001);
        assert!(win.encode("漢").is_err());assert!(win.decode(&[0]).is_err());
    }
    #[test]
    fn standard_quotes_and_additional_nominal_metrics_are_explicit() {
        let pdf=Document::new();
        for base in ["Helvetica","Helvetica-Bold","Helvetica-Oblique","Helvetica-BoldOblique",
            "Times-Roman","Times-Bold","Times-Italic","Times-BoldItalic",
            "Courier","Courier-Bold","Courier-Oblique","Courier-BoldOblique"] {
            let font=StandardFont::read(&pdf,&resource(base,"StandardEncoding")).unwrap();
            assert_eq!(font.decode(&[39,96]).unwrap(),"’‘");
            assert_eq!(font.encode("’‘").unwrap(),vec![39,96]);
            for c in ['ı','Ł','ł','ˇ','˘','˙','˚','˛','˝','⁄'] {
                let value=c.to_string();assert_eq!(font.decode(&font.encode(&value).unwrap()).unwrap(),value);
                assert!(font.advance(&value).unwrap()>0.);
            }
            assert!(font.encode("é").is_err());
        }
    }
    #[test]
    fn undefined_alias_and_oversized_operands_never_silently_roundtrip() {
        let font=StandardFont::read(&Document::new(),&resource("Helvetica","WinAnsiEncoding")).unwrap();
        assert_eq!(font.encode("• ").unwrap(),vec![149,32]);
        // Whether an alternate code is defined or rejected, it cannot prove
        // itself by silently adopting a different source byte.
        for code in [127,129,141,143,144,157,160] {
            if let Ok(text)=font.decode(&[code]) {assert_ne!(font.encode(&text).unwrap(),vec![code]);}
        }
        assert!(font.encode(&"é".repeat(2049)).is_err());
        assert!(font.decode(&vec![233;2049]).is_err());
        assert!(font.decode(&vec![32;4097]).is_err());
    }
    #[test]
    fn override_resources_do_not_inherit_nominal_authority() {
        let mut pdf=Document::new();
        for field in ["Widths","FirstChar","LastChar","ToUnicode","FontDescriptor"] {
            let mut font=resource("Helvetica","WinAnsiEncoding");font.set(field,Object::Null);
            assert!(StandardFont::read(&pdf,&font).is_err(),"{field}");
        }
        let base=pdf.add_object(Object::Name(b"WinAnsiEncoding".to_vec()));
        let encoding=pdf.add_object(dictionary!{"BaseEncoding"=>base,"Differences"=>vec![65.into(),Object::Name(b"A".to_vec())]});
        let mut font=resource("Helvetica","WinAnsiEncoding");font.set("Encoding",encoding);
        assert!(StandardFont::read(&pdf,&font).is_ok());
        font.set("Encoding",dictionary!{"BaseEncoding"=>"WinAnsiEncoding","Differences"=>vec![65.into(),Object::Name(b"B".to_vec())]});
        assert!(StandardFont::read(&pdf,&font).is_err());
        for base in ["Arial","ABCDEF+Helvetica","Symbol","ZapfDingbats"] {
            assert!(StandardFont::read(&pdf,&resource(base,"WinAnsiEncoding")).is_err());
        }
    }
}
