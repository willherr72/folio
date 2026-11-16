//! One bounded source-font page; this probe is never installed in a document.
use super::*;
use crate::form_appearance::Copier;
use lopdf::dictionary;

const PROBE_LIMIT:usize=32*1024*1024;

pub(super) fn verify(pdfium:&Pdfium,source:&Document,resources:&Dictionary,bounds:[f64;4],layout:&Layout)->EngineResult<()> {
    if layout.characters>MAX_CHARACTERS||layout.operations.iter().any(|op|op.operator=="gs"){
        return Err(bad("source list native probe has unsupported state or excess characters"));
    }
    let mut probe=Document::with_version("1.7");
    let table=resolve(source,resources.get(b"Font").map_err(|_|bad("missing source list fonts"))?)?
        .as_dict().map_err(|_|bad("invalid source list fonts"))?;
    let mut fonts=Dictionary::new();
    {
        let mut copy=Copier::with_byte_limit(source,&mut probe,PROBE_LIMIT);
        for name in &layout.fonts {
            let font=table.get(name).map_err(|_|bad("missing source list probe font"))?;
            copy.reserve_bytes(name.len()+128)?;
            fonts.set(name.clone(),copy.copy(font)?);
        }
    }
    let width=bounds[2]-bounds[0];let height=bounds[3]-bounds[1];
    if width>1_000_000.||height>1_000_000. {return Err(bad("source list probe dimensions exceed their bound"));}
    let mut working=crate::form_graph::CopyBudget::with_byte_limit(PROBE_LIMIT);
    for op in &layout.operations {
        working.reserve_bytes(std::mem::size_of::<Operation>()+op.operator.len())?;
        working.array(&op.operands)?;
    }
    let mut operations=vec![Operation::new("q",vec![]),Operation::new("cm",vec![1.into(),0.into(),0.into(),1.into(),Object::Real(-bounds[0] as f32),Object::Real(-bounds[1] as f32)])];
    for(index,op)in layout.operations.iter().enumerate(){
        match op.operator.as_str(){
            // Consume every path while retaining W/W* and graphics/text state.
            "S"|"s"|"f"|"F"|"f*"|"B"|"B*"|"b"|"b*"=>operations.push(Operation::new("n",vec![])),
            "Do"|"sh"=>{},
            "Tf" if !op.operands.first().and_then(|v|v.as_name().ok()).is_some_and(|name|layout.fonts.contains(name))=>{},
            "Tj"|"TJ"|"'"|"\"" if !layout.marked.contains(&index)=>{},
            _=>operations.push(op.clone()),
        }
    }
    operations.push(Operation::new("Q",vec![]));
    let body=encode_bounded(&operations)?;
    let content=probe.add_object(Stream::new(Dictionary::new(),body));
    let pages=probe.new_object_id();
    let page=probe.add_object(dictionary!{"Type"=>"Page","Parent"=>pages,
        "MediaBox"=>vec![0.into(),0.into(),Object::Real(width as f32),Object::Real(height as f32)],
        "Resources"=>dictionary!{"Font"=>fonts},"Contents"=>content});
    probe.objects.insert(pages,dictionary!{"Type"=>"Pages","Kids"=>vec![page.into()],"Count"=>1}.into());
    let catalog=probe.add_object(dictionary!{"Type"=>"Catalog","Pages"=>pages});probe.trailer.set("Root",catalog);
    let mut writer=crate::pdf_output::BoundedWriter::new(PROBE_LIMIT);
    let saved=probe.save_to(&mut writer);let bytes=writer.finish()?;saved.map_err(|_|bad("source list probe serialization failed"))?;
    drop(probe);drop(operations);
    let native=pdfium.load_pdf_from_byte_slice(&bytes,None)?;
    let page=native.pages().get(0)?;
    let glyphs=crate::engine::list_source_glyphs(&page,MAX_CHARACTERS)?;
    if glyphs.len()!=layout.characters {return Err(bad("source list native glyph count does not match its labels"));}
    let mut offset=0;
    for(row_position,row)in layout.rows.iter().enumerate(){
        let mut visible=false;
        for(character_position,expected)in row.label.chars().enumerate(){
            let(text,glyph)=&glyphs[offset];offset+=1;
            if text.chars().next()!=Some(expected)||text.chars().count()!=1||glyph.iter().any(|v|!v.is_finite()) {
                return Err(bad("source list native glyph does not match its bound font label"));
            }
            let origin=point(layout.basis,glyph[0] as f64+bounds[0],glyph[1] as f64+bounds[1]);
            if !near(origin[1],row.origin[1])||character_position==0&&!near(origin[0],row.origin[0]){
                return Err(bad("source list native glyph origin differs from its row"));
            }
            let raw=[glyph[2] as f64+bounds[0],glyph[3] as f64+bounds[1],(glyph[4]-glyph[2]) as f64,(glyph[5]-glyph[3]) as f64];
            if raw[2]<=0.||raw[3]<=0. {if expected.is_whitespace(){continue;}return Err(bad("source list label contains a nonpainting glyph"));}
            let ink=rectangle(raw,layout.basis)?;
            let Ok(ink)=intersection(ink,layout.clip)else{continue;};
            visible=true;
            for paint in &layout.paints {
                if paint.rows.contains(&row_position){
                    let horizontal=extent_tolerance(ink[2]-ink[0]);
                    let vertical=extent_tolerance(ink[3]-ink[1]);
                    if ink[0]<paint.rectangle[0]-horizontal||ink[1]<paint.rectangle[1]-vertical
                        ||ink[2]>paint.rectangle[2]+horizontal||ink[3]>paint.rectangle[3]+vertical{
                        return Err(bad("source list selection does not contain its selected glyph ink"));
                    }
                }else if intersection(ink,paint.rectangle).is_ok(){
                    return Err(bad("source list selection overlaps an unselected row"));
                }
            }
        }
        if !visible&&layout.paints.iter().any(|paint|paint.rows.contains(&row_position)){
            return Err(bad("source list selected row has no visible native ink"));
        }
    }
    Ok(())
}
