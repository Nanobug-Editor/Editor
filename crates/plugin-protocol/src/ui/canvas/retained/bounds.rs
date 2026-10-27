//! Conservative transformed hulls include old/new pixels and stroke extension without native allocation.
use super::*;

/// Input/foreground changes affect only those owners; unrelated large drawing datasets stay clean.
pub(super) fn metadata_changed(before: &Canvas, after: &Canvas) -> Option<crate::Rect> {
    let mut result = None;
    for (canvas, other) in [(before, after), (after, before)] {
        let scene = canvas.scene.as_ref()?;
        let peer = other.scene.as_ref()?;
        for target in &scene.targets {
            if peer.targets.iter().any(|value| value == target)
                && same_space(scene, peer, target.space.as_deref())
            {
                continue;
            }
            let anchor = CanvasAnchor {
                identity: scene.identity,
                scene_revision: scene.revision,
                target: CanvasTargetRef {
                    id: target.id.clone(),
                    epoch: target.epoch,
                },
                space: target.space.clone(),
                rect: Default::default(),
            };
            if let Some(placement) = anchor.placement(scene)
                && placement.visible
            {
                result = union(
                    result,
                    node_bounds(
                        &SceneNode::Shape {
                            id: String::new(),
                            geometry: target.geometry.clone(),
                            fill: None,
                            stroke: None,
                        },
                        placement.transform,
                    ),
                );
            }
        }
        for overlay in &canvas.overlays {
            if other.overlays.iter().any(|value| value == overlay)
                && same_space(scene, peer, overlay.anchor.space.as_deref())
            {
                continue;
            }
            if let Some(placement) = overlay.anchor.placement(scene)
                && placement.visible
            {
                result = union(
                    result,
                    Some(rect_bounds(
                        crate::Rect {
                            x: overlay.anchor.rect.x + overlay.rect.x,
                            y: overlay.anchor.rect.y + overlay.rect.y,
                            w: overlay.rect.w,
                            h: overlay.rect.h,
                        },
                        placement.transform,
                    )),
                );
            }
        }
    }
    result
}

/// Equal wire metadata may still move or disappear when an ancestor transform/clip changes.
fn same_space(before: &CanvasScene, after: &CanvasScene, id: Option<&str>) -> bool {
    match (before.group_placement(id), after.group_placement(id)) {
        (Some(a), Some(b)) => {
            a.transform == b.transform && a.visible == b.visible && a.clips == b.clips
        }
        (None, None) => true,
        _ => false,
    }
}

/// Metadata may change native focus or overlays outside artwork; include their full conservative hulls.
pub(super) fn canvas(canvas: &Canvas) -> Option<crate::Rect> {
    let scene = canvas.scene.as_ref()?;
    let mut result = scene.roots.iter().fold(None, |result, node| {
        union(result, node_bounds(node, SceneTransform::default()))
    });
    for target in &scene.targets {
        let anchor = CanvasAnchor {
            identity: scene.identity,
            scene_revision: scene.revision,
            target: CanvasTargetRef {
                id: target.id.clone(),
                epoch: target.epoch,
            },
            space: target.space.clone(),
            rect: Default::default(),
        };
        if let Some(placement) = anchor.placement(scene)
            && placement.visible
        {
            result = union(
                result,
                node_bounds(
                    &SceneNode::Shape {
                        id: String::new(),
                        geometry: target.geometry.clone(),
                        fill: None,
                        stroke: None,
                    },
                    placement.transform,
                ),
            );
        }
    }
    for overlay in &canvas.overlays {
        if let Some(placement) = overlay.anchor.placement(scene)
            && placement.visible
        {
            result = union(
                result,
                Some(rect_bounds(
                    crate::Rect {
                        x: overlay.anchor.rect.x + overlay.rect.x,
                        y: overlay.anchor.rect.y + overlay.rect.y,
                        w: overlay.rect.w,
                        h: overlay.rect.h,
                    },
                    placement.transform,
                )),
            );
        }
    }
    result
}

