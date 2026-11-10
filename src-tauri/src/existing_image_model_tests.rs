use super::*;
use lopdf::{dictionary, Document, Object, Stream};
use std::io::Cursor;

#[test]
fn positions_and_removes_only_the_selected_shared_invocation() {
    for nested in [false, true] {
        let input = bytes(&mut fixture(nested));
        let mut model = Model::read(&input, 0).unwrap();
        let selected = model.images()[0].clone();
        let sibling = model.images()[1].clone();
        let target = [0., -75., 30., 10., 60., 110.];
        let output = model.change(&selected.path, Some(target)).unwrap();
        let mut after = Model::read(&output, 0).unwrap();
        assert_eq!(after.images().len(), 2);
        for (a, b) in after.images()[0].matrix.iter().zip(target) {
            assert!((a - b).abs() < 1e-4);
        }
        assert_eq!(after.images()[1].matrix, sibling.matrix);
        let path = after.images()[0].path.clone();
        let removed = after.change(&path, None).unwrap();
        let removed = Model::read(&removed, 0).unwrap();
        assert_eq!(removed.images().len(), 1);
        assert_eq!(removed.images()[0].matrix, sibling.matrix);
        assert_eq!(
            Model::read(&input, 0).unwrap().images()[0].matrix,
            selected.matrix
        );
    }
}

#[test]
fn repeated_positioning_is_bounded_and_interoperates_with_replacement() {
    let mut current = bytes(&mut fixture(true));
    let prepared = prepared();
    let mut counts = None;
    for i in 0..16 {
        let mut model = Model::read(&current, 0).unwrap();
        let selected = model.images()[0].clone();
        let mut target = selected.matrix;
        target[4] += 1.;
        target[5] -= 1.;
        current = model.change(&selected.path, Some(target)).unwrap();
        let mut model = Model::read(&current, 0).unwrap();
        let selected = model.images()[0].clone();
        assert_eq!(selected.path.len(), 2);
        current = model
            .rewrite(&selected.path, Some((&prepared, ImageFitMode::Fit, 1.)))
            .unwrap();
        let pdf = Document::load_mem(&current).unwrap();
        let model = Model::read(&current, 0).unwrap();
        let measured = (
            pdf.objects.len(),
            model.root.ops.len(),
            model.root.children.values().next().unwrap().ops.len(),
        );
        if i > 0 {
            assert_eq!(Some(measured), counts);
        } else {
            counts = Some(measured);
        }
        assert_eq!(model.images().len(), 2);
    }
}

#[test]
fn position_noop_preserves_raw_matrix_and_invalid_targets_are_rejected() {
    let input = bytes(&mut fixture(true));
    let mut model = Model::read(&input, 0).unwrap();
    let selected = model.images()[0].clone();
    let output = model.change(&selected.path, Some(selected.matrix)).unwrap();
    let after = Model::read(&output, 0).unwrap();
    assert_eq!(after.images()[0].matrix, selected.matrix);
    assert_eq!(after.images()[0].path, selected.path);
    for matrix in [[0.; 6], [f64::NAN; 6], [1e9, 0., 0., 1., 0., 0.]] {
        assert!(model.change(&selected.path, Some(matrix)).is_err());
    }
    assert!(model.change(&[100_000], None).is_err());
}
#[test]
fn selected_reflection_and_intervening_clip_survive_position_coalescing() {
    let mut pdf = fixture(false);
    let page = *pdf.get_pages().values().next().unwrap();
    let content=pdf.add_object(Stream::new(dictionary!{},b"q -100 15 25 50 160 30 cm 0 0 1 1 re W n /Shared Do Q q 100 0 0 50 220 30 cm /Shared Do Q".to_vec()));
    pdf.get_dictionary_mut(page)
        .unwrap()
        .set("Contents", content);
    let mut current = bytes(&mut pdf);
    let mut count = None;
    for _ in 0..10 {
        let mut model = Model::read(&current, 0).unwrap();
        let selected = model.images()[0].clone();
        let sibling = model.images()[1].matrix;
        let target = ImageTransform {
            delta_x: 1.,
            delta_y: 0.,
            scale_x: 0.99,
            scale_y: 1.01,
            rotation_degrees: 2.,
        }
        .matrix(selected.matrix, 0)
        .unwrap();
        assert!(target[0] * target[3] - target[1] * target[2] < 0.);
        current = model.change(&selected.path, Some(target)).unwrap();
        let after = Model::read(&current, 0).unwrap();
        assert_eq!(after.images()[1].matrix, sibling);
        assert_eq!(
            after.root.ops[1].operands,
            vec![
                (-100).into(),
                15.into(),
                25.into(),
                50.into(),
                160.into(),
                30.into()
            ]
        );
        assert_eq!(after.root.ops[2].operator, "re");
        assert_eq!(after.root.ops[3].operator, "W");
        assert_eq!(after.root.ops[4].operator, "n");
        if let Some(n) = count {
            assert_eq!(after.root.ops.len(), n);
        } else {
            count = Some(after.root.ops.len());
        }
    }
}

