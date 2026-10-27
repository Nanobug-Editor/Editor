//! Portable affine geometry uses logical pixels and never relies on native enum layouts.
use super::*;

/// Column-major affine coefficients [a,b,c,d,tx,ty], mapping x'=ax+cy+tx, y'=bx+dy+ty.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SceneTransform(pub [f32; 6]);
impl Default for SceneTransform {
    fn default() -> Self {
        Self([1., 0., 0., 1., 0., 0.])
    }
}
impl SceneTransform {
    /// Project local `(x,y)` into the parent's logical coordinate system.
    pub fn point(self, x: f32, y: f32) -> (f32, f32) {
        let [a, b, c, d, tx, ty] = self.0;
        (a * x + c * y + tx, b * x + d * y + ty)
    }
    /// Compose `child` first, then this transform; used identically by painting and hit testing.
    pub fn compose(self, child: Self) -> Self {
        let [a, b, c, d, x, y] = self.0;
        let [e, f, g, h, u, v] = child.0;
        Self([
            a * e + c * f,
            b * e + d * f,
            a * g + c * h,
            b * g + d * h,
            a * u + c * v + x,
            b * u + d * v + y,
        ])
    }
    /// Return the inverse for input mapping, or None for nonfinite/singular transforms.
    pub fn inverse(self) -> Option<Self> {
        if !self.valid() {
            return None;
        }
        let [a, b, c, d, x, y] = self.0;
        let det = a * d - b * c;
        Some(Self([
            d / det,
            -b / det,
            -c / det,
            a / det,
            (c * y - d * x) / det,
            (b * x - a * y) / det,
        ]))
    }
    /// Admit finite, bounded, invertible transforms before any coordinate allocation.
    pub fn valid(self) -> bool {
        let [a, b, c, d, _, _] = self.0;
        self.0.into_iter().all(coordinate)
            && (a * d - b * c).is_finite()
            && (a * d - b * c).abs() >= 0.000_001
    }
}

/// Rectangle corners are top-left, top-right, bottom-right and bottom-left.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SceneGeometry {
    /// An axis-aligned local rectangle with independently bounded corner radii.
    Rect {
        /// Nonnegative local size.
        rect: crate::Rect,
        /// Optional four circular corner radii.
        #[serde(default)]
        radii: [f32; 4],
    },
    /// A structured path; a Move starts each subpath and Close closes only the current subpath.
    Path {
        /// Ordered path operations, sharing the whole scene's segment budget.
        segments: Vec<ScenePathSegment>,
        /// Interior winding rule.
        #[serde(default)]
        rule: SceneFillRule,
    },
}

/// Fill rule uses portable names, independent of GPUI/lyon values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneFillRule {
    /// Nonzero winding, the default.
    #[default]
    Nonzero,
    /// Alternate subpath interiors.
    EvenOdd,
}

