use lopdf::{content::Content, dictionary, Document, Object};
use std::path::Path;
#[path = "embedded_forms.rs"]
mod embedded;

pub const CASES: [&str; 7] = [
    "page-quarter",
    "form-rotation",
    "form-shear",
    "mirror",
    "text-rotation",
    "composed",
    "tight-rotation",
];
fn array(values: &[f32]) -> Vec<Object> {
    values.iter().map(|v| Object::Real(*v)).collect()
}
pub fn fixture(path: &Path, kind: &str, case: &str, rotation: i32, first: &str) {
    let first = if kind == "standard" && first == "Cafe old" {
        "Café old"
    } else {
        first
    };
    embedded::fixture(path, kind == "cid", rotation, true, first);
    let mut pdf = Document::load(path).unwrap();
    let standard = pdf.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica","Encoding"=>"WinAnsiEncoding"});
    let page_matrix = if case == "page-quarter" {
        "0 1 -1 0 200 260"
    } else if case == "composed" {
        "0.8 0.3 -0.2 1.1 160 320"
    } else if case == "mirror" {
        "1 0 0 1 340 380"
    } else {
        "1 0 0 1 140 340"
    };
    let second = if case == "mirror" {
        "0.8 0 0 0.8 340 140"
    } else {
        "0.8 0 0 0.8 140 140"
    };
    for (i, page_id) in pdf.get_pages().into_values().enumerate() {
        let page = pdf.get_dictionary_mut(page_id).unwrap();
        page.set("MediaBox", array(&[0., 0., 600., 620.]));
        page.set("CropBox", array(&[10., 20., 590., 600.]));
        let contents = page.get(b"Contents").unwrap().as_reference().unwrap();
        let content = if i == 0 {
            format!("BT /N 12 Tf 1 0 0 1 30 60 Tm (Neighbor) Tj ET q {page_matrix} cm /First Do Q q {second} cm /Shared Do Q")
        } else {
            format!("q {page_matrix} cm /Shared Do Q")
        };
        pdf.get_object_mut(contents)
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .set_plain_content(content.into_bytes());
    }
    for object in pdf.objects.values_mut() {
        if kind == "standard" {
            let dict = match &mut *object {
                Object::Dictionary(d) => Some(d),
                Object::Stream(s) => Some(&mut s.dict),
                _ => None,
            };
            if let Some(dict) = dict {
                if let Ok(fonts) = dict
                    .get_mut(b"Resources")
                    .and_then(Object::as_dict_mut)
                    .and_then(|r| r.get_mut(b"Font"))
                    .and_then(Object::as_dict_mut)
                {
                    if fonts.has(b"F1") {
                        fonts.set("F1", standard);
                    }
                }
            }
        }
        let Ok(stream) = object.as_stream_mut() else {
            continue;
        };
        if stream.dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Form") {
            continue;
        }
        let mut content = Content::decode(&stream.get_plain_content().unwrap()).unwrap();
        let leaf = content.operations.iter().any(|op| op.operator == "Tj");
        stream.dict.set(
            "BBox",
            array(if leaf {
                &[-30., -30., 240., 160.]
            } else {
                &[-100., -100., 380., 220.]
            }),
        );
        if case == "tight-rotation" && leaf {
            stream.dict.set("BBox", array(&[4., 9., 110., 26.]));
        }
        if !leaf && matches!(case, "form-rotation" | "tight-rotation") {
            stream
                .dict
                .set("Matrix", array(&[0.8660254, 0.5, -0.5, 0.8660254, 0., 0.]));
        }
        if leaf && case == "form-shear" {
            stream
                .dict
                .set("Matrix", array(&[1., 0.25, 0.3, 1., 0., 0.]));
        }
        if !leaf && case == "mirror" {
            stream.dict.set("Matrix", array(&[-1., 0., 0., 1., 0., 0.]));
        }
        if case == "composed" {
            stream.dict.set(
                "Matrix",
                array(if leaf {
                    &[1.1, 0.15, 0.2, 0.9, 2., 3.]
                } else {
                    &[0.9, -0.2, 0.15, 1.1, 8., 3.]
                }),
            );
        }
        for op in &mut content.operations {
            if kind == "standard" && op.operator == "Tj" {
                let bytes = op.operands[0]
                    .as_str()
                    .unwrap()
                    .iter()
                    .map(|v| if *v == 1 { b'e' } else { *v })
                    .collect();
                op.operands[0] = Object::String(bytes, lopdf::StringFormat::Hexadecimal);
            }
            if case == "text-rotation" && op.operator == "Tm" {
                op.operands = array(&[0.8660254, 0.5, -0.5, 0.8660254, 25., 15.]);
            }
            if case == "composed" {
                if op.operator == "cm" {
                    op.operands = array(&[1., 0.1, -0.1, 1., 3., 4.]);
                }
                if op.operator == "Tm" {
                    op.operands = array(&[0.97, 0.2, -0.1, 1., 10., 15.]);
                }
            }
        }
        stream.set_plain_content(content.encode().unwrap());
    }
    pdf.save(path).unwrap();
}

pub fn mutate(path: &Path, case: &str) {
    let mut pdf = Document::load(path).unwrap();
    if case == "collision" {
        let page = *pdf.get_pages().values().next().unwrap();
        let content = pdf
            .get_dictionary(page)
            .unwrap()
            .get(b"Contents")
            .unwrap()
            .as_reference()
            .unwrap();
        pdf.get_object_mut(content).unwrap().as_stream_mut().unwrap().set_plain_content(
            b"BT /N 12 Tf 1 0 0 1 30 60 Tm (Neighbor) Tj ET q 0 1 -1 0 200 260 cm /First Do Q q 0 1 -1 0 200 350 cm /Shared Do Q".to_vec());
    }
    for object in pdf.objects.values_mut() {
        let Ok(s) = object.as_stream_mut() else {
            continue;
        };
        if s.dict.get(b"Subtype").and_then(Object::as_name).ok() != Some(b"Form") {
            continue;
        }
        let leaf = Content::decode(&s.get_plain_content().unwrap())
            .unwrap()
            .operations
            .iter()
            .any(|op| matches!(op.operator.as_str(), "Tj" | "TJ"));
        match case {
            "singular" => {
                s.dict.set("Matrix", array(&[1., 1., 1., 1., 0., 0.]));
            }
            "near-singular" => {
                s.dict.set("Matrix", array(&[1., 1., 1., 1.00001, 0., 0.]));
            }
            "excessive" => {
                s.dict.set("Matrix", array(&[101., 0., 0., 1., 0., 0.]));
            }
            "composed-excessive" => {
                s.dict.set("Matrix", array(&[11., 0., 0., 11., 0., 0.]));
            }
            "clipped" if leaf => {
                s.dict.set("BBox", array(&[4., 9., 20., 15.]));
            }
            "ancestor-edge" if !leaf => {
                s.dict.set("BBox", array(&[0., 0., 90., 40.]));
            }
            "leaf-edge" if leaf => {
                s.dict.set("BBox", array(&[0., 0., 90., 40.]));
            }
            _ => {}
        }
    }
    pdf.save(path).unwrap();
}
