//! Original-byte queries use native base glyphs and the same complete affine/clip chain as raster paint.
use super::super::bounds::geometry_bounds;
use super::*;
use protocol::ui::{
    CanvasCaretRect, CanvasSelectionRects, CanvasTextAffinity, CanvasTextQuery, CanvasTextResult,
};
use std::hash::{Hash, Hasher};

/// Immutable paragraph placement; clip paths borrow the admitted scene rather than copying segments.
struct Located<'a> {
    drawing: ParagraphDrawing,
    transform: SceneTransform,
    clips: Vec<(SceneTransform, &'a SceneGeometry)>,
    visible: bool,
}
/// Locate original source under its complete ancestor chain, including transparent/complex groups.
fn locate<'a>(
    node: &'a SceneNode,
    parent: SceneTransform,
    clips: &mut Vec<(SceneTransform, &'a SceneGeometry)>,
    visible: bool,
    query: &CanvasTextQuery,
) -> Option<Result<Located<'a>, String>> {
    match node {
        SceneNode::Paragraph {
            id,
            layout_key,
            text,
            rect,
            style,
        } if id == &query.paragraph && layout_key == &query.layout_key => Some(
            ParagraphDrawing::new(
                id,
                layout_key,
                text,
                *rect,
                style,
                SceneTransform::default(),
            )
            .map(|drawing| Located {
                drawing,
                transform: parent,
                clips: clips.clone(),
                visible,
            }),
        ),
        SceneNode::Group {
            transform,
            clip,
            opacity,
            children,
            ..
        } => {
            let transform = parent.compose(*transform);
            if let Some(clip) = clip {
                clips.push((transform, clip));
            }
            let found = children
                .iter()
                .find_map(|node| locate(node, transform, clips, visible && *opacity > 0., query));
            if clip.is_some() {
                clips.pop();
            }
            found
        }
        _ => None,
    }
}
impl Located<'_> {
    /// The native base layout has the exact font/glyph advances used by the worker's paragraph pixels.
    fn layout(
        &self,
        view: &CanvasView,
        window: &Window,
    ) -> Result<editor_ui::text::paragraph::CanvasParagraph, String> {
        if !self.visible {
            return Err("Paragraph is hidden by its ancestor opacity".into());
        }
        let context = self.drawing.context(
            &view.font,
            view.foreground,
            protocol::Rect {
                x: 0.,
                y: 0.,
                w: 1.,
                h: 1.,
            },
            Bounds::default(),
        );
        editor_ui::text::paragraph::CanvasParagraph::new(
            self.drawing.text.clone().into(),
            context.font,
            context.font_size,
            context.color,
            context.bounds,
            &context.style,
            window,
        )
    }
    /// Return a conservative projected rectangle intersected with every ancestor clip and native viewport.
    fn rect(&self, view: &CanvasView, bounds: Bounds<Pixels>) -> Option<protocol::Rect> {
        let source = protocol::Rect {
            x: bounds.origin.x / px(1.),
            y: bounds.origin.y / px(1.),
            w: bounds.size.width / px(1.),
            h: bounds.size.height / px(1.),
        };
        let mut rect = geometry_bounds(
            &SceneGeometry::Rect {
                rect: source,
                radii: [0.; 4],
            },
            self.transform,
            0.,
        );
        for (transform, clip) in &self.clips {
            rect = intersect(rect, geometry_bounds(clip, *transform, 0.))?;
        }
        let projected = view.project(rect, view.bounds);
        content_rect(view, rect_bounds(projected, view.bounds.origin))
    }
    /// Caret thickness follows the native one-window-logical-pixel policy independently of group/viewport scale.
    fn caret(&self, view: &CanvasView, mut rect: Bounds<Pixels>) -> Option<protocol::Rect> {
        let basis = view.project(
            protocol::Rect {
                x: 0.,
                y: 0.,
                w: 1.,
                h: 1.,
            },
            view.bounds,
        );
        let [a, b, _, _, _, _] = self.transform.0;
        let scale = (a * a + b * b).sqrt() * basis.w.abs();
        rect.size.width =
            px((1. / scale)
                .min(self.drawing.rect.x + self.drawing.rect.w - rect.origin.x / px(1.)));
        (rect.size.width > px(0.))
            .then(|| self.rect(view, rect))
            .flatten()
    }
}

