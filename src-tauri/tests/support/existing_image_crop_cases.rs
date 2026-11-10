use super::*;
use folio_engine::ImageCrop;

#[test]
fn crop_saved_physical_occurrences_restore_pixels_and_preserve_siblings() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    for rotation in [0, 90, 180, 270] {
        let path = temp.path().join(format!("crop-{rotation}.pdf"));
        fixture(&path, rotation, 1.7);
        let input = fs::read(&path).unwrap();
        let source = e.open_document(&path).unwrap();
        let before = e.list_existing_images(&source.id, 0).unwrap().images;
        let selected = &before[0];
        assert!(selected.crop.is_none());
        let crop = ImageCrop {left:0.125, top:0.25, right:0.875, bottom:0.75};
        let edited = e.preview_image_edit_occurrence(&source.id,&[0,1,0],2,&selected.object_path,&selected.selection_id,ExistingImageEdit::Adjust{transform:unchanged_transform(),crop}).unwrap();
        assert_eq!(edited.page_indices, vec![0,1,2]);
        for i in [0,1] { assert_eq!(e.render_page(&source.id,i,800).unwrap(),e.render_page(&edited.document.id,i,800).unwrap()); }
        let after = e.list_existing_images(&edited.document.id,2).unwrap().images;
        assert_eq!(after[0].corners, selected.corners);
        assert_eq!(after[0].crop, Some(crop));
        assert_eq!(after[1].corners, before[1].corners);
        assert!(after[1].crop.is_none());
        assert!(after[0].bounds.width < selected.bounds.width);
        assert!(after[0].bounds.height < selected.bounds.height);
        assert_ne!(e.render_page(&source.id,0,800).unwrap(), e.render_page(&edited.document.id,2,800).unwrap());
        let output = temp.path().join(format!("cropped-{rotation}.pdf"));
        save(&e,&edited.document,&output);
        artifact(&format!("Crop-original-{rotation}.pdf"),&input);
        artifact(&format!("Crop-edited-{rotation}.pdf"),&fs::read(&output).unwrap());
        let reopened = e.open_document(&output).unwrap();
        let image = e.list_existing_images(&reopened.id,2).unwrap().images.remove(0);
        assert_eq!(image.crop,Some(crop));
        let noop = e.preview_image_edit_occurrence(&reopened.id,&[0,1,2],2,&image.object_path,&image.selection_id,ExistingImageEdit::Adjust{transform:unchanged_transform(),crop}).unwrap();
        assert_eq!(e.render_page(&reopened.id,2,800).unwrap(),e.render_page(&noop.document.id,2,800).unwrap());
        let restored = e.preview_image_edit_occurrence(&reopened.id,&[0,1,2],2,&image.object_path,&image.selection_id,ExistingImageEdit::Adjust{transform:unchanged_transform(),crop:ImageCrop::FULL}).unwrap();
        assert_eq!(e.render_page(&source.id,0,800).unwrap(),e.render_page(&restored.document.id,2,800).unwrap());
        let restore_path = temp.path().join(format!("restored-{rotation}.pdf"));
        save(&e,&restored.document,&restore_path);
        artifact(&format!("Crop-restored-{rotation}.pdf"),&fs::read(&restore_path).unwrap());
        assert_eq!(fs::read(&path).unwrap(),input);
        for id in [&source.id,&edited.document.id,&reopened.id,&noop.document.id,&restored.document.id] { e.close_document(id).unwrap(); }
    }
}

#[test]
fn crop_and_transform_are_atomic_and_invalid_crops_or_stale_ids_are_rejected() {
    let e = engine();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("atomic.pdf");
    fixture(&path,90,1.7);
    let source = e.open_document(&path).unwrap();
    let before = e.list_existing_images(&source.id,0).unwrap().images;
    let selected = &before[0];
    let crop = ImageCrop {left:0.25,top:0.1,right:0.9,bottom:0.8};
    for invalid in [ImageCrop{left:f64::NAN,..crop},ImageCrop{right:1.01,..crop},ImageCrop{bottom:0.1001,..crop}] {
        assert!(e.preview_image_edit_occurrence(&source.id,&[0,1],0,&selected.object_path,&selected.selection_id,ExistingImageEdit::Adjust{transform:unchanged_transform(),crop:invalid}).is_err());
    }
    assert!(e.preview_image_edit_occurrence(&source.id,&[0,1],0,&selected.object_path,&"0".repeat(64),ExistingImageEdit::Adjust{transform:unchanged_transform(),crop}).is_err());
    let changed = e.preview_image_edit_occurrence(&source.id,&[0,1],0,&selected.object_path,&selected.selection_id,ExistingImageEdit::Adjust{transform:ImageTransform{delta_x:17.,delta_y:-8.5,scale_x:0.7,scale_y:0.8,rotation_degrees:30.},crop}).unwrap();
    let after = e.list_existing_images(&changed.document.id,0).unwrap().images;
    assert!((after[0].crop.unwrap().left-0.25).abs()<1e-6);
    assert_eq!(after[1].corners,before[1].corners);
    for (axis,delta) in [(true,17.),(false,-8.5)] {
        let center=|v:&folio_engine::ExistingImage| if axis {(v.corners[0].x+v.corners[2].x) as f64/2.}else{(v.corners[0].y+v.corners[2].y) as f64/2.};
        assert!((center(&after[0])-center(selected)-delta).abs()<0.002);
    }
    assert_eq!(e.render_page(&source.id,1,800).unwrap(),e.render_page(&changed.document.id,1,800).unwrap());
    e.close_document(&changed.document.id).unwrap();
    e.close_document(&source.id).unwrap();
}

