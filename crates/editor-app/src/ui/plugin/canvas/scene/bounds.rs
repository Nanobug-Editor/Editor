//! Conservative affine hulls are shared by numeric raster preparation and native text clipping.
use super::*;

/// Bound validated geometry after `transform`, without allocating a second control-point array.
/// `minimum_extent` is zero for geometric queries and one for raster/gradient surface denominators.
pub(super) fn geometry_bounds(
    geometry: &SceneGeometry,
    transform: SceneTransform,
    minimum_extent: f32,
) -> protocol::Rect {
    let mut minimum = (f32::INFINITY, f32::INFINITY);
    let mut maximum = (f32::NEG_INFINITY, f32::NEG_INFINITY);
    let mut point = |x, y| {
        let (x, y) = transform.point(x, y);
        minimum.0 = minimum.0.min(x);
        minimum.1 = minimum.1.min(y);
        maximum.0 = maximum.0.max(x);
        maximum.1 = maximum.1.max(y);
    };
    match geometry {
        SceneGeometry::Rect { rect, .. } => {
            point(rect.x, rect.y);
            point(rect.x + rect.w, rect.y);
            point(rect.x, rect.y + rect.h);
            point(rect.x + rect.w, rect.y + rect.h);
        }
        SceneGeometry::Path { segments, .. } => {
            for segment in segments {
                match *segment {
                    ScenePathSegment::Move { x, y } | ScenePathSegment::Line { x, y } => {
                        point(x, y)
                    }
                    ScenePathSegment::Quadratic { cx, cy, x, y } => {
                        point(cx, cy);
                        point(x, y);
                    }
                    ScenePathSegment::Cubic {
                        x1,
                        y1,
                        x2,
                        y2,
                        x,
                        y,
                    } => {
                        point(x1, y1);
                        point(x2, y2);
                        point(x, y);
                    }
                    ScenePathSegment::Close => {}
                }
            }
        }
    }
    if !minimum.0.is_finite() {
        return protocol::Rect {
            x: 0.,
            y: 0.,
            w: minimum_extent,
            h: minimum_extent,
        };
    }
    protocol::Rect {
        x: minimum.0,
        y: minimum.1,
        w: (maximum.0 - minimum.0).max(minimum_extent),
        h: (maximum.1 - minimum.1).max(minimum_extent),
    }
}
