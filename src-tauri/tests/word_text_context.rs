use folio_engine::{DocumentInfo, ExportRequest, PagePlan, PdfEngine};
use lopdf::{dictionary, Document, Object, Stream};
use std::{fs, path::PathBuf};

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll")).unwrap()
}
fn plans(source: &DocumentInfo) -> Vec<PagePlan> {
    source.pages.iter().enumerate().map(|(i,p)|PagePlan {id:format!("p{i}"),source_id:source.id.clone(),page_index:i,width:p.width,height:p.height,rotation:0,crop:None,overlays:p.overlays.clone()}).collect()
}
fn fixture(tagged: bool, clip: &str, rotation: i64, unit: f32) -> Document {
    let mut pdf=Document::with_version("1.7");let tree=pdf.new_object_id();
    let font=pdf.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let group=pdf.add_object(dictionary! {"Type"=>"Group","S"=>"Transparency","CS"=>"DeviceRGB"});
    let mut pages=vec![];
    for index in 0..2 {
        let mark=|id|if tagged {format!("/Span << /MCID {id} /Lang (en-US) >> BDC")}else{String::new()};
        let close=if tagged {"EMC"}else{""};
        let text=if index==0 {"Old content"}else{"Second page"};
        let content=format!("{} q {clip} BT /F1 18 Tf 1 0 0 1 40 300 Tm ({text}) Tj ET Q {close} {} q 0 0 600 400 re W* n BT /F1 18 Tf 1 0 0 1 40 150 Tm (Neighbor) Tj ET Q {close}",mark(0),mark(1));
        let stream=pdf.add_object(Stream::new(dictionary!{},content.into_bytes()));
        let page=pdf.add_object(dictionary! {"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),600.into(),400.into()],"CropBox"=>vec![10.into(),10.into(),590.into(),390.into()],"Rotate"=>rotation,"UserUnit"=>Object::Real(unit),"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}},"Contents"=>stream,"Group"=>group});
        if tagged {pdf.get_dictionary_mut(page).unwrap().set("StructParents",index);}
        pages.push(page);
    }
    pdf.objects.insert(tree,dictionary! {"Type"=>"Pages","Count"=>2,"Kids"=>pages.iter().copied().map(Object::Reference).collect::<Vec<_>>()}.into());
    let root=pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>tree,"Lang"=>Object::string_literal("en-US")});pdf.trailer.set("Root",root);
    let link=pdf.add_object(dictionary! {"Type"=>"Annot","Subtype"=>"Link","Rect"=>vec![400.into(),50.into(),500.into(),80.into()],"Border"=>vec![0.into(),0.into(),0.into()],"Dest"=>vec![pages[1].into(),Object::Name(b"Fit".to_vec())]});
    pdf.get_dictionary_mut(pages[0]).unwrap().set("Annots",vec![Object::Reference(link)]);
    if tagged {
        let structure=pdf.new_object_id();let document=pdf.new_object_id();let mut owners=vec![];let mut nums=vec![];
        for (index,page) in pages.iter().enumerate() {
            let mut slots=vec![];
            for mcid in 0..2 {
                let owner=pdf.add_object(dictionary! {"Type"=>"StructElem","S"=>"P","P"=>document,"Pg"=>*page,"K"=>mcid});
                owners.push(Object::Reference(owner));slots.push(Object::Reference(owner));
            }
            let slots=pdf.add_object(Object::Array(slots));nums.extend([Object::Integer(index as i64),Object::Reference(slots)]);
        }
        pdf.objects.insert(document,dictionary! {"Type"=>"StructElem","S"=>"Document","P"=>structure,"K"=>owners}.into());
        let parent_tree=pdf.add_object(dictionary! {"Nums"=>nums});
        pdf.objects.insert(structure,dictionary! {"Type"=>"StructTreeRoot","K"=>document,"ParentTree"=>parent_tree}.into());
        let catalog=pdf.get_dictionary_mut(root).unwrap();catalog.set("StructTreeRoot",structure);catalog.set("MarkInfo",dictionary! {"Marked"=>true});
    }
    pdf
}
fn opened(engine:&PdfEngine,temp:&tempfile::TempDir,name:&str,pdf:&mut Document)->DocumentInfo {
    let path=temp.path().join(name);pdf.save(&path).unwrap();engine.open_document(path).unwrap()
}
fn assert_graph(before:&Document,after:&Document) {
    assert_eq!(before.get_pages(),after.get_pages());
    assert_eq!(before.catalog().unwrap(),after.catalog().unwrap());
    for (id,obj) in &before.objects {
        let preserve=match obj {
            Object::Dictionary(d)=>d.get(b"Type").and_then(Object::as_name).is_ok_and(|n|matches!(n,b"StructElem"|b"StructTreeRoot"|b"Group"|b"Annot")) || d.has(b"Nums"),
            _=>false,
        };
        if preserve {assert_eq!(after.get_object(*id).unwrap(),obj,"object {id:?}");}
    }
    // Structure arrays must retain identity. Font resource arrays may legitimately
    // be replaced with equivalent imported font resources and become unreachable.
    fn structure_graph(before:&Document,after:&Document,value:&Object,seen:&mut std::collections::HashSet<(u32,u16)>) {
        let value=if let Object::Reference(id)=value {
            if !seen.insert(*id) {return;}
            let value=before.get_object(*id).unwrap();assert_eq!(after.get_object(*id).unwrap(),value,"structure {id:?}");value
        }else{value};
        match value {
            Object::Array(values)=>for value in values {structure_graph(before,after,value,seen);},
            Object::Dictionary(d)=>for key in [b"K".as_slice(),b"ParentTree",b"Nums",b"Kids"] {if let Ok(value)=d.get(key) {structure_graph(before,after,value,seen);}},
            _=>{},
        }
    }
    if let Ok(root)=before.catalog().unwrap().get(b"StructTreeRoot") {structure_graph(before,after,root,&mut std::collections::HashSet::new());}
    for id in before.get_pages().values() {
        let a=before.get_dictionary(*id).unwrap();let b=after.get_dictionary(*id).unwrap();
        for key in [b"StructParents".as_slice(),b"Group",b"Rotate",b"UserUnit",b"MediaBox",b"CropBox",b"Annots"] {assert_eq!(a.get(key).ok(),b.get(key).ok(),"{key:?}");}
    }
}
fn capture(name:&str,bytes:&[u8]) {
    if let Some(folder)=std::env::var_os("FOLIO_WORD_CONTEXT_OUTPUTS") {let folder=PathBuf::from(folder);fs::create_dir_all(&folder).unwrap();fs::write(folder.join(name),bytes).unwrap();}
}

fn paragraph_request(selection:&folio_engine::StyledParagraphSelection,text:&str)->folio_engine::StyledParagraphLayoutRequest {
    serde_json::from_value(serde_json::json!({"objectIndices":selection.object_indices,"expectedText":selection.text,
        "runs":[{"text":text,"styleId":0}],"bounds":{"x":30.,"y":65.,"width":220.,"height":110.},
        "alignment":"left","lineSpacing":1.2})).unwrap()
}

