use super::*;
use lopdf::{dictionary, Stream};
fn replace_names(value: &mut Object, name: &str) {
    match value {
        Object::Array(values) => {
            for v in values {
                replace_names(v, name);
            }
        }
        Object::Dictionary(dict) => {
            for (key, v) in dict.iter_mut() {
                if key == b"BaseFont" || key == b"FontName" {
                    *v = Object::Name(name.as_bytes().to_vec());
                } else {
                    replace_names(v, name);
                }
            }
        }
        Object::Stream(stream) => {
            for (key, v) in stream.dict.iter_mut() {
                if key == b"BaseFont" || key == b"FontName" {
                    *v = Object::Name(name.as_bytes().to_vec());
                } else {
                    replace_names(v, name);
                }
            }
        }
        _ => {}
    }
}
#[test]
fn cursor_names_reserve_inline_and_indirect_font_definitions() {
    let values = vec![
        Object::Dictionary(dictionary! {"BaseFont"=>"placeholder"}),
        Object::Dictionary(
            dictionary! {"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>dictionary!{"BaseFont"=>"placeholder"}}}},
        ),
        Object::Array(vec![Object::Dictionary(
            dictionary! {"BaseFont"=>"placeholder"},
        )]),
        Object::Stream(Stream::new(
            dictionary! {"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>dictionary!{"BaseFont"=>"placeholder"}}}},
            vec![],
        )),
        Object::Dictionary(dictionary! {"Descriptor"=>dictionary!{"FontName"=>"placeholder"}}),
    ];
    for value in values {
        let mut pdf = Document::new();
        let id = pdf.add_object(value);
        let first = name(&pdf, "program", &[(3, 500.25)]).unwrap();
        replace_names(pdf.get_object_mut(id).unwrap(), &first);
        let next = name(&pdf, "program", &[(3, 500.25)]).unwrap();
        assert_ne!(next, first, "Nested definition did not reserve its name");
        assert_eq!(next, name(&pdf, "program", &[(3, 500.25)]).unwrap());
    }
    // A referenced name still reserves the label in a nested direct dictionary.
    let mut pdf = Document::new();
    let string = pdf.add_object(Object::Name(b"placeholder".to_vec()));
    pdf.add_object(dictionary!{"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>dictionary!{"BaseFont"=>string}}}});
    let first = name(&pdf, "program", &[(3, 500.25)]).unwrap();
    *pdf.get_object_mut(string).unwrap() = Object::Name(first.as_bytes().to_vec());
    assert_ne!(first, name(&pdf, "program", &[(3, 500.25)]).unwrap());
}
#[test]
fn cursor_name_search_rejects_excessive_direct_container_depth() {
    let mut value = Object::Null;
    for _ in 0..34 {
        value = Object::Array(vec![value]);
    }
    let mut pdf = Document::new();
    pdf.add_object(value);
    assert!(name(&pdf, "program", &[(3, 500.25)]).is_err());
}
