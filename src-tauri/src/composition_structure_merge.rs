//! Reconcile source-local structure names and ownership without content import.
#![allow(dead_code)]
use crate::{EngineError,EngineResult};
use crate::composition_structure::{StructureInfo,Parents};
use lopdf::{dictionary,Dictionary,Document,Object,ObjectId,StringFormat};
use std::collections::{BTreeMap,BTreeSet};

pub(crate) fn merge(
    doc:&mut Document,
    sources:&[StructureInfo],
    check:&mut impl FnMut()->EngineResult<()>,
)->EngineResult<ObjectId>{
    let mut work=Work{check,steps:0,bytes:16*1024*1024};
    work.step(0)?;require(!sources.is_empty() && sources.len()<=100_000)?;
    let reserved=original_roles(doc,sources,&mut work)?;
    let base=doc.max_id.max(doc.objects.keys().next_back().map_or(0,|id|id.0));
    let end=base.checked_add(3).ok_or_else(invalid)?;
    let root_id=(base+1,0);let parent_id=(base+2,0);let idtree_id=(base+3,0);
    let mut fields=BTreeMap::new();let mut replacements=BTreeMap::new();
    let mut kids=vec![];let mut nums=vec![];let mut id_entries=BTreeMap::new();
    let mut roles=Dictionary::new();let mut classes=Dictionary::new();
    let mut namespaces=Vec::new();let mut namespace_seen=BTreeSet::new();
    let mut extras:BTreeMap<Vec<u8>,Vec<Object>>=BTreeMap::new();
    let mut owner_count=0i64;let mut element_count=0usize;
    for (index,source) in sources.iter().enumerate(){
        work.step(0)?;
        element_count=element_count.checked_add(source.elements.len()).ok_or_else(invalid)?;
        require(element_count<=100_000)?;
        let source_root=doc.get_dictionary(source.root.ok_or_else(invalid)?).map_err(bad)?;
        for (key,_) in source_root.iter(){
            work.step(0)?;
            require([b"Type".as_slice(),b"K",b"ParentTree",b"ParentTreeNextKey",b"RoleMap",b"ClassMap",b"IDTree",b"Namespaces",b"AF",b"PronunciationLexicon"].contains(&key.as_slice()))?;
        }
        let mut attempt=0usize;
        let prefix=loop {
            work.step(0)?;
            let candidate=if attempt==0 {format!("FolioSource{index}:")}else{format!("FolioSource{index}_{attempt}:")};
            let captures=reserved.range(candidate.as_bytes().to_vec()..).next().is_some_and(|name|name.starts_with(candidate.as_bytes()));
            if !captures{break candidate;}
            attempt+=1;
        };
        let source_ids=identifiers(doc,source,&mut work)?;
        let mut renamed_ids=BTreeMap::new();
        for (old,element) in source_ids {
            let new=work.qualify(prefix.as_bytes(),&old)?;
            work.charge(new.len()+32)?;
            fields.insert((element,b"ID".to_vec()),Object::String(new.clone(),StringFormat::Hexadecimal));
            work.charge(new.len()+32)?;
            id_entries.insert(new.clone(),element);
            renamed_ids.insert(old,new);
        }
        let mut source_roles=BTreeMap::new();
        if let Some(value)=optional(doc,source_root,b"RoleMap",&mut work)? {
            let map=value.as_dict().map_err(bad)?;
            for (key,value) in map.iter(){
                let target=resolve(doc,value,&mut work)?.1.as_name().map_err(bad)?;
                work.charge(key.len()+target.len()+64)?;
                source_roles.insert(key.clone(),target.to_vec());
            }
        }
        for (key,target) in &source_roles {
            let key=work.qualify(prefix.as_bytes(),key)?;
            let target=role_name(target,&source_roles,prefix.as_bytes(),&mut work)?;
            roles.set(key,Object::Name(target));
        }
        let mut source_namespaces=BTreeSet::new();
        if let Some(value)=optional(doc,source_root,b"Namespaces",&mut work)? {
            for value in value.as_array().map_err(bad)? {
                let (id,object)=resolve(doc,value,&mut work)?;let id=id.ok_or_else(invalid)?;
                let namespace=object.as_dict().map_err(bad)?;
                require(resolve(doc,namespace.get(b"Type").map_err(bad)?,&mut work)?.1.as_name().map_err(bad)?==b"Namespace")?;
                resolve(doc,namespace.get(b"NS").map_err(bad)?,&mut work)?.1.as_str().map_err(bad)?;
                source_namespaces.insert(id);
                if namespace_seen.insert(id){work.charge(32)?;namespaces.push(Object::Reference(id));}
            }
        }
        for key in [b"AF".as_slice(),b"PronunciationLexicon"] {
            if let Some(value)=optional(doc,source_root,key,&mut work)? {
                for value in value.as_array().map_err(bad)? {
                    extras.entry(key.to_vec()).or_default().push(work.copy(value,0)?);
                }
            }
        }
        let mut attrs=Attributes{doc,ids:&renamed_ids,fields:&mut fields,replacements:&mut replacements,done:BTreeSet::new(),active:BTreeSet::new()};
        if let Some(value)=optional(doc,source_root,b"ClassMap",&mut work)? {
            for (key,value) in value.as_dict().map_err(bad)?.iter(){
                let key=work.qualify(prefix.as_bytes(),key)?;
                classes.set(key,attrs.rewrite(value,false,&mut work,0)?);
            }
        }
        for element in &source.elements {
            work.step(0)?;
            let dictionary=doc.get_dictionary(*element).map_err(bad)?;
            if let Some(namespace)=optional(doc,dictionary,b"NS",&mut work)? {
                let id=resolve(doc,dictionary.get(b"NS").map_err(bad)?,&mut work)?.0.ok_or_else(invalid)?;
                require(namespace.as_dict().is_ok() && source_namespaces.contains(&id))?;
            } else {
                let old=resolve(doc,dictionary.get(b"S").map_err(bad)?,&mut work)?.1.as_name().map_err(bad)?;
                let new=role_name(old,&source_roles,prefix.as_bytes(),&mut work)?;
                attrs.fields.insert((*element,b"S".to_vec()),Object::Name(new));
            }
            if let Some(value)=optional(doc,dictionary,b"C",&mut work)? {
                let renamed=class_names(doc,value,prefix.as_bytes(),&mut work)?;
                attrs.fields.insert((*element,b"C".to_vec()),renamed);
            }
            if optional(doc,dictionary,b"A",&mut work)?.is_some() {
                let value=dictionary.get(b"A").map_err(bad)?;
                let rewritten=attrs.rewrite(value,true,&mut work,0)?;
                attrs.fields.insert((*element,b"A".to_vec()),rewritten);
            }
        }
        for child in &source.top {
            work.charge(64)?;
            attrs.fields.insert((*child,b"P".to_vec()),Object::Reference(root_id));
            kids.push(Object::Reference(*child));
        }
        for owner in source.owners.values() {
            work.step(0)?;require(owner_count<100_000)?;
            work.charge(64)?;
            let (key,value)=match &owner.parents {
                Parents::Whole(parent)=>(b"StructParent".as_slice(),Object::Reference(*parent)),
                Parents::Marked(parents)=>{
                    work.charge(parents.len().checked_mul(32).ok_or_else(invalid)?)?;
                    (b"StructParents".as_slice(),Object::Array(parents.iter().map(|parent|parent.map_or(Object::Null,Object::Reference)).collect()))
                }
            };
            attrs.fields.insert((owner.object,key.to_vec()),Object::Integer(owner_count));
            nums.push(Object::Integer(owner_count));nums.push(value);owner_count+=1;
        }
    }
    let mut root=dictionary!{"Type"=>"StructTreeRoot","K"=>kids,"ParentTree"=>parent_id,"ParentTreeNextKey"=>owner_count};
    if !roles.is_empty(){root.set("RoleMap",roles);}
    if !classes.is_empty(){root.set("ClassMap",classes);}
    if !namespaces.is_empty(){root.set("Namespaces",namespaces);}
    for (key,value) in extras {root.set(key,value);}
    let mut names=Vec::new();
    for (key,id) in id_entries {work.step(0)?;names.push(Object::String(key,StringFormat::Hexadecimal));names.push(Object::Reference(id));}
    if !names.is_empty(){root.set("IDTree",idtree_id);}
    // Validate every target before the single non-fallible commit. Stream
    // dictionaries are patched in place; their encoded payloads are untouched.
    for ((id,_),_) in &fields {
        work.step(0)?;
        require(matches!(doc.objects.get(id),Some(Object::Dictionary(_)|Object::Stream(_))))?;
    }
    for id in replacements.keys(){work.step(0)?;require(matches!(doc.objects.get(id),Some(Object::Array(_))))?;}
    work.step(0)?;
    for (id,value) in replacements {doc.objects.insert(id,value);}
    for ((id,key),value) in fields {
        match doc.objects.get_mut(&id).unwrap(){
            Object::Dictionary(dictionary)=>dictionary.set(key,value),
            Object::Stream(stream)=>stream.dict.set(key,value),
            _=>unreachable!("validated field target"),
        }
    }
    doc.objects.insert(root_id,root.into());
    doc.objects.insert(parent_id,dictionary!{"Nums"=>nums}.into());
    doc.objects.insert(idtree_id,dictionary!{"Names"=>names}.into());
    doc.max_id=end;
    Ok(root_id)
}