#[test]
fn crop_alpha_fitted_reflected_image_survives_disjoint_minimum_restore_replace_and_remove() {
    let e=engine();let temp=tempfile::tempdir().unwrap();let path=temp.path().join("affine.pdf");fixture(&path,0,1.);
    let mut pdf=Document::load(&path).unwrap();let page=*pdf.get_pages().values().next().unwrap();
    let content=pdf.add_object(Stream::new(dictionary!{},b"q 0 0 1 rg 50 20 150 85 re f Q q -100 15 25 50 160 30 cm /Shared Do Q q 100 0 0 50 220 30 cm /Shared Do Q q 0 0 0 rg 120 20 5 100 re f Q BT /F 12 Tf 20 160 Td (Keep all text and spaces) Tj ET".to_vec()));
    pdf.get_dictionary_mut(page).unwrap().set("Contents",content);pdf.save(&path).unwrap();
    let source=e.open_document(&path).unwrap();let image=e.list_existing_images(&source.id,0).unwrap().images.remove(0);let asset=asset(&e);
    let fitted=e.preview_image_replacement_occurrence(&source.id,&[0,1],0,&image.object_path,&image.selection_id,&asset.image.id,ImageFitMode::Fit).unwrap();
    let baseline=e.render_page(&fitted.document.id,0,800).unwrap();let mut current=fitted.document.clone();
    for (i,crop) in [ImageCrop{right:0.2,..ImageCrop::FULL},ImageCrop{left:0.8,..ImageCrop::FULL},ImageCrop{left:0.999,top:0.999,..ImageCrop::FULL},ImageCrop::FULL].into_iter().enumerate() {
        let selected=e.list_existing_images(&current.id,0).unwrap().images.remove(0);
        let edited=e.preview_image_edit_occurrence(&current.id,&[0,1],0,&selected.object_path,&selected.selection_id,ExistingImageEdit::Adjust{transform:unchanged_transform(),crop}).unwrap();
        let output=temp.path().join(format!("crop-{i}.pdf"));save(&e,&edited.document,&output);
        artifact(&format!("Crop-affine-alpha-{i}.pdf"),&fs::read(&output).unwrap());
        let reopened=e.open_document(&output).unwrap();
        let selected=e.list_existing_images(&reopened.id,0).unwrap().images.remove(0);
        let noop=e.preview_image_edit_occurrence(&reopened.id,&[0,1],0,&selected.object_path,&selected.selection_id,ExistingImageEdit::Adjust{transform:unchanged_transform(),crop:selected.crop.unwrap_or(ImageCrop::FULL)}).unwrap();
        assert_eq!(e.render_page(&reopened.id,0,800).unwrap(),e.render_page(&noop.document.id,0,800).unwrap());
        for id in [&edited.document.id,&noop.document.id] {e.close_document(id).unwrap();}
        if current.id!=fitted.document.id {e.close_document(&current.id).unwrap();}current=reopened;
    }
    assert_eq!(e.render_page(&current.id,0,800).unwrap(),baseline);
    let selected=e.list_existing_images(&current.id,0).unwrap().images.remove(0);
    let replaced=e.preview_image_replacement_occurrence(&current.id,&[0,1],0,&selected.object_path,&selected.selection_id,&asset.image.id,ImageFitMode::Fill).unwrap();
    let selected=e.list_existing_images(&replaced.document.id,0).unwrap().images.remove(0);assert_eq!(selected.crop,None);
    let removed=e.preview_image_edit_occurrence(&replaced.document.id,&[0,1],0,&selected.object_path,&selected.selection_id,ExistingImageEdit::Remove).unwrap();
    assert_eq!(e.list_existing_images(&removed.document.id,0).unwrap().images.len(),1);
    assert_eq!(e.render_page(&source.id,1,800).unwrap(),e.render_page(&removed.document.id,1,800).unwrap());
    for id in [&source.id,&fitted.document.id,&current.id,&replaced.document.id,&removed.document.id] {e.close_document(id).unwrap();}
}
