//! Renderer-only inverse envelopes; never source-selection authority.
use super::{Interval, RendererTransform};
use crate::{redaction_content::require, EngineResult};

impl RendererTransform {
    /// Enclose both readers' inverse formulas, including their intermediates.
    /// Ambiguous determinants and unbounded intermediates are not admitted.
    pub fn reader_inverse(self) -> EngineResult<Self> {
        let neg = |v: Interval| Interval([-v.0[1], -v.0[0]]);
        let sub = |a: Interval, b: Interval| a.add(neg(b));
        let [a, b, c, d, e, f] = self.0;
        let determinant = sub(a.mul(d)?, b.mul(c)?)?;
        // MuPDF treats this entire band as singular. This interval also covers
        // PDFium's binary32 determinant and its exact-zero branch. Refusing
        // uncertainty avoids choosing either reader's unrelated fallback.
        require(determinant.0[0] > f64::EPSILON || determinant.0[1] < -f64::EPSILON)?;
        let divide = |n: Interval| {
            if determinant.0[1] < 0. {
                neg(n).div(neg(determinant))
            } else {
                n.div(determinant)
            }
        };
        // PDFium divides each numerator separately in binary32.
        let pdfium = [
            divide(d)?,
            divide(neg(b))?,
            divide(neg(c))?,
            divide(a)?,
            divide(sub(c.mul(f)?, d.mul(e)?)?)?,
            divide(sub(b.mul(e)?, a.mul(f)?)?)?,
        ];
        // MuPDF first computes the reciprocal, then the linear terms, then
        // translation, in binary64. Each outward binary32 interval includes
        // the exact arithmetic and both precisions' rounded intermediates.
        let reciprocal = divide(Interval([1.; 2]))?;
        let ma = d.mul(reciprocal)?;
        let mb = neg(b).mul(reciprocal)?;
        let mc = neg(c).mul(reciprocal)?;
        let md = a.mul(reciprocal)?;
        let mupdf = [
            ma,
            mb,
            mc,
            md,
            neg(e).mul(ma)?.add(neg(f).mul(mc)?)?,
            neg(e).mul(mb)?.add(neg(f).mul(md)?)?,
        ];
        let mut result = pdfium;
        for (out, other) in result.iter_mut().zip(mupdf) {
            *out = Interval::enclosing(out.0[0].min(other.0[0]), out.0[1].max(other.0[1]))?;
        }
        Ok(Self(result))
    }
}

#[cfg(test)]
#[path = "redaction_render_inverse_tests.rs"]
mod tests;
