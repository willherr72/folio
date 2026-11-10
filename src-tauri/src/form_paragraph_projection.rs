//! One invocation becomes ordinary page text only inside a worker-owned candidate.
use super::*;

#[cfg(test)]
#[path = "form_paragraph_inherited_tests.rs"]
mod inherited_tests;

pub(in crate::engine) struct FormParagraphScope {
    pub anchor_index: usize,
    pub anchor_text: String,
    pub object_indices: Vec<usize>,
}
struct PromotedPage {bytes:Vec<u8>,root:usize}

impl Model {
    fn promote_paragraph(&self,path:&[usize])->EngineResult<PromotedPage>{
        if path.len()<2||path.len()>DEPTH+1||!self.occurrences.iter().any(|o|o.path==path){
            return Err(invalid("The selected form paragraph path is stale."));
        }
        let root=path[0];
        let range=self.root_invocations.get(&root).ok_or_else(||invalid("The selected form invocation is missing."))?;
        let selected=self.root.operations.iter().enumerate().filter(|(_,op)|op.operator=="Do"||glyph_show(op)).nth(root)
            .filter(|(_,op)|op.operator=="Do").map(|(index,_)|index).ok_or_else(||invalid("The selected form invocation is stale."))?;
        let count=self.root.operations.len().checked_add(range.len()).and_then(|n|n.checked_sub(1))
            .filter(|n|*n<=100_000).ok_or_else(||invalid("Promoted paragraph operation limit exceeded."))?;
        let mut copies=crate::form_graph::CopyBudget::default();
        copies.dictionary(&self.root.resources)?;let mut resources=self.root.resources.clone();
        let mut fonts=match resources.get(b"Font") {Ok(value)=>dictionary_value(&self.pdf,value,&mut copies)?,Err(_)=>Dictionary::new()};
        let mut local=self.flat_operations.get(range.clone()).ok_or_else(||invalid("Missing form validation operators."))?.to_vec();
        let mut aliases=HashMap::<Vec<u8>,Vec<u8>>::new();
        for op in &mut local {
            if op.operator!="Tf" {continue;}
            let name=op.operands.first().and_then(|v|v.as_name().ok()).ok_or_else(||err("promoted Tf"))?.to_vec();
            let alias=if let Some(alias)=aliases.get(&name){alias.clone()}else{
                let font=self.flat_fonts.get(&name).map_err(err)?;
                if let Object::Dictionary(dictionary)=font {copies.dictionary(dictionary)?;}
                let mut alias=format!("FolioParagraph{}",aliases.len()).into_bytes();
                while fonts.has(&alias){alias.push(b'_');}
                fonts.set(alias.clone(),font.clone());aliases.insert(name,alias.clone());alias
            };
            op.operands[0]=Object::Name(alias);
        }
        resources.set("Font",fonts);
        let mut operations=Vec::with_capacity(count);
        operations.extend_from_slice(&self.root.operations[..selected]);operations.append(&mut local);
        operations.extend_from_slice(&self.root.operations[selected+1..]);
        let data=Content{operations}.encode().map_err(err)?;
        if data.len()>LIMIT {return Err(invalid("Promoted paragraph content exceeds the size limit."));}
        let mut pdf=self.pdf.clone();let content=pdf.add_object(Stream::new(dictionary!{},data));
        let page=pdf.get_dictionary_mut(self.page).map_err(err)?;page.set("Contents",content);page.set("Resources",resources);
        let bytes=save(&mut pdf)?;if bytes.len() as u64>MAX_SOURCE_BYTES{return Err(invalid("Promoted paragraph source exceeds the size limit."));}
        Ok(PromotedPage{bytes,root})
    }
}

impl WorkerRuntime {
    pub(in crate::engine) fn with_form_paragraph_projection<T>(
        &mut self,source_id:&str,page_index:usize,path:&[usize],
        use_page:impl FnOnce(&mut Self,&str,&FormParagraphScope)->EngineResult<T>,
    )->EngineResult<T>{
        if path.len()<2||path.len()>DEPTH+1{return Err(invalid("Select a bounded form paragraph path."));}
        self.form_source_admission(source_id,true)?;
        self.with_text_projection(source_id,page_index,|rt,baseline_id|{
            let source=rt.editing_document(baseline_id)?;
            let model=Model::read_mode(source.bytes()?.as_ref(),true)?;
            let before=validate_flat(rt,source.bytes()?.as_ref(),&model)?;
            let promoted=model.promote_paragraph(path)?;
            let original_path=source.original_path.clone();let policy=source.policy.clone();let legacy=source.legacy_comment_import;
            let projection=rt.register_generated_document(promoted.bytes,"Private form paragraph.pdf")?;
            let owned=rt.documents.get_mut(&projection.id).unwrap();owned.original_path=original_path;owned.policy=policy;owned.legacy_comment_import=legacy;
            let result=(||{
                let original=rt.editing_document(baseline_id)?.pdf()?.pages().get(0)?;
                let projected=rt.editing_document(&projection.id)?;let page=projected.pdf()?.pages().get(0)?;
                let a=page_geometry(&original)?;let b=page_geometry(&page)?;
                if [a.left,a.bottom,a.right,a.top]!=[b.left,b.bottom,b.right,b.top]||a.rotation!=b.rotation||raster(&original)?!=raster(&page)?
                    ||!same_glyphs(&glyph_snapshot(&original)?,&glyph_snapshot(&page)?){
                    return Err(invalid("Preparing this form paragraph changes page text or appearance."));
                }
                let proofs=FontProofs::read(projected.bytes()?.as_ref())?;let after=snapshot(&page,Some(&proofs))?;
                let expected=before.iter().zip(&model.occurrences).filter(|(_,o)|o.path.len()==1||o.path[0]==promoted.root).collect::<Vec<_>>();
                if expected.len()!=after.len(){return Err(invalid("Promoted paragraph text mapping is ambiguous."));}
                let mut scope=FormParagraphScope{anchor_index:usize::MAX,anchor_text:String::new(),object_indices:vec![]};
                for ((old,occurrence),new) in expected.into_iter().zip(&after){
                    let mut old=old.clone();old.index=new.index;
                    if !same_style(&old,new)||!close_values(&old.bounds,&new.bounds)
                        ||!old.text.split_whitespace().eq(new.text.split_whitespace()){
                        return Err(invalid("Promoting the paragraph changed a font, text or position."));
                    }
                    if occurrence.path.len()>1&&occurrence.path[0]==promoted.root{scope.object_indices.push(new.index);}
                    if occurrence.path==path {scope.anchor_index=new.index;scope.anchor_text=old.text;}
                }
                if !scope.object_indices.contains(&scope.anchor_index){return Err(invalid("The promoted paragraph anchor is missing."));}
                drop(page);drop(original);
                use_page(rt,&projection.id,&scope)
            })();
            rt.discard_unpublished_source(&projection.id);result
        })
    }
}
