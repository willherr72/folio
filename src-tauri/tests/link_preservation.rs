use folio_engine::{DocumentInfo, ExportRequest, PagePlan, PdfEngine};
use lopdf::{dictionary, Dictionary, Document, Object, Stream};
use std::{fs, path::PathBuf};

fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}
fn fixture(label: &str) -> Document {
    let mut pdf = Document::with_version("1.7");
    let tree = pdf.new_object_id();
    let font =
        pdf.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let mut pages = Vec::new();
    for index in 0..2 {
        let content = pdf.add_object(Stream::new(
            dictionary! {},
            format!("BT /F1 18 Tf 30 260 Td ({label} page {index}) Tj ET").into_bytes(),
        ));
        pages.push(pdf.add_object(dictionary! {"Type"=>"Page","Parent"=>tree,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()],"Resources"=>dictionary! {"Font"=>dictionary! {"F1"=>font}},"Contents"=>content}));
    }
    for (index, page) in pages.iter().enumerate() {
        let other = pages[1 - index];
        let mut annotations = Vec::new();
        for (slot, kind) in ["direct", "named", "uri", "self", "chain"]
            .iter()
            .enumerate()
        {
            let mut link = dictionary! {"Type"=>"Annot","Subtype"=>"Link","P"=>*page,"NM"=>Object::string_literal(*kind),"Rect"=>vec![20.into(),(40+slot as i64*30).into(),180.into(),(60+slot as i64*30).into()],"Border"=>vec![0.into(),0.into(),1.into()],"C"=>vec![1.into(),0.into(),0.into()]};
            match *kind {
                "direct"=>link.set("Dest",vec![Object::Reference(other),Object::Name(b"Fit".to_vec())]),
                "named"=>link.set("A",dictionary! {"S"=>"GoTo","D"=>Object::string_literal(format!("p{}",1-index))}),
                "uri"=>link.set("A",dictionary! {"S"=>"URI","URI"=>Object::string_literal(format!("https://example.invalid/{label}")),"IsMap"=>false}),
                "self"=>link.set("Dest",vec![Object::Reference(*page),Object::Name(b"XYZ".to_vec()),20.into(),Object::Null,Object::Real(1.25)]),
                "chain"=>{link.set("A",dictionary! {"S"=>"URI","URI"=>Object::string_literal("https://example.invalid/chain"),"Next"=>vec![Object::Dictionary(dictionary! {"S"=>"GoTo","D"=>Object::string_literal(format!("p{}",1-index))}),Object::Dictionary(dictionary! {"S"=>"GoToR","F"=>Object::string_literal("external.pdf"),"D"=>Object::string_literal("remote-target"),"NewWindow"=>true})]});link.set("AA",dictionary! {"E"=>dictionary! {"S"=>"GoTo","D"=>vec![Object::Reference(*page),Object::Name(b"FitH".to_vec()),Object::Null]}});},_=>unreachable!(),
            }
            annotations.push(Object::Reference(pdf.add_object(link)));
        }
        pdf.get_dictionary_mut(*page)
            .unwrap()
            .set("Annots", annotations);
    }
    pdf.objects.insert(tree,dictionary! {"Type"=>"Pages","Kids"=>pages.iter().map(|p|Object::Reference(*p)).collect::<Vec<_>>(),"Count"=>2}.into());
    let root=pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>tree,"Names"=>dictionary! {"Dests"=>dictionary! {"Names"=>vec![Object::string_literal("p0"),Object::Array(vec![Object::Reference(pages[0]),Object::Name(b"Fit".to_vec())]),Object::string_literal("p1"),Object::Array(vec![Object::Reference(pages[1]),Object::Name(b"Fit".to_vec())])]}}});
    pdf.trailer.set("Root", root);
    pdf
}
fn open(
    engine: &PdfEngine,
    temp: &tempfile::TempDir,
    name: &str,
    pdf: &mut Document,
) -> DocumentInfo {
    let path = temp.path().join(name);
    pdf.save(&path).unwrap();
    engine.open_document(path).unwrap()
}
fn plans(selection: &[(&DocumentInfo, usize)]) -> Vec<PagePlan> {
    selection.iter().enumerate().map(|(i,(s,p))|serde_json::from_value(serde_json::json!({"id":format!("p{i}"),"sourceId":s.id,"pageIndex":p,"width":s.pages[*p].width,"height":s.pages[*p].height,"rotation":0,"overlays":[]})).unwrap()).collect()
}
fn resolve<'a>(pdf: &'a Document, value: &'a Object) -> &'a Object {
    pdf.dereference(value).unwrap().1
}
fn links(pdf: &Document, page: usize) -> Vec<Dictionary> {
    let page = *pdf.get_pages().values().nth(page).unwrap();
    resolve(
        pdf,
        pdf.get_dictionary(page).unwrap().get(b"Annots").unwrap(),
    )
    .as_array()
    .unwrap()
    .iter()
    .filter_map(|v| {
        let d = resolve(pdf, v).as_dict().unwrap();
        d.get(b"Subtype")
            .and_then(|value| resolve(pdf, value).as_name())
            .is_ok_and(|n| n == b"Link")
            .then(|| d.clone())
    })
    .collect()
}
fn destination(pdf: &Document, value: &Object) -> usize {
    // Unchanged complete-source Save preserves named destinations. Resolve the
    // fixture's flat destination tree independently instead of requiring the
    // composer's optional array canonicalization on every save route.
    let value = resolve(pdf, value);
    let named = match value {
        Object::Name(name) => Some((true, name.as_slice())),
        Object::String(name, _) => Some((false, name.as_slice())),
        _ => None,
    };
    let value = if let Some((legacy, name)) = named {
        let catalog = pdf.catalog().unwrap();
        if legacy {
            resolve(pdf, catalog.get(b"Dests").unwrap()).as_dict().unwrap().get(name).unwrap()
        } else {
            let names = resolve(pdf, catalog.get(b"Names").unwrap()).as_dict().unwrap();
            let tree = resolve(pdf, names.get(b"Dests").unwrap()).as_dict().unwrap();
            let entries = resolve(pdf, tree.get(b"Names").unwrap()).as_array().unwrap();
            assert_eq!(entries.len() % 2, 0, "malformed fixture name tree");
            let mut matches = entries.chunks_exact(2).filter(|pair| {
                resolve(pdf, &pair[0]).as_str().unwrap() == name
            });
            let target = &matches.next().expect("named destination is unresolved")[1];
            assert!(matches.next().is_none(), "ambiguous fixture destination name");
            target
        }
    } else { value };
    let value = resolve(pdf, value);
    let value = if let Object::Dictionary(dict) = value {
        resolve(pdf, dict.get(b"D").unwrap())
    } else { value };
    let a = value.as_array().expect("local destination must resolve to an array");
    let id = a[0]
        .as_reference()
        .expect("local target must reference an output page");
    pdf.get_pages()
        .values()
        .position(|p| *p == id)
        .expect("link must target a real output page")
}
fn capture(name: &str, inputs: &[(&str, &[u8])], output: &std::path::Path) {
    if let Some(root) = std::env::var_os("FOLIO_LINK_OUTPUT") {
        let root = PathBuf::from(root);
        fs::create_dir_all(&root).unwrap();
        use std::io::Write;
        for (suffix, bytes) in inputs {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(root.join(format!("{name}-{suffix}.pdf")))
                .unwrap()
                .write_all(bytes)
                .unwrap();
        }
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join(format!("{name}-output.pdf")))
            .unwrap()
            .write_all(&fs::read(output).unwrap())
            .unwrap();
    }
}
fn assert_links(pdf: &Document, page: usize, other: Option<usize>, label: &str) {
    let links = links(pdf, page);
    assert_eq!(links.len(), 5);
    if let Some(other) = other {
        assert_eq!(destination(pdf, links[0].get(b"Dest").unwrap()), other);
        let action = resolve(pdf, links[1].get(b"A").unwrap()).as_dict().unwrap();
        assert_eq!(destination(pdf, action.get(b"D").unwrap()), other);
    } else {
        assert!(!links[0].has(b"Dest"));
        assert!(!links[1].has(b"A"));
    }
    let uri = resolve(pdf, links[2].get(b"A").unwrap()).as_dict().unwrap();
    assert_eq!(
        uri.get(b"URI").unwrap().as_str().unwrap(),
        format!("https://example.invalid/{label}").as_bytes()
    );
    assert!(!uri.get(b"IsMap").unwrap().as_bool().unwrap());
    let self_dest = links[3].get(b"Dest").unwrap();
    assert_eq!(destination(pdf, self_dest), page);
    let self_dest = resolve(pdf, self_dest).as_array().unwrap();
    assert_eq!(self_dest[1].as_name().unwrap(), b"XYZ");
    assert!(matches!(self_dest[3], Object::Null));
    assert_eq!(self_dest[4].as_float().unwrap(), 1.25);
    let chain = resolve(pdf, links[4].get(b"A").unwrap()).as_dict().unwrap();
    let next = resolve(pdf, chain.get(b"Next").unwrap())
        .as_array()
        .unwrap();
    assert_eq!(next.len(), if other.is_some() { 2 } else { 1 });
    if let Some(other) = other {
        assert_eq!(
            destination(
                pdf,
                resolve(pdf, &next[0]).as_dict().unwrap().get(b"D").unwrap()
            ),
            other
        );
    }
    let remote = resolve(pdf, next.last().unwrap()).as_dict().unwrap();
    assert_eq!(remote.get(b"S").unwrap().as_name().unwrap(), b"GoToR");
    assert_eq!(
        remote.get(b"D").unwrap().as_str().unwrap(),
        b"remote-target"
    );
    assert!(remote.get(b"NewWindow").unwrap().as_bool().unwrap());
    let aa = resolve(pdf, links[4].get(b"AA").unwrap())
        .as_dict()
        .unwrap();
    let entry = resolve(pdf, aa.get(b"E").unwrap()).as_dict().unwrap();
    assert_eq!(destination(pdf, entry.get(b"D").unwrap()), page);
}