fn bad(message:impl std::fmt::Display)->EngineError {EngineError::InvalidRequest(format!("PDF structure merge: {message}"))}
fn invalid()->EngineError {bad("inconsistent or unsupported structure metadata")}
fn require(value:bool)->EngineResult<()> {if value{Ok(())}else{Err(invalid())}}
struct Work<'a>{check:&'a mut dyn FnMut()->EngineResult<()>,steps:usize,bytes:usize}
impl Work<'_>{
    fn step(&mut self,depth:usize)->EngineResult<()>{(self.check)()?;self.steps+=1;require(depth<=64 && self.steps<=1_000_000)}
    fn charge(&mut self,bytes:usize)->EngineResult<()>{self.step(0)?;self.bytes=self.bytes.checked_sub(bytes).ok_or_else(||bad("metadata budget exceeded"))?;Ok(())}
    fn qualify(&mut self,prefix:&[u8],value:&[u8])->EngineResult<Vec<u8>>{self.charge(prefix.len().checked_add(value.len()).ok_or_else(invalid)?.checked_add(32).ok_or_else(invalid)?)?;let mut out=prefix.to_vec();out.extend_from_slice(value);Ok(out)}
    fn copy(&mut self,value:&Object,depth:usize)->EngineResult<Object>{
        self.step(depth)?;self.charge(32)?;
        match value {
            Object::Name(bytes)|Object::String(bytes,_)=>{self.charge(bytes.len())?;Ok(value.clone())},
            Object::Array(values)=>Ok(Object::Array(values.iter().map(|value|self.copy(value,depth+1)).collect::<EngineResult<_>>()?)),
            Object::Dictionary(dictionary)=>{let mut out=Dictionary::new();for (key,value) in dictionary.iter(){self.charge(key.len())?;out.set(key.clone(),self.copy(value,depth+1)?);}Ok(out.into())},
            Object::Stream(_)=>Err(bad("direct attribute stream")),
            _=>Ok(value.clone()),
        }
    }
}
fn resolve<'a>(doc:&'a Document,mut value:&'a Object,work:&mut Work<'_>)->EngineResult<(Option<ObjectId>,&'a Object)>{
    let mut seen=BTreeSet::new();let mut canonical=None;
    loop {work.step(seen.len())?;match value {Object::Reference(id)=>{require(seen.insert(*id))?;canonical=Some(*id);value=doc.objects.get(id).ok_or_else(invalid)?;},_=>return Ok((canonical,value))}}
}
fn optional<'a>(doc:&'a Document,dictionary:&'a Dictionary,key:&[u8],work:&mut Work<'_>)->EngineResult<Option<&'a Object>>{
    match dictionary.get(key){Ok(value)=>{let value=resolve(doc,value,work)?.1;Ok((!matches!(value,Object::Null)).then_some(value))},Err(_)=>Ok(None)}
}
fn role_name(name:&[u8],roles:&BTreeMap<Vec<u8>,Vec<u8>>,prefix:&[u8],work:&mut Work<'_>)->EngineResult<Vec<u8>>{
    if roles.contains_key(name){work.qualify(prefix,name)}else{work.charge(name.len()+32)?;Ok(name.to_vec())}
}
fn original_roles(doc:&Document,sources:&[StructureInfo],work:&mut Work<'_>)->EngineResult<BTreeSet<Vec<u8>>>{
    let mut names=BTreeSet::new();
    for source in sources {
        let root=doc.get_dictionary(source.root.ok_or_else(invalid)?).map_err(bad)?;
        if let Some(roles)=optional(doc,root,b"RoleMap",work)? {
            for (key,value) in roles.as_dict().map_err(bad)?.iter(){
                let target=resolve(doc,value,work)?.1.as_name().map_err(bad)?;
                work.charge(key.len()+target.len()+64)?;names.insert(key.clone());names.insert(target.to_vec());
            }
        }
        for element in &source.elements {
            let element=doc.get_dictionary(*element).map_err(bad)?;
            let name=resolve(doc,element.get(b"S").map_err(bad)?,work)?.1.as_name().map_err(bad)?;
            work.charge(name.len()+32)?;names.insert(name.to_vec());
        }
    }
    Ok(names)
}
fn class_names(doc:&Document,value:&Object,prefix:&[u8],work:&mut Work<'_>)->EngineResult<Object>{
    match resolve(doc,value,work)?.1 {
        Object::Name(name)=>Ok(Object::Name(work.qualify(prefix,name)?)),
        Object::Array(values)=>{
            let mut out=vec![];let mut previous=false;
            for value in values {match resolve(doc,value,work)?.1 {
                Object::Name(name)=>{out.push(Object::Name(work.qualify(prefix,name)?));previous=true;},
                Object::Integer(revision)=>{require(previous && *revision>=0)?;work.charge(32)?;out.push(Object::Integer(*revision));previous=false;},
                _=>return Err(invalid()),
            }}Ok(Object::Array(out))
        }
        _=>Err(invalid()),
    }
}

