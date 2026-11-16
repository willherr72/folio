//! Launch-owned authentication data; never serialized in operation results.
use crate::private_storage::Sensitive;
use std::{io::Read,sync::Arc};
pub(crate) const MAX_BYTES:u64=128*1024;
const BAD:&str="Credentials must be a bounded JSON array matching input slots, with null or single-line UTF-8 passwords of at most 1024 bytes.";
#[derive(Clone)]
pub(crate) struct Credentials{entries:Vec<Option<Arc<Sensitive>>>}
impl Credentials{
 pub fn none(count:usize)->Self{Self{entries:vec![None;count]}}
 pub fn read(reader:impl Read,count:usize)->Result<Self,&'static str>{
  if !(1..=64).contains(&count){return Err(BAD);}
  let bytes=read_sensitive(reader,MAX_BYTES as usize).map_err(|_|BAD)?;
  let mut parser=Parser{bytes:&bytes.0,position:0};
  parser.expect(b'[')?;
  let mut entries=Vec::new();
  parser.whitespace();
  if parser.peek()!=Some(b']'){
   loop{
    if entries.len()==64{return Err(BAD);}
    parser.whitespace();
    let secret=if parser.peek()==Some(b'"'){Some(Arc::new(parser.string()?))}else{
     if parser.bytes.get(parser.position..parser.position+4)!=Some(b"null"){return Err(BAD);}
     parser.position+=4;None
    };
    entries.push(secret);parser.whitespace();
    if parser.peek()!=Some(b','){break;}parser.position+=1;
   }
  }
  parser.expect(b']')?;parser.whitespace();
  if parser.position!=parser.bytes.len()||entries.len()!=count{return Err(BAD);}
  Ok(Self{entries})
 }
 pub fn len(&self)->usize{self.entries.len()}
 pub fn get(&self,index:usize)->Option<&str>{self.entries.get(index).and_then(Option::as_ref).map(|v|std::str::from_utf8(&v.0).expect("validated credential"))}
 pub fn secret(&self,index:usize)->Option<Arc<Sensitive>>{self.entries.get(index).cloned().flatten()}
}

// Allocate the entire bound before reading plaintext. No growth/reallocation or
// unowned decoder scratch may release an earlier plaintext allocation.
pub(crate) fn read_sensitive(mut reader:impl Read,limit:usize)->Result<Sensitive,std::io::Error>{
 let mut bytes=Sensitive(vec![0;limit+1]);let mut used=0;
 while used<bytes.0.len(){
  let n=match reader.read(&mut bytes.0[used..]){Err(e)if e.kind()==std::io::ErrorKind::Interrupted=>continue,result=>result?};
  if n==0{break;}used+=n;
 }
 if used>limit{return Err(std::io::Error::new(std::io::ErrorKind::InvalidData,"Credential input exceeds its byte limit."));}
 // Unused initialized bytes are zero; truncation cannot leave secret capacity.
 bytes.0.truncate(used);Ok(bytes)
}

pub(crate) fn encode(values:&[Option<&str>])->Result<Sensitive,&'static str>{
 let mut bytes=Sensitive(vec![0;MAX_BYTES as usize]);
 let used={let mut cursor=std::io::Cursor::new(bytes.0.as_mut_slice());
  serde_json::to_writer(&mut cursor,values).map_err(|_|"Cannot encode private composition credentials within their byte limit.")?;
  cursor.position()as usize
 };
 bytes.0.truncate(used);Ok(bytes)
}

