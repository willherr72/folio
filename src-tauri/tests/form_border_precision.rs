use folio_engine::{FillFormFieldRequest, FormFieldValue, PdfEngine};
use lopdf::{dictionary, Dictionary, Document, Object, Stream};
use std::path::{Path, PathBuf};

fn engine() -> PdfEngine {
    let dll = std::env::var_os("FOLIO_TEST_PDFIUM_LIBRARY")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"));
    PdfEngine::start(dll).unwrap()
}

fn fixture(kind: &str, width: f32, unit: f32, legacy: bool) -> Vec<u8> {
    let mut d = Document::with_version("1.7");
    let pages = d.new_object_id();
    let page = d.new_object_id();
    let font = d.add_object(dictionary! {"Type"=>"Font", "Subtype"=>"Type1", "BaseFont"=>"Helvetica"});
    let resources = dictionary! {"Font"=>dictionary!{"F1"=>font}};
    let paint = format!("0 g 0 0 280 50 re {width} {width} {} {} re f*", 280.-2.*width, 50.-2.*width);
    let ap = d.add_object(Stream::new(dictionary! {"Type"=>"XObject", "Subtype"=>"Form", "BBox"=>vec![0.into(),0.into(),280.into(),50.into()], "Resources"=>resources.clone()}, paint.into_bytes()));
    let mut field = dictionary! {"Type"=>"Annot", "Subtype"=>"Widget", "FT"=>if kind=="text"{"Tx"}else{"Ch"},
        "T"=>Object::string_literal("border"), "V"=>Object::string_literal(""), "DV"=>Object::string_literal(""),
        "P"=>page, "Rect"=>vec![20.into(),120.into(),300.into(),170.into()], "F"=>4,
        "MK"=>dictionary!{"BC"=>vec![0.into()]}, "AP"=>dictionary!{"N"=>ap}};
    if legacy {
        field.set("Border", vec![0.into(),0.into(),Object::Real(width)]);
    } else {
        field.set("BS", dictionary!{"W"=>width,"S"=>"S"});
    }
    if kind != "text" {
        field.set("Opt", vec![Object::string_literal("First"), Object::string_literal("Second")]);
        field.set("Ff", if kind=="combo" {1<<17} else {0});
    }
    let field = d.add_object(field);
    d.objects.insert(page, Object::Dictionary(dictionary!{"Type"=>"Page","Parent"=>pages,
        "MediaBox"=>vec![0.into(),0.into(),320.into(),240.into()],"UserUnit"=>unit,
        "Resources"=>Dictionary::new(),"Annots"=>vec![Object::Reference(field)]}));
    d.objects.insert(pages, Object::Dictionary(dictionary!{"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1}));
    let form = d.add_object(dictionary!{"Fields"=>vec![Object::Reference(field)],"DA"=>Object::string_literal("/F1 12 Tf 0 g"),"DR"=>resources});
    let root = d.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages,"AcroForm"=>form});
    d.trailer.set("Root", root);
    let mut bytes=vec![];d.save_to(&mut bytes).unwrap();bytes
}

fn assert_border(bytes: &[u8], width: f32) {
    let d=Document::load_mem(bytes).unwrap();
    let page=d.get_dictionary(*d.get_pages().get(&1).unwrap()).unwrap();
    let annots=d.dereference(page.get(b"Annots").unwrap()).unwrap().1.as_array().unwrap();
    let field=d.dereference(&annots[0]).unwrap().1.as_dict().unwrap();
    let ap=d.dereference(field.get(b"AP").unwrap()).unwrap().1.as_dict().unwrap();
    let normal=d.dereference(ap.get(b"N").unwrap()).unwrap().1.as_stream().unwrap();
    let operations=lopdf::content::Content::decode(&normal.get_plain_content().unwrap()).unwrap();
    let rectangles=operations.operations.iter().take_while(|op|op.operator!="f*")
        .filter(|op|op.operator=="re").map(|op|op.operands.iter().map(|n|n.as_float().unwrap()).collect::<Vec<_>>()).collect::<Vec<_>>();
    assert!(rectangles.len()>=2,"fractional border missing: {rectangles:?}");
    for (actual,expected) in rectangles.iter().take(2).flatten().zip([0.,0.,280.,50.,width,width,280.-2.*width,50.-2.*width]) {
        assert!((actual-expected).abs()<0.0001,"border changed: {rectangles:?}, width={width}");
    }
}

fn export(e:&PdfEngine, info:&folio_engine::DocumentInfo, path:&Path) {
    let p=&info.pages[0];
    let page=serde_json::from_value(serde_json::json!({"id":"border-page","sourceId":info.id,"pageIndex":0,
        "width":p.width,"height":p.height,"rotation":0,"overlays":p.overlays})).unwrap();
    e.export_pdf(folio_engine::ExportRequest{pages:vec![page],flatten:false},path).unwrap();
}

fn roundtrip(kind:&str) {
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for (width,unit,legacy) in [(0.5,1.,false),(1.5,1.,false),(0.5,1.,true),(1./1.7,1.7,false),(1.,1.,false)] {
        let bytes=fixture(kind,width,unit,legacy);
        let source=temp.path().join("source.pdf");std::fs::write(&source,&bytes).unwrap();
        let original=e.open_document(&source).unwrap();
        let before=e.list_form_fields(&original.id,0).unwrap().remove(0);
        let wanted=if kind=="text" {FormFieldValue::Text{text:"Edited answer".into()}}
                   else {FormFieldValue::Choice{indices:vec![1]}};
        let filled=e.fill_form_field(&original.id,FillFormFieldRequest{page_index:0,annotation_index:before.annotation_index,
            expected_value:before.value,value:wanted.clone()}).unwrap();
        assert_border(&e.source_bytes(&filled.id).unwrap(),width);
        let saved=temp.path().join("saved.pdf");export(&e,&filled,&saved);
        assert_border(&std::fs::read(&saved).unwrap(),width);
        let reopened=e.open_document(&saved).unwrap();
        let after=e.list_form_fields(&reopened.id,0).unwrap().remove(0);
        assert_eq!(after.value,wanted);
        let b=before.bounds.unwrap();let a=after.bounds.unwrap();
        for (old,new) in [b.x,b.y,b.width,b.height].into_iter().zip([a.x,a.y,a.width,a.height]) {
            assert!((old-new).abs()<0.001,"physical bounds changed");
        }
        assert_eq!(e.source_bytes(&original.id).unwrap().as_ref(),bytes.as_slice());
        assert_eq!(std::fs::read(&source).unwrap(),bytes);
        if let Some(out)=std::env::var_os("FOLIO_FORM_BORDER_OUTPUT") {
            let out=PathBuf::from(out);std::fs::create_dir_all(&out).unwrap();
            let name=format!("{kind}-{width:.5}-{unit}-{legacy}");
            std::fs::copy(&source,out.join(format!("{name}-source.pdf"))).unwrap();
            std::fs::copy(&saved,out.join(format!("{name}-saved.pdf"))).unwrap();
        }
        for id in [&original.id,&filled.id,&reopened.id] {e.close_document(id).unwrap();}
    }
}

#[test]
fn text_fill_preserves_fractional_border_and_physical_bounds() {roundtrip("text");}
#[test]
fn dropdown_fill_preserves_fractional_border_and_physical_bounds() {roundtrip("combo");}
#[test]
fn list_fill_preserves_fractional_border_and_physical_bounds() {roundtrip("list");}