#[test]
fn save_retains_direct_and_named_internal_links() {
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    let source = open(&engine, &temp, "source.pdf", &mut fixture("Original"));
    let original = fs::read(temp.path().join("source.pdf")).unwrap();
    let path = temp.path().join("saved.pdf");
    engine
        .export_pdf(
            ExportRequest {
                pages: plans(&[(&source, 0), (&source, 1)]),
                flatten: false,
            },
            &path,
        )
        .unwrap();
    assert_eq!(fs::read(&path).unwrap(), original,
        "unchanged complete-source Save must retain original bytes");
    let output = Document::load(&path).unwrap();
    for page in 0..2 {
        let links = links(&output, page);
        assert_eq!(links.len(), 5);
        assert_eq!(
            destination(
                &output,
                links[0]
                    .get(b"Dest")
                    .expect("direct internal link was lost")
            ),
            1 - page
        );
        let action = resolve(&output, links[1].get(b"A").unwrap())
            .as_dict()
            .unwrap();
        assert_eq!(destination(&output, action.get(b"D").unwrap()), 1 - page);
        assert_eq!(destination(&output, links[3].get(b"Dest").unwrap()), page);
    }
    let reopened = engine.open_document(&path).unwrap();
    for page in 0..2 {
        assert_eq!(
            engine.render_page(&source.id, page, 600).unwrap(),
            engine.render_page(&reopened.id, page, 600).unwrap()
        );
    }
    assert_eq!(fs::read(temp.path().join("source.pdf")).unwrap(), original);
    capture("save", &[("source", &original)], &path);
    engine.close_document(&reopened.id).unwrap();
    engine.close_document(&source.id).unwrap();
}

