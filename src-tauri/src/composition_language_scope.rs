//! Page-content framing used after source graph language admission by Save.
#![allow(dead_code)]
use crate::{EngineError, EngineResult};
use lopdf::{content::{Content,Operation}, Object, ResourceLimits};
use std::sync::Arc;

pub(crate) struct Envelope {
    pub prefix: Vec<u8>,
    pub suffix: &'static [u8],
    pub operations: Vec<Operation>,
}

pub(crate) fn inspect(
    data: &[u8],
    language: &[u8],
    mut remaining: ResourceLimits,
    cancelled: Arc<dyn Fn() -> bool + Send + Sync>,
) -> EngineResult<(Envelope, ResourceLimits)> {
    probe(&cancelled)?;
    require(data.len()<=16*1024*1024 && language.len()<=1024)?;
    let aggregate=remaining;
    remaining.object_bytes=remaining.object_bytes.min(32*1024*1024);
    remaining.object_nodes=remaining.object_nodes.min(200_000);
    let local=remaining;
    let (content,mut remaining)=Content::decode_strict_with_resource_control(data,remaining,cancelled.clone())
        .map_err(|error|EngineError::InvalidRequest(format!("PDF page language scope: {error}")))?;
    // The local ceiling is not consumption: carry only actual charges back to
    // the caller's aggregate allowances for subsequent pages.
    remaining.object_bytes=aggregate.object_bytes-(local.object_bytes-remaining.object_bytes);
    remaining.object_nodes=aggregate.object_nodes-(local.object_nodes-remaining.object_nodes);
    // Mark and text scopes cannot cross. Graphics saves have a separate stack;
    // retain Tr across q/Q so a hidden/clipping-only page is not misclassified.
    #[derive(PartialEq)] enum Scope {Mark,Text}
    let mut scopes=Vec::new();
    let mut saved_modes=Vec::new();
    let (mut text,mut path,mut pending_clip)=(false,false,false);
    let (mut visible,mut clipping,mut clipping_text,mut saved)=(false,false,false,false);
    let (mut mode,mut compatibility)=(0i64,0usize);
    for operation in &content.operations {
        probe(&cancelled)?;
        let args=&operation.operands;
        match operation.operator.as_str() {
            "BMC"|"BDC"|"MP"|"DP"=>{
                require(!path)?;
                let properties=matches!(operation.operator.as_str(),"BDC"|"DP");
                require(args.len()==if properties {2}else{1})?;
                require(matches!(&args[0],Object::Name(_)))?;
                if properties {require(matches!(&args[1],Object::Name(_)|Object::Dictionary(_)))?;}
                if matches!(operation.operator.as_str(),"BMC"|"BDC") {
                    require(scopes.len()<64)?;scopes.push(Scope::Mark);
                }
            }
            "EMC"=>{require(args.is_empty() && !path && scopes.pop()==Some(Scope::Mark))?;}
            "BT"=>{
                require(args.is_empty() && !path && !text && scopes.len()<64)?;
                scopes.push(Scope::Text);text=true;
            }
            "ET"=>{require(args.is_empty() && text && scopes.pop()==Some(Scope::Text))?;text=false;}
            "q"=>{
                require(args.is_empty() && !text && saved_modes.len()<64)?;
                saved_modes.push(mode);saved=true;
            }
            "Q"=>{require(args.is_empty() && !text)?;mode=saved_modes.pop().ok_or_else(bad)?;saved=true;}
            "Tr"=>{
                require(args.len()==1)?;
                mode=args[0].as_i64().map_err(|_|bad())?;
                require((0..=7).contains(&mode))?;
            }
            "Tj"|"TJ"|"'"|"\""=>{
                require(text)?;
                match operation.operator.as_str() {
                    "TJ"=>{
                        require(args.len()==1)?;
                        let values=args[0].as_array().map_err(|_|bad())?;
                        for value in values {probe(&cancelled)?;require(matches!(value,Object::String(..)) || number(value))?;}
                    }
                    "\""=>{require(args.len()==3 && number(&args[0]) && number(&args[1]) && matches!(&args[2],Object::String(..)))?;}
                    _=>{require(args.len()==1 && matches!(&args[0],Object::String(..)))?;}
                }
                visible|=mode!=3 && mode!=7;
                clipping|=mode==7;
                clipping_text|=mode==7;
            }
            "m"|"re"=>{require(!text)?;numbers(args,if operation.operator=="m"{2}else{4})?;path=true;}
            "l"|"c"|"v"|"y"|"h"=>{
                require(!text && path)?;
                numbers(args,match operation.operator.as_str(){"l"=>2,"c"=>6,"h"=>0,_=>4})?;
            }
            "W"|"W*"=>{require(args.is_empty() && !text && path && !pending_clip)?;pending_clip=true;}
            "n"|"S"|"s"|"f"|"F"|"f*"|"B"|"B*"|"b"|"b*"=>{
                require(args.is_empty() && !text && path)?;
                if operation.operator=="n" {clipping|=pending_clip;} else {visible=true;}
                path=false;pending_clip=false;
            }
            "Do"|"sh"=>{
                require(!text && !path && args.len()==1 && matches!(&args[0],Object::Name(_)))?;
                visible=true;
            }
            "BX"=>{require(args.is_empty() && compatibility<64)?;compatibility+=1;}
            "EX"=>{require(args.is_empty() && compatibility>0)?;compatibility-=1;}
            // These operators do not affect the framing or clipping-sequence
            // classification. This helper is not a full resource/graphics proof.
            "w"|"J"|"j"|"M"|"d"|"ri"|"i"|"gs"|"cm"|
            "Tc"|"Tw"|"Tz"|"TL"|"Tf"|"Ts"|"Td"|"TD"|"Tm"|"T*"|
            "CS"|"cs"|"SC"|"SCN"|"sc"|"scn"|"G"|"g"|"RG"|"rg"|"K"|"k"=>{},
            _=>return Err(bad()),
        }
    }
    require(scopes.is_empty() && saved_modes.is_empty() && !path && compatibility==0)?;
    // PDF 1.7 10.5.2: introducing a marked clipping sequence containing q/Q
    // is invalid even if the original unmarked page itself was valid.
    require(!clipping || visible || !saved)?;
    // A non-clipping marked sequence excludes clipping objects, including
    // clipping text. A legal outer Span alone would not give that text the
    // requested language. It needs a separately owned inner language scope.
    require(!clipping_text || !visible)?;
    probe(&cancelled)?;
    let mut prefix=b"/Span << /Lang <".to_vec();
    const HEX:&[u8]=b"0123456789ABCDEF";
    for byte in language {prefix.push(HEX[(byte>>4) as usize]);prefix.push(HEX[(byte&15) as usize]);}
    prefix.extend_from_slice(b"> >> BDC\n");
    Ok((Envelope{prefix,suffix:b"\nEMC\n",operations:content.operations},remaining))
}

fn bad()->EngineError {EngineError::InvalidRequest("PDF page language scope needs unsupported or inconsistent content handling".into())}
fn require(ok:bool)->EngineResult<()> {if ok {Ok(())} else {Err(bad())}}
fn probe(cancelled:&Arc<dyn Fn()->bool+Send+Sync>)->EngineResult<()> {
    if cancelled(){Err(EngineError::InvalidRequest("PDF page language scope cancelled".into()))}else{Ok(())}
}
fn number(value:&Object)->bool {match value {Object::Integer(_)=>true,Object::Real(value)=>value.is_finite(),_=>false}}
fn numbers(values:&[Object],count:usize)->EngineResult<()> {require(values.len()==count && values.iter().all(number))}

#[cfg(test)]
#[path="composition_language_scope_tests.rs"]
mod tests;
