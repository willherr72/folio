//! Source program authority: expected glyphs and widths come from independently
//! decoded public font fixtures, not from the candidate verifier.
use super::*;
use lopdf::{dictionary, Stream};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/fixtures/source-cff/");

fn replace_review_program(pdf: &mut Document, original: &[u8], replacement: &[u8]) {
    let mut replaced=0;
    for object in pdf.objects.values_mut() {
        if let Ok(stream)=object.as_stream_mut() {
            if stream.content==original {stream.set_content(replacement.to_vec());replaced+=1;}
        }
    }
    assert_eq!(replaced,1);
}

#[test]
fn source_cff_review_rejects_malformed_tail_after_explicit_width() {
    let (mut pdf,font,original)=resource("fractional",false,false);
    assert!(verify(&pdf,&font,&original).is_ok());
    let prefix=[0xff,0x01,0xf4,0xc0,0x00,0x8b,0x8b,0x15];
    let start=original.windows(prefix.len()).position(|part|part==prefix).unwrap();
    let mut changed=original.clone();changed[start+prefix.len()]=10; // Empty-stack callsubr after rmoveto.
    replace_review_program(&mut pdf,&original,&changed);
    assert!(verify(&pdf,&font,&changed).is_err(),"A known width must not authorize an invalid remaining glyph program");
}

#[test]
fn source_cff_review_charges_post_width_subroutine_expansion() {
    let (mut pdf,font,original)=resource("fractional",false,false);
    let changed=std::fs::read(format!("{FIXTURES}review-post-width-expansion.cff")).unwrap();
    replace_review_program(&mut pdf,&original,&changed);
    assert!(verify(&pdf,&font,&changed).is_err(),"Post-width expansion must consume the shared two-million work limit");
}

#[test]
fn source_cff_review_refuses_competing_raw_outline_tables() {
    for fixture in ["review-conflicting-cff2.otf","review-malformed-competing-glyf.otf"] {
        let (mut pdf,font,original)=resource("sid",true,false);
        assert!(verify(&pdf,&font,&original).is_ok());
        let changed=std::fs::read(format!("{FIXTURES}{fixture}")).unwrap();
        replace_review_program(&mut pdf,&original,&changed);
        assert!(verify(&pdf,&font,&changed).is_err(),"Unsupported competing outline tag must refuse even if its parser returns None: {fixture}");
    }
}

#[test]
fn source_cff_large_custom_builtin_proves_far_supplement_with_shared_budget() {
    let raw=std::fs::read(format!("{FIXTURES}large-custom-encoding-v2.cff")).unwrap();
    let mut pdf=Document::with_version("1.7");
    let data=pdf.add_object(Stream::new(dictionary!{"Subtype"=>"Type1C"},raw.clone()));
    let unicode=pdf.add_object(Stream::new(dictionary!{},b"1 begincodespacerange <00> <FF> endcodespacerange 1 beginbfchar <FF> <0041> endbfchar".to_vec()));
    let font=dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"FolioCffLargeEncoding","FontDescriptor"=>dictionary!{"Flags"=>32,"FontFile3"=>data},"FirstChar"=>255,"LastChar"=>255,"Widths"=>vec![Object::Integer(500)],"ToUnicode"=>unicode};
    let proof=verify(&pdf,&font,&raw).expect("A legal complete glyph table must not consume one charset scan per byte code");
    assert_eq!(proof.glyph('A'),Some(65135));
    assert_eq!(proof.encode("A").unwrap(),vec![255]);assert_eq!(proof.advance("A").unwrap(),0.5);
}

#[test]
fn source_cff_opentype_truetype_wrapper_uses_existing_truetype_proof() {
    let raw=include_bytes!("../../tests/fixtures/embedded-text/DejaVuSerif-subset.ttf");
    let face=ttf_parser::Face::parse(raw,0).unwrap();let gid=face.glyph_index('O').unwrap();
    let width=face.glyph_hor_advance(gid).unwrap()as f32*1000./face.units_per_em()as f32;
    let mut pdf=Document::with_version("1.7");
    let data=pdf.add_object(Stream::new(dictionary!{"Subtype"=>"OpenType"},raw.to_vec()));
    let descriptor=pdf.add_object(dictionary!{"Flags"=>32,"FontFile3"=>data});
    let font=dictionary!{"Type"=>"Font","Subtype"=>"TrueType","BaseFont"=>"DejaVuSerif","Encoding"=>"WinAnsiEncoding","FontDescriptor"=>descriptor,"FirstChar"=>79,"LastChar"=>79,"Widths"=>vec![Object::Real(width)]};
    assert_eq!(program(&pdf,&font).unwrap(),raw);
    let proof=verify(&pdf,&font,raw).unwrap();assert_eq!(proof.glyph('O'),Some(gid.0));
    assert_eq!(proof.encode("O").unwrap(),b"O");
    pdf.get_dictionary_mut(descriptor).unwrap().set("FontFile2",data);
    assert!(program(&pdf,&font).is_err(),"A second stream entry cannot be silently selected");
    assert!(verify(&pdf,&font,raw).is_err());
}

