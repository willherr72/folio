//! A complete radio group with explicit, physically sized on/off appearances.
use super::*;

fn oriented_size(rect:&AnnotationRect,rotation:u16)->(f32,f32){
    if rotation==90||rotation==270 {(rect.height,rect.width)}else{(rect.width,rect.height)}
}
fn point(x:f32,y:f32,rect:&AnnotationRect,rotation:u16)->(f32,f32){
    let (x,y)=match rotation {90=>(rect.width-y,x),180=>(rect.width-x,rect.height-y),270=>(y,rect.height-x),_=>(x,y)};
    (rect.x+x,rect.y+y)
}
fn circle(x:f32,y:f32,r:f32)->String {
    let k=r*0.5522848;
    format!("{} {y} m {} {} {} {} {x} {} c {} {} {} {} {} {y} c {} {} {} {} {x} {} c {} {} {} {} {} {y} c",
        x+r,x+r,y+k,x+k,y+r,y+r,x-k,y+r,x-r,y+k,x-r,x-r,y-k,x-k,y-r,y-r,x+k,y-r,x+r,y-k,x+r)
}
fn label_hex(label:&str)->EngineResult<String>{
    let extended=['€','\0','‚','ƒ','„','…','†','‡','ˆ','‰','Š','‹','Œ','\0','Ž','\0','\0','‘','’','“','”','•','–','—','˜','™','š','›','œ','\0','ž','Ÿ'];
    let mut result=String::new();
    for c in label.chars(){
        let byte=if (' '..='~').contains(&c)||('\u{a0}'..='\u{ff}').contains(&c){c as u8}else{extended.iter().position(|v|*v==c&&c!='\0').map(|i|i as u8+128).ok_or_else(||bad("radio label is outside Helvetica's repertoire"))?};
        use std::fmt::Write;write!(result,"{byte:02X}").map_err(bad)?;
    }
    Ok(result)
}
fn appearance(doc:&mut Document,rect:&AnnotationRect,unit:f32,rotation:u16,font:lopdf::ObjectId,size:f32,label:&str,on:bool)->EngineResult<lopdf::ObjectId>{
    let (w,h)=oriented_size(rect,rotation);
    let matrix:Vec<Object>=match rotation {
        90=>vec![0.into(),1.into(),(-1).into(),0.into(),rect.width.into(),0.into()],
        180=>vec![(-1).into(),0.into(),0.into(),(-1).into(),rect.width.into(),rect.height.into()],
        270=>vec![0.into(),(-1).into(),1.into(),0.into(),0.into(),rect.height.into()],
        _=>vec![1.into(),0.into(),0.into(),1.into(),0.into(),0.into()],
    };
    let cx=8./unit;let cy=h/2.;let font_size=size/unit;
    let mut content=format!("q 0 0 {w} {h} re W n 0 G 0 g {} w {} S ",1./unit,circle(cx,cy,5./unit));
    if on {content.push_str(&format!("{} f ",circle(cx,cy,2.5/unit)));}
    // Adobe Helvetica AFM FontBBox spans -225..931 per 1000 em. Center
    // that complete vertical extent, including capitals' accents/descenders.
    // Its 1.156 em height fits the admitted 1.2 em + 4 pt row without clipping.
    let baseline=cy-font_size*(0.931-0.225)/2.;
    content.push_str(&format!("BT /F0 {font_size} Tf 1 0 0 1 {} {baseline} Tm <{}> Tj ET Q",18./unit,label_hex(label)?));
    Ok(doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","FormType"=>1,"BBox"=>vec![0.into(),0.into(),w.into(),h.into()],"Matrix"=>matrix,"Resources"=>dictionary!{"Font"=>dictionary!{"F0"=>font}}},content.into_bytes())))
}
pub(super) fn create(doc:&mut Document,page:lopdf::ObjectId,request:&CreateFormFieldRequest,rect:&AnnotationRect,unit:f32,rotation:u16,font:lopdf::ObjectId,options:&[String],annots:&mut Vec<Object>,created:&mut Vec<CreatedWidget>)->EngineResult<lopdf::ObjectId>{
    let parent=doc.new_object_id();let (w,h)=oriented_size(rect,rotation);let row=h/options.len() as f32;let mut kids=vec![];
    for (i,label) in options.iter().enumerate(){
        let y=h-(i+1) as f32*row;
        let (x1,y1)=point(0.,y,rect,rotation);let (x2,y2)=point(w,y+row,rect,rotation);
        let bounds=AnnotationRect{x:x1.min(x2),y:y1.min(y2),width:(x2-x1).abs(),height:(y2-y1).abs()};
        let off=appearance(doc,&bounds,unit,rotation,font,request.font_size,label,false)?;
        let on=appearance(doc,&bounds,unit,rotation,font,request.font_size,label,true)?;
        let mut states=dictionary!{"Off"=>off};states.set(i.to_string(),on);
        let widget=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","Parent"=>parent,"P"=>page,"F"=>4,"AS"=>"Off",
            "Rect"=>vec![bounds.x.into(),bounds.y.into(),(bounds.x+bounds.width).into(),(bounds.y+bounds.height).into()],
            "MK"=>dictionary!{"R"=>i64::from(rotation)},"AP"=>dictionary!{"N"=>states}});
        created.push(CreatedWidget{annotation_index:annots.len(),bounds,option_index:Some(i)});
        annots.push(widget.into());kids.push(widget.into());
    }
    doc.objects.insert(parent,dictionary!{"FT"=>"Btn","T"=>lopdf::text_string(&request.name),"TU"=>lopdf::text_string(&request.label),"Ff"=>(1<<15)|(1<<14)|if request.required{2}else{0},
        "V"=>"Off","Opt"=>options.iter().map(|s|lopdf::text_string(s)).collect::<Vec<_>>(),"Kids"=>kids}.into());
    Ok(parent)
}
