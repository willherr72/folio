//! Borrowed source-bound intentions for a complete page draft. No PDF mutation.
use super::*;

#[derive(Debug)]
pub(in crate::engine) struct AnnotationSource<'a> {
    pub page_index: usize,
    pub annotation_index: usize,
    pub annotation_id: Option<lopdf::ObjectId>,
    pub popup_indices: &'a [usize],
    pub overlay_id: &'a str,
}
impl<'a> AnnotationSource<'a> {
    pub(in crate::engine) fn source_group(&self)->crate::composition_annotation_receipts::SourceGroup<'a> {
        crate::composition_annotation_receipts::SourceGroup{annotation:crate::composition_annotation_receipts::SourceSlot{page_index:self.page_index,slot:self.annotation_index,annotation_id:self.annotation_id},popup_slots:self.popup_indices}
    }
}
#[derive(Debug)]
pub(in crate::engine) enum OwnedChange<'a, 'd> {
    Keep(AnnotationSource<'a>, &'d Overlay),
    Replace(AnnotationSource<'a>, &'d Overlay),
    Delete(AnnotationSource<'a>),
}
pub(in crate::engine) struct PageChanges<'a, 'd> {
    pub owned: Vec<OwnedChange<'a, 'd>>,
    pub added: Vec<&'d Overlay>,
    /// Complete, validated order, including unchanged source drawings.
    pub draft: &'d [Overlay],
}
pub(in crate::engine) struct Publication<'a> {
    pages: BTreeMap<usize, Vec<&'a Binding>>,
    page_count: usize,
    metadata: usize,
    semantic_bytes: usize,
    steps: usize,
    failed: bool,
}
fn invalid()->EngineError {
    EngineError::InvalidRequest("Imported overlay publication requires verified source ownership and a valid complete page draft.".into())
}
impl Index {
    pub(in crate::engine) fn publication<'a>(
        &'a self, source: &crate::source_storage::SourceIdentity, page_count: usize,
        check: &mut impl FnMut()->EngineResult<()>,
    )->EngineResult<Publication<'a>> {
        self.publication_with_metadata(source,page_count,MAX_METADATA,check)
    }
    pub(in crate::engine) fn publication_with_metadata<'a>(
        &'a self, source:&crate::source_storage::SourceIdentity, page_count:usize, metadata:usize,
        check:&mut impl FnMut()->EngineResult<()>,
    )->EngineResult<Publication<'a>> {
        check()?;
        // Runtime source handles own immutable Arc bytes. A replacement or
        // recovered source must complete the existing transfer/restore proof
        // before its index can be used here. No source-wide payload copy/hash.
        if !self.trusted || !self.source.same_source(source) {
            return Err(EngineError::InvalidRequest("The source ownership of these drawings could not be verified, so they cannot be safely saved with this document structure.".into()));
        }
        if page_count==0 || page_count>100_000 || metadata>MAX_METADATA {return Err(invalid());}
        let mut plan=Publication{pages:BTreeMap::new(),page_count,metadata,semantic_bytes:MAX_METADATA,steps:MAX_WORK,failed:false};
        for binding in &self.bindings {
            plan.step(check)?;
            if binding.page_index>=page_count{return Err(invalid());}
            plan.reserve(1,128)?;
            plan.pages.entry(binding.page_index).or_default().push(binding);
        }
        for bindings in plan.pages.values_mut(){check()?;bindings.sort_by_key(|b|b.annotation_index);}
        check()?;
        Ok(plan)
    }
}
impl<'a> Publication<'a> {
    /// The coordinator processes one source at a time and carries this remaining
    /// allowance forward while previously returned borrowed plans stay live.
    pub(in crate::engine) fn remaining_metadata(&self)->usize {self.metadata}
    pub(in crate::engine) fn page<'d>(
        &mut self, page:usize, draft:&'d [Overlay], check:&mut impl FnMut()->EngineResult<()>,
    )->EngineResult<PageChanges<'a,'d>> {
        if self.failed {return Err(invalid());}
        let result=self.page_inner(page,draft,check);
        if result.is_err(){self.failed=true;}
        result
    }
    fn step(&mut self,check:&mut impl FnMut()->EngineResult<()>)->EngineResult<()> {
        check()?;self.steps=self.steps.checked_sub(1).ok_or_else(invalid)?;Ok(())
    }
    fn reserve(&mut self,count:usize,size:usize)->EngineResult<()> {
        self.metadata=self.metadata.checked_sub(count.checked_mul(size).ok_or_else(invalid)?).ok_or_else(invalid)?;Ok(())
    }
    fn page_inner<'d>(&mut self,page:usize,draft:&'d [Overlay],check:&mut impl FnMut()->EngineResult<()>)->EngineResult<PageChanges<'a,'d>> {
        self.step(check)?;
        if page>=self.page_count || draft.len()>10_000{return Err(invalid());}
        self.reserve(draft.len(),160)?;
        let mut remaining=BTreeMap::new();
        for overlay in draft {
            self.step(check)?;let id=label(overlay);
            if id.trim().is_empty() || id.len()>1024{return Err(invalid());}
            self.reserve(id.len(),1)?;
            if remaining.insert(id,overlay).is_some(){return Err(invalid());}
        }
        let count=self.pages.get(&page).map_or(0,Vec::len);
        self.reserve(count,2*std::mem::size_of::<OwnedChange<'a,'d>>())?;
        let mut owned=Vec::with_capacity(count);
        for index in 0..count {
            self.step(check)?;
            let binding=self.pages[&page][index];
            let source=AnnotationSource{page_index:page,annotation_index:binding.annotation_index,annotation_id:binding.annotation_id,popup_indices:&binding.popup_indices,overlay_id:&binding.overlay_id};
            let change=if let Some(overlay)=remaining.remove(binding.overlay_id.as_str()) {
                let steps=semantic_work(overlay,binding.popup_indices.len(),self.steps).ok_or_else(invalid)?;
                self.steps-=steps;
                // This writer borrows text/paths/asset IDs and hashes directly;
                // it does not clone overlays, decode assets or load appearances.
                let Some((hash,bytes))=semantics(overlay,self.semantic_bytes) else {self.semantic_bytes=0;return Err(invalid());};
                self.semantic_bytes-=bytes;
                self.step(check)?;
                if hash==binding.initial_semantics_sha256 || binding.public_echo_sha256.as_ref()==Some(&hash) {OwnedChange::Keep(source,overlay)} else {OwnedChange::Replace(source,overlay)}
            } else {OwnedChange::Delete(source)};
            owned.push(change);
        }
        let mut added=Vec::with_capacity(remaining.len());
        for overlay in draft {self.step(check)?;if remaining.contains_key(label(overlay)){added.push(overlay);}}
        self.step(check)?;
        Ok(PageChanges{owned,added,draft})
    }
}