#[test]
fn reorder_duplicate_extract_and_preview_keep_correct_targets() {
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    let source = open(&engine, &temp, "source.pdf", &mut fixture("Original"));
    let original = fs::read(temp.path().join("source.pdf")).unwrap();
    for (name, order) in [("reorder", vec![1, 0, 1]), ("extract", vec![0])] {
        let selection: Vec<_> = order.iter().map(|p| (&source, *p)).collect();
        let path = temp.path().join(format!("{name}.pdf"));
        engine
            .export_pdf(
                ExportRequest {
                    pages: plans(&selection),
                    flatten: false,
                },
                &path,
            )
            .unwrap();
        let pdf = Document::load(&path).unwrap();
        for (page, original) in order.iter().enumerate() {
            assert_links(
                &pdf,
                page,
                order.iter().position(|p| *p == 1 - original),
                "Original",
            );
        }
        let reopened = engine.open_document(&path).unwrap();
        for (page, old) in order.iter().enumerate() {
            assert_eq!(
                engine.render_page(&source.id, *old, 600).unwrap(),
                engine.render_page(&reopened.id, page, 600).unwrap()
            );
        }
        engine.close_document(&reopened.id).unwrap();
        capture(name, &[("source", &original)], &path);
    }
    let plan = plans(&[(&source, 1)]).remove(0);
    assert_eq!(
        engine.render_page_plan(plan, 600).unwrap(),
        engine.render_page(&source.id, 1, 600).unwrap()
    );
    assert_eq!(fs::read(temp.path().join("source.pdf")).unwrap(), original);
    engine.close_document(&source.id).unwrap();
}