// This deliberately small JSON grammar avoids serde's ordinary String and
// unescape scratch allocations. Every decoded byte has zeroing ownership even
// when a later entry, closing delimiter, or slot-count check fails.
struct Parser<'a>{bytes:&'a[u8],position:usize}
impl Parser<'_>{
 fn peek(&self)->Option<u8>{self.bytes.get(self.position).copied()}
 fn whitespace(&mut self){while matches!(self.peek(),Some(b' '|b'\t'|b'\r'|b'\n')){self.position+=1;}}
 fn expect(&mut self,value:u8)->Result<(),&'static str>{self.whitespace();if self.peek()!=Some(value){return Err(BAD);}self.position+=1;Ok(())}
 fn next(&mut self)->Result<u8,&'static str>{let byte=self.peek().ok_or(BAD)?;self.position+=1;Ok(byte)}
 fn hex(&mut self)->Result<u16,&'static str>{
  let mut value=0;for _ in 0..4{let byte=self.next()?;let digit=match byte{b'0'..=b'9'=>byte-b'0',b'a'..=b'f'=>byte-b'a'+10,b'A'..=b'F'=>byte-b'A'+10,_=>return Err(BAD)};value=value*16+digit as u16;}Ok(value)
 }
 fn append(secret:&mut Sensitive,bytes:&[u8])->Result<(),&'static str>{
  if secret.0.len()+bytes.len()>1024||bytes.iter().any(|v|matches!(*v,0|b'\r'|b'\n')){return Err(BAD);}
  secret.0.extend_from_slice(bytes);Ok(())
 }
 fn string(&mut self)->Result<Sensitive,&'static str>{
  self.expect(b'"')?;let mut secret=Sensitive(Vec::with_capacity(1024));let mut scratch=Sensitive(vec![0;4]);
  loop{
   match self.next()?{
    b'"'=>{std::str::from_utf8(&secret.0).map_err(|_|BAD)?;return Ok(secret);}
    b'\\'=>match self.next()?{
     byte @ (b'"'|b'\\'|b'/')=>Self::append(&mut secret,&[byte])?,
     b'b'=>Self::append(&mut secret,&[8])?,b'f'=>Self::append(&mut secret,&[12])?,
     b'n'=>Self::append(&mut secret,b"\n")?,b'r'=>Self::append(&mut secret,b"\r")?,b't'=>Self::append(&mut secret,b"\t")?,
     b'u'=>{
      let first=self.hex()?;
      let scalar=if (0xd800..=0xdbff).contains(&first){
       if self.next()!=Ok(b'\\')||self.next()!=Ok(b'u'){return Err(BAD);}
       let second=self.hex()?;if !(0xdc00..=0xdfff).contains(&second){return Err(BAD);}
       0x10000+((first as u32-0xd800)<<10)+(second as u32-0xdc00)
      }else{first as u32};
      let value=char::from_u32(scalar).ok_or(BAD)?;
      let buffer:&mut[u8;4]=scratch.0.as_mut_slice().try_into().unwrap();
      Self::append(&mut secret,value.encode_utf8(buffer).as_bytes())?;
     }
     _=>return Err(BAD)
    },
    0..=31=>return Err(BAD),
    byte=>Self::append(&mut secret,&[byte])?
   }
  }
 }
}

