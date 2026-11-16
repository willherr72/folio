//! CFF source resources keep their original bytes and exact PDF character codes.
//! Rendering names and ToUnicode alone never establish glyph/width authority.
use super::*;
use super::super::encoding_names as names;
#[path = "embedded_cff_glyph_names.rs"]
mod glyph_names;
use ttf_parser::{cff, GlyphId, Tag};

const WORK: usize = 2_000_000;

pub(super) fn uses(pdf: &Document, font: &Dictionary) -> bool {
    descendant(pdf,font).ok().filter(|child|matches!(child.get(b"Subtype").and_then(Object::as_name).ok(),Some(b"Type1"|b"CIDFontType0")))
        .and_then(|child|child.get(b"FontDescriptor").ok())
        .and_then(|value|dict(pdf,value).ok()).is_some_and(|descriptor|descriptor.has(b"FontFile3"))
}

fn container<'a>(pdf: &'a Document, font: &'a Dictionary) -> EngineResult<(&'a Dictionary,bool,&'a [u8])> {
    let composite=font.get(b"Subtype").and_then(Object::as_name).ok()==Some(b"Type0");
    let child=descendant(pdf,font)?;
    if (composite && child.get(b"Subtype").and_then(Object::as_name).ok()!=Some(b"CIDFontType0"))
        || (!composite && font.get(b"Subtype").and_then(Object::as_name).ok()!=Some(b"Type1")) {
        return Err(invalid("The CFF program and PDF font subtype disagree."));
    }
    if child.has(b"CIDToGIDMap") {return Err(invalid("A CFF CIDFont cannot use a TrueType CIDToGIDMap."));}
    let descriptor=dict(pdf,field(child,b"FontDescriptor")?)?;
    if [b"FontFile".as_slice(),b"FontFile2",b"FontFile3"].iter().filter(|name|descriptor.has(name)).count()!=1 {
        return Err(invalid("The embedded CFF font has competing font programs."));
    }
    let value=pdf.dereference(field(descriptor,b"FontFile3")?).map_err(|_|invalid("The CFF stream is malformed."))?.1
        .as_stream().map_err(|_|invalid("The CFF stream is malformed."))?;
    let subtype=field(&value.dict,b"Subtype")?.as_name().map_err(|_|invalid("The CFF stream subtype is malformed."))?;
    if subtype!=b"OpenType" && subtype!=if composite {b"CIDFontType0C".as_slice()}else{b"Type1C".as_slice()} {
        return Err(invalid("The CFF stream container and PDF font subtype disagree."));
    }
    Ok((descriptor,composite,subtype))
}

pub(super) fn program(pdf: &Document, font: &Dictionary) -> EngineResult<Vec<u8>> {
    let (descriptor,_,_)=container(pdf,font)?;
    stream(pdf,field(descriptor,b"FontFile3")?,MAX_FONT)
}

fn charge(work: &mut usize, amount: usize) -> EngineResult<()> {
    *work=work.checked_sub(amount).ok_or_else(||invalid("The embedded CFF proof exceeds its work limit."))?;
    Ok(())
}

fn glyph_character(name: &str) -> Option<char> {
    if let Some(ch)=glyph_names::character(name) {return Some(ch);}
    let digits=if name.starts_with("uni") && name.len()==7 {&name[3..]}
        else if name.starts_with('u') && (5..=7).contains(&name.len()) {&name[1..]}else{return None;};
    if !digits.bytes().all(|b|b.is_ascii_hexdigit()) {return None;}
    u32::from_str_radix(digits,16).ok().and_then(char::from_u32)
}

fn named_encoding(name: &[u8]) -> EngineResult<&'static [Option<&'static str>;256]> {
    match name {
        b"StandardEncoding"=>Ok(&names::STANDARD_ENCODING),
        b"WinAnsiEncoding"=>Ok(&names::WIN_ANSI_ENCODING),
        b"MacRomanEncoding"=>Ok(&names::MAC_ROMAN_ENCODING),
        b"ExpertEncoding"=>Ok(&names::EXPERT_ENCODING),
        _=>Err(invalid("The CFF font uses an unsupported simple encoding.")),
    }
}

