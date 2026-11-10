use lopdf::{dictionary, Document, Object, Stream};
use std::path::Path;

#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum Mode {
    RepeatedOuter,
    RepeatedInnerClose,
    RepeatedInnerFar,
}

pub fn write_fixture(path: &Path, mode: Mode, user_unit: f32) {
    let mut pdf = Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let font =
        pdf.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let leaf = pdf.add_object(Stream::new(dictionary! {
        "Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),220.into(),70.into()],
        "Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}
    },b"BT /F1 12 Tf 5 40 Td (Nested first line) Tj 0 -16 Td (Nested second line) Tj ET".to_vec()));
    let program = match mode {
        Mode::RepeatedOuter => b"q /Inner Do Q".to_vec(),
        Mode::RepeatedInnerClose => b"q /Inner Do Q q 1 0 0 1 0 -32 cm /Inner Do Q".to_vec(),
        Mode::RepeatedInnerFar => b"q /Inner Do Q q 1 0 0 1 0 -120 cm /Inner Do Q".to_vec(),
    };
    let outer = pdf.add_object(Stream::new(dictionary! {
        "Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),(-120).into(),220.into(),70.into()],
        "Resources"=>dictionary!{"XObject"=>dictionary!{"Inner"=>leaf}}
    },program));
    let mut kids = Vec::<Object>::new();
    for index in 0..3 {
        let program = if index == 0 {
            b"BT /F1 14 Tf 40 350 Td (Page heading) Tj ET q 1 0 0 1 40 230 cm /Reusable Do Q q 1 0 0 1 330 230 cm /Reusable Do Q".to_vec()
        } else {
            b"q 1 0 0 1 40 230 cm /Reusable Do Q".to_vec()
        };
        let content = pdf.add_object(Stream::new(dictionary! {}, program));
        let page = pdf.add_object(dictionary! {
            "Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),600.into(),400.into()],
            "UserUnit"=>user_unit,"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font},"XObject"=>dictionary!{"Reusable"=>outer}},
            "Contents"=>content
        });
        kids.push(page.into());
    }
    let note = pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![550.into(),50.into(),570.into(),70.into()],"Contents"=>Object::string_literal("Keep note")});
    pdf.get_dictionary_mut(kids[2].as_reference().unwrap())
        .unwrap()
        .set("Annots", vec![Object::Reference(note)]);
    pdf.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Count"=>3,"Kids"=>kids}.into(),
    );
    let root = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    pdf.trailer.set("Root", root);
    pdf.save(path).unwrap();
}
