use folio_engine::{ExportRequest, ImageFitMode, PagePlan, PdfEngine};
use folio_engine::{ExistingImageEdit, ImageTransform};
#[path = "support/existing_image_crop_cases.rs"]
mod image_crop_cases;
use lopdf::{dictionary, Document, Object, Stream};
use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};
fn engine() -> PdfEngine {
    PdfEngine::start(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/pdfium/pdfium.dll"))
        .unwrap()
}
fn unchanged_transform() -> ImageTransform {
    ImageTransform {delta_x:0.,delta_y:0.,scale_x:1.,scale_y:1.,rotation_degrees:0.}
}
#[test]
fn moving_an_entire_unit_image_container_keeps_all_image_pixels() {
    let e=engine();let temp=tempfile::tempdir().unwrap();
    for rotation in [0,90,180,270] {
        let input=temp.path().join(format!("container-{rotation}.pdf"));fixture(&input,rotation,1.7);
        let original=fs::read(&input).unwrap();let source=e.open_document(&input).unwrap();
        let selected=e.list_existing_images(&source.id,0).unwrap().images.remove(0);
        let candidate=e.preview_image_edit_occurrence(&source.id,&[0,1],0,&selected.object_path,&selected.selection_id,
            ExistingImageEdit::Adjust {transform:ImageTransform {delta_x:17.,..unchanged_transform()},crop:folio_engine::ImageCrop::FULL}).unwrap();
        // Independent expected content moves the original whole Form invocation.
        // Its existing unit BBox moves with the image; no mask permits lost ink.
        let mut expected=Document::load_mem(&original).unwrap();let page=*expected.get_pages().values().next().unwrap();
        let mut content=lopdf::content::Content::decode_strict(&expected.get_page_content_with_limit(page,1024*1024).unwrap()).unwrap();
        let matrix=&mut content.operations.iter_mut().find(|op|op.operator=="cm").unwrap().operands;
        let (axis,value)=match rotation {0=>(4,30),90=>(5,40),180=>(4,10),270=>(5,20),_=>unreachable!()};matrix[axis]=value.into();
        let stream=expected.add_object(Stream::new(dictionary!{},content.encode().unwrap()));expected.get_dictionary_mut(page).unwrap().set("Contents",stream);
        let golden=temp.path().join(format!("golden-{rotation}.pdf"));expected.save(&golden).unwrap();let golden=e.open_document(&golden).unwrap();
        for width in [680,1360] {assert!(e.render_page(&candidate.document.id,0,width).unwrap()==e.render_page(&golden.id,0,width).unwrap(),"full-image pixels, rotation {rotation}, width {width}");}
        assert_eq!(e.render_page(&source.id,1,680).unwrap(),e.render_page(&candidate.document.id,1,680).unwrap());
        let saved=temp.path().join(format!("moved-{rotation}.pdf"));save(&e,&candidate.document,&saved);let reopened=e.open_document(&saved).unwrap();
        let selected=e.list_existing_images(&reopened.id,0).unwrap().images.remove(0);
        let unchanged=e.preview_image_edit_occurrence(&reopened.id,&[0,1],0,&selected.object_path,&selected.selection_id,
            ExistingImageEdit::Adjust {transform:unchanged_transform(),crop:folio_engine::ImageCrop::FULL}).unwrap();
        assert_eq!(e.render_page(&unchanged.document.id,0,680).unwrap(),e.render_page(&golden.id,0,680).unwrap());
        assert_eq!(fs::read(input).unwrap(),original);
        for id in [&source.id,&candidate.document.id,&golden.id,&reopened.id,&unchanged.document.id] {e.close_document(id).unwrap();}
    }
}
#[test]
fn positions_physical_images_then_removes_only_selected_saved_occurrence() {
    let e=engine(); let temp=tempfile::tempdir().unwrap();
    for rotation in [0,90,180,270] {
        let path=temp.path().join(format!("position-{rotation}.pdf")); fixture(&path,rotation,1.7);
        let bytes=fs::read(&path).unwrap(); let source=e.open_document(&path).unwrap();
        let before=e.list_existing_images(&source.id,0).unwrap().images;
        let selected=&before[0];
        let edit=ExistingImageEdit::Transform{transform:ImageTransform{delta_x:17.,delta_y:-8.5,scale_x:0.7,scale_y:0.8,rotation_degrees:30.}};
        let changed=e.preview_image_edit_occurrence(&source.id,&[0,1,0],2,&selected.object_path,&selected.selection_id,edit).unwrap();
        assert_eq!(changed.page_indices,vec![0,1,2]);
        for i in [0,1] { assert_eq!(e.render_page(&source.id,i,800).unwrap(),e.render_page(&changed.document.id,i,800).unwrap()); }
        let after=e.list_existing_images(&changed.document.id,2).unwrap().images;
        assert_eq!(after.len(),2); assert_eq!(after[1].corners,before[1].corners);
        let center=|v:&folio_engine::ExistingImage| ((v.corners[0].x+v.corners[2].x) as f64/2.,(v.corners[0].y+v.corners[2].y) as f64/2.);
        let (cx,cy)=center(selected); let (ax,ay)=center(&after[0]);
        assert!((ax-cx-17.).abs()<0.002);assert!((ay-cy+8.5).abs()<0.002);
        let (s,c)=30f64.to_radians().sin_cos();
        for (axis,scale) in [(1,0.7),(3,0.8)] {
            let x=(selected.corners[axis].x-selected.corners[0].x) as f64*scale;
            let y=(selected.corners[axis].y-selected.corners[0].y) as f64*scale;
            assert!(((after[0].corners[axis].x-after[0].corners[0].x) as f64-(c*x-s*y)).abs()<0.002);
            assert!(((after[0].corners[axis].y-after[0].corners[0].y) as f64-(s*x+c*y)).abs()<0.002);
        }
        let output=temp.path().join(format!("moved-{rotation}.pdf"));save(&e,&changed.document,&output);
        artifact(&format!("Position-original-{rotation}.pdf"),&bytes);artifact(&format!("Position-edited-{rotation}.pdf"),&fs::read(&output).unwrap());
        let reopened=e.open_document(&output).unwrap();let selected=e.list_existing_images(&reopened.id,2).unwrap().images.remove(0);
        let noop=e.preview_image_edit_occurrence(&reopened.id,&[0,1,2],2,&selected.object_path,&selected.selection_id,ExistingImageEdit::Transform{transform:unchanged_transform()}).unwrap();
        assert_eq!(e.render_page(&reopened.id,2,800).unwrap(),e.render_page(&noop.document.id,2,800).unwrap());
        let removed=e.preview_image_edit_occurrence(&reopened.id,&[0,1,2],2,&selected.object_path,&selected.selection_id,ExistingImageEdit::Remove).unwrap();
        let images=e.list_existing_images(&removed.document.id,2).unwrap().images;
        assert_eq!(images.len(),1);assert_eq!(images[0].corners,before[1].corners);
        let deleted=temp.path().join(format!("removed-{rotation}.pdf"));save(&e,&removed.document,&deleted);artifact(&format!("Position-removed-{rotation}.pdf"),&fs::read(&deleted).unwrap());
        let deleted=e.open_document(&deleted).unwrap();assert_eq!(e.list_existing_images(&deleted.id,2).unwrap().images.len(),1);
        assert_eq!(fs::read(&path).unwrap(),bytes);
        for id in [&source.id,&changed.document.id,&reopened.id,&noop.document.id,&removed.document.id,&deleted.id] {e.close_document(id).unwrap();}
    }
}
#[test]
fn image_position_requests_reject_invalid_values_and_stale_identity() {
    let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("image.pdf");fixture(&path,0,1.);
    let source=e.open_document(&path).unwrap();let image=e.list_existing_images(&source.id,0).unwrap().images.remove(0);
    for transform in [ImageTransform{delta_x:f64::NAN,..unchanged_transform()},ImageTransform{delta_y:1e6+1.,..unchanged_transform()},ImageTransform{scale_x:0.,..unchanged_transform()},ImageTransform{scale_y:1001.,..unchanged_transform()},ImageTransform{rotation_degrees:361.,..unchanged_transform()}] {
        assert!(e.preview_image_edit_occurrence(&source.id,&[0,1],0,&image.object_path,&image.selection_id,ExistingImageEdit::Transform{transform}).is_err());
    }
    assert!(e.preview_image_edit_occurrence(&source.id,&[0,1],0,&image.object_path,&"0".repeat(64),ExistingImageEdit::Remove).is_err());
    assert!(e.preview_image_edit_occurrence(&source.id,&[0,1],2,&image.object_path,&image.selection_id,ExistingImageEdit::Remove).is_err());
    e.close_document(&source.id).unwrap();
}
#[test]
fn repeated_saved_reflected_positions_preserve_clipping_and_reveal_underlying_paint_on_remove() {
    let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("reflected.pdf");fixture(&path,0,1.);
    let mut pdf=Document::load(&path).unwrap();let page=*pdf.get_pages().values().next().unwrap();
    let contents=pdf.add_object(Stream::new(dictionary!{},b"q 0 0 1 rg 50 20 150 85 re f Q q -100 15 25 50 160 30 cm /Shared Do Q q 100 0 0 50 220 30 cm /Shared Do Q q 0 0 0 rg 120 20 5 100 re f Q BT /F 12 Tf 20 160 Td (Keep all text and spaces) Tj ET".to_vec()));
    pdf.get_dictionary_mut(page).unwrap().set("Contents",contents);pdf.save(&path).unwrap();
    let original=fs::read(&path).unwrap();artifact("Position-affine-original.pdf",&original);let source=e.open_document(&path).unwrap();let mut current=source.clone();let mut count=None;
    for i in 0..8 {
        let selected=e.list_existing_images(&current.id,0).unwrap().images.remove(0);
        artifact(&format!("Position-affine-before-{i}.pdf"),&fs::read(if i==0{path.clone()}else{temp.path().join(format!("round-{}.pdf",i-1))}).unwrap());
        let changed=e.preview_image_edit_occurrence(&current.id,&[0,1],0,&selected.object_path,&selected.selection_id,ExistingImageEdit::Transform{transform:ImageTransform{delta_x:1.,delta_y:-0.5,scale_x:0.99,scale_y:1.01,rotation_degrees:2.}}).unwrap_or_else(|error|panic!("round {i}: {error}"));
        let file=temp.path().join(format!("round-{i}.pdf"));save(&e,&changed.document,&file);
        let saved=fs::read(&file).unwrap();let object_count=Document::load_mem(&saved).unwrap().objects.len();
        if let Some(n)=count {assert_eq!(object_count,n);} else {count=Some(object_count);}
        let reopened=e.open_document(&file).unwrap();let image=e.list_existing_images(&reopened.id,0).unwrap().images.remove(0);
        let a=&image.corners;assert!((a[1].x-a[0].x)*(a[3].y-a[0].y)-(a[1].y-a[0].y)*(a[3].x-a[0].x)>0.,"reflection retained in displayed coordinates");
        e.close_document(&changed.document.id).unwrap();if current.id!=source.id{e.close_document(&current.id).unwrap();}current=reopened;
        if i==7{artifact("Position-affine-edited.pdf",&saved);}
    }
    let selected=e.list_existing_images(&current.id,0).unwrap().images.remove(0);
    let removed=e.preview_image_edit_occurrence(&current.id,&[0,1],0,&selected.object_path,&selected.selection_id,ExistingImageEdit::Remove).unwrap();
    let file=temp.path().join("removed.pdf");save(&e,&removed.document,&file);artifact("Position-affine-removed.pdf",&fs::read(&file).unwrap());
    let pixels=image::load_from_memory(&e.render_page(&removed.document.id,0,800).unwrap()).unwrap().to_rgba8();
    assert_eq!(pixels.get_pixel(160,280).0,[0,0,255,255],"underlying blue paint revealed");
    assert_eq!(pixels.get_pixel(244,280).0,[0,0,0,255],"later black paint retained");
    assert_eq!(e.render_page(&source.id,1,800).unwrap(),e.render_page(&removed.document.id,1,800).unwrap());
    assert_eq!(fs::read(&path).unwrap(),original);
    for id in [&source.id,&current.id,&removed.document.id]{e.close_document(id).unwrap();}
}
fn artifact(name: &str, bytes: &[u8]) {
    if let Some(root) = std::env::var_os("FOLIO_EXISTING_IMAGE_CORPUS") {
        let root = PathBuf::from(root);
        fs::create_dir_all(&root).unwrap();
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join(name))
            .unwrap();
        file.write_all(bytes).unwrap();
    }
}
fn fixture(path: &Path, rotation: i32, unit: f32) {
    let mut pdf = Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let image=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>2,"Height"=>1,"ColorSpace"=>"DeviceRGB","BitsPerComponent"=>8},vec![255,0,0,0,255,0]));
    let leaf=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),1.into(),1.into()],"Resources"=>dictionary!{"XObject"=>dictionary!{"I"=>image}}},b"/I Do".to_vec()));
    let font =
        pdf.add_object(dictionary! {"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica"});
    let resources =
        dictionary! {"XObject"=>dictionary!{"Shared"=>leaf},"Font"=>dictionary!{"F"=>font}};
    let content=pdf.add_object(Stream::new(dictionary!{},b"q 100 0 0 50 20 30 cm /Shared Do Q q 100 0 0 50 220 30 cm /Shared Do Q BT /F 12 Tf 20 160 Td (Keep all text and spaces) Tj ET".to_vec()));
    let note=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Text","Rect"=>vec![350.into(),150.into(),365.into(),165.into()],"Contents"=>Object::string_literal("Keep image note"),"NM"=>Object::string_literal("image-note")});
    let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![(-10).into(),(-10).into(),410.into(),210.into()],"CropBox"=>vec![0.into(),0.into(),400.into(),200.into()],"Rotate"=>rotation,"UserUnit"=>unit,"Resources"=>resources.clone(),"Contents"=>content,"Annots"=>vec![Object::Reference(note)]});
    let other = pdf.add_object(Stream::new(
        dictionary! {},
        b"BT /F 12 Tf 20 160 Td (Other page stays) Tj ET".to_vec(),
    ));
    let other_page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),400.into(),200.into()],"Resources"=>resources,"Contents"=>other});
    pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![Object::Reference(page),Object::Reference(other_page)],"Count"=>2}.into());
    let root = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    pdf.trailer.set("Root", root);
    pdf.save(path).unwrap();
}
fn asset(e: &PdfEngine) -> folio_engine::ImageRegistration {
    let image = image::RgbaImage::from_pixel(8, 8, image::Rgba([0, 0, 255, 128]));
    let mut data = Cursor::new(vec![]);
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut data, image::ImageFormat::Png)
        .unwrap();
    e.images().register(data.into_inner()).unwrap()
}
fn save(e: &PdfEngine, doc: &folio_engine::DocumentInfo, path: &Path) {
    e.export_pdf(
        ExportRequest {
            flatten: false,
            pages: doc
                .pages
                .iter()
                .enumerate()
                .map(|(i, p)| PagePlan {
                    id: format!("p{i}"),
                    source_id: doc.id.clone(),
                    page_index: i,
                    width: p.width,
                    height: p.height,
                    rotation: 0,
                    crop: None,
                    overlays: vec![],
                })
                .collect(),
        },
        path,
    )
    .unwrap();
}
#[test]
fn replaces_only_the_selected_shared_image_and_retains_complete_source() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("images.pdf");
    fixture(&path, 0, 1.);
    let original = fs::read(&path).unwrap();
    artifact("Original.pdf", &original);
    let source = e.open_document(&path).unwrap();
    let imported = asset(&e);
    let list = e.list_existing_images(&source.id, 0).unwrap();
    assert_eq!(list.images.len(), 2);
    let selected = &list.images[0];
    assert_eq!(selected.bounds.x, 20.);
    assert_eq!(selected.bounds.width, 100.);
    let changed = e
        .preview_image_replacement_occurrence(
            &source.id,
            &[0, 1, 0],
            2,
            &selected.object_path,
            &selected.selection_id,
            &imported.image.id,
            ImageFitMode::Fit,
        )
        .unwrap();
    assert_eq!(changed.page_indices, vec![0, 1, 2]);
    assert_eq!(changed.document.pages.len(), 3);
    assert_eq!(
        e.render_page(&source.id, 0, 800).unwrap(),
        e.render_page(&changed.document.id, 0, 800).unwrap()
    );
    assert_eq!(
        e.render_page(&source.id, 1, 800).unwrap(),
        e.render_page(&changed.document.id, 1, 800).unwrap()
    );
    let after = e.list_existing_images(&changed.document.id, 2).unwrap();
    assert_eq!(after.images.len(), 2);
    assert_eq!(after.images[0].pixel_width, 8);
    assert_eq!(after.images[1].pixel_width, 2);
    let before_text = e.extract_text(&source.id, 0).unwrap();
    let after_text = e.extract_text(&changed.document.id, 2).unwrap();
    assert_eq!(before_text, after_text);
    let before = image::load_from_memory(&e.render_page(&source.id, 0, 800).unwrap())
        .unwrap()
        .to_rgba8();
    let after = image::load_from_memory(&e.render_page(&changed.document.id, 2, 800).unwrap())
        .unwrap()
        .to_rgba8();
    for y in 0..before.height() {
        for x in 300..before.width() {
            assert_eq!(
                before.get_pixel(x, y),
                after.get_pixel(x, y),
                "untouched at {x},{y}"
            );
        }
    }
    let output = temp.path().join("Edited.pdf");
    save(&e, &changed.document, &output);
    artifact("Edited-duplicate.pdf", &fs::read(&output).unwrap());
    let reopened = e.open_document(&output).unwrap();
    assert_eq!(
        e.render_page(&changed.document.id, 2, 800).unwrap(),
        e.render_page(&reopened.id, 2, 800).unwrap()
    );
    assert!(e
        .list_native_comments(&reopened.id)
        .unwrap()
        .iter()
        .any(|c| c.page_index == 2 && c.text == "Keep image note"));
    assert_eq!(fs::read(&path).unwrap(), original);
    for id in [&reopened.id, &changed.document.id, &source.id] {
        e.close_document(id).unwrap();
    }
    e.images().release(&imported.token).unwrap();
}
#[test]
fn physical_crop_rotation_frames_survive_saved_replacement_and_reedit() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let imported = asset(&e);
    for rotation in [0, 90, 180, 270] {
        let path = temp.path().join(format!("geometry{rotation}.pdf"));
        fixture(&path, rotation, 1.7);
        let source = e.open_document(&path).unwrap();
        let original = e
            .list_existing_images(&source.id, 0)
            .unwrap()
            .images
            .remove(0);
        let changed = e
            .preview_image_replacement_occurrence(
                &source.id,
                &[0, 1],
                0,
                &original.object_path,
                &original.selection_id,
                &imported.image.id,
                ImageFitMode::Fill,
            )
            .unwrap();
        assert_eq!(changed.document.pages, source.pages);
        let output = temp.path().join(format!("saved{rotation}.pdf"));
        save(&e, &changed.document, &output);
        let reopened = e.open_document(&output).unwrap();
        let selected = e
            .list_existing_images(&reopened.id, 0)
            .unwrap()
            .images
            .remove(0);
        assert_eq!(selected.bounds, original.bounds);
        assert_eq!(selected.corners, original.corners);
        let again = e
            .preview_image_replacement_occurrence(
                &reopened.id,
                &[0, 1],
                0,
                &selected.object_path,
                &selected.selection_id,
                &imported.image.id,
                ImageFitMode::Fill,
            )
            .unwrap();
        assert_eq!(
            e.render_page(&again.document.id, 0, 800).unwrap(),
            e.render_page(&reopened.id, 0, 800).unwrap()
        );
        for id in [
            &again.document.id,
            &reopened.id,
            &changed.document.id,
            &source.id,
        ] {
            e.close_document(id).unwrap();
        }
    }
    e.images().release(&imported.token).unwrap();
}
#[test]
fn rejects_stale_image_identity_and_invalid_occurrences() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("stale.pdf");
    fixture(&path, 0, 1.);
    let source = e.open_document(&path).unwrap();
    let second = e.open_document(&path).unwrap();
    let imported = asset(&e);
    let selected = e
        .list_existing_images(&source.id, 0)
        .unwrap()
        .images
        .remove(0);
    assert!(e
        .preview_image_replacement_occurrence(
            &second.id,
            &[0, 1],
            0,
            &selected.object_path,
            &selected.selection_id,
            &imported.image.id,
            ImageFitMode::Fit
        )
        .is_err());
    assert!(e
        .preview_image_replacement_occurrence(
            &source.id,
            &[0, 1],
            0,
            &[99],
            &selected.selection_id,
            &imported.image.id,
            ImageFitMode::Fit
        )
        .is_err());
    assert!(e
        .preview_image_replacement_occurrence(
            &source.id,
            &[0, 99],
            0,
            &selected.object_path,
            &selected.selection_id,
            &imported.image.id,
            ImageFitMode::Fit
        )
        .is_err());
    for id in [&second.id, &source.id] {
        e.close_document(id).unwrap();
    }
    e.images().release(&imported.token).unwrap();
}