#[test]
fn source_cff_truetype_opentype_simple_and_cid_keep_native_editing() {
    use crate::{PdfEngine,ExportRequest,PagePlan};
    let raw=include_bytes!("../../tests/fixtures/shaped-text/DejaVuSans.ttf");
    let asset=crate::fonts::FontAsset::parse(raw.to_vec()).unwrap();let face=asset.face().unwrap();
    let engine=PdfEngine::start(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
    let dir=tempfile::tempdir().unwrap();
    for composite in [false,true] {
        let mut pdf=Document::with_version("1.7");let pages=pdf.new_object_id();
        let source=if composite {
            let source=super::super::super::persistence::install_font_resource(&mut pdf,&asset).unwrap();
            let font=pdf.get_dictionary(source.as_reference().unwrap()).unwrap();
            let child=font.get(b"DescendantFonts").unwrap().as_array().unwrap()[0].as_reference().unwrap();
            let descriptor=pdf.get_dictionary(child).unwrap().get(b"FontDescriptor").unwrap().as_reference().unwrap();
            let data=pdf.get_dictionary(descriptor).unwrap().get(b"FontFile2").unwrap().clone();
            pdf.get_dictionary_mut(descriptor).unwrap().remove(b"FontFile2");pdf.get_dictionary_mut(descriptor).unwrap().set("FontFile3",data.clone());
            pdf.get_object_mut(data.as_reference().unwrap()).unwrap().as_stream_mut().unwrap().dict.set("Subtype","OpenType");source
        }else{
            let data=pdf.add_object(Stream::new(dictionary!{"Subtype"=>"OpenType","Length1"=>raw.len()as i64},raw.to_vec()));
            let descriptor=pdf.add_object(dictionary!{"Type"=>"FontDescriptor","FontName"=>"FolioOpenTypeTrueTypeTest","Flags"=>32,"FontBBox"=>vec![(-1000).into(),(-1000).into(),2000.into(),2000.into()],"ItalicAngle"=>0,"Ascent"=>1000,"Descent"=>-300,"CapHeight"=>800,"StemV"=>80,"FontFile3"=>data});
            let widths:Vec<Object>=(32u8..=126).map(|code|Object::Real(face.glyph_hor_advance(face.glyph_index(code as char).unwrap()).unwrap()as f32*1000./face.units_per_em()as f32)).collect();
            Object::Reference(pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"TrueType","BaseFont"=>"FolioOpenTypeTrueTypeTest","Encoding"=>"WinAnsiEncoding","FirstChar"=>32,"LastChar"=>126,"Widths"=>widths,"FontDescriptor"=>descriptor}))
        };
        let neighbor=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
        let text=if composite {"<004F006C00640054006500780074>"}else{"(OldText)"};
        let content=pdf.add_object(Stream::new(dictionary!{},format!("BT /Source 18 Tf 72 680 Td {text} Tj ET BT /Neighbor 12 Tf 72 620 Td (Unchanged neighbor) Tj ET").into_bytes()));
        let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),612.into(),792.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"Source"=>source,"Neighbor"=>neighbor}},"Contents"=>content});
        pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>1,"Kids"=>vec![Object::Reference(page)]}.into());
        let catalog=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});pdf.trailer.set("Root",catalog);
        let input=dir.path().join(format!("opentype-truetype-{composite}.pdf"));pdf.save(&input).unwrap();
        let original=std::fs::read(&input).unwrap();let source=engine.open_document(&input).unwrap();
        let runs=engine.list_occurrence_text_runs(&source.id,0).unwrap().runs;let selected=runs.iter().find(|r|r.text=="OldText").unwrap();
        assert!(selected.supported,"{composite}: {:?}",selected.reason);
        let edited=engine.replace_text_occurrence(&source.id,&[0],0,selected.object_index,&selected.text,"NewText",None).unwrap();
        let saved=dir.path().join(format!("edited-{composite}.pdf"));
        engine.export_pdf(ExportRequest{flatten:false,pages:vec![PagePlan{crop:None,id:"page".into(),source_id:edited.document.id.clone(),page_index:0,width:source.pages[0].width,height:source.pages[0].height,rotation:0,overlays:vec![]}]},&saved).unwrap();
        let reopened=engine.open_document(&saved).unwrap();let after=engine.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;
        assert!(after.iter().any(|run|run.text=="NewText"&&run.supported));
        let neighbor=runs.iter().find(|run|run.text=="Unchanged neighbor").unwrap();
        assert_eq!(after.iter().find(|run|run.text==neighbor.text).unwrap().bounds,neighbor.bounds);
        assert_eq!(std::fs::read(&input).unwrap(),original);
        if let Some(root)=std::env::var_os("FOLIO_CFF_SESSION_OUTPUTS") {
            let out=std::path::PathBuf::from(root).join(format!("opentype-truetype-{composite}"));std::fs::create_dir(&out).unwrap();
            std::fs::copy(input,out.join("original.pdf")).unwrap();std::fs::copy(saved,out.join("edited.pdf")).unwrap();
        }
        for id in [&reopened.id,&edited.document.id,&source.id] {engine.close_document(id).unwrap();}
    }
}

