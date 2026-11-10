//! Affine form clips stay in their own coordinates, fixed at form invocation.
use super::{invalid, EngineResult, EPSILON};

pub(super) fn checked(m: [f32; 6]) -> EngineResult<[f32; 6]> {
    let determinant = m[0] as f64 * m[3] as f64 - m[1] as f64 * m[2] as f64;
    let squared_norm: f64 = m[..4].iter().map(|v| (*v as f64).powi(2)).sum();
    let largest_scale = ((squared_norm
        + (squared_norm * squared_norm - 4. * determinant * determinant)
            .max(0.)
            .sqrt())
        / 2.)
        .sqrt();
    if m.iter().any(|v| !v.is_finite() || v.abs() > 1_000_000.0)
        || m[..4].iter().any(|v| v.abs() > 100.0)
        // Minimum singular value: preserve the original >0.0001 axis-scale
        // bound without rejecting a uniformly small, well-conditioned matrix.
        || determinant.abs() <= 0.0001 * largest_scale
    {
        return Err(invalid(
            "The form transform is singular or exceeds safe editing bounds.",
        ));
    }
    Ok(m)
}

pub(super) fn compose(a: [f32; 6], b: [f32; 6]) -> EngineResult<[f32; 6]> {
    let a = a.map(f64::from);
    let b = b.map(f64::from);
    checked([
        (a[0] * b[0] + a[2] * b[1]) as f32,
        (a[1] * b[0] + a[3] * b[1]) as f32,
        (a[0] * b[2] + a[2] * b[3]) as f32,
        (a[1] * b[2] + a[3] * b[3]) as f32,
        (a[0] * b[4] + a[2] * b[5] + a[4]) as f32,
        (a[1] * b[4] + a[3] * b[5] + a[5]) as f32,
    ])
}

#[derive(Clone)]
pub(super) struct ClipBox {
    bounds: [f64; 4],
    inverse: [f64; 6],
    tolerance: [f64; 2],
}
impl ClipBox {
    pub(super) fn new(bounds: [f32; 4], matrix: [f32; 6]) -> EngineResult<Self> {
        let m = checked(matrix)?.map(f64::from);
        if bounds
            .iter()
            .any(|v| !v.is_finite() || v.abs() > 1_000_000.0)
            || bounds[0] >= bounds[2]
            || bounds[1] >= bounds[3]
        {
            return Err(invalid("Malformed form bounding box."));
        }
        let determinant = m[0] * m[3] - m[1] * m[2];
        let inverse = [
            m[3] / determinant,
            -m[1] / determinant,
            -m[2] / determinant,
            m[0] / determinant,
            (m[2] * m[5] - m[3] * m[4]) / determinant,
            (m[1] * m[4] - m[0] * m[5]) / determinant,
        ];
        // Local distances to clip edges are signed distances in page space
        // times the corresponding inverse-row norm. Keep the existing 0.02pt
        // page-space tolerance even for scaling, shear or reflection.
        let tolerance = [
            inverse[0].hypot(inverse[2]) * EPSILON as f64,
            inverse[1].hypot(inverse[3]) * EPSILON as f64,
        ];
        Ok(Self {
            bounds: bounds.map(f64::from),
            inverse,
            tolerance,
        })
    }

    pub(super) fn contains(&self, corners: &[[f32; 2]; 4]) -> bool {
        let m = self.inverse;
        corners.iter().all(|&[x, y]| {
            let u = m[0] * x as f64 + m[2] * y as f64 + m[4];
            let v = m[1] * x as f64 + m[3] * y as f64 + m[5];
            u.is_finite()
                && v.is_finite()
                && u >= self.bounds[0] - self.tolerance[0]
                && u <= self.bounds[2] + self.tolerance[0]
                && v >= self.bounds[1] - self.tolerance[1]
                && v <= self.bounds[3] + self.tolerance[1]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_previously_supported_compensated_small_scales() {
        let inner = checked([0.0002, 0., 0., 0.0002, 0., 0.]).unwrap();
        let result = compose([100., 0., 0., 100., 0., 0.], inner).unwrap();
        assert!((result[0] - 0.02).abs() < 0.000001);
    }
    fn point(m: [f32; 6], x: f32, y: f32) -> [f32; 2] {
        [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]]
    }
    #[test]
    fn rotated_clip_uses_quad_corners_not_world_envelopes() {
        let m = [0.70710677, 0.70710677, -0.70710677, 0.70710677, 120., 80.];
        let clip = ClipBox::new([0., 0., 100., 20.], m).unwrap();
        let inside = [
            point(m, 1., 1.),
            point(m, 99., 1.),
            point(m, 99., 19.),
            point(m, 1., 19.),
        ];
        assert!(clip.contains(&inside));
        let outside = [
            point(m, 80., 21.),
            point(m, 85., 21.),
            point(m, 85., 22.),
            point(m, 80., 22.),
        ];
        // This quad fits the world-axis envelope [~106,80,191,165], yet is
        // outside the actual rotated clip. A world-envelope check is unsafe.
        assert!(outside
            .iter()
            .all(|p| p[0] > 106. && p[0] < 191. && p[1] > 80. && p[1] < 165.));
        assert!(!clip.contains(&outside));
    }
    #[test]
    fn composition_keeps_pdf_order_and_inverse_handles_reflection() {
        let a = [0., 2., -1., 0., 50., 70.];
        let b = [-1., 0.2, 0.3, 0.8, 8., 9.];
        let c = compose(a, b).unwrap();
        let p = point(b, 5., 7.);
        let expected = point(a, p[0], p[1]);
        let actual = point(c, 5., 7.);
        assert!(
            (actual[0] - expected[0]).abs() < 0.0001 && (actual[1] - expected[1]).abs() < 0.0001
        );
        let clip = ClipBox::new([0., 0., 10., 10.], c).unwrap();
        assert!(clip.contains(&[
            point(c, 1., 1.),
            point(c, 9., 1.),
            point(c, 9., 9.),
            point(c, 1., 9.)
        ]));
        assert!(!clip.contains(&[point(c, 11., 5.); 4]));
    }
    #[test]
    fn tolerance_stays_in_page_points_and_invalid_matrices_are_refused() {
        for scale in [0.1, 1., 10., 100.] {
            let clip = ClipBox::new([0., 0., 10., 10.], [scale, 0., 0., scale, 0., 0.]).unwrap();
            assert!(clip.contains(&[[-0.01, 5. * scale]; 4]));
            assert!(!clip.contains(&[[-0.03, 5. * scale]; 4]));
        }
        for m in [
            [1., 1., 1., 1., 0., 0.],
            [1., 1., 1., 1.00001, 0., 0.],
            [101., 0., 0., 1., 0., 0.],
            [f32::NAN, 0., 0., 1., 0., 0.],
        ] {
            assert!(checked(m).is_err());
        }
        assert!(compose([11., 0., 0., 11., 0., 0.], [11., 0., 0., 11., 0., 0.]).is_err());
    }
}