#[test]
fn preserves_annotation_appearance_that_inherits_the_old_page_image_alias() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("appearance.pdf");
    fixture(&path, 0, 1.);
    let mut pdf = Document::load(&path).unwrap();
    let page = *pdf.get_pages().values().next().unwrap();
    let appearance=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),1.into(),1.into()]},b"/Shared Do".to_vec()));
    let annotation=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Stamp","Rect"=>vec![220.into(),100.into(),320.into(),150.into()],"AP"=>dictionary!{"N"=>appearance}});
    pdf.get_dictionary_mut(page)
        .unwrap()
        .get_mut(b"Annots")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .push(annotation.into());
    pdf.save(&path).unwrap();
    let source = e.open_document(&path).unwrap();
    let imported = asset(&e);
    let selected = e
        .list_existing_images(&source.id, 0)
        .unwrap()
        .images
        .remove(0);
    let changed = e
        .preview_image_replacement_occurrence(
            &source.id,
            &[0, 1],
            0,
            &selected.object_path,
            &selected.selection_id,
            &imported.image.id,
            ImageFitMode::Stretch,
        )
        .unwrap();
    let before = image::load_from_memory(&e.render_page(&source.id, 0, 800).unwrap())
        .unwrap()
        .to_rgba8();
    let after = image::load_from_memory(&e.render_page(&changed.document.id, 0, 800).unwrap())
        .unwrap()
        .to_rgba8();
    for y in 0..before.height() {
        for x in 300..before.width() {
            assert_eq!(
                before.get_pixel(x, y),
                after.get_pixel(x, y),
                "annotation region {x},{y}"
            );
        }
    }
    e.close_document(&changed.document.id).unwrap();
    e.close_document(&source.id).unwrap();
    e.images().release(&imported.token).unwrap();
}