fn resource(label: &str, wrapped: bool, composite: bool) -> (Document, Dictionary, Vec<u8>) {
    let metrics: serde_json::Value = serde_json::from_slice(&std::fs::read(format!("{FIXTURES}metrics.json")).unwrap()).unwrap();
    let rows = metrics["fonts"].as_array().unwrap().iter().find(|row| row["label"] == label).unwrap();
    let keyed = rows["cid"].as_bool().unwrap();
    assert!(!keyed || composite);
    let bytes = std::fs::read(format!("{FIXTURES}{label}.{}", if wrapped { "otf" } else { "cff" })).unwrap();
    let mut pdf = Document::with_version("1.7");
    let data = pdf.add_object(Stream::new(dictionary! {"Subtype" => if wrapped {"OpenType"} else if composite {"CIDFontType0C"} else {"Type1C"}}, bytes.clone()));
    let descriptor = pdf.add_object(dictionary! {"Type"=>"FontDescriptor", "FontName"=>"FolioCffTest", "Flags"=>32, "FontFile3"=>data});
    let mut mappings = Vec::new();
    let mut widths = Vec::new();
    let face = if label == "sid" || label == "cid" { Some(ttf_parser::Face::parse(if label == "sid" {include_bytes!("../../tests/fixtures/source-cff/sid.otf")} else {include_bytes!("../../tests/fixtures/source-cff/cid.otf")},0).unwrap()) } else {None};
    for ch in if face.is_some() {" OldTextNew"} else {" AB"}.chars().collect::<std::collections::BTreeSet<_>>() {
        let glyph = rows["glyphs"].as_array().unwrap().iter().find(|row| {
            if let Some(face) = &face {face.glyph_index(ch).is_some_and(|id| row["gid"].as_u64() == Some(u64::from(id.0)))}
            else { row["name"].as_str() == Some(if ch == ' ' {"space"} else if ch == 'A' {"A"} else {"B"}) }
        }).unwrap();
        let code = if composite { if keyed {glyph["cid"].as_u64().unwrap()} else {glyph["gid"].as_u64().unwrap()} } else {ch as u64};
        let width = (glyph["width"].as_f64().unwrap() * glyph["matrix"][0].as_f64().unwrap() * 1000.) as f32;
        mappings.push((code, ch)); widths.push((code as u16, width));
    }
    let code_len = if composite {4} else {2};
    let unicode = format!("1 begincodespacerange <{}> <{}> endcodespacerange {} beginbfchar {} endbfchar", if composite {"0000"} else {"00"}, if composite {"FFFF"} else {"FF"}, mappings.len(), mappings.iter().map(|(code,ch)| format!("<{code:0code_len$X}> <{:04X}>", *ch as u32)).collect::<Vec<_>>().join(" "));
    let unicode = pdf.add_object(Stream::new(dictionary!{}, unicode.into_bytes()));
    let font = if composite {
        let w: Vec<Object> = widths.iter().flat_map(|(code,width)| [Object::Integer(i64::from(*code)), Object::Array(vec![Object::Real(*width)])]).collect();
        let child = pdf.add_object(dictionary! {"Type"=>"Font", "Subtype"=>"CIDFontType0", "BaseFont"=>"FolioCffTest", "FontDescriptor"=>descriptor, "CIDSystemInfo"=>dictionary!{"Registry"=>Object::string_literal("Adobe"),"Ordering"=>Object::string_literal("Identity"),"Supplement"=>0}, "W"=>w});
        dictionary! {"Type"=>"Font", "Subtype"=>"Type0", "BaseFont"=>"FolioCffTest", "Encoding"=>"Identity-H", "DescendantFonts"=>vec![Object::Reference(child)], "ToUnicode"=>unicode}
    } else {
        let first = widths.iter().map(|(c,_)| *c).min().unwrap(); let last = widths.iter().map(|(c,_)| *c).max().unwrap();
        let w: Vec<Object> = (first..=last).map(|c| Object::Real(widths.iter().find(|(code,_)| *code == c).map_or(0., |(_,w)| *w))).collect();
        dictionary! {"Type"=>"Font", "Subtype"=>"Type1", "BaseFont"=>"FolioCffTest", "Encoding"=>"StandardEncoding", "FirstChar"=>i64::from(first), "LastChar"=>i64::from(last), "Widths"=>w, "FontDescriptor"=>descriptor, "ToUnicode"=>unicode}
    };
    (pdf,font,bytes)
}