fn identifiers(doc:&Document,source:&StructureInfo,work:&mut Work<'_>)->EngineResult<BTreeMap<Vec<u8>,ObjectId>>{
    let mut expected=BTreeMap::new();
    for id in &source.elements {
        work.step(0)?;
        let dictionary=doc.get_dictionary(*id).map_err(bad)?;
        if let Some(value)=optional(doc,dictionary,b"ID",work)? {
            let bytes=value.as_str().map_err(bad)?;work.charge(bytes.len()+64)?;
            require(expected.insert(bytes.to_vec(),*id).is_none())?;
        }
    }
    let root=doc.get_dictionary(source.root.ok_or_else(invalid)?).map_err(bad)?;
    let mut actual=BTreeMap::new();
    if let Ok(value)=root.get(b"IDTree") {
        if !matches!(resolve(doc,value,work)?.1,Object::Null){name_tree(doc,value,&mut actual,&mut BTreeSet::new(),work,0)?;}
    }
    require(expected==actual)?;
    Ok(expected)
}
fn name_tree(doc:&Document,value:&Object,entries:&mut BTreeMap<Vec<u8>,ObjectId>,seen:&mut BTreeSet<ObjectId>,work:&mut Work<'_>,depth:usize)->EngineResult<Option<(Vec<u8>,Vec<u8>)>>{
    work.step(depth)?;
    let (id,value)=resolve(doc,value,work)?;
    if let Some(id)=id{require(seen.len()<100_000 && seen.insert(id))?;}
    let dictionary=value.as_dict().map_err(bad)?;
    let names=optional(doc,dictionary,b"Names",work)?;
    let kids=optional(doc,dictionary,b"Kids",work)?;
    require(names.is_some()!=kids.is_some())?;
    let mut range:Option<(Vec<u8>,Vec<u8>)>=None;
    if let Some(names)=names {
        let values=names.as_array().map_err(bad)?;
        require(values.len()%2==0 && values.len()/2<=100_000usize.saturating_sub(entries.len()))?;
        for pair in values.chunks_exact(2){
            let key=resolve(doc,&pair[0],work)?.1.as_str().map_err(bad)?;
            require(range.as_ref().is_none_or(|(_,prior)|prior.as_slice()<key))?;
            let id=resolve(doc,&pair[1],work)?.0.ok_or_else(invalid)?;
            work.charge(key.len().checked_mul(3).ok_or_else(invalid)?+64)?;
            require(entries.insert(key.to_vec(),id).is_none())?;
            range=Some((range.map_or_else(||key.to_vec(),|(first,_)|first),key.to_vec()));
        }
    }else{
        let values=kids.unwrap().as_array().map_err(bad)?;require(!values.is_empty())?;
        for value in values {
            let next=name_tree(doc,value,entries,seen,work,depth+1)?.ok_or_else(invalid)?;
            require(range.as_ref().is_none_or(|(_,prior)|prior<&next.0))?;
            range=Some((range.map_or(next.0,|(first,_)|first),next.1));
        }
    }
    if let Some(limits)=optional(doc,dictionary,b"Limits",work)? {
        let limits=limits.as_array().map_err(bad)?;require(limits.len()==2)?;
        let first=resolve(doc,&limits[0],work)?.1.as_str().map_err(bad)?;
        let last=resolve(doc,&limits[1],work)?.1.as_str().map_err(bad)?;
        require(range.as_ref().is_some_and(|(a,b)|a.as_slice()==first && b.as_slice()==last))?;
    }
    Ok(range)
}

