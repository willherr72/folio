use lopdf::{dictionary, Document, Object, Stream};
use std::path::{Path, PathBuf};

fn asset(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/embedded-text")
        .join(name)
}
pub fn encoded(text: &str, cid: bool) -> Vec<u8> {
    if cid {
        text.encode_utf16().flat_map(u16::to_be_bytes).collect()
    } else {
        text.chars()
            .map(|c| {
                if c == 'é' {
                    1
                } else {
                    u8::try_from(c as u32).unwrap()
                }
            })
            .collect()
    }
}
fn font(cid: bool) -> (Document, lopdf::ObjectId) {
    if !cid {
        let pdf = Document::load(asset("reportlab-subset.pdf")).unwrap();
        let id = pdf
            .objects
            .iter()
            .find_map(|(id, o)| {
                o.as_dict()
                    .ok()
                    .filter(|d| {
                        d.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"TrueType")
                    })
                    .map(|_| *id)
            })
            .unwrap();
        return (pdf, id);
    }
    let bytes = std::fs::read(asset("DejaVuSerif-subset.ttf")).unwrap();
    let face = ttf_parser::Face::parse(&bytes, 0).unwrap();
    let mut chars: Vec<_> = "Old Cafe Edit café Neighbor safe ".chars().collect();
    chars.sort();
    chars.dedup();
    let mut map = vec![0u8; 512];
    let mut widths = vec![];
    let mut unicode=String::from("/CIDInit /ProcSet findresource begin 12 dict begin begincmap /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def /CMapName /Fixture def /CMapType 2 def 1 begincodespacerange <0000> <FFFF> endcodespacerange\n");
    unicode.push_str(&format!("{} beginbfchar\n", chars.len()));
    for c in chars {
        let gid = face.glyph_index(c).unwrap();
        let code = c as usize;
        map[code * 2..code * 2 + 2].copy_from_slice(&gid.0.to_be_bytes());
        widths.extend([
            Object::Integer(code as i64),
            Object::Array(vec![Object::Real(
                face.glyph_hor_advance(gid).unwrap() as f32 * 1000. / face.units_per_em() as f32,
            )]),
        ]);
        unicode.push_str(&format!("<{code:04X}> <{code:04X}>\n"));
    }
    unicode.push_str("endbfchar endcmap CMapName currentdict /CMap defineresource pop end end");
    let mut pdf = Document::with_version("1.7");
    let file = pdf.add_object(Stream::new(
        dictionary! {"Length1"=>bytes.len() as i64},
        bytes,
    ));
    let descriptor=pdf.add_object(dictionary!{"Type"=>"FontDescriptor","FontName"=>"ABCDEF+DejaVuSerif","Flags"=>32,"FontBBox"=>vec![(-1000).into(),(-1000).into(),2000.into(),2000.into()],"ItalicAngle"=>0,"Ascent"=>1000,"Descent"=>-300,"CapHeight"=>800,"StemV"=>80,"FontFile2"=>file});
    let map = pdf.add_object(Stream::new(dictionary! {}, map));
    let unicode = pdf.add_object(Stream::new(dictionary! {}, unicode.into_bytes()));
    let descendant=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"CIDFontType2","BaseFont"=>"ABCDEF+DejaVuSerif","CIDSystemInfo"=>dictionary!{"Registry"=>Object::string_literal("Adobe"),"Ordering"=>Object::string_literal("Identity"),"Supplement"=>0},"FontDescriptor"=>descriptor,"DW"=>0,"W"=>widths,"CIDToGIDMap"=>map});
    let id=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type0","BaseFont"=>"ABCDEF+DejaVuSerif","Encoding"=>"Identity-H","DescendantFonts"=>vec![descendant.into()],"ToUnicode"=>unicode});
    (pdf, id)
}
pub fn fixture(path: &Path, cid: bool, rotation: i32, inherited: bool, first: &str) {
    let (mut pdf, font) = font(cid);
    let pages = pdf.new_object_id();
    let resources = dictionary! {"Font"=>dictionary!{"F1"=>font}};
    let mut leaf = |text: &str| {
        let data = lopdf::content::Content {
            operations: vec![
                lopdf::content::Operation::new("BT", vec![]),
                lopdf::content::Operation::new("Tf", vec![Object::Name(b"F1".to_vec()), 16.into()]),
                lopdf::content::Operation::new("rg", vec![0.2.into(), 0.4.into(), 0.6.into()]),
                lopdf::content::Operation::new(
                    "Tm",
                    vec![1.into(), 0.into(), 0.into(), 1.into(), 5.into(), 10.into()],
                ),
                lopdf::content::Operation::new(
                    "Tj",
                    vec![Object::String(
                        encoded(text, cid),
                        lopdf::StringFormat::Hexadecimal,
                    )],
                ),
                lopdf::content::Operation::new("ET", vec![]),
            ],
        }
        .encode()
        .unwrap();
        let mut dict = dictionary! {"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),240.into(),40.into()]};
        if !inherited {
            dict.set("Resources", resources.clone());
        }
        pdf.add_object(Stream::new(dict, data))
    };
    let original = leaf("Café old");
    let changed = if first == "Café old" {
        original
    } else {
        leaf(first)
    };
    let mut wrapper = |child| {
        pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),250.into(),50.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font},"XObject"=>dictionary!{"Leaf"=>child}}},b"q 1 0 0 1 3 4 cm /Leaf Do Q".to_vec()))
    };
    let shared = wrapper(original);
    let first = if original == changed {
        shared
    } else {
        wrapper(changed)
    };
    let neighbor =
        pdf.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let mut kids = vec![];
    for i in 0..2 {
        let data = if i == 0 {
            b"BT /N 12 Tf 1 0 0 1 20 80 Tm (Neighbor) Tj ET q 1 0 0 1 20 280 cm /First Do Q q 0.8 0 0 0.8 20 180 cm /Shared Do Q".as_slice()
        } else {
            b"q 1 0 0 1 20 280 cm /Shared Do Q".as_slice()
        };
        let content = pdf.add_object(Stream::new(dictionary! {}, data.to_vec()));
        let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),400.into(),400.into()],"Rotate"=>rotation,"Resources"=>dictionary!{"Font"=>dictionary!{"N"=>neighbor,"F1"=>font},"XObject"=>dictionary!{"First"=>first,"Shared"=>shared}},"Contents"=>content});
        kids.push(page.into());
    }
    pdf.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Kids"=>kids,"Count"=>2}.into(),
    );
    let catalog = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    pdf.trailer.set("Root", catalog);
    pdf.save(path).unwrap();
}
pub fn programs(bytes: &[u8]) -> Vec<Vec<u8>> {
    let pdf = Document::load_mem(bytes).unwrap();
    let mut values = vec![];
    for object in pdf.objects.values() {
        if let Ok(dict) = object.as_dict() {
            if let Ok(file) = dict.get(b"FontFile2") {
                let stream = pdf.dereference(file).unwrap().1.as_stream().unwrap();
                values.push(stream.get_plain_content().unwrap());
            }
        }
    }
    values.sort();
    values.dedup();
    values
}
