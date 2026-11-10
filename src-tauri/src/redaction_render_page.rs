//! Canonical page/view binding for the existing private area-page subset.
use super::{Interval, RendererTransform};
use crate::{redaction_content::require, EngineResult};

pub(crate) struct PageDevice {
    page: [f32; 2],
    device: [u32; 2],
}
impl PageDevice {
    /// Device support of a filled straight-edge polygon, after native vertex
    /// transformation. Fixed raster grids cannot extend more than one pixel
    /// beyond their quantized endpoints. Include the readers' hard coordinate
    /// clamps and PDFium's unclamped snapped-rectangle shortcut. Either route
    /// can be selected after transform rounding.
    pub fn fill_pixels(
        &self,
        bounds: crate::redaction_area_paths::AreaRect,
        pdfium: bool,
    ) -> EngineResult<Option<[i32; 4]>> {
        let coordinates = [bounds.left, bounds.bottom, bounds.right, bounds.top];
        require(
            coordinates.into_iter().all(f64::is_finite)
                && bounds.left <= bounds.right
                && bounds.bottom <= bounds.top,
        )?;
        let limit = if pdfium { 32000. } else { 1048576. };
        let mut c = coordinates.map(|n| n.clamp(-limit, limit));
        if pdfium {
            c[0] = c[0].min(coordinates[0]);
            c[1] = c[1].min(coordinates[1]);
            c[2] = c[2].max(coordinates[2]);
            c[3] = c[3].max(coordinates[3]);
        }
        let out = [
            (c[0].floor() - 1.).clamp(0., self.device[0] as f64) as i32,
            (c[1].floor() - 1.).clamp(0., self.device[1] as f64) as i32,
            (c[2].ceil() + 1.).clamp(0., self.device[0] as f64) as i32,
            (c[3].ceil() + 1.).clamp(0., self.device[1] as f64) as i32,
        ];
        Ok((out[0] < out[2] && out[1] < out[3]).then_some(out))
    }
    pub fn new(page: [f32; 2], device: [u32; 2]) -> EngineResult<Self> {
        require(
            page.into_iter()
                .all(|n| n.is_finite() && (1. ..=20000.).contains(&n))
                && device.into_iter().all(|n| (1..=1 << 24).contains(&n)),
        )?;
        Ok(Self { page, device })
    }
    fn scale(&self) -> EngineResult<[Interval; 2]> {
        Ok([
            Interval([f64::from(self.device[0]); 2]).div(Interval([f64::from(self.page[0]); 2]))?,
            Interval([f64::from(self.device[1]); 2]).div(Interval([f64::from(self.page[1]); 2]))?,
        ])
    }
    /// FPDF_RenderPageBitmap, origin=(0,0), rotation=0, canonical page boxes.
    pub fn pdfium(&self) -> EngineResult<RendererTransform> {
        let [x, y] = self.scale()?;
        Ok(RendererTransform([
            x,
            Interval([0.; 2]),
            Interval([0.; 2]),
            Interval([-y.0[1], -y.0[0]]),
            Interval([0.; 2]),
            Interval([f64::from(self.device[1]); 2]),
        ]))
    }
    /// MuPDF page interpretation using the requested binary32 axis scales and
    /// an identity draw device. Page-height multiplication precedes content cm.
    pub fn mupdf(&self) -> EngineResult<RendererTransform> {
        let scales = [
            self.device[0] as f32 / self.page[0],
            self.device[1] as f32 / self.page[1],
        ];
        RendererTransform::known([scales[0], 0., 0., scales[1], 0., 0.])?.concat(
            RendererTransform::known([1., 0., 0., -1., 0., self.page[1]])?,
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redaction_page_device_fill_includes_unclamped_rectangle_fast_path() {
        let view = PageDevice::new([100., 100.], [50000, 50000]).unwrap();
        let rect = crate::redaction_area_paths::AreaRect {
            left: 40000.25,
            bottom: 40000.25,
            right: 40010.75,
            top: 40010.75,
        };
        let actual = view.fill_pixels(rect, true).unwrap().unwrap();
        assert!(actual[0] <= 40000 && actual[1] <= 40000 && actual[2] > 40010 && actual[3] > 40010);
    }
    #[test]
    fn redaction_page_device_uses_real_viewport_dimensions_and_reader_order() {
        for (page, device) in [
            ([192., 160.], [240, 200]),
            ([612.125, 792.25], [937, 1213]),
            ([19999.9, 1.1], [2400, 1]),
        ] {
            let view = PageDevice::new(page, device).unwrap();
            let sx = device[0] as f32 / page[0];
            let sy = device[1] as f32 / page[1];
            let native = [sx, 0., 0., -sy, 0., device[1] as f32];
            let independent = [sx, 0., 0., -sy, 0., page[1] * sy];
            for (actual, expected) in [
                (view.pdfium().unwrap(), native),
                (view.mupdf().unwrap(), independent),
            ] {
                for (v, n) in actual.coefficients().into_iter().zip(expected) {
                    assert!(v[0] <= f64::from(n) && f64::from(n) <= v[1]);
                }
            }
        }
    }
    #[test]
    fn redaction_page_device_invalid_dimensions_refuse() {
        for (page, device) in [
            ([0., 160.], [240, 200]),
            ([192., f32::NAN], [240, 200]),
            ([192., 160.], [0, 200]),
            ([20001., 160.], [240, 200]),
        ] {
            assert!(PageDevice::new(page, device).is_err());
        }
    }
}