#[test]
fn same_named_destinations_from_two_sources_never_cross_bind() {
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    let a = open(&engine, &temp, "a.pdf", &mut fixture("A"));
    let b = open(&engine, &temp, "b.pdf", &mut fixture("B"));
    let path = temp.path().join("combined.pdf");
    engine
        .export_pdf(
            ExportRequest {
                pages: plans(&[(&a, 1), (&b, 1), (&a, 0), (&b, 0)]),
                flatten: false,
            },
            &path,
        )
        .unwrap();
    let pdf = Document::load(&path).unwrap();
    for (page, other, label) in [(0, 2, "A"), (1, 3, "B"), (2, 0, "A"), (3, 1, "B")] {
        assert_links(&pdf, page, Some(other), label);
    }
    capture(
        "combined",
        &[
            ("a", &fs::read(temp.path().join("a.pdf")).unwrap()),
            ("b", &fs::read(temp.path().join("b.pdf")).unwrap()),
        ],
        &path,
    );
    engine.close_document(&a.id).unwrap();
    engine.close_document(&b.id).unwrap();
}

#[test]
fn legacy_indirect_named_and_numeric_destinations_preserve_view_parameters() {
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    let mut doc = fixture("Legacy");
    let pages: Vec<_> = doc.get_pages().values().copied().collect();
    let destination_id = doc.add_object(Object::Array(vec![
        1.into(),
        Object::Name(b"XYZ".to_vec()),
        Object::Null,
        73.into(),
        Object::Null,
    ]));
    let named = doc.add_object(dictionary! {"D"=>destination_id});
    doc.catalog_mut().unwrap().remove(b"Names");
    doc.catalog_mut().unwrap().set("Dests",dictionary! {"p0"=>vec![Object::Reference(pages[0]),Object::Name(b"Fit".to_vec())],"p1"=>named});
    // The shared fixture normally uses Names/Dests String operands. This variant
    // removes that namespace, so convert its two known local GoTo sites to Names.
    // The remote GoToR string deliberately remains unchanged.
    for (index, page) in pages.iter().enumerate() {
        let annotations = doc.get_dictionary(*page).unwrap().get(b"Annots").unwrap().as_array().unwrap().clone();
        for slot in [1, 4] {
            let link = doc.get_dictionary_mut(annotations[slot].as_reference().unwrap()).unwrap();
            let action = link.get_mut(b"A").unwrap().as_dict_mut().unwrap();
            let local = if slot == 1 { action } else {
                action.get_mut(b"Next").unwrap().as_array_mut().unwrap()[0].as_dict_mut().unwrap()
            };
            local.set("D", Object::Name(format!("p{}", 1-index).into_bytes()));
        }
    }
    let first = doc
        .get_dictionary(pages[0])
        .unwrap()
        .get(b"Annots")
        .unwrap()
        .as_array()
        .unwrap()[0]
        .as_reference()
        .unwrap();
    doc.get_dictionary_mut(first)
        .unwrap()
        .set("Dest", Object::Name(b"p1".to_vec()));
    let source = open(&engine, &temp, "legacy.pdf", &mut doc);
    let path = temp.path().join("saved.pdf");
    engine
        .export_pdf(
            ExportRequest {
                pages: plans(&[(&source, 1), (&source, 0)]),
                flatten: false,
            },
            &path,
        )
        .unwrap();
    let pdf = Document::load(&path).unwrap();
    let link = links(&pdf, 1).remove(0);
    let destination_value = link.get(b"Dest").unwrap();
    assert_eq!(destination(&pdf, destination_value), 0);
    let values = resolve(&pdf, destination_value).as_array().unwrap();
    assert_eq!(values[1].as_name().unwrap(), b"XYZ");
    assert!(matches!(values[2], Object::Null));
    assert_eq!(values[3].as_i64().unwrap(), 73);
    assert!(matches!(values[4], Object::Null));
    capture(
        "legacy",
        &[("source", &fs::read(temp.path().join("legacy.pdf")).unwrap())],
        &path,
    );
    engine.close_document(&source.id).unwrap();
}

