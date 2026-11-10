//! Paragraph validation in a private projection, publication into the full source.
use super::*;

pub(super) enum Request {
    Map{source:String,page:usize,request:crate::types::ParagraphDraftRequest,reply:mpsc::Sender<EngineResult<ParagraphDraftMapping>>},
    InspectText{source:String,page:usize,indices:Vec<usize>,reply:mpsc::Sender<EngineResult<TextParagraphSelection>>},
    SuggestText{source:String,page:usize,index:usize,expected:String,reply:mpsc::Sender<EngineResult<TextParagraphSelection>>},
    InspectStyled{source:String,page:usize,indices:Vec<usize>,reply:mpsc::Sender<EngineResult<StyledParagraphSelection>>},
    SuggestStyled{source:String,page:usize,index:usize,expected:String,reply:mpsc::Sender<EngineResult<StyledParagraphSelection>>},
    Layout{source:String,page:usize,request:StyledParagraphLayoutRequest,reply:mpsc::Sender<EngineResult<StyledParagraphLayout>>},
    Fit{source:String,page:usize,request:StyledParagraphLayoutRequest,reply:mpsc::Sender<EngineResult<AnnotationRect>>},
    PreviewText{source:String,occurrences:Vec<usize>,selected:usize,indices:Vec<usize>,expected:String,replacement:String,bounds:AnnotationRect,formatting:ParagraphFormatting,reply:mpsc::Sender<EngineResult<PageContentReplacement>>},
    PreviewStyled{source:String,occurrences:Vec<usize>,selected:usize,request:StyledParagraphLayoutRequest,reply:mpsc::Sender<EngineResult<PageContentReplacement>>},
}

fn selected_page(occurrences:&[usize],selected:usize)->EngineResult<usize>{
    if occurrences.is_empty()||occurrences.len()>100_000||selected>=occurrences.len(){
        return Err(EngineError::InvalidRequest("Invalid paragraph page occurrence selection.".into()));
    }
    Ok(occurrences[selected])
}

impl PdfEngine {
    pub fn map_paragraph_draft(&self,source_id:&str,page_index:usize,indices:&[usize],expected_paragraph:&str,anchor:usize,expected_source:&str,styled:bool)->EngineResult<ParagraphDraftMapping>{
        let request=crate::types::ParagraphDraftRequest::checked(indices,expected_paragraph,anchor,expected_source,styled)?;
        self.paragraph_request(|reply|Request::Map{source:source_id.into(),page:page_index,request,reply})
    }
    fn paragraph_request<T>(&self,request:impl FnOnce(mpsc::Sender<EngineResult<T>>)->Request)->EngineResult<T>{
        let (reply,receive)=mpsc::channel();self.send(WorkerRequest::OccurrenceParagraph(request(reply)))?;
        receive.recv().map_err(|_|EngineError::WorkerStopped)?
    }
    pub fn inspect_occurrence_text_paragraph(&self,source_id:&str,page_index:usize,indices:&[usize])->EngineResult<TextParagraphSelection>{
        self.in_points(source_id,page_index,self.inspect_occurrence_text_raw(source_id,page_index,indices))
    }
    pub(super) fn inspect_occurrence_text_raw(&self,source_id:&str,page_index:usize,indices:&[usize])->EngineResult<TextParagraphSelection>{
        self.paragraph_request(|reply|Request::InspectText{source:source_id.into(),page:page_index,indices:indices.to_vec(),reply})
    }
    pub fn suggest_occurrence_text_paragraph(&self,source_id:&str,page_index:usize,index:usize,expected:&str)->EngineResult<TextParagraphSelection>{
        self.in_points(source_id,page_index,self.paragraph_request(|reply|Request::SuggestText{source:source_id.into(),page:page_index,index,expected:expected.into(),reply}))
    }
    pub fn inspect_occurrence_styled_paragraph(&self,source_id:&str,page_index:usize,indices:&[usize])->EngineResult<StyledParagraphSelection>{
        self.in_points(source_id,page_index,self.inspect_occurrence_styled_raw(source_id,page_index,indices))
    }
    pub(super) fn inspect_occurrence_styled_raw(&self,source_id:&str,page_index:usize,indices:&[usize])->EngineResult<StyledParagraphSelection>{
        self.paragraph_request(|reply|Request::InspectStyled{source:source_id.into(),page:page_index,indices:indices.to_vec(),reply})
    }
    pub fn suggest_occurrence_styled_paragraph(&self,source_id:&str,page_index:usize,index:usize,expected:&str)->EngineResult<StyledParagraphSelection>{
        self.in_points(source_id,page_index,self.paragraph_request(|reply|Request::SuggestStyled{source:source_id.into(),page:page_index,index,expected:expected.into(),reply}))
    }
    /// Like the legacy layout API, the native diagnostic layout uses raw PDF axes.
    pub fn layout_occurrence_styled_paragraph(&self,source_id:&str,page_index:usize,request:&StyledParagraphLayoutRequest)->EngineResult<StyledParagraphLayout>{
        let request=self.styled_in_raw_mode(source_id,page_index,request,true)?;
        self.paragraph_request(|reply|Request::Layout{source:source_id.into(),page:page_index,request,reply})
    }
    pub fn fit_occurrence_styled_paragraph(&self,source_id:&str,page_index:usize,request:&StyledParagraphLayoutRequest)->EngineResult<AnnotationRect>{
        let request=self.styled_in_raw_mode(source_id,page_index,request,true)?;
        self.in_points(source_id,page_index,self.paragraph_request(|reply|Request::Fit{source:source_id.into(),page:page_index,request,reply}))
    }
    pub fn preview_text_paragraph_occurrence(&self,source_id:&str,occurrences:&[usize],selected:usize,indices:&[usize],expected:&str,replacement:&str,bounds:&AnnotationRect,formatting:&ParagraphFormatting)->EngineResult<PageContentReplacement>{
        let page=selected_page(occurrences,selected)?;
        if formatting.font_size.is_some_and(|size|!physical_units::paragraph_size(size,1.)){
            return Err(EngineError::InvalidRequest("Use a font size from 1 to 300 pt.".into()));
        }
        let (bounds,formatting)=self.uniform_in_raw_mode(source_id,page,indices,bounds,formatting,true)?;
        let mut result=self.paragraph_request(|reply|Request::PreviewText{source:source_id.into(),occurrences:occurrences.to_vec(),selected,indices:indices.to_vec(),expected:expected.into(),replacement:replacement.into(),bounds,formatting,reply})?;
        result.document=self.document_in_points(Ok(result.document))?;Ok(result)
    }
    pub fn preview_styled_paragraph_occurrence(&self,source_id:&str,occurrences:&[usize],selected:usize,request:&StyledParagraphLayoutRequest)->EngineResult<PageContentReplacement>{
        let page=selected_page(occurrences,selected)?;
        let request=self.styled_in_raw_mode(source_id,page,request,true)?;
        let mut result=self.paragraph_request(|reply|Request::PreviewStyled{source:source_id.into(),occurrences:occurrences.to_vec(),selected,request,reply})?;
        result.document=self.document_in_points(Ok(result.document))?;Ok(result)
    }
}

