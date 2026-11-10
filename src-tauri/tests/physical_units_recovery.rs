use folio_engine::private_storage::{seal, unseal, Purpose};
use folio_engine::{PagePlan, PdfEngine, RecoveryStore};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

fn engine() -> PdfEngine {
    let path = std::env::var_os("FOLIO_PDFIUM_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")
        });
    PdfEngine::start(path).unwrap()
}

fn write_pdf(path: &Path, label: &str) {
    let stream = format!("BT /F1 20 Tf 18 265 Td ({label}) Tj ET");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 300] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>".to_string(),
        format!("<< /Length {} >>\nstream\n{stream}\nendstream", stream.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    let mut pdf = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
    }
    let xref = pdf.len();
    pdf.extend_from_slice(b"xref\n0 6\n0000000000 65535 f \n");
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!("trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    fs::write(path, pdf).unwrap();
}

fn workspace(source: &str) -> Value {
    json!({"version":1,"activeId":"tab-a","tabs":[{
        "id":"tab-a","savedDigest":"old digest","dirty":true,"zoom":110,
        "scrollPosition":{"top":80,"left":12},
        "document":{"name":"Original.pdf","selectedPageId":"page-a","selectedOverlayId":"text-a","pages":[{
            "id":"page-a","sourceId":source,"pageIndex":0,"width":200,"height":300,"rotation":90,
            "overlays":[{"type":"text","id":"text-a","text":"Editable note","x":20,"y":30,"fontSize":14,"color":"#123456"}]
        }]}
    }]})
}

fn manifests(root: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("checkpoint-")
                && p.extension().is_some_and(|e| e == "json")
        })
        .collect();
    paths.sort();
    paths
}


#[test]
fn physical_units_migrate_legacy_recovery_lengths_once() {
    let engine=engine();
    let font=engine.fonts().register(fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
    for version in [1,2,3,4] { for unit in [0.5_f32,2.] {
        let temp=tempfile::tempdir().unwrap();let input=temp.path().join("scaled.pdf");
        write_pdf(&input,"Recovery units");let mut pdf=lopdf::Document::load(&input).unwrap();
        let id=*pdf.get_pages().values().next().unwrap();pdf.get_dictionary_mut(id).unwrap().set("UserUnit",unit);pdf.save(&input).unwrap();
        let original=fs::read(&input).unwrap();let source=engine.open_document(&input).unwrap();
        let mut expected=workspace(&source.id);
        expected["tabs"][0]["dirty"]=json!(false);
        let page=&mut expected["tabs"][0]["document"]["pages"][0];
        page["width"]=json!(200.*unit);page["height"]=json!(300.*unit);
        page["overlays"]=json!([
            {"type":"text","id":"text-a","text":"Editable note","x":20.*unit,"y":30.*unit,"fontSize":14.*unit,"fontId":font.id,"shaping":{"version":1,"direction":"ltr","ligatures":true},"color":"#123456","area":{"version":1,"width":100.*unit,"height":50.*unit,"inset":4.*unit,"lineSpacing":1.5,"alignment":"center"}},
            {"type":"ink","id":"ink-a","paths":[[{"x":10.*unit,"y":20.*unit},{"x":40.*unit,"y":50.*unit}]],"strokeWidth":2.*unit,"color":"#334455"},
            {"type":"highlight","id":"mark-a","rects":[{"x":30.*unit,"y":40.*unit,"width":50.*unit,"height":10.*unit}],"color":"#FFFF00","opacity":0.4},
            {"type":"comment","id":"comment-a","x":80.*unit,"y":90.*unit,"text":"Remember","color":"#FFFF00"}
        ]);
        let root=temp.path().join("recovery");let store=RecoveryStore::new(root.clone()).unwrap();
        store.save(&engine,expected.clone()).unwrap();
        let path=manifests(&root).pop().unwrap();let mut manifest:Value=serde_json::from_slice(&unseal(Purpose::Workspace,&fs::read(&path).unwrap(),32*1024*1024).unwrap()).unwrap();
        assert_eq!(manifest["version"],4);
        if version<3 {
            manifest["version"]=json!(version);
            let page=&mut manifest["workspace"]["tabs"][0]["document"]["pages"][0];
            page["width"]=json!(200);page["height"]=json!(300);
            let overlays=page["overlays"].as_array_mut().unwrap();
            for (i,fields) in [(0,vec!["x","y","fontSize"]),(1,vec!["strokeWidth"]),(3,vec!["x","y"])] {
                for field in fields {overlays[i][field]=json!(overlays[i][field].as_f64().unwrap()/unit as f64);}
            }
            for field in ["width","height","inset"] {overlays[0]["area"][field]=json!(overlays[0]["area"][field].as_f64().unwrap()/unit as f64);}
            for point in overlays[1]["paths"][0].as_array_mut().unwrap() {for field in ["x","y"] {point[field]=json!(point[field].as_f64().unwrap()/unit as f64);}}
            for field in ["x","y","width","height"] {overlays[2]["rects"][0][field]=json!(overlays[2]["rects"][0][field].as_f64().unwrap()/unit as f64);}
            fs::write(&path,seal(Purpose::Workspace,&serde_json::to_vec(&manifest).unwrap()).unwrap()).unwrap();
        } else if version==3 {
            manifest["version"]=json!(3);
            fs::write(&path,seal(Purpose::Workspace,&serde_json::to_vec(&manifest).unwrap()).unwrap()).unwrap();
        }
        engine.close_document(&source.id).unwrap();
        let restored=store.load(&engine).unwrap().unwrap();let actual=&restored["tabs"][0]["document"]["pages"][0];
        let expected_page=&expected["tabs"][0]["document"]["pages"][0];
        let actual_plan:PagePlan=serde_json::from_value(actual.clone()).unwrap();let expected_plan:PagePlan=serde_json::from_value(expected_page.clone()).unwrap();
        assert_eq!((actual_plan.width,actual_plan.height),(expected_plan.width,expected_plan.height));assert_eq!(actual_plan.overlays,expected_plan.overlays);
        assert_eq!(restored["tabs"][0]["dirty"],false);assert_eq!(restored["tabs"][0]["scrollPosition"],expected["tabs"][0]["scrollPosition"]);
        store.save(&engine,restored.clone()).unwrap();let second=store.load(&engine).unwrap().unwrap();let second_page=&second["tabs"][0]["document"]["pages"][0];
        assert_eq!(second_page["overlays"],actual["overlays"]);assert_eq!(second_page["width"],actual["width"]);
        for id in [actual["sourceId"].as_str().unwrap(),second_page["sourceId"].as_str().unwrap()] {engine.close_document(id).unwrap();}
        assert_eq!(fs::read(&input).unwrap(),original);
    }}
}
