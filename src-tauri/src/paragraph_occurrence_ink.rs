//! Measure and reserve physical glyphs; Unicode expansion never adds advances.
use super::*;
use styled_paragraph_layout::{MeasuredOccurrence, OccurrenceLayout};
use styled_paragraph_plan::ResolvedStyle;
use paragraph_highlight_geometry::Background;

impl WorkerRuntime {
    pub(super) fn native_occurrence_ink(&self,p:&Prepared,styles:&[ResolvedStyle],atoms:&mut [MeasuredOccurrence],include_unmarked:bool)->EngineResult<()> {
        let mut pdf=p.pdf.clone();let mut bound=styles.to_vec();
        styled_paragraph_publish::install_substitutes(&mut pdf,p.page_id,&p.resources,&mut bound)?;
        let template=styled_paragraph_native_metrics::metric_page_template(&pdf,p.page_id)?;
        let fonts=pdf.dereference(template.get(b"Resources").map_err(|_|invalid("Missing occurrence metric resources."))?)
            .and_then(|(_,r)|r.as_dict()).and_then(|r|r.get(b"Font")).and_then(|f|pdf.dereference(f)).and_then(|(_,f)|f.as_dict())
            .map_err(|_|invalid("Invalid occurrence metric fonts."))?.clone();
        let pages=pdf.new_object_id();let mut kids=vec![];let mut probes=vec![];let mut content_bytes=0usize;
        for (id,style) in bound.iter().enumerate().filter(|(_,s)|include_unmarked||s.background_id.is_some()) {
            let mut seen=HashSet::new();let selected=atoms.iter().enumerate().filter(|(_,a)|a.style_id==id&&!a.text.chars().all(char::is_whitespace))
                .filter(|(_,a)|seen.insert(a.encoded.clone())).map(|(i,_)|i).collect::<Vec<_>>();
            if selected.is_empty(){continue;}
            let bytes=selected.iter().flat_map(|i|atoms[*i].encoded.iter().copied()).collect::<Vec<_>>();
            let ops=vec![Operation::new("q",vec![]),Operation::new("BT",vec![]),Operation::new("Tf",vec![Object::Name(style.resource_name.clone()),style.size.into()]),
                Operation::new("Tc",vec![0.into()]),Operation::new("Tw",vec![0.into()]),Operation::new("Tz",vec![100.into()]),Operation::new("Ts",vec![0.into()]),Operation::new("Tr",vec![0.into()]),
                Operation::new("Tm",vec![1.into(),0.into(),0.into(),1.into(),0.into(),0.into()]),Operation::new("Tj",vec![Object::String(bytes,StringFormat::Hexadecimal)]),Operation::new("ET",vec![]),Operation::new("Q",vec![])];
            let data=Content{operations:ops}.encode().map_err(|_|invalid("Occurrence metrics could not be encoded."))?;
            content_bytes=content_bytes.checked_add(data.len()).filter(|n|*n<=LIMIT).ok_or_else(||invalid("Occurrence metrics exceed the content budget."))?;
            let content=pdf.add_object(Stream::new(dictionary!{},data));let mut page=template.clone();page.set("Parent",pages);page.set("Contents",content);
            page.set("Resources",dictionary!{"Font"=>{let mut d=lopdf::Dictionary::new();d.set(style.resource_name.clone(),fonts.get(&style.resource_name).map_err(|_|invalid("Missing occurrence font."))?.clone());d}});
            kids.push(Object::Reference(pdf.add_object(page)));probes.push((id,selected));
        }
        if probes.is_empty(){return Ok(());}
        pdf.objects.insert(pages,dictionary!{"Type"=>"Pages","Count"=>kids.len() as i64,"Kids"=>kids}.into());
        pdf.catalog_mut().map_err(|_|invalid("Missing occurrence metric catalog."))?.set("Pages",pages);
        let mut bytes=vec![];pdf.save_to(&mut bytes).map_err(|_|invalid("Occurrence metrics could not be saved."))?;
        if bytes.len() as u64>MAX_SOURCE_BYTES{return Err(invalid("Occurrence metrics exceed the source budget."));}
        let document=self.pdfium.load_pdf_from_reader(Cursor::new(bytes),None)?;
        if document.pages().len() as usize!=probes.len(){return Err(invalid("Occurrence metric pages changed."));}
        let mut measured=HashMap::new();
        for (index,(id,selected)) in probes.iter().enumerate() {
            let page=document.pages().get(index as i32)?;if page.objects().len()!=1{return Err(invalid("Ambiguous occurrence metric ownership."));}
            let glyphs=page_object_glyphs(&page)?;let glyphs=glyphs.get(&0).ok_or_else(||invalid("Missing occurrence glyphs."))?;
            let mut cursor=0;let mut x=0.;
            for i in selected {
                let atom=&atoms[*i];let mut bounds:Option<[f32;4]>=None;
                for ch in atom.text.chars() {
                    let glyph=glyphs.get(cursor).ok_or_else(||invalid("Missing expanded source glyph."))?;cursor+=1;
                    if glyph.text!=ch.to_string()||!close_values(&glyph.position[..2],&[x,0.])||glyph.position.iter().any(|v|!v.is_finite()) {return Err(invalid("Occurrence glyph geometry differs from its encoded source."));}
                    let p=glyph.position;let ink=[p[2]-p[0],p[3]-p[1],p[4]-p[0],p[5]-p[1]];
                    if ink.iter().any(|v|v.abs()>100_000.)||ink[2]<ink[0]||ink[3]<ink[1]{return Err(invalid("Invalid occurrence ink."));}
                    bounds=Some(bounds.map_or(ink,|b|[b[0].min(ink[0]),b[1].min(ink[1]),b[2].max(ink[2]),b[3].max(ink[3])]));
                }
                measured.insert((*id,atom.encoded.clone()),bounds.ok_or_else(||invalid("Empty physical source glyph."))?);
                x+=atom.metric.advance;if !x.is_finite()||x.abs()>1_000_000.{return Err(invalid("Occurrence metric cursor exceeds its budget."));}
            }
            if cursor!=glyphs.len(){return Err(invalid("Unexpected native occurrence glyphs."));}
        }
        for atom in atoms {if let Some(ink)=measured.get(&(atom.style_id,atom.encoded.clone())){atom.metric.ink=*ink;atom.reservation=atom.metric;}}
        Ok(())
    }
}