impl WorkerRuntime {
    // Original ownership lives only for this synchronous, private projection.
    // Generic form/group/image projections never receive paragraph authority.
    fn with_paragraph_projection<T>(&mut self,source:&str,page:usize,use_page:impl FnOnce(&mut Self,&str)->EngineResult<T>)->EngineResult<T> {
        let original=self.editing_document(source)?;
        let native=original.pdf()?.pages().get(page_index_i32(page)?)?;
        let context=text_edit::ParagraphSourcePage::read(original.bytes()?.as_ref(),page,&native)?.map(Arc::new);
        drop(native);
        self.with_text_projection(source,page,move|rt,id|{
            if let Some(context)=context {rt.paragraph_contexts.insert(id.into(),context);}
            let result=use_page(rt,id);rt.paragraph_contexts.remove(id);result
        })
    }

    pub(super) fn handle_occurrence_paragraph(&mut self,request:Request){
        match request {
            Request::Map{source,page,request,reply}=>{
                let result=self.with_paragraph_projection(&source,page,|rt,id|rt.map_paragraph_draft_inner(id,0,&request))
                    .map(|mut mapping|{mapping.source_id=source;mapping.page_index=page;mapping});
                let _=reply.send(result);
            }
            Request::InspectText{source,page,indices,reply}=>{let _=reply.send(self.with_paragraph_projection(&source,page,|rt,id|rt.inspect_text_paragraph(id,0,&indices)));}
            Request::SuggestText{source,page,index,expected,reply}=>{let _=reply.send(self.with_paragraph_projection(&source,page,|rt,id|rt.suggest_text_paragraph(id,0,index,&expected)));}
            Request::InspectStyled{source,page,indices,reply}=>{let _=reply.send(self.with_paragraph_projection(&source,page,|rt,id|rt.inspect_styled_paragraph(id,0,&indices)));}
            Request::SuggestStyled{source,page,index,expected,reply}=>{let _=reply.send(self.with_paragraph_projection(&source,page,|rt,id|rt.suggest_styled_paragraph(id,0,index,&expected)));}
            Request::Layout{source,page,request,reply}=>{let _=reply.send(self.with_paragraph_projection(&source,page,|rt,id|rt.layout_styled_paragraph(id,0,&request)));}
            Request::Fit{source,page,request,reply}=>{let _=reply.send(self.with_paragraph_projection(&source,page,|rt,id|rt.fit_styled_paragraph(id,0,&request)));}
            Request::PreviewText{source,occurrences,selected,indices,expected,replacement,bounds,formatting,reply}=>{
                let result=(|| {
                    let proof=self.paragraph_text_context(&source,selected_page(&occurrences,selected)?,&indices,false)?;
                    self.publish_context_text_occurrence(&source,&occurrences,selected,proof.as_ref(),|rt,page|
                        rt.with_paragraph_projection(&source,page,|rt,id|rt.preview_text_paragraph(id,0,&indices,&expected,&replacement,&bounds,&formatting)))
                })();
                if let Err(mpsc::SendError(Ok(result)))=reply.send(result){self.discard_unpublished_source(&result.document.id);}
            }
            Request::PreviewStyled{source,occurrences,selected,request,reply}=>{
                let result=(|| {
                    let proof=self.paragraph_text_context(&source,selected_page(&occurrences,selected)?,&request.object_indices,true)?;
                    self.publish_context_text_occurrence(&source,&occurrences,selected,proof.as_ref(),|rt,page|
                        rt.with_paragraph_projection(&source,page,|rt,id|rt.preview_styled_paragraph(id,0,&request)))
                })();
                if let Err(mpsc::SendError(Ok(result)))=reply.send(result){self.discard_unpublished_source(&result.document.id);}
            }
        }
    }
}
