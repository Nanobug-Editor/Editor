//! Bounded fill intersections suppress invisible targets and semantic boxes inside path clip holes.
use super::*;
use lyon_tessellation::{
    FillGeometryBuilder, FillOptions, FillTessellator, FillVertex, GeometryBuilder,
    GeometryBuilderError, VertexId,
};

/// A filled triangle has no holes; the tessellator already resolved the source winding rule.
#[derive(Clone, Copy)]
pub(super) struct Triangle([math::Point; 3]);

/// The native tessellator cannot grow vertex or triangle output past the shared worker allowance.
struct Mesh<'a> {
    points: Vec<math::Point>,
    triangles: Vec<Triangle>,
    charged: &'a mut usize,
    limit: usize,
    overflow: bool,
}
impl GeometryBuilder for Mesh<'_> {
    fn add_triangle(&mut self, a: VertexId, b: VertexId, c: VertexId) {
        let points = [
            self.points[a.to_usize()],
            self.points[b.to_usize()],
            self.points[c.to_usize()],
        ];
        if cross(points[0], points[1], points[2]) == 0. {
            return;
        }
        if self.overflow || self.charged.saturating_add(3) > self.limit {
            self.overflow = true;
            return;
        }
        *self.charged += 3;
        self.triangles.push(Triangle(points));
    }
}
impl FillGeometryBuilder for Mesh<'_> {
    fn add_fill_vertex(&mut self, vertex: FillVertex) -> Result<VertexId, GeometryBuilderError> {
        if self.overflow || *self.charged >= self.limit {
            return Err(GeometryBuilderError::TooManyVertices);
        }
        *self.charged += 1;
        let id = VertexId::from_usize(self.points.len());
        self.points.push(vertex.position());
        Ok(id)
    }
}
/// Fill only already flattened paths, stopping before expanded output exceeds the scene vertex quota.
pub(super) fn mesh(
    path: &path::Path,
    rule: path::FillRule,
    charged: &mut usize,
    limit: usize,
) -> Result<Vec<Triangle>, String> {
    let mut output = Mesh {
        points: Vec::new(),
        triangles: Vec::new(),
        charged,
        limit,
        overflow: false,
    };
    FillTessellator::new()
        .tessellate_path(
            path,
            &FillOptions::default().with_fill_rule(rule),
            &mut output,
        )
        .map_err(|error| format!("Canvas visible fill preparation failed: {error}"))?;
    if output.overflow {
        return Err("Canvas visible fill exceeds its vertex budget".into());
    }
    Ok(output.triangles)
}

/// Intersect real fill triangles with every ancestor, retaining only positive-area visible fragments.
/// Clipped AABBs remain conservative, but an empty geometric intersection never receives native focus.
pub(super) fn bounds(
    triangles: Vec<Triangle>,
    clips: &[std::sync::Arc<FlatClip>],
    charged: &mut usize,
    limit: usize,
    work: &mut usize,
) -> Result<Option<protocol::Rect>, String> {
    let mut polygons = triangles
        .into_iter()
        .map(|triangle| triangle.0.to_vec())
        .collect::<Vec<_>>();
    for clip in clips {
        if clip.triangles.is_empty() {
            return Ok(None);
        }
        let mut next = Vec::new();
        for polygon in polygons {
            let original = polygon_bounds(&polygon);
            for triangle in &clip.triangles {
                // Charge even rejected pairs before testing or allocating intermediate fragments.
                let cost = 16 * (polygon.len() + 3);
                if *work < cost {
                    return Err("Canvas visible fill intersection work budget exhausted".into());
                }
                *work -= cost;
                let overlap = intersect(original, polygon_bounds(&triangle.0));
                if overlap.w <= 0. || overlap.h <= 0. {
                    continue;
                }
                if let Some((points, length)) = clipped(&polygon, triangle)? {
                    if charged.saturating_add(length) > limit {
                        return Err("Canvas visible fragments exceed their vertex budget".into());
                    }
                    *charged += length;
                    next.push(points[..length].to_vec());
                }
            }
        }
        polygons = next;
        if polygons.is_empty() {
            return Ok(None);
        }
    }
    let mut result = None;
    for polygon in polygons {
        let rect = polygon_bounds(&polygon);
        result = Some(match result {
            None => rect,
            Some(old) => {
                let old: protocol::Rect = old;
                let x = old.x.min(rect.x);
                let y = old.y.min(rect.y);
                protocol::Rect {
                    x,
                    y,
                    w: (old.x + old.w).max(rect.x + rect.w) - x,
                    h: (old.y + old.h).max(rect.y + rect.h) - y,
                }
            }
        });
    }
    Ok(result)
}
/// A triangle clipped by at most sixteen ancestor triangles needs at most 51 convex corners.
/// Fixed scratch buffers prevent an adversarial intersection from allocating before admission.
fn clipped(
    polygon: &[math::Point],
    triangle: &Triangle,
) -> Result<Option<([math::Point; 64], usize)>, String> {
    let mut input = [math::point(0., 0.); 64];
    let mut length = polygon.len();
    if length > 64 {
        return Err("Canvas visible polygon exceeds its corner budget".into());
    }
    input[..length].copy_from_slice(polygon);
    let direction = cross(triangle.0[0], triangle.0[1], triangle.0[2]).signum();
    for edge in 0..3 {
        let a = triangle.0[edge];
        let b = triangle.0[(edge + 1) % 3];
        let mut output = [math::point(0., 0.); 64];
        let mut count = 0;
        let mut previous = input[length - 1];
        let mut previous_distance = cross(a, b, previous) * direction;
        for current in &input[..length] {
            let distance = cross(a, b, *current) * direction;
            if (distance >= 0.) != (previous_distance >= 0.) {
                let t = previous_distance / (previous_distance - distance);
                push(
                    &mut output,
                    &mut count,
                    math::point(
                        (f64::from(previous.x) + (f64::from(current.x) - f64::from(previous.x)) * t)
                            as f32,
                        (f64::from(previous.y) + (f64::from(current.y) - f64::from(previous.y)) * t)
                            as f32,
                    ),
                )?;
            }
            if distance >= 0. {
                push(&mut output, &mut count, *current)?;
            }
            previous = *current;
            previous_distance = distance;
        }
        if count < 3 {
            return Ok(None);
        }
        input = output;
        length = count;
    }
    let origin = input[0];
    let area = (1..length - 1)
        .map(|index| cross(origin, input[index], input[index + 1]))
        .sum::<f64>();
    Ok((area != 0.).then_some((input, length)))
}
/// No fragment can silently overflow its bounded convex scratch geometry.
fn push(
    points: &mut [math::Point; 64],
    length: &mut usize,
    point: math::Point,
) -> Result<(), String> {
    if *length >= points.len() {
        return Err("Canvas visible polygon exceeds its corner budget".into());
    }
    points[*length] = point;
    *length += 1;
    Ok(())
}
/// Double-precision predicates keep large valid Canvas coordinates from losing small intersections.
fn cross(a: math::Point, b: math::Point, c: math::Point) -> f64 {
    (f64::from(b.x) - f64::from(a.x)) * (f64::from(c.y) - f64::from(a.y))
        - (f64::from(b.y) - f64::from(a.y)) * (f64::from(c.x) - f64::from(a.x))
}
/// Native boxes enclose only surviving filled fragments, rather than an ancestor's empty hole.
fn polygon_bounds(points: &[math::Point]) -> protocol::Rect {
    let x = points.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
    let y = points.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
    protocol::Rect {
        x,
        y,
        w: points.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max) - x,
        h: points.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max) - y,
    }
}