#[test]
fn coexisting_legacy_and_modern_link_names_keep_distinct_pages_after_reorder() {
    let engine=engine();let temp=tempfile::tempdir().unwrap();let mut doc=fixture("Separate namespaces");
    let pages=doc.get_pages().values().copied().collect::<Vec<_>>();
    doc.catalog_mut().unwrap().set("Dests",dictionary!{"p1"=>vec![Object::Reference(pages[0]),Object::Name(b"Fit".to_vec())]});
    let first=doc.get_dictionary(pages[0]).unwrap().get(b"Annots").unwrap().as_array().unwrap()[0].as_reference().unwrap();
    let alias=doc.add_object(Object::Name(b"p1".to_vec()));doc.get_dictionary_mut(first).unwrap().set("Dest",alias);
    let source=open(&engine,&temp,"separate.pdf",&mut doc);let original=fs::read(temp.path().join("separate.pdf")).unwrap();
    let path=temp.path().join("reordered.pdf");
    engine.export_pdf(ExportRequest{pages:plans(&[(&source,1),(&source,0)]),flatten:false},&path).unwrap();
    let output=Document::load(&path).unwrap();let page_links=links(&output,1);
    assert_eq!(destination(&output,page_links[0].get(b"Dest").unwrap()),1,"Name p1 still targets original page zero");
    let action=resolve(&output,page_links[1].get(b"A").unwrap()).as_dict().unwrap();
    assert_eq!(destination(&output,action.get(b"D").unwrap()),0,"String p1 still targets original page one");
    assert_eq!(fs::read(temp.path().join("separate.pdf")).unwrap(),original);
    engine.close_document(&source.id).unwrap();
}