/// Union empty/finite logical hulls; a guest hint can enlarge but never shrink this result.
pub(super) fn union(a: Option<crate::Rect>, b: Option<crate::Rect>) -> Option<crate::Rect> {
    match (a, b) {
        (Some(a), Some(b)) => {
            let x = a.x.min(b.x);
            let y = a.y.min(b.y);
            Some(crate::Rect {
                x,
                y,
                w: (a.x + a.w).max(b.x + b.w) - x,
                h: (a.y + a.h).max(b.y + b.h) - y,
            })
        }
        (a, b) => a.or(b),
    }
}

/// Only affected subtrees contribute. Clip omission safely overestimates curved/even-odd edges.
pub(super) fn affected(scene: &CanvasScene, changed: &BTreeSet<&str>) -> Option<crate::Rect> {
    let mut result = None;
    for node in &scene.roots {
        result = union(
            result,
            affected_node(node, SceneTransform::default(), changed),
        );
    }
    result
}

/// An unchanged parent still carries the transforms and visibility of a changed descendant.
fn affected_node(
    node: &SceneNode,
    transform: SceneTransform,
    changed: &BTreeSet<&str>,
) -> Option<crate::Rect> {
    if changed.contains(node.id()) {
        return node_bounds(node, transform);
    }
    if let SceneNode::Group {
        transform: child,
        opacity,
        children,
        ..
    } = node
    {
        if *opacity == 0. {
            return None;
        }
        return children.iter().fold(None, |result, node| {
            union(
                result,
                affected_node(node, transform.compose(*child), changed),
            )
        });
    }
    None
}

/// Bound the control-point hull under inherited transforms; Bézier curves remain inside that hull.
fn node_bounds(node: &SceneNode, transform: SceneTransform) -> Option<crate::Rect> {
    match node {
        SceneNode::Group {
            transform: child,
            opacity,
            children,
            ..
        } => {
            if *opacity == 0. {
                return None;
            }
            children.iter().fold(None, |result, node| {
                union(result, node_bounds(node, transform.compose(*child)))
            })
        }
        SceneNode::Image { rect, .. } | SceneNode::Paragraph { rect, .. } => {
            Some(rect_bounds(*rect, transform))
        }
        SceneNode::Shape {
            geometry, stroke, ..
        } => {
            let mut result = None;
            let mut point = |x, y| {
                let (x, y) = transform.point(x, y);
                result = union(result, Some(crate::Rect { x, y, w: 0., h: 0. }));
            };
            match geometry {
                SceneGeometry::Rect { rect, .. } => result = Some(rect_bounds(*rect, transform)),
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
            result.map(|mut rect| {
                if let Some(stroke) = stroke {
                    let [a, b, c, d, _, _] = transform.0;
                    let pad = stroke.width
                        * stroke.miter_limit.max(1.)
                        * (a.abs() + c.abs()).max(b.abs() + d.abs());
                    rect.x -= pad;
                    rect.y -= pad;
                    rect.w += 2. * pad;
                    rect.h += 2. * pad;
                }
                rect
            })
        }
    }
}

/// The four projected corners conservatively bound any affine rectangle.
fn rect_bounds(rect: crate::Rect, transform: SceneTransform) -> crate::Rect {
    let points = [
        (rect.x, rect.y),
        (rect.x + rect.w, rect.y),
        (rect.x, rect.y + rect.h),
        (rect.x + rect.w, rect.y + rect.h),
    ]
    .map(|(x, y)| transform.point(x, y));
    let x = points.iter().map(|p| p.0).fold(f32::INFINITY, f32::min);
    let y = points.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
    crate::Rect {
        x,
        y,
        w: points.iter().map(|p| p.0).fold(f32::NEG_INFINITY, f32::max) - x,
        h: points.iter().map(|p| p.1).fold(f32::NEG_INFINITY, f32::max) - y,
    }
}
