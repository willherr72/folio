//! Independent public/constructed metrics contract for the bounded parser API.
use std::path::PathBuf;
use serde_json::Value;
use ttf_parser::{cff::Table,GlyphId};

fn fixtures()->PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/source-cff")
}
fn fixture(name:&str)->Vec<u8> {
    let base=std::env::var_os("FOLIO_CFF_FIXTURES").map(PathBuf::from).unwrap_or_else(fixtures);
    std::fs::read(base.join(name)).unwrap()
}
fn expected()->Value {serde_json::from_slice(&fixture("metrics.json")).unwrap()}

fn iso_adobe_font()->Vec<u8> {
    let names=index(&[b"FolioIsoAdobeControl".to_vec()]);
    let mut top=dict_integer(4+names.len()+index(&[vec![0;6]]).len()+4);top.push(17);
    let mut out=vec![1,0,4,4];out.extend(names);out.extend(index(&[top]));out.extend([0,0,0,0]);out.extend(index(&vec![vec![14];229]));out
}

#[test]
fn complete_encoding_resolution_shares_metadata_budget_on_large_font() {
    let bytes=fixture("large-custom-encoding-v2.cff");let mut work=2_000_000;
    let table=Table::parse_for_proof(&bytes,&mut work).unwrap();
    assert_eq!(table.glyph_infos(&mut work).collect::<Result<Vec<_>,_>>().unwrap().len(),65535);
    let codes=table.encoding_glyphs_with_budget(&mut work).unwrap();
    for code in 0..255 {assert_eq!(codes[code],Some(GlyphId((code+1)as u16)));}
    assert_eq!(codes[255],Some(GlyphId(65135)));assert!(work>0);
    assert_eq!(table.encoding_glyphs_with_budget(&mut 0).unwrap_err(),ttf_parser::CFFError::WorkLimitReached);
}

#[test]
fn complete_encoding_preserves_omissions_and_predefined_expert_slots() {
    for name in ["fractional.cff","custom-encoding.cff","expert-encoding.cff"] {
        let bytes=fixture(name);let table=Table::parse_for_proof(&bytes,&mut 2_000_000).unwrap();
        let codes=table.encoding_glyphs_with_budget(&mut 2_000_000).unwrap();
        assert_eq!(codes[32],Some(GlyphId(1)));
        if name=="expert-encoding.cff" {assert_eq!(codes[65],None);}else{assert_eq!(codes[65],Some(GlyphId(2)));}
        if name=="custom-encoding.cff" {assert_eq!(codes[67],None);}
    }
    let bytes=iso_adobe_font();let table=Table::parse_for_proof(&bytes,&mut 2_000_000).unwrap();
    let codes=table.encoding_glyphs_with_budget(&mut 2_000_000).unwrap();assert_eq!(codes[65],Some(GlyphId(34)));
}

#[test]
fn complete_encoding_rejects_missing_supplements_overlapping_codes_and_charsets() {
    let bytes=fixture("large-custom-encoding-v2.cff");
    let start=bytes.windows(6).position(|w|w==[0x80,255,0,1,2,3]).unwrap();
    let mut missing=bytes.clone();missing[start+259..start+261].copy_from_slice(&65535u16.to_be_bytes());
    let table=Table::parse_for_proof(&missing,&mut 2_000_000).unwrap();
    assert!(table.encoding_glyphs_with_budget(&mut 2_000_000).is_err(),"A supplement must resolve to an actual charset SID");
    let mut overlap=bytes.clone();overlap[start+258]=0;
    assert!(Table::parse_for_proof(&overlap,&mut 2_000_000).is_none(),"A supplement cannot also own a base code");
    let mut overflow=bytes.clone();overflow[start..start+4].copy_from_slice(&[1,1,250,10]);
    assert!(Table::parse_for_proof(&overflow,&mut 2_000_000).is_none(),"An encoding range cannot wrap byte255");
    let mut duplicate=bytes.clone();let charset=bytes.windows(7).position(|w|w==[0,255,254,255,253,255,252]).unwrap();
    duplicate[charset+3..charset+5].copy_from_slice(&65534u16.to_be_bytes());
    let table=Table::parse_for_proof(&duplicate,&mut 2_000_000).unwrap();
    assert!(table.encoding_glyphs_with_budget(&mut 2_000_000).is_err(),"Repeated SIDs must not authorize an order-dependent glyph");
}