struct Attributes<'a,'b> {
    doc:&'a Document,
    ids:&'a BTreeMap<Vec<u8>,Vec<u8>>,
    fields:&'b mut BTreeMap<(ObjectId,Vec<u8>),Object>,
    replacements:&'b mut BTreeMap<ObjectId,Object>,
    done:BTreeSet<(ObjectId,bool)>,
    active:BTreeSet<ObjectId>,
}
impl Attributes<'_,'_>{
    fn headers(&self,dictionary:&Dictionary,work:&mut Work<'_>)->EngineResult<Option<Object>>{
        let owner=resolve(self.doc,dictionary.get(b"O").map_err(bad)?,work)?.1.as_name().map_err(bad)?;
        if owner!=b"Table"{return Ok(None);}
        let Some(value)=optional(self.doc,dictionary,b"Headers",work)? else{return Ok(None)};
        let values=value.as_array().map_err(bad)?;
        let mut out=Vec::new();
        for value in values {
            let id=resolve(self.doc,value,work)?.1.as_str().map_err(bad)?;
            let new=self.ids.get(id).ok_or_else(||bad("table Header refers to an unknown element ID"))?;
            work.charge(new.len()+32)?;
            out.push(Object::String(new.clone(),StringFormat::Hexadecimal));
        }
        Ok(Some(Object::Array(out)))
    }
    fn rewrite(&mut self,value:&Object,revisions:bool,work:&mut Work<'_>,depth:usize)->EngineResult<Object>{
        work.step(depth)?;
        let (id,resolved)=resolve(self.doc,value,work)?;
        if let Some(id)=id{
            if self.done.contains(&(id,revisions)){return work.copy(value,0);}
            require(self.active.insert(id))?;
        }
        let result=match resolved {
            Object::Dictionary(dictionary)=>{
                let headers=self.headers(dictionary,work)?;
                if let Some(id)=id {
                    if let Some(headers)=headers{self.fields.insert((id,b"Headers".to_vec()),headers);}
                    work.copy(value,0)?
                }else{
                    let Object::Dictionary(mut out)=work.copy(resolved,depth)? else {unreachable!("dictionary copy")};
                    if let Some(headers)=headers{out.set("Headers",headers);}
                    Object::Dictionary(out)
                }
            }
            Object::Stream(stream)=>{
                let id=id.ok_or_else(invalid)?;
                if let Some(headers)=self.headers(&stream.dict,work)?{self.fields.insert((id,b"Headers".to_vec()),headers);}
                work.copy(value,0)?
            }
            Object::Array(values)=>{
                let mut out=Vec::new();let mut previous=false;
                for value in values {
                    if let Object::Integer(revision)=resolve(self.doc,value,work)?.1 {
                        require(revisions && previous && *revision>=0)?;
                        out.push(work.copy(value,0)?);previous=false;
                    }else{
                        out.push(self.rewrite(value,revisions,work,depth+1)?);previous=true;
                    }
                }
                let out=Object::Array(out);
                if let Some(id)=id{self.replacements.insert(id,out);work.copy(value,0)?}else{out}
            }
            _=>return Err(invalid()),
        };
        if let Some(id)=id{self.active.remove(&id);self.done.insert((id,revisions));}
        Ok(result)
    }
}

#[cfg(test)]
#[path="composition_structure_merge_tests.rs"]
mod tests;