fn font_context_case(tagged:bool,state:&str,show:&str,dependent:bool) {
    font_context_case_at(tagged,state,show,dependent,0,1.);
}
fn font_context_case_at(tagged:bool,state:&str,show:&str,dependent:bool,rotation:i64,unit:f32) {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let font=engine.fonts().register(fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
    let mut pdf=fixture(tagged,"20 200 500 150 re W n",rotation,unit);
    let page=*pdf.get_pages().get(&1).unwrap();
    let original=String::from_utf8(pdf.get_page_content(page)).unwrap();
    let tail=if dependent {" [-4000] TJ (Suffix) Tj"}else{""};
    let content=original.replacen("(Old content) Tj",&format!("{state} {show}{tail}"),1);
    let stream=pdf.add_object(Stream::new(dictionary!{},content.into_bytes()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);
    let source=opened(&engine,&temp,"font-context.pdf",&mut pdf);
    let original_bytes=engine.source_bytes(&source.id).unwrap();
    // Compare the actual parsed input graph; saving canonicalizes Real(1) to Integer(1).
    let graph=Document::load_mem(&original_bytes).unwrap();
    let before=engine.list_occurrence_text_runs(&source.id,0).unwrap().runs;
    let selected=before.iter().find(|r|r.text.starts_with("Old content")).unwrap();
    let replacement="New content";
    let changed=engine.replace_text_occurrence(&source.id,&[0,1],0,selected.object_index,&selected.text,replacement,Some(&font.id))
        .unwrap_or_else(|e|panic!("tagged={tagged} state={state} show={show} dependent={dependent}: {e}"));
    let after=engine.list_occurrence_text_runs(&changed.document.id,0).unwrap().runs;
    let edited=after.iter().find(|r|r.object_index==selected.object_index).unwrap();
    assert_eq!(edited.source_text.as_deref().unwrap_or(&edited.text),replacement);
    assert_eq!(edited.font_size,selected.font_size);assert_ne!(edited.font_name,selected.font_name);
    assert!(edited.supported && edited.can_substitute,"saved run must remain editable: {edited:?}");
    assert!(selected.can_substitute,"discovery must advertise admitted source substitution");
    for old in before.iter().filter(|r|r.object_index!=selected.object_index) {
        let new=after.iter().find(|r|r.object_index==old.object_index).unwrap();
        assert_eq!(new.text,old.text);assert_eq!(new.font_name,old.font_name);assert_eq!(new.font_size,old.font_size);
        for (a,b) in [new.bounds.x,new.bounds.y,new.bounds.width,new.bounds.height].into_iter().zip([old.bounds.x,old.bounds.y,old.bounds.width,old.bounds.height]) {
            assert!((a-b).abs()<=0.0001,"dependent cursor exceeded floating-point serialization tolerance: {a} vs {b}");
        }
    }
    let original_image=image::load_from_memory(&engine.render_page(&source.id,0,1200).unwrap()).unwrap().to_rgb8();
    let changed_image=image::load_from_memory(&engine.render_page(&changed.document.id,0,1200).unwrap()).unwrap().to_rgb8();
    assert_eq!(original_image.dimensions(),changed_image.dimensions());
    let sx=original_image.width() as f32/source.pages[0].width;let sy=original_image.height() as f32/source.pages[0].height;
    let left=selected.bounds.x.min(edited.bounds.x)*sx-3.;let right=(selected.bounds.x+selected.bounds.width).max(edited.bounds.x+edited.bounds.width)*sx+3.;
    let top=selected.bounds.y.min(edited.bounds.y)*sy-3.;let bottom=(selected.bounds.y+selected.bounds.height).max(edited.bounds.y+edited.bounds.height)*sy+3.;
    let mut differences=0;
    for (x,y,pixel) in original_image.enumerate_pixels(){if pixel!=changed_image.get_pixel(x,y){
        differences+=1;assert!(x as f32>=left&&x as f32<=right&&y as f32>=top&&y as f32<=bottom,"changed pixel outside selected run: {x},{y}");
    }}
    assert!(differences>20,"changed text/font must produce an actual edit");
    let bytes=engine.source_bytes(&changed.document.id).unwrap();assert_graph(&graph,&Document::load_mem(&bytes).unwrap());
    assert_eq!(engine.render_page(&source.id,1,800).unwrap(),engine.render_page(&changed.document.id,1,800).unwrap());
    let saved=temp.path().join("saved.pdf");engine.export_pdf(ExportRequest{pages:plans(&changed.document),flatten:false},&saved).unwrap();
    let reopened=engine.open_document(&saved).unwrap();
    let run=engine.list_occurrence_text_runs(&reopened.id,0).unwrap().runs.into_iter().find(|r|r.object_index==selected.object_index).unwrap();
    // Re-edit using the embedded program, after removing the external registration.
    engine.fonts().remove(&font.id).unwrap();
    // Growth without a font request uses the pre-existing positioned-edit path;
    // its dependent-cursor limitations are separate from this substitution test.
    let again=if dependent {replacement}else{"New contents"};
    let reedited=engine.replace_text_occurrence(&reopened.id,&[0,1],0,run.object_index,&run.text,again,None).unwrap();
    assert_graph(&graph,&Document::load_mem(&engine.source_bytes(&reedited.document.id).unwrap()).unwrap());
    assert_eq!(engine.source_bytes(&source.id).unwrap(),original_bytes);
    capture(&format!("font-context-{tagged}-{}.pdf",if dependent{"dependent"}else{"independent"}),&bytes);
}

#[test]
fn font_context_substitution_preserves_positioned_shows_and_tagged_graphs() {
    for tagged in [false,true] {
        for (state,show) in [("","(Old content) Tj"),("0.5 Tc","[(Old) -20 ( content) 15] TJ"),("1 Tw 0.25 Tc","[(Old ) 12 (content)] TJ")] {
            font_context_case(tagged,state,show,false);
        }
    }
}

#[test]
fn font_context_substitution_preserves_dependent_following_text() {
    for tagged in [false,true] {font_context_case(tagged,"0.5 Tc","[(Old) -20 ( content) 15] TJ",true);}
}

#[test]
fn saved_dependent_text_grows_without_external_font_and_keeps_following_text() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let font=engine.fonts().register(fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
    let mut pdf=fixture(true,"20 200 500 150 re W n",0,1.);
    let page=*pdf.get_pages().get(&1).unwrap();
    let data=String::from_utf8(pdf.get_page_content(page)).unwrap()
        .replacen("(Old content) Tj","0.5 Tc 1 Tw [(Old) -20 ( content) 15] TJ [-4000] TJ (Suffix) Tj",1);
    let stream=pdf.add_object(Stream::new(dictionary!{},data.into_bytes()));
    pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);
    let source=opened(&engine,&temp,"dependent-source.pdf",&mut pdf);
    let original=engine.source_bytes(&source.id).unwrap();
    let graph=Document::load_mem(&original).unwrap();
    let selected=engine.list_occurrence_text_runs(&source.id,0).unwrap().runs.remove(0);
    let changed=engine.replace_text_occurrence(&source.id,&[0,1],0,selected.object_index,&selected.text,"New content",Some(&font.id)).unwrap();
    let path=temp.path().join("dependent-saved.pdf");
    engine.export_pdf(ExportRequest{pages:plans(&changed.document),flatten:false},&path).unwrap();
    let mut saved=engine.open_document(&path).unwrap();
    engine.fonts().remove(&font.id).unwrap();
    let saved_bytes=engine.source_bytes(&saved.id).unwrap();
    capture("dependent-embedded-source.pdf",&saved_bytes);
    let saved_graph=Document::load_mem(&saved_bytes).unwrap();
    let saved_ops=lopdf::content::Content::decode(&saved_graph.get_page_content(page)).unwrap().operations.len();
    let suffix=engine.list_occurrence_text_runs(&saved.id,0).unwrap().runs[1].bounds.clone();
    for (cycle,replacement) in ["New contents","New","New content","New contents","New","New content"].into_iter().enumerate() {
        let before=engine.list_occurrence_text_runs(&saved.id,0).unwrap().runs;
        let run=&before[0];
        let edited=engine.replace_text_occurrence(&saved.id,&[0,1],0,run.object_index,&run.text,replacement,None)
            .expect("saved dependent text must grow without its external font");
        let after=engine.list_occurrence_text_runs(&edited.document.id,0).unwrap().runs;
        assert_eq!(after[0].source_text.as_deref().unwrap_or(&after[0].text),replacement);
        assert_eq!(after[0].font_name,run.font_name);assert_eq!(after[0].font_size,run.font_size);
        for old in &before[1..] {
            let new=after.iter().find(|r|r.object_index==old.object_index).unwrap();
            assert_eq!(new.text,old.text);assert_eq!(new.font_name,old.font_name);
            for (a,b) in [new.bounds.x,new.bounds.y,new.bounds.width,new.bounds.height].into_iter().zip([old.bounds.x,old.bounds.y,old.bounds.width,old.bounds.height]) {
                assert!((a-b).abs()<=0.0001,"following text moved: {a} vs {b}");
            }
        }
        let a=image::load_from_memory(&engine.render_page(&saved.id,0,1200).unwrap()).unwrap().to_rgb8();
        let b=image::load_from_memory(&engine.render_page(&edited.document.id,0,1200).unwrap()).unwrap().to_rgb8();
        let bounds=&after[0].bounds;
        let sx=a.width() as f32/saved.pages[0].width;let sy=a.height() as f32/saved.pages[0].height;
        let left=run.bounds.x.min(bounds.x)*sx-3.;let right=(run.bounds.x+run.bounds.width).max(bounds.x+bounds.width)*sx+3.;
        let top=run.bounds.y.min(bounds.y)*sy-3.;let bottom=(run.bounds.y+run.bounds.height).max(bounds.y+bounds.height)*sy+3.;
        let mut changes=0;
        for (x,y,pixel) in a.enumerate_pixels(){if pixel!=b.get_pixel(x,y){changes+=1;
            assert!(x as f32>=left&&x as f32<=right&&y as f32>=top&&y as f32<=bottom,"unselected pixel changed: {x},{y}");
        }}
        assert!(changes>20);
        let bytes=engine.source_bytes(&edited.document.id).unwrap();
        let output=Document::load_mem(&bytes).unwrap();
        assert_graph(&graph,&output);
        assert_eq!(engine.render_page(&saved.id,1,800).unwrap(),engine.render_page(&edited.document.id,1,800).unwrap());
        assert_eq!(engine.source_bytes(&source.id).unwrap(),original);
        for (a,b) in [after[1].bounds.x,after[1].bounds.y].into_iter().zip([suffix.x,suffix.y]){assert!((a-b).abs()<=0.0001,"cumulative cursor drift");}
        assert_eq!(lopdf::content::Content::decode(&output.get_page_content(page)).unwrap().operations.len(),saved_ops,"correction chain grew");
        let saved_path=temp.path().join(format!("dependent-cycle-{cycle}.pdf"));
        engine.export_pdf(ExportRequest{pages:plans(&edited.document),flatten:false},&saved_path).unwrap();
        capture(&format!("dependent-embedded-cycle-{cycle}.pdf"),&fs::read(&saved_path).unwrap());
        engine.close_document(&saved.id).unwrap();engine.close_document(&edited.document.id).unwrap();
        saved=engine.open_document(&saved_path).unwrap();
    }
    let run=engine.list_occurrence_text_runs(&saved.id,0).unwrap().runs.remove(0);
    for (expected,text) in [("stale","New"),(run.text.as_str(),"WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW"),(run.text.as_str(),"New 漢")] {
        assert!(engine.replace_text_occurrence(&saved.id,&[0,1],0,run.object_index,expected,text,None).is_err());
    }
}

#[test]
fn dependent_source_text_spacing_matches_hand_positioned_reference() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    // Courier is 600 units per glyph. At 18pt, Tc=.5, Tw=1, the
    // 11-character source with one space advances 125.3pt. The following
    // -4000 TJ adds 72pt, so Suffix must start at x=237.3, independently
    // of the replacement. A quote moves the baseline from 324 to 300.
    for (name,setup,show,replacement,reference,suffix_x) in [
        ("tj-grow","0.5 Tc 1 Tw","(Old content) Tj","New text here","(New text here) Tj","237.3"),
        ("tj-shrink","0.5 Tc 1 Tw","(Old content) Tj","Short","(Short) Tj","237.3"),
        ("quote-grow","24 TL 0 24 Td 0.5 Tc 1 Tw","(Old content) '","New text here","(New text here) Tj","237.3"),
        ("double-quote-grow","24 TL 0 24 Td","1 0.5 (Old content) \"","New text here","(New text here) Tj","237.3"),
        // Net -5 thousandths of 18pt adds .09pt to the source cursor.
        ("array-grow","0.5 Tc 1 Tw","[(Old) -20 ( content) 15] TJ","New text here","[(New) -20 ( text he) 15 (re)] TJ","237.39"),
        ("array-shrink","0.5 Tc 1 Tw","[(Old) -20 ( content) 15] TJ","Short","[(Sho) -20 (rt)] TJ","237.39"),
    ] {
        let mut source_pdf=fixture(true,"20 200 500 150 re W n",0,1.);
        for obj in source_pdf.objects.values_mut(){if let Ok(d)=obj.as_dict_mut(){if d.has(b"BaseFont"){d.set("BaseFont","Courier");}}}
        let page=*source_pdf.get_pages().get(&1).unwrap();
        let data=String::from_utf8(source_pdf.get_page_content(page)).unwrap();
        let original=data.replacen("(Old content) Tj",&format!("{setup} {show} [-4000] TJ (Suffix) Tj 24 TL T* (Next line) Tj"),1);
        let stream=source_pdf.add_object(Stream::new(dictionary!{},original.into_bytes()));source_pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);
        let mut expected=source_pdf.clone();
        // Reset suffix position explicitly, then restore the original line
        // origin for Next line. No production advance/correction helper is used.
        let golden=data.replacen("(Old content) Tj",&format!("0.5 Tc 1 Tw {reference} 1 0 0 1 {suffix_x} 300 Tm (Suffix) Tj 1 0 0 1 40 276 Tm (Next line) Tj"),1);
        let stream=expected.add_object(Stream::new(dictionary!{},golden.into_bytes()));expected.get_dictionary_mut(page).unwrap().set("Contents",stream);
        let source=opened(&engine,&temp,&format!("{name}.pdf"),&mut source_pdf);
        let control=opened(&engine,&temp,&format!("{name}-reference.pdf"),&mut expected);
        let run=engine.list_occurrence_text_runs(&source.id,0).unwrap().runs.remove(0);
        let result=engine.replace_text_occurrence(&source.id,&[0,1],0,run.object_index,&run.text,replacement,None).unwrap_or_else(|e|panic!("{name}: {e}"));
        let actual=engine.page_text(&result.document.id,0).unwrap();let want=engine.page_text(&control.id,0).unwrap();
        assert_eq!(actual.characters.len(),want.characters.len(),"{name}");
        for (a,b) in actual.characters.iter().zip(&want.characters){
            assert_eq!(a.text,b.text,"{name}");
            for (x,y) in [a.x,a.y,a.width,a.height].into_iter().zip([b.x,b.y,b.width,b.height]){assert!((x-y).abs()<0.0001,"{name}: glyph {} {x} vs {y}",a.text);}
        }
        assert_eq!(engine.render_page(&result.document.id,0,1200).unwrap(),engine.render_page(&control.id,0,1200).unwrap(),"{name}");
        capture(&format!("dependent-{name}-source.pdf"),&engine.source_bytes(&source.id).unwrap());
        capture(&format!("dependent-{name}-reference.pdf"),&engine.source_bytes(&control.id).unwrap());
        capture(&format!("dependent-{name}-edited.pdf"),&engine.source_bytes(&result.document.id).unwrap());
        for id in [source.id,control.id,result.document.id]{engine.close_document(&id).unwrap();}
    }
}