#[path = "support/protected_fixture.rs"]
mod protected_fixture;
#[test]
fn image_edit_preserves_protection_and_rejects_unauthorized_duplicate_pages() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("protected.pdf");
    let imported = asset(&e);
    for writable in [false, true] {
        fixture(&path, 0, 1.);
        let rights = if writable {
            lopdf::encryption::Permissions::MODIFIABLE
        } else {
            lopdf::encryption::Permissions::PRINTABLE
        };
        let original = protected_fixture::encrypt(&fs::read(&path).unwrap(), rights);
        fs::write(&path, &original).unwrap();
        let source = e.open_document_with_password(&path, "reader").unwrap();
        if !writable {
            assert!(e.list_existing_images(&source.id, 0).is_err());
        } else {
            let selected = e
                .list_existing_images(&source.id, 0)
                .unwrap()
                .images
                .remove(0);
            assert!(e
                .preview_image_replacement_occurrence(
                    &source.id,
                    &[0, 0],
                    1,
                    &selected.object_path,
                    &selected.selection_id,
                    &imported.image.id,
                    ImageFitMode::Fit
                )
                .is_err());
            let changed = e
                .preview_image_replacement_occurrence(
                    &source.id,
                    &[0, 1],
                    0,
                    &selected.object_path,
                    &selected.selection_id,
                    &imported.image.id,
                    ImageFitMode::Fit,
                )
                .unwrap();
            assert_eq!(changed.document.protection, source.protection);
            assert!(e.extract_text(&changed.document.id, 0).is_err());
            let output = temp.path().join("Protected-edited.pdf");
            save(&e, &changed.document, &output);
            for password in ["reader", "owner"] {
                assert!(folio_engine::protection::unlock_pdf(
                    &fs::read(&output).unwrap(),
                    Some(password)
                )
                .is_ok());
            }
            e.close_document(&changed.document.id).unwrap();
        }
        assert_eq!(fs::read(&path).unwrap(), original);
        e.close_document(&source.id).unwrap();
    }
    e.images().release(&imported.token).unwrap();
}