#[cfg(test)]
mod tests {
    fn identity(bytes: Arc<[u8]>) -> crate::source_storage::SourceIdentity { crate::source_storage::SourceStore::new().capture(bytes).unwrap().identity().clone() }
    use super::*;
    fn highlight(id:&str,x:f32)->Overlay {Overlay::Highlight(HighlightOverlay{id:id.into(),rects:vec![AnnotationRect{x,y:20.,width:50.,height:12.}],color:"#ffff00".into(),opacity:None,text:Some("Original note".into())})}
    fn source()->Arc<[u8]>{Arc::from(b"immutable source bytes".as_slice())}
    fn index()->Index {
        let mut i=Index::capturing(identity(source()));
        // Capture order is deliberately different from source annotation order.
        assert!(i.capture(0,5,Some((9,0)),&[],&highlight("deleted",100.)));
        assert!(i.capture(0,1,Some((7,0)),&[2],&highlight("kept",10.)));
        assert!(i.capture(0,3,None,&[],&highlight("changed",80.)));
        i
    }
    #[test]
    fn imported_overlay_publication_carries_capacity_across_distinct_sources() {
        let a=index();let b=index();let draft=[highlight("kept",10.),highlight("changed",81.),highlight("new",150.)];
        let mut baseline=a.publication(&a.source,1,&mut ||Ok(())).unwrap();baseline.page(0,&draft,&mut ||Ok(())).unwrap();
        let cost=MAX_METADATA-baseline.remaining_metadata();assert!(cost>0 && cost*2<MAX_METADATA);
        for (allowance,accepted) in [(cost*2-1,false),(cost*2,true)] {
            let mut first=a.publication_with_metadata(&a.source,1,allowance,&mut ||Ok(())).unwrap();
            let retained=first.page(0,&draft,&mut ||Ok(())).unwrap();
            let mut independent=b.publication_with_metadata(&b.source,1,allowance,&mut ||Ok(())).unwrap();
            independent.page(0,&draft,&mut ||Ok(())).unwrap();
            let mut second=b.publication_with_metadata(&b.source,1,first.remaining_metadata(),&mut ||Ok(())).unwrap();
            assert_eq!(second.page(0,&draft,&mut ||Ok(())).is_ok(),accepted,"retained source plans share capacity");
            assert_eq!(retained.owned.len(),3);
        }
    }
    #[test]
    fn imported_overlay_publication_binds_keep_replace_delete_and_new_without_payload_copies() {
        let i=index();let before=i.snapshot().unwrap();
        let draft=[highlight("changed",85.),highlight("new",150.),highlight("kept",10.)];
        let mut plan=i.publication(&i.source,1,&mut ||Ok(())).unwrap();
        let page=plan.page(0,&draft,&mut ||Ok(())).unwrap();
        assert_eq!(page.owned.len(),3);assert_eq!(page.added.len(),1);
        match &page.owned[0] {OwnedChange::Keep(b,_)=>{assert_eq!(b.annotation_index,1);assert_eq!(b.annotation_id,Some((7,0)));assert_eq!(b.popup_indices,&[2]);assert_eq!(b.overlay_id,"kept");},other=>panic!("{other:?}")}
        match &page.owned[1] {OwnedChange::Replace(b,o)=>{assert_eq!(b.annotation_index,3);assert_eq!(b.annotation_id,None);assert!(std::ptr::eq(*o,&draft[0]));},other=>panic!("{other:?}")}
        match &page.owned[2] {OwnedChange::Delete(b)=>assert_eq!(b.annotation_index,5),other=>panic!("{other:?}")}
        assert!(std::ptr::eq(page.added[0],&draft[1]));assert_eq!(i.snapshot().unwrap(),before);
    }
    #[test]
    fn imported_overlay_publication_refuses_unbound_stale_and_ambiguous_drafts() {
        let i=index();
        assert!(i.publication(&identity(source()),1,&mut ||Ok(())).is_err(),"equal bytes in an unrelated source instance are not the bound source handle");
        let unbound=Index::unbound(i.source.clone());
        assert!(unbound.publication(&i.source,1,&mut ||Ok(())).is_err());
        assert!(i.publication(&i.source,0,&mut ||Ok(())).is_err());
        let mut p=i.publication(&i.source,1,&mut ||Ok(())).unwrap();
        assert!(p.page(1,&[],&mut ||Ok(())).is_err());
        assert!(p.page(0,&[],&mut ||Ok(())).is_err(),"a failed transaction cannot be reused");
        for draft in [vec![highlight("kept",10.),highlight("kept",10.)],vec![highlight("",10.)],vec![highlight(&"x".repeat(1025),10.)]] {
            let mut p=i.publication(&i.source,1,&mut ||Ok(())).unwrap();
            assert!(p.page(0,&draft,&mut ||Ok(())).is_err());
        }
    }
    #[test]
    fn imported_overlay_publication_keeps_repeated_occurrence_decisions_independent() {
        let mut i=Index::capturing(identity(source()));
        assert!(i.capture(0,0,Some((7,0)),&[],&highlight("same",10.)));
        assert!(i.capture(1,0,Some((9,0)),&[],&highlight("same",20.)));
        let mut p=i.publication(&i.source,2,&mut ||Ok(())).unwrap();
        let first=[highlight("same",10.)];let changed=[highlight("same",15.)];let other=[highlight("same",20.)];
        let a=p.page(0,&first,&mut ||Ok(())).unwrap();
        let b=p.page(0,&changed,&mut ||Ok(())).unwrap();
        let c=p.page(1,&other,&mut ||Ok(())).unwrap();
        assert!(matches!(a.owned[0],OwnedChange::Keep(_,_)));assert!(matches!(b.owned[0],OwnedChange::Replace(_,_)));
        assert!(matches!(&c.owned[0],OwnedChange::Keep(s,_) if s.annotation_id==Some((9,0))));
    }
    #[test]
    fn imported_overlay_publication_budgets_and_cancellation_are_cumulative() {
        let i=index();let draft=[highlight("kept",10.)];
        for mode in 0..3 {
            let mut p=i.publication(&i.source,1,&mut ||Ok(())).unwrap();
            match mode {0=>p.metadata=1,1=>p.semantic_bytes=1,_=>p.steps=1}
            assert!(p.page(0,&draft,&mut ||Ok(())).is_err());
            assert!(p.page(0,&[],&mut ||Ok(())).is_err());
        }
        let mut calls=0;let mut p=i.publication(&i.source,1,&mut ||Ok(())).unwrap();
        let before=p.metadata;
        p.page(0,&draft,&mut ||{calls+=1;Ok(())}).unwrap();
        let used=before-p.metadata;assert!(used>0);
        p.metadata=used;
        p.page(0,&draft,&mut ||Ok(())).unwrap();
        assert!(p.page(0,&draft,&mut ||Ok(())).is_err(),"later pages cannot reset the reservation");
        for stop in [1,calls/2,calls] {
            let mut seen=0;let mut p=i.publication(&i.source,1,&mut ||Ok(())).unwrap();
            assert!(p.page(0,&draft,&mut ||{seen+=1;if seen==stop{Err(invalid())}else{Ok(())}}).is_err());
        }
    }
    #[test]
    fn imported_overlay_publication_accepts_verified_recovery_labels_and_rejects_page_count_drift() {
        let original=index();let mut snapshot=original.snapshot().unwrap();
        for b in &mut snapshot.bindings {b.overlay_id=format!("restored-{}",b.overlay_id);}
        let (restored,_)=original.restore(snapshot).unwrap();
        let draft=[highlight("restored-kept",10.)];
        let mut p=restored.publication(&restored.source,1,&mut ||Ok(())).unwrap();
        assert!(matches!(p.page(0,&draft,&mut ||Ok(())).unwrap().owned[0],OwnedChange::Keep(_,_)));
        let mut invalid_index=Index::capturing(identity(source()));
        assert!(invalid_index.capture(1,0,None,&[],&highlight("second-page",20.)));
        assert!(invalid_index.publication(&invalid_index.source,1,&mut ||Ok(())).is_err());
        let empty=Index::capturing(identity(source()));
        let mut p=empty.publication(&empty.source,1,&mut ||Ok(())).unwrap();
        let page=p.page(0,&draft,&mut ||Ok(())).unwrap();
        assert!(page.owned.is_empty());assert_eq!(page.added.len(),1);
    }
}