#[test]
fn source_font_context_edit_keeps_pages_with_many_unused_font_aliases_editable() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let mut pdf=fixture(true,"20 200 500 150 re W n",0,1.);
    let page=*pdf.get_pages().get(&1).unwrap();
    let resources=pdf.get_dictionary_mut(page).unwrap().get_mut(b"Resources").unwrap().as_dict_mut().unwrap();
    let fonts=resources.get_mut(b"Font").unwrap().as_dict_mut().unwrap();
    let font=fonts.get(b"F1").unwrap().clone();
    for n in 0..65 {fonts.set(format!("Unused{n}"),font.clone());}
    let source=opened(&engine,&temp,"many-font-aliases.pdf",&mut pdf);
    let run=engine.list_occurrence_text_runs(&source.id,0).unwrap().runs.remove(0);
    let edited=engine.replace_text_occurrence(&source.id,&[0,1],0,run.object_index,&run.text,"New content",None)
        .expect("source-font edits must not acquire the substitution font-search limit");
    assert_eq!(engine.list_occurrence_text_runs(&edited.document.id,0).unwrap().runs[0].text,"New content");
}

#[test]
fn font_context_quote_operators_retain_line_motion_and_spacing() {
    for tagged in [false,true] {for show in ["(Old content) '","1 0.25 (Old content) \""] {
        font_context_case(tagged,"24 TL 0 24 Td",show,false);
    }}
}

