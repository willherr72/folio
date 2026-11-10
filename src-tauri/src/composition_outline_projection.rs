//! Selection of original outline objects, sharing the navigation pass budget.
use super::*;

impl Walker<'_, '_, '_> {
    pub(super) fn project_outline(
        &mut self, outline:&OutlineInfo, source:usize, removed_elements:&BTreeSet<ObjectId>,
        check:&mut impl FnMut()->EngineResult<()>,
    )->EngineResult<BTreeSet<ObjectId>> {
        let Some(root)=outline.root else {return Ok(BTreeSet::new())};
        let mut children=BTreeMap::<ObjectId,Vec<ObjectId>>::new();
        for id in &outline.items {
            self.destinations.charge(96,check)?;
            let dictionary=self.document.get_dictionary(*id).map_err(bad)?;
            let parent=self.destinations.resolve(dictionary.get(b"Parent").map_err(bad)?,check)?.0.ok_or_else(invalid)?;
            children.entry(parent).or_default().push(*id);
        }
        let mut removed=BTreeSet::new();
        let mut visible=BTreeMap::<ObjectId,i64>::new();
        // Inspection supplies preorder. Reverse order decides children before
        // their parent without changing the original sibling or logical order.
        for id in outline.items.iter().rev() {
            self.destinations.tick(0,check)?;
            let dictionary=self.document.get_dictionary(*id).map_err(bad)?;
            let kids=children.entry(*id).or_default();
            kids.retain(|child|!removed.contains(child));
            let gone=if let Some(destination)=self.optional(dictionary,b"Dest",check)? {
                matches!(self.local(source,destination,check)?,Some(Object::Null))
            } else if let Some(action)=self.optional(dictionary,b"A",check)? {
                let action=self.destinations.resolve(action,check)?.1.as_dict().map_err(bad)?;
                let kind=self.destinations.resolve(action.get(b"S").map_err(bad)?,check)?.1.as_name().map_err(bad)?;
                kind==b"GoTo" && matches!(self.local(source,action.get(b"D").map_err(bad)?,check)?,Some(Object::Null))
            } else {false};
            if gone && kids.is_empty() {removed.insert(*id);continue;}
            let descendants=kids.iter().map(|id|visible[id]).sum::<i64>();
            let count=self.optional(dictionary,b"Count",check)?
                .map(|value|self.destinations.resolve(value,check)?.1.as_i64().map_err(bad)).transpose()?;
            let open=count.is_some_and(|count|count>0);
            visible.insert(*id,1+if open {descendants}else{0});
            self.topology(*id,kids,if open {descendants}else{-descendants},check)?;
            if let Some(element)=self.optional(dictionary,b"SE",check)? {
                let element=self.destinations.resolve(element,check)?.0.ok_or_else(invalid)?;
                if removed_elements.contains(&element) {
                    self.stage_fields(*id,BTreeMap::from([(b"SE".to_vec(),Object::Null)]),check)?;
                }
            }
        }
        let top=children.entry(root).or_default();
        top.retain(|id|!removed.contains(id));
        let count=top.iter().map(|id|visible[id]).sum();
        self.topology(root,top,count,check)?;
        Ok(removed)
    }

    fn topology(&mut self,parent:ObjectId,kids:&[ObjectId],count:i64,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<()> {
        let mut changes=Fields::new();
        self.changed_reference(parent,b"First",kids.first().copied(),&mut changes,check)?;
        self.changed_reference(parent,b"Last",kids.last().copied(),&mut changes,check)?;
        let dictionary=self.document.get_dictionary(parent).map_err(bad)?;
        let old=self.optional(dictionary,b"Count",check)?
            .map(|value|self.destinations.resolve(value,check)?.1.as_i64().map_err(bad)).transpose()?;
        if old.unwrap_or(0)!=count {changes.insert(b"Count".to_vec(),Object::Integer(count));}
        self.stage_fields(parent,changes,check)?;
        for (index,id) in kids.iter().enumerate() {
            let mut changes=Fields::new();
            self.changed_reference(*id,b"Prev",index.checked_sub(1).map(|i|kids[i]),&mut changes,check)?;
            self.changed_reference(*id,b"Next",kids.get(index+1).copied(),&mut changes,check)?;
            self.stage_fields(*id,changes,check)?;
        }
        Ok(())
    }

    fn changed_reference(&mut self,id:ObjectId,key:&[u8],target:Option<ObjectId>,changes:&mut Fields,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<()> {
        let dictionary=self.document.get_dictionary(id).map_err(bad)?;
        let old=self.optional(dictionary,key,check)?
            .map(|value|self.destinations.resolve(value,check)?.0.ok_or_else(invalid)).transpose()?;
        if old!=target {changes.insert(key.to_vec(),target.map_or(Object::Null,Object::Reference));}
        Ok(())
    }
}
