//! Strict source-proof entry point, separate from permissive rendering APIs.
use super::*;

fn spend(work: &mut usize, count: usize) -> Option<()> {
    match work.checked_sub(count) { Some(left) => {*work=left;Some(())}, None => {*work=0;None} }
}

fn index(index: Index<'_>, work: &mut usize) -> Option<()> {
    spend(work,index.len() as usize)?;
    for i in 0..index.len() { index.get(i)?; }
    Some(())
}

fn integer(value: f64) -> Option<usize> {
    if value.is_finite() && value >= 0.0 && value <= i32::MAX as f64 && (value as i32)as f64==value {Some(value as usize)}else{None}
}

fn dictionary(data: &[u8], private: bool, work: &mut usize) -> Option<Option<Range<usize>>> {
    spend(work,data.len())?;
    // One extra slot distinguishes overflow from an exactly full operand stack.
    let mut operands=[0.0;MAX_OPERANDS_LEN+1];
    let mut parser=DictionaryParser::new(data,&mut operands);
    let mut seen=[false;1240]; let mut range=None;
    while let Some(operator)=parser.parse_next() {
        let op=usize::from(operator.get());
        if op>=seen.len() || seen[op] {return None;}
        seen[op]=true;
        parser.parse_operands()?;
        let values=parser.operands();
        if values.len()>MAX_OPERANDS_LEN || values.iter().any(|v|!v.is_finite() || !(*v as f32).is_finite()) {return None;}
        if private {
            match op {
                19 => {if values.len()!=1 {return None;} integer(values[0])?;},
                20|21 => {if values.len()!=1 {return None;}},
                // These operators belong to Top/Font DICTs, never Private DICTs.
                15..=18|1206|1207|1230|1236|1237 => return None,
                _ => {},
            }
        } else {
            match op {
                15..=17|1236|1237 => {if values.len()!=1 {return None;} integer(values[0])?;},
                18 => {
                    if values.len()!=2 {return None;}
                    let length=integer(values[0])?;let start=integer(values[1])?;
                    range=Some(start..start.checked_add(length)?);
                },
                1207 => {
                    if values.len()!=6 {return None;}
                    let determinant=values[0]*values[3]-values[1]*values[2];
                    if determinant==0.0 || !determinant.is_finite() {return None;}
                },
                1206 => {if values!=[2.0] {return None;}}, // Type2 charstrings only.
                1205 => {if values!=[0.0] {return None;}}, // Filled outlines only.
                1220 => return None, // SyntheticBase needs another font program.
                1230 => {if values.len()!=3 || values.iter().any(|v|integer(*v).is_none()) {return None;}},
                _ => {},
            }
        }
    }
    if !parser.at_end() {return None;}
    Some(range)
}

fn private(data: &[u8], range: Range<usize>, work: &mut usize) -> Option<()> {
    let bytes=data.get(range.clone())?;
    dictionary(bytes,true,work)?;
    if let Some(offset)=parse_private_dict(bytes).local_subroutines_offset {
        let start=range.start.checked_add(offset)?;
        if start<range.end {return None;}
        index(parse_index::<u16>(&mut Stream::new_at(data,start)?)?,work)?;
    }
    Some(())
}

impl<'a> Table<'a> {
    /// Resolves the whole declared simple encoding with one bounded charset
    /// pass, including exact predefined SIDs and nonidentity supplements.
    #[cfg(feature = "glyph-names")]
    pub fn encoding_glyphs_with_budget(&self, remaining_work: &mut usize)
        -> Result<[Option<GlyphId>;256],CFFError> {
        match self.kind {
            FontKind::SID(ref sid)=>sid.encoding.glyphs_with_budget(self.charset,self.number_of_glyphs.get(),remaining_work),
            FontKind::CID(_)=>Err(CFFError::UnsupportedOperator),
        }
    }

    /// Parses one static CFF1 font and validates complete metric dictionaries,
    /// indexes and declared encodings with a shared caller work allowance.
    pub fn parse_for_proof(data: &'a [u8], remaining_work: &mut usize) -> Option<Self> {
        if data.len()<4 || data[0]!=1 || data[1]!=0 || data[2]<4 || !(1..=4).contains(&data[3]) {return None;}
        let mut stream=Stream::new_at(data,usize::from(data[2]))?;
        let names=parse_index::<u16>(&mut stream)?;
        let tops=parse_index::<u16>(&mut stream)?;
        if names.len()!=1 || tops.len()!=1 {return None;}
        index(names,remaining_work)?;index(tops,remaining_work)?;
        let range=dictionary(tops.get(0)?,false,remaining_work)?;
        let table=Self::parse(data)?;
        index(table.strings,remaining_work)?;
        index(table.global_subrs,remaining_work)?;
        index(table.char_strings,remaining_work)?;
        match table.kind {
            FontKind::SID(ref sid) => {
                if let Some(range)=range {private(data,range,remaining_work)?;}
                index(sid.local_subrs,remaining_work)?;
                sid.encoding.validate(table.number_of_glyphs.get(),remaining_work)?;
            },
            FontKind::CID(ref cid) => {
                if range.is_some() {return None;}
                index(cid.fd_array,remaining_work)?;
                for i in 0..cid.fd_array.len() {
                    let range=dictionary(cid.fd_array.get(i)?,false,remaining_work)??;
                    private(data,range,remaining_work)?;
                }
            },
        }
        Some(table)
    }

    /// Name of a declared predefined simple-font encoding, without fallback.
    pub fn predefined_encoding(&self) -> Option<&'static str> {
        match self.kind {FontKind::SID(ref sid)=>sid.encoding.predefined_name(),FontKind::CID(_)=>None}
    }
}
