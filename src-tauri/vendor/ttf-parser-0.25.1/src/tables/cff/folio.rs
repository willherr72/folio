//! Bounded proof primitives used by Folio without changing convenience APIs.
use super::*;

pub(super) fn charge(work: &mut Option<&mut usize>, amount: usize) -> Option<()> {
    if let Some(remaining) = work.as_deref_mut() {
        match remaining.checked_sub(amount) {
            Some(value) => *remaining = value,
            None => { *remaining = 0; return None; }
        }
    }
    Some(())
}

fn valid_matrix(matrix: Matrix) -> Option<Matrix> {
    if ![matrix.sx,matrix.ky,matrix.kx,matrix.sy,matrix.tx,matrix.ty].iter().all(|v|v.is_finite()) {
        return None;
    }
    let determinant = matrix.sx * matrix.sy - matrix.kx * matrix.ky;
    if !determinant.is_finite() || determinant == 0.0 { return None; }
    Some(matrix)
}

#[cfg(feature = "glyph-names")]
fn font_matrix(data: &[u8], work: &mut Option<&mut usize>) -> Option<Matrix> {
    charge(work, data.len())?;
    let mut operands = [0.0; MAX_OPERANDS_LEN];
    let mut parser = DictionaryParser::new(data, &mut operands);
    let mut matrix = Matrix { sx:1.0, ky:0.0, kx:0.0, sy:1.0, tx:0.0, ty:0.0 };
    while let Some(operator) = parser.parse_next() {
        if operator.get() == top_dict_operator::FONT_MATRIX {
            parser.parse_operands()?;
            let values = parser.operands();
            if values.len() != 6 { return None; }
            matrix = Matrix { sx:values[0]as f32, ky:values[1]as f32, kx:values[2]as f32,
                sy:values[3]as f32, tx:values[4]as f32, ty:values[5]as f32 };
            valid_matrix(matrix)?;
        }
    }
    if !parser.at_end() { return None; }
    Some(matrix)
}

#[cfg(feature = "glyph-names")]
fn concatenate(parent: Matrix, child: Matrix) -> Option<Matrix> {
    valid_matrix(Matrix {
        sx: parent.sx * child.sx + parent.kx * child.ky,
        ky: parent.ky * child.sx + parent.sy * child.ky,
        kx: parent.sx * child.kx + parent.kx * child.sy,
        sy: parent.ky * child.kx + parent.sy * child.sy,
        tx: parent.sx * child.tx + parent.kx * child.ty + parent.tx,
        ty: parent.ky * child.tx + parent.sy * child.ty + parent.ty,
    })
}

/// One glyph's actual program identifier and glyph-space transformation.
#[cfg(feature = "glyph-names")]
#[derive(Clone,Copy,Debug)]
pub struct GlyphInfo<'a> {
    /// Index in the CharStrings INDEX, independent of a PDF character code.
    pub glyph_id: GlyphId,
    /// Name for a name-keyed program; absent for CID-keyed programs.
    pub name: Option<&'a str>,
    /// Actual charset CID for a CID-keyed program; absent otherwise.
    pub cid: Option<u16>,
    /// Top matrix concatenated with the selected Font DICT matrix.
    pub matrix: Matrix,
}

/// Error-reporting, allocation-free traversal of every program glyph.
#[cfg(feature = "glyph-names")]
pub struct GlyphInfos<'a,'work> {
    table: Table<'a>,
    charset: super::super::charset::CharsetIter<'a>,
    glyph: u16,
    work: &'work mut usize,
    failed: bool,
    seen: [u64;1024],
    matrices: [Option<Matrix>;256],
}

#[cfg(feature = "glyph-names")]
impl core::fmt::Debug for GlyphInfos<'_,'_> {
    fn fmt(&self, output: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        output.debug_struct("GlyphInfos").field("glyph",&self.glyph)
            .field("count",&self.table.number_of_glyphs).field("failed",&self.failed).finish()
    }
}

