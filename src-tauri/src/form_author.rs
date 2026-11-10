//! Immutable creation of common AcroForm fields, preserving existing references.
use crate::{AnnotationRect, EngineError, EngineResult};
use lopdf::{dictionary, Dictionary, Document, Object, Stream};
use serde::Deserialize;
use std::collections::HashSet;
#[path = "form_radio_author.rs"]
mod radio;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum NewFormField {
    Text { multiline: bool },
    Checkbox,
    Dropdown { options: Vec<String> },
    List { options: Vec<String>, #[serde(rename = "multiSelect")] multi_select: bool },
    Radio { options: Vec<String> },
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateFormFieldRequest {
    pub bounds: AnnotationRect,
    pub rotation: u16,
    pub name: String,
    pub label: String,
    pub required: bool,
    pub font_size: f32,
    pub field: NewFormField,
}
pub(crate) fn bad(message: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidRequest(format!("Create form field: {message}"))
}
fn valid_label(s: &str) -> bool {
    s.len() <= 1024 && s.chars().count() <= 256 && !s.chars().any(char::is_control)
}
impl CreateFormFieldRequest {
    pub(crate) fn validate(&self) -> EngineResult<()> {
        if !valid_label(&self.name) || self.name.trim().is_empty() || self.name.contains('.') || !valid_label(&self.label) {
            return Err(bad("use a nonempty field name without dots or control characters, at most 256 characters; labels have the same length limit"));
        }
        let r=&self.bounds;
        if ![r.x,r.y,r.width,r.height,self.font_size].iter().all(|n|n.is_finite()) || r.x<0. || r.y<0. || r.width<4. || r.height<4. || !(1. ..=300.).contains(&self.font_size) || ![0,90,180,270].contains(&self.rotation) {
            return Err(bad("invalid field bounds, rotation or font size"));
        }
        if let NewFormField::Dropdown{options} | NewFormField::List{options,..} | NewFormField::Radio{options} = &self.field {
            if matches!(self.field,NewFormField::Radio{..}) && !(2..=32).contains(&options.len()) {return Err(bad("provide 2–32 radio choices"));}
            let mut seen=HashSet::new();
            if options.is_empty() || options.len()>256 || options.iter().map(String::len).sum::<usize>()>32768 || options.iter().any(|o|!valid_label(o)||o.trim().is_empty()||!seen.insert(o)) {
                return Err(bad("provide 1–256 unique nonempty choices, at most 32768 UTF-8 bytes total"));
            }
            if options.iter().any(|s|s.chars().any(|c|!(' '..='~').contains(&c)&&!('\u{a0}'..='\u{ff}').contains(&c)&&!"€‚ƒ„…†‡ˆ‰Š‹ŒŽ‘’“”•–—˜™š›œžŸ".contains(c))) {
                return Err(bad("choice labels must use characters supported by Helvetica (Western European text)"));
            }
            if matches!(self.field,NewFormField::Radio{..}) {
                let height=if self.rotation==90||self.rotation==270 {r.width}else{r.height};
                if height/options.len() as f32+0.001<(self.font_size*1.2+4.).max(16.) {return Err(bad("make the radio group taller or reduce the font size so each choice fits"));}
            }
        }
        Ok(())
    }
    pub(crate) fn kind(&self) -> &'static str {
        match self.field {NewFormField::Text{..}=>"text",NewFormField::Checkbox=>"checkbox",NewFormField::Dropdown{..}=>"combo",NewFormField::List{..}=>"list",NewFormField::Radio{..}=>"radio"}
    }
}
fn resolved<'a>(doc: &'a Document, mut value: &'a Object) -> EngineResult<&'a Object> {
    let mut seen=HashSet::new();
    while let Object::Reference(id)=value {
        if seen.len()>=64 || !seen.insert(*id) {return Err(bad("cyclic or deep reference"));}
        value=doc.get_object(*id).map_err(bad)?;
    }
    Ok(value)
}
fn copy_dict(doc:&Document, value:Option<&Object>, budget:&mut crate::form_graph::CopyBudget)->EngineResult<Dictionary>{
    match value {
        None|Some(Object::Null)=>Ok(Dictionary::new()),
        Some(value)=>{let d=resolved(doc,value)?.as_dict().map_err(bad)?;budget.dictionary(d)?;Ok(d.clone())}
    }
}
fn copy_array(doc:&Document,value:Option<&Object>,budget:&mut crate::form_graph::CopyBudget)->EngineResult<Vec<Object>>{
    match value {
        None|Some(Object::Null)=>Ok(vec![]),
        Some(value)=>{let a=resolved(doc,value)?.as_array().map_err(bad)?;budget.array(a)?;Ok(a.clone())}
    }
}
fn appearance(doc:&mut Document,w:f32,h:f32,border:f32,on:bool)->lopdf::ObjectId {
    let content=format!("q 0 G {border} w {} {} {} {} re S {} Q",border/2.,border/2.,w-border,h-border,
        if on {format!("{} {} m {} {} l {} {} l S",w*0.2,h*0.5,w*0.42,h*0.25,w*0.8,h*0.8)}else{String::new()});
    doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","FormType"=>1,"BBox"=>vec![0.into(),0.into(),w.into(),h.into()],"Resources"=>dictionary!{}},content.into_bytes()))
}

fn list_appearance(doc:&mut Document,rect:&AnnotationRect,unit:f32,rotation:u16,font:lopdf::ObjectId,size:f32,options:&[String])->EngineResult<lopdf::ObjectId>{
    let (w,h)=if rotation==90||rotation==270 {(rect.height,rect.width)}else{(rect.width,rect.height)};
    let matrix:Vec<Object>=match rotation {
        90=>vec![0.into(),1.into(),(-1).into(),0.into(),rect.width.into(),0.into()],
        180=>vec![(-1).into(),0.into(),0.into(),(-1).into(),rect.width.into(),rect.height.into()],
        270=>vec![0.into(),(-1).into(),1.into(),0.into(),0.into(),rect.height.into()],
        _=>vec![1.into(),0.into(),0.into(),1.into(),0.into(),0.into()],
    };
    let border=1./unit;let pad=2./unit;let font_size=size/unit;
    let mut data=format!("q 0 G {border} w {} {} {} {} re S {} {} {} {} re W n BT /F0 {font_size} Tf 0 g\n",border/2.,border/2.,w-border,h-border,border,border,(w-2.*border).max(0.),(h-2.*border).max(0.));
    // Explicit WinAnsi byte positions, including undefined slots, avoid treating
    // Unicode option labels as literal UTF-8 glyph codes in a simple font.
    let extended=['€','\0','‚','ƒ','„','…','†','‡','ˆ','‰','Š','‹','Œ','\0','Ž','\0','\0','‘','’','“','”','•','–','—','˜','™','š','›','œ','\0','ž','Ÿ'];
    let mut y=h-pad-font_size*0.9;
    for option in options {
        if y+font_size<border {break;}
        let mut hex=String::new();
        for c in option.chars() {
            let byte=if (' '..='~').contains(&c)||('\u{a0}'..='\u{ff}').contains(&c){c as u8}else{extended.iter().position(|v|*v==c&&c!='\0').map(|i|i as u8+128).ok_or_else(||bad("choice label is outside Helvetica's repertoire"))?};
            use std::fmt::Write;write!(hex,"{byte:02X}").map_err(bad)?;
        }
        data.push_str(&format!("1 0 0 1 {pad} {y} Tm <{hex}> Tj\n"));
        y-=font_size*1.2;
    }
    data.push_str("ET Q");
    Ok(doc.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","FormType"=>1,"BBox"=>vec![0.into(),0.into(),w.into(),h.into()],"Matrix"=>matrix,"Resources"=>dictionary!{"Font"=>dictionary!{"F0"=>font}}},data.into_bytes())))
}

pub(crate) struct CreatedWidget {pub annotation_index:usize,pub bounds:AnnotationRect,pub option_index:Option<usize>}
pub(crate) struct CreatedField {pub bytes:Vec<u8>,pub widgets:Vec<CreatedWidget>}

/// `rect` is raw PDF bottom-left coordinates; all public validation happens in points.
pub(crate) fn create(bytes:&[u8],page_index:usize,request:&CreateFormFieldRequest,rect:&AnnotationRect,unit:f32,rotation:u16)->EngineResult<CreatedField> {
    let (mut doc,names)=crate::acroforms::authoring_document(bytes)?;
    if names.iter().any(|n|n==&request.name||n.starts_with(&format!("{}.",request.name))) {return Err(bad("a field with that name already exists"));}
    let page=*doc.get_pages().values().nth(page_index).ok_or_else(||bad("page outside source"))?;
    let mut budget=crate::form_graph::CopyBudget::default();
    let mut form=copy_dict(&doc,doc.catalog().map_err(bad)?.get(b"AcroForm").ok(),&mut budget)?;
    let mut fields=copy_array(&doc,form.get(b"Fields").ok(),&mut budget)?;
    let mut annots=copy_array(&doc,doc.get_dictionary(page).map_err(bad)?.get(b"Annots").ok(),&mut budget)?;
    let mut resources=copy_dict(&doc,form.get(b"DR").ok(),&mut budget)?;
    let mut fonts=copy_dict(&doc,resources.get(b"Font").ok(),&mut budget)?;
    let font_name=(0..=100000).map(|i|format!("FolioField{i}")).find(|n|!fonts.has(n.as_bytes())).ok_or_else(||bad("too many font resources"))?;
    let font=doc.add_object(dictionary!{"Type"=>"Font","Subtype"=>"Type1","BaseFont"=>"Helvetica","Encoding"=>"WinAnsiEncoding"});
    fonts.set(font_name.clone(),font);let fonts=doc.add_object(fonts);resources.set("Font",fonts);
    let resources=doc.add_object(resources);form.set("DR",resources);
    let off=appearance(&mut doc,rect.width,rect.height,1./unit,false);
    let mut widget=dictionary!{"Type"=>"Annot","Subtype"=>"Widget","T"=>lopdf::text_string(&request.name),"TU"=>lopdf::text_string(&request.label),"P"=>page,"F"=>4,
        "Rect"=>vec![rect.x.into(),rect.y.into(),(rect.x+rect.width).into(),(rect.y+rect.height).into()],
        "MK"=>dictionary!{"R"=>i64::from(rotation),"BC"=>vec![0.into(),0.into(),0.into()]},
        "BS"=>dictionary!{"W"=>1./unit,"S"=>"S"},"AP"=>dictionary!{"N"=>off},
        "DA"=>Object::string_literal(format!("/{font_name} {} Tf 0 g",request.font_size/unit))};
    let mut flags=if request.required {2}else{0};
    let mut created_widgets=vec![];
    match &request.field {
        NewFormField::Text{multiline}=>{widget.set("FT","Tx");widget.set("V",Object::string_literal(""));if *multiline {flags|=1<<12;}}
        NewFormField::Checkbox=>{widget.set("FT","Btn");widget.set("V","Off");widget.set("AS","Off");let on=appearance(&mut doc,rect.width,rect.height,1./unit,true);widget.set("AP",dictionary!{"N"=>dictionary!{"Off"=>off,"Yes"=>on}});}
        NewFormField::Dropdown{options}|NewFormField::List{options,..}=>{
            widget.set("FT","Ch");widget.set("Opt",options.iter().map(|s|lopdf::text_string(s)).collect::<Vec<_>>());
            if matches!(request.field,NewFormField::Dropdown{..}) {flags|=1<<17;}
            if matches!(request.field,NewFormField::List{multi_select:true,..}) {flags|=1<<21;}
            if matches!(request.field,NewFormField::List{..}) {
                let ap=list_appearance(&mut doc,rect,unit,rotation,font,request.font_size,options)?;
                widget.set("AP",dictionary!{"N"=>ap});
            }
        }
        NewFormField::Radio{options}=>{
            let parent=radio::create(&mut doc,page,request,rect,unit,rotation,font,options,&mut annots,&mut created_widgets)?;
            fields.push(parent.into());
        }
    }
    widget.set("Ff",flags);
    if !matches!(request.field,NewFormField::Radio{..}) {
        created_widgets.push(CreatedWidget{annotation_index:annots.len(),bounds:rect.clone(),option_index:None});
        let widget=doc.add_object(widget);fields.push(widget.into());annots.push(widget.into());
    }
    form.set("Fields",fields);let form=doc.add_object(form);doc.catalog_mut().map_err(bad)?.set("AcroForm",form);
    doc.get_dictionary_mut(page).map_err(bad)?.set("Annots",annots);
    crate::page_resources::prune_unreachable(&mut doc)?;
    let output=crate::pdf_output::save(&mut doc)?;
    // Include the new widget in the same graph limits used by filling/export.
    crate::acroforms::authoring_document(&output)?;
    Ok(CreatedField{bytes:output,widgets:created_widgets})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn blank() -> Document {
        let mut doc=Document::with_version("1.7");let pages=doc.new_object_id();
        let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()]});
        doc.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1}.into());
        let root=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});doc.trailer.set("Root",root);doc
    }
    #[test]
    fn review_shared_indirect_names_have_an_expansion_budget() {
        let mut doc=blank();let text=doc.add_object(Object::string_literal(vec![b'a';128*1024]));let mut roots=vec![];
        for i in 0..24 {
            let parent=doc.new_object_id();let child=doc.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","FT"=>"Tx","Parent"=>parent,"T"=>text});
            doc.objects.insert(parent,dictionary!{"T"=>Object::string_literal(format!("p{i}")),"Kids"=>vec![Object::Reference(child)]}.into());roots.push(Object::Reference(parent));
        }
        let form=doc.add_object(dictionary!{"Fields"=>roots});doc.catalog_mut().unwrap().set("AcroForm",form);
        let bytes=crate::pdf_output::save(&mut doc).unwrap();
        match crate::acroforms::authoring_document(&bytes) {
            Err(e)=>assert!(e.to_string().contains("name budget"),"{e}"),
            Ok(_)=>panic!("shared indirect names expanded beyond the authoring name budget"),
        }
    }
    #[test]
    fn review_resolved_field_metadata_is_charged_before_node_cloning() {
        let mut doc=blank();let large=doc.add_object(Object::Name(vec![b'a';100_000]));let mut roots=vec![];
        for i in 0..200 {let id=doc.add_object(dictionary!{"Type"=>large,"Subtype"=>"Widget","FT"=>"Tx","T"=>Object::string_literal(format!("field{i}"))});roots.push(Object::Reference(id));}
        let form=doc.add_object(dictionary!{"Fields"=>roots});doc.catalog_mut().unwrap().set("AcroForm",form);
        let bytes=crate::pdf_output::save(&mut doc).unwrap();
        assert!(crate::acroforms::authoring_document(&bytes).is_err(),"resolved indirect metadata escaped the copy budget");
    }
    #[test]
    fn review_unfilled_list_appearance_contains_clipped_option_text() {
        let mut doc=blank();let bytes=crate::pdf_output::save(&mut doc).unwrap();
        let rect=AnnotationRect{x:10.,y:20.,width:100.,height:60.};
        let request=CreateFormFieldRequest{bounds:rect.clone(),rotation:0,name:"list".into(),label:String::new(),required:false,font_size:12.,field:NewFormField::List{options:vec!["One".into(),"Two".into()],multi_select:false}};
        let bytes=create(&bytes,0,&request,&rect,1.,0).unwrap().bytes;let doc=Document::load_mem(&bytes).unwrap();
        let page=doc.get_dictionary(*doc.get_pages().get(&1).unwrap()).unwrap();let widget=doc.get_dictionary(page.get(b"Annots").unwrap().as_array().unwrap()[0].as_reference().unwrap()).unwrap();
        let ap=doc.get_object(widget.get(b"AP").unwrap().as_dict().unwrap().get(b"N").unwrap().as_reference().unwrap()).unwrap().as_stream().unwrap();
        let content=lopdf::content::Content::decode(&ap.get_plain_content().unwrap()).unwrap();
        assert_eq!(content.operations.iter().filter(|op|op.operator=="Tj").count(),2,"unfilled list must paint its two rows");
        assert!(content.operations.iter().any(|op|op.operator=="W"));
    }
    #[test]
    fn repeated_creation_does_not_retain_obsolete_form_roots() {
        let mut doc=Document::with_version("1.7");let pages=doc.new_object_id();
        let page=doc.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,"MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()]});
        doc.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![Object::Reference(page)],"Count"=>1}.into());
        let root=doc.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});doc.trailer.set("Root",root);
        let mut bytes=crate::pdf_output::save(&mut doc).unwrap();
        let rect=AnnotationRect{x:10.,y:20.,width:100.,height:30.};
        for i in 0..8 {
            let request=CreateFormFieldRequest{bounds:rect.clone(),rotation:0,name:format!("field{i}"),label:String::new(),required:false,font_size:12.,field:NewFormField::Text{multiline:false}};
            bytes=create(&bytes,0,&request,&rect,1.,0).unwrap().bytes;
        }
        let mut doc=Document::load_mem(&bytes).unwrap();let before=doc.objects.clone();
        crate::page_resources::prune_unreachable(&mut doc).unwrap();
        // Saving inserts an xref stream, which is reached from startxref rather
        // than the catalog graph. It is the only permissible removed object.
        for (id,value) in before {
            if !doc.objects.contains_key(&id) {
                assert!(value.as_stream().is_ok_and(|s|s.dict.get(b"Type").and_then(Object::as_name).is_ok_and(|n|n==b"XRef")),"creation retained obsolete object {id:?}: {value:?}");
            }
        }
    }
}
