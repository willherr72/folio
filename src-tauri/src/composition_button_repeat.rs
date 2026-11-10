//! Shared-answer copies use equal named states, not new positional choices.
use super::*;
const RADIO:i64=1<<15;
const UNISON:i64=1<<25;

/// Only the occurrence caller uses this adapter. Selection retains positional
/// export tables. Original controls must be the unchanged prefix of `controls`.
pub(crate) fn repeat(doc:&Document,forms:&FormInfo,field:ObjectId,controls:&[(ObjectId,ObjectId)],budget:&mut Budget,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Patches>{
    let mut w=Walk{doc,budget,check,patches:BTreeMap::new()};w.step(0)?;
    let node=forms.nodes.get(&field).ok_or_else(||bad("missing repeated field"))?;
    if node.kind!=Some(FieldType::Button){return Ok(w.patches);}
    let flags=match w.attr(forms,field,b"Ff")?{None|Some(Object::Null)=>0,Some(Object::Integer(n))=>*n,_=>return Err(bad("invalid repeated button flags"))};
    require((0..=u32::MAX as i64).contains(&flags),"invalid repeated button flags")?;
    if flags&65536!=0{return Ok(w.patches);}
    let options=match w.attr(forms,field,b"Opt")?{None|Some(Object::Null)=>None,Some(v)=>Some(v.as_array().map_err(bad)?)};
    // Named checkboxes and already-unison radios already share each copied
    // state. Their original equivalence classes require no conversion.
    if options.is_none() && (flags&RADIO==0 || flags&UNISON!=0){return Ok(w.patches);}
    if options.is_some(){
        // PDF null is equivalent to an absent dictionary entry. Do not rely on
        // a reader treating null as a barrier to an ancestor's positional Opt.
        // A future inherited-table adapter must materialize unaffected siblings
        // before retiring that ancestor table.
        let mut ancestor=node.parent;
        while let Some(id)=ancestor{
            w.step(0)?;let dict=doc.get_dictionary(id).map_err(bad)?;
            require(w.optional(dict,b"Opt")?.is_none(),"inherited indexed button exports need a faithful ancestor adapter before duplication")?;
            ancestor=forms.nodes.get(&id).ok_or_else(||bad("missing button ancestor"))?.parent;
        }
    }
    let originals=&node.children;
    require(node.field && node.widget_page.is_none() && !originals.is_empty() && controls.len()>originals.len() && controls.len()<=4000,"invalid repeated button controls")?;
    if let Some(options)=options{require(options.len()==originals.len(),"button export table does not match controls")?;}
    let mut indices=BTreeMap::new();let mut names=Vec::new();let mut states=Vec::new();let mut checked=Vec::new();let mut distinct=BTreeSet::new();
    for (i,&widget) in originals.iter().enumerate(){
        w.step(0)?;w.budget.reserve(256)?;
        require(controls[i]==(widget,widget) && forms.nodes.get(&widget).is_some_and(|n|!n.field && n.owner==field && n.widget_page.is_some()),"original button ownership changed")?;
        require(indices.insert(widget,i).is_none(),"repeated original button")?;
        let old=w.on_state(widget)?;
        let name=if let Some(options)=options{
            require(old==i.to_string().as_bytes(),"indexed button state does not match original position")?;
            let raw=w.resolve(&options[i])?.as_str().map_err(bad)?;
            w.budget.reserve(raw.len().checked_mul(6).and_then(|n|n.checked_add(512)).ok_or_else(||bad("export name overflow"))?)?;
            let text=crate::form_properties::text(doc,&options[i],raw.len().saturating_mul(3))?;
            w.export_name(&text)?
        }else{w.budget.reserve(old.len())?;old.clone()};
        require(distinct.insert(name.clone()),"repeated button exports need distinct original choices")?;
        let widget_dict=doc.get_dictionary(widget).map_err(bad)?;
        let state=w.optional(widget_dict,b"AS")?.map(Object::as_name).transpose().map_err(bad)?;
        require(state.map_or(true,|s|s==b"Off" || s==old),"button appearance state is inconsistent")?;
        checked.push(state==Some(old.as_slice()));states.push(old);names.push(name);
    }
    // Converting originally distinct controls must not merge two choices or
    // reinterpret an inconsistent saved/default answer.
    let mut selected=None;
    for key in [b"V".as_slice(),b"DV"]{
        let value=w.attr(forms,field,key)?;
        let name=match value{None|Some(Object::Null)=>None,Some(value)=>Some(value.as_name().map_err(bad)?)};
        let index=match name{None|Some(b"Off")=>None,Some(name)=>Some(states.iter().position(|state|state==name).ok_or_else(||bad("button value or default has no original control"))?)};
        if key==b"V"{selected=index;}
        if options.is_some(){if let Some(i)=index{w.budget.reserve(names[i].len())?;w.put(field,key,Object::Name(names[i].clone()))?;}}
    }
    require(checked.iter().enumerate().all(|(i,on)|*on==(selected==Some(i))),"saved button answer disagrees with its checked controls")?;
    let mut outputs=BTreeSet::new();
    for &(original,output) in controls{
        w.step(0)?;w.budget.reserve(64)?;require(output.0!=0 && outputs.insert(output),"invalid repeated button output")?;
        let &i=indices.get(&original).ok_or_else(||bad("copy is not owned by this button field"))?;
        if options.is_some(){
            let ap=w.named_appearance(original,&states[i],&names[i])?;w.put(output,b"AP",ap)?;
            if checked[i]{w.budget.reserve(names[i].len())?;w.put(output,b"AS",Object::Name(names[i].clone()))?;}
        }
    }
    // There is no ancestor table to inherit; null retires only this local Opt.
    if options.is_some(){w.put(field,b"Opt",Object::Null)?;}
    if flags&RADIO!=0 && flags&UNISON==0{w.put(field,b"Ff",Object::Integer(flags|UNISON))?;}
    w.step(0)?;Ok(w.patches)
}

impl Walk<'_,'_>{
    fn export_name(&mut self,text:&str)->EngineResult<Vec<u8>>{
        require(!text.is_empty() && text!="Off" && text.chars().count()<=127 && !text.chars().any(char::is_control),"button export cannot be represented as a shared named state")?;
        // A fixed 256-entry table from the parser's public text-string decoder
        // avoids font encodings and never substitutes unrepresentable text.
        static TABLE:std::sync::OnceLock<[Option<char>;256]>=std::sync::OnceLock::new();
        let table=TABLE.get_or_init(||std::array::from_fn(|i|lopdf::decode_text_string(&Object::string_literal(vec![i as u8])).ok().and_then(|s|{let mut c=s.chars();let first=c.next();if c.next().is_none(){first}else{None}})));
        self.budget.reserve(512)?;let mut name=Vec::new();
        for c in text.chars(){self.step(0)?;name.push(table.iter().position(|entry|*entry==Some(c)).ok_or_else(||bad("button export needs Unicode positional encoding; shared copies are unsupported"))? as u8);}
        require(lopdf::decode_text_string(&Object::string_literal(name.clone())).map_err(bad)?==text,"button name does not round-trip losslessly")?;
        Ok(name)
    }
    fn on_state(&mut self,widget:ObjectId)->EngineResult<Vec<u8>>{
        let dict=self.doc.get_dictionary(widget).map_err(bad)?;
        let ap=self.optional(dict,b"AP")?.ok_or_else(||bad("repeated button lacks an appearance"))?.as_dict().map_err(bad)?;
        let normal=self.optional(ap,b"N")?.ok_or_else(||bad("repeated button lacks normal states"))?.as_dict().map_err(bad)?;
        require(normal.len()==2 && normal.has(b"Off"),"repeated button needs exactly one on state and Off")?;
        let mut name=None;
        for (key,value) in normal.iter(){self.step(0)?;self.budget.reserve(key.len()+64)?;require(matches!(self.resolve(value)?,Object::Stream(_)),"button state is not a stream")?;if key!=b"Off"{require(!key.is_empty() && key.len()<=127,"invalid button state name")?;name=Some(key.clone());}}
        name.ok_or_else(||bad("missing button on state"))
    }
    fn named_appearance(&mut self,widget:ObjectId,old:&[u8],new:&[u8])->EngineResult<Object>{
        let dict=self.doc.get_dictionary(widget).map_err(bad)?;
        let ap=self.optional(dict,b"AP")?.ok_or_else(||bad("missing button appearance"))?.as_dict().map_err(bad)?;let mut out=Dictionary::new();
        for (key,value) in ap.iter(){
            self.step(0)?;self.budget.reserve(key.len()+96)?;let resolved=self.resolve(value)?;
            if matches!(resolved,Object::Null){out.set(key.clone(),self.copy(value)?);continue;}
            require([b"N".as_slice(),b"R",b"D"].contains(&key.as_slice()),"unknown button appearance role")?;
            if let Object::Dictionary(states)=resolved{
                let mut renamed=Dictionary::new();
                for (name,value) in states.iter(){self.step(0)?;self.budget.reserve(name.len()+new.len()+96)?;require(name==b"Off" || name==old,"unsupported button appearance state")?;require(matches!(self.resolve(value)?,Object::Stream(_)),"button state is not a stream")?;renamed.set(if name==old{new.to_vec()}else{name.clone()},self.copy(value)?);}
                out.set(key.clone(),renamed);
            }else{require(key!=b"N" && matches!(resolved,Object::Stream(_)),"invalid button appearance role")?;out.set(key.clone(),self.copy(value)?);}
        }
        Ok(out.into())
    }
}
