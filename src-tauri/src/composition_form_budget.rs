//! One cumulative metadata/traversal/parser allowance for form composition.
use crate::{EngineError,EngineResult};
use lopdf::ResourceLimits;
pub(crate) struct Budget {
    pub(crate) nodes:usize,pub(crate) steps:usize,pub(crate) metadata:usize,
    pub(crate) parser:ResourceLimits,
}
impl Default for Budget {
    fn default()->Self {Self{nodes:0,steps:0,metadata:16*1024*1024,
        parser:ResourceLimits{input_bytes:16*1024*1024,decoded_bytes:16*1024*1024,
            object_bytes:16*1024*1024,object_nodes:100_000,xref_entries:0}}}
}
impl Budget {
    pub(crate) fn step(&mut self,depth:usize,check:&mut dyn FnMut()->EngineResult<()>)->EngineResult<()> {
        check()?;
        self.steps=self.steps.checked_add(1).ok_or_else(||bad("step overflow"))?;
        if depth>64 || self.steps>100_000 {return Err(bad("traversal exceeds depth64 or 100000 steps"));}Ok(())
    }
    pub(crate) fn reserve(&mut self,bytes:usize)->EngineResult<()> {
        self.metadata=self.metadata.checked_sub(bytes).ok_or_else(||bad("metadata exceeds 16 MiB"))?;Ok(())
    }
}
fn bad(message:&str)->EngineError {EngineError::InvalidRequest(format!("PDF form composition budget: {message}"))}