fn fixture(nested: bool) -> Document {
    let mut pdf = Document::with_version("1.7");
    let pages = pdf.new_object_id();
    let image=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>2,"Height"=>1,"ColorSpace"=>"DeviceRGB","BitsPerComponent"=>8},vec![255,0,0,0,255,0]));
    let leaf = if nested {
        pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),1.into(),1.into()],"Matrix"=>vec![1.into(),0.into(),Object::Real(0.25),1.into(),0.into(),0.into()],"Resources"=>dictionary!{"XObject"=>dictionary!{"I"=>image}}},b"/I Do".to_vec()))
    } else {
        image
    };
    let content = pdf.add_object(Stream::new(
        dictionary! {},
        b"q 100 0 0 50 10 20 cm /Shared Do Q q -80 0 20 60 300 40 cm /Shared Do Q".to_vec(),
    ));
    let page=pdf.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),400.into(),200.into()],"Resources"=>dictionary!{"XObject"=>dictionary!{"Shared"=>leaf}},"Contents"=>content});
    pdf.objects.insert(
        pages,
        dictionary! {"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1}.into(),
    );
    let root = pdf.add_object(dictionary! {"Type"=>"Catalog","Pages"=>pages});
    pdf.trailer.set("Root", root);
    pdf
}
fn bytes(pdf: &mut Document) -> Vec<u8> {
    let mut bytes = vec![];
    pdf.save_to(&mut bytes).unwrap();
    bytes
}
#[test]
fn unit_image_container_recognition_requires_complete_unclipped_image_semantics() {
    for (content,depth,size) in [
        ("/I Do",1,(2,1)),
        ("q /I Do Q",1,(2,1)),
        ("q 1 0 0 1 0 0 cm /I Do Q",1,(2,1)),
        ("0 1 -1 0 1 0 cm /I Do",1,(1,2)),
        ("q -1 0 0 1 1 0 cm /I Do Q",1,(2,1)),
        ("q 1 0 0 1 0 0.1 cm /I Do Q",2,(2,1)),
        ("q 0.5 0 0 1 0 0 cm /I Do Q",2,(2,1)),
        ("q 0 0 1 1 re f /I Do Q",2,(2,1)),
        ("/I Do /I Do",2,(2,1)),
    ] {
        let mut doc=fixture(true);let page=*doc.get_pages().values().next().unwrap();
        let form=doc.get_dictionary(page).unwrap().get(b"Resources").unwrap().as_dict().unwrap().get(b"XObject").unwrap().as_dict().unwrap().get(b"Shared").unwrap().as_reference().unwrap();
        let stream=doc.get_object_mut(form).unwrap().as_stream_mut().unwrap();stream.dict.remove(b"Matrix");stream.set_plain_content(content.as_bytes().to_vec());
        let model=Model::read(&bytes(&mut doc),0).unwrap();
        assert_eq!(model.images()[0].path.len(),depth,"{content}");
        assert_eq!((model.images()[0].width,model.images()[0].height),size,"{content}");
    }
}
fn prepared() -> PreparedPdfImage {
    let image = image::RgbaImage::from_pixel(4, 4, image::Rgba([0, 0, 255, 128]));
    let mut bytes = Cursor::new(vec![]);
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    crate::prepare_pdf_image(&crate::ImageAsset::parse(bytes.into_inner()).unwrap()).unwrap()
}
#[test]
fn discovers_shared_images_and_composes_nested_affine_frames() {
    let mut pdf = fixture(true);
    let model = Model::read(&bytes(&mut pdf), 0).unwrap();
    let images = model.images();
    assert_eq!(images.len(), 2);
    assert_eq!(images[0].path, vec![2, 0]);
    assert_eq!(images[0].matrix, [100., 0., 25., 50., 10., 20.]);
    assert_eq!(
        images[0].corners,
        [[10., 20.], [110., 20.], [135., 70.], [35., 70.]]
    );
    assert_eq!((images[0].width, images[0].height), (2, 1));
    assert_eq!(images[1].matrix, [-80., 0., 0., 60., 300., 40.]);
}
#[test]
fn replaces_one_shared_invocation_without_rebinding_the_sibling() {
    for nested in [false, true] {
        let input = bytes(&mut fixture(nested));
        let mut model = Model::read(&input, 0).unwrap();
        let path = model.images()[0].path.clone();
        let sibling = model.images()[1].clone();
        let output = model
            .rewrite(&path, Some((&prepared(), ImageFitMode::Stretch, 1.)))
            .unwrap();
        let after = Model::read(&output, 0).unwrap();
        assert_eq!(after.images().len(), 2);
        assert_eq!(after.images()[0].width, 4);
        assert_eq!(after.images()[1].width, 2);
        assert_eq!(after.images()[1].matrix, sibling.matrix);
        assert_eq!(Model::read(&input, 0).unwrap().images()[0].width, 2);
    }
}
#[test]
fn repeated_replacement_keeps_frame_depth_and_resource_count_bounded() {
    let mut current = bytes(&mut fixture(true));
    let prepared = prepared();
    let mut first_count = None;
    let mut first_len = 0;
    for i in 0..12 {
        let mut model = Model::read(&current, 0).unwrap();
        let image = model.images()[0].clone();
        assert_eq!(image.path.len(), 2);
        assert_eq!(image.matrix, [100., 0., 25., 50., 10., 20.]);
        current = model
            .rewrite(
                &image.path,
                Some((
                    &prepared,
                    if i % 2 == 0 {
                        ImageFitMode::Fit
                    } else {
                        ImageFitMode::Fill
                    },
                    1.,
                )),
            )
            .unwrap();
        let pdf = Document::load_mem(&current).unwrap();
        if let Some(count) = first_count {
            assert_eq!(pdf.objects.len(), count);
            assert!(current.len() < first_len + 200);
        } else {
            first_count = Some(pdf.objects.len());
            first_len = current.len();
        }
    }
}
#[test]
fn no_op_clone_keeps_image_geometry_and_original_invocations() {
    let input = bytes(&mut fixture(true));
    let mut model = Model::read(&input, 0).unwrap();
    let before = model.images().to_vec();
    let output = model.rewrite(&before[0].path, None).unwrap();
    let after = Model::read(&output, 0).unwrap();
    assert_eq!(after.images().len(), 2);
    for (a, b) in before.iter().zip(after.images()) {
        assert_eq!(a.matrix, b.matrix);
        assert_eq!(a.width, b.width);
    }
}
#[test]
fn rejects_stale_paths_and_invalid_asset_ratios() {
    let input = bytes(&mut fixture(false));
    let mut model = Model::read(&input, 0).unwrap();
    let prepared = prepared();
    assert!(model
        .rewrite(&[100], Some((&prepared, ImageFitMode::Fit, 1.)))
        .is_err());
    for ratio in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(model
            .rewrite(&[2], Some((&prepared, ImageFitMode::Fit, ratio)))
            .is_err());
    }
}
#[test]
fn refuses_inherited_form_resources_and_opaque_consumers() {
    let mut missing = fixture(true);
    for object in missing.objects.values_mut() {
        if let Ok(stream) = object.as_stream_mut() {
            if stream.dict.get(b"Subtype").and_then(Object::as_name).ok()
                == Some(b"Form".as_slice())
            {
                stream.dict.remove(b"Resources");
            }
        }
    }
    assert!(Model::read(&bytes(&mut missing), 0).is_err());
    for category in ["Pattern", "Font", "ExtGState"] {
        let mut pdf = fixture(false);
        let page = *pdf.get_pages().values().next().unwrap();
        let resources = pdf
            .get_dictionary_mut(page)
            .unwrap()
            .get_mut(b"Resources")
            .unwrap()
            .as_dict_mut()
            .unwrap();
        let value = match category {
            "Pattern" => dictionary! {"P"=>dictionary!{"PatternType"=>1}},
            "Font" => dictionary! {"F"=>dictionary!{"Subtype"=>"Type3"}},
            _ => dictionary! {"G"=>dictionary!{"SMask"=>dictionary!{"S"=>"Alpha"}}},
        };
        resources.set(category, value);
        assert!(Model::read(&bytes(&mut pdf), 0).is_err(), "{category}");
    }
}
#[test]
fn rejects_singular_unbalanced_and_oversized_source_images() {
    for content in ["0 0 0 20 0 0 cm /Shared Do", "Q /Shared Do", "q /Shared Do"] {
        let mut pdf = fixture(false);
        let page = *pdf.get_pages().values().next().unwrap();
        let id = pdf.add_object(Stream::new(dictionary! {}, content.as_bytes().to_vec()));
        pdf.get_dictionary_mut(page).unwrap().set("Contents", id);
        assert!(Model::read(&bytes(&mut pdf), 0).is_err(), "{content}");
    }
    let mut pdf = fixture(false);
    for object in pdf.objects.values_mut() {
        if let Ok(stream) = object.as_stream_mut() {
            if stream.dict.get(b"Subtype").and_then(Object::as_name).ok()
                == Some(b"Image".as_slice())
            {
                stream.dict.set("Width", 32769);
            }
        }
    }
    assert!(Model::read(&bytes(&mut pdf), 0).is_err());
}
#[test]
fn fit_fill_stretch_have_centered_predictable_transforms() {
    let matrix = [200., 0., 0., 100., 0., 0.];
    assert_eq!(
        fitting(matrix, 1., ImageFitMode::Fit).unwrap(),
        [0.5, 0., 0., 1., 0.25, 0.]
    );
    assert_eq!(
        fitting(matrix, 1., ImageFitMode::Fill).unwrap(),
        [1., 0., 0., 2., 0., -0.5]
    );
    assert_eq!(
        fitting(matrix, 1., ImageFitMode::Stretch).unwrap(),
        IDENTITY
    );
}
#[test]
fn does_not_trust_modified_frame_markers_or_extra_paint() {
    let mut model = Model::read(&bytes(&mut fixture(false)), 0).unwrap();
    let output = model
        .rewrite(&[2], Some((&prepared(), ImageFitMode::Fit, 1.)))
        .unwrap();
    let mut pdf = Document::load_mem(&output).unwrap();
    for object in pdf.objects.values_mut() {
        if let Ok(stream) = object.as_stream_mut() {
            if stream.content.windows(2).any(|s| s == b"re") {
                stream.dict.set("FolioImageFrame", true);
                let mut data = stream.content.clone();
                data.extend_from_slice(b" 0 0 1 1 re f");
                stream.set_plain_content(data);
            }
        }
    }
    let after = Model::read(&bytes(&mut pdf), 0).unwrap();
    // The inner prepared image is now a recognized whole unit container, but
    // the painted outer form must still remain an independent clipped scope.
    assert_eq!(after.images()[0].path, vec![2, 5]);
    let outer=&after.root.children[&2];
    assert_eq!(outer.ops.last().unwrap().operator,"f");
    let name=after.root.ops[2].operands[0].as_name().unwrap();
    let stream=resolve(&after.pdf,resource(&after.pdf,&after.root.resources,name).unwrap()).unwrap().as_stream().unwrap();
    assert!(frame_dimensions(&after.pdf,stream,outer).unwrap().is_none());
}
#[test]
fn rejects_cyclic_masks_before_the_renderer_sees_them() {
    let mut pdf = fixture(false);
    let id = pdf
        .objects
        .iter()
        .find_map(|(id, o)| {
            o.as_stream()
                .ok()
                .filter(|s| s.dict.has(b"Width"))
                .map(|_| *id)
        })
        .unwrap();
    pdf.get_object_mut(id)
        .unwrap()
        .as_stream_mut()
        .unwrap()
        .dict
        .set("SMask", id);
    assert!(Model::read(&bytes(&mut pdf), 0).is_err());
}
#[test]
fn enforces_occurrence_stack_and_aggregate_pixel_budgets() {
    for content in [
        "/Shared Do ".repeat(5001),
        format!("{} /Shared Do {}", "q ".repeat(257), "Q ".repeat(257)),
        "0 g ".repeat(100001),
    ] {
        let mut pdf = fixture(false);
        let page = *pdf.get_pages().values().next().unwrap();
        let id = pdf.add_object(Stream::new(dictionary! {}, content.into_bytes()));
        pdf.get_dictionary_mut(page).unwrap().set("Contents", id);
        assert!(Model::read(&bytes(&mut pdf), 0).is_err());
    }
    let mut pdf = fixture(false);
    let page = *pdf.get_pages().values().next().unwrap();
    let mut table = lopdf::Dictionary::new();
    for i in 0..3 {
        let id=pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Image","Width"=>6000,"Height"=>5000,"ColorSpace"=>"DeviceRGB","BitsPerComponent"=>8},vec![0]));
        table.set(format!("I{i}"), id);
    }
    let content = pdf.add_object(Stream::new(
        dictionary! {},
        b"/I0 Do /I1 Do /I2 Do".to_vec(),
    ));
    let p = pdf.get_dictionary_mut(page).unwrap();
    p.set("Resources", dictionary! {"XObject"=>table});
    p.set("Contents", content);
    assert!(Model::read(&bytes(&mut pdf), 0).is_err());
}

