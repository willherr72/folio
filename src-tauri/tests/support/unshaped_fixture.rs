use folio_engine::{DocumentInfo, ExportRequest, PagePlan, PdfEngine};
use lopdf::{content::{Content, Operation}, dictionary, Document, Object, Stream};
use std::{fs, path::{Path, PathBuf}};

pub const FONT: &[u8] = include_bytes!("../../../tests/fixtures/shaped-text/DejaVuSans.ttf");

pub fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()
}

pub fn fixture(path: &Path, values: &[&str], rotation: u16, unit: f32, bad_width: bool) {
    fixture_with_extra(path,values,rotation,unit,bad_width,"");
}

pub fn fixture_with_extra(path: &Path, values: &[&str], rotation: u16, unit: f32, bad_width: bool, extra:&str) {
    let face=ttf_parser::Face::parse(FONT,0).unwrap();
    let mut chars:Vec<_>=values.iter().flat_map(|s|s.chars()).chain("Neighbor safe".chars()).chain(extra.chars()).collect();
    chars.sort_unstable();chars.dedup();
    let mut widths=vec![];let mut mappings=String::new();
    for ch in &chars {
        let gid=face.glyph_index(*ch).expect("Fixture character must exist in the real font");
        let mut width=face.glyph_hor_advance(gid).unwrap()as f32*1000./face.units_per_em()as f32;
        if bad_width&&*ch=='Δ'{width+=50.;}
        widths.extend([Object::Integer(gid.0 as i64),Object::Array(vec![width.into()])]);
        mappings.push_str(&format!("<{:04X}> <{:04X}>\n",gid.0,*ch as u32));
    }
    let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
    let program=pdf.add_object(Stream::new(dictionary!{"Length1"=>FONT.len()as i64},FONT.to_vec()));
    let descriptor=pdf.add_object(dictionary!{"Type"=>"FontDescriptor","FontName"=>"ABCDEF+DejaVuSans","Flags"=>32,
        "FontBBox"=>vec![(-1100).into(),(-1100).into(),2100.into(),2100.into()],"ItalicAngle"=>0,
        "Ascent"=>1000,"Descent"=>-300,"CapHeight"=>800,"StemV"=>80,"FontFile2"=>program});
    let child=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"CIDFontType2","BaseFont"=>"ABCDEF+DejaVuSans",
        "CIDSystemInfo"=>dictionary!{"Registry"=>Object::string_literal("Adobe"),"Ordering"=>Object::string_literal("Identity"),"Supplement"=>0},
        "FontDescriptor"=>descriptor,"DW"=>0,"W"=>widths,"CIDToGIDMap"=>"Identity"});
    let cmap=pdf.add_object(Stream::new(dictionary!{},format!("1 begincodespacerange <0000> <FFFF> endcodespacerange\n{} beginbfchar\n{mappings}endbfchar",chars.len()).into_bytes()));
    let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type0","BaseFont"=>"ABCDEF+DejaVuSans",
        "Encoding"=>"Identity-H","DescendantFonts"=>vec![child.into()],"ToUnicode"=>cmap});
    let encode=|text:&str|text.chars().flat_map(|ch|face.glyph_index(ch).unwrap().0.to_be_bytes()).collect::<Vec<_>>();
    let mut operations=vec![];
    for(text,y)in[(values[0],300),("Neighbor safe",150)] {
        operations.extend([Operation::new("BT",vec![]),Operation::new("Tf",vec![Object::Name(b"F1".to_vec()),14.into()]),
            Operation::new("rg",vec![0.1.into(),0.2.into(),0.3.into()]),Operation::new("Td",vec![40.into(),y.into()]),
            Operation::new("Tj",vec![Object::string_literal(encode(text))]),Operation::new("ET",vec![])]);
    }
    let content=pdf.add_object(Stream::new(dictionary!{},Content{operations}.encode().unwrap()));
    let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),400.into(),400.into()],
        "Rotate"=>rotation as i64,"UserUnit"=>unit,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>content});
    pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());
    let catalog=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",catalog);
    pdf.save(path).unwrap();
}

pub fn font_streams(path:&Path)->Vec<(String,Vec<u8>)> {
    let pdf=Document::load(path).unwrap();let mut values=vec![];
    for d in pdf.objects.values().filter_map(|o|o.as_dict().ok()) {
        for key in [b"ToUnicode".as_slice(),b"FontFile2"] {
            if let Ok(value)=d.get(key) {
                let stream=pdf.dereference(value).unwrap().1.as_stream().unwrap();
                values.push((String::from_utf8(key.to_vec()).unwrap(),stream.get_plain_content().unwrap()));
            }
        }
    }
    values.sort();values.dedup();values
}

pub fn export(engine:&PdfEngine,doc:&DocumentInfo,path:&Path) {
    engine.export_pdf(ExportRequest{flatten:false,pages:vec![PagePlan{id:"page".into(),source_id:doc.id.clone(),page_index:0,
        width:doc.pages[0].width,height:doc.pages[0].height,rotation:0,crop:None,overlays:vec![]}]},path).unwrap();
}

pub fn capture(label:&str,path:&Path) {
    if let Some(root)=std::env::var_os("FOLIO_UNSHAPED_OUTPUTS") {
        let folder=PathBuf::from(root).join(label);fs::create_dir_all(&folder).unwrap();
        fs::copy(path,folder.join(path.file_name().unwrap())).unwrap();
    }
}

