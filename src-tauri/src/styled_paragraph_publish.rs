//! Emit checked source-font fragments, then verify reopened output before admission.
use super::*;
use styled_paragraph_plan::{ResolvedPlan,ResolvedStyle};

pub(super) fn install_substitutes(pdf:&mut Document,page_id:lopdf::ObjectId,source_resources:&lopdf::Dictionary,styles:&mut [ResolvedStyle])->EngineResult<()> {
    let mut installed=HashMap::<String,Vec<u8>>::new();
    let mut resources=source_resources.clone();
    for style in styles.iter_mut(){
        if let Some(asset)=&style.substitute {
            style.resource_name=if let Some(name)=installed.get(&asset.info.id){name.clone()}else{
                let name=paragraph_font::install_integral(pdf,page_id,&resources,asset)?;
                // install clones/replaces Resources. Carry the new dictionary
                // forward so a second imported font cannot discard the first.
                resources=pdf.get_dictionary(page_id).and_then(|d|d.get(b"Resources")).and_then(Object::as_dict)
                    .map_err(|_|invalid("Missing installed paragraph resources."))?.clone();
                installed.insert(asset.info.id.clone(),name.clone());name
            };
        }
    }
    Ok(())
}
pub(super) struct Emission{pub bytes:Vec<u8>,pub block_start:usize}
pub(super) fn fragment_range(prepared:&Prepared,count:usize,total:usize)->EngineResult<std::ops::Range<usize>>{
    let first=*prepared.selection.object_indices.first().ok_or_else(||invalid("Missing paragraph source span."))?;
    let start=prepared.before.iter().take_while(|run|run.index<first).count();
    let end=start.checked_add(count).filter(|end|*end<=total).ok_or_else(||invalid("Missing emitted paragraph fragments."))?;
    Ok(start..end)
}
pub(super) fn emit_located(prepared:&mut Prepared,layout:&StyledParagraphLayout,styles:&mut [ResolvedStyle])->EngineResult<Emission> {
    install_substitutes(&mut prepared.pdf,prepared.page_id,&prepared.resources,styles)?;
    let mut anchor=prepared.removals.first().ok_or_else(||invalid("Missing paragraph source show."))?.0;
    let leading_adjustment=styled_paragraph_splice::leading_adjustment(&prepared.content.operations[anchor])?;
    let mut discard=vec![false;prepared.content.operations.len()];
    for mark in &prepared.highlight_sources {
        let index=mark.paint_operator;
        if index>=prepared.content.operations.len(){return Err(invalid("Missing original highlight paint."));}
        if prepared.remembered {
            let start=index.checked_sub(3).ok_or_else(||invalid("Invalid owned highlight block."))?;
            let ops=prepared.content.operations.get(start..index+2).ok_or_else(||invalid("Missing owned highlight block."))?;
            if ops.iter().zip(["q","rg","re","f","Q"]).any(|(op,name)|op.operator!=name){return Err(invalid("Invalid owned highlight block."));}
            discard[start..index+2].fill(true);
        }else{
            // Keep path construction/state; replace only the proven fill with
            // an end-path operation. Arbitrary surrounding operators are not owned.
            prepared.content.operations[index]=Operation::new("n",vec![]);
        }
    }
    const OWNED_BLOCK:[&str;13]=["q","BT","Tf","Tc","Tw","Tz","Ts","Tr","rg","Tm","Tj","ET","Q"];
    for &(index,distance) in &prepared.removals {
        // A verified saved fragment has its own balanced q/BT/.../ET/Q block.
        // Remove that complete self-contained block, not just its show, so
        // repeated editing cannot accumulate obsolete state operators forever.
        let block=index.checked_sub(10).and_then(|start|prepared.content.operations.get(start..index+3).map(|ops|(start,ops)));
        if let Some((start,_))=block.filter(|(_,ops)|prepared.remembered&&ops.iter().zip(OWNED_BLOCK).all(|(op,name)|op.operator==name)){
            if index==prepared.removals[0].0{anchor=start;}
            discard[start..index+3].fill(true);
        }else{prepared.content.operations[index]=Operation::new("TJ",vec![Object::Array(vec![Object::Real(distance)])]);}
    }
    if prepared.remembered {
        if let Some(context)=prepared.context.as_ref().filter(|c|c.has_clip()) {
            // Only the independently bound canonical fragment/paint blocks are
            // owned. Prove the exact surrounding recipe and a wholly owned
            // interior before removing the envelope on a repeated edit.
            let start=discard.iter().position(|v|*v).ok_or_else(||invalid("Missing owned paragraph envelope."))?;
            let end=discard.iter().rposition(|v|*v).unwrap()+1;
            let ops=&prepared.content.operations;
            let outer=context.owned_clip_prefix_start(ops,start)?;
            if !discard[start..end].iter().all(|v|*v)
                ||!ops.get(end).is_some_and(|op|op.operator=="Q"&&op.operands.is_empty()) {
                return Err(invalid("The saved paragraph clip envelope is not independently owned."));
            }
            discard[outer..=end].fill(true);anchor=outer;
        }
    }
    anchor=discard[..anchor].iter().filter(|discard|!**discard).count();
    let mut ordinal=0;prepared.content.operations.retain(|_|{let keep=!discard[ordinal];ordinal+=1;keep});
    let mut fragments=Vec::new();
    for fragment in layout.lines.iter().flat_map(|line|&line.runs) {
        let style=&styles[fragment.style_id];let size=style.size;
        let encoded=style.encode(&fragment.text)?;
        fragments.extend([
            Operation::new("q",vec![]),Operation::new("BT",vec![]),
            Operation::new("Tf",vec![Object::Name(style.resource_name.clone()),size.into()]),
            Operation::new("Tc",vec![0.into()]),Operation::new("Tw",vec![0.into()]),
            Operation::new("Tz",vec![100.into()]),Operation::new("Ts",vec![0.into()]),Operation::new("Tr",vec![0.into()]),
            Operation::new("rg",style.color[..3].iter().map(|c|Object::Real(*c as f32/255.)).collect()),
            Operation::new("Tm",vec![1.into(),0.into(),0.into(),1.into(),fragment.origin[0].into(),fragment.origin[1].into()]),
            Operation::new("Tj",vec![Object::String(encoded,StringFormat::Hexadecimal)]),
            Operation::new("ET",vec![]),Operation::new("Q",vec![]),
        ]);
    }
    let first=prepared.before.iter().find(|run|run.index==prepared.selection.object_indices[0]).ok_or_else(||invalid("Missing paragraph source anchor."))?;
    let (operations,block_start)=styled_paragraph_splice::insert(std::mem::take(&mut prepared.content.operations),anchor,fragments,first,leading_adjustment,prepared.context.as_ref().filter(|c|c.has_clip()))?;
    let data=Content{operations}.encode().map_err(|_|invalid("The styled paragraph could not be encoded."))?;
    if data.len()>LIMIT{return Err(invalid("The edited content exceeds the size limit."));}
    let content_id=prepared.pdf.add_object(Stream::new(dictionary!{},data));
    prepared.pdf.get_dictionary_mut(prepared.page_id).map_err(|_|invalid("Missing paragraph page."))?.set("Contents",content_id);
    let mut bytes=Vec::new();prepared.pdf.save_to(&mut bytes).map_err(|_|invalid("The styled paragraph could not be saved."))?;
    if bytes.len() as u64>MAX_SOURCE_BYTES{return Err(invalid("The edited page exceeds the source size limit."));}
    Ok(Emission{bytes,block_start})
}