#[test]
fn crops_and_restores_one_shared_logical_image_without_changing_its_pixels() {
    for nested in [false, true] {
        let input = bytes(&mut fixture(nested));
        let mut model = Model::read(&input, 0).unwrap();
        let selected = model.images()[0].clone();
        let sibling = model.images()[1].clone();
        let proof = model.image_fingerprint(&selected.path).unwrap();
        let crop = ImageCrop {
            left: 0.1,
            top: 0.2,
            right: 0.8,
            bottom: 0.9,
        };
        let output = model.adjust(&selected.path, selected.matrix, crop).unwrap();
        let mut next = Model::read(&output, 0).unwrap();
        assert_eq!(next.images().len(), 2);
        let image = next.images()[0].clone();
        assert_eq!(image.path, selected.path);
        assert_eq!(image.matrix, selected.matrix);
        let actual = image.crop.unwrap();
        assert!((actual.left - 0.1).abs() < 1e-6);
        assert!((actual.bottom - 0.9).abs() < 1e-6);
        assert_eq!(next.images()[1].matrix, sibling.matrix);
        assert!(next.images()[1].crop.is_none());
        assert_eq!(next.image_fingerprint(&image.path).unwrap(), proof);
        let restored = next
            .adjust(&image.path, image.matrix, ImageCrop::FULL)
            .unwrap();
        let restored = Model::read(&restored, 0).unwrap();
        assert_eq!(restored.images()[0].crop, None);
        assert_eq!(restored.image_fingerprint(&image.path).unwrap(), proof);
    }
}

