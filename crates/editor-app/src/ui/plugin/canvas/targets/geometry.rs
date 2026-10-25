//! Prepare bounded flat hit paths on the existing scene worker, never in a native pointer callback.
use super::*;
use lyon_algorithms::{
    math,
    path::{self, builder::BorderRadii, iterator::PathIterator as _},
};
use protocol::ui::{CanvasScene, SceneFillRule, ScenePathSegment};

/// Prepared paths share scene retirement and its aggregate native vertex allowance.
#[derive(Default)]
pub(in crate::ui::plugin::canvas) struct PreparedTargets {
    regions: Vec<Region>,
    /// Semantic rectangles reuse the admitted target/group clips; no invisible logical dataset is expanded.
    semantic_bounds: BTreeMap<String, protocol::Rect>,
    /// Flat vertices are charged together with visible native tessellation.
    pub vertices: usize,
}
/// One exact filled hit shape and its inexpensive rejection rectangle.
struct Region {
    pointer_events: bool,
    id: String,
    epoch: u64,
    path: path::Path,
    rule: path::FillRule,
    bounds: protocol::Rect,
    clips: Vec<std::sync::Arc<FlatClip>>,
}
/// Shared flattened ancestor clips are charged once per group space, never copied per target.
struct FlatClip {
    path: path::Path,
    rule: path::FillRule,
    bounds: protocol::Rect,
}
impl PreparedTargets {
    /// Flatten admitted source shapes within both the scene and remaining worker vertex budgets.
    pub fn new(scene: &CanvasScene, limit: usize) -> Result<Self, String> {
        let mut output = Self::default();
        let limit = limit.min(128_000);
        let mut spaces: BTreeMap<Option<String>, Vec<std::sync::Arc<FlatClip>>> = BTreeMap::new();
        for target in &scene.targets {
            let placement = scene
                .group_placement(target.space.as_deref())
                .ok_or("Missing target group space")?;
            if !placement.visible {
                continue;
            }
            if !spaces.contains_key(&target.space) {
                let mut clips = Vec::new();
                for (transform, geometry) in &placement.clips {
                    clips.push(std::sync::Arc::new(flat_clip(
                        geometry,
                        *transform,
                        &mut output.vertices,
                        limit,
                    )?));
                }
                spaces.insert(target.space.clone(), clips);
            }
            let clips = spaces[&target.space].clone();
            let source = source_path(&target.geometry);
            let [a, b, c, d, x, y] = placement.transform.0;
            let transform = math::Transform::new(a, b, c, d, x, y);
            let bounds = lyon_algorithms::aabb::bounding_box(source.iter().transformed(&transform));
            let mut builder = path::Path::builder();
            for event in source.iter().transformed(&transform).flattened(0.25) {
                output.vertices += 1;
                if output.vertices > limit {
                    return Err("Canvas hit geometry exceeds its flat-vertex budget".into());
                }
                match event {
                    path::Event::Begin { at } => {
                        builder.begin(at);
                    }
                    path::Event::Line { to, .. } => {
                        builder.line_to(to);
                    }
                    path::Event::End { close, .. } => builder.end(close),
                    _ => unreachable!("flattened native hit paths contain no curves"),
                }
            }
            let rule = match &target.geometry {
                SceneGeometry::Path {
                    rule: SceneFillRule::EvenOdd,
                    ..
                } => path::FillRule::EvenOdd,
                _ => path::FillRule::NonZero,
            };
            let mut bounds = protocol::Rect {
                x: bounds.min.x,
                y: bounds.min.y,
                w: bounds.width(),
                h: bounds.height(),
            };
            for clip in &clips {
                bounds = intersect(bounds, clip.bounds);
            }
            if bounds.w <= 0. || bounds.h <= 0. {
                continue;
            }
            output.regions.push(Region {
                pointer_events: target.pointer_events,
                id: target.id.clone(),
                epoch: target.epoch,
                path: builder.build(),
                rule,
                bounds,
                clips,
            });
        }
        for node in &scene.semantics {
            let rect = if let Some(id) = &node.target {
                scene
                    .targets
                    .iter()
                    .find(|target| target.id == *id)
                    .and_then(|target| output.bounds(id, target.epoch))
            } else if let Some(rect) = node.rect {
                let placement = scene
                    .group_placement(node.space.as_deref())
                    .ok_or("Missing semantic group space")?;
                if !placement.visible {
                    continue;
                }
                if !spaces.contains_key(&node.space) {
                    let clips = placement
                        .clips
                        .iter()
                        .map(|(transform, geometry)| {
                            flat_clip(geometry, *transform, &mut output.vertices, limit)
                                .map(std::sync::Arc::new)
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    spaces.insert(node.space.clone(), clips);
                }
                let points = [
                    placement.transform.point(rect.x, rect.y),
                    placement.transform.point(rect.x + rect.w, rect.y),
                    placement.transform.point(rect.x, rect.y + rect.h),
                    placement.transform.point(rect.x + rect.w, rect.y + rect.h),
                ];
                let x = points.iter().map(|p| p.0).fold(f32::INFINITY, f32::min);
                let y = points.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
                let mut rect = protocol::Rect {
                    x,
                    y,
                    w: points.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max) - x,
                    h: points.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max) - y,
                };
                for clip in &spaces[&node.space] {
                    rect = intersect(rect, clip.bounds);
                }
                (rect.w > 0. && rect.h > 0.).then_some(rect)
            } else {
                None
            };
            if let Some(rect) = rect {
                output.semantic_bounds.insert(node.id.clone(), rect);
            }
        }
        Ok(output)
    }
    /// Hit the actual back-to-front fill, retaining original target identity rather than array indices.
    pub fn hit(&self, x: f32, y: f32) -> Option<(String, u64)> {
        let point = math::point(x, y);
        self.regions
            .iter()
            .rev()
            .find(|region| {
                region.pointer_events
                    && region.bounds.contains(x, y)
                    && lyon_algorithms::hit_test::hit_test_path(
                        &point,
                        region.path.iter(),
                        region.rule,
                        0.25,
                    )
                    && region.clips.iter().all(|clip| {
                        lyon_algorithms::hit_test::hit_test_path(
                            &point,
                            clip.path.iter(),
                            clip.rule,
                            0.25,
                        )
                    })
            })
            .map(|region| (region.id.clone(), region.epoch))
    }
    /// Projected focus/accessibility boxes conservatively enclose the same exact native hit shape.
    pub fn bounds(&self, id: &str, epoch: u64) -> Option<protocol::Rect> {
        self.regions
            .iter()
            .find(|region| region.id == id && region.epoch == epoch)
            .map(|region| region.bounds)
    }
    /// The semantic adapter cannot expose a geometry rejected or hidden by the preparation worker.
    pub fn semantic_bounds(&self, id: &str) -> Option<protocol::Rect> {
        self.semantic_bounds.get(id).copied()
    }
}
/// Compile one transformed clip within the same pre-allocation vertex quota as all target paths.
fn flat_clip(
    geometry: &SceneGeometry,
    transform: protocol::ui::SceneTransform,
    vertices: &mut usize,
    limit: usize,
) -> Result<FlatClip, String> {
    let source = source_path(geometry);
    let [a, b, c, d, x, y] = transform.0;
    let transform = math::Transform::new(a, b, c, d, x, y);
    let bounds = lyon_algorithms::aabb::bounding_box(source.iter().transformed(&transform));
    let mut builder = path::Path::builder();
    for event in source.iter().transformed(&transform).flattened(0.25) {
        *vertices += 1;
        if *vertices > limit {
            return Err("Canvas clips exceed their hit vertex budget".into());
        }
        match event {
            path::Event::Begin { at } => {
                builder.begin(at);
            }
            path::Event::Line { to, .. } => {
                builder.line_to(to);
            }
            path::Event::End { close, .. } => builder.end(close),
            _ => unreachable!("flat clip has no curves"),
        }
    }
    let rule = if matches!(
        geometry,
        SceneGeometry::Path {
            rule: SceneFillRule::EvenOdd,
            ..
        }
    ) {
        path::FillRule::EvenOdd
    } else {
        path::FillRule::NonZero
    };
    Ok(FlatClip {
        path: builder.build(),
        rule,
        bounds: protocol::Rect {
            x: bounds.min.x,
            y: bounds.min.y,
            w: bounds.width(),
            h: bounds.height(),
        },
    })
}
/// Conservative native focus/semantic box uses every ancestor clip's projected intersection.
fn intersect(left: protocol::Rect, right: protocol::Rect) -> protocol::Rect {
    let x = left.x.max(right.x);
    let y = left.y.max(right.y);
    protocol::Rect {
        x,
        y,
        w: (left.x + left.w).min(right.x + right.w).max(x) - x,
        h: (left.y + left.h).min(right.y + right.h).max(y) - y,
    }
}
/// Build a portable fill path once; open contours are implicitly closed by filled hit testing.
fn source_path(geometry: &SceneGeometry) -> path::Path {
    let mut builder = path::Path::builder();
    match geometry {
        SceneGeometry::Rect { rect, radii } => builder.add_rounded_rectangle(
            &math::Box2D::new(
                math::point(rect.x, rect.y),
                math::point(rect.x + rect.w, rect.y + rect.h),
            ),
            &BorderRadii {
                top_left: radii[0],
                top_right: radii[1],
                bottom_right: radii[2],
                bottom_left: radii[3],
            },
            path::Winding::Positive,
        ),
        SceneGeometry::Path { segments, .. } => {
            let mut open = false;
            for segment in segments {
                match *segment {
                    ScenePathSegment::Move { x, y } => {
                        if open {
                            builder.end(false);
                        }
                        builder.begin(math::point(x, y));
                        open = true;
                    }
                    ScenePathSegment::Line { x, y } => {
                        builder.line_to(math::point(x, y));
                    }
                    ScenePathSegment::Quadratic { cx, cy, x, y } => {
                        builder.quadratic_bezier_to(math::point(cx, cy), math::point(x, y));
                    }
                    ScenePathSegment::Cubic {
                        x1,
                        y1,
                        x2,
                        y2,
                        x,
                        y,
                    } => {
                        builder.cubic_bezier_to(
                            math::point(x1, y1),
                            math::point(x2, y2),
                            math::point(x, y),
                        );
                    }
                    ScenePathSegment::Close => {
                        builder.end(true);
                        open = false;
                    }
                }
            }
            if open {
                builder.end(false);
            }
        }
    }
    builder.build()
}
