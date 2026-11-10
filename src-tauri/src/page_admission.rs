//! Writer-owned page admission metadata; no external input can supply a proof.
use super::{EngineResult,PageInfo,PdfDocument,PdfPageIndex};
use lopdf::{Dictionary,Document,Object};
use sha2::{Digest,Sha256};

const MIN_PAGES:usize=32;
const MAX_PAGES:usize=100_000;
const MAX_BYTES:usize=8*1024*1024;
const MAX_VALUES:usize=100_000;
const MAX_DEPTH:usize=64;

pub(super) struct Proof {digest:[u8;32],encoded_len:usize,pages:usize}
impl Proof {
    pub(super) fn matches(&self,bytes:&[u8],pages:usize)->bool {
        self.encoded_len==bytes.len() && self.pages==pages && self.digest==<[u8;32]>::from(Sha256::digest(bytes))
    }
}
pub(super) struct Publication {bytes:Vec<u8>,proof:Option<Proof>}
impl Publication {
    pub(super) fn save(document:&mut Document,allow_fast:bool)->EngineResult<Self>{
        // This capability is derived from our actual writer input/output, never
        // restored from PDF hints, frontend metadata or another source version.
        let eligible=allow_fast && document.encryption_state.is_none();
        let bytes=crate::pdf_output::save(document)?;
        let proof=if eligible && bytes.len()<=MAX_BYTES {
            let pages=document.get_pages().len();
            ((MIN_PAGES..=MAX_PAGES).contains(&pages) && annotation_free(document)).then(||Proof {
                digest:Sha256::digest(&bytes).into(),encoded_len:bytes.len(),pages,
            })
        } else {None};
        Ok(Self {bytes,proof})
    }
    pub(super) fn into_bytes(self)->Vec<u8>{self.bytes}
    pub(super) fn into_parts(self)->(Vec<u8>,Option<Proof>){(self.bytes,self.proof)}
}
// Every indirect object is scanned separately; references are not followed.
// Uncertainty only disables the optimization, never rejects the publication.
fn annotation_free(document:&Document)->bool {
    fn dictionary(value:&Dictionary,depth:usize,left:&mut usize)->bool {
        !value.has(b"Annots") && value.iter().all(|(_,value)|object(value,depth+1,left))
    }
    fn object(value:&Object,depth:usize,left:&mut usize)->bool {
        if depth>MAX_DEPTH || *left==0 {return false;}
        *left-=1;
        match value {
            Object::Array(values)=>values.iter().all(|value|object(value,depth+1,left)),
            Object::Dictionary(value)=>dictionary(value,depth,left),
            Object::Stream(value)=>dictionary(&value.dict,depth,left),
            _=>true,
        }
    }
    let mut left=MAX_VALUES-1; // Count the trailer dictionary as one value.
    dictionary(&document.trailer,0,&mut left) && document.objects.values().all(|value|object(value,0,&mut left))
}

pub(super) fn native_size(document:&PdfDocument<'_>,index:PdfPageIndex)->Option<PageInfo>{
    let size=document.pages().page_size(index).ok()?;
    let width=size.width().value;let height=size.height().value;
    (width.is_finite() && height.is_finite() && width>0. && height>0.)
        .then_some(PageInfo {width,height,overlays:Vec::new()})
}

#[cfg(test)]
#[path="page_admission_tests.rs"]
mod tests;