#[test]
fn exact_expert_encoding_never_uses_standard_uppercase_slots() {
    let bytes=fixture("expert-encoding.cff");let table=Table::parse_for_proof(&bytes,&mut 2_000_000).unwrap();
    assert_eq!(table.predefined_encoding(),Some("ExpertEncoding"));
    assert_eq!(table.glyph_index_exact(32),Some(GlyphId(1)));
    assert_eq!(table.glyph_index_exact(65),None,"CFF ExpertEncoding has no Standard A at code65");
}

#[test]
fn source_proof_checks_complete_dictionary_arity_and_budget() {
    fn index(bytes:&[u8])->Vec<u8> {let mut out=vec![0,1,1,1,(bytes.len()+1)as u8];out.extend(bytes);out}
    fn font(extra:&[u8])->Vec<u8> {
        let name=index(b"Strict");
        let start=4+name.len()+5+6+extra.len()+4;
        let mut top=vec![29];top.extend((start as i32).to_be_bytes());top.push(17);top.extend(extra);
        let mut data=vec![1,0,4,4];data.extend(name);data.extend(index(&top));data.extend([0,0,0,0]);data.extend(index(&[14]));data
    }
    let valid=font(&[]);let mut work=1000;
    assert!(Table::parse_for_proof(&valid,&mut work).is_some());
    assert!(Table::parse_for_proof(&valid,&mut 0).is_none());
    for extra in [vec![140,140,140,140,140,12,7],vec![140,12,6],vec![140,12,20],vec![139,16,139,16],vec![140]] {
        let malformed=font(&extra);
        assert!(Table::parse(&malformed).is_some(),"Control must exercise the permissive parser's old acceptance");
        assert!(Table::parse_for_proof(&malformed,&mut 1000).is_none(),"Malformed metric context must not gain source authority: {extra:?}");
    }
}

#[test]
fn source_proof_parses_all_independent_raw_fixtures_with_shared_budget() {
    for name in ["sid.cff","cid.cff","fractional.cff","fractional-2048.cff","custom-encoding.cff","repeated-subroutines.cff"] {
        let bytes=fixture(name);
        assert!(Table::parse_for_proof(&bytes,&mut 2_000_000).is_some(),"{name}");
    }
}

#[test]
fn metadata_and_fractional_widths_match_independent_readers() {
    for font in expected()["fonts"].as_array().unwrap() {
        let bytes=fixture(font["path"].as_str().unwrap());let table=Table::parse(&bytes).unwrap();
        let mut budget=2_000_000;
        let infos=table.glyph_infos(&mut budget).collect::<Result<Vec<_>,_>>().unwrap();
        let wanted=font["glyphs"].as_array().unwrap();assert_eq!(infos.len(),wanted.len());
        for (info,row)in infos.iter().zip(wanted) {
            let gid=row["gid"].as_u64().unwrap()as u16;assert_eq!(info.glyph_id,GlyphId(gid));
            assert_eq!(info.cid,row["cid"].as_u64().map(|v|v as u16));
            assert_eq!(info.name,row["name"].as_str());
            let matrix=[info.matrix.sx,info.matrix.ky,info.matrix.kx,info.matrix.sy,info.matrix.tx,info.matrix.ty];
            for (value,wanted)in matrix.iter().zip(row["matrix"].as_array().unwrap()) {
                assert!((*value-wanted.as_f64().unwrap()as f32).abs()<1e-7,"matrix GID{gid}");
            }
            let width=table.glyph_width_with_budget(GlyphId(gid),&mut budget)
                .unwrap_or_else(||panic!("{} GID{gid} width proof failed, remaining work {budget}",font["path"]));
            assert!((width-row["width"].as_f64().unwrap()as f32).abs()<0.001,"width GID{gid}");
        }
        assert!(budget<2_000_000,"Metadata and metric work must be charged");
    }
}

#[test]
fn zero_budget_is_an_explicit_error_instead_of_empty_success() {
    let bytes=fixture("cid.cff");let table=Table::parse(&bytes).unwrap();let mut budget=0;
    let mut infos=table.glyph_infos(&mut budget);
    assert!(infos.next().unwrap().is_err());assert!(infos.next().is_none());
    let mut budget=0;assert!(table.glyph_width_with_budget(GlyphId(2),&mut budget).is_none());
}