#[cfg(feature = "glyph-names")]
impl<'a> GlyphInfos<'a,'_> {
    fn read(&mut self) -> Result<GlyphInfo<'a>,CFFError> {
        let mut work = Some(&mut *self.work);
        charge(&mut work,1).ok_or(CFFError::WorkLimitReached)?;
        let identifier = self.charset.next().ok_or(CFFError::ReadOutOfBounds)?;
        let slot = usize::from(identifier.0) / 64;
        let bit = 1u64 << (identifier.0 % 64);
        if self.seen[slot] & bit != 0 { return Err(CFFError::InvalidArgumentsStackLength); }
        self.seen[slot] |= bit;
        let glyph = GlyphId(self.glyph);
        let top = valid_matrix(self.table.matrix).ok_or(CFFError::InvalidArgumentsStackLength)?;
        let (name,cid,matrix) = match self.table.kind {
            FontKind::SID(_) => {
                let index = usize::from(identifier.0);
                let name = if let Some(name) = STANDARD_NAMES.get(index) {
                    charge(&mut work,name.len()).ok_or(CFFError::WorkLimitReached)?;
                    *name
                } else {
                    let data = self.table.strings.get((index - STANDARD_NAMES.len())as u32)
                        .ok_or(CFFError::ReadOutOfBounds)?;
                    charge(&mut work,data.len()).ok_or(CFFError::WorkLimitReached)?;
                    core::str::from_utf8(data).map_err(|_|CFFError::ReadOutOfBounds)?
                };
                (Some(name),None,top)
            }
            FontKind::CID(ref cid) => {
                let index = cid.fd_select.font_dict_index_with_work(glyph,&mut work)
                    .ok_or(if work.as_deref().map_or(false,|v|*v==0) {CFFError::WorkLimitReached}else{CFFError::ReadOutOfBounds})?;
                let child = match self.matrices[usize::from(index)] {
                    Some(matrix) => matrix,
                    None => {
                        let data = cid.fd_array.get(u32::from(index)).ok_or(CFFError::ReadOutOfBounds)?;
                        let matrix = font_matrix(data,&mut work).ok_or(if work.as_deref().map_or(false,|v|*v==0) {CFFError::WorkLimitReached}else{CFFError::ReadOutOfBounds})?;
                        self.matrices[usize::from(index)] = Some(matrix);
                        matrix
                    }
                };
                (None,Some(identifier.0),concatenate(top,child).ok_or(CFFError::InvalidArgumentsStackLength)?)
            }
        };
        self.glyph += 1;
        Ok(GlyphInfo {glyph_id:glyph,name,cid,matrix})
    }
}

#[cfg(feature = "glyph-names")]
impl<'a> Iterator for GlyphInfos<'a,'_> {
    type Item = Result<GlyphInfo<'a>,CFFError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.failed || self.glyph >= self.table.number_of_glyphs.get() { return None; }
        let value = self.read();
        if value.is_err() { self.failed = true; }
        Some(value)
    }
}

// Conservative ink evidence gathered during the existing charged Type2 walk.
// Move positions and control-point bounds alone do not establish a filled
// contour. Require nonzero signed area per closed contour; cancellation can
// exclude unusual real outlines, which is preferable to authorizing no ink.
struct InkBounds {
    bounds: RectF,
    contour: RectF,
    origin: (f64, f64),
    last: (f64, f64),
    area: f64,
    area_scale: f64,
    terms: usize,
    active: bool,
}

impl InkBounds {
    fn new() -> Self {
        Self { bounds: RectF::new(), contour: RectF::new(), origin: (0.0, 0.0),
            last: (0.0, 0.0), area: 0.0, area_scale: 0.0, terms: 0, active: false }
    }

    fn relative(&self, x: f32, y: f32) -> (f64, f64) {
        (f64::from(x) - self.origin.0, f64::from(y) - self.origin.1)
    }

