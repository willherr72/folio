//! A private authority for same-source text publication. Construction requires
//! the original structure, native object mapping, and unclipped selected ink.
use super::*;
use lopdf::{content::{Content,Operation}, Document, ObjectId};
use text_context::{PageContext,ShowContext};
use text_context_tags::Ownership;

fn bad() -> EngineError {invalid("The text edit could not preserve its original clipping or tag ownership.")}
fn operations(pdf:&Document,page:ObjectId)->EngineResult<Vec<Operation>> {
    let bytes=pdf.get_page_content_with_limit(page,16*1024*1024).map_err(|_|bad())?;
    Content::decode_strict(&bytes).map(|c|c.operations).map_err(|_|bad())
}
fn equal(a:&Operation,b:&Operation)->bool {a.operator==b.operator && a.operands==b.operands}

fn decoded_values(pdf:&Document,page:ObjectId,bytes:&[u8],runs:&[RunSnapshot])->EngineResult<Vec<Option<String>>> {
    let values=source_text::values_on_page(pdf,page,runs)?;
    let needs_proof=|run:&RunSnapshot,value:&Option<String>|value.is_none() && run.embedded && run.text.is_empty()
        && run.bounds==[run.matrix[4],run.matrix[5],run.matrix[4],run.matrix[5]];
    if !runs.iter().zip(&values).any(|(run,value)|needs_proof(run,value)){return Ok(values);}
    // Context snapshots normally need only native style identity. Decode a blank
    // embedded show only with the existing bounded font proof. Failure adds no
    // authority and does not reject unrelated visible runs on complex pages.
    let Ok(proofs)=FontProofs::read(bytes) else{return Ok(values);};
    let mut bound=runs.to_vec();
    for (run,value) in bound.iter_mut().zip(&values) {
        if needs_proof(run,value) {run.proof=run.program.as_deref().and_then(|hash|proofs.find(&run.font,hash).ok());}
    }
    Ok(source_text::values_on_page(pdf,page,&bound).unwrap_or(values))
}

