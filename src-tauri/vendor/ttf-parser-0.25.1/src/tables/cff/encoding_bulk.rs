//! One bounded charset pass for all declared single-byte encoding entries.
use super::*;
use super::super::{charset::CharsetIter,CFFError};

fn charge(remaining: &mut usize, count: usize) -> Result<(),CFFError> {
    match remaining.checked_sub(count) {
        Some(left)=>{*remaining=left;Ok(())},
        None=>{*remaining=0;Err(CFFError::WorkLimitReached)},
    }
}

impl Encoding<'_> {
    pub(crate) fn glyphs_with_budget(&self,charset: Charset<'_>,glyphs: u16,remaining: &mut usize)
        -> Result<[Option<GlyphId>;256],CFFError> {
        charge(remaining,1)?;
        if self.validate(glyphs,remaining).is_none() {return Err(if *remaining==0 {CFFError::WorkLimitReached}else{CFFError::InvalidArgumentsStackLength});}
        let mut output=[None;256];let mut sids=[0u16;256];
        match self.kind {
            EncodingKind::Standard=>{for (code,sid) in STANDARD_ENCODING.iter().enumerate(){sids[code]=u16::from(*sid);}},
            EncodingKind::Expert=>sids=expert_sids::EXPERT,
            EncodingKind::Format0(ref table)=>{
                for (index,code) in table.into_iter().enumerate(){output[usize::from(code)]=Some(GlyphId((index+1)as u16));}
            },
            EncodingKind::Format1(ref table)=>{
                let mut glyph=1u16;
                for range in *table {
                    let last=range.first.checked_add(range.left).ok_or(CFFError::ReadOutOfBounds)?;
                    for code in range.first..=last {output[usize::from(code)]=Some(GlyphId(glyph));glyph+=1;}
                }
            },
        }
        for item in self.supplemental {sids[usize::from(item.code)]=item.name.0;output[usize::from(item.code)]=None;}
        charge(remaining,256)?;
        let mut wanted=[(0u16,0u16);256];let mut count=0;
        for (code,sid) in sids.iter().enumerate() {
            if *sid!=0 {wanted[count]=(*sid,code as u16);count+=1;}
        }
        // The at-most256 sort/lookup keys are bounded independently of font
        // size. A fixed conservative charge covers this small in-place sort.
        charge(remaining,256*16)?;
        let wanted=&mut wanted[..count];wanted.sort_unstable();
        let mut seen=[0u64;1024];let mut identifiers=CharsetIter::new(charset,glyphs);
        for gid in 0..glyphs {
            charge(remaining,1)?;
            let sid=identifiers.next().ok_or(CFFError::ReadOutOfBounds)?.0;
            let slot=usize::from(sid)/64;let bit=1u64<<(sid%64);
            if seen[slot]&bit!=0 {return Err(CFFError::InvalidArgumentsStackLength);}
            seen[slot]|=bit;
            let mut low=0;let mut high=count;
            while low<high {
                charge(remaining,1)?;let mid=low+(high-low)/2;
                if wanted[mid].0<sid {low=mid+1;}else{high=mid;}
            }
            while low<count && wanted[low].0==sid {
                charge(remaining,1)?;
                if gid!=0 {output[usize::from(wanted[low].1)]=Some(GlyphId(gid));}
                low+=1;
            }
        }
        for item in self.supplemental {
            if item.name.0!=0 && output[usize::from(item.code)].is_none() {return Err(CFFError::ReadOutOfBounds);}
        }
        Ok(output)
    }
}