fn verify_text(prepared:&Prepared,layout:&StyledParagraphLayout,styles:&[ResolvedStyle],page:&PdfPage<'_>,proofs:&FontProofs,bytes:&[u8],original_values:&HashMap<usize,String>)->EngineResult<()> {
    let after=bound_snapshot(page,Some(proofs),bytes)?;
    let values=source_text::values(bytes,&after).unwrap_or_default();
    let source_values:HashMap<_,_>=after.iter().zip(values).filter_map(|(run,value)|value.map(|value|(run.index,value))).collect();
    let remaining:Vec<_>=prepared.before.iter().filter(|r|!prepared.selection.object_indices.contains(&r.index)).collect();
    let fragments:Vec<_>=layout.lines.iter().flat_map(|line|&line.runs).collect();
    if after.len()!=remaining.len()+fragments.len(){return Err(invalid("The styled paragraph changed unexpected text objects."));}
    let range=fragment_range(prepared,fragments.len(),after.len())?;
    let all_glyphs=page_object_glyphs(page)?;
    for (old,new) in remaining.iter().zip(after[..range.start].iter().chain(&after[range.end..])){
        let mut adjusted=(*old).clone();adjusted.index=new.index;
        let old_glyphs=&prepared.glyphs[&old.index];let new_glyphs=&all_glyphs[&new.index];
        // Reader-generated separators may change when the edited paragraph moves.
        // An alternative to raw equality requires exact independently decoded
        // neighbor text on both sides; glyph, style, bounds and raster still bind.
        let source_matches=original_values.get(&old.index).is_some_and(|value|source_values.get(&new.index)==Some(value));
        let same_neighbor=same_run(&adjusted,new)||(source_matches&&same_style(&adjusted,new)&&close_values(&adjusted.bounds,&new.bounds));
        if !same_neighbor||old_glyphs.len()!=new_glyphs.len()
            ||!old_glyphs.iter().zip(new_glyphs).all(|(a,b)|a.text==b.text&&close_values(&a.position,&b.position)) {
            return Err(invalid("The styled paragraph would change surrounding text."));
        }
    }
    let actual=after[range].iter().collect::<Vec<_>>();
    verify_fragments(layout,styles,&actual,page,&source_values)
}