#[test]
fn malformed_and_cyclic_navigation_fail_without_overwriting_output() {
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    for kind in ["unresolved", "cycle", "bad-view", "bad-rect"] {
        let mut doc = fixture("Bad");
        let page = *doc.get_pages().values().next().unwrap();
        let annotations = doc
            .get_dictionary(page)
            .unwrap()
            .get(b"Annots")
            .unwrap()
            .as_array()
            .unwrap()
            .clone();
        let link = annotations[1].as_reference().unwrap();
        match kind {
            "unresolved"=>doc.get_dictionary_mut(link).unwrap().set("A",dictionary! {"S"=>"GoTo","D"=>Object::string_literal("missing")}),
            "cycle"=>{let action=doc.new_object_id();doc.objects.insert(action,dictionary! {"S"=>"URI","URI"=>Object::string_literal("https://example.invalid"),"Next"=>action}.into());doc.get_dictionary_mut(link).unwrap().set("A",action);},
            "bad-view"=>doc.get_dictionary_mut(link).unwrap().set("A",dictionary! {"S"=>"GoTo","D"=>vec![Object::Reference(page),Object::Name(b"XYZ".to_vec())]}),
            "bad-rect"=>doc.get_dictionary_mut(link).unwrap().set("Rect",vec![0.into(),1.into(),2.into()]),_=>unreachable!(),
        }
        let source = open(&engine, &temp, &format!("{kind}.pdf"), &mut doc);
        let original = fs::read(temp.path().join(format!("{kind}.pdf"))).unwrap();
        let path = temp.path().join(format!("{kind}-out.pdf"));
        let unchanged = temp.path().join(format!("{kind}-unchanged.pdf"));
        engine.export_pdf(ExportRequest {
            pages: plans(&[(&source, 0), (&source, 1)]), flatten: false,
        }, &unchanged).unwrap();
        assert_eq!(fs::read(&unchanged).unwrap(), original,
            "unchanged Save must preserve the source graph, including {kind}");
        fs::write(&path, b"preserve prior output").unwrap();
        assert!(
            engine
                .export_pdf(
                    ExportRequest {
                        pages: plans(&[(&source, 1), (&source, 0)]),
                        flatten: false
                    },
                    &path
                )
                .is_err(),
            "{kind}"
        );
        assert_eq!(fs::read(&path).unwrap(), b"preserve prior output");
        assert_eq!(
            fs::read(temp.path().join(format!("{kind}.pdf"))).unwrap(),
            original
        );
        engine.close_document(&source.id).unwrap();
    }
}

#[test]
fn repeated_large_actions_exceed_metadata_budget_before_publication() {
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    let mut doc = fixture("Large");
    let page = *doc.get_pages().values().next().unwrap();
    let link = doc
        .get_dictionary(page)
        .unwrap()
        .get(b"Annots")
        .unwrap()
        .as_array()
        .unwrap()[2]
        .as_reference()
        .unwrap();
    doc.get_dictionary_mut(link).unwrap().set("A",dictionary! {"S"=>"URI","URI"=>Object::String(vec![b'a';2*1024*1024],lopdf::StringFormat::Hexadecimal)});
    let source = open(&engine, &temp, "large.pdf", &mut doc);
    let path = temp.path().join("large-out.pdf");
    assert!(engine
        .export_pdf(
            ExportRequest {
                pages: plans(&[(&source, 0), (&source, 0), (&source, 0)]),
                flatten: false
            },
            &path
        )
        .is_err());
    assert!(!path.exists());
    engine.close_document(&source.id).unwrap();
}