pub(in crate::engine) struct SourcePage {
    page:ObjectId,
    operations:Arc<Vec<Operation>>,
    context:Arc<PageContext>,
    ownership:Ownership,
    runs:Arc<Vec<RunSnapshot>>,
    source_values:Vec<Option<String>>,
    glyphs:HashMap<usize,Vec<Glyph>>,
    group:Option<text_context_group::Group>,
}
impl SourcePage {
    pub(in crate::engine) fn read(bytes:&[u8],page_index:usize,native:&PdfPage<'_>)->EngineResult<Option<Self>> {
        let _phase=crate::native_phase!("context.source_read");
        let pdf={let _phase=crate::native_phase!("context.parse");crate::pdf_output::view(bytes)?};
        let page=*pdf.get_pages().values().nth(page_index).ok_or_else(bad)?;
        let operations=operations(&pdf,page)?;
        let page_dict=pdf.get_dictionary(page).map_err(|_|bad())?;
        let required=page_dict.has(b"Group") || page_dict.has(b"StructParents") || pdf.catalog().map_err(|_|bad())?.has(b"StructTreeRoot")
            || operations.iter().any(|o|matches!(o.operator.as_str(),"W"|"W*"|"BDC"|"BMC")
                || (o.operator=="Tr" && o.operands.first().is_some_and(|v|v.as_float().is_ok_and(|n|n>=4.))));
        if !required {return Ok(None);}
        let context=text_context::inspect(&operations)?;
        let runs=snapshot(native,None)?;
        if context.shows.len()!=runs.len() {return Err(bad());}
        // Font names/sizes and show ordinal must map to actual native objects;
        // unsupported text decoding remains unavailable, never guessed.
        let source_values=decoded_values(&pdf,page,bytes,&runs)?;
        let ownership=Ownership::read(&pdf,page)?;
        let glyphs=text_paragraph::page_object_glyphs(native)?;
        let group=text_context_group::descriptor(&pdf,page)?;
        Ok(Some(Self {page,operations:Arc::new(operations),context:Arc::new(context),ownership,runs:Arc::new(runs),source_values,glyphs,group}))
    }
    pub(in crate::engine) fn paragraph(&self,indices:&[usize],mixed:bool)->EngineResult<PublicationProof> {
        if indices.is_empty() || indices.len()>4096 || indices.windows(2).any(|p|p[0]>=p[1]) {return Err(bad());}
        let mut ordinals=Vec::new();let mut contexts=Vec::new();
        for index in indices {
            let ordinal=self.runs.iter().position(|r|r.index==*index).ok_or_else(bad)?;
            let context=&self.context.shows[ordinal].1;
            check_ink(context,&self.runs[ordinal],self.glyphs.get(index).ok_or_else(bad)?,self.source_values[ordinal].as_deref())?;
            ordinals.push(ordinal);contexts.push(context.clone());
        }
        self.ownership.check(&contexts)?;
        if !mixed && contexts[0].has_marked() {return Err(invalid("Use styled paragraph editing to preserve marked text ownership."));}
        let first=ordinals[0];
        Ok(PublicationProof {page:self.page,operations:self.operations.clone(),runs:self.runs.clone(),ordinal:first,
            show:self.context.shows[first].0,context:contexts.remove(0),group:self.group.clone(),tagged:self.ownership.tagged(),
            substitution:None,paragraph:Some(ParagraphProof{source:self.context.clone(),ordinals,at_source:mixed})})
    }
    pub(super) fn proof(&self,index:usize)->EngineResult<PublicationProof> {
        let ordinal=self.runs.iter().position(|r|r.index==index).ok_or_else(bad)?;
        let context=&self.context.shows[ordinal].1;
        self.ownership.check(std::slice::from_ref(context))?;
        check_ink(context,&self.runs[ordinal],self.glyphs.get(&index).ok_or_else(bad)?,self.source_values[ordinal].as_deref())?;
        Ok(PublicationProof {page:self.page,operations:self.operations.clone(),runs:self.runs.clone(),ordinal,
            show:self.context.shows[ordinal].0,context:context.clone(),paragraph:None,substitution:None,group:self.group.clone(),tagged:self.ownership.tagged()})
    }
}
fn check_ink(context:&ShowContext,run:&RunSnapshot,glyphs:&[Glyph],source:Option<&str>)->EngineResult<()> {
    // PDFium may omit every extracted glyph for an embedded space-only show.
    // Exact decoded operands plus native zero-area bounds at the text origin
    // establish a nonpainting spacer. Blank extraction alone grants no authority.
    let origin=[run.matrix[4],run.matrix[5],run.matrix[4],run.matrix[5]];
    if glyphs.is_empty() && run.text.is_empty()
        && source.is_some_and(|text|!text.is_empty()&&text.bytes().all(|b|b==b' '))
        && run.mode==PdfPageTextRenderMode::FilledUnstroked
        && run.matrix.iter().all(|v|v.is_finite()) && run.bounds==origin
        && context.contains(origin.map(f64::from)) {
        return Ok(());
    }
    if glyphs.is_empty() || !context.contains(run.bounds.map(f64::from)) || glyphs.iter().any(|g| {
        !context.contains([g.position[2] as f64,g.position[3] as f64,g.position[4] as f64,g.position[5] as f64])
    }) {return Err(invalid("The selected or replacement text extends outside its clipping path."));}
    Ok(())
}