// Source values must be independently decoded from the saved operands, never
// copied from the requested layout or its persisted hints.
pub(super) fn verify_fragments(layout:&StyledParagraphLayout,styles:&[ResolvedStyle],actual:&[&RunSnapshot],page:&PdfPage<'_>,source_values:&HashMap<usize,String>)->EngineResult<()> {
    let fragments:Vec<_>=layout.lines.iter().flat_map(|line|&line.runs).collect();
    if fragments.len()!=actual.len(){return Err(invalid("The saved paragraph has unexpected fragments."));}
    let all_glyphs=page_object_glyphs(page)?;
    for (fragment,actual) in fragments.iter().zip(actual){
        let style=&styles[fragment.style_id];let expected=&style.source;let size=style.size;
        let font_matches=if let Some(asset)=&style.substitute {
            actual.font==format!("Folio{}Paragraph",asset.info.id)&&actual.program.as_deref()==Some(asset.info.id.as_str())&&actual.embedded
                &&actual.proof.as_ref().is_some_and(|p|p.encode(&fragment.text).is_ok_and(|bytes|bytes==paragraph_font::encode(&fragment.text)))
        }else{actual.font==expected.font&&actual.program==expected.program&&actual.proof==expected.proof&&actual.standard==expected.standard&&actual.embedded==expected.embedded};
        // PDFium can return an empty extraction for an encoded space-only show.
        // Admit only our known ASCII-space fragment, with all font/state/origin
        // checks below still mandatory; foreign empty source runs stay refused.
        let space_fragment=!fragment.text.is_empty()&&fragment.text.bytes().all(|b|b==b' ')&&actual.text.trim().is_empty();
        if paragraph_layout::extraction_text(&actual.text)!=paragraph_layout::extraction_text(&fragment.text)
            ||!font_matches||actual.color!=style.color
            ||actual.mode!=PdfPageTextRenderMode::FilledUnstroked
            ||(actual.font_size-size).abs()>EPSILON||!close_values(&actual.matrix[..4],&[1.,0.,0.,1.])
            ||!close_values(&actual.matrix[4..],&fragment.origin)
            ||(source_edit_run(actual,source_values.get(&actual.index).map(String::as_str)).reason.is_some()&&!whitespace_run(actual)&&!space_fragment) {
            return Err(invalid("The styled paragraph could not preserve its replacement text and formatting."));
        }
        let glyphs=&all_glyphs[&actual.index];
        let visible:Vec<_>=glyphs.iter().filter(|g|!g.text.trim().is_empty()).collect();
        let mut expected_glyphs=Vec::new();let mut x=fragment.origin[0];
        for ch in fragment.text.chars(){
            let value=ch.to_string();
            if !ch.is_whitespace(){expected_glyphs.push((value.clone(),[x,fragment.origin[1]]));}
            x+=style.advance(ch)?;
        }
        if visible.len()!=expected_glyphs.len()||!visible.iter().zip(&expected_glyphs).all(|(a,b)|a.text==b.0&&close_values(&a.position[..2],&b.1)) {
            return Err(invalid("The styled paragraph could not preserve its glyph positions."));
        }
        let bounds=layout.pdf_bounds;
        if visible.iter().any(|g|g.position.iter().any(|v|!v.is_finite())||g.position[2]<bounds[0]-EPSILON||g.position[3]<bounds[1]-EPSILON||g.position[4]>bounds[2]+EPSILON||g.position[5]>bounds[3]+EPSILON) {
            return Err(invalid("The styled paragraph text does not fit inside its area."));
        }
    }
    Ok(())
}