#[test]
fn exact_builtin_encoding_does_not_invent_standard_fallback_codes() {
    let bytes=fixture("custom-encoding.cff");let table=Table::parse(&bytes).unwrap();
    assert_eq!(table.glyph_index_exact(32),Some(GlyphId(1)));
    assert_eq!(table.glyph_index_exact(65),Some(GlyphId(2)));
    assert_eq!(table.glyph_index_exact(66),Some(GlyphId(3)));
    assert_eq!(table.glyph_index_exact(67),None);
    assert_eq!(table.glyph_index(67),Some(GlyphId(4)),"Existing convenience API keeps its documented fallback");
}

#[test]
fn repeated_subroutine_expansion_consumes_shared_work_budget() {
    let bytes=fixture("repeated-subroutines.cff");let table=Table::parse(&bytes).unwrap();
    let mut small=32;assert!(table.glyph_width_with_budget(GlyphId(2),&mut small).is_none());assert_eq!(small,0);
    let mut sufficient=100_000;
    assert_eq!(table.glyph_width_with_budget(GlyphId(2),&mut sufficient),Some(500.75));
    assert!(sufficient<99_900,"A recursion-depth check alone misses repeated expansion");
}

fn dict_integer(value:usize)->Vec<u8> {
    let mut out=vec![29];out.extend_from_slice(&(value as i32).to_be_bytes());out
}
fn index(items:&[Vec<u8>])->Vec<u8> {
    let mut out=(items.len()as u16).to_be_bytes().to_vec();
    if items.is_empty(){return out;}
    out.push(4);let mut offset=1u32;out.extend_from_slice(&offset.to_be_bytes());
    for item in items {offset+=item.len()as u32;out.extend_from_slice(&offset.to_be_bytes());}
    for item in items {out.extend_from_slice(item);}
    out
}
fn largest_range_font()->Vec<u8> {
    const COUNT:usize=65535;
    fn top(charset:usize,strings:usize,array:usize,select:usize)->Vec<u8> {
        let mut data=Vec::new();
        for value in [391,392,0]{data.extend(dict_integer(value));}data.extend([12,30]);
        for(value,op)in[(charset,vec![15]),(strings,vec![17]),(array,vec![12,36]),(select,vec![12,37])]{data.extend(dict_integer(value));data.extend(op);}
        data
    }
    fn font_dict(private:usize)->Vec<u8> {
        let mut data=dict_integer(6);data.extend(dict_integer(private));data.push(18);data
    }
    let header=vec![1,0,4,4];let names=index(&[b"FolioBoundedRanges".to_vec()]);
    let string_names=index(&[b"Adobe".to_vec(),b"Identity".to_vec()]);let global=index(&[]);
    let start=header.len()+names.len()+index(&[top(0,0,0,0)]).len()+string_names.len()+global.len();
    let mut charset=vec![1];
    for gid in 1..COUNT {charset.extend_from_slice(&((COUNT-gid)as u16).to_be_bytes());charset.push(0);}
    let charstrings=index(&vec![vec![14];COUNT]);
    let charset_offset=start;let strings_offset=start+charset.len();
    let array_offset=strings_offset+charstrings.len();let array_len=index(&[font_dict(0),font_dict(0)]).len();
    let select_offset=array_offset+array_len;
    let mut select=vec![3];select.extend_from_slice(&(COUNT as u16).to_be_bytes());
    for gid in 0..COUNT {select.extend_from_slice(&(gid as u16).to_be_bytes());select.push((gid%2)as u8);}
    select.extend_from_slice(&(COUNT as u16).to_be_bytes());
    let private_offset=select_offset+select.len();let array=index(&[font_dict(private_offset),font_dict(private_offset+6)]);
    let mut data=header;data.extend(names);data.extend(index(&[top(charset_offset,strings_offset,array_offset,select_offset)]));data.extend(string_names);data.extend(global);
    data.extend(charset);data.extend(charstrings);data.extend(array);data.extend(select);
    for width in [500,600]{data.extend(dict_integer(width));data.push(20);}
    data
}

#[test]
fn maximum_range_tables_fit_the_shared_work_allowance() {
    let bytes=largest_range_font();let table=Table::parse(&bytes).unwrap();assert_eq!(table.number_of_glyphs(),65535);
    let mut budget=2_000_000;let mut count=0;
    for info in table.glyph_infos(&mut budget) {
        let info=info.unwrap();let gid=info.glyph_id.0;
        assert_eq!(info.cid,Some(if gid==0 {0}else{65535-gid}));count+=1;
    }
    assert_eq!(count,65535);
    for gid in [0,1,32767,65534] {
        assert_eq!(table.glyph_width_with_budget(GlyphId(gid),&mut budget),Some(if gid%2==0 {500.0}else{600.0}));
    }
    assert!(budget>0);
}