#[test]
fn source_cff_raw_and_wrapped_simple_and_cid_prove_exact_source_codes() {
    for (label,wrapped,composite) in [("sid",false,false),("sid",true,false),("sid",false,true),("sid",true,true),("cid",false,true),("cid",true,true)] {
        let (mut pdf,font,bytes) = resource(label,wrapped,composite);
        let proof = verify(&pdf,&font,&bytes).unwrap_or_else(|e| panic!("{label}/{wrapped}/{composite}: {e}"));
        assert_eq!(proof.decode(&proof.encode("NewText").unwrap()).unwrap(), "NewText");
        assert_eq!(program(&pdf,&font).unwrap(), bytes);
        pdf.add_object(font); let mut saved = vec![]; pdf.save_to(&mut saved).unwrap();
        let proofs = FontProofs::read(&saved).unwrap();
        assert_eq!(proofs.find("FolioCffTest", &key(&bytes)).unwrap().characters, proof.characters);
    }
}

#[test]
fn source_cff_fractional_and_nondefault_units_are_independent_width_authority() {
    for (label,expected) in [("fractional",0.50075),("fractional-2048",500.75/2048.)] {
        let (pdf,mut font,bytes) = resource(label,false,false);
        let proof = verify(&pdf,&font,&bytes).unwrap();
        assert!((proof.advance("A").unwrap()-expected).abs()<0.000001);
        let first = font.get(b"FirstChar").unwrap().as_i64().unwrap();
        let mut widths = font.get(b"Widths").unwrap().as_array().unwrap().clone();
        widths[(65-first)as usize] = Object::Real(expected*1000.+4.);
        font.set("Widths",widths);
        assert!(verify(&pdf,&font,&bytes).is_err(), "PDF widths cannot authorize another program advance");
    }
}

#[test]
fn source_cff_container_and_descendant_conflicts_refuse() {
    let (mut pdf,font,bytes) = resource("sid",false,false);
    let descriptor = font.get(b"FontDescriptor").unwrap().as_reference().unwrap();
    let competing = pdf.new_object_id();
    pdf.get_dictionary_mut(descriptor).unwrap().set("FontFile2", Object::Reference(competing));
    assert!(program(&pdf,&font).is_err());
    assert!(verify(&pdf,&font,&bytes).is_err());
    let (mut pdf,font,bytes) = resource("cid",false,true);
    let child = font.get(b"DescendantFonts").unwrap().as_array().unwrap()[0].as_reference().unwrap();
    pdf.get_dictionary_mut(child).unwrap().set("Subtype","CIDFontType2");
    assert!(verify(&pdf,&font,&bytes).is_err());
}

#[test]
fn source_cff_differences_select_literal_names_and_reject_forged_unicode() {
    let (mut pdf,mut font,bytes)=resource("fractional",false,false);
    font.set("Encoding",dictionary!{"BaseEncoding"=>"StandardEncoding","Differences"=>vec![Object::Integer(65),Object::Name(b"B".to_vec())]});
    assert!(verify(&pdf,&font,&bytes).is_err(),"B cannot be authorized as A by its ToUnicode alone");
    let widths=font.get(b"Widths").unwrap().as_array().unwrap().clone();
    let mut shifted=vec![Object::Real(0.);67];shifted[7]=widths[33].clone();shifted[32]=widths[0].clone();shifted[66]=widths[34].clone();
    font.set("FirstChar",0);font.set("LastChar",66);font.set("Widths",shifted);
    font.set("Encoding",dictionary!{"BaseEncoding"=>"StandardEncoding","Differences"=>vec![Object::Integer(7),Object::Name(b"A".to_vec()),Object::Integer(65),Object::Name(b".notdef".to_vec())]});
    let unicode=pdf.add_object(Stream::new(dictionary!{},b"1 begincodespacerange <00> <FF> endcodespacerange 3 beginbfchar <07> <0041> <20> <0020> <42> <0042> endbfchar".to_vec()));
    font.set("ToUnicode",unicode);
    assert_eq!(verify(&pdf,&font,&bytes).unwrap().encode("A B").unwrap(),vec![7,32,66]);
    font.set("Encoding",dictionary!{"Differences"=>vec![Object::Integer(7),Object::Name(b"A".to_vec()),Object::Integer(7),Object::Name(b"B".to_vec())]});
    assert!(verify(&pdf,&font,&bytes).is_err());
}