#[cfg(test)]mod tests{
 use super::*;
 #[test]fn credentials_decode_standard_json_escapes_and_reject_malformed_unicode(){
  for value in ["", "quoted\" slash/ reverse\\ tab\t", "café", "\u{1f680}", "\u{8}\u{c}", "é"].into_iter().chain(["a"].into_iter()){
   let encoded=serde_json::to_vec(&[Some(value),None]).unwrap();let parsed=Credentials::read(encoded.as_slice(),2).unwrap();assert_eq!(parsed.get(0),Some(value));assert_eq!(parsed.get(1),None);
  }
  let escaped=br#"["\u0061\u00e9\ud83d\ude80\t\b\f\/\"\\", null]"#;
  let reference:Vec<Option<String>>=serde_json::from_slice(escaped).unwrap();let parsed=Credentials::read(escaped.as_slice(),2).unwrap();assert_eq!(parsed.get(0),reference[0].as_deref());
  for bad in [br#"["\ud800"]"#.as_slice(),br#"["\udc00"]"#,br#"["\ud800\u0061"]"#,br#"["\u00xz"]"#,br#"["\q"]"#,br#"[null,]"#,br#"[,null]"#,br#"[null]true"#,br#"[nullnull]"#,br#"["unterminated]"#,b"[\"\xff\"]",b"[\"raw\tcontrol\"]"]{assert!(Credentials::read(bad,1).is_err());}
 }
 #[test]fn credentials_match_json_string_corpus_without_unowned_decode_storage(){
  let alphabet=['a','é','\u{1f680}','"','\\','/','\t','\u{8}','\u{c}','\n','\r','\0'];
  for a in alphabet{for b in alphabet{
   let text=format!("{a}{b}");let encoded=serde_json::to_vec(&[&text]).unwrap();let got=Credentials::read(encoded.as_slice(),1);
   if text.bytes().any(|v|matches!(v,0|b'\n'|b'\r')){assert!(got.is_err());}else{assert_eq!(got.unwrap().get(0),Some(text.as_str()));}
  }}
  let exact=format!("[\"{}\"]","\\u00e9".repeat(512));assert!(Credentials::read(exact.as_bytes(),1).is_ok());
  let over=format!("[\"{}\"]","\\u00e9".repeat(513));assert!(Credentials::read(over.as_bytes(),1).is_err());
  let exact_slots=serde_json::to_vec(&vec![None::<&str>;64]).unwrap();assert_eq!(Credentials::read(exact_slots.as_slice(),64).unwrap().len(),64);
  let over_slots=serde_json::to_vec(&vec![None::<&str>;65]).unwrap();assert!(Credentials::read(over_slots.as_slice(),64).is_err());
 }
 #[test]fn credential_buffers_handle_short_reads_interruption_and_encoding_limits(){
  struct Short<'a>{bytes:&'a[u8],interrupted:bool}
  impl Read for Short<'_>{fn read(&mut self,out:&mut[u8])->std::io::Result<usize>{
   if !self.interrupted{self.interrupted=true;return Err(std::io::ErrorKind::Interrupted.into());}
   if self.bytes.is_empty(){return Ok(0);}out[0]=self.bytes[0];self.bytes=&self.bytes[1..];Ok(1)
  }}
  assert_eq!(Credentials::read(Short{bytes:br#"["secret"]"#,interrupted:false},1).unwrap().get(0),Some("secret"));
  let values=[Some("quoted\" / café\t"),None,Some("")];let encoded=encode(&values).unwrap();let decoded=Credentials::read(encoded.0.as_slice(),3).unwrap();assert_eq!(decoded.get(0),values[0]);
  let worst="\u{1}".repeat(1024);assert!(encode(&vec![Some(worst.as_str());64]).is_err());
  let mut exact=vec![b' ';MAX_BYTES as usize];exact[..6].copy_from_slice(b"[null]");assert!(Credentials::read(exact.as_slice(),1).is_ok());exact.push(b' ');assert!(Credentials::read(exact.as_slice(),1).is_err());
 }
 #[test]fn credentials_accept_unicode_null_and_empty_but_bound_shape_and_bytes(){
  let c=Credentials::read("[null,\"\",\"café\"]".as_bytes(),3).unwrap();assert_eq!(c.get(0),None);assert_eq!(c.get(1),Some(""));assert_eq!(c.get(2),Some("café"));
  let secret=c.secret(2).unwrap();assert_eq!(secret.0,"café".as_bytes());
  for bad in["[]","[1]","[true]","{}","[\"bad\\nline\"]","[\"bad\\u0000line\"]","[null,null]"]{assert!(Credentials::read(bad.as_bytes(),1).is_err());}
  let exact=serde_json::to_vec(&vec!["é".repeat(512)]).unwrap();assert!(Credentials::read(exact.as_slice(),1).is_ok());
  let over=serde_json::to_vec(&vec!["é".repeat(513)]).unwrap();assert!(Credentials::read(over.as_slice(),1).is_err());
  assert!(Credentials::read(vec![b' ';MAX_BYTES as usize+1].as_slice(),1).is_err());
 }
}