    fn bezier(&mut self, x: [f64; 4], y: [f64; 4]) {
        // Integral of x dy - y dx for a degree <= 3 polynomial. Fixed work,
        // no subdivision, allocation, or second outline traversal.
        for i in 0..4 {
            for j in 1..4 {
                let left = x[i] * y[j];
                let right = y[i] * x[j];
                let weight = j as f64 / (i + j) as f64;
                self.area += (left - right) * weight;
                self.area_scale += (left.abs() + right.abs()) * weight;
                self.terms += 1;
            }
        }
    }
}

impl OutlineBuilder for InkBounds {
    fn move_to(&mut self, x: f32, y: f32) {
        self.close();
        self.origin = (f64::from(x), f64::from(y));
        self.last = (0.0, 0.0);
        self.area = 0.0;
        self.area_scale = 0.0;
        self.terms = 0;
        self.contour = RectF::new();
        self.contour.extend_by(x, y);
        self.active = true;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let point = self.relative(x, y);
        let left = self.last.0 * point.1;
        let right = self.last.1 * point.0;
        self.area += left - right;
        self.area_scale += left.abs() + right.abs();
        self.terms += 1;
        self.contour.extend_by(x, y);
        self.last = point;
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let a = self.last;
        let b = self.relative(x1, y1);
        let c = self.relative(x, y);
        self.bezier([a.0, 2.0*(b.0-a.0), a.0-2.0*b.0+c.0, 0.0],
                    [a.1, 2.0*(b.1-a.1), a.1-2.0*b.1+c.1, 0.0]);
        self.contour.extend_by(x1, y1);
        self.contour.extend_by(x, y);
        self.last = c;
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let a = self.last;
        let b = self.relative(x1, y1);
        let c = self.relative(x2, y2);
        let d = self.relative(x, y);
        self.bezier([a.0, 3.0*(b.0-a.0), 3.0*(a.0-2.0*b.0+c.0), -a.0+3.0*b.0-3.0*c.0+d.0],
                    [a.1, 3.0*(b.1-a.1), 3.0*(a.1-2.0*b.1+c.1), -a.1+3.0*b.1-3.0*c.1+d.1]);
        self.contour.extend_by(x1, y1);
        self.contour.extend_by(x2, y2);
        self.contour.extend_by(x, y);
        self.last = d;
    }

    fn close(&mut self) {
        // The closing edge ends at the relative origin, contributing zero.
        // Exclude cancellation residue, not only exact zero. The generous
        // relative guard scales with accumulated terms and their magnitudes;
        // it can conservatively exclude ill-conditioned genuine outlines.
        let roundoff = 64.0 * f64::EPSILON * (self.terms as f64 + 1.0) * self.area_scale;
        if self.active && self.area.is_finite() && self.area.abs() > roundoff {
            self.bounds.extend_by(self.contour.x_min, self.contour.y_min);
            self.bounds.extend_by(self.contour.x_max, self.contour.y_max);
        }
        self.active = false;
    }
}

