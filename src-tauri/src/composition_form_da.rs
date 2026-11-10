//! Byte-preserving default-appearance font-name rewriting.
#![allow(dead_code)]
use crate::{composition_form_budget::Budget,EngineError,EngineResult};
use lopdf::{content::Content,Object};
use std::{collections::BTreeMap,ops::Range,sync::Arc};
pub(crate) fn rewrite(bytes:&[u8],aliases:&BTreeMap<Vec<u8>,Vec<u8>>,budget:&mut Budget,cancelled:Arc<dyn Fn()->bool+Send+Sync>,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Option<Vec<u8>>> {
    checkpoint(budget,&cancelled,check,0)?;require(bytes.len()<=64*1024,"DA exceeds 64 KiB")?;
    let (content,remaining)=Content::decode_strict_with_resource_control(bytes,budget.parser,cancelled.clone()).map_err(bad)?;
    budget.parser=remaining;
    let mut changed=false;
    for operation in &content.operations {
        checkpoint(budget,&cancelled,check,0)?;
        if operation.operator!="Tf"{continue;}
        let [Object::Name(name),size]=operation.operands.as_slice() else{return Err(bad("invalid Tf operands"));};
        require(size.as_float().is_ok_and(|n|n.is_finite() && n>=0.),"invalid Tf size")?;
        let alias=aliases.get(name).ok_or_else(||bad("DA font is absent from its source map"))?;
        require(!name.is_empty() && !alias.is_empty() && alias.len()<=64*1024,"invalid font name")?;
        changed|=name!=alias;
    }
    checkpoint(budget,&cancelled,check,0)?;
    if !changed{return Ok(None);}
    let mut edits:Vec<(Range<usize>,&[u8])>=Vec::new();
    {
        let mut scanner=Scanner{bytes,at:0,budget,cancelled:cancelled.clone(),check};
        for operation in &content.operations {
            for (index,operand) in operation.operands.iter().enumerate() {
                let span=scanner.operand(operand)?;
                if operation.operator=="Tf" && index==0 {
                    let name=operand.as_name().map_err(bad)?;let alias=&aliases[name];
                    if name!=alias.as_slice() {
                        scanner.budget.reserve(alias.len().checked_add(64).ok_or_else(||bad("edit size overflow"))?)?;
                        edits.push((span,alias));
                    }
                }
            }
            scanner.space()?;let start=scanner.at;
            while scanner.peek().is_some_and(|byte|byte.is_ascii_alphabetic() || b"*'\"".contains(&byte)){scanner.advance()?;}
            require(&bytes[start..scanner.at]==operation.operator.as_bytes(),"operator span disagrees with strict parser")?;
        }
        scanner.space()?;require(scanner.at==bytes.len(),"unconsumed DA tokens")?;
    }
    let mut length=bytes.len();
    for (span,alias) in &edits {
        checkpoint(budget,&cancelled,check,0)?;
        let encoded=alias.iter().try_fold(1usize,|n,byte|n.checked_add(if regular(*byte) && *byte!=b'#' && (33..=126).contains(byte){1}else{3})).ok_or_else(||bad("encoded name overflow"))?;
        length=length.checked_sub(span.len()).and_then(|n|n.checked_add(encoded)).ok_or_else(||bad("output length overflow"))?;
    }
    require(length<=64*1024,"rewritten DA exceeds 64 KiB")?;
    budget.reserve(length.checked_add(64).ok_or_else(||bad("output size overflow"))?)?;
    let mut output=Vec::with_capacity(length);let mut copied=0;
    for (span,alias) in edits {
        checkpoint(budget,&cancelled,check,0)?;
        require(copied<=span.start && span.start<span.end && span.end<=bytes.len(),"overlapping DA edits")?;
        output.extend_from_slice(&bytes[copied..span.start]);output.push(b'/');
        for byte in alias {
            if regular(*byte) && *byte!=b'#' && (33..=126).contains(byte){output.push(*byte);}else{
                output.push(b'#');output.push(b"0123456789ABCDEF"[(byte>>4) as usize]);output.push(b"0123456789ABCDEF"[(byte&15) as usize]);
            }
        }
        copied=span.end;
    }
    output.extend_from_slice(&bytes[copied..]);require(output.len()==length,"rewritten DA length mismatch")?;
    checkpoint(budget,&cancelled,check,0)?;
    let (verified,remaining)=Content::decode_strict_with_resource_control(&output,budget.parser,cancelled.clone()).map_err(bad)?;
    budget.parser=remaining;
    require(verified.operations.len()==content.operations.len(),"DA operation count changed")?;
    for (original,actual) in content.operations.iter().zip(&verified.operations) {
        checkpoint(budget,&cancelled,check,0)?;
        require(original.operator==actual.operator,"DA operator changed")?;
        if original.operator=="Tf" {
            let name=original.operands[0].as_name().map_err(bad)?;
            require(actual.operands.len()==2 && actual.operands[0].as_name().map_err(bad)?==aliases[name] && actual.operands[1..]==original.operands[1..],"DA font rewrite changed other operands")?;
        }else{require(original.operands==actual.operands,"DA rewrite changed unrelated operands")?;}
    }
    checkpoint(budget,&cancelled,check,0)?;Ok(Some(output))
}

fn bad(message:impl std::fmt::Display)->EngineError {EngineError::InvalidRequest(format!("PDF form DA rewrite: {message}"))}
fn require(condition:bool,message:&str)->EngineResult<()> {if condition{Ok(())}else{Err(bad(message))}}
fn checkpoint(budget:&mut Budget,cancelled:&Arc<dyn Fn()->bool+Send+Sync>,check:&mut dyn FnMut()->EngineResult<()>,depth:usize)->EngineResult<()> {
    if cancelled(){return Err(bad("cancelled"));}budget.step(depth,check)
}
fn whitespace(byte:u8)->bool {b" \t\n\r\0\x0C".contains(&byte)}
fn regular(byte:u8)->bool {!whitespace(byte) && !b"()<>[]{}/%".contains(&byte)}
struct Scanner<'a,'b>{bytes:&'a [u8],at:usize,budget:&'b mut Budget,cancelled:Arc<dyn Fn()->bool+Send+Sync>,check:&'b mut dyn FnMut()->EngineResult<()>}
impl Scanner<'_,'_> {
    fn peek(&self)->Option<u8>{self.bytes.get(self.at).copied()}
    fn step(&mut self,depth:usize)->EngineResult<()> {checkpoint(self.budget,&self.cancelled,self.check,depth)}
    fn advance(&mut self)->EngineResult<()> {
        require(self.at<self.bytes.len(),"unexpected DA end")?;self.at+=1;
        if self.at%256==0{self.step(0)?;}Ok(())
    }
    fn space(&mut self)->EngineResult<()> {
        self.step(0)?;
        loop {
            while self.peek().is_some_and(whitespace){self.advance()?;}
            if self.peek()!=Some(b'%'){return Ok(());}
            while self.peek().is_some_and(|byte|byte!=b'\r' && byte!=b'\n'){self.advance()?;}
        }
    }
    fn exact(&mut self,bytes:&[u8])->EngineResult<()> {
        require(self.bytes.get(self.at..self.at+bytes.len())==Some(bytes),"scalar span disagrees with strict parser")?;
        for _ in bytes{self.advance()?;}Ok(())
    }
    fn operand(&mut self,value:&Object)->EngineResult<Range<usize>> {
        self.space()?;let start=self.at;
        match value {
            Object::Name(name)=>{
                self.exact(b"/")?;while self.peek().is_some_and(regular){self.advance()?;}
                self.budget.reserve(self.at-start)?;
                let mut index=start+1;let mut decoded=0;
                while index<self.at {
                    let byte=if self.bytes[index]==b'#' {
                        let pair=self.bytes.get(index+1..index+3).ok_or_else(||bad("incomplete name escape"))?;
                        let high=(pair[0] as char).to_digit(16).ok_or_else(||bad("invalid name escape"))?;
                        let low=(pair[1] as char).to_digit(16).ok_or_else(||bad("invalid name escape"))?;
                        index+=3;(high*16+low) as u8
                    }else{let byte=self.bytes[index];index+=1;byte};
                    require(name.get(decoded)==Some(&byte),"name span disagrees with strict parser")?;decoded+=1;
                }
                require(decoded==name.len(),"name span length disagrees with strict parser")?;
            },
            Object::Integer(_)|Object::Real(_)=>{
                if self.peek().is_some_and(|byte|byte==b'+' || byte==b'-'){self.advance()?;}
                while self.peek().is_some_and(|byte|byte.is_ascii_digit()){self.advance()?;}
                if matches!(value,Object::Real(_)){self.exact(b".")?;while self.peek().is_some_and(|byte|byte.is_ascii_digit()){self.advance()?;}}
            },
            Object::Boolean(value)=>self.exact(if *value{b"true"}else{b"false"})?,
            Object::Null=>self.exact(b"null")?,
            Object::Array(_)|Object::Dictionary(_)|Object::String(..)=>self.aggregate(0)?,
            _=>return Err(bad("unsupported direct DA operand")),
        }
        require(start<self.at,"empty operand span")?;Ok(start..self.at)
    }
    fn aggregate(&mut self,depth:usize)->EngineResult<()> {
        self.step(depth)?;self.space()?;
        match self.peek().ok_or_else(||bad("missing operand"))? {
            b'('=>{
                self.advance()?;let mut nesting=1usize;
                while nesting>0 {
                    match self.peek().ok_or_else(||bad("unterminated string"))? {
                        b'\\'=>{self.advance()?;if self.peek().is_some(){self.advance()?;}},
                        b'('=>{nesting+=1;require(nesting<=64,"literal string nesting exceeds limit")?;self.advance()?;},
                        b')'=>{nesting-=1;self.advance()?;},
                        _=>self.advance()?,
                    }
                }
            },
            b'['=>{
                self.advance()?;loop{self.space()?;if self.peek()==Some(b']'){self.advance()?;break;}self.aggregate(depth+1)?;}
            },
            b'<' if self.bytes.get(self.at+1)==Some(&b'<')=>{
                self.exact(b"<<")?;loop{self.space()?;if self.bytes.get(self.at..self.at+2)==Some(b">>"){self.exact(b">>")?;break;}self.aggregate(depth+1)?;}
            },
            b'<'=>{self.advance()?;while self.peek().is_some_and(|byte|byte!=b'>'){self.advance()?;}self.exact(b">")?;},
            b'/'=>{self.advance()?;while self.peek().is_some_and(regular){self.advance()?;}},
            byte if regular(byte)=>{while self.peek().is_some_and(regular){self.advance()?;}},
            _=>return Err(bad("unexpected aggregate delimiter")),
        }
        Ok(())
    }
}
#[cfg(test)]
#[path="composition_form_da_tests.rs"]
mod tests;
