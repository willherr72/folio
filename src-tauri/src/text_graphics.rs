//! Conservative occupied regions for graphics near editable text.
use super::*;

pub(super) fn occupied_regions(
    object: &PdfPageObject<'_>,
    geometry: PageGeometry,
) -> EngineResult<Vec<[f32; 4]>> {
    let rect = object.bounds()?;
    let full = [rect.left().value, rect.bottom().value, rect.right().value, rect.top().value];
    if !valid(full) { return Err(invalid("The neighboring graphics have invalid bounds.")); }
    let Some(path) = object.as_path_object() else { return Ok(vec![full]); };
    if !path.is_stroked()? || path.fill_mode()? != PdfPathFillMode::None {
        return Ok(vec![full]);
    }
    let matrix = path.matrix()?;
    let values = [matrix.a(), matrix.b(), matrix.c(), matrix.d(), matrix.e(), matrix.f()];
    let width = path.stroke_width()?.value;
    if values.iter().any(|v| !v.is_finite()) || !width.is_finite() || width < 0.0 {
        return Ok(vec![full]);
    }
    let [a,b,c,d,_,_] = values.map(f64::from);
    if a*d-b*c == 0.0 { return Ok(vec![full]); }
    let segments = path.segments().transform(matrix);
    if !matches!(segments.len(), 4 | 5) { return Ok(vec![full]); }
    let mut points = Vec::with_capacity(5);
    for i in 0..segments.len() {
        let segment = segments.get(i)?;
        let expected = if i == 0 { PdfPathSegmentType::MoveTo } else { PdfPathSegmentType::LineTo };
        if segment.segment_type() != expected || segment.is_close() != (i == segments.len()-1) {
            return Ok(vec![full]);
        }
        let (x,y) = segment.point();
        if !x.value.is_finite() || !y.value.is_finite() { return Ok(vec![full]); }
        points.push([x.value,y.value]);
    }
    if points.len() == 5 && points[0] != points[4] { return Ok(vec![full]); }
    let mut horizontal = [false;4];
    for i in 0..4 {
        let p = points[i]; let q = points[(i+1)%4];
        let same_x = p[0] == q[0]; let same_y = p[1] == q[1];
        // Exact axes avoid treating a nearly horizontal sloping edge as empty.
        if same_x == same_y { return Ok(vec![full]); }
        horizontal[i] = same_y;
    }
    if (0..4).any(|i| horizontal[i] == horizontal[(i+1)%4]) { return Ok(vec![full]); }
    let center = [
        points.iter().map(|p|p[0]).fold(f32::INFINITY,f32::min),
        points.iter().map(|p|p[1]).fold(f32::INFINITY,f32::min),
        points.iter().map(|p|p[0]).fold(f32::NEG_INFINITY,f32::max),
        points.iter().map(|p|p[1]).fold(f32::NEG_INFINITY,f32::max),
    ];
    let Some(fringe) = raster_fringe(geometry) else { return Ok(vec![full]); };
    // Affine transforms turn the circular stroke pen into an ellipse. These
    // projection radii bound that pen on each displayed axis, including reflection
    // and nonuniform scale. Continuous strips conservatively cover dashed caps.
    let padding = [f64::from(width)*0.5*a.hypot(c)+fringe,
                   f64::from(width)*0.5*b.hypot(d)+fringe];
    Ok(border_regions(full,center,padding).unwrap_or_else(||vec![full]))
}

fn valid(r:[f32;4])->bool {
    r.iter().all(|v|v.is_finite()) && r[0]<=r[2] && r[1]<=r[3]
}

fn raster_fringe(geometry:PageGeometry)->Option<f64> {
    let size=geometry.displayed_size(); let w=f64::from(size.width); let h=f64::from(size.height);
    if !w.is_finite() || !h.is_finite() || w<=0. || h<=0. { return None; }
    let scale=(1000.0/w).min(1600.0/h);
    // The renderer rounds f32 dimensions. Subtract one pixel before flooring
    // to conservatively cover that rounding; degenerate rasters get no exemption.
    let pixels_w=(w*scale-1.0).floor(); let pixels_h=(h*scale-1.0).floor();
    if pixels_w<1. || pixels_h<1. { return None; }
    Some(6.0*(w/pixels_w).max(h/pixels_h)+2.0*f64::from(EPSILON))
}

fn border_regions(full:[f32;4],center:[f32;4],padding:[f64;2])->Option<Vec<[f32;4]>> {
    if !valid(full) || !valid(center) || padding.iter().any(|p|!p.is_finite()||*p<0.) { return None; }
    let [l,b,r,t]=center.map(f64::from); let [x,y]=padding;
    // Round away from the empty interior, never toward it. Large translations or
    // tiny surviving areas that collapse at f32 precision retain full bounds.
    let inner=[((l+x) as f32).next_up(),((b+y) as f32).next_up(),
               ((r-x) as f32).next_down(),((t-y) as f32).next_down()];
    let outer=[((l-x).min(f64::from(full[0])) as f32).next_down(),
               ((b-y).min(f64::from(full[1])) as f32).next_down(),
               ((r+x).max(f64::from(full[2])) as f32).next_up(),
               ((t+y).max(f64::from(full[3])) as f32).next_up()];
    if !valid(inner) || !valid(outer) || inner[0]>=inner[2] || inner[1]>=inner[3] { return None; }
    Some(vec![
        [outer[0],outer[1],inner[0],outer[3]],
        [inner[2],outer[1],outer[2],outer[3]],
        [inner[0],outer[1],inner[2],inner[1]],
        [inner[0],inner[3],inner[2],outer[3]],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn border_proof_rejects_nonfinite_or_collapsed_interior_and_rounds_conservatively() {
        let rect=[0.,0.,100.,60.];
        for padding in [[f64::INFINITY,1.],[f64::NAN,1.],[-1.,1.],[50.,1.],[1.,30.]] {
            assert!(border_regions(rect,rect,padding).is_none());
        }
        let large=[1.0e20f32,1.0e20f32,(1.0e20f32).next_up(),(1.0e20f32).next_up()];
        assert!(border_regions(large,large,[0.,0.]).is_none());
        let regions=border_regions(rect,rect,[2.3,4.7]).unwrap();
        assert_eq!(regions.len(),4);
        assert!(regions[0][2] as f64>=2.3 && regions[1][0] as f64<=97.7);
        assert!(regions[2][3] as f64>=4.7 && regions[3][1] as f64<=55.3);
        assert!(!regions.iter().any(|r|adds_overlap(&[0.;4],&[10.,10.,90.,50.],r)));
    }
    #[test]
    fn raster_margin_accounts_for_rounding_rotation_and_degenerate_pages() {
        for rotation in [0,90,180,270] {
            let g=PageGeometry{left:0.,bottom:0.,right:612.,top:792.,rotation};
            let margin=raster_fringe(g).unwrap();
            assert!(margin>=6.*(612.0f64/1000.).max(792./1600.));
        }
        for right in [0.,f32::NAN,f32::INFINITY,0.000001] {
            assert!(raster_fringe(PageGeometry{left:0.,bottom:0.,right,top:792.,rotation:0}).is_none());
        }
    }
}
