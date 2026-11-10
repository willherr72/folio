//! Copy-local named destinations use the same source proof and cumulative budget.
use super::*;
impl Prepared<'_> {
    pub(crate) fn for_copy(&mut self,source:usize,value:&Object,old:ObjectId,new:ObjectId,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Option<Object>> {
        if self.target(source,value,check)?!=old{return self.local(source,value,check);}
        require(self.sources.get(source).is_some_and(|s|s.pages.contains(&new)))?;
        let value=self.work.resolve(self.document,value,check)?.1;
        let (key,format)=match value {
            Object::Name(key)=>(key,None),Object::String(key,format)=>(key,Some(*format)),
            _=>return self.copied_destination(value,new,false,check).map(Some),
        };
        let modern=format.is_some();self.work.charge(key.len()+96,check)?;
        let cache_key=(source,new,modern,key.clone());
        let name=if let Some(name)=self.copied_bindings.get(&cache_key) {
            self.work.charge(name.len()+32,check)?;name.clone()
        }else{
            let entries=if modern{&self.sources[source].modern}else{&self.sources[source].legacy};
            let entry=entries.get(key).ok_or_else(invalid)?.value;
            let mut serial=0usize;
            let name=loop {
                self.work.step(check,0)?;serial=serial.checked_add(1).ok_or_else(invalid)?;
                let name=format!("Folio.Copy.{}.{}.{}",new.0,new.1,serial).into_bytes();
                let mut occupied=self.copied_entries.contains_key(&(modern,name.clone()));
                for source in &self.sources {
                    self.work.step(check,0)?;
                    let entries=if modern{&source.modern}else{&source.legacy};
                    occupied|=entries.get(&name).is_some_and(|entry|entry.renamed.is_none());
                    if source.renamed {for entry in entries.values(){
                        self.work.step(check,0)?;occupied|=entry.renamed.as_ref()==Some(&name);
                    }}
                }
                if !occupied{break name;}
            };
            self.work.entry(check)?;self.work.charge(name.len().checked_mul(3).ok_or_else(invalid)?+160,check)?;
            let replacement=self.copied_destination(entry,new,true,check)?;
            self.copied_entries.insert((modern,name.clone()),replacement);
            self.copied_bindings.insert(cache_key,name.clone());name
        };
        Ok(Some(match format{Some(format)=>Object::String(name,format),None=>Object::Name(name)}))
    }
    fn copied_destination(&mut self,value:&Object,new:ObjectId,named:bool,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<Object>{
        let value=self.work.resolve(self.document,value,check)?.1;
        if named {if let Object::Dictionary(dict)=value {
            let destination=self.work.resolve(self.document,dict.get(b"D").map_err(bad)?,check)?.1;
            let replacement=self.copied_destination(destination,new,false,check)?;
            let mut output=Dictionary::new();
            for (key,value) in dict.iter(){self.work.charge(key.len()+64,check)?;if key==b"D"{continue;}output.set(key.clone(),self.work.copy(value,check,0)?);}
            output.set("D",replacement);return Ok(output.into());
        }}
        let values=value.as_array().map_err(bad)?;require((2..=6).contains(&values.len()))?;
        let mut output=Vec::new();self.work.charge(64,check)?;output.push(Object::Reference(new));
        for value in &values[1..]{output.push(self.work.copy(value,check,0)?);}
        Ok(Object::Array(output))
    }
}