#[test]
fn source_cff_missing_unicode_uses_actual_names_and_custom_builtin_never_falls_back() {
    let (pdf,mut font,bytes)=resource("fractional",false,false);
    font.remove(b"ToUnicode");font.remove(b"Encoding");
    assert_eq!(verify(&pdf,&font,&bytes).unwrap().encode("AB").unwrap(),b"AB");
    let custom=include_bytes!("../../tests/fixtures/source-cff/custom-encoding.cff");
    let descriptor=font.get(b"FontDescriptor").unwrap().as_reference().unwrap();
    let program_id=pdf.get_dictionary(descriptor).unwrap().get(b"FontFile3").unwrap().as_reference().unwrap();
    let mut pdf=pdf;pdf.objects.insert(program_id,Object::Stream(Stream::new(dictionary!{"Subtype"=>"Type1C"},custom.to_vec())));
    let proof=verify(&pdf,&font,custom).unwrap();
    assert_eq!(proof.encode("AB").unwrap(),b"AB");
    assert!(proof.encode("C").is_err());
}

#[test]
fn source_cff_native_single_run_save_reopen_and_original_program_preservation() {
    use crate::{PdfEngine,ExportRequest,PagePlan};
    let engine=PdfEngine::start(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
    let dir=tempfile::tempdir().unwrap();
    let export=|document:&crate::DocumentInfo,path:&std::path::Path| {
        engine.export_pdf(ExportRequest {flatten:false,pages:vec![PagePlan {
            crop:None,id:"page".into(),source_id:document.id.clone(),page_index:0,
            width:document.pages[0].width,height:document.pages[0].height,rotation:0,overlays:vec![],
        }]},path).unwrap();
    };
    for stem in ["sid-raw-simple","sid-wrapped-simple","sid-raw-composite","sid-wrapped-composite","cid-raw-composite","cid-wrapped-composite","fractional-raw-simple","fractional-2048-raw-simple"] {
        let input=std::path::PathBuf::from(FIXTURES).join("pdf").join(format!("{stem}.pdf"));
        let original=std::fs::read(&input).unwrap();let source=engine.open_document(&input).unwrap();
        let before=engine.list_occurrence_text_runs(&source.id,0).unwrap().runs;
        let run=before.iter().find(|run|run.text=="OldText" || run.text=="AB").unwrap();
        assert!(run.supported,"{stem}: {:?}",run.reason);
        let replacement=if run.text=="AB" {"BA"}else{"NewText"};
        let edited=engine.replace_text_occurrence(&source.id,&[0],0,run.object_index,&run.text,replacement,None).unwrap();
        let output=dir.path().join(format!("{stem}.pdf"));export(&edited.document,&output);
        let reopened=engine.open_document(&output).unwrap();
        let after=engine.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;
        let target=after.iter().find(|run|run.text==replacement).unwrap();
        assert!(target.supported,"{stem} reopened: {:?}",target.reason);
        assert_eq!(target.font_name,run.font_name);assert_eq!(target.font_size,run.font_size);
        let neighbor=before.iter().find(|run|run.text=="Unchanged neighbor").unwrap();
        let remaining=after.iter().find(|run|run.text==neighbor.text).unwrap();
        assert_eq!(remaining.bounds,neighbor.bounds);
        let programs=|bytes:&[u8]| {
            let pdf=Document::load_mem(bytes).unwrap();
            let mut programs:Vec<_>=pdf.objects.values().filter_map(|value|value.as_dict().ok()).filter_map(|font| {
                if font.get(b"Type").and_then(Object::as_name).ok()!=Some(b"Font") || !cff::uses(&pdf,font) {return None;}
                program(&pdf,font).ok().map(|bytes|key(&bytes))
            }).collect();programs.sort();programs.dedup();programs
        };
        assert!(!programs(&original).is_empty());assert_eq!(programs(&std::fs::read(&output).unwrap()),programs(&original));
        let again=engine.replace_text_occurrence(&reopened.id,&[0],0,target.object_index,&target.text,&run.text,None).unwrap();
        let restored=dir.path().join(format!("{stem}-restored.pdf"));export(&again.document,&restored);
        assert_eq!(std::fs::read(&input).unwrap(),original);
        for id in [&again.document.id,&reopened.id,&edited.document.id,&source.id] {engine.close_document(id).unwrap();}
        if let Some(root)=std::env::var_os("FOLIO_CFF_SESSION_OUTPUTS") {
            let root=std::path::PathBuf::from(root).join(stem);std::fs::create_dir(&root).unwrap();
            std::fs::copy(input,root.join("original.pdf")).unwrap();std::fs::copy(output,root.join("edited.pdf")).unwrap();std::fs::copy(restored,root.join("restored.pdf")).unwrap();
        }
    }
}

fn cff_ligature_cases() -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(format!("{FIXTURES}ligatures-v1/authority.json")).unwrap()).unwrap()
}
fn cff_ligature_resource(row:&serde_json::Value)->(Document,Dictionary,Vec<u8>,Vec<u8>) {
    let pdf=Document::load(format!("{FIXTURES}ligatures-v1/{}",row["path"].as_str().unwrap())).unwrap();
    let page=*pdf.get_pages().values().next().unwrap();
    let resources=pdf.get_dictionary(page).unwrap().get(b"Resources").unwrap().as_dict().unwrap();
    let fonts=resources.get(b"Font").unwrap().as_dict().unwrap();
    let font=dict(&pdf,fonts.get(b"Source").unwrap()).unwrap().clone();
    let bytes=program(&pdf,&font).unwrap();
    let encoded=row["sourceHex"].as_str().unwrap().as_bytes().chunks_exact(2).map(|b|u8::from_str_radix(std::str::from_utf8(b).unwrap(),16).unwrap()).collect();
    (pdf,font,bytes,encoded)
}
#[test]
fn cff_source_ligatures_bind_raw_sid_cid_and_wrapped_physical_glyphs() {
    let authority=cff_ligature_cases();let mut errors=Vec::new();
    for row in authority["cases"].as_array().unwrap().iter().filter(|r|r["layout"]=="single") {
        let (pdf,font,bytes,encoded)=cff_ligature_resource(row);
        let proof=verify(&pdf,&font,&bytes).unwrap();
        match proof.bind(&encoded) {
            Ok(Some(bound))=>{
                assert_eq!(bound.encoded().text(),"Office");assert_eq!(bound.encoded().bytes(),encoded);
                assert_eq!(bound.encoded().atoms().len(),if row["sequence"]=="fi" {5}else{4});
                for atom in bound.encoded().atoms(){let ink=proof.source_glyph_ink(atom.gid,18.).unwrap().unwrap();assert!(ink.iter().all(|v|v.is_finite())&&ink[2]>ink[0]&&ink[3]>ink[1]);}
                assert!(proof.source_glyph_ink(0,18.).is_err());assert!(proof.source_glyph_ink(bound.encoded().atoms()[0].gid,f32::NAN).is_err());
                assert_eq!(bound.replace("Office").unwrap().bytes(),encoded);
                let grown=bound.replace("Office this").unwrap();assert!(grown.bytes().starts_with(&encoded));assert_eq!(proof.decode(grown.bytes()).unwrap(),"Office this");
                assert_eq!(proof.decode(bound.replace("Ofice").unwrap().bytes()).unwrap(),"Ofice");
            },
            other=>errors.push(format!("{}: {other:?}",row["name"])),
        }
    }
    assert!(errors.is_empty(),"CFF source ligature bindings missing: {errors:?}");
}
#[test]
fn cff_source_ligatures_keep_scalar_and_complete_form_proof_policies() {
    for row in cff_ligature_cases()["cases"].as_array().unwrap().iter().filter(|r|r["layout"]=="single") {
        let (pdf,font,bytes,_)=cff_ligature_resource(row);let proof=verify(&pdf,&font,&bytes).unwrap();
        assert_eq!(proof.decode(&proof.encode("safe").unwrap()).unwrap(),"safe");
        assert!(cff::verify(&pdf,&font,&bytes,true).is_err(),"Complete form proof must not silently become partial");
    }
}
#[test]
fn cff_source_ligatures_exclude_ambiguous_sequences_without_revoking_scalars() {
    for row in cff_ligature_cases()["cases"].as_array().unwrap().iter().filter(|r|r["layout"]=="single") {
        let (mut pdf,font,bytes,encoded)=cff_ligature_resource(row);let id=font.get(b"ToUnicode").unwrap().as_reference().unwrap();
        let stream=pdf.get_object_mut(id).unwrap().as_stream_mut().unwrap();let content=String::from_utf8(stream.content.clone()).unwrap();
        assert!(content.contains("<00660069>")&&content.contains("<006600660069>"));
        stream.set_content(content.replace("<00660069>","<006600660069>").into_bytes());
        let proof=verify(&pdf,&font,&bytes).unwrap();assert_eq!(proof.decode(&proof.encode("safe").unwrap()).unwrap(),"safe");assert!(proof.decode(&encoded).is_err());
    }
}