impl WorkerRuntime {
    pub(in crate::engine) fn preview_styled_paragraph(&mut self,source_id:&str,page_index:usize,request:&StyledParagraphLayoutRequest)->EngineResult<DocumentInfo> {
        let ResolvedPlan{mut prepared,original,layout,mut styles}=self.prepare_styled_paragraph(source_id,page_index,request)?;
        let geometry=prepared.geometry;let page_size=geometry.displayed_size();let new_bounds=layout.pdf_bounds;
        let mut changed_area=new_bounds;
        for mark in &prepared.highlight_sources {let r=mark.rectangle.bounds;changed_area=[changed_area[0].min(r[0]),changed_area[1].min(r[1]),changed_area[2].max(r[2]),changed_area[3].max(r[3])];}
        for run in &prepared.before {
            if request.object_indices.contains(&run.index){
                changed_area[0]=changed_area[0].min(run.bounds[0]);changed_area[1]=changed_area[1].min(run.bounds[1]);
                changed_area[2]=changed_area[2].max(run.bounds[2]);changed_area[3]=changed_area[3].max(run.bounds[3]);
            }else if !run.text.trim().is_empty()&&adds_overlap(&[0.;4],&new_bounds,&run.bounds){
                return Err(invalid("The paragraph area would overlap neighboring text."));
            }
        }
        {
            let copy=self.editable_page_copy(source_id,page_index)?;let page=copy.pages().get(0)?;
            for (index,object) in page.objects().iter().enumerate(){
                if object.as_text_object().is_some(){continue;}
                if prepared.highlight_sources.iter().any(|mark|mark.rectangle.object_index==index){continue;}
                if !(index<request.object_indices[0]&&solid_page_background(&object,geometry)?)
                    &&occupied_regions(&object,geometry)?.iter().any(|other|adds_overlap(&[0.;4],&changed_area,other)){
                    return Err(invalid("The paragraph overlaps graphics or highlighting. That selection is not supported yet."));
                }
            }
        }
        let values=source_text::values_on_page(&prepared.pdf,prepared.page_id,&prepared.before).unwrap_or_default();
        let original_values=prepared.before.iter().zip(values).filter_map(|(run,value)|value.map(|value|(run.index,value))).collect();
        styled_paragraph_memory::retain_unaffected(&mut prepared)?;
        let emitted=paragraph_highlight_publish::emit(self,&mut prepared,&layout,&mut styles)?;
        let bytes:Arc<[u8]>=styled_paragraph_memory::write(self,&emitted,&prepared,&layout,&styles,request)?.into();
        let bytes=self.preserve_text_edit_comments(source_id,page_index,bytes)?;
        let document=self.pdfium.load_pdf_from_reader(Cursor::new(bytes.clone()),None)?;
        {
            let page=document.pages().get(0)?;let proofs=FontProofs::read(&bytes)?;
            verify_text(&prepared,&layout,&styles,&page,&proofs,&bytes,&original_values)?;
            let after=snapshot(&page,Some(&proofs))?;let count=layout.lines.iter().map(|l|l.runs.len()).sum::<usize>();
            let ids=after[fragment_range(&prepared,count,after.len())?].iter().map(|r|r.index).collect::<Vec<_>>();
            let mut saved=prepare(&bytes,after,&ids,geometry,true)?;
            if !self.restore_styled_memory(&mut saved,&page)? {return Err(invalid("The saved paragraph settings could not be verified."));}
            let changed=raster(&page)?;
            if original.dimensions()!=changed.dimensions(){return Err(invalid("The paragraph changed the page dimensions."));}
            let sx=original.width() as f32/page_size.width;let sy=original.height() as f32/page_size.height;
            let area=displayed_bounds(geometry,changed_area);
            for (x,y,pixel) in original.enumerate_pixels(){
                if pixel!=changed.get_pixel(x,y)&&((x as f32)<area.x*sx-3.||(x as f32)>(area.x+area.width)*sx+3.||(y as f32)<area.y*sy-3.||(y as f32)>(area.y+area.height)*sy+3.){
                    return Err(invalid("The styled paragraph would change surrounding content's appearance."));
                }
            }
        }
        let source=self.editing_document(source_id)?;let path=source.path.clone();let original_path=source.original_path.clone();
        let policy=source.policy.as_ref().map(|p|p.page(page_index)).transpose()?;let protection=policy.as_ref().map(SourcePolicy::info);
        let name=path.file_name().and_then(|n|n.to_str()).unwrap_or("document.pdf").to_string();let id=Uuid::new_v4().to_string();
        let retained = self.source_store.capture(bytes.clone())?;
        self.documents.insert(id.clone(),OpenDocument{form_admission:form_admission::Admission::default(),physical_units:OnceLock::from(physical_units::read(&bytes,document.pages().len() as usize).map_err(|e|e.to_string())),legacy_comment_import:false,imported_annotations:Default::default(),unpublished_fonts:vec![],image_leases:None,
            path,original_path,document: native_residency::ResidentDocument::new(document, self.pdfium, retained.clone()),imported_overlay_sources:imported_overlay_sources::Index::unbound(retained.identity().clone()),source_bytes:retained,for_printing:false,print_max_dpi:None,policy,text_edit_restriction:OnceLock::new(),bookmarks:OnceLock::new()});
        Ok(DocumentInfo{id,name,pages:vec![page_size],bookmarks:vec![],bookmark_error:None,protection})
    }
}