#[test]
fn font_context_repeated_substitutions_reuse_resources_and_state() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let font=engine.fonts().register(fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
    let source=opened(&engine,&temp,"repeat-font.pdf",&mut fixture(true,"20 200 500 150 re W n",0,1.));
    let original=engine.source_bytes(&source.id).unwrap();let graph=Document::load_mem(&original).unwrap();
    let mut current=source.clone();let mut counts=None;
    for cycle in 0..6 {
        let run=engine.list_occurrence_text_runs(&current.id,0).unwrap().runs.into_iter().find(|r|r.object_index==0).unwrap();
        let text=if cycle%2==0{"New content"}else{"New contents"};
        let changed=engine.replace_text_occurrence(&current.id,&[0,1],0,run.object_index,&run.text,text,Some(&font.id)).unwrap();
        let bytes=engine.source_bytes(&changed.document.id).unwrap();let pdf=Document::load_mem(&bytes).unwrap();assert_graph(&graph,&pdf);
        let page=*pdf.get_pages().get(&1).unwrap();
        let ops=lopdf::content::Content::decode(&pdf.get_page_content(page)).unwrap().operations;
        let resources=pdf.get_dictionary(page).unwrap().get(b"Resources").unwrap();let resources=pdf.dereference(resources).unwrap().1.as_dict().unwrap();
        let fonts=pdf.dereference(resources.get(b"Font").unwrap()).unwrap().1.as_dict().unwrap();
        let now=(ops.len(),fonts.len());if let Some(expected)=counts{assert_eq!(now,expected,"resource/operator growth at cycle{cycle}");}else{counts=Some(now);}
        assert_eq!(engine.render_page(&source.id,1,800).unwrap(),engine.render_page(&changed.document.id,1,800).unwrap());
        if current.id!=source.id{engine.close_document(&current.id).unwrap();}current=changed.document;
    }
    assert_eq!(engine.source_bytes(&source.id).unwrap(),original);
}