// The fields and constructor stay private to this module. Neither an incoming
// PDF nor a client can supply a trusted context to bypass publication guards.
struct ParagraphProof {source:Arc<PageContext>,ordinals:Vec<usize>,at_source:bool}
pub(in crate::engine) struct PublicationProof {
    page:ObjectId,
    operations:Arc<Vec<Operation>>,
    runs:Arc<Vec<RunSnapshot>>,
    ordinal:usize,
    show:usize,
    context:ShowContext,
    group:Option<text_context_group::Group>,
    tagged:bool,
    paragraph:Option<ParagraphProof>,
    substitution:Option<Arc<Vec<Operation>>>,
}
impl PublicationProof {
    pub(super) fn bind_substitution(&mut self,plan:&text_substitution::Plan)->EngineResult<()> {
        if self.paragraph.is_some()||!plan.matches_source(&self.operations,self.ordinal){return Err(bad());}
        self.substitution=Some(plan.candidate());Ok(())
    }
    pub(in crate::engine) fn tagged(&self)->bool {self.tagged}
    fn page_context(&self,pdf:&Document,page:ObjectId,edited:bool)->EngineResult<()> {
        if text_context_group::descriptor(pdf,page)?!=self.group {return Err(bad());}
        let current=operations(pdf,page)?;
        if edited && self.paragraph.is_some() {return self.paragraph_context(&current);}
        if let Some(expected)=self.substitution.as_ref().filter(|_|edited) {
            if expected.len()!=current.len()||!expected.iter().zip(&current).all(|(a,b)|equal(a,b)){return Err(bad());}
        } else {
            if current.len()!=self.operations.len() {return Err(bad());}
            for (index,(original,changed)) in self.operations.iter().zip(&current).enumerate() {
                if edited && index==self.show {
                    if original.operator!=changed.operator || (original.operator=="\"" && original.operands.get(..2)!=changed.operands.get(..2)) {return Err(bad());}
                } else if !equal(original,changed) {return Err(bad());}
            }
        }
        let context=text_context::inspect(&current)?;
        if context.shows.len()!=self.runs.len() || context.shows[self.ordinal].1.binding()?!=self.context.binding()? {return Err(bad());}
        // Exact operator preservation above also retains unknown/unselected
        // contexts. Selected clipping and semantic scopes need positive proof.
        Ok(())
    }
    fn generated_range(&self,total:usize)->EngineResult<std::ops::Range<usize>> {
        let p=self.paragraph.as_ref().ok_or_else(bad)?;
        let remaining=self.runs.len()-p.ordinals.len();
        let count=total.checked_sub(remaining).filter(|n|*n>0&&*n<=4096).ok_or_else(bad)?;
        let start=if p.at_source {p.ordinals[0]} else {remaining};
        let end=start.checked_add(count).filter(|end|*end<=total).ok_or_else(bad)?;Ok(start..end)
    }
    fn paragraph_context(&self,ops:&[Operation])->EngineResult<()> {
        let p=self.paragraph.as_ref().ok_or_else(bad)?;let current=text_context::inspect(ops)?;
        if p.source.marked_events.len()!=current.marked_events.len() || !p.source.marked_events.iter().zip(&current.marked_events).all(|(a,b)|equal(a,b)) {return Err(bad());}
        let generated=self.generated_range(current.shows.len())?;
        let binding=self.context.binding()?;
        for (_,ctx) in &current.shows[generated.clone()] {if ctx.binding()?!=binding {return Err(bad());}}
        let old=p.source.shows.iter().enumerate().filter(|(i,_)|!p.ordinals.contains(i)).map(|(_,s)|s);
        let new=current.shows[..generated.start].iter().chain(&current.shows[generated.end..]);
        for ((a,ac),(b,bc)) in old.zip(new) {
            // Unselected marks remain opaque and exact. Comparing them does not
            // authorize editing their semantics; selected contexts above still
            // require admission and original-document ownership.
            if !equal(&self.operations[*a],&ops[*b]) || ac.preserved_binding()?!=bc.preserved_binding()? {return Err(bad());}
        }
        Ok(())
    }
    pub(super) fn baseline(&self,bytes:&[u8],runs:&[RunSnapshot])->EngineResult<()> {
        let pdf=crate::pdf_output::load(bytes)?;let page=*pdf.get_pages().values().next().ok_or_else(bad)?;
        if runs.len()!=self.runs.len() || !runs.iter().zip(self.runs.iter()).all(|(a,b)|same_run(a,b)) {return Err(bad());}
        self.page_context(&pdf,page,false)
    }
    pub(in crate::engine) fn verify_candidate(&self,bytes:&[u8],native:&PdfPage<'_>)->EngineResult<()> {
        self.candidate(bytes,native,&snapshot(native,None)?)
    }
    pub(super) fn candidate(&self,bytes:&[u8],native:&PdfPage<'_>,runs:&[RunSnapshot])->EngineResult<()> {
        let pdf=crate::pdf_output::load(bytes)?;let page=*pdf.get_pages().values().next().ok_or_else(bad)?;
        self.page_context(&pdf,page,true)?;
        let source_values=decoded_values(&pdf,page,bytes,runs)?;
        if self.paragraph.is_some() {
            let glyphs=text_paragraph::page_object_glyphs(native)?;
            for ordinal in self.generated_range(runs.len())? {
                let run=&runs[ordinal];
                check_ink(&self.context,run,glyphs.get(&run.index).ok_or_else(bad)?,source_values[ordinal].as_deref())?;
            }
            return Ok(());
        }
        if runs.len()!=self.runs.len() || runs[self.ordinal].index!=self.runs[self.ordinal].index {return Err(bad());}
        let run=&runs[self.ordinal];
        check_ink(&self.context,run,&text_paragraph::object_glyphs(native,run.index)?,source_values[self.ordinal].as_deref())
    }
    pub(in crate::engine) fn publication(&self,target:&Document,target_page:ObjectId,source:&Document,source_page:ObjectId)->EngineResult<()> {
        self.page_context(target,target_page,false)?;
        if self.tagged && target_page!=self.page {return Err(invalid("Duplicating tagged text pages is not supported yet."));}
        text_context_tags::prove(target,target_page,std::slice::from_ref(&self.context))?;
        self.page_context(source,source_page,true)
    }
    pub(in crate::engine) fn published(&self,pdf:&Document,page:ObjectId)->EngineResult<()> {
        self.page_context(pdf,page,true)?;
        text_context_tags::prove(pdf,page,std::slice::from_ref(&self.context))
    }
}

