//! Private full-source page views for pixels/text, never editable PDF output.
use super::*;
use super::imported_overlay_sources::publication::{OwnedChange,PageChanges};
use crate::composition_annotation_receipts::Receipt;

pub(super) struct ReadPage {
    pub bytes: Arc<[u8]>,
    pub page_index: usize,
    pub unit: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_recovered_drawings_remain_readable_without_granting_ownership(){
        let mut rt=crate::engine::structured_overlay_render::tests::runtime();
        let info=rt.create_blank_document(300.,300.).unwrap();
        let mut plan:PagePlan=serde_json::from_value(serde_json::json!({"id":"page","sourceId":info.id,"pageIndex":0,"width":300,"height":300,"rotation":0,"overlays":[{"type":"text","id":"note","x":30,"y":60,"text":"Recovered note","fontSize":12,"color":"#0000ff"}]})).unwrap();
        let original=rt.compose_pdf(ExportRequest{pages:vec![plan.clone()],flatten:false},Some(vec![]),CompositionIntent::Save).unwrap();
        let path=std::env::temp_dir().join(format!("folio-legacy-read-{}.pdf",Uuid::new_v4()));
        let info=rt.open_bytes(original.clone(),path.clone(),path,true,true,false,OpenSecurity::Restored(None)).unwrap();
        let info=rt.restore_annotation_sources(info,None).unwrap();assert_eq!(info.pages[0].overlays.len(),1);
        plan.source_id=info.id.clone();plan.overlays=info.pages[0].overlays.clone();
        for deleted in [false,true]{
            let mut draft=plan.clone();if deleted{draft.overlays.clear();}
            for intent in [CompositionIntent::Preview,CompositionIntent::ImageExport,CompositionIntent::TextExport]{
                let view=rt.prepare_read_page(draft.clone(),intent,Arc::new(||false)).unwrap();
                let doc=rt.pdfium.load_pdf_from_reader(Cursor::new(view.bytes),None).unwrap();
                if intent==CompositionIntent::TextExport{
                    let text=rt.read_document_page_text(&doc,view.page_index,None).unwrap();
                    let text=text.characters.iter().map(|c|c.text.as_str()).collect::<String>();
                    assert_eq!(text.contains("Recovered note"),!deleted);
                }else{
                    let page=doc.pages().get(page_index_i32(view.page_index).unwrap()).unwrap();
                    let rgb=page.render_with_config(&PdfRenderConfig::new().set_target_width(300).render_form_data(true)).unwrap().as_image().unwrap().into_rgb8();
                    assert_eq!(rgb.pixels().any(|p|p[2]>180&&p[0]<100&&p[1]<100),!deleted);
                }
                let source=rt.document(&info.id).unwrap();assert!(!source.imported_overlay_sources.bound_to(source.source_bytes.identity()));assert_eq!(source.bytes().unwrap().as_ref(),original.as_slice());
            }
        }
    }
}

