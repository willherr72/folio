//! Original encoded images in current-user protected recovery payloads.
use super::*;
use crate::{ImageAsset,image_pdf::prepare_pdf_image};
use std::sync::Arc;

pub(super) const MAX_IMAGE_BYTES:u64=64*1024*1024;
#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ImageSnapshot {pub file:String,pub length:u64,pub checksum:String}

pub(super) fn image_references(workspace:&Value)->Result<BTreeMap<String,BTreeSet<String>>,String>{
    let mut by_source:BTreeMap<String,BTreeSet<String>>=BTreeMap::new();
    for tab in workspace["tabs"].as_array().ok_or("Invalid recovery tabs")? {
        for page in tab["document"]["pages"].as_array().ok_or("Invalid recovery pages")? {
            let ids=by_source.entry(page["sourceId"].as_str().ok_or("Invalid recovery source")?.to_owned()).or_default();
            for overlay in page["overlays"].as_array().ok_or("Invalid recovery overlays")? {
                if overlay["type"]=="image" {
                    ids.insert(overlay["assetId"].as_str().filter(|id|valid_font_id(id)).ok_or("Invalid recovery image reference")?.to_owned());
                }
            }
        }
    }
    if by_source.values().flatten().collect::<BTreeSet<_>>().len()>64 {return Err("Recovery workspace has too many images".into());}
    Ok(by_source)
}
pub(super) fn image_reference_ids(workspace:&Value)->Result<BTreeSet<String>,String>{Ok(image_references(workspace)?.into_values().flatten().collect())}

impl RecoveryStore {
    pub(super) fn image_directory(&self)->Result<PathBuf,String>{
        let path=self.root.join("images");
        if fs::canonicalize(&path).map_err(io_error)?!=path{return Err("Recovery image folder is redirected".into());}
        Ok(path)
    }
    pub(super) fn image_path(&self,id:&str,snapshot:&ImageSnapshot)->Result<PathBuf,String>{
        if !valid_font_id(id)||snapshot.file!=format!("{id}.image")||snapshot.checksum!=id||snapshot.length==0||snapshot.length>MAX_IMAGE_BYTES {
            return Err("Recovery image metadata is invalid".into());
        }
        let path=self.image_directory()?.join(&snapshot.file);
        let metadata=fs::symlink_metadata(&path).map_err(io_error)?;
        if !metadata.is_file()||private_length(&path,Purpose::Image,MAX_IMAGE_BYTES)?!=snapshot.length||fs::canonicalize(&path).map_err(io_error)?!=path {
            return Err("Recovery image is incomplete or redirected".into());
        }
        Ok(path)
    }
    pub(super) fn save_images(&self,engine:&PdfEngine,workspace:&Value,sources:&BTreeMap<String,SourceSnapshot>)->Result<BTreeMap<String,ImageSnapshot>,String>{
        // A forged recovery plan must not launder a protected image through an
        // unrelated unprotected source. Consult native source policy, never JSON.
        for(source,ids)in image_references(workspace)? {
            let policy=sources.get(&source).and_then(|snapshot|snapshot.policy.as_ref()).ok_or("Missing native recovery source policy")?;
            for id in ids {engine.images().authorize(&id,policy.image_origin()).map_err(|e|e.to_string())?;}
        }
        let mut snapshots=BTreeMap::new();
        for id in image_reference_ids(workspace)? {
            let asset=engine.images().get(&id).map_err(|e|format!("Cannot save recovery image: {e}"))?;
            let snapshot=ImageSnapshot{file:format!("{id}.image"),length:asset.bytes().len() as u64,checksum:id.clone()};
            if self.image_path(&id,&snapshot).is_err(){private_write(&self.image_directory()?.join(&snapshot.file),Purpose::Image,asset.bytes())?;}
            snapshots.insert(id,snapshot);
        }
        Ok(snapshots)
    }
    pub(super) fn validate_images(&self,manifest:&Manifest)->Result<(),String>{
        if image_reference_ids(&manifest.workspace)?!=manifest.images.keys().cloned().collect()
            ||manifest.images.len()>64
            ||manifest.images.values().try_fold(0u64,|sum,image|sum.checked_add(image.length)).is_none_or(|sum|sum>256*1024*1024) {
            return Err("Recovery image mapping does not match the workspace or exceeds its limit".into());
        }
        for(id,snapshot)in &manifest.images{self.image_path(id,snapshot)?;}
        Ok(())
    }
    pub(super) fn read_images(&self,manifest:&Manifest)->Result<BTreeMap<String,Arc<ImageAsset>>,String>{
        let mut assets=BTreeMap::new();
        for(id,snapshot)in &manifest.images{
            let bytes=read_private(&self.image_path(id,snapshot)?,Purpose::Image,MAX_IMAGE_BYTES)?;
            if bytes.len() as u64!=snapshot.length{return Err("Recovery image length does not match".into());}
            let asset=Arc::new(ImageAsset::parse(bytes).map_err(|e|format!("Recovery image is invalid: {e}"))?);
            if asset.info().id!=*id{return Err("Recovery image checksum does not match".into());}
            prepare_pdf_image(&asset).map_err(|e|format!("Recovery image appearance is unsupported: {e}"))?;
            assets.insert(id.clone(),asset);
        }
        Ok(assets)
    }
    pub(super) fn remove_image_files(&self,retained:&BTreeSet<String>)->Result<(),String>{
        for entry in fs::read_dir(self.image_directory()?).map_err(io_error)?{
            let entry=entry.map_err(io_error)?;let name=entry.file_name().to_string_lossy().into_owned();
            if name.strip_suffix(".image").is_some_and(valid_font_id)&&!retained.contains(&name){fs::remove_file(entry.path()).map_err(io_error)?;}
        }
        Ok(())
    }
}
