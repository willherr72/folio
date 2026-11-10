//! Bounded immutable preparation cache. Authorization remains at native composition
//! and recovery boundaries; cache hits never confer permission or acquire an asset.
use super::*;
pub(super) struct PreparedEntry {
    pub prepared:Arc<crate::PreparedPdfImage>,
    pub bytes:usize,
    used:u64,
}
impl ImageRegistry {
    pub(crate) fn prepared_pdf(&self,id:&str)->EngineResult<Arc<crate::PreparedPdfImage>> {
        let asset={
            let mut state=self.lock()?;
            let asset=state.assets.get(id).ok_or_else(||invalid("The requested image is not loaded. Import it again."))?.asset.clone();
            state.prepared_tick=state.prepared_tick.wrapping_add(1);let used=state.prepared_tick;
            if let Some(entry)=state.prepared.get_mut(id){entry.used=used;return Ok(entry.prepared.clone());}
            asset
        };
        // Decoding/compression can be expensive; never hold the registry mutex.
        let prepared=Arc::new(crate::prepare_pdf_image(&asset)?);
        let bytes=crate::image_persistence::resource_bytes(prepared.form());
        let mut state=self.lock()?;
        if !state.assets.contains_key(id)||bytes>self.prepared_limit{return Ok(prepared);}
        state.prepared_tick=state.prepared_tick.wrapping_add(1);let used=state.prepared_tick;
        if let Some(entry)=state.prepared.get_mut(id){entry.used=used;return Ok(entry.prepared.clone());}
        while state.prepared_bytes>self.prepared_limit-bytes {
            let oldest=state.prepared.iter().min_by_key(|(_,entry)|entry.used).map(|(id,_)|id.clone()).expect("nonempty prepared cache");
            let evicted=state.prepared.remove(&oldest).unwrap();state.prepared_bytes-=evicted.bytes;
        }
        state.prepared_bytes+=bytes;
        state.prepared.insert(id.to_owned(),PreparedEntry{prepared:prepared.clone(),bytes,used});
        Ok(prepared)
    }
}