impl WorkerRuntime {
    pub(super) fn prepare_read_page(
        &mut self, plan: PagePlan, intent: CompositionIntent,
        cancelled: Arc<dyn Fn()->bool+Send+Sync>,
    )->EngineResult<ReadPage> {
        if !matches!(intent,CompositionIntent::Preview|CompositionIntent::ImageExport|CompositionIntent::TextExport) {
            return Err(EngineError::InvalidRequest("Original page views are read-only.".into()));
        }
        let mut check=||if cancelled(){Err(EngineError::InvalidRequest("Page export cancelled.".into()))}else{Ok(())};
        check()?;
        let request=ExportRequest{pages:vec![plan],flatten:intent==CompositionIntent::TextExport};
        self.validate_export(&request)?;
        self.authorize_composition(&request,&[],intent)?;
        let plan=&request.pages[0];
        let source=self.document(&plan.source_id)?;
        let unit=self.source_page_unit(&plan.source_id,plan.page_index)?;
        let page_index=plan.page_index;
        // This fast path is safe only if no editable source annotations were
        // removed at open. An empty draft can otherwise mean deliberate deletion.
        let imported=source.imported_annotations.get(&page_index).is_some_and(|v|!v.is_empty());
        if imported && !source.imported_overlay_sources.bound_to(source.source_bytes.identity()) {
            // Legacy recovery intentionally has no source publication authority.
            // Preserve its established visual read path without granting any.
            check()?;
            return self.composed_read_page(request,intent,unit);
        }
        if !imported && plan.overlays.is_empty() && plan.crop.is_none() && plan.rotation==0 {
            check()?;
            return Ok(ReadPage{bytes:source.bytes()?,page_index,unit});
        }
        let geometry=page_geometry(&source.pdf()?.pages().get(page_index_i32(page_index)?)?)?;
        let mut graph=crate::composition_graph::CompositionGraph::new();
        graph.import_bytes(0,source.bytes()?.as_ref(),cancelled.clone())?;
        let pages=match graph.source_pages(0,&mut check) {
            Ok(pages)=>pages,
            Err(_)=>{
                // The graph-native ownership route deliberately rejects repeated
                // leaves and unusual trees. Retain the existing guarded page-copy
                // read path for those sources; never project them as a full view.
                check()?;drop(graph);
                return self.composed_read_page(request,intent,unit);
            }
        };
        let page=*pages.get(page_index).ok_or_else(||EngineError::InvalidRequest("Original page mapping differs.".into()))?;
        if pages.iter().filter(|id|**id==page).count()!=1 {
            check()?;drop(graph);
            return self.composed_read_page(request,intent,unit);
        }
        let mut document=if !imported && plan.overlays.is_empty() {
            graph.into_single_source(&mut check)?
        }else{
            // A private read view never authors structure/language metadata.
            // The original catalog remains intact, including unrelated actions.
            let language=crate::composition_annotation_additions::SourceLanguage::default();
            // Blank/new sources have no imported ownership index. Their current
            // drawings are additions, not replacements of source annotations.
            let mut publication=if imported {Some(source.imported_overlay_sources.publication(source.source_bytes.identity(),pages.len(),&mut check)?)}else{None};
            let changes=if let Some(publication)=&mut publication {
                publication.page(page_index,&plan.overlays,&mut check)?
            }else{PageChanges{owned:vec![],added:plan.overlays.iter().collect(),draft:&plan.overlays}};
            let slots=changes.owned.iter().map(|change|match change {
                OwnedChange::Keep(s,_)|OwnedChange::Replace(s,_)|OwnedChange::Delete(s)=>s.source_group(),
            }).collect::<Vec<_>>();
            let groups=graph.source_annotation_groups(0,&slots,&mut check)?;
            // No page projection: the validated source anchors are the output
            // anchors. Receipts still undergo normal slot/identity verification.
            let receipts=groups.iter().flat_map(|g|std::iter::once(g.annotation).chain(g.popups.iter().copied()))
                .map(|origin|Receipt{origin,output_page:origin.page,output_slot:origin.slot,annotation:origin.annotation}).collect::<Vec<_>>();
            let document=graph.into_single_source(&mut check)?;
            let prepared=self.prepare_structured_overlay_changes(&request,&[&changes],&mut check)?;
            let target=structured_overlay_publish::TargetPage{page_id:page,geometry,unit};
            let change=structured_overlay_batch::PagePlan{source_page:page,source_page_index:page_index,source_language:&language,changes:&changes,groups:&groups};
            structured_overlay_batch::publish_read_view(document,&[target],&[change],&receipts,prepared,request.flatten,&mut check)?.document
        };
        if plan.rotation!=0 {
            document.get_dictionary_mut(page).map_err(|e|EngineError::InvalidRequest(e.to_string()))?
                .set("Rotate",i64::from((geometry.rotation+plan.rotation)%360));
        }
        page_crop::apply_page(&mut document,page,plan,&geometry)?;
        crate::composition_graph::validate_retained(&document,&mut check)?;
        let bytes=crate::pdf_output::save_checked(&mut document,&mut check)?;
        check()?;
        Ok(ReadPage{bytes:bytes.into(),page_index,unit})
    }

    fn composed_read_page(&mut self,request:ExportRequest,intent:CompositionIntent,unit:f32)->EngineResult<ReadPage>{
        let bytes=self.compose_pdf(request,Some(vec![]),intent)?;
        Ok(ReadPage{bytes:bytes.into(),page_index:0,unit})
    }
}