#[test]
fn accepts_jpeg_orientation_and_preserves_affine_clipping_and_paint_order() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("affine.pdf");
    fixture(&path, 0, 1.);
    let mut pdf = Document::load(&path).unwrap();
    let page = *pdf.get_pages().values().next().unwrap();
    let content=pdf.add_object(Stream::new(dictionary!{},b"q 20 30 100 50 re W n /Fade gs 90 15 25 50 20 30 cm /Shared Do Q q -100 0 0 50 320 30 cm /Shared Do Q 0 g 15 49 140 3 re f BT /F 12 Tf 20 160 Td (Keep all text and spaces) Tj ET".to_vec()));
    let p = pdf.get_dictionary_mut(page).unwrap();
    p.set("Contents", content);
    p.get_mut(b"Resources").unwrap().as_dict_mut().unwrap().set(
        "ExtGState",
        dictionary! {"Fade"=>dictionary!{"ca"=>Object::Real(0.7),"CA"=>Object::Real(0.7)}},
    );
    pdf.save(&path).unwrap();
    artifact("Affine-original.pdf", &fs::read(&path).unwrap());
    let source = e.open_document(&path).unwrap();
    let selected = e
        .list_existing_images(&source.id, 0)
        .unwrap()
        .images
        .remove(0);
    let before = image::load_from_memory(&e.render_page(&source.id, 0, 800).unwrap())
        .unwrap()
        .to_rgba8();
    for orientation in 1..=8u8 {
        let pixels = image::RgbImage::from_fn(80, 40, |x, y| {
            image::Rgb(match (x < 40, y < 20) {
                (true, true) => [255, 0, 0],
                (false, true) => [0, 255, 0],
                (true, false) => [0, 0, 255],
                _ => [255, 255, 0],
            })
        });
        let mut bytes = vec![];
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 95)
            .encode_image(&pixels)
            .unwrap();
        let mut exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0".to_vec();
        exif.extend_from_slice(&[orientation, 0, 0, 0, 0, 0, 0, 0]);
        let mut tagged = vec![255, 216, 255, 225];
        tagged.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        tagged.extend(exif);
        tagged.extend_from_slice(&bytes[2..]);
        artifact(&format!("orientation-{orientation}.jpg"), &tagged);
        let imported = e.images().register(tagged).unwrap();
        let changed = e
            .preview_image_replacement_occurrence(
                &source.id,
                &[0, 1],
                0,
                &selected.object_path,
                &selected.selection_id,
                &imported.image.id,
                ImageFitMode::Stretch,
            )
            .unwrap();
        let listed = e.list_existing_images(&changed.document.id, 0).unwrap();
        assert_eq!(
            (listed.images[0].pixel_width, listed.images[0].pixel_height),
            (imported.image.width, imported.image.height)
        );
        assert_eq!(listed.images[0].corners, selected.corners);
        let after = image::load_from_memory(&e.render_page(&changed.document.id, 0, 800).unwrap())
            .unwrap()
            .to_rgba8();
        for (x, y, pixel) in before.enumerate_pixels() {
            if x > 245 || x < 38 || y < 238 || y > 342 || y == 300 {
                assert_eq!(
                    pixel,
                    after.get_pixel(x, y),
                    "outside clip/paint orientation {orientation} at {x},{y}"
                );
            }
        }
        let output = temp.path().join(format!("orientation-{orientation}.pdf"));
        save(&e, &changed.document, &output);
        artifact(
            &format!("orientation-{orientation}.pdf"),
            &fs::read(output).unwrap(),
        );
        e.close_document(&changed.document.id).unwrap();
        e.images().release(&imported.token).unwrap();
    }
    e.close_document(&source.id).unwrap();
}

