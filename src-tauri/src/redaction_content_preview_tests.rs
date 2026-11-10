use super::*;
use lopdf::{dictionary, Document, Object, Stream};

fn fixture() -> (WorkerRuntime, String, DocumentInfo, Vec<TextRedactionMark>) {
    fixture_text("Left SECRET Right", 56.1)
}
fn fixture_text(
    text: &str,
    left: f64,
) -> (WorkerRuntime, String, DocumentInfo, Vec<TextRedactionMark>) {
    let (mut rt, owner, original, marks) = registered_fixture_text(text, left);
    let source = rt.prepare_redaction_source(
        &owner,
        ExportRequest {
            pages: (0..2).map(|page_index| PagePlan {
                id: format!("page-{page_index}"),
                source_id: original.id.clone(),
                page_index,
                width: 200.,
                height: 180.,
                rotation: 0,
                crop: None,
                overlays: vec![],
            }).collect(),
            flatten: false,
        },
        Some(vec![]),
    ).unwrap().document;
    (rt, owner, source, marks)
}
fn registered_fixture_text(
    text: &str,
    left: f64,
) -> (WorkerRuntime, String, DocumentInfo, Vec<TextRedactionMark>) {
    let mut rt = WorkerRuntime::new(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"),
        Arc::new(FontRegistry::new()),
    )
    .unwrap();
    let mut pdf = Document::new();
    let pages = pdf.new_object_id();
    let font=pdf.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Courier","Encoding"=>"WinAnsiEncoding"});
    let image=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>64,"Height"=>48,"BitsPerComponent"=>8,"ColorSpace"=>"DeviceRGB","Filter"=>"DCTDecode"},include_bytes!("../tests/fixtures/redaction-jpeg/rgb-sub2-progressive1.jpg").to_vec()));
    let mut kids = vec![];
    let program=format!("BT /F1 12 Tf 1 0 0 1 20 150 Tm ({text}) Tj ET q 1 0 0 rg 20 25 30 10 re f Q q 96 0 0 72 20 50 cm /Image Do Q");
    for program in [program.as_bytes(), b""] {
        let content = pdf.add_object(Stream::new(dictionary! {}, program.to_vec()));
        let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),200.into(),180.into()],"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font},"XObject"=>dictionary!{"Image"=>image}},"Contents"=>content});
        kids.push(Object::Reference(page));
    }
    pdf.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Count"=>2,"Kids"=>kids}.into(),
    );
    let root = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    pdf.trailer.set("Root", root);
    let mut bytes = vec![];
    pdf.save_to(&mut bytes).unwrap();
    let source = rt
        .register_generated_document(bytes, "combined-source.pdf")
        .unwrap();
    let owner = rt.begin_redaction_session().unwrap();
    let marks = [
        (0, left, 20., 42., 14.),
        (0, 22., 148., 8., 4.),
        (0, 30., 70., 4., 4.),
        (1, 0., 0., 1., 1.),
    ]
    .into_iter()
    .map(|(page_index, x, y, width, height)| TextRedactionMark {
        page_index,
        rect: TextMarkRect {
            x,
            y,
            width,
            height,
        },
    })
    .collect();
    (rt, owner, source, marks)
}

#[test]
fn redaction_content_preview_rejects_unprepared_registered_source_without_allocation() {
    let (mut rt, owner, source, marks) = registered_fixture_text("Left SECRET Right", 56.1);
    let before = rt.document(&source.id).unwrap().bytes().unwrap();
    let count = rt.documents.len();
    assert!(rt.preview_content_redaction(&owner, &source.id, &marks).is_err());
    assert_eq!(rt.documents.len(), count);
    assert!(rt.redaction_previews.pending.is_empty());
    assert_eq!(&*before, &*rt.document(&source.id).unwrap().bytes().unwrap());
}