#[test]
fn crop_and_geometry_adjustment_update_the_same_shared_alias_atomically() {
    let input = bytes(&mut fixture(false));
    let mut model = Model::read(&input, 0).unwrap();
    let image = model.images()[0].clone();
    let sibling = model.images()[1].clone();
    let target = [0., -75., 30., 10., 60., 110.];
    let output = model
        .adjust(
            &image.path,
            target,
            ImageCrop {
                left: 0.25,
                top: 0.25,
                right: 0.75,
                bottom: 0.75,
            },
        )
        .unwrap();
    let after = Model::read(&output, 0).unwrap();
    assert_eq!(after.images().len(), 2);
    assert!(after.images()[0].crop.is_some());
    for (a, b) in after.images()[0].matrix.iter().zip(target) {
        assert!((a - b).abs() < 1e-4);
    }
    assert_eq!(after.images()[1].matrix, sibling.matrix);
    assert!(after.images()[1].crop.is_none());
    assert_eq!(
        after.image_fingerprint(&after.images()[0].path).unwrap(),
        model.image_fingerprint(&image.path).unwrap()
    );
}

#[test]
fn repeated_crops_and_replacements_preserve_crop_and_bound_wrapper_depth() {
    let mut current = bytes(&mut fixture(true));
    let mut counts = [None,None];
    let crop = ImageCrop {
        left: 0.125,
        top: 0.25,
        right: 0.875,
        bottom: 0.75,
    };
    for i in 0..16 {
        let mut model = Model::read(&current, 0).unwrap();
        let selected = model.images()[0].clone();
        current = model
            .adjust(
                &selected.path,
                selected.matrix,
                if i % 2 == 0 { crop } else { ImageCrop::FULL },
            )
            .unwrap();
        let mut model = Model::read(&current, 0).unwrap();
        let selected = model.images()[0].clone();
        let wanted = selected.crop;
        current = model
            .rewrite(&selected.path, Some((&prepared(), ImageFitMode::Fit, 1.)))
            .unwrap();
        let model = Model::read(&current, 0).unwrap();
        let pdf = Document::load_mem(&current).unwrap();
        assert_eq!(model.images()[0].path.len(), 2);
        assert_eq!(model.images()[0].crop, wanted);
        assert_eq!(model.images().len(), 2);
        if let Some(count) = counts[i%2] {
            assert_eq!(pdf.objects.len(), count);
        } else {
            counts[i%2] = Some(pdf.objects.len());
        }
    }
}

