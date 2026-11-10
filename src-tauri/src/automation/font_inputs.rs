//! Explicit immutable automation inputs; possession of a registry ID is not authority.
use super::{invalid, io, text_preview::valid_hash};
use crate::{
    fonts::{FontAsset, FontRegistration, FontRegistry},
    EngineResult,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs::File, io::Read, path::PathBuf, sync::Arc};

pub(crate) const MAX_FILES: usize = 16;
const MAX_FILE_BYTES: usize = 16 * 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct FontChoice {
    pub font_id: String,
    pub name: String,
    pub weight: u16,
    pub italic: bool,
    pub bytes: usize,
}
impl FontChoice {
    pub fn from_asset(asset: &FontAsset) -> EngineResult<Self> {
        let choice = Self {
            font_id: asset.info.id.clone(),
            name: asset.info.name.clone(),
            weight: asset.info.weight,
            italic: asset.info.italic,
            bytes: asset.bytes.len(),
        };
        choice.validate()?;
        Ok(choice)
    }
    pub fn validate(&self) -> EngineResult<()> {
        if !valid_hash(&self.font_id)
            || self.name.is_empty()
            || self.name.len() > 512
            || !(1..=1000).contains(&self.weight)
            || !(1..=MAX_FILE_BYTES).contains(&self.bytes)
        {
            return Err(invalid("Invalid or oversized automation font metadata."));
        }
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct FontInputs {
    assets: BTreeMap<String, Arc<FontAsset>>,
}
impl FontInputs {
    pub fn load(paths: &[PathBuf]) -> EngineResult<Self> {
        Self::load_bounded(paths, MAX_FILE_BYTES, MAX_TOTAL_BYTES)
    }
    fn load_bounded(paths: &[PathBuf], per_file: usize, total: usize) -> EngineResult<Self> {
        if paths.len() > MAX_FILES {
            return Err(invalid("Supply at most16 font files."));
        }
        let mut result = Self::default();
        let mut remaining = total;
        for path in paths {
            let file = File::open(path).map_err(io)?;
            let metadata = file.metadata().map_err(io)?;
            let limit = remaining.min(per_file);
            if !metadata.is_file() || metadata.len() == 0 || metadata.len() > limit as u64 {
                return Err(invalid(
                    "Fonts must be nonempty regular files, at most16 MiB each and64 MiB total.",
                ));
            }
            let mut bytes = Vec::with_capacity(metadata.len() as usize);
            file.take(limit as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(io)?;
            if bytes.is_empty() || bytes.len() > limit {
                return Err(invalid("Font inputs exceeded their byte limit."));
            }
            remaining -= bytes.len(); // Charge duplicates too, before parsing/deduplication.
            let asset = Arc::new(FontAsset::parse(bytes)?);
            FontChoice::from_asset(&asset)?;
            result.assets.entry(asset.info.id.clone()).or_insert(asset);
        }
        Ok(result)
    }
    pub fn catalog(&self) -> Value {
        json!({"fonts":self.assets.values().map(|asset|FontChoice::from_asset(asset).expect("validated immutable asset")).collect::<Vec<_>>()})
    }
    pub fn get(&self, id: &str) -> EngineResult<Arc<FontAsset>> {
        self.assets.get(id).cloned().ok_or_else(||invalid("The font is not authorized for this request. Supply its file with --font and use its catalog fontId."))
    }
    pub fn install(&self, registry: &Arc<FontRegistry>) -> EngineResult<FontInstall> {
        let assets = self.assets.values().cloned().collect::<Vec<_>>();
        Ok(FontInstall {
            registry: registry.clone(),
            registrations: registry.register_assets(&assets)?,
        })
    }
}
pub(crate) struct FontInstall {
    registry: Arc<FontRegistry>,
    registrations: Vec<FontRegistration>,
}
impl Drop for FontInstall {
    fn drop(&mut self) {
        let _ = self.registry.rollback_imports(&self.registrations);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    fn font() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")
    }
    #[test]
    fn inputs_charge_duplicates_and_refuse_limits_before_parsing() {
        let path = font();
        let size = fs::metadata(&path).unwrap().len() as usize;
        assert!(FontInputs::load_bounded(&[path.clone()], size - 1, size).is_err());
        assert!(
            FontInputs::load_bounded(&[path.clone(), path.clone()], size, size * 2 - 1).is_err()
        );
        let inputs =
            FontInputs::load_bounded(&[path.clone(), path.clone()], size, size * 2).unwrap();
        assert_eq!(inputs.assets.len(), 1);
        assert!(FontInputs::load(&vec![path; 17]).is_err());
        let d = tempfile::tempdir().unwrap();
        let bad = d.path().join("bad");
        fs::write(&bad, b"").unwrap();
        assert!(FontInputs::load(&[bad.clone()]).is_err());
        fs::write(&bad, b"bad font").unwrap();
        assert!(FontInputs::load(&[bad]).is_err());
        assert!(FontInputs::load(&[d.path().to_owned()]).is_err());
    }
    #[test]
    fn immutable_assets_and_registration_ownership_survive_external_changes() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("font.ttf");
        fs::copy(font(), &p).unwrap();
        let inputs = FontInputs::load(&[p.clone()]).unwrap();
        let catalog = inputs.catalog();
        let id = catalog["fonts"][0]["fontId"].as_str().unwrap();
        let bytes = inputs.get(id).unwrap().bytes.clone();
        fs::write(&p, b"changed").unwrap();
        fs::remove_file(&p).unwrap();
        assert_eq!(inputs.get(id).unwrap().bytes, bytes);
        let registry = Arc::new(FontRegistry::new());
        {
            let _scope = inputs.install(&registry).unwrap();
            assert!(registry.get(id).is_ok());
        }
        assert!(registry.get(id).is_err());
        registry.register(bytes.to_vec()).unwrap();
        {
            let _scope = inputs.install(&registry).unwrap();
        }
        assert!(
            registry.get(id).is_ok(),
            "Preexisting registration is borrowed"
        );
        registry.remove(id).unwrap();
        {
            let _scope = inputs.install(&registry).unwrap();
            registry.register(bytes.to_vec()).unwrap();
        }
        assert!(
            registry.get(id).is_ok(),
            "A later external claim owns its registration"
        );
        assert!(
            FontInputs::default().get(id).is_err(),
            "A globally loaded ID does not authorize use"
        );
    }
}