#[test]
fn redaction_content_preview_owns_mixed_candidate_and_requires_review_before_apply() {
    let (mut rt, owner, source, marks) = fixture();
    let original = rt.document(&source.id).unwrap().bytes().unwrap();
    let preview = rt
        .preview_content_redaction(&owner, &source.id, &marks)
        .unwrap();
    assert_eq!(
        (
            preview.removed_characters,
            preview.removed_paths,
            preview.affected_images
        ),
        (6, 1, 1)
    );
    assert_eq!(preview.pages.len(), 1);
    assert_eq!(preview.pages[0].page_index, 0);
    assert!(preview.pages[0].extends_beyond_marks);
    assert_eq!(preview.source_id, source.id);
    assert_eq!(preview.candidate.pages.len(), 2);
    assert_eq!(&*rt.document(&source.id).unwrap().bytes().unwrap(), &*original);
    assert!(rt.apply_redaction(&owner, &preview.preview_id).is_err());
    assert!(rt
        .redaction_previews
        .pending
        .contains_key(&preview.preview_id));
    rt.discard_redaction(&owner, &preview.preview_id).unwrap();
    assert!(!rt.documents.contains_key(&preview.candidate.id));
    assert!(rt.documents.contains_key(&source.id));
}

#[test]
fn redaction_content_preview_image_only_budget_and_owner_refusals_are_atomic() {
    let (mut rt, owner, source, marks) = fixture();
    let preview = rt
        .preview_content_redaction(&owner, &source.id, &marks[2..3])
        .unwrap();
    assert_eq!(
        (
            preview.removed_characters,
            preview.removed_paths,
            preview.affected_images
        ),
        (0, 0, 1)
    );
    let other = rt.begin_redaction_session().unwrap();
    assert!(rt.discard_redaction(&other, &preview.preview_id).is_err());
    let count = rt.documents.len();
    rt.redaction_previews.geometry_limit = 0;
    assert!(rt
        .preview_content_redaction(&owner, &source.id, &marks)
        .is_err());
    assert_eq!(rt.documents.len(), count);
    rt.release_redaction_owner(&owner);
    assert!(!rt.documents.contains_key(&preview.candidate.id));
    assert!(rt
        .preview_content_redaction(&owner, &source.id, &marks)
        .is_err());
}

#[test]
fn redaction_content_preview_paired_raster_binds_dimensions_and_reviewed_candidate() {
    let (mut rt, owner, source, marks) = fixture();
    let preview = rt
        .preview_content_redaction(&owner, &source.id, &marks)
        .unwrap();
    let other = rt.begin_redaction_session().unwrap();
    assert!(rt
        .render_content_redaction(&other, &source.id, Some(&preview.preview_id), 0, 900)
        .is_err());
    assert!(rt
        .render_content_redaction(&owner, &source.id, Some(&preview.preview_id), 0, 1201)
        .is_err());
    let raster = rt
        .render_content_redaction(&owner, &source.id, Some(&preview.preview_id), 0, 900)
        .unwrap();
    assert_eq!((raster.width, raster.height), (900, 810));
    assert_eq!(raster.source_id, source.id);
    assert_eq!(
        raster.candidate_id.as_deref(),
        Some(preview.candidate.id.as_str())
    );
    assert_eq!(
        raster.preview_id.as_deref(),
        Some(preview.preview_id.as_str())
    );
    let source_image = image::load_from_memory(&raster.source_png)
        .unwrap()
        .to_rgba8();
    let candidate_image = image::load_from_memory(raster.candidate_png.as_ref().unwrap())
        .unwrap()
        .to_rgba8();
    let disclosure = image::load_from_memory(raster.disclosure_png.as_ref().unwrap())
        .unwrap()
        .to_rgba8();
    assert_eq!(source_image.dimensions(), (900, 810));
    assert_eq!(candidate_image.dimensions(), source_image.dimensions());
    assert_eq!(disclosure.dimensions(), source_image.dimensions());
    let mut changed = 0;
    for ((before, after), mask) in source_image
        .pixels()
        .zip(candidate_image.pixels())
        .zip(disclosure.pixels())
    {
        if before != after {
            changed += 1;
            assert_ne!(mask[3], 0);
        }
    }
    assert!(changed > 100);
    assert!(disclosure.pixels().any(|p| p[0] == 220 && p[3] != 0));
    assert!(disclosure.pixels().any(|p| p[0] == 245 && p[3] != 0));
    let applied = rt.apply_redaction(&owner, &preview.preview_id).unwrap();
    assert_eq!(applied.id, preview.candidate.id);
    assert!(rt.apply_redaction(&owner, &preview.preview_id).is_err());
}