#[test]
fn crop_validation_and_minimum_serialization_are_stable() {
    for crop in [
        ImageCrop {
            left: f64::NAN,
            ..ImageCrop::FULL
        },
        ImageCrop {
            right: f64::INFINITY,
            ..ImageCrop::FULL
        },
        ImageCrop {
            left: -0.1,
            ..ImageCrop::FULL
        },
        ImageCrop {
            right: 0.0009,
            ..ImageCrop::FULL
        },
        ImageCrop {
            top: 0.9,
            bottom: 0.1,
            ..ImageCrop::FULL
        },
    ] {
        let mut model = Model::read(&bytes(&mut fixture(false)), 0).unwrap();
        let image = model.images()[0].clone();
        assert!(model.adjust(&image.path, image.matrix, crop).is_err());
    }
    let mut current = bytes(&mut fixture(false));
    let mut prior = None;
    for _ in 0..16 {
        let mut model = Model::read(&current, 0).unwrap();
        let image = model.images()[0].clone();
        let requested = image.crop.unwrap_or(ImageCrop {
            left: 0.999,
            top: 0.999,
            right: 1.,
            bottom: 1.,
        });
        current = model.adjust(&image.path, image.matrix, requested).unwrap();
        let model = Model::read(&current, 0).unwrap();
        let actual = model.images()[0].crop.unwrap();
        actual.validate().unwrap();
        assert!((actual.right - actual.left - 0.001).abs() < 1e-7);
        if let Some(previous) = prior {
            assert_eq!(actual, previous);
        }
        prior = Some(actual);
    }
}