pub(super) fn reserve(atoms:&mut [MeasuredOccurrence],styles:&[ResolvedStyle],backgrounds:&[Background])->EngineResult<()> {
    if !paragraph_highlight_geometry::valid_backgrounds(backgrounds){return Err(invalid("Invalid occurrence backgrounds."));}
    for atom in atoms {
        atom.reservation=atom.metric;
        let style=styles.get(atom.style_id).ok_or_else(||invalid("Missing occurrence style."))?;
        let b=style.background_id.map(|id|backgrounds.get(id).ok_or_else(||invalid("Missing occurrence background."))).transpose()?;
        let ink=atom.metric.ink;
        if !atom.text.chars().all(char::is_whitespace)&&ink[2]>ink[0]&&ink[3]>ink[1] {if let Some(b)=b {
            let paint=if let Some(cell)=b.cell{cell.paint(0.,atom.metric.advance,ink,style.size)?}else{[ink[0]-b.padding[0],ink[1]-b.padding[1],ink[2]+b.padding[2],ink[3]+b.padding[3]]};
            atom.reservation.ink=[ink[0].min(paint[0]),ink[1].min(paint[1]),ink[2].max(paint[2]),ink[3].max(paint[3])];
        }}
    }
    Ok(())
}

pub(super) fn validate_layout(result:&OccurrenceLayout,atoms:&[MeasuredOccurrence],styles:&[ResolvedStyle],backgrounds:&[Background])->EngineResult<()> {
    if backgrounds.is_empty(){return Ok(());}
    let mut fragments=vec![];
    for placed in &result.fragments {
        let mut ink=vec![];
        for item in placed {
            let atom=atoms.get(item.ordinal).ok_or_else(||invalid("Missing placed occurrence."))?;
            let m=atom.metric;let s=&styles[atom.style_id];let [x,y]=item.origin;
            if !atom.text.chars().all(char::is_whitespace)&&m.ink[2]>m.ink[0]&&m.ink[3]>m.ink[1] {
                let bounds=[x+m.ink[0],y+m.ink[1],x+m.ink[2],y+m.ink[3]];
                let paint=if let Some(cell)=s.background_id.and_then(|id|backgrounds[id].cell){cell.paint(x,m.advance,bounds,s.size)?}else{bounds};
                ink.push((bounds,paint));
            }
        }
        fragments.push(ink);
    }
    let ids=styles.iter().map(|s|s.background_id).collect::<Vec<_>>();
    paragraph_highlight_geometry::rectangles_from_ink(&result.layout,&fragments,&ids,backgrounds)?;Ok(())
}
