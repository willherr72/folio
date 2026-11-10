use super::*;
use image::{ImageFormat, Rgba, RgbaImage};

fn asset(color: u8) -> Arc<ImageAsset> {
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([color, 20, 30, 128])))
        .write_to(&mut bytes, ImageFormat::Png)
        .unwrap();
    Arc::new(ImageAsset::parse(bytes.into_inner()).unwrap())
}
#[test]
fn prepared_images_reuse_bounded_resources_without_retaining_released_assets() {
    let a=asset(101);let b=asset(102);let c=asset(103);
    let size=|asset:&ImageAsset|crate::image_persistence::resource_bytes(crate::prepare_pdf_image(asset).unwrap().form());
    let mut registry=ImageRegistry::new();registry.prepared_limit=size(&a)+size(&b);
    let owners=registry.register_assets(&[a.clone(),b.clone(),c.clone()]).unwrap();
    let first=registry.prepared_pdf(&a.info.id).unwrap();let second=registry.prepared_pdf(&b.info.id).unwrap();
    assert!(Arc::ptr_eq(&first,&registry.prepared_pdf(&a.info.id).unwrap()));
    registry.prepared_pdf(&c.info.id).unwrap();
    let state=registry.lock().unwrap();assert!(state.prepared.contains_key(&a.info.id));assert!(!state.prepared.contains_key(&b.info.id));assert!(state.prepared_bytes<=registry.prepared_limit);drop(state);
    assert!(!Arc::ptr_eq(&second,&registry.prepared_pdf(&b.info.id).unwrap()));
    let borrow=Arc::downgrade(&second);registry.release_all(&owners).unwrap();
    assert!(registry.ids().unwrap().is_empty());assert_eq!(registry.lock().unwrap().prepared_bytes,0);
    assert!(registry.prepared_pdf(&b.info.id).is_err());assert!(borrow.upgrade().is_some());drop(second);assert!(borrow.upgrade().is_none());
}
#[test]
fn oversized_prepared_images_are_usable_without_entering_the_cache() {
    let mut registry=ImageRegistry::new();registry.prepared_limit=0;
    let image=asset(104);let owners=registry.register_assets(&[image.clone()]).unwrap();
    let first=registry.prepared_pdf(&image.info.id).unwrap();let second=registry.prepared_pdf(&image.info.id).unwrap();
    assert!(!Arc::ptr_eq(&first,&second));assert_eq!(registry.lock().unwrap().prepared_bytes,0);
    registry.release_all(&owners).unwrap();
}
fn registry(limits: RegistryLimits) -> ImageRegistry {
    ImageRegistry {
        state: Mutex::new(RegistryState::default()),
        limits,
        prepared_limit:128*1024*1024,
    }
}

#[test]
fn count_encoded_decoded_and_lease_boundaries_are_atomic_and_reclaimable() {
    let a = asset(1);
    let b = asset(2);
    let generous = RegistryLimits {
        assets: 10,
        encoded: 100_000,
        decoded: 100_000,
        leases: 10,
    };
    for limits in [
        RegistryLimits {
            assets: 1,
            ..generous
        },
        RegistryLimits {
            encoded: a.bytes.len() + b.bytes.len() - 1,
            ..generous
        },
        RegistryLimits {
            decoded: a.decoded_bytes + b.decoded_bytes - 1,
            ..generous
        },
        RegistryLimits {
            leases: 1,
            ..generous
        },
    ] {
        let r = registry(limits);
        let first = r.register_assets(std::slice::from_ref(&a)).unwrap();
        assert!(r.register_assets(std::slice::from_ref(&b)).is_err());
        assert_eq!(r.ids().unwrap(), BTreeSet::from([a.info.id.clone()]));
        let state = r.lock().unwrap();
        assert_eq!(state.encoded, a.bytes.len());
        assert_eq!(state.decoded, a.decoded_bytes);
        assert_eq!(state.leases.len(), 1);
        drop(state);
        r.release_all(&first).unwrap();
        assert!(r.ids().unwrap().is_empty());
        let second = r.register_assets(&[b.clone()]).unwrap();
        r.release_all(&second).unwrap();
    }
}

#[test]
fn exact_budgets_and_duplicate_asset_accounting_are_supported() {
    let a = asset(10);
    let b = asset(20);
    let r = registry(RegistryLimits {
        assets: 2,
        encoded: a.bytes.len() + b.bytes.len(),
        decoded: a.decoded_bytes + b.decoded_bytes,
        leases: 3,
    });
    let registered = r
        .register_assets(&[a.clone(), a.clone(), b.clone()])
        .unwrap();
    let state = r.lock().unwrap();
    assert_eq!(state.assets.len(), 2);
    assert_eq!(state.encoded, a.bytes.len() + b.bytes.len());
    assert_eq!(state.decoded, 32);
    drop(state);
    assert!(r.acquire(&a.info.id).is_err());
    r.release(&registered[0].token).unwrap();
    let extra = r.acquire(&a.info.id).unwrap();
    r.release_all(&registered).unwrap();
    assert!(r.get(&a.info.id).is_ok());
    r.release(&extra.token).unwrap();
    let state = r.lock().unwrap();
    assert_eq!(
        (state.encoded, state.decoded, state.leases.len()),
        (0, 0, 0)
    );
}

#[test]
fn failed_batch_does_not_refresh_or_create_any_owner() {
    let a = asset(1);
    let b = asset(2);
    let c = asset(3);
    let r = registry(RegistryLimits {
        assets: 2,
        ..REGISTRY_LIMITS
    });
    let first = r.register_assets(std::slice::from_ref(&a)).unwrap();
    assert!(r.register_assets(&[a, b, c]).is_err());
    r.release_all(&first).unwrap();
    assert!(r.ids().unwrap().is_empty());
    assert!(r.lock().unwrap().leases.is_empty());
    assert!(admitted(usize::MAX, 1, usize::MAX, "bytes").is_err());
}