#[test]
fn crop_retains_named_and_default_colors_and_proves_image_graph_after_renumbering() {
    let mut pdf = fixture(false);
    let page = *pdf.get_pages().values().next().unwrap();
    let image = resource(&pdf, pdf.get_dictionary(page).unwrap().get(b"Resources").unwrap().as_dict().unwrap(), b"Shared").unwrap().as_reference().unwrap();
    pdf.get_object_mut(image).unwrap().as_stream_mut().unwrap().dict.set("ColorSpace", Object::Name(b"LocalRGB".to_vec()));
    let color = pdf.add_object(Object::Array(vec![Object::Name(b"CalRGB".to_vec()), dictionary!{"WhitePoint"=>vec![1.into(),1.into(),1.into()]}.into()]));
    pdf.get_dictionary_mut(page).unwrap().get_mut(b"Resources").unwrap().as_dict_mut().unwrap().set("ColorSpace", dictionary!{"LocalRGB"=>color,"DefaultRGB"=>color});
    let mut model = Model::read(&bytes(&mut pdf), 0).unwrap();
    let selected = model.images()[0].clone();
    let proof = model.image_fingerprint(&selected.path).unwrap();
    let output = model.adjust(&selected.path, selected.matrix, ImageCrop {left:0.25,right:0.75,..ImageCrop::FULL}).unwrap();
    let mut saved = Document::load_mem(&output).unwrap();
    saved.renumber_objects_with(1000);
    let next = Model::read(&bytes(&mut saved), 0).unwrap();
    assert_eq!(next.images()[0].crop.unwrap().left, 0.25);
    assert_eq!(next.image_fingerprint(&next.images()[0].path).unwrap(), proof);
    let image_id = saved.objects.iter().find(|(_,o)| o.as_stream().is_ok_and(|s| s.dict.get(b"Subtype").is_ok_and(|v| v.as_name().is_ok_and(|n| n==b"Image")))).map(|(id,_)| *id).unwrap();
    saved.get_object_mut(image_id).unwrap().as_stream_mut().unwrap().content[0] ^= 255;
    let changed = Model::read(&bytes(&mut saved), 0).unwrap();
    assert_ne!(changed.image_fingerprint(&changed.images()[0].path).unwrap(), proof);
    saved.get_object_mut(image_id).unwrap().as_stream_mut().unwrap().content[0] ^= 255;
    let color_id = saved.objects.iter().find(|(_,o)| o.as_array().is_ok_and(|a| a.first().is_some_and(|v| v.as_name().is_ok_and(|n| n==b"CalRGB")))).map(|(id,_)| *id).unwrap();
    saved.objects.insert(color_id, Object::Name(b"DeviceRGB".to_vec()));
    let changed = Model::read(&bytes(&mut saved), 0).unwrap();
    assert_ne!(changed.image_fingerprint(&changed.images()[0].path).unwrap(), proof);
}