#[test]
fn repeated_saved_replacement_does_not_accumulate_old_image_graphs() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("repeat.pdf");
    fixture(&path, 0, 1.);
    let imported = asset(&e);
    let mut source = e.open_document(&path).unwrap();
    let mut size_bound = 0;
    let mut count_bound = 0;
    for i in 0..7 {
        let selected = e
            .list_existing_images(&source.id, 0)
            .unwrap()
            .images
            .remove(0);
        let changed = e
            .preview_image_replacement_occurrence(
                &source.id,
                &[0, 1],
                0,
                &selected.object_path,
                &selected.selection_id,
                &imported.image.id,
                if i % 2 == 0 {
                    ImageFitMode::Fit
                } else {
                    ImageFitMode::Fill
                },
            )
            .unwrap();
        let output = temp.path().join(format!("round{i}.pdf"));
        save(&e, &changed.document, &output);
        let bytes = fs::read(&output).unwrap();
        let pdf = Document::load_mem(&bytes).unwrap();
        if i == 0 {
            size_bound = bytes.len() + 300;
            count_bound = pdf.objects.len() + 2;
        } else {
            assert!(
                bytes.len() <= size_bound,
                "round{i}: {}>{size_bound}",
                bytes.len()
            );
            assert!(pdf.objects.len() <= count_bound, "round{i}: object growth");
        }
        let reopened = e.open_document(output).unwrap();
        e.close_document(&source.id).unwrap();
        e.close_document(&changed.document.id).unwrap();
        source = reopened;
    }
    e.close_document(&source.id).unwrap();
    e.images().release(&imported.token).unwrap();
}

