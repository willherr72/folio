use super::*;
use lopdf::{dictionary, Document, Object, Stream};

fn runtime() -> WorkerRuntime {
    WorkerRuntime::new(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),Arc::new(FontRegistry::new())).unwrap()
}
fn generated(rt:&WorkerRuntime) -> Vec<u8> {
    generated_on(rt,0,1.0)
}
fn generated_on(rt:&WorkerRuntime,rotation:u16,unit:f32) -> Vec<u8> {
    let mut pdf=Document::with_version("1.7");let tree=pdf.new_object_id();
    let content=pdf.add_object(Stream::new(dictionary!{},b"0.2 0.4 0.8 rg 40 295 20 40 re f".to_vec()));
    let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Rotate"=>i64::from(rotation),"UserUnit"=>unit,"Resources"=>dictionary!{},"Contents"=>content});
    pdf.objects.insert(tree,dictionary!{"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1}.into());
    let catalog=pdf.add_object(dictionary!{"Type"=>"Catalog","Pages"=>tree});pdf.trailer.set("Root",catalog);
    let native=rt.pdfium.load_pdf_from_reader(Cursor::new(crate::pdf_output::save(&mut pdf).unwrap()),None).unwrap();
    {
        let mut page=native.pages().get(0).unwrap();let geometry=page_geometry(&page).unwrap();
        add_highlight_annotation(&mut page,geometry,&HighlightOverlay{id:"source".into(),rects:vec![AnnotationRect{x:20.,y:80.,width:80.,height:20.}],color:"#ffff00".into(),text:Some("Keep the authored appearance".into()),opacity:Some(0.5)}).unwrap();
        // Force the native renderer to generate its own AP before saving.
        page.render_with_config(&PdfRenderConfig::new().set_target_width(1).render_annotations(true)).unwrap();
    }
    crate::pdf_output::from_pdfium(&native).unwrap()
}
fn annotation(pdf:&Document)->&lopdf::Dictionary {
    let page=pdf.get_pages()[&1];let annots=pdf.get_dictionary(page).unwrap().get(b"Annots").unwrap();
    let first=&pdf.dereference(annots).unwrap().1.as_array().unwrap()[0];pdf.dereference(first).unwrap().1.as_dict().unwrap()
}
fn open(rt:&mut WorkerRuntime,bytes:Vec<u8>,editable:bool)->DocumentInfo {
    let path=std::env::temp_dir().join(format!("folio-highlight-admission-{}.pdf",Uuid::new_v4()));
    rt.open_bytes(bytes,path.clone(),path,editable,true,false,OpenSecurity::Restored(None)).unwrap()
}
fn capture(name:&str,bytes:&[u8]) {
    if let Some(root)=std::env::var_os("FOLIO_STRUCTURED_OVERLAY_CAPTURE_DIR") {
        let root=PathBuf::from(root);std::fs::create_dir_all(&root).unwrap();std::fs::write(root.join(format!("{name}.pdf")),bytes).unwrap();
    }
}

fn missing_appearance(rt: &WorkerRuntime, change: impl FnOnce(&mut lopdf::Dictionary)) -> Vec<u8> {
    let mut pdf = Document::load_mem(&generated(rt)).unwrap();
    let mut item = annotation(&pdf).clone();
    item.remove(b"AP");
    change(&mut item);
    let page = pdf.get_pages()[&1];
    pdf.get_dictionary_mut(page).unwrap().set("Annots", vec![Object::Dictionary(item)]);
    crate::pdf_output::save(&mut pdf).unwrap()
}

fn assert_missing_appearance_stays_native(change: impl FnOnce(&mut lopdf::Dictionary), name: &str) {
    let mut rt = runtime();
    let bytes = missing_appearance(&rt, change);
    let original = open(&mut rt, bytes.clone(), false);
    let pixels = rt.render_page(&original.id, 0, 900).unwrap();
    let imported = open(&mut rt, bytes.clone(), true);
    assert!(imported.pages[0].overlays.is_empty(), "{name}: lossy model conversion must retain the native annotation");
    assert_eq!(rt.render_page(&imported.id, 0, 900).unwrap(), pixels);
    assert_eq!(rt.document(&imported.id).unwrap().bytes().unwrap().as_ref(), bytes);
    capture(name, &bytes);
    let plan = PagePlan { id: "page".into(), source_id: imported.id.clone(), page_index: 0,
        width: imported.pages[0].width, height: imported.pages[0].height, rotation: 0, crop: None, overlays: vec![] };
    let request = ExportRequest { pages: vec![plan.clone()], flatten: false };
    assert_eq!(rt.render_page_plan(plan, 900).unwrap(), pixels);
    let saved = rt.compose_pdf(request.clone(), None, CompositionIntent::Save).unwrap();
    capture(&format!("{name}-saved"), &saved);
    let reopened = open(&mut rt, saved, true);
    assert!(reopened.pages[0].overlays.is_empty());
    assert_eq!(rt.render_page(&reopened.id, 0, 900).unwrap(), pixels);
    let printed = rt.prepare_print(request).unwrap();
    assert_eq!(rt.render_page(&printed.id, 0, 900).unwrap(), pixels);
}

#[test]
fn highlight_admission_missing_appearance_keeps_exact_native_values_editable() {
    let mut rt = runtime();
    let bytes = missing_appearance(&rt, |_| {});
    let info = open(&mut rt, bytes, true);
    assert_eq!(info.pages[0].overlays.len(), 1);
}

#[test]
fn highlight_admission_missing_appearance_defaults_transparency_and_rotation_round_trip() {
    let mut rt = runtime();
    for rotation in [0, 90, 180, 270] {
        for unit in [1.0, 1.7] {
            for mode in ["exact", "defaults", "transparent", "offpage"] {
                let mut pdf = Document::load_mem(&generated_on(&rt, rotation, unit)).unwrap();
                let mut item = annotation(&pdf).clone(); item.remove(b"AP");
                if mode == "defaults" { item.remove(b"C"); item.remove(b"CA"); }
                if mode == "transparent" { item.set("CA", 0); }
                if mode == "offpage" {
                    item.set("Rect", vec![(-20).into(),280.into(),60.into(),300.into()]);
                    item.set("QuadPoints", vec![(-20).into(),300.into(),60.into(),300.into(),(-20).into(),280.into(),60.into(),280.into()]);
                }
                item.set("Contents", Object::Null);
                let page = pdf.get_pages()[&1];
                pdf.get_dictionary_mut(page).unwrap().set("Annots", vec![Object::Dictionary(item)]);
                let bytes = crate::pdf_output::save(&mut pdf).unwrap();
                let original = open(&mut rt, bytes.clone(), false);
                let expected = rt.render_page(&original.id, 0, 600).unwrap();
                let info = open(&mut rt, bytes, true);
                assert_eq!(info.pages[0].overlays.len(), 1, "{rotation}/{unit}/{mode}");
                let request = ExportRequest { pages: vec![PagePlan { id: "page".into(), source_id: info.id.clone(), page_index: 0,
                    width: info.pages[0].width, height: info.pages[0].height, rotation: 0, crop: None, overlays: info.pages[0].overlays.clone() }], flatten: false };
                let saved = rt.compose_pdf(request, None, CompositionIntent::Save).unwrap();
                let reopened = open(&mut rt, saved.clone(), true);
                assert_eq!(reopened.pages[0].overlays.len(), 1, "saved {rotation}/{unit}/{mode}");
                let raw = open(&mut rt, saved, false);
                assert_eq!(rt.render_page(&raw.id, 0, 600).unwrap(), expected, "pixels {rotation}/{unit}/{mode}");
            }
        }
    }
}

#[test]
fn highlight_admission_missing_appearance_refuses_small_quad_skew() {
    assert_missing_appearance_stays_native(|item| {
        let values = item.get_mut(b"QuadPoints").unwrap().as_array_mut().unwrap();
        values[0] = Object::Real(values[0].as_float().unwrap() + 0.125);
    }, "missing-ap-skew");
}

#[test]
fn highlight_admission_missing_appearance_refuses_color_rounding() {
    assert_missing_appearance_stays_native(|item| {
        item.set("C", vec![Object::Real(0.12345), 1.into(), 0.into()]);
    }, "missing-ap-color");
}

#[test]
fn highlight_admission_missing_appearance_refuses_opacity_rounding() {
    assert_missing_appearance_stays_native(|item| { item.set("CA", 0.501f32); }, "missing-ap-opacity");
}

#[test]
fn highlight_admission_missing_appearance_refuses_changed_clip_rectangle() {
    assert_missing_appearance_stays_native(|item| {
        let values = item.get_mut(b"Rect").unwrap().as_array_mut().unwrap();
        values[2] = Object::Real(values[2].as_float().unwrap() - 10.0);
    }, "missing-ap-rect");
}

fn repeated_metadata(key: &str, bytes: Vec<u8>, count: usize, name: bool) -> Vec<u8> {
    let rt = runtime();
    let mut pdf = Document::load_mem(&generated(&rt)).unwrap();
    let mut item = annotation(&pdf).clone();
    item.set(key, if name { Object::Name(bytes) } else { Object::String(bytes, lopdf::StringFormat::Literal) });
    let id = pdf.add_object(item);
    let page = pdf.get_pages()[&1];
    pdf.get_dictionary_mut(page).unwrap().set("Annots", vec![Object::Reference(id); count]);
    crate::pdf_output::save(&mut pdf).unwrap()
}

#[test]
fn highlight_admission_metadata_reserves_repeated_subtypes_before_copying() {
    let bytes = repeated_metadata("Subtype", vec![b'A'; 512 * 1024], 40, true);
    assert!(bytes.len() < 1024 * 1024, "small shared source must exercise import amplification");
    assert!(parse_annotation_metadata(&bytes).is_err(), "repeated names exceed cumulative metadata allowance");
}

#[test]
fn highlight_admission_metadata_reserves_native_note_conversion() {
    let bytes = repeated_metadata("Contents", vec![b'A'; 1024 * 1024], 10, false);
    assert!(parse_annotation_metadata(&bytes).is_err(), "native string conversion needs a cumulative reservation before contents()");
}

#[test]
fn highlight_admission_metadata_bounds_cumulative_portable_decode() {
    let mut record = vec![b' '; 256 * 1024];
    record[0] = b'{'; record[1] = b'}';
    let bytes = repeated_metadata("Folio", record, 129, false);
    assert!(parse_annotation_metadata(&bytes).is_err(), "failed decodes must not reuse the 32 MiB cumulative portable allowance");
}

#[test]
fn highlight_admission_metadata_bounds_json_nodes_before_tree_allocation() {
    let mut record = b"[".to_vec();
    record.extend_from_slice(b"0");
    for _ in 1..200_000 { record.extend_from_slice(b",0"); }
    record.push(b']');
    let bytes = repeated_metadata("Folio", record, 1, false);
    assert!(bytes.len() < 1024 * 1024);
    assert!(parse_annotation_metadata(&bytes).is_err(), "small JSON bytes must not authorize an oversized owned value tree");
}
#[test]
fn highlight_admission_renderer_equivalent_authored_appearance_remains_editable() {
    let mut rt=runtime();let bytes=generated(&rt);let parsed=Document::load_mem(&bytes).unwrap();
    let ap=parsed.dereference(annotation(&parsed).get(b"AP").unwrap()).unwrap().1.as_dict().unwrap();
    assert!(!parsed.dereference(ap.get(b"N").unwrap()).unwrap().1.as_stream().unwrap().content.is_empty());
    capture("equivalent-highlight",&bytes);
    let info=open(&mut rt,bytes.clone(),true);
    assert_eq!(info.pages[0].overlays.len(),1,"known renderer-equivalent artwork stays editable");
    assert_eq!(serde_json::to_value(rt.document(&info.id).unwrap().imported_overlay_sources.snapshot().unwrap()).unwrap()["bindings"].as_array().unwrap().len(),1);
    assert_eq!(rt.document(&info.id).unwrap().bytes().unwrap().as_ref(),bytes);
}
#[test]
fn highlight_admission_custom_paint_stays_native_and_visible() {
    let mut rt=runtime();let mut pdf=Document::load_mem(&generated(&rt)).unwrap();
    let mut annot=annotation(&pdf).clone();let ap=pdf.dereference(annot.get(b"AP").unwrap()).unwrap().1.as_dict().unwrap();
    let mut normal=pdf.dereference(ap.get(b"N").unwrap()).unwrap().1.as_stream().unwrap().clone();
    normal.dict.remove(b"Filter");normal.dict.remove(b"DecodeParms");normal.set_content(b"q 1 0 0 rg 0 0 300 400 re f Q".to_vec());
    let changed=pdf.add_object(normal);annot.set("AP",dictionary!{"N"=>changed});let page=pdf.get_pages()[&1];
    pdf.get_dictionary_mut(page).unwrap().set("Annots",vec![Object::Dictionary(annot)]);
    let bytes=crate::pdf_output::save(&mut pdf).unwrap();capture("custom-highlight",&bytes);
    crate::pdf_output::load(&bytes).expect("fixture must pass strict source parsing");
    let raw=open(&mut rt,bytes.clone(),false);let expected=rt.render_page(&raw.id,0,600).unwrap();
    let pixels=image::load_from_memory(&expected).unwrap().into_rgb8();
    assert!(pixels.pixels().filter(|p|p[0]>230&&p[1]<30&&p[2]<30).count()>100,"negative fixture must visibly paint custom red artwork");
    let info=open(&mut rt,bytes.clone(),true);
    assert!(info.pages[0].overlays.is_empty(),"unproved custom AP must not become a reconstructed yellow highlight");
    assert_eq!(rt.render_page(&info.id,0,600).unwrap(),expected,"original artwork must remain visible");
    assert_eq!(rt.document(&info.id).unwrap().bytes().unwrap().as_ref(),bytes);
    let plan=PagePlan{id:"page".into(),source_id:info.id.clone(),page_index:0,width:info.pages[0].width,height:info.pages[0].height,rotation:0,crop:None,overlays:vec![]};
    assert_eq!(rt.render_page_plan(plan.clone(),600).unwrap(),expected,"preview preserves original custom artwork");
    let request=ExportRequest{pages:vec![plan],flatten:false};
    let saved=rt.compose_pdf(request.clone(),None,CompositionIntent::Save).unwrap();
    capture("custom-highlight-saved",&saved);
    let reopened=open(&mut rt,saved,true);
    assert!(reopened.pages[0].overlays.is_empty());
    assert_eq!(rt.render_page(&reopened.id,0,600).unwrap(),expected,"saved/reopened custom artwork is unchanged");
    let printed=rt.prepare_print(request).unwrap();
    assert_eq!(rt.render_page(&printed.id,0,600).unwrap(),expected,"print preparation retains native artwork");
    // Force a real composition beside the native artwork, not only no-op Save.
    let added=HighlightOverlay{id:"added".into(),rects:vec![AnnotationRect{x:180.,y:180.,width:60.,height:20.}],color:"#00ff00".into(),text:None,opacity:Some(1.)};
    let expected_edited={
        let native=rt.pdfium.load_pdf_from_reader(Cursor::new(bytes.clone()),None).unwrap();
        {let mut page=native.pages().get(0).unwrap();let geometry=page_geometry(&page).unwrap();add_highlight_annotation(&mut page,geometry,&added).unwrap();page.render_with_config(&PdfRenderConfig::new().set_target_width(1).render_annotations(true)).unwrap();}
        crate::pdf_output::from_pdfium(&native).unwrap()
    };
    let plan=PagePlan{id:"edited".into(),source_id:info.id.clone(),page_index:0,width:300.,height:400.,rotation:0,crop:None,overlays:vec![Overlay::Highlight(added)]};
    let edited=rt.compose_pdf(ExportRequest{pages:vec![plan],flatten:false},None,CompositionIntent::Save).unwrap();
    assert_ne!(edited,bytes,"added overlay must force composition");
    let parsed=crate::pdf_output::load(&edited).unwrap();
    assert!(parsed.get_page_annotations(parsed.get_pages()[&1]).unwrap().iter().all(|annotation|annotation.has(b"AP")),"saved highlights must carry explicit appearances for other readers");
    capture("custom-highlight-edited",&edited);capture("custom-highlight-edited-reference",&expected_edited);
    let desired=open(&mut rt,expected_edited,false);let actual=open(&mut rt,edited.clone(),false);
    let desired_pixels=rt.render_page(&desired.id,0,600).unwrap();
    assert_ne!(desired_pixels,expected,"positive control must paint the new green highlight");
    assert_eq!(rt.render_page(&actual.id,0,600).unwrap(),desired_pixels,"real edited Save must preserve old artwork and paint the new overlay");
    let reopened=open(&mut rt,edited,true);assert_eq!(reopened.pages[0].overlays.len(),1,"only the new standard highlight is editable");
    assert_eq!(rt.document(&info.id).unwrap().bytes().unwrap().as_ref(),bytes);
    // Exercise the public image-export worker, including its actual PNG encoder.
    let temp=tempfile::tempdir().unwrap();let source=temp.path().join("custom.pdf");std::fs::write(&source,&bytes).unwrap();
    let engine=PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap();
    let info=engine.open_document(&source).unwrap();assert!(info.pages[0].overlays.is_empty());
    let plan=PagePlan{id:"image".into(),source_id:info.id,page_index:0,width:300.,height:400.,rotation:0,crop:None,overlays:vec![]};
    use crate::page_image_export::{PageImageExportRequest,PageImageOptions,PageImageFormat,PageImageControl};
    let exported=engine.export_page_images(PageImageExportRequest{pages:vec![plan],page_numbers:vec![1],workspace_page_count:1,options:PageImageOptions{format:PageImageFormat::Png,dpi:72,jpeg_quality:90}},temp.path(),Arc::new(PageImageControl::new()),Arc::new(|_|{})).unwrap().unwrap();
    let png=image::open(exported.folder.join("page-0001.png")).unwrap().into_rgb8();
    let expected=image::load_from_memory(&rt.render_page(&raw.id,0,300).unwrap()).unwrap().into_rgb8();
    assert_eq!(png,expected,"public PNG export preserves custom artwork");
    assert_eq!(std::fs::read(source).unwrap(),bytes);
}

#[test]
fn highlight_admission_changed_frames_resources_and_states_stay_native() {
    let mut rt=runtime();let original=generated(&rt);
    for mode in ["bbox","matrix","rect","blend","rollover","state"] {
        let mut pdf=Document::load_mem(&original).unwrap();let mut annot=annotation(&pdf).clone();
        let ap=pdf.dereference(annot.get(b"AP").unwrap()).unwrap().1.as_dict().unwrap();
        let mut normal=pdf.dereference(ap.get(b"N").unwrap()).unwrap().1.as_stream().unwrap().clone();
        match mode {
            "bbox"=>normal.dict.set("BBox",vec![0.into(),0.into(),200.into(),400.into()]),
            "matrix"=>normal.dict.set("Matrix",vec![1.into(),0.into(),0.into(),1.into(),10.into(),0.into()]),
            "rect"=>annot.set("Rect",vec![20.into(),300.into(),120.into(),320.into()]),
            "blend"=>normal.dict.set("Resources",dictionary!{"ExtGState"=>dictionary!{"GS"=>dictionary!{"Type"=>"ExtGState","AIS"=>false,"BM"=>"Normal","CA"=>0.5019608f32,"ca"=>0.5019608f32}}}),
            "state"=>annot.set("AS",Object::Name(b"Other".to_vec())),
            "rollover"=>(),
            _=>unreachable!(),
        }
        let changed=pdf.add_object(normal);let mut appearance=dictionary!{"N"=>changed};
        if mode=="rollover" { appearance.set("R",changed); }
        annot.set("AP",appearance);
        let page=pdf.get_pages()[&1];pdf.get_dictionary_mut(page).unwrap().set("Annots",vec![Object::Dictionary(annot)]);
        let bytes=crate::pdf_output::save(&mut pdf).unwrap();capture(&format!("highlight-{mode}"),&bytes);
        crate::pdf_output::load(&bytes).expect("fixture must pass strict source parsing");
    let raw=open(&mut rt,bytes.clone(),false);let expected=rt.render_page(&raw.id,0,600).unwrap();
        let info=open(&mut rt,bytes.clone(),true);
        assert!(info.pages[0].overlays.is_empty(),"{mode} must remain native");
        assert_eq!(rt.render_page(&info.id,0,600).unwrap(),expected,"{mode} native pixels");
        assert_eq!(rt.document(&info.id).unwrap().bytes().unwrap().as_ref(),bytes);
    }
}

#[test]
fn highlight_admission_equivalent_indirect_resources_and_uncompressed_stream_import() {
    let mut rt=runtime();let mut pdf=Document::load_mem(&generated(&rt)).unwrap();
    let mut annot=annotation(&pdf).clone();let ap=pdf.dereference(annot.get(b"AP").unwrap()).unwrap().1.as_dict().unwrap();
    let mut normal=pdf.dereference(ap.get(b"N").unwrap()).unwrap().1.as_stream().unwrap().clone();
    normal.set_content(normal.decompressed_content().unwrap());normal.dict.remove(b"Filter");
    let resources=normal.dict.get(b"Resources").unwrap().clone();let resource_id=pdf.add_object(resources);
    let alias=pdf.add_object(Object::Reference(resource_id));normal.dict.set("Resources",alias);
    let normal=pdf.add_object(normal);let appearance=pdf.add_object(dictionary!{"N"=>normal});annot.set("AP",appearance);
    let page=pdf.get_pages()[&1];pdf.get_dictionary_mut(page).unwrap().set("Annots",vec![Object::Dictionary(annot)]);
    let bytes=crate::pdf_output::save(&mut pdf).unwrap();capture("equivalent-indirect-highlight",&bytes);
    let info=open(&mut rt,bytes.clone(),true);
    assert_eq!(info.pages[0].overlays.len(),1,"transport and reference changes preserve equivalent appearance admission");
    assert_eq!(rt.document(&info.id).unwrap().bytes().unwrap().as_ref(),bytes);
}

#[test]
fn highlight_admission_equivalent_rotated_and_scaled_appearances_import() {
    let mut rt=runtime();
    for rotation in [0,90,180,270] { for unit in [1.0,1.7] {
        let bytes=generated_on(&rt,rotation,unit);
        crate::pdf_output::load(&bytes).unwrap();
        let info=open(&mut rt,bytes.clone(),true);
        assert_eq!(info.pages[0].overlays.len(),1,"rotation {rotation} unit {unit}");
        assert_eq!(rt.document(&info.id).unwrap().bytes().unwrap().as_ref(),bytes);
        let mut rotated=crate::pdf_output::load(&bytes).unwrap();let page=rotated.get_pages()[&1];
        rotated.get_dictionary_mut(page).unwrap().set("Rotate",i64::from((rotation+90)%360));
        let rotated=crate::pdf_output::save(&mut rotated).unwrap();
        let info=open(&mut rt,rotated,true);
        assert_eq!(info.pages[0].overlays.len(),1,"workspace rotation keeps original native AP traversal editable: source {rotation}, unit {unit}");
    }}
}

#[test]
fn highlight_admission_publication_refuses_wrong_target_and_cancellation() {
    let rt=runtime();let bytes=generated(&rt);let mut doc=crate::pdf_output::load(&bytes).unwrap();
    let before=doc.objects.clone();
    let highlight=HighlightOverlay{id:"wrong".into(),rects:vec![AnnotationRect{x:20.,y:80.,width:80.,height:20.}],color:"#00ff00".into(),text:None,opacity:Some(0.5)};
    let request=ExportRequest{pages:vec![PagePlan{id:"p".into(),source_id:"unused".into(),page_index:0,width:300.,height:400.,rotation:0,crop:None,overlays:vec![Overlay::Highlight(highlight)]}],flatten:false};
    let geometry=PageGeometry{left:0.,bottom:0.,right:300.,top:400.,rotation:0};
    let mut publication=highlight_appearance::Publication::default();publication.record(0,0,0).unwrap();
    assert!(publication.apply(rt.pdfium,&mut doc,&request,&[geometry],&mut ||Ok(())).is_err());
    assert_eq!(doc.objects,before,"shape mismatch must not alter the target document");
    let mut publication=highlight_appearance::Publication::default();publication.record(0,0,0).unwrap();
    assert!(publication.apply(rt.pdfium,&mut doc,&request,&[geometry],&mut ||Err(EngineError::InvalidRequest("cancelled".into()))).is_err());
    assert_eq!(doc.objects,before,"early cancellation must not alter the target document");
}