#[test]
fn cff_source_ligatures_refuse_known_glyph_relabeling_and_keep_scalar_text() {
    for row in cff_ligature_cases()["cases"].as_array().unwrap().iter().filter(|r|r["layout"]=="single"&&(r["label"]=="sid"||r["wrapped"]==true)) {
        let (mut pdf,font,bytes,encoded)=cff_ligature_resource(row);let id=font.get(b"ToUnicode").unwrap().as_reference().unwrap();
        let stream=pdf.get_object_mut(id).unwrap().as_stream_mut().unwrap();let content=String::from_utf8(stream.content.clone()).unwrap();
        stream.set_content(content.replace("<00660069>","<004f004f>").replace("<006600660069>","<004f004f004f>").into_bytes());
        let proof=verify(&pdf,&font,&bytes).unwrap();assert_eq!(proof.decode(&proof.encode("safe").unwrap()).unwrap(),"safe");assert!(proof.decode(&encoded).is_err());
    }
}
#[test]
fn cff_source_ligatures_bad_sequence_width_never_revokes_valid_scalar_codes() {
    for row in cff_ligature_cases()["cases"].as_array().unwrap().iter().filter(|r|r["layout"]=="single") {
        let (mut pdf,mut font,bytes,encoded)=cff_ligature_resource(row);
        let symbol=row["symbols"].as_array().unwrap().iter().find(|s|s["text"]==row["sequence"]).unwrap();let code=symbol["code"].as_i64().unwrap();
        if row["composite"]==true {
            let child=font.get(b"DescendantFonts").unwrap().as_array().unwrap()[0].as_reference().unwrap();
            let widths=pdf.get_dictionary_mut(child).unwrap().get_mut(b"W").unwrap().as_array_mut().unwrap();
            let pair=widths.chunks_exact_mut(2).find(|p|p[0].as_i64().unwrap()==code).unwrap();pair[1]=Object::Array(vec![Object::Real(symbol["width"].as_f64().unwrap()as f32+10.)]);
        }else{
            let first=font.get(b"FirstChar").unwrap().as_i64().unwrap();
            font.get_mut(b"Widths").unwrap().as_array_mut().unwrap()[(code-first)as usize]=Object::Real(symbol["width"].as_f64().unwrap()as f32+10.);
        }
        let proof=verify(&pdf,&font,&bytes).unwrap();assert_eq!(proof.decode(&proof.encode("safe").unwrap()).unwrap(),"safe");assert!(proof.decode(&encoded).is_err());
    }
}