impl<'a> Table<'a> {
    /// Iterates exact charset identifiers with a shared caller work allowance.
    /// Failure is returned once and is distinct from successful exhaustion.
    #[cfg(feature = "glyph-names")]
    pub fn glyph_infos<'work>(&self, remaining_work: &'work mut usize) -> GlyphInfos<'a,'work> {
        GlyphInfos { table:*self, charset:super::super::charset::CharsetIter::new(self.charset,self.number_of_glyphs.get()),
            glyph:0, work:remaining_work, failed:false, seen:[0;1024], matrices:[None;256] }
    }

    /// Resolves the declared built-in encoding, without Standard fallback.
    pub fn glyph_index_exact(&self, code: u8) -> Option<GlyphId> {
        match self.kind {
            FontKind::SID(ref sid) => sid.encoding.exact_code_to_gid(&self.charset,code),
            FontKind::CID(_) => None,
        }
    }

    /// Reads a fractional SID/CID advance after bounded complete Type2 validation.
    /// The result is in raw charstring units; use the applicable glyph matrix.
    pub fn glyph_width_with_budget(&self, glyph: GlyphId, remaining_work: &mut usize) -> Option<f32> {
        self.glyph_metrics_with_budget(glyph, remaining_work).map(|value| value.0)
    }

    /// Reads the fractional advance and outline rectangle in one bounded full
    /// Type2 validation. Only contours with nonzero signed area contribute to
    /// the rectangle. A valid glyph without such evidence returns zero bounds.
    pub fn glyph_metrics_with_budget(&self, glyph: GlyphId, remaining_work: &mut usize) -> Option<(f32, Rect)> {
        let mut work = Some(remaining_work);
        charge(&mut work,1)?;
        let (default,nominal) = match self.kind {
            FontKind::SID(ref sid) => (sid.default_width,sid.nominal_width),
            FontKind::CID(ref cid) => {
                let index = cid.fd_select.font_dict_index_with_work(glyph,&mut work)?;
                let data = cid.fd_array.get(u32::from(index))?;
                charge(&mut work,data.len())?;
                let range = parse_font_dict(data)?;
                let data = self.table_data.get(range)?;
                charge(&mut work,data.len())?;
                let private = parse_private_dict(data);
                (private.default_width.unwrap_or(0.0),private.nominal_width.unwrap_or(0.0))
            }
        };
        let data = self.char_strings.get(u32::from(glyph.0))?;
        let mut ink = InkBounds::new();
        let (_,width) = parse_char_string_with_work(data,self,glyph,false,&mut ink,work,true).ok()?;
        ink.close();
        let bounds = if ink.bounds.is_default() {Rect::zero()} else {ink.bounds.to_rect()?};
        let width = width.map(|value|nominal+value).unwrap_or(default);
        if width.is_finite() {Some((width,bounds))} else {None}
    }
}

#[cfg(test)]
mod ink_tests {
    use super::*;

    #[test]
    fn moves_and_diagonal_retracing_have_no_ink_bounds() {
        let mut ink = InkBounds::new();
        ink.move_to(0.0, 0.0); ink.move_to(100.0, 100.0); ink.close();
        assert!(ink.bounds.is_default());
        ink.move_to(0.0, 0.0); ink.line_to(100.0, 100.0);
        ink.line_to(0.0, 0.0); ink.close();
        assert!(ink.bounds.is_default());
        ink.move_to(0.0, 0.0); ink.curve_to(25.0, 25.0, 75.0, 75.0, 100.0, 100.0); ink.close();
        assert!(ink.bounds.is_default());
    }

    #[test]
    fn retraced_curve_cannot_gain_ink_from_roundoff() {
        let mut ink = InkBounds::new();
        ink.move_to(0.0, 0.0);
        ink.curve_to(1.0, 2.0, 3.0, 7.0, 0.0, 0.0);
        ink.curve_to(3.0, 7.0, 1.0, 2.0, 0.0, 0.0);
        ink.close();
        assert!(ink.bounds.is_default(), "Retraced outline has zero filled area");
    }

    #[test]
    fn curved_contour_has_area_despite_coincident_endpoints() {
        let mut ink = InkBounds::new();
        ink.move_to(0.0, 0.0); ink.curve_to(100.0, 0.0, 0.0, 100.0, 0.0, 0.0); ink.close();
        assert_eq!(ink.bounds.to_rect(), Some(Rect {x_min: 0, y_min: 0, x_max: 100, y_max: 100}));
        ink.move_to(-200.0, -200.0); ink.close();
        assert_eq!(ink.bounds.to_rect(), Some(Rect {x_min: 0, y_min: 0, x_max: 100, y_max: 100}));
    }
}