#[test]
fn styled_replay_keeps_paths_and_pending_clips_that_survive_graphics_restore() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    for (case,clip) in [
        "q 0 0 600 400 re Q W n",
        "q 0 0 600 400 re W Q n",
        "q 0 0 600 400 re Q W q n Q",
        "0 0 600 400 re q W n Q",
    ].iter().enumerate() {
        let mut pdf=fixture(false,"",0,1.);let page=*pdf.get_pages().get(&1).unwrap();
        let contents=format!("{clip} BT /F1 18 Tf 1 0 0 1 40 300 Tm (Old content) Tj ET BT /F1 18 Tf 1 0 0 1 40 150 Tm (Neighbor) Tj ET");
        let stream=pdf.add_object(Stream::new(dictionary!{},contents.into_bytes()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);
        let source=opened(&engine,&temp,&format!("escaping-path-{case}.pdf"),&mut pdf);
        let original=engine.source_bytes(&source.id).unwrap();let mut current=source.clone();let mut count=None;
        for cycle in 0..4 {
            let runs=engine.list_occurrence_text_runs(&current.id,0).unwrap().runs;
            let selection=engine.suggest_occurrence_styled_paragraph(&current.id,0,runs[0].object_index,&runs[0].text).unwrap();
            let text=if cycle%2==0 {"Old content this\nSecond line"}else{"Old content this\nAnother line"};
            let result=engine.preview_styled_paragraph_occurrence(&current.id,&[0,1],0,&paragraph_request(&selection,text)).unwrap();
            let saved=temp.path().join(format!("escaping-path-{case}-{cycle}.pdf"));
            engine.export_pdf(ExportRequest{pages:plans(&result.document),flatten:false},&saved).unwrap();
            let bytes=fs::read(&saved).unwrap();let pdf=Document::load_mem(&bytes).unwrap();
            let ops=lopdf::content::Content::decode(&pdf.get_page_content(*pdf.get_pages().get(&1).unwrap())).unwrap().operations.len();
            if let Some(count)=count {assert_eq!(ops,count,"escaped-path replay accumulated operations");}count=Some(ops);
            let reopened=engine.open_document(saved).unwrap();let after=engine.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;
            let before=runs.iter().find(|r|r.text=="Neighbor").unwrap();let neighbor=after.iter().find(|r|r.text=="Neighbor").unwrap();assert_eq!(neighbor.bounds,before.bounds);
            assert_eq!(engine.suggest_occurrence_styled_paragraph(&reopened.id,0,after[0].object_index,&after[0].text).unwrap().text,text);
            engine.close_document(&result.document.id).unwrap();if current.id!=source.id {engine.close_document(&current.id).unwrap();}current=reopened;
        }
        assert_eq!(engine.source_bytes(&source.id).unwrap(),original);engine.close_document(&current.id).unwrap();engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn clipped_styled_paragraph_growth_and_saved_reedits_preserve_context_without_wrapper_growth() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    for tagged in [false,true] {
        let source=opened(&engine,&temp,&format!("paragraph-{tagged}.pdf"),&mut fixture(tagged,"20 200 250 140 re W* n",0,1.));
        let original=engine.source_bytes(&source.id).unwrap();let graph=Document::load_mem(&original).unwrap();
        let neighbor=engine.list_occurrence_text_runs(&source.id,0).unwrap().runs.remove(1);
        let mut current=source.clone();let mut operation_count=None;
        for cycle in 0..4 {
            let selection=engine.suggest_occurrence_styled_paragraph(&current.id,0,0,if cycle==0 {"Old content"}else{"Old content this"}).unwrap();
            let text=if cycle%2==0 {"Old content this\nSecond edited line"}else{"Old content this\nAnother edited line"};
            let request=paragraph_request(&selection,text);
            let edited=engine.preview_styled_paragraph_occurrence(&current.id,&[0,1],0,&request).unwrap();
            let saved=temp.path().join(format!("paragraph-{tagged}-{cycle}.pdf"));
            engine.export_pdf(ExportRequest{pages:plans(&edited.document),flatten:false},&saved).unwrap();
            let bytes=fs::read(&saved).unwrap();let pdf=Document::load_mem(&bytes).unwrap();assert_graph(&graph,&pdf);
            let ops=lopdf::content::Content::decode(&pdf.get_page_content(*pdf.get_pages().get(&1).unwrap())).unwrap().operations;
            if let Some(count)=operation_count {assert_eq!(ops.len(),count,"obsolete paragraph envelopes accumulated at cycle {cycle}");}
            operation_count=Some(ops.len());
            let reopened=engine.open_document(&saved).unwrap();
            let runs=engine.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;
            let actual=runs.iter().find(|r|r.text=="Neighbor").unwrap();assert_eq!(actual.bounds,neighbor.bounds);
            assert_eq!(engine.render_page(&source.id,1,800).unwrap(),engine.render_page(&reopened.id,1,800).unwrap());
            let restored=engine.suggest_occurrence_styled_paragraph(&reopened.id,0,runs[0].object_index,&runs[0].text).unwrap();
            assert_eq!(restored.text,text);assert_eq!(restored.bounds,request.bounds);
            engine.close_document(&edited.document.id).unwrap();if current.id!=source.id {engine.close_document(&current.id).unwrap();}current=reopened;
        }
        assert_eq!(engine.source_bytes(&source.id).unwrap(),original);
        engine.close_document(&current.id).unwrap();engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn clipped_uniform_paragraph_reedits_are_bounded_and_marked_append_is_refused() {
    use folio_engine::{AnnotationRect,ParagraphFormatting};
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let source=opened(&engine,&temp,"uniform.pdf",&mut fixture(false,"20 200 250 140 re W* n",0,1.));
    let area=AnnotationRect{x:30.,y:65.,width:220.,height:110.};
    let mut current=source.clone();let mut count=None;
    for cycle in 0..3 {
        let runs=engine.list_occurrence_text_runs(&current.id,0).unwrap().runs;
        let first=runs.iter().find(|r|r.text.starts_with("Old content")).unwrap();
        let selection=engine.suggest_occurrence_text_paragraph(&current.id,0,first.object_index,&first.text).unwrap();
        let text=if cycle%2==0 {"Old content this\nSecond line"}else{"Old content this\nAnother line"};
        let result=engine.preview_text_paragraph_occurrence(&current.id,&[0,1],0,&selection.object_indices,&selection.text,text,&area,&ParagraphFormatting::default()).unwrap();
        let bytes=engine.source_bytes(&result.document.id).unwrap();let pdf=Document::load_mem(&bytes).unwrap();
        let ops=lopdf::content::Content::decode(&pdf.get_page_content(*pdf.get_pages().get(&1).unwrap())).unwrap().operations;
        if let Some(count)=count {assert_eq!(ops.len(),count,"uniform clip wrappers accumulated");}count=Some(ops.len());
        let runs=engine.list_occurrence_text_runs(&result.document.id,0).unwrap().runs;
        let first=runs.iter().find(|r|r.text.starts_with("Old content")).unwrap();
        let saved=engine.suggest_occurrence_text_paragraph(&result.document.id,0,first.object_index,&first.text).unwrap();assert_eq!(saved.text,text);assert_eq!(saved.bounds,area);
        if current.id!=source.id {engine.close_document(&current.id).unwrap();}current=result.document;
    }
    engine.close_document(&current.id).unwrap();engine.close_document(&source.id).unwrap();
    let tagged=opened(&engine,&temp,"uniform-tagged.pdf",&mut fixture(true,"20 200 250 140 re W* n",0,1.));
    assert!(engine.inspect_occurrence_text_paragraph(&tagged.id,0,&[0]).unwrap_err().to_string().contains("styled"));
    engine.close_document(&tagged.id).unwrap();
}

#[test]
fn styled_context_hints_do_not_authorize_changed_clip_or_language_with_identical_pixels() {
    use lopdf::content::{Content,Operation};
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let source=opened(&engine,&temp,"hint-source.pdf",&mut fixture(true,"20 200 250 140 re W* n",0,1.));
    let selection=engine.inspect_occurrence_styled_paragraph(&source.id,0,&[0]).unwrap();
    let request=paragraph_request(&selection,"Old content this\nSecond line");
    let saved=engine.preview_styled_paragraph_occurrence(&source.id,&[0,1],0,&request).unwrap();
    let bytes=engine.source_bytes(&saved.document.id).unwrap();
    let runs=engine.list_occurrence_text_runs(&saved.document.id,0).unwrap().runs;
    let indices=runs.iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect::<Vec<_>>();
    for change in ["clip","language","interior"] {
        let mut pdf=Document::load_mem(&bytes).unwrap();let page=*pdf.get_pages().get(&1).unwrap();
        let mut content=Content::decode(&pdf.get_page_content(page)).unwrap();
        match change {
            "clip"=>for op in &mut content.operations {if op.operator=="re"&&op.operands==vec![20.into(),200.into(),250.into(),140.into()] {op.operands=vec![21.into(),200.into(),249.into(),140.into()];}},
            "language"=>{let op=content.operations.iter_mut().find(|op|op.operator=="BDC").unwrap();op.operands[1].as_dict_mut().unwrap().set("Lang",Object::string_literal("fr-FR"));},
            _=>{let position=content.operations.iter().position(|op|op.operator=="Tj").unwrap();content.operations.insert(position+3,Operation::new("n",vec![]));},
        }
        let stream=pdf.add_object(Stream::new(dictionary!{},content.encode().unwrap()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);
        let altered=opened(&engine,&temp,&format!("hint-{change}.pdf"),&mut pdf);
        assert_eq!(engine.render_page(&saved.document.id,0,800).unwrap(),engine.render_page(&altered.id,0,800).unwrap(),"{change} must be invisible");
        let selection=engine.inspect_occurrence_styled_paragraph(&altered.id,0,&indices).unwrap();
        assert_ne!(selection.bounds,request.bounds,"{change} restored stale paragraph context");
        engine.close_document(&altered.id).unwrap();
    }
    for id in [&source.id,&saved.document.id] {engine.close_document(id).unwrap();}
}

#[test]
fn styled_paragraph_preserves_clipped_highlights_and_refuses_cross_scope_or_outside_area() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let mut pdf=fixture(true,"20 200 250 140 re W* n",0,1.);let page=*pdf.get_pages().get(&1).unwrap();
    let original=String::from_utf8(pdf.get_page_content(page)).unwrap();
    // The highlight remains a source-owned graphic; the new envelope must own
    // both its replacement paint and the replacement text on every re-edit.
    let original=original.replacen("BT /F1 18 Tf","q 1 1 0 rg 38 296 98 22 re f Q BT /F1 18 Tf",1);
    let stream=pdf.add_object(Stream::new(dictionary!{},original.into_bytes()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);
    let source=opened(&engine,&temp,"highlight-context.pdf",&mut pdf);
    let runs=engine.list_occurrence_text_runs(&source.id,0).unwrap().runs;let index=runs[0].object_index;
    assert!(engine.inspect_occurrence_styled_paragraph(&source.id,0,&[index,runs[1].object_index]).is_err());
    let selection=engine.inspect_occurrence_styled_paragraph(&source.id,0,&[index]).unwrap();
    assert_eq!(selection.styles[0].background_color.as_deref(),Some("#FFFF00"));
    let mut request=paragraph_request(&selection,"Old content this\nSecond line");
    let good=request.bounds.clone();request.bounds.width=300.;
    assert!(engine.layout_occurrence_styled_paragraph(&source.id,0,&request).unwrap_err().to_string().contains("clipping"));request.bounds=good;
    let result=engine.preview_styled_paragraph_occurrence(&source.id,&[0,1],0,&request).unwrap();
    let bytes=engine.source_bytes(&result.document.id).unwrap();let pdf=Document::load_mem(&bytes).unwrap();
    let count=lopdf::content::Content::decode(&pdf.get_page_content(*pdf.get_pages().get(&1).unwrap())).unwrap().operations.len();
    let runs=engine.list_occurrence_text_runs(&result.document.id,0).unwrap().runs;
    let selected=runs.iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect::<Vec<_>>();
    let selection=engine.inspect_occurrence_styled_paragraph(&result.document.id,0,&selected).unwrap();
    assert_eq!(selection.styles[0].background_color.as_deref(),Some("#FFFF00"));
    let again=engine.preview_styled_paragraph_occurrence(&result.document.id,&[0,1],0,&paragraph_request(&selection,"Old content this\nAnother line")).unwrap();
    let pdf=Document::load_mem(&engine.source_bytes(&again.document.id).unwrap()).unwrap();
    assert_eq!(lopdf::content::Content::decode(&pdf.get_page_content(*pdf.get_pages().get(&1).unwrap())).unwrap().operations.len(),count);
    for id in [&source.id,&result.document.id,&again.document.id] {engine.close_document(id).unwrap();}
}

#[test]
fn clipped_styled_paragraph_keeps_separate_embedded_space_fragments() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let mut pdf=fixture(true,"20 200 250 140 re W* n",0,1.);
    let source=opened(&engine,&temp,"clipped-space.pdf",&mut pdf);
    let original=engine.source_bytes(&source.id).unwrap();
    let run=engine.list_occurrence_text_runs(&source.id,0).unwrap().runs.into_iter().find(|r|r.text=="Old content").unwrap();
    let selection=engine.inspect_occurrence_styled_paragraph(&source.id,0,&[run.object_index]).unwrap();
    let font=engine.fonts().register(fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
    let mut request=paragraph_request(&selection,"Old content this");
    request.runs=vec![
        folio_engine::StyledParagraphRun{text:"Old".into(),style_id:0},
        folio_engine::StyledParagraphRun{text:" ".into(),style_id:1},
        folio_engine::StyledParagraphRun{text:"content this".into(),style_id:0},
    ];
    request.style_changes=serde_json::from_value(serde_json::json!([
        {"styleId":1,"sourceStyleId":0,"fontId":font.id,"color":"#0033A6"}
    ])).unwrap();
    let changed=engine.preview_styled_paragraph_occurrence(&source.id,&[0,1],0,&request).unwrap();
    assert_eq!(engine.source_bytes(&source.id).unwrap(),original);
    assert_graph(&Document::load_mem(&original).unwrap(),&Document::load_mem(&engine.source_bytes(&changed.document.id).unwrap()).unwrap());
    assert_eq!(engine.render_page(&source.id,1,800).unwrap(),engine.render_page(&changed.document.id,1,800).unwrap());
    let saved=temp.path().join("clipped-space-saved.pdf");
    engine.export_pdf(ExportRequest{pages:plans(&changed.document),flatten:false},&saved).unwrap();
    engine.fonts().remove(&font.id).unwrap();
    let reopened=engine.open_document(&saved).unwrap();
    let runs=engine.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;
    let ids=runs.iter().filter(|r|r.text!="Neighbor").map(|r|r.object_index).collect::<Vec<_>>();
    let selection=engine.inspect_occurrence_styled_paragraph(&reopened.id,0,&ids).unwrap();
    assert_eq!(selection.text,"Old content this");
    assert!(selection.runs.iter().any(|r|r.text==" "));
    let mut replay=paragraph_request(&selection,&selection.text);replay.runs=selection.runs.clone();
    let again=engine.preview_styled_paragraph_occurrence(&reopened.id,&[0,1],0,&replay).unwrap();
    assert_eq!(engine.render_page(&changed.document.id,0,1200).unwrap(),engine.render_page(&again.document.id,0,1200).unwrap());
    assert_graph(&Document::load_mem(&original).unwrap(),&Document::load_mem(&engine.source_bytes(&again.document.id).unwrap()).unwrap());
    capture("clipped-space-source.pdf",&original);
    capture("clipped-space-saved.pdf",&fs::read(&saved).unwrap());
    capture("clipped-space-reedit.pdf",&engine.source_bytes(&again.document.id).unwrap());
    for id in [&source.id,&changed.document.id,&reopened.id,&again.document.id]{engine.close_document(id).unwrap();}
}

#[test]
#[ignore="requires explicit locally generated original Word corpus"]
fn original_word_styled_growth_save_reopen_reedit() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();let folder=PathBuf::from(std::env::var_os("FOLIO_WORD_CONTEXT_INPUTS").unwrap());
    for name in ["word-False","word-True"] {
        let path=folder.join(format!("{name}.pdf"));let original=fs::read(&path).unwrap();let source=engine.open_document(&path).unwrap();
        let mut current=source.clone();let mut count=None;
        for cycle in 0..3 {
            let runs=engine.list_occurrence_text_runs(&current.id,0).unwrap().runs;let first=&runs[0];
            let selected=engine.suggest_occurrence_styled_paragraph(&current.id,0,first.object_index,&first.text).unwrap();
            let text=if cycle%2==0 {"Simple sentence for editing this.\nSimple sentence for editing."}else{"Simple sentence for editing this this.\nSimple sentence for editing."};
            let mut request=paragraph_request(&selected,text);request.bounds=folio_engine::AnnotationRect{x:90.024,y:70.,width:270.,height:75.};
            let result=engine.preview_styled_paragraph_occurrence(&current.id,&[0,1],0,&request).unwrap();
            let saved=temp.path().join(format!("{name}-paragraph-{cycle}.pdf"));engine.export_pdf(ExportRequest{pages:plans(&result.document),flatten:false},&saved).unwrap();
            let bytes=fs::read(&saved).unwrap();capture(&format!("{name}-paragraph-{cycle}.pdf"),&bytes);
            let after=Document::load_mem(&bytes).unwrap();assert_graph(&Document::load_mem(&original).unwrap(),&after);
            let ops=lopdf::content::Content::decode(&after.get_page_content(*after.get_pages().get(&1).unwrap())).unwrap().operations.len();
            if let Some(count)=count {assert_eq!(ops,count);}count=Some(ops);
            let reopened=engine.open_document(saved).unwrap();let runs=engine.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;
            let restored=engine.suggest_occurrence_styled_paragraph(&reopened.id,0,runs[0].object_index,&runs[0].text).unwrap();assert_eq!(restored.text,text);assert_eq!(restored.bounds,request.bounds);
            assert_eq!(engine.render_page(&source.id,1,1000).unwrap(),engine.render_page(&reopened.id,1,1000).unwrap());
            engine.close_document(&result.document.id).unwrap();if current.id!=source.id {engine.close_document(&current.id).unwrap();}current=reopened;
        }
        assert_eq!(fs::read(path).unwrap(),original);engine.close_document(&current.id).unwrap();engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn clipped_single_run_insert_save_reopen_reedit_preserves_graph_and_neighbors() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    for tagged in [false,true] {for (rotation,unit) in [(0,1.),(90,1.7),(270,2.)] {
        let name=format!("t{tagged}-r{rotation}-u{unit}");
        let source=opened(&engine,&temp,&format!("{name}.pdf"),&mut fixture(tagged,"0 0 600 400 re W* n",rotation,unit));
        let original=engine.source_bytes(&source.id).unwrap();let before=Document::load_mem(&original).unwrap();
        let listed=engine.list_occurrence_text_runs(&source.id,0).unwrap();let selected=&listed.runs[0];
        assert!(selected.supported,"{name}: {:?}",selected.reason);
        let result=engine.replace_text_occurrence(&source.id,&[0,1],0,selected.object_index,&selected.text,"Old this content",None).unwrap();
        assert_eq!(engine.render_page(&source.id,1,800).unwrap(),engine.render_page(&result.document.id,1,800).unwrap());
        let runs=engine.list_occurrence_text_runs(&result.document.id,0).unwrap().runs;
        assert_eq!(runs[0].text,"Old this content");assert_eq!(runs[1].text,"Neighbor");assert_eq!(runs[1].bounds,listed.runs[1].bounds);
        let path=temp.path().join(format!("{name}-saved.pdf"));engine.export_pdf(ExportRequest {pages:plans(&result.document),flatten:false},&path).unwrap();
        let bytes=fs::read(&path).unwrap();let after=Document::load_mem(&bytes).unwrap();assert_graph(&before,&after);
        let reopened=engine.open_document(&path).unwrap();
        let run=engine.list_occurrence_text_runs(&reopened.id,0).unwrap().runs.remove(0);
        let again=engine.replace_text_occurrence(&reopened.id,&[0,1],0,run.object_index,&run.text,"Old this this content",None).unwrap();
        assert_eq!(engine.list_occurrence_text_runs(&again.document.id,0).unwrap().runs[0].text,"Old this this content");
        assert_graph(&before,&Document::load_mem(&engine.source_bytes(&again.document.id).unwrap()).unwrap());
        assert_eq!(engine.source_bytes(&source.id).unwrap(),original);
        capture(&format!("{name}-source.pdf"),&original);capture(&format!("{name}-saved.pdf"),&bytes);capture(&format!("{name}-reedit.pdf"),&engine.source_bytes(&again.document.id).unwrap());
        for id in [&source.id,&result.document.id,&reopened.id,&again.document.id] {engine.close_document(id).unwrap();}
    }}
}

#[test]
fn clipped_source_or_replacement_and_unsupported_paths_refuse() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    for (index,clip,replacement) in [
        (0,"40 290 20 40 re W n","New"),
        (1,"35 290 100 40 re W n","Old content this this"),
        (2,"0 0 m 600 0 l 600 400 l h W n","New"),
    ] {
        let source=opened(&engine,&temp,&format!("bad{index}.pdf"),&mut fixture(true,clip,0,1.));
        let bytes=engine.source_bytes(&source.id).unwrap();
        let result=engine.replace_text_occurrence(&source.id,&[0,1],0,0,"Old content",replacement,None);
        assert!(result.is_err(),"{clip}");assert_eq!(engine.source_bytes(&source.id).unwrap(),bytes);
        engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn tagged_duplication_and_stale_requests_refuse_but_plain_clipped_copy_can_edit() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    for tagged in [true,false] {
        let source=opened(&engine,&temp,&format!("duplicate{tagged}.pdf"),&mut fixture(tagged,"0 0 600 400 re W n",0,1.));
        let error=engine.replace_text_occurrence(&source.id,&[0,1],0,0,"stale","New",None).unwrap_err();assert!(error.to_string().contains("changed since"),"{error}");
        let result=engine.replace_text_occurrence(&source.id,&[0,1,0],2,0,"Old content","Old this content",None);
        if tagged {assert!(result.unwrap_err().to_string().contains("tagged"));}
        else {let result=result.unwrap();assert_eq!(result.page_indices,vec![0,1,2]);assert_eq!(engine.render_page(&source.id,0,800).unwrap(),engine.render_page(&result.document.id,0,800).unwrap());engine.close_document(&result.document.id).unwrap();}
        engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn unsupported_selected_semantics_remain_ineligible_and_generic_replacement_stays_guarded() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let mut pdf=fixture(true,"0 0 600 400 re W n",0,1.);
    let selected_owner=*pdf.objects.iter().find(|(_,o)|o.as_dict().is_ok_and(|d|d.get(b"S").and_then(Object::as_name).ok()==Some(b"P"))).unwrap().0;
    pdf.get_dictionary_mut(selected_owner).unwrap().set("ActualText",Object::string_literal("Original meaning"));
    let source=opened(&engine,&temp,"alternate.pdf",&mut pdf);
    assert!(!engine.list_occurrence_text_runs(&source.id,0).unwrap().runs[0].supported);
    assert!(engine.replace_text_occurrence(&source.id,&[0,1],0,0,"Old content","New",None).is_err());
    assert!(engine.replace_page_content(&source.id,0,&source.id,1).is_err());
    engine.close_document(&source.id).unwrap();
}

#[path="support/protected_fixture.rs"]
mod protected_fixture;

#[test]
fn clipped_tagged_edit_preserves_password_permissions_and_original_bytes() {
    use lopdf::encryption::Permissions;
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    for (name,rights,allowed) in [("edit",Permissions::MODIFIABLE,true),("assemble",Permissions::ASSEMBLABLE,false)] {
        let mut plain=vec![];fixture(true,"0 0 600 400 re W n",0,1.).save_to(&mut plain).unwrap();
        let bytes=protected_fixture::encrypt(&plain,rights);let path=temp.path().join(format!("{name}.pdf"));fs::write(&path,&bytes).unwrap();
        let source=engine.open_document_with_password(&path,"reader").unwrap();
        let result=engine.replace_text_occurrence(&source.id,&[0,1],0,0,"Old content","Old this content",None);
        if allowed {
            let result=result.unwrap();assert_eq!(result.document.protection,source.protection);
            assert!(matches!(engine.extract_text(&result.document.id,0),Err(folio_engine::EngineError::PermissionDenied(_))));
            let saved=temp.path().join("saved-protected.pdf");engine.export_pdf(ExportRequest{pages:plans(&result.document),flatten:false},&saved).unwrap();
            let reader=engine.open_document_with_password(&saved,"reader").unwrap();
            let owner=engine.open_document_with_password(&saved,"owner").unwrap();
            assert_eq!(reader.protection.as_ref().unwrap().permissions,source.protection.as_ref().unwrap().permissions);
            assert!(engine.extract_text(&owner.id,0).unwrap().contains("Old this content"));
            for id in [&result.document.id,&reader.id,&owner.id] {engine.close_document(id).unwrap();}
        } else {assert!(matches!(result,Err(folio_engine::EngineError::PermissionDenied(_))));}
        assert_eq!(fs::read(path).unwrap(),bytes);engine.close_document(&source.id).unwrap();
    }
}

#[test]
#[ignore="requires explicit locally generated original Word corpus"]
fn original_word_single_insert_save_reopen_reedit() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();let folder=PathBuf::from(std::env::var_os("FOLIO_WORD_CONTEXT_INPUTS").unwrap());
    for name in ["word-False","word-True"] {
        let path=folder.join(format!("{name}.pdf"));let original=fs::read(&path).unwrap();let source=engine.open_document(&path).unwrap();
        let run=engine.list_occurrence_text_runs(&source.id,0).unwrap().runs.remove(0);assert!(run.supported,"{name}: {:?}",run.reason);
        let result=engine.replace_text_occurrence(&source.id,&[0,1],0,run.object_index,&run.text,"Simple sentence for editing this.",None).unwrap();
        let saved=temp.path().join(format!("{name}-saved.pdf"));engine.export_pdf(ExportRequest {pages:plans(&result.document),flatten:false},&saved).unwrap();
        let reopened=engine.open_document(&saved).unwrap();let run=engine.list_occurrence_text_runs(&reopened.id,0).unwrap().runs.remove(0);
        let again=engine.replace_text_occurrence(&reopened.id,&[0,1],0,run.object_index,&run.text,"Simple sentence for editing this this.",None).unwrap();
        assert_eq!(engine.render_page(&source.id,1,1000).unwrap(),engine.render_page(&again.document.id,1,1000).unwrap());
        assert_graph(&Document::load_mem(&original).unwrap(),&Document::load_mem(&engine.source_bytes(&again.document.id).unwrap()).unwrap());
        assert_eq!(fs::read(path).unwrap(),original);
        capture(&format!("{name}-saved.pdf"),&fs::read(saved).unwrap());capture(&format!("{name}-reedit.pdf"),&engine.source_bytes(&again.document.id).unwrap());
        for id in [&source.id,&result.document.id,&reopened.id,&again.document.id] {engine.close_document(id).unwrap();}
    }
}

#[test]
fn font_context_alternating_fonts_preserve_saved_state_without_growth() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let fonts=["corpus/fonts/DejaVuSerif.ttf","shaped-text/DejaVuSans.ttf"].map(|name|engine.fonts().register(fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures").join(name)).unwrap()).unwrap());
    let source=opened(&engine,&temp,"alternate.pdf",&mut fixture(true,"20 200 500 150 re W n",0,1.));
    let original=engine.source_bytes(&source.id).unwrap();let graph=Document::load_mem(&original).unwrap();
    let before=engine.list_occurrence_text_runs(&source.id,0).unwrap().runs;
    let mut current=source.clone();let mut counts=None;
    for cycle in 0..6 {
        let run=engine.list_occurrence_text_runs(&current.id,0).unwrap().runs.remove(0);
        let changed=engine.replace_text_occurrence(&current.id,&[0,1],0,run.object_index,&run.text,"New content",Some(&fonts[cycle%2].id)).unwrap();
        let saved=temp.path().join(format!("saved-{cycle}.pdf"));engine.export_pdf(ExportRequest{pages:plans(&changed.document),flatten:false},&saved).unwrap();
        let bytes=fs::read(&saved).unwrap();let pdf=Document::load_mem(&bytes).unwrap();assert_graph(&graph,&pdf);
        let page=*pdf.get_pages().get(&1).unwrap();
        let ops=lopdf::content::Content::decode(&pdf.get_page_content(page)).unwrap().operations;
        let resources=pdf.dereference(pdf.get_dictionary(page).unwrap().get(b"Resources").unwrap()).unwrap().1.as_dict().unwrap();
        let resources=pdf.dereference(resources.get(b"Font").unwrap()).unwrap().1.as_dict().unwrap();
        let now=(ops.len(),resources.len());
        if cycle==1{counts=Some(now);}else if cycle>1{assert_eq!(Some(now),counts,"font/operator growth at cycle {cycle}");}
        let reopened=engine.open_document(saved).unwrap();let runs=engine.list_occurrence_text_runs(&reopened.id,0).unwrap().runs;
        assert_eq!(runs[0].text,"New content");assert_eq!(runs[0].font_name,format!("Folio{}Paragraph",fonts[cycle%2].id));
        assert_eq!(runs[1].bounds,before[1].bounds);assert_eq!(runs[1].font_name,before[1].font_name);assert_eq!(runs[1].text,before[1].text);
        assert_eq!(engine.render_page(&source.id,1,800).unwrap(),engine.render_page(&reopened.id,1,800).unwrap());
        engine.close_document(&changed.document.id).unwrap();if current.id!=source.id{engine.close_document(&current.id).unwrap();}current=reopened;
    }
    for font in &fonts{engine.fonts().remove(&font.id).unwrap();}
    let run=engine.list_occurrence_text_runs(&current.id,0).unwrap().runs.remove(0);
    let final_edit=engine.replace_text_occurrence(&current.id,&[0,1],0,run.object_index,&run.text,"New contents",None).unwrap();
    assert_eq!(engine.list_occurrence_text_runs(&final_edit.document.id,0).unwrap().runs[0].text,"New contents");
    assert_eq!(engine.source_bytes(&source.id).unwrap(),original);
}

#[test]
fn font_context_refuses_stale_clipped_overlapping_and_missing_glyph_edits() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let font=engine.fonts().register(fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
    for case in ["stale","clip","overlap","glyph","unknown-font"] {
        let mut pdf=fixture(true,if case=="clip"{"35 290 100 40 re W n"}else{"0 0 600 400 re W n"},0,1.);
        if case=="overlap" {
            let page=*pdf.get_pages().get(&1).unwrap();
            let content=String::from_utf8(pdf.get_page_content(page)).unwrap().replace("40 150 Tm (Neighbor)","170 300 Tm (Neighbor)");
            let stream=pdf.add_object(Stream::new(dictionary!{},content.into_bytes()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);
        }
        let source=opened(&engine,&temp,&format!("{case}.pdf"),&mut pdf);let original=engine.source_bytes(&source.id).unwrap();
        let run=engine.list_occurrence_text_runs(&source.id,0).unwrap().runs.remove(0);
        let expected=if case=="stale"{"stale text"}else{&run.text};
        let replacement=match case{"clip"=>"New content this this","overlap"=>"MMMMMMMMMMMMMMMM","glyph"=>"New \u{1f984}",_=>"New content"};
        let id=if case=="unknown-font"{"unknown-font"}else{&font.id};
        let error=engine.replace_text_occurrence(&source.id,&[0,1],0,run.object_index,expected,replacement,Some(id)).unwrap_err();
        if case=="stale"{assert!(error.to_string().contains("changed since"),"{error}");}
        assert_eq!(engine.source_bytes(&source.id).unwrap(),original,"{case}");
        engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn font_context_substitution_preserves_passwords_and_edit_permissions() {
    use lopdf::encryption::Permissions;
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let font=engine.fonts().register(fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
    for (name,rights,allowed) in [("allowed",Permissions::MODIFIABLE,true),("denied",Permissions::ASSEMBLABLE,false)] {
        let mut plain=vec![];fixture(true,"0 0 600 400 re W n",0,1.).save_to(&mut plain).unwrap();
        let bytes=protected_fixture::encrypt(&plain,rights);let path=temp.path().join(format!("{name}.pdf"));fs::write(&path,&bytes).unwrap();
        let source=engine.open_document_with_password(&path,"reader").unwrap();
        let result=engine.replace_text_occurrence(&source.id,&[0,1],0,0,"Old content","New content",Some(&font.id));
        if allowed {
            let changed=result.unwrap();assert_eq!(changed.document.protection,source.protection);
            assert!(matches!(engine.extract_text(&changed.document.id,0),Err(folio_engine::EngineError::PermissionDenied(_))));
            let saved=temp.path().join("saved-font.pdf");engine.export_pdf(ExportRequest{pages:plans(&changed.document),flatten:false},&saved).unwrap();
            assert!(engine.open_document(&saved).is_err());
            let reader=engine.open_document_with_password(&saved,"reader").unwrap();let owner=engine.open_document_with_password(&saved,"owner").unwrap();
            assert_eq!(reader.protection.as_ref().unwrap().permissions,source.protection.as_ref().unwrap().permissions);
            assert!(engine.extract_text(&owner.id,0).unwrap().contains("New content"));
            assert_eq!(engine.list_occurrence_text_runs(&owner.id,0).unwrap().runs[0].font_name,format!("Folio{}Paragraph",font.id));
            for id in [&changed.document.id,&reader.id,&owner.id]{engine.close_document(id).unwrap();}
        }else{assert!(matches!(result,Err(folio_engine::EngineError::PermissionDenied(_))));}
        assert_eq!(fs::read(path).unwrap(),bytes);engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn font_context_rotated_physical_pages_preserve_saved_glyphs_and_pixels() {
    for tagged in [false,true] {for (rotation,unit) in [(90,1.7),(270,2.)] {
        font_context_case_at(tagged,"0.25 Tc","[(Old) -20 ( content)] TJ",false,rotation,unit);
    }}
}


#[test]
fn source_font_edits_preserve_unused_large_spacing_assignments() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let font=engine.fonts().register(fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap()).unwrap();
    for (case,unused) in [
        ("reset-character","1000001 Tc 0 Tc"),
        ("reset-word","1000001 Tw 0 Tw"),
        ("restore-character","q 1000001 Tc Q"),
        ("restore-word","q 1000001 Tw Q"),
    ] {
        let mut base=fixture(true,"20 200 500 150 re W n",0,1.);
        let page=*base.get_pages().get(&1).unwrap();
        let mut pdf=base.clone();
        let data=String::from_utf8(pdf.get_page_content(page)).unwrap().replacen("BT",&format!("BT {unused}"),1);
        let content=pdf.add_object(Stream::new(dictionary!{},data.into_bytes()));pdf.get_dictionary_mut(page).unwrap().set("Contents",content);
        let control=opened(&engine,&temp,&format!("{case}-control.pdf"),&mut base);
        let source=opened(&engine,&temp,&format!("{case}.pdf"),&mut pdf);
        assert_eq!(engine.render_page(&source.id,0,1200).unwrap(),engine.render_page(&control.id,0,1200).unwrap());
        let original=engine.source_bytes(&source.id).unwrap();let graph=Document::load_mem(&original).unwrap();
        let run=engine.list_occurrence_text_runs(&source.id,0).unwrap().runs.remove(0);assert!(run.supported);
        let edited=engine.replace_text_occurrence(&source.id,&[0,1],0,run.object_index,&run.text,"New content",None)
            .unwrap_or_else(|e|panic!("unused {case} must not block source-font editing: {e}"));
        assert_eq!(engine.list_occurrence_text_runs(&edited.document.id,0).unwrap().runs[0].text,"New content");
        let after=Document::load_mem(&engine.source_bytes(&edited.document.id).unwrap()).unwrap();assert_graph(&graph,&after);
        let old_ops=lopdf::content::Content::decode(&graph.get_page_content(page)).unwrap().operations;
        let new_ops=lopdf::content::Content::decode(&after.get_page_content(page)).unwrap().operations;
        assert_eq!(old_ops.len(),new_ops.len());
        let selected=old_ops.iter().position(|op|op.operator=="Tj").unwrap();
        for (i,(a,b)) in old_ops.iter().zip(&new_ops).enumerate(){
            assert_eq!(a.operator,b.operator);if i!=selected{assert_eq!(a.operands,b.operands,"retained source operation {i}");}
        }
        assert_eq!(engine.render_page(&source.id,1,800).unwrap(),engine.render_page(&edited.document.id,1,800).unwrap());
        // Explicit substitution keeps its existing bounded-state admission.
        assert!(engine.replace_text_occurrence(&source.id,&[0,1],0,run.object_index,&run.text,"New content",Some(&font.id)).is_err());
        assert_eq!(engine.source_bytes(&source.id).unwrap(),original);
        for id in [source.id,control.id,edited.document.id]{engine.close_document(&id).unwrap();}
    }
}

#[test]
fn source_font_edits_keep_dormant_word_spacing_and_refuse_exposing_it() {
    dormant_word_spacing_case("1000001 Tw (Oldcontent) Tj",false);
}

#[test]
fn source_font_edits_retain_existing_double_quote_context_limit() {
    // Both the previous accepted CLI and the corrected CLI reject this source
    // during context inspection. This milestone does not widen that policy.
    dormant_word_spacing_case("1000001 0 (Oldcontent) \"",true);
}

fn dormant_word_spacing_case(show:&str,context_refused:bool) {
    let engine=engine();let temp=tempfile::tempdir().unwrap();
    let mut pdf=fixture(true,"20 200 500 150 re W n",0,1.);let page=*pdf.get_pages().get(&1).unwrap();
    let data=String::from_utf8(pdf.get_page_content(page)).unwrap().replacen("(Old content) Tj",show,1);
    let stream=pdf.add_object(Stream::new(dictionary!{},data.into_bytes()));pdf.get_dictionary_mut(page).unwrap().set("Contents",stream);
    let source=opened(&engine,&temp,"dormant-large-word-space.pdf",&mut pdf);let original=engine.source_bytes(&source.id).unwrap();
    let run=engine.list_occurrence_text_runs(&source.id,0).unwrap().runs.remove(0);
    if context_refused {
        assert!(!run.supported);
        assert!(run.reason.as_deref().unwrap().contains("clipping or marked-content context"));
        let error=engine.replace_text_occurrence(&source.id,&[0,1],0,run.object_index,&run.text,"Newcontent",None).unwrap_err();
        assert!(error.to_string().contains("clipping or marked-content context"));
        assert_eq!(engine.source_bytes(&source.id).unwrap(),original);
        return;
    }
    let edited=engine.replace_text_occurrence(&source.id,&[0,1],0,run.object_index,&run.text,"Newcontent",None)
        .expect("dormant finite word spacing must remain editable without spaces");
    let path=temp.path().join("dormant-saved.pdf");engine.export_pdf(ExportRequest{pages:plans(&edited.document),flatten:false},&path).unwrap();
    let reopened=engine.open_document(&path).unwrap();let run=engine.list_occurrence_text_runs(&reopened.id,0).unwrap().runs.remove(0);
    let again=engine.replace_text_occurrence(&reopened.id,&[0,1],0,run.object_index,&run.text,"Newcontents",None).unwrap();
    assert_eq!(engine.list_occurrence_text_runs(&again.document.id,0).unwrap().runs[0].text,"Newcontents");
    let error=engine.replace_text_occurrence(&reopened.id,&[0,1],0,run.object_index,&run.text,"New content",None).unwrap_err();
    assert!(error.to_string().contains("clipping")||error.to_string().contains("visible page"),"must retain and enforce the dormant spacing: {error}");
    assert_eq!(engine.source_bytes(&source.id).unwrap(),original);
}
