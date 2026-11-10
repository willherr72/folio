//! Transaction ownership for recovery imports, including native annotation fonts.
use super::*;

impl FontImportScope {
    pub(crate) fn new(registry: Arc<FontRegistry>) -> Arc<Self> {
        Arc::new(Self { registry, marker: Arc::new(()), created: Mutex::new(BTreeSet::new()) })
    }

    pub(crate) fn register(&self, bytes: Vec<u8>) -> EngineResult<FontInfo> {
        let asset = Arc::new(FontAsset::parse(bytes)?);
        self.register_assets(std::slice::from_ref(&asset))?;
        Ok(asset.info.clone())
    }

    pub(crate) fn register_assets(&self, assets: &[Arc<FontAsset>]) -> EngineResult<()> {
        self.registry.register_assets_scoped(assets, Some(self))?;
        Ok(())
    }

    /// Retain referenced imports permanently; release only our still-owned extras.
    pub(crate) fn adopt(&self, retained: &BTreeSet<String>) -> EngineResult<()> {
        let mut state = self.registry.lock()?;
        let mut created = self.created.lock().map_err(|_| invalid("Font import ownership is unavailable."))?;
        for id in created.iter() {
            if state.registrations.get(id).is_some_and(|marker| Arc::ptr_eq(marker,&self.marker)) {
                if retained.contains(id) {
                    state.registrations.insert(id.clone(), Arc::new(()));
                } else {
                    state.registrations.remove(id);
                    if let Some(asset) = state.assets.remove(id) { state.bytes -= asset.bytes.len(); }
                }
            }
        }
        created.clear();
        Ok(())
    }
}

impl Drop for FontImportScope {
    fn drop(&mut self) { let _ = self.adopt(&BTreeSet::new()); }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(unique: bool) -> Vec<u8> {
        let mut bytes = std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap();
        if unique { bytes.extend_from_slice(uuid::Uuid::new_v4().as_bytes()); }
        bytes
    }

    #[test]
    fn repeated_own_imports_are_released_and_accounted_once() {
        let registry = Arc::new(FontRegistry::new());
        let scope = FontImportScope::new(registry.clone());
        let bytes = bytes(false);
        let id = scope.register(bytes.clone()).unwrap().id;
        scope.register(bytes).unwrap();
        assert_eq!(registry.ids().unwrap().len(),1);
        drop(scope);
        assert!(registry.get(&id).is_err());
        assert_eq!(registry.lock().unwrap().bytes,0);
    }

    #[test]
    fn external_claim_survives_rollback_and_subsequent_scope_reuse() {
        let registry = Arc::new(FontRegistry::new());
        let scope = FontImportScope::new(registry.clone());
        let bytes = bytes(false);
        let id = scope.register(bytes.clone()).unwrap().id;
        registry.register(bytes.clone()).unwrap();
        scope.register(bytes).unwrap();
        drop(scope);
        assert!(registry.get(&id).is_ok());
    }

    #[test]
    fn borrowed_existing_font_is_not_owned_by_the_scope() {
        let registry = Arc::new(FontRegistry::new());
        let bytes = bytes(false);
        let id = registry.register(bytes.clone()).unwrap().id;
        let scope = FontImportScope::new(registry.clone());
        scope.register(bytes).unwrap();
        drop(scope);
        assert!(registry.get(&id).is_ok());
    }

    #[test]
    fn adoption_retains_only_referenced_owned_imports() {
        let registry = Arc::new(FontRegistry::new());
        let scope = FontImportScope::new(registry.clone());
        let keep = scope.register(bytes(false)).unwrap().id;
        let discard = scope.register(bytes(true)).unwrap().id;
        scope.adopt(&BTreeSet::from([keep.clone()])).unwrap();
        drop(scope);
        assert!(registry.get(&keep).is_ok());
        assert!(registry.get(&discard).is_err());
        let retained_bytes = registry.get(&keep).unwrap().bytes.len();
        assert_eq!(registry.lock().unwrap().bytes,retained_bytes);
    }
}