impl WorkerRuntime {
    pub(in crate::engine) fn paragraph_text_context(&self,source_id:&str,page_index:usize,indices:&[usize],mixed:bool)->EngineResult<Option<PublicationProof>> {
        let source=self.editing_document(source_id)?;
        let native=source.pdf()?.pages().get(page_index_i32(page_index)?)?;
        SourcePage::read(source.bytes()?.as_ref(),page_index,&native)?.map(|page|page.paragraph(indices,mixed)).transpose()
    }

    pub(in crate::engine) fn single_text_context(&self,source_id:&str,page_index:usize,index:usize)->EngineResult<Option<PublicationProof>> {
        let source=self.editing_document(source_id)?;
        let native=source.pdf()?.pages().get(page_index_i32(page_index)?)?;
        SourcePage::read(source.bytes()?.as_ref(),page_index,&native)?.map(|page|page.proof(index)).transpose()
    }

    pub(in crate::engine) fn single_font_text_context(&self,source_id:&str,page_index:usize,index:usize,expected:&str,replacement:&str,font_id:Option<&str>)->EngineResult<Option<PublicationProof>> {
        let _phase=crate::native_phase!("context.font");
        let mut context=self.single_text_context(source_id,page_index,index)?;
        if let Some(context)=&mut context {
            let copy=self.text_transaction_page_copy(source_id,page_index,true)?;
            let bytes=copy.save_to_bytes()?;let proofs=FontProofs::read(&bytes)?;
            let runs=bound_snapshot(&copy.pages().get(0)?,Some(&proofs),&bytes)?;
            let selected=runs.iter().find(|r|r.index==index).ok_or_else(bad)?;
            if selected.text!=expected{return Err(invalid("The text changed since it was selected. Select the run again."));}
            let value=source_text::selected(&bytes,&runs,index);let selected=source_edit_run(selected,value.as_deref());
            let plan=if let Some(id)=font_id {
                let font=self.fonts.get(id)?;
                text_substitution::Plan::prepare(&bytes,&selected,&runs,replacement,&font)?
            } else {
                text_substitution::Plan::prepare_original(&bytes,&selected,&runs,replacement)?
            };
            context.bind_substitution(&plan)?;
        }
        Ok(context)
    }
}

#[cfg(test)]
#[path="text_context_edit_tests.rs"]
mod tests;