/// Curve endpoints and control points are local logical pixels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScenePathSegment {
    /// Start a new contour at `(x,y)`.
    Move {
        /// Horizontal position.
        x: f32,
        /// Vertical position.
        y: f32,
    },
    /// Extend the contour by a straight segment.
    Line {
        /// Endpoint x.
        x: f32,
        /// Endpoint y.
        y: f32,
    },
    /// A quadratic curve with control `(cx,cy)` and endpoint `(x,y)`.
    Quadratic {
        /// Control x.
        cx: f32,
        /// Control y.
        cy: f32,
        /// Endpoint x.
        x: f32,
        /// Endpoint y.
        y: f32,
    },
    /// A cubic curve with controls `(x1,y1)`/`(x2,y2)` and endpoint `(x,y)`.
    Cubic {
        /// First control x.
        x1: f32,
        /// First control y.
        y1: f32,
        /// Second control x.
        x2: f32,
        /// Second control y.
        y2: f32,
        /// Endpoint x.
        x: f32,
        /// Endpoint y.
        y: f32,
    },
    /// Close the current contour to its starting point.
    Close,
}
impl SceneGeometry {
    /// Bound local contour length with control polygons before any renderer expands dashes.
    /// Rounded rectangle arcs shorten its perimeter; Bézier curves stay below their control polygon.
    pub(super) fn length_bound(&self) -> f64 {
        let distance = |a: (f32, f32), b: (f32, f32)| {
            (f64::from(a.0) - f64::from(b.0)).hypot(f64::from(a.1) - f64::from(b.1))
        };
        match self {
            Self::Rect { rect, .. } => 2. * (f64::from(rect.w) + f64::from(rect.h)),
            Self::Path { segments, .. } => {
                let mut current = (0., 0.);
                let mut start = current;
                let mut length = 0.;
                for segment in segments {
                    match *segment {
                        ScenePathSegment::Move { x, y } => {
                            current = (x, y);
                            start = current;
                        }
                        ScenePathSegment::Line { x, y } => {
                            length += distance(current, (x, y));
                            current = (x, y);
                        }
                        ScenePathSegment::Quadratic { cx, cy, x, y } => {
                            length += distance(current, (cx, cy)) + distance((cx, cy), (x, y));
                            current = (x, y);
                        }
                        ScenePathSegment::Cubic {
                            x1,
                            y1,
                            x2,
                            y2,
                            x,
                            y,
                        } => {
                            length += distance(current, (x1, y1))
                                + distance((x1, y1), (x2, y2))
                                + distance((x2, y2), (x, y));
                            current = (x, y);
                        }
                        ScenePathSegment::Close => {
                            length += distance(current, start);
                            current = start;
                        }
                    }
                }
                length
            }
        }
    }
    /// Charge one rectangle or every explicit path segment to the scene budget.
    pub fn segment_count(&self) -> usize {
        match self {
            Self::Rect { .. } => 1,
            Self::Path { segments, .. } => segments.len(),
        }
    }
    /// Validate local and fully transformed bounds; malformed contour ordering is rejected.
    pub fn valid(&self, transform: SceneTransform) -> bool {
        self.valid_with_margin(transform, 0.)
    }
    /// A conservative projected stroke margin must fit before host tessellation allocates vertices.
    pub(super) fn valid_with_margin(&self, transform: SceneTransform, margin: f32) -> bool {
        let point = |x, y| {
            let (px, py) = transform.point(x, y);
            coordinate(x)
                && coordinate(y)
                && coordinate(px - margin)
                && coordinate(px + margin)
                && coordinate(py - margin)
                && coordinate(py + margin)
        };
        match self {
            Self::Rect { rect, radii } => {
                rect.w >= 0.
                    && rect.h >= 0.
                    && [rect.x, rect.y, rect.w, rect.h].into_iter().all(coordinate)
                    && radii
                        .iter()
                        .all(|r| r.is_finite() && *r >= 0. && *r <= 1_000_000.)
                    && [
                        (rect.x, rect.y),
                        (rect.x + rect.w, rect.y),
                        (rect.x, rect.y + rect.h),
                        (rect.x + rect.w, rect.y + rect.h),
                    ]
                    .into_iter()
                    .all(|(x, y)| point(x, y))
            }
            Self::Path { segments, .. } => {
                let mut open = false;
                !segments.is_empty()
                    && segments.iter().all(|s| match *s {
                        ScenePathSegment::Move { x, y } => {
                            open = true;
                            point(x, y)
                        }
                        ScenePathSegment::Line { x, y } => open && point(x, y),
                        ScenePathSegment::Quadratic { cx, cy, x, y } => {
                            open && point(cx, cy) && point(x, y)
                        }
                        ScenePathSegment::Cubic {
                            x1,
                            y1,
                            x2,
                            y2,
                            x,
                            y,
                        } => open && point(x1, y1) && point(x2, y2) && point(x, y),
                        ScenePathSegment::Close => {
                            let valid = open;
                            open = false;
                            valid
                        }
                    })
            }
        }
    }
}
/// All public canvas coordinates must be finite and within the existing drawing extent.
fn coordinate(value: f32) -> bool {
    value.is_finite() && value.abs() <= 1_000_000.
}