#[test]
fn unique_direct_image_alias_keeps_its_inheriting_annotation_after_save(){
 let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("unique-appearance.pdf");fixture(&path,0,1.);let mut pdf=Document::load(&path).unwrap();let page=*pdf.get_pages().values().next().unwrap();
 let image=pdf.objects.iter().find_map(|(id,o)|o.as_stream().ok().filter(|s|s.dict.has(b"Width")).map(|_|*id)).unwrap();
 let content=pdf.add_object(Stream::new(dictionary!{},b"q 100 0 0 50 20 30 cm /Only Do Q BT /F 12 Tf 20 160 Td (Keep all text and spaces) Tj ET".to_vec()));
 let appearance=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),1.into(),1.into()]},b"/Only Do".to_vec()));
 let annotation=pdf.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Stamp","Rect"=>vec![220.into(),100.into(),320.into(),150.into()],"AP"=>dictionary!{"N"=>appearance}});
 let p=pdf.get_dictionary_mut(page).unwrap();p.set("Contents",content);p.get_mut(b"Resources").unwrap().as_dict_mut().unwrap().set("XObject",dictionary!{"Only"=>image});p.get_mut(b"Annots").unwrap().as_array_mut().unwrap().push(annotation.into());pdf.save(&path).unwrap();
 let source=e.open_document(&path).unwrap();let imported=asset(&e);let selected=e.list_existing_images(&source.id,0).unwrap().images.remove(0);assert_eq!(selected.object_path.len(),1);
 let changed=e.preview_image_replacement_occurrence(&source.id,&[0,1],0,&selected.object_path,&selected.selection_id,&imported.image.id,ImageFitMode::Stretch).unwrap();let output=temp.path().join("saved.pdf");save(&e,&changed.document,&output);let reopened=e.open_document(output).unwrap();
 let before=image::load_from_memory(&e.render_page(&source.id,0,800).unwrap()).unwrap().to_rgba8();let after=image::load_from_memory(&e.render_page(&reopened.id,0,800).unwrap()).unwrap().to_rgba8();
 for y in 0..before.height(){for x in 300..before.width(){assert_eq!(before.get_pixel(x,y),after.get_pixel(x,y),"inherited unique alias at {x},{y}");}}
 for (i,edit) in [ExistingImageEdit::Transform{transform:ImageTransform{delta_x:5.,delta_y:0.,scale_x:0.7,scale_y:0.7,rotation_degrees:15.}},ExistingImageEdit::Remove].into_iter().enumerate(){
  let changed=e.preview_image_edit_occurrence(&source.id,&[0,1],0,&selected.object_path,&selected.selection_id,edit).unwrap();let file=temp.path().join(format!("edit-{i}.pdf"));save(&e,&changed.document,&file);let reopened=e.open_document(file).unwrap();
  let after=image::load_from_memory(&e.render_page(&reopened.id,0,800).unwrap()).unwrap().to_rgba8();
  for y in 0..before.height(){for x in 300..before.width(){assert_eq!(before.get_pixel(x,y),after.get_pixel(x,y),"position/remove borrowed annotation at {x},{y}");}}
  assert_eq!(e.list_existing_images(&reopened.id,0).unwrap().images.len(),if i==0{1}else{0});for id in [&reopened.id,&changed.document.id]{e.close_document(id).unwrap();}
 }
 for id in [&reopened.id,&changed.document.id,&source.id]{e.close_document(id).unwrap();}e.images().release(&imported.token).unwrap();
}

