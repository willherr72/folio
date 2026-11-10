//! Private drawing planner for area redaction. No parsing or publication authority.

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AreaRect {
    pub left: f64,
    pub bottom: f64,
    pub right: f64,
    pub top: f64,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum PaintKind {
    Fill,
    Stroke,
    FillStroke,
    CloseStroke,
    CloseFillStroke,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum PathCommand {
    Save,
    Restore,
    Concat([f64; 6]),
    LineWidth(f64),
    LineJoin(u8),
    LineCap(u8),
    MiterLimit(f64),
    Move([f64; 2]),
    Line([f64; 2]),
    Curve([f64; 6]),
    CurveV([f64; 4]),
    CurveY([f64; 4]),
    Rect([f64; 4]),
    Close,
    Paint(PaintKind),
    End,
    Clip,
    /// Only for commands the strict adapter proves leave geometry/state unchanged.
    Unchanged,
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PathError {
    InvalidMark,
    Limit,
    Geometry,
    State,
    Unsupported,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PathDecision {
    pub first: usize,
    pub last: usize,
    pub bounds: AreaRect,
    pub painted: bool,
    pub remove: bool,
    pub extends_beyond_marks: bool,
}

pub(crate) fn plan_paths(
    commands: &[PathCommand],
    marks: &[AreaRect],
) -> Result<Vec<PathDecision>, PathError> {
    if !(1..=64).contains(&marks.len()) || marks.iter().any(|r| !r.valid_mark()) {
        return Err(PathError::InvalidMark);
    }
    if commands.len() > 100_000 {
        return Err(PathError::Limit);
    }
    let mut state = State::default();
    let mut stack = Vec::new();
    let mut path: Option<Path> = None;
    let mut decisions = Vec::new();
    for (index, command) in commands.iter().enumerate() {
        use PathCommand::*;
        // Clipping has persistent semantics. It must never become an ignored
        // operation, even when the current mark misses its control hull.
        if matches!(command, Clip | Unsupported) {
            return Err(PathError::Unsupported);
        }
        match *command {
            Move(point) => {
                let bounds = state.matrix.point(point)?;
                if let Some(path) = path.as_mut() {
                    path.bounds.include(bounds);
                } else {
                    path = Some(Path {
                        first: index,
                        bounds,
                    });
                }
            }
            Rect([x, y, width, height]) => {
                for n in [x, y, width, height] {
                    bounded(n)?;
                }
                let right = Interval::exact(x).add(Interval::exact(width))?;
                let top = Interval::exact(y).add(Interval::exact(height))?;
                let corners = [
                    [Interval::exact(x), Interval::exact(y)],
                    [right, Interval::exact(y)],
                    [right, top],
                    [Interval::exact(x), top],
                ];
                for point in corners {
                    let bounds = state.matrix.interval_point(point)?;
                    if let Some(path) = path.as_mut() {
                        path.bounds.include(bounds);
                    } else {
                        path = Some(Path {
                            first: index,
                            bounds,
                        });
                    }
                }
            }
            Line(point) => {
                let path = path.as_mut().ok_or(PathError::State)?;
                path.bounds.include(state.matrix.point(point)?);
            }
            Curve(points) => {
                let path = path.as_mut().ok_or(PathError::State)?;
                for pair in points.chunks_exact(2) {
                    path.bounds.include(state.matrix.point([pair[0], pair[1]])?);
                }
            }
            CurveV(points) | CurveY(points) => {
                let path = path.as_mut().ok_or(PathError::State)?;
                // Implicit current/end control points are already included.
                for pair in points.chunks_exact(2) {
                    path.bounds.include(state.matrix.point([pair[0], pair[1]])?);
                }
            }
            Close => {
                path.as_ref().ok_or(PathError::State)?;
            }
            Paint(_) | End => {
                let Some(path) = path.take() else {
                    if matches!(command, End) {
                        continue;
                    }
                    return Err(PathError::State);
                };
                let painted = matches!(command, Paint(_));
                let stroke = matches!(command, Paint(kind) if !matches!(kind, PaintKind::Fill));
                let bounds = if stroke {
                    state.stroke_bounds(path.bounds)?
                } else {
                    path.bounds
                };
                let remove = marks.iter().any(|mark| mark.touches(bounds));
                decisions.push(PathDecision {
                    first: path.first,
                    last: index,
                    bounds,
                    painted,
                    remove,
                    extends_beyond_marks: remove && !marks.iter().any(|mark| mark.contains(bounds)),
                });
            }
            _ => {
                // A PDF path is not part of the saved graphics state. Instead
                // of accidentally restoring it, reject interleaved state here.
                if path.is_some() {
                    return Err(PathError::State);
                }
                match *command {
                    Save => {
                        if stack.len() == 64 {
                            return Err(PathError::Limit);
                        }
                        stack.push(state);
                    }
                    Restore => state = stack.pop().ok_or(PathError::State)?,
                    Concat(matrix) => state.matrix = state.matrix.concat(Matrix::new(matrix)?)?,
                    LineWidth(width) => {
                        bounded(width)?;
                        if width < 0. {
                            return Err(PathError::Geometry);
                        }
                        state.width = width;
                    }
                    MiterLimit(limit) => {
                        bounded(limit)?;
                        if limit < 1. {
                            return Err(PathError::Geometry);
                        }
                        state.miter = limit;
                    }
                    LineJoin(join) => {
                        if join > 2 {
                            return Err(PathError::State);
                        }
                        state.join = join;
                    }
                    LineCap(cap) => {
                        if cap > 2 {
                            return Err(PathError::State);
                        }
                        // All caps use the square-cap envelope below.
                    }
                    Unchanged => {}
                    _ => return Err(PathError::Unsupported),
                }
            }
        }
    }
    if path.is_some() || !stack.is_empty() {
        return Err(PathError::State);
    }
    Ok(decisions)
}

const COORDINATE_LIMIT: f64 = 1_000_000.;
fn bounded(n: f64) -> Result<(), PathError> {
    if n.is_finite() && n.abs() <= COORDINATE_LIMIT {
        Ok(())
    } else {
        Err(PathError::Geometry)
    }
}
impl AreaRect {
    pub(crate) fn valid_mark(self) -> bool {
        [self.left, self.bottom, self.right, self.top]
            .into_iter()
            .all(|n| bounded(n).is_ok())
            && self.left < self.right
            && self.bottom < self.top
    }
    pub(crate) fn include(&mut self, other: Self) {
        self.left = self.left.min(other.left);
        self.bottom = self.bottom.min(other.bottom);
        self.right = self.right.max(other.right);
        self.top = self.top.max(other.top);
    }
    pub(crate) fn touches(self, other: Self) -> bool {
        self.left <= other.right
            && other.left <= self.right
            && self.bottom <= other.top
            && other.bottom <= self.top
    }
    pub(crate) fn contains(self, other: Self) -> bool {
        self.left <= other.left
            && self.bottom <= other.bottom
            && self.right >= other.right
            && self.top >= other.top
    }
}

// Directed intervals retain rounding uncertainty through concatenation, not
// just at the final point. Exact integer fixtures remain exact. Subnormal
// arithmetic is refused because a zero residual would not prove exactness.
#[derive(Clone, Copy, Debug)]
struct Interval {
    low: f64,
    high: f64,
}
impl Interval {
    fn exact(n: f64) -> Self {
        Self { low: n, high: n }
    }
    fn negate(self) -> Self {
        Self {
            low: -self.high,
            high: -self.low,
        }
    }
    fn max_abs(self) -> f64 {
        self.low.abs().max(self.high.abs())
    }
    fn sum(a: f64, b: f64) -> Result<Self, PathError> {
        let value = a + b;
        if !value.is_finite() || (value != 0. && value.abs() < f64::MIN_POSITIVE) {
            return Err(PathError::Geometry);
        }
        let virtual_b = value - a;
        let residual = (a - (value - virtual_b)) + (b - virtual_b);
        Ok(Self {
            low: if residual < 0. {
                value.next_down()
            } else {
                value
            },
            high: if residual > 0. {
                value.next_up()
            } else {
                value
            },
        })
    }
    fn product(a: f64, b: f64) -> Result<Self, PathError> {
        let value = a * b;
        if !value.is_finite() || (a != 0. && b != 0. && value.abs() < f64::MIN_POSITIVE) {
            return Err(PathError::Geometry);
        }
        let residual = a.mul_add(b, -value);
        // A 106-bit exact product can renormalize upward while its residual
        // rounds to signed zero. Include the whole binade above MIN/EPS, plus
        // its upper rounding boundary; testing only < (or <=) MIN/EPS misses
        // half-subnormal residuals. One outward ulp encloses that uncertainty.
        let near_underflow =
            a != 0. && b != 0. && value.abs() <= 2. * f64::MIN_POSITIVE / f64::EPSILON;
        Ok(Self {
            low: if residual < 0. || near_underflow {
                value.next_down()
            } else {
                value
            },
            high: if residual > 0. || near_underflow {
                value.next_up()
            } else {
                value
            },
        })
    }
    fn add(self, other: Self) -> Result<Self, PathError> {
        Ok(Self {
            low: Self::sum(self.low, other.low)?.low,
            high: Self::sum(self.high, other.high)?.high,
        })
    }
    fn mul(self, other: Self) -> Result<Self, PathError> {
        let pairs = [
            (self.low, other.low),
            (self.low, other.high),
            (self.high, other.low),
            (self.high, other.high),
        ];
        let mut result = Self {
            low: f64::INFINITY,
            high: f64::NEG_INFINITY,
        };
        for (a, b) in pairs {
            let product = Self::product(a, b)?;
            result.low = result.low.min(product.low);
            result.high = result.high.max(product.high);
        }
        Ok(result)
    }
    fn checked(self) -> Result<Self, PathError> {
        bounded(self.low)?;
        bounded(self.high)?;
        Ok(self)
    }
}

#[derive(Clone, Copy)]
struct Matrix([Interval; 6]);

/// Image occurrence transform using the same bounded outward arithmetic as paths.
#[derive(Clone, Copy)]
pub(crate) struct ImageTransform(Matrix);
impl Default for ImageTransform {
    fn default() -> Self {
        Self(Matrix([1., 0., 0., 1., 0., 0.].map(Interval::exact)))
    }
}
impl ImageTransform {
    pub(crate) fn concat(self, matrix: [f64; 6]) -> Result<Self, PathError> {
        Ok(Self(self.0.concat(Matrix::new(matrix)?)?))
    }
    /// Bound already serialized image-unit paint geometry with the same
    /// directed matrix arithmetic used for source sample selection.
    pub(crate) fn unit_bounds(self, rect: AreaRect) -> Result<AreaRect, PathError> {
        if !rect.valid_mark()
            || rect.left < 0.
            || rect.bottom < 0.
            || rect.right > 1.
            || rect.top > 1.
        {
            return Err(PathError::Geometry);
        }
        let mut bounds = self.0.point([rect.left, rect.bottom])?;
        for point in [
            [rect.left, rect.top],
            [rect.right, rect.bottom],
            [rect.right, rect.top],
        ] {
            bounds.include(self.0.point(point)?);
        }
        Ok(bounds)
    }
    /// Geometry only: caller must separately establish renderer support.
    /// Image-unit support is clipped before transformation; page support is not.
    pub(crate) fn disclosure_bounds(
        self,
        rect: AreaRect,
        unit_margin: [f64; 2],
        page_margin: [f64; 2],
    ) -> Result<AreaRect, PathError> {
        // Validate the original region before clipping; clipping must not
        // turn malformed input into valid paint/disclosure geometry.
        self.unit_bounds(rect)?;
        for margin in unit_margin.into_iter().chain(page_margin) {
            bounded(margin)?;
            if margin < 0. {
                return Err(PathError::Geometry);
            }
        }
        let [ux, uy] = unit_margin.map(|n| n.min(1.));
        let source = AreaRect {
            left: Interval::sum(rect.left, -ux)?.low.max(0.),
            bottom: Interval::sum(rect.bottom, -uy)?.low.max(0.),
            right: Interval::sum(rect.right, ux)?.high.min(1.),
            top: Interval::sum(rect.top, uy)?.high.min(1.),
        };
        let bounds = self.unit_bounds(source)?;
        let [px, py] = page_margin;
        let result = AreaRect {
            left: Interval::sum(bounds.left, -px)?.low,
            bottom: Interval::sum(bounds.bottom, -py)?.low,
            right: Interval::sum(bounds.right, px)?.high,
            top: Interval::sum(bounds.top, py)?.high,
        };
        if !result.valid_mark() {
            return Err(PathError::Geometry);
        }
        Ok(result)
    }
    pub(crate) fn pixel_bounds(
        self,
        edges: [u32; 4],
        width: u32,
        height: u32,
    ) -> Result<AreaRect, PathError> {
        let [x0, y0, x1, y1] = edges;
        if width == 0
            || height == 0
            || width > 32768
            || height > 32768
            || x0 >= x1
            || x1 > width
            || y0 >= y1
            || y1 > height
        {
            return Err(PathError::Geometry);
        }
        fn ratio(n: u32, d: u32) -> Interval {
            let value = f64::from(n) / f64::from(d);
            // Integers convert exactly. Enclose nontrivial division rounding;
            // exact binary fractions can safely retain their exact endpoints.
            if d.is_power_of_two() || n == 0 || n == d {
                Interval::exact(value)
            } else {
                Interval {
                    low: value.next_down(),
                    high: value.next_up(),
                }
            }
        }
        let left = ratio(x0, width);
        let right = ratio(x1, width);
        let bottom = ratio(height - y1, height);
        let top = ratio(height - y0, height);
        let mut bounds = self.0.interval_point([left, bottom])?;
        for point in [[left, top], [right, bottom], [right, top]] {
            bounds.include(self.0.interval_point(point)?);
        }
        Ok(bounds)
    }
}
impl Matrix {
    fn new(values: [f64; 6]) -> Result<Self, PathError> {
        for n in values {
            bounded(n)?;
        }
        Self(values.map(Interval::exact)).validate()
    }
    fn validate(self) -> Result<Self, PathError> {
        for n in self.0 {
            n.checked()?;
        }
        let [a, b, c, d, _, _] = self.0;
        let scale = [a, b, c, d]
            .into_iter()
            .map(Interval::max_abs)
            .fold(0., f64::max);
        // A deliberately bounded subset: reject physical collapse rather than
        // treating tiny linear components plus a large translation as points.
        if scale < 1e-9 {
            return Err(PathError::Geometry);
        }
        let determinant = a.mul(d)?.add(b.mul(c)?.negate())?;
        let minimum = if determinant.low > 0. {
            determinant.low
        } else if determinant.high < 0. {
            -determinant.high
        } else {
            return Err(PathError::Geometry);
        };
        if minimum / scale / scale < 1e-12 {
            return Err(PathError::Geometry);
        }
        Ok(self)
    }
    fn concat(self, local: Self) -> Result<Self, PathError> {
        let [a, b, c, d, e, f] = self.0;
        let [g, h, i, j, k, l] = local.0;
        Self([
            a.mul(g)?.add(c.mul(h)?)?,
            b.mul(g)?.add(d.mul(h)?)?,
            a.mul(i)?.add(c.mul(j)?)?,
            b.mul(i)?.add(d.mul(j)?)?,
            a.mul(k)?.add(c.mul(l)?)?.add(e)?,
            b.mul(k)?.add(d.mul(l)?)?.add(f)?,
        ])
        .validate()
    }
    fn point(self, point: [f64; 2]) -> Result<AreaRect, PathError> {
        for n in point {
            bounded(n)?;
        }
        self.interval_point(point.map(Interval::exact))
    }
    fn interval_point(self, [x, y]: [Interval; 2]) -> Result<AreaRect, PathError> {
        x.checked()?;
        y.checked()?;
        let [a, b, c, d, e, f] = self.0;
        let x_out = a.mul(x)?.add(c.mul(y)?)?.add(e)?.checked()?;
        let y_out = b.mul(x)?.add(d.mul(y)?)?.add(f)?.checked()?;
        Ok(AreaRect {
            left: x_out.low,
            bottom: y_out.low,
            right: x_out.high,
            top: y_out.high,
        })
    }
}

struct Path {
    first: usize,
    bounds: AreaRect,
}
#[derive(Clone, Copy)]
struct State {
    matrix: Matrix,
    width: f64,
    miter: f64,
    join: u8,
}
impl Default for State {
    fn default() -> Self {
        Self {
            matrix: Matrix([1., 0., 0., 1., 0., 0.].map(Interval::exact)),
            width: 1.,
            miter: 10.,
            join: 0,
        }
    }
}
impl State {
    fn stroke_bounds(self, bounds: AreaRect) -> Result<AreaRect, PathError> {
        if self.width == 0. {
            return Err(PathError::Unsupported);
        }
        let radius = Interval::exact(self.width).mul(Interval::exact(0.5))?;
        let factor = if self.join == 0 {
            2. * self.miter
        } else {
            std::f64::consts::SQRT_2.next_up()
        };
        let radius = radius.mul(Interval::exact(factor))?;
        let [a, b, c, d, _, _] = self.matrix.0;
        // L1 row norms overestimate Euclidean norms without sqrt rounding.
        // They safely bound affine shear, reflection and nonuniform scaling.
        let x_scale = Interval::exact(a.max_abs()).add(Interval::exact(c.max_abs()))?;
        let y_scale = Interval::exact(b.max_abs()).add(Interval::exact(d.max_abs()))?;
        let x_pad = radius.mul(x_scale)?.high;
        let y_pad = radius.mul(y_scale)?.high;
        let left = Interval::sum(bounds.left, -x_pad)?.low;
        let bottom = Interval::sum(bounds.bottom, -y_pad)?.low;
        let right = Interval::sum(bounds.right, x_pad)?.high;
        let top = Interval::sum(bounds.top, y_pad)?.high;
        for n in [left, bottom, right, top] {
            bounded(n)?;
        }
        Ok(AreaRect {
            left,
            bottom,
            right,
            top,
        })
    }
}

#[cfg(test)]
#[path = "redaction_area_paths_tests.rs"]
mod tests;