fn cff_ligature_control(row:&serde_json::Value,kind:&str)->(Document,Dictionary,Vec<u8>,Vec<u8>) {
    let (mut pdf,font,original,encoded)=cff_ligature_resource(row);
    let suffix=if row["wrapped"]==true {"otf"}else{"cff"};
    let changed=std::fs::read(format!("{FIXTURES}ligatures-v1/{kind}-{}.{suffix}",row["label"].as_str().unwrap())).unwrap();
    replace_review_program(&mut pdf,&original,&changed);(pdf,font,changed,encoded)
}
#[test]
fn cff_source_ligatures_zero_ink_sequences_do_not_disable_real_scalar_spaces() {
    for row in cff_ligature_cases()["cases"].as_array().unwrap().iter().filter(|r|r["layout"]=="single") {
        let (pdf,font,bytes,encoded)=cff_ligature_control(row,"empty");let proof=verify(&pdf,&font,&bytes).unwrap();
        assert_eq!(proof.decode(&proof.encode("safe safe").unwrap()).unwrap(),"safe safe");assert!(proof.decode(&encoded).is_err());
    }
}
#[test]
fn cff_source_ligatures_work_expansion_cannot_exhaust_unrelated_scalar_proof() {
    let row=cff_ligature_cases()["cases"].as_array().unwrap().iter().find(|r|r["name"]=="sid-raw-simple-ffi-single").unwrap().clone();
    let (mut pdf,mut font,bytes,_)=cff_ligature_control(&row,"budget");
    // Put the hostile sequence before every scalar in CMap order. Its original
    // code is 201; actual source bytes using it must still remain ineligible.
    let id=font.get(b"ToUnicode").unwrap().as_reference().unwrap();
    let map=pdf.get_object_mut(id).unwrap().as_stream_mut().unwrap();
    let content=String::from_utf8(map.content.clone()).unwrap();map.set_content(content.replace("<c9> <006600660069>","<01> <006600660069>").into_bytes());
    let first=font.get(b"FirstChar").unwrap().as_i64().unwrap();let old=font.get(b"Widths").unwrap().as_array().unwrap().clone();
    let width=old[(201-first)as usize].clone();let mut widths=vec![Object::Integer(0);first as usize-1];widths[0]=width;widths.extend(old);
    font.set("FirstChar",1);font.set("Widths",widths);
    let mut encoding=font.get(b"Encoding").unwrap().as_dict().unwrap().clone();
    let mut differences=encoding.get(b"Differences").unwrap().as_array().unwrap().clone();
    let at=differences.chunks_exact(2).position(|pair|pair[0].as_i64().unwrap()==201).unwrap()*2;differences[at]=Object::Integer(1);encoding.set("Differences",differences);font.set("Encoding",encoding);
    let proof=verify(&pdf,&font,&bytes).expect("A newly considered sequence must not consume the established scalar proof budget");
    assert_eq!(proof.decode(&proof.encode("safe").unwrap()).unwrap(),"safe");assert!(proof.decode(&[1]).is_err());
}
#[test]
fn cff_source_ligatures_preserve_established_raw_composite_scalar_semantics() {
    let row=cff_ligature_cases()["cases"].as_array().unwrap().iter().find(|r|r["name"]=="sid-raw-cid-fi-single").unwrap().clone();
    let (mut pdf,font,bytes,_)=cff_ligature_resource(&row);let id=font.get(b"ToUnicode").unwrap().as_reference().unwrap();
    let map=pdf.get_object_mut(id).unwrap().as_stream_mut().unwrap();let content=String::from_utf8(map.content.clone()).unwrap();
    map.set_content(content.replace("<004f>","<0051>").into_bytes());
    let proof=verify(&pdf,&font,&bytes).expect("Raw composite scalar semantics are owned by the existing ToUnicode/CID proof, not simple-font glyph-name inference");
    assert_eq!(proof.decode(&proof.encode("Q").unwrap()).unwrap(),"Q");
}
#[test]
fn cff_source_ligatures_width_and_ink_share_the_same_bounded_charstring_walk() {
    let bytes=std::fs::read(format!("{FIXTURES}ligatures-v1/sid.cff")).unwrap();let mut work=2_000_000;
    let table=ttf_parser::cff::Table::parse_for_proof(&bytes,&mut work).unwrap();
    let glyphs=table.glyph_infos(&mut work).collect::<Result<Vec<_>,_>>().unwrap();
    for name in ["space","fi","ffi"] {
        let glyph=glyphs.iter().find(|g|g.name==Some(name)).unwrap();let (mut a,mut b)=(100_000,100_000);
        let width=table.glyph_width_with_budget(glyph.glyph_id,&mut a).unwrap();let (measured,bounds)=table.glyph_metrics_with_budget(glyph.glyph_id,&mut b).unwrap();
        assert_eq!(width,measured);assert_eq!(a,b,"No extra unbounded outline pass");assert_eq!(bounds.x_max>bounds.x_min&&bounds.y_max>bounds.y_min,name!="space");
    }
}

#[test]
fn cff_source_ligatures_move_only_and_degenerate_contours_are_not_ink() {
    let mut admitted=Vec::new();
    for row in cff_ligature_cases()["cases"].as_array().unwrap().iter().filter(|r|r["layout"]=="single") {
        for kind in ["move-only","diagonal","retraced-curve"] {
            let (pdf,font,bytes,encoded)=cff_ligature_control(row,kind);
            let proof=verify(&pdf,&font,&bytes).expect("Excluded sequences must preserve scalar proofs");
            assert_eq!(proof.decode(&proof.encode("safe safe").unwrap()).unwrap(),"safe safe");
            if proof.decode(&encoded).is_ok() {admitted.push(format!("{} {kind}",row["name"]));}
        }
    }
    assert!(admitted.is_empty(),"Non-painting source sequences admitted: {admitted:?}");
}