fn simple_codes(pdf: &Document,font: &Dictionary,table: &cff::Table<'_>,
    glyphs: &[cff::GlyphInfo<'_>],work: &mut usize) -> EngineResult<[Option<u16>;256]> {
    let by_name: HashMap<_,_>=glyphs.iter().filter_map(|g|g.name.map(|n|(n,g.glyph_id.0))).collect();
    if by_name.len()!=glyphs.len() {return Err(invalid("The CFF glyph names are ambiguous."));}
    let mut codes=[None;256];
    let implicit=font.get(b"Encoding").is_err();
    let encoding=font.get(b"Encoding").ok().map(|v|pdf.dereference(v).map(|(_,v)|v))
        .transpose().map_err(|_|invalid("The CFF encoding cannot be resolved."))?;
    let (base,differences)=match encoding {
        Some(Object::Name(name))=>(Some(named_encoding(name)?),None),
        Some(Object::Dictionary(value))=>{
            let base=match value.get(b"BaseEncoding") {
                Ok(value)=>Some(named_encoding(pdf.dereference(value).map_err(|_|invalid("The CFF base encoding is malformed."))?.1.as_name().map_err(|_|invalid("The CFF base encoding is malformed."))?)?),
                Err(_)=>None,
            };
            (base,value.get(b"Differences").ok())
        },
        None=>(None,None),
        _=>return Err(invalid("The CFF encoding is malformed.")),
    };
    if let Some(base)=base {
        for (code,name) in base.iter().enumerate() {codes[code]=name.and_then(|name|by_name.get(name).copied()).filter(|id|*id!=0);}
    } else {
        // Resolve declared built-ins by their actual SIDs/GIDs, rather than
        // interpreting them as PDF name overrides or rescanning per byte.
        codes=table.encoding_glyphs_with_budget(work).map_err(|_|invalid("The declared CFF encoding cannot be verified within its work limit."))?.map(|gid|gid.map(|gid|gid.0));
    }
    if let Some(value)=differences {
        let differences=array(pdf,value)?;
        if differences.len()>512 {return Err(invalid("The CFF Differences encoding exceeds its limit."));}
        let mut code=None;let mut seen=[false;256];let mut pending=false;
        for value in differences {
            match value {
                Object::Integer(value) if (0..=255).contains(value) && !pending=>{code=Some(*value as usize);pending=true;},
                Object::Name(name)=>{
                    let index=code.filter(|code|*code<256).ok_or_else(||invalid("The CFF Differences encoding has an invalid character range."))?;
                    if seen[index] {return Err(invalid("The CFF Differences encoding is ambiguous."));}
                    seen[index]=true;
                    pending=false;
                    charge(work,name.len())?;
                    let name=std::str::from_utf8(name).map_err(|_|invalid("The CFF glyph name is malformed."))?;
                    codes[index]=by_name.get(name).copied().filter(|id|*id!=0);
                    code=Some(index+1);
                },
                _=>return Err(invalid("The CFF Differences encoding is malformed.")),
            }
        }
        if code.is_none() || pending {return Err(invalid("The CFF Differences encoding is incomplete."));}
    } else if !implicit && encoding.is_some_and(|v|matches!(v,Object::Dictionary(_))) && base.is_none() {
        return Err(invalid("The CFF encoding dictionary has no declared mapping."));
    }
    Ok(codes)
}

pub(super) fn verify(pdf: &Document,font: &Dictionary,bytes: &[u8],all_scalars: bool) -> EngineResult<VerifiedFont> {
    let (_,composite,subtype)=container(pdf,font)?;
    if program(pdf,font)?!=bytes {return Err(invalid("The native CFF program differs from its source resource."));}
    let face=if subtype==b"OpenType" {
        let face=ttf_parser::Face::parse(bytes,0).map_err(|_|invalid("The embedded OpenType CFF program is malformed."))?;
        if face.is_variable() || face.raw_face().table(Tag::from_bytes(b"CFF2")).is_some()
            || face.raw_face().table(Tag::from_bytes(b"glyf")).is_some()
            || face.tables().hmtx.is_none() || face.tables().cff.is_none() {
            return Err(invalid("Only static CFF1 outlines and horizontal metrics can be edited."));
        }
        crate::fonts::bounded_cmap_coverage(face.raw_face().table(Tag::from_bytes(b"cmap")).ok_or_else(||invalid("The OpenType CFF program has no character map."))?)?;
        Some(face)
    }else{None};
    let raw=if let Some(face)=&face {face.raw_face().table(Tag::from_bytes(b"CFF ")).ok_or_else(||invalid("The OpenType font has no CFF1 table."))?}else{bytes};
    let mut work=WORK;
    let table=cff::Table::parse_for_proof(raw,&mut work).ok_or_else(||invalid("The CFF program dictionaries, indexes, or encoding cannot be verified."))?;
    let glyphs: Vec<_>=table.glyph_infos(&mut work).collect::<Result<_,_>>().map_err(|_|invalid("The CFF glyph identities or matrices cannot be verified."))?;
    let keyed=glyphs.first().is_some_and(|g|g.cid.is_some());
    if keyed && !composite {return Err(invalid("A CID-keyed CFF program requires a composite font resource."));}
    if face.as_ref().is_some_and(|f|usize::from(f.number_of_glyphs())!=glyphs.len()) {return Err(invalid("The OpenType glyph counts disagree."));}
    let child=descendant(pdf,font)?;
    if composite && field(font,b"Encoding")?.as_name().ok()!=Some(b"Identity-H") {
        return Err(invalid("Only horizontal Identity-H CFF composite encodings can be edited."));
    }
    let codes=if composite {None}else{Some(simple_codes(pdf,font,&table,&glyphs,&mut work)?)};
    let by_cid: HashMap<_,_>=glyphs.iter().filter_map(|g|g.cid.map(|cid|(cid,g.glyph_id.0))).collect();
    let widths=if composite && !child.has(b"W") {BTreeMap::new()}else{widths(pdf,child,composite)?};
    let unicode=if let Ok(value)=font.get(b"ToUnicode") {
        let bytes=stream(pdf,value,MAX_CMAP)?;
        if all_scalars {
            cmap_with_policy(&bytes,if composite {2}else{1},CMapPolicy::Complete)?.into_iter()
                .map(|(code,ch)|(code,ch.map_or(SourceMapping::Unavailable,SourceMapping::Scalar))).collect()
        }else{source_cmap(&bytes,if composite {2}else{1})?}
    } else if let Some(codes)=&codes {
        codes.iter().enumerate().filter(|(code,_)|widths.contains_key(&(*code as u16))).filter_map(|(code,gid)|gid.and_then(|id|glyphs.get(usize::from(id))).and_then(|g|g.name).and_then(glyph_character)
            .filter(|c|all_scalars || crate::fonts::allowed_character(*c)).map(|ch|(code as u16,SourceMapping::Scalar(ch)))).collect()
    } else {return Err(invalid("The composite CFF resource has no ToUnicode mapping."));};
    // Count declared candidates before filtering physical evidence. An invalid
    // alias must never make its duplicate become the chosen reverse mapping.
    let mut sequence_counts=BTreeMap::new();
    for mapping in unicode.values() {
        if let SourceMapping::Sequence(text)=mapping {
            *sequence_counts.entry(text.clone()).or_insert(0usize)+=1;
        }
    }
    let mut characters=BTreeMap::new();let mut advances=HashMap::new();let mut sequences=Vec::new();
    // Preserve the existing scalar proof before spending its remaining work on
    // newly admitted sequences, regardless of source character-code order.
    let (scalars,candidates):(Vec<_>,Vec<_>)=unicode.into_iter().partition(|(_,mapping)|!matches!(mapping,SourceMapping::Sequence(_)));
    for (code,mapping) in scalars.into_iter().chain(candidates) {
        let (character,text)=match mapping {
            SourceMapping::Scalar(ch) if all_scalars || crate::fonts::allowed_character(ch)=>(Some(ch),ch.to_string()),
            SourceMapping::Sequence(text) if !all_scalars && source_sequence(&text)=>(None,text),
            _=>continue,
        };
        // Failure of a newly considered sequence leaves that code unavailable.
        // The established scalar proof remains strict, as does exact decoding of
        // any source operand containing an excluded code.
        let proved=(||->EngineResult<(u16,f32,[f32;4])> {
            let gid=if let Some(codes)=&codes {codes.get(usize::from(code)).copied().flatten()}
                else if keyed {by_cid.get(&code).copied()}else{Some(code)};
            let gid=gid.filter(|id|*id!=0).ok_or_else(||invalid("The CFF mapping selects an absent or undefined glyph."))?;
            let glyph=glyphs.get(usize::from(gid)).ok_or_else(||invalid("The CFF mapping selects a missing glyph."))?;
            let m=glyph.matrix;
            if m.sx<=0. || m.sy<=0. || m.kx!=0. || m.ky!=0. || m.tx!=0. || m.ty!=0. {
                return Err(invalid("The CFF glyph matrix is not a supported horizontal font matrix."));
            }
            if let (Some(face),Some(ch))=(&face,character) {
                let maps=face.tables().cmap.ok_or_else(||invalid("The OpenType CFF character map is absent."))?;
                if maps.subtables.len()>32 {return Err(invalid("The OpenType CFF program has too many character maps."));}
                let mut proved=false;
                for subtable in maps.subtables.into_iter().filter(|t|t.is_unicode()) {
                    use ttf_parser::cmap::Format;
                    let max=match subtable.format {Format::UnicodeVariationSequences(_)=>continue,Format::ByteEncodingTable(_)=>255,Format::HighByteMappingThroughTable(_)|Format::SegmentMappingToDeltaValues(_)|Format::TrimmedTableMapping(_)=>65535,_=>0x10ffff};
                    if ch as u32>max {continue;}
                    charge(&mut work,1)?;
                    if subtable.glyph_index(ch as u32)!=Some(GlyphId(gid)) {return Err(invalid("The CFF source mapping disagrees with its OpenType Unicode cmap."));}
                    proved=true;
                }
                if !proved {return Err(invalid("The OpenType CFF Unicode glyph cannot be verified."));}
            } else if face.is_none() && (character.is_none() || !composite) {
                if let Some(expected)=glyph.name.and_then(glyph_character) {
                    let agrees=if let Some(ch)=character {expected==ch}else{
                        use unicode_normalization::UnicodeNormalization;
                        expected.to_string().nfkd().eq(text.nfkd())
                    };
                    if !agrees {return Err(invalid("The CFF glyph name disagrees with its Unicode mapping."));}
                }
            }
            let (actual,ink)=if let Some(value)=advances.get(&gid) {*value}else{
                let (raw,bounds)=table.glyph_metrics_with_budget(GlyphId(gid),&mut work).ok_or_else(||invalid("The CFF glyph width and outline cannot be verified within its work limit."))?;
                let advance=raw*m.sx*1000.;
                if !advance.is_finite() || !(0.0..=10000.).contains(&advance) {return Err(invalid("The CFF glyph advance is invalid."));}
                if let Some(face)=&face {
                    let sfnt=face.glyph_hor_advance(GlyphId(gid)).ok_or_else(||invalid("The OpenType CFF glyph has no horizontal metric."))? as f32*1000./face.units_per_em() as f32;
                    if (advance-sfnt).abs()>1.01 || (m.sx*face.units_per_em() as f32-1.).abs()>0.00001 || (m.sy*face.units_per_em() as f32-1.).abs()>0.00001 {
                        return Err(invalid("The CFF charstring matrix/advance disagrees with the OpenType metrics."));
                    }
                }
                let value=(advance,[bounds.x_min as f32*m.sx,bounds.y_min as f32*m.sy,bounds.x_max as f32*m.sx,bounds.y_max as f32*m.sy]);
                advances.insert(gid,value);value
            };
            let width=if let Some(width)=widths.get(&code) {*width}else if composite {
                match child.get(b"DW") {Ok(value)=>value.as_float().map_err(|_|invalid("The CFF default width is malformed."))?,Err(_)=>1000.}
            }else{return Err(invalid("The CFF glyph has no PDF width."));};
            if !width.is_finite() || !(0.0..=10000.).contains(&width) || (actual-width).abs()>1.01 {return Err(invalid("The PDF width disagrees with its embedded CFF program."));}
            Ok((gid,width,ink))
        })();
        let (gid,width,ink)=match proved {Ok(value)=>value,Err(error) if character.is_some()=>return Err(error),Err(_)=>continue};
        if let Some(ch)=character {
            if characters.insert(ch,(code,gid,width)).is_some() {return Err(invalid("The CFF resource has ambiguous Unicode reverse mappings."));}
        }else if ink.iter().all(|v|v.is_finite()) && ink[2]>ink[0] && ink[3]>ink[1] && width>0. && sequence_counts[&text]==1 {
            sequences.push(super::super::source_glyphs::Symbol{code,text,gid,width});
        }
    }
    if let Some(face)=&face {
        let aliases=sequence_aliases(face,&sequences);let mut ordinal=0;
        sequences.retain(|_|{let keep=aliases[ordinal];ordinal+=1;keep});
    }
    let used=characters.values().map(|(_,gid,_)|*gid).chain(sequences.iter().map(|symbol|symbol.gid)).collect::<HashSet<_>>();
    let source_ink=Some(Arc::new(advances.into_iter().filter(|(gid,_)|used.contains(gid)).map(|(gid,(_,ink))|(gid,ink)).collect()));
    let book=if sequences.is_empty(){None}else{
        sequences.extend(characters.iter().map(|(ch,(code,gid,width))|super::super::source_glyphs::Symbol{code:*code,text:ch.to_string(),gid:*gid,width:*width}));
        Some(Arc::new(super::super::source_glyphs::Codebook::new(sequences,if composite {2}else{1}).map_err(invalid)?))
    };
    if characters.is_empty() && book.is_none() {return Err(invalid("The CFF resource has no verified characters."));}
    Ok(VerifiedFont {characters,book,source_ink,code_len:if composite {2}else{1},requires_source_operands:true,preserve_source_encoding:true,
        display_name:face.as_ref().and_then(crate::fonts::font_display_name)})
}