/// Query native glyph geometry, then map results through the exact admitted scene/viewport transform.
pub(in crate::ui::plugin::canvas) fn query(
    view: &CanvasView,
    window: &Window,
) -> Vec<CanvasTextResult> {
    let Some(scene) = &view.drawing.scene else {
        return Vec::new();
    };
    let mut fragments = 8192usize;
    view.drawing
        .text_queries
        .iter()
        .map(|request| {
            let resolved = scene
                .roots
                .iter()
                .find_map(|node| {
                    locate(
                        node,
                        SceneTransform::default(),
                        &mut Vec::new(),
                        true,
                        request,
                    )
                })
                .unwrap_or_else(|| Err("Paragraph identity was replaced".into()))
                .and_then(|drawing| {
                    let layout = drawing.layout(view, window)?;
                    let inverse = drawing
                        .transform
                        .inverse()
                        .ok_or("Paragraph transform is not invertible")?;
                    let hits = request
                        .points
                        .iter()
                        .map(|point| {
                            let (x, y) = inverse.point(point.x, point.y);
                            layout.byte_for_point(gpui_kit::point(px(x), px(y)))
                        })
                        .collect();
                    let positions = request
                        .positions
                        .iter()
                        .map(|position| CanvasCaretRect {
                            byte: position.byte,
                            affinity: position.affinity,
                            rect: layout
                                .caret_bounds(
                                    position.byte,
                                    position.affinity == CanvasTextAffinity::Downstream,
                                )
                                .and_then(|rect| drawing.caret(view, rect)),
                        })
                        .collect();
                    let selections = request
                        .selections
                        .iter()
                        .map(|range| {
                            Ok(CanvasSelectionRects {
                                range: range.clone(),
                                rects: layout
                                    .selection_rects(range.start..range.end)?
                                    .into_iter()
                                    .filter_map(|rect| drawing.rect(view, rect))
                                    .collect(),
                            })
                        })
                        .collect::<Result<Vec<_>, String>>()?;
                    let lines = layout
                        .visible_lines()?
                        .into_iter()
                        .filter_map(|mut line| {
                            line.rect = drawing
                                .rect(view, rect_bounds(line.rect, point(px(0.), px(0.))))?;
                            Some(line)
                        })
                        .collect::<Vec<_>>();
                    let count = lines.len()
                        + selections
                            .iter()
                            .map(|selection| selection.rects.len())
                            .sum::<usize>();
                    fragments = fragments
                        .checked_sub(count)
                        .ok_or("Native text geometry exceeds its shared fragment budget")?;
                    let mut environment = std::collections::hash_map::DefaultHasher::new();
                    layout.font_environment().hash(&mut environment);
                    for value in drawing.transform.0 {
                        value.to_bits().hash(&mut environment);
                    }
                    let basis = view.project(
                        protocol::Rect {
                            x: 0.,
                            y: 0.,
                            w: 1.,
                            h: 1.,
                        },
                        view.bounds,
                    );
                    basis.w.to_bits().hash(&mut environment);
                    basis.h.to_bits().hash(&mut environment);
                    Ok((
                        environment.finish().max(1),
                        hits,
                        positions,
                        selections,
                        lines,
                    ))
                });
            let (font_environment, hits, positions, selections, lines, error) = match resolved {
                Ok((font, hits, positions, selections, lines)) => {
                    (font, hits, positions, selections, lines, None)
                }
                Err(error) => (
                    0,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Some(error.chars().take(128).collect()),
                ),
            };
            CanvasTextResult {
                id: request.id.clone(),
                paragraph: request.paragraph.clone(),
                layout_key: request.layout_key.clone(),
                points: request.points.clone(),
                font_environment,
                hits,
                positions,
                selections,
                lines,
                error,
            }
        })
        .collect()
}
/// Intersect projected content boxes without changing their original source-byte interpretation.
fn intersect(left: protocol::Rect, right: protocol::Rect) -> Option<protocol::Rect> {
    let x = left.x.max(right.x);
    let y = left.y.max(right.y);
    let w = (left.x + left.w).min(right.x + right.w) - x;
    let h = (left.y + left.h).min(right.y + right.h) - y;
    (w > 0. && h > 0.).then_some(protocol::Rect { x, y, w, h })
}
/// Native output geometry is clipped in window coordinates, then returned in Canvas content units.
fn content_rect(view: &CanvasView, rect: Bounds<Pixels>) -> Option<protocol::Rect> {
    let origin = point(
        rect.origin.x.max(view.bounds.origin.x),
        rect.origin.y.max(view.bounds.origin.y),
    );
    let right =
        (rect.origin.x + rect.size.width).min(view.bounds.origin.x + view.bounds.size.width);
    let bottom =
        (rect.origin.y + rect.size.height).min(view.bounds.origin.y + view.bounds.size.height);
    if right <= origin.x || bottom <= origin.y {
        return None;
    }
    let basis = view.project(
        protocol::Rect {
            x: 0.,
            y: 0.,
            w: 1.,
            h: 1.,
        },
        view.bounds,
    );
    Some(protocol::Rect {
        x: ((origin.x - view.bounds.origin.x) / px(1.) - basis.x) / basis.w,
        y: ((origin.y - view.bounds.origin.y) / px(1.) - basis.y) / basis.h,
        w: (right - origin.x) / px(1.) / basis.w,
        h: (bottom - origin.y) / px(1.) / basis.h,
    })
}