#[path = "support/protected_fixture.rs"]
mod protected_fixture;
#[test]
fn indirect_actions_and_annotation_names_survive_save() {
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    let mut doc = fixture("Indirect");
    let pages: Vec<_> = doc.get_pages().values().copied().collect();
    for page in pages {
        let annotations = doc
            .get_dictionary(page)
            .unwrap()
            .get(b"Annots")
            .unwrap()
            .as_array()
            .unwrap()
            .clone();
        for annotation in annotations {
            let id = annotation.as_reference().unwrap();
            for key in [b"A".as_slice(), b"AA", b"NM", b"Subtype"] {
                if let Ok(value) = doc.get_dictionary(id).unwrap().get(key).cloned() {
                    let value = doc.add_object(value);
                    doc.get_dictionary_mut(id).unwrap().set(key, value);
                }
            }
        }
    }
    let source = open(&engine, &temp, "source.pdf", &mut doc);
    let path = temp.path().join("saved.pdf");
    engine
        .export_pdf(
            ExportRequest {
                pages: plans(&[(&source, 0), (&source, 1)]),
                flatten: false,
            },
            &path,
        )
        .unwrap();
    let pdf = Document::load(&path).unwrap();
    assert_links(&pdf, 0, Some(1), "Indirect");
    assert_links(&pdf, 1, Some(0), "Indirect");
    capture(
        "indirect",
        &[("source", &fs::read(temp.path().join("source.pdf")).unwrap())],
        &path,
    );
    engine.close_document(&source.id).unwrap();
}
#[test]
fn review_action_identity_targets_fail_before_publication() {
    rejected_action_cases(false);
}
#[test]
fn review_missing_goto_destination_fails_before_publication() {
    rejected_action_cases(true);
}
fn rejected_action_cases(missing_destination: bool) {
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    for location in ["A", "AA", "Next"] {
        let mut doc = fixture("Unsafe");
        let page = *doc.get_pages().values().next().unwrap();
        let target=doc.add_object(dictionary! {"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![0.into(),0.into(),10.into(),10.into()],"Contents"=>Object::string_literal("Target")});
        let annotations = doc
            .get_dictionary_mut(page)
            .unwrap()
            .get_mut(b"Annots")
            .unwrap()
            .as_array_mut()
            .unwrap();
        let link = annotations[1].as_reference().unwrap();
        annotations.push(Object::Reference(target));
        let action = if missing_destination {
            dictionary! {"S"=>"GoTo"}
        } else {
            dictionary! {"S"=>"Hide","T"=>target,"H"=>true}
        };
        match location {
            "A"=>doc.get_dictionary_mut(link).unwrap().set("A",action),
            "AA"=>doc.get_dictionary_mut(link).unwrap().set("AA",dictionary! {"E"=>action}),
            "Next"=>doc.get_dictionary_mut(link).unwrap().set("A",dictionary! {"S"=>"URI","URI"=>Object::string_literal("https://example.invalid"),"Next"=>vec![Object::Dictionary(action)]}),
            _=>unreachable!(),
        }
        let source = open(&engine, &temp, &format!("{location}.pdf"), &mut doc);
        let original = fs::read(temp.path().join(format!("{location}.pdf"))).unwrap();
        let output = temp.path().join(format!("{location}-output.pdf"));
        let unchanged = temp.path().join(format!("{location}-unchanged.pdf"));
        engine.export_pdf(ExportRequest {
            pages: plans(&[(&source, 0), (&source, 1)]), flatten: false,
        }, &unchanged).unwrap();
        assert_eq!(fs::read(&unchanged).unwrap(), original,
            "unchanged Save detached an action's original graph");
        fs::write(&output, b"unchanged output").unwrap();
        assert!(
            engine
                .export_pdf(
                    ExportRequest {
                        pages: plans(&[(&source, 1), (&source, 0)]),
                        flatten: false
                    },
                    &output
                )
                .is_err(),
            "{location}: unsafe action silently published"
        );
        assert_eq!(fs::read(&output).unwrap(), b"unchanged output");
        assert_eq!(
            fs::read(temp.path().join(format!("{location}.pdf"))).unwrap(),
            original
        );
        engine.close_document(&source.id).unwrap();
    }
}
#[test]
fn data_only_actions_and_shared_script_streams_survive_repeated_saves() {
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    let mut doc = fixture("Actions");
    let script = doc.add_object(Stream::new(
        dictionary! {},
        b"/* inert synthetic action */".to_vec(),
    ));
    let pages: Vec<_> = doc.get_pages().values().copied().collect();
    for page in pages {
        let link = doc
            .get_dictionary(page)
            .unwrap()
            .get(b"Annots")
            .unwrap()
            .as_array()
            .unwrap()[4]
            .as_reference()
            .unwrap();
        let next = doc
            .get_dictionary_mut(link)
            .unwrap()
            .get_mut(b"A")
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .get_mut(b"Next")
            .unwrap()
            .as_array_mut()
            .unwrap();
        next.push(Object::Dictionary(
            dictionary! {"S"=>"Named","N"=>"NextPage"},
        ));
        next.push(Object::Dictionary(
            dictionary! {"S"=>"JavaScript","JS"=>script},
        ));
    }
    let mut source = open(&engine, &temp, "source.pdf", &mut doc);
    let original = fs::read(temp.path().join("source.pdf")).unwrap();
    let mut object_count = None;
    for round in 0..6 {
        let output = temp.path().join(format!("save-{round}.pdf"));
        engine
            .export_pdf(
                ExportRequest {
                    pages: plans(&[(&source, 0), (&source, 1)]),
                    flatten: false,
                },
                &output,
            )
            .unwrap();
        let pdf = Document::load(&output).unwrap();
        for page in 0..2 {
            let links = links(&pdf, page);
            assert_eq!(destination(&pdf, links[0].get(b"Dest").unwrap()), 1 - page);
            let next = resolve(&pdf, links[4].get(b"A").unwrap())
                .as_dict()
                .unwrap()
                .get(b"Next")
                .unwrap()
                .as_array()
                .unwrap();
            assert_eq!(next.len(), 4);
            let named = resolve(&pdf, &next[2]).as_dict().unwrap();
            assert_eq!(named.get(b"S").unwrap().as_name().unwrap(), b"Named");
            assert_eq!(named.get(b"N").unwrap().as_name().unwrap(), b"NextPage");
            let js = resolve(&pdf, &next[3]).as_dict().unwrap();
            assert_eq!(js.get(b"S").unwrap().as_name().unwrap(), b"JavaScript");
            let stream = resolve(&pdf, js.get(b"JS").unwrap()).as_stream().unwrap();
            assert_eq!(
                stream.get_plain_content().unwrap(),
                b"/* inert synthetic action */"
            );
        }
        if let Some(count) = object_count {
            assert_eq!(pdf.objects.len(), count, "repeated-save object growth");
        } else {
            object_count = Some(pdf.objects.len());
        }
        let reopened = engine.open_document(&output).unwrap();
        engine.close_document(&source.id).unwrap();
        source = reopened;
    }
    assert_eq!(fs::read(temp.path().join("source.pdf")).unwrap(), original);
    engine.close_document(&source.id).unwrap();
}
#[test]
fn protected_links_survive_save_without_changing_permissions() {
    use lopdf::encryption::Permissions;
    let engine = engine();
    let temp = tempfile::tempdir().unwrap();
    let mut doc = fixture("Protected");
    let mut raw = Vec::new();
    doc.save_to(&mut raw).unwrap();
    let encrypted = protected_fixture::encrypt(&raw, Permissions::COPYABLE);
    let source_path = temp.path().join("protected.pdf");
    fs::write(&source_path, &encrypted).unwrap();
    let source = engine
        .open_document_with_password(&source_path, "reader")
        .unwrap();
    let path = temp.path().join("saved.pdf");
    engine
        .export_pdf(
            ExportRequest {
                pages: plans(&[(&source, 0), (&source, 1)]),
                flatten: false,
            },
            &path,
        )
        .unwrap();
    let reopened = engine.open_document_with_password(&path, "reader").unwrap();
    assert_eq!(
        reopened.protection.as_ref().unwrap().permissions,
        source.protection.as_ref().unwrap().permissions
    );
    let bytes = engine.source_bytes(&reopened.id).unwrap();
    let pdf = Document::load_mem(&bytes).unwrap();
    assert_links(&pdf, 0, Some(1), "Protected");
    assert_links(&pdf, 1, Some(0), "Protected");
    assert_eq!(fs::read(&source_path).unwrap(), encrypted);
    capture("protected", &[("source", &encrypted)], &path);
    engine.close_document(&reopened.id).unwrap();
    engine.close_document(&source.id).unwrap();
}