#[test]
fn redaction_content_preview_initial_raster_requires_owned_prepared_source() {
    let (mut rt, owner, source, _) = registered_fixture_text("Left SECRET Right", 56.1);
    assert!(rt
        .render_content_redaction(&owner, &source.id, None, 0, 900)
        .is_err());
    let prepared = rt
        .prepare_redaction_source(
            &owner,
            ExportRequest {
                pages: vec![PagePlan {
                    id: "first".into(),
                    source_id: source.id.clone(),
                    page_index: 0,
                    width: 200.,
                    height: 180.,
                    rotation: 0,
                    crop: None,
                    overlays: vec![],
                }],
                flatten: false,
            },
            Some(vec![]),
        )
        .unwrap();
    let other = rt.begin_redaction_session().unwrap();
    assert!(rt
        .render_content_redaction(&other, &prepared.document.id, None, 0, 900)
        .is_err());
    let raster = rt
        .render_content_redaction(&owner, &prepared.document.id, None, 0, 64)
        .unwrap();
    assert_eq!((raster.width, raster.height), (64, 58));
    assert!(
        raster.candidate_id.is_none()
            && raster.candidate_png.is_none()
            && raster.disclosure_png.is_none()
    );
    rt.release_redaction_owner(&owner);
    assert!(rt
        .render_content_redaction(&owner, &prepared.document.id, None, 0, 64)
        .is_err());
}

#[test]
fn redaction_content_preview_refuses_even_one_changed_retained_text_pixel() {
    // Leading removal splits the original Tj. At this viewport its different
    // native float accumulation changes one retained pixel by one gray level.
    let (mut rt, owner, source, marks) = fixture_text("SECRET keep", 20.1);
    let original = rt.document(&source.id).unwrap().bytes().unwrap();
    let preview = rt
        .preview_content_redaction(&owner, &source.id, &marks)
        .unwrap();
    let error = rt
        .render_content_redaction(&owner, &source.id, Some(&preview.preview_id), 0, 900)
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("outside the disclosed content bounds"));
    assert!(rt.apply_redaction(&owner, &preview.preview_id).is_err());
    assert_eq!(&*original, &*rt.document(&source.id).unwrap().bytes().unwrap());
    rt.discard_redaction(&owner, &preview.preview_id).unwrap();
    assert!(!rt.documents.contains_key(&preview.candidate.id));
}

#[test]
fn redaction_content_preview_dropped_replies_close_candidates_and_do_not_review_pages() {
    let (mut rt, owner, source, marks) = fixture();
    let count = rt.documents.len();
    let (reply, receive) = mpsc::channel();
    drop(receive);
    rt.handle_text_redaction(Request::PreviewContent {
        owner: owner.clone(),
        source: source.id.clone(),
        marks: marks.clone(),
        reply,
    });
    assert_eq!(rt.documents.len(), count);
    assert!(rt.redaction_previews.pending.is_empty());
    let preview = rt
        .preview_content_redaction(&owner, &source.id, &marks)
        .unwrap();
    let (reply, receive) = mpsc::channel();
    drop(receive);
    rt.handle_text_redaction(Request::RenderContent {
        owner: owner.clone(),
        source: source.id.clone(),
        preview: Some(preview.preview_id.clone()),
        page: 0,
        width: 900,
        reply,
    });
    assert!(rt.apply_redaction(&owner, &preview.preview_id).is_err());
    rt.render_content_redaction(&owner, &source.id, Some(&preview.preview_id), 0, 900)
        .unwrap();
    let (reply, receive) = mpsc::channel();
    drop(receive);
    rt.handle_text_redaction(Request::RenderContent {
        owner: owner.clone(),
        source: source.id.clone(),
        preview: Some(preview.preview_id.clone()),
        page: 0,
        width: 900,
        reply,
    });
    // A lost re-render reply does not undo an earlier successful review.
    assert!(rt.apply_redaction(&owner, &preview.preview_id).is_ok());
}

#[test]
fn redaction_content_preview_expiry_and_source_close_invalidate_the_pair() {
    for expire in [false, true] {
        let (mut rt, owner, source, marks) = fixture();
        let preview = rt
            .preview_content_redaction(&owner, &source.id, &marks)
            .unwrap();
        if expire {
            rt.redaction_previews
                .pending
                .get_mut(&preview.preview_id)
                .unwrap()
                .created = Instant::now() - TTL - Duration::from_secs(1);
        } else {
            rt.close_document(&source.id).unwrap();
        }
        assert!(rt
            .render_content_redaction(&owner, &source.id, Some(&preview.preview_id), 0, 900)
            .is_err());
        assert!(rt.apply_redaction(&owner, &preview.preview_id).is_err());
        assert!(!rt.documents.contains_key(&preview.candidate.id));
    }
}