#[test]
fn replacement_cannot_reuse_an_image_retained_only_by_another_protected_source() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("destination.pdf");
    fixture(&path, 0, 1.);
    let destination = e.open_document(&path).unwrap();
    let owner = e
        .open_document_with_password(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../tests/fixtures/protection/aes_128.pdf"),
            "owner",
        )
        .unwrap();
    let pixels = image::RgbaImage::from_pixel(5, 3, image::Rgba([71, 82, 93, 128]));
    let mut bytes = Cursor::new(vec![]);
    image::DynamicImage::ImageRgba8(pixels)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    let imported = e.images().register(bytes.into_inner()).unwrap();
    let asset_id = imported.image.id.clone();
    let overlay = folio_engine::Overlay::Image(folio_engine::ImageOverlay {
        id: "protected-picture".into(),
        asset_id: asset_id.clone(),
        x: 20.,
        y: 20.,
        width: 80.,
        height: 50.,
        rotation: 0,
    });
    let plan = PagePlan {
        id: "p".into(),
        source_id: owner.id.clone(),
        page_index: 0,
        width: owner.pages[0].width,
        height: owner.pages[0].height,
        rotation: 0,
        crop: None,
        overlays: vec![overlay],
    };
    let protected = temp.path().join("protected-picture.pdf");
    e.export_pdf(
        ExportRequest {
            pages: vec![plan],
            flatten: false,
        },
        &protected,
    )
    .unwrap();
    e.close_document(&owner.id).unwrap();
    e.images().release(&imported.token).unwrap();
    let reader = e.open_document_with_password(&protected, "reader").unwrap();
    let selected = e
        .list_existing_images(&destination.id, 0)
        .unwrap()
        .images
        .remove(0);
    assert!(matches!(
        e.preview_image_replacement_occurrence(
            &destination.id,
            &[0, 1],
            0,
            &selected.object_path,
            &selected.selection_id,
            &asset_id,
            ImageFitMode::Fit
        ),
        Err(folio_engine::EngineError::PermissionDenied(_))
    ));
    e.close_document(&reader.id).unwrap();
    e.close_document(&destination.id).unwrap();
}
