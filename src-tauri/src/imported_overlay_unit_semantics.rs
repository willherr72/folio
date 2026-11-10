//! Hash exactly the raw -> public points -> raw echo without copying payloads.
//! This is derived from a freshly proved source; it is never recovery authority.
use super::*;
use serde::ser::{SerializeSeq,Error};
#[derive(Clone,Copy)]
struct Echo(f32);
impl Echo {
    fn value(self,raw:f32,endpoints:&[f32])->Option<f32>{
        if self.0==1. {return raw.is_finite().then_some(raw);}
        let physical=endpoints.iter().copied().find(|v|raw==*v*self.0.recip()).unwrap_or(raw*self.0);
        let value=physical*self.0.recip();value.is_finite().then_some(value)
    }
}
struct Points<'a>(&'a [Point],Echo);
impl Serialize for Points<'_>{
    fn serialize<S:serde::Serializer>(&self,s:S)->Result<S::Ok,S::Error>{
        let mut seq=s.serialize_seq(Some(self.0.len()))?;
        for p in self.0{seq.serialize_element(&Point{x:self.1.value(p.x,&[]).ok_or_else(||S::Error::custom("physical overflow"))?,y:self.1.value(p.y,&[]).ok_or_else(||S::Error::custom("physical overflow"))?})?;}
        seq.end()
    }
}
struct Paths<'a>(&'a [Vec<Point>],Echo);
impl Serialize for Paths<'_>{
    fn serialize<S:serde::Serializer>(&self,s:S)->Result<S::Ok,S::Error>{let mut seq=s.serialize_seq(Some(self.0.len()))?;for p in self.0{seq.serialize_element(&Points(p,self.1))?;}seq.end()}
}
struct Rects<'a>(&'a [AnnotationRect],Echo);
impl Serialize for Rects<'_>{
    fn serialize<S:serde::Serializer>(&self,s:S)->Result<S::Ok,S::Error>{
        let mut seq=s.serialize_seq(Some(self.0.len()))?;
        for r in self.0{
            let value=|v|self.1.value(v,&[]).ok_or_else(||S::Error::custom("physical overflow"));
            seq.serialize_element(&AnnotationRect{x:value(r.x)?,y:value(r.y)?,width:value(r.width)?,height:value(r.height)?})?;
        }seq.end()
    }
}
pub(super) fn semantics(overlay:&Overlay,unit:f32,limit:usize)->Option<(String,usize)>{
    if !unit.is_finite() || unit<=0. || unit>75_000. || !unit.recip().is_finite(){return None;}
    let echo=Echo(unit);let value=|v|echo.value(v,&[]);
    let mut sink=SemanticHash{hash:Sha256::new(),used:0,limit};
    match overlay {
        Overlay::Text(v)=>{
            let mut area=v.area.clone(); // Fixed-sized settings, no text/path/asset copy.
            if let Some(a)=&mut area{a.width=echo.value(a.width,&[14400.])?;a.height=echo.value(a.height,&[14400.])?;a.inset=value(a.inset)?;}
            serde_json::to_writer(&mut sink,&("text",value(v.x)?,value(v.y)?,&v.text,echo.value(v.font_size,&[1.,512.])?,v.font_name,&v.font_id,&v.shaping,&area,&v.color,v.rotation))
        },
        Overlay::Image(v)=>serde_json::to_writer(&mut sink,&("image",&v.asset_id,value(v.x)?,value(v.y)?,value(v.width)?,value(v.height)?,v.rotation)),
        Overlay::Ink(v)=>serde_json::to_writer(&mut sink,&("ink",Paths(&v.paths,echo),&v.color,echo.value(v.stroke_width,&[100.])?)),
        Overlay::Highlight(v)=>serde_json::to_writer(&mut sink,&("highlight",Rects(&v.rects,echo),&v.color,&v.text,v.opacity)),
        Overlay::Comment(v)=>serde_json::to_writer(&mut sink,&("comment",value(v.x)?,value(v.y)?,&v.text,&v.color)),
    }.ok()?;
    Some((hex(sink.hash.finalize()),sink.used))
}