#[test]
fn crop_recognition_does_not_promote_extra_paint_or_nested_clips() {
    for (content, nested) in [
        ("q 0.999 0 0.001 1 re W n /Picture Do Q", false),
        ("q 0 0.999 1 0.001 re W n /Picture Do Q", false),
        ("q 0.25 0 0.5 1 re W n /Picture Do Q 0 0 1 1 re f", false),
        ("q -0.2 0 0.5 1 re W n /Picture Do Q", false),
        ("q 0.25 0 0.5 1 re W n /Picture Do Q", true),
    ] {
        let mut pdf = fixture(false);
        let page = *pdf.get_pages().values().next().unwrap();
        let scope = pdf.get_dictionary(page).unwrap().get(b"Resources").unwrap().as_dict().unwrap().clone();
        let mut child = resource(&pdf, &scope, b"Shared").unwrap().clone();
        if nested {
            child = crop::new_frame(&mut pdf, &scope, &child, ImageCrop {left:0.125,right:0.875,..ImageCrop::FULL}, &mut crate::form_graph::CopyBudget::default()).unwrap().into();
        }
        let wrapper = pdf.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),1.into(),1.into()],"Resources"=>dictionary!{"XObject"=>dictionary!{"Picture"=>child}}}, content.as_bytes().to_vec()));
        pdf.get_dictionary_mut(page).unwrap().get_mut(b"Resources").unwrap().as_dict_mut().unwrap().get_mut(b"XObject").unwrap().as_dict_mut().unwrap().set("Shared", wrapper);
        let mut model = Model::read(&bytes(&mut pdf), 0).unwrap();
        let image = model.images()[0].clone();
        assert_eq!(image.path, vec![2,4], "outer wrapper stays fixed: {content}");
        assert_eq!(image.crop.is_some(), nested);
        if nested {
            let result = model.adjust(&image.path, image.matrix, ImageCrop::FULL).unwrap();
            let next = Model::read(&result, 0).unwrap();
            assert_eq!(next.images()[0].path, vec![2,4]);
            assert_eq!(next.root.children[&2].ops[1].operands, model.root.children[&2].ops[1].operands);
            let removed = Model::read(&result,0).unwrap().change(&image.path,None).unwrap();
            assert_eq!(Model::read(&removed,0).unwrap().images().len(),1);
        }
    }
}

#[test]
fn full_restore_removes_redundant_clip_when_color_context_is_identical() {
    let input=bytes(&mut fixture(false));let mut model=Model::read(&input,0).unwrap();let image=model.images()[0].clone();
    let output=model.adjust(&image.path,image.matrix,ImageCrop{left:0.2,..ImageCrop::FULL}).unwrap();
    let mut model=Model::read(&output,0).unwrap();let selected=model.images()[0].clone();
    let restored=model.adjust(&selected.path,selected.matrix,ImageCrop::FULL).unwrap();
    let after=Model::read(&restored,0).unwrap();
    assert_eq!(after.images()[0].crop,None,"avoid redundant full clips changing edge antialiasing in independent readers");
    assert_eq!(after.image_fingerprint(&after.images()[0].path).unwrap(),model.image_fingerprint(&selected.path).unwrap());
}
