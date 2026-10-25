//! Serialize validated complex layers to inert SVG for the existing bounded background rasterizer.
use super::*;
use std::fmt::Write as _;

/// Produce one finite SVG and its canvas-space bounds; no user XML or external resource is emitted.
pub(super) fn encode(
    scene: &CanvasScene,
    photos: &BTreeMap<String, std::sync::Arc<super::super::super::images::Photo>>,
) -> Result<(String, protocol::Rect), String> {
    let mut writer = Writer {
        defs: String::new(),
        body: String::new(),
        next: 0,
        bounds: None,
    };
    for root in &scene.roots {
        writer.node(root, SceneTransform::default(), photos)?;
    }
    let bounds = writer.bounds.unwrap_or(protocol::Rect {
        x: 0.,
        y: 0.,
        w: 1.,
        h: 1.,
    });
    let width = bounds.w.max(1.);
    let height = bounds.h.max(1.);
    let source = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"{} {} {width} {height}\"><defs>{}</defs>{}</svg>",
        bounds.x, bounds.y, writer.defs, writer.body
    );
    if source.len() > 4 * 1024 * 1024 {
        return Err("Canvas raster source exceeds its expansion budget".into());
    }
    Ok((
        source,
        protocol::Rect {
            w: width,
            h: height,
            ..bounds
        },
    ))
}
/// Own generated numeric IDs and the union of transformed primitive bounds for one compilation.
struct Writer {
    defs: String,
    body: String,
    next: usize,
    bounds: Option<protocol::Rect>,
}
impl Writer {
    /// Encode one validated node, preserving true group opacity and local clip coordinates.
    fn node(
        &mut self,
        node: &SceneNode,
        parent: SceneTransform,
        photos: &BTreeMap<String, std::sync::Arc<super::super::super::images::Photo>>,
    ) -> Result<(), String> {
        match node {
            SceneNode::Group {
                transform,
                clip,
                opacity,
                children,
                ..
            } => {
                let clip_attr = if let Some(clip) = clip {
                    let id = self.next;
                    self.next += 1;
                    // SVG clipping has its own winding property; fill-rule does not cut clip holes.
                    let _ = write!(
                        self.defs,
                        "<clipPath id=\"c{id}\" clipPathUnits=\"userSpaceOnUse\"><path d=\"{}\" clip-rule=\"{}\"/></clipPath>",
                        path(clip),
                        rule(clip)
                    );
                    format!(" clip-path=\"url(#c{id})\"")
                } else {
                    String::new()
                };
                let [a, b, c, d, x, y] = transform.0;
                let _ = write!(
                    self.body,
                    "<g transform=\"matrix({a} {b} {c} {d} {x} {y})\" opacity=\"{opacity}\"{clip_attr}>"
                );
                for child in children {
                    self.node(child, parent.compose(*transform), photos)?;
                }
                self.body.push_str("</g>");
            }
            SceneNode::Shape {
                geometry,
                fill,
                stroke,
                ..
            } => {
                let local = geometry_bounds(geometry, SceneTransform::default());
                let mut bounds = geometry_bounds(geometry, parent);
                if let Some(stroke) = stroke {
                    let [a, b, c, d, _, _] = parent.0;
                    // Row sums conservatively bound each projected axis, including shear.
                    let margin = stroke.width
                        * 0.5
                        * stroke.miter_limit
                        * (a.abs() + c.abs()).max(b.abs() + d.abs());
                    bounds.x -= margin;
                    bounds.y -= margin;
                    bounds.w += 2. * margin;
                    bounds.h += 2. * margin;
                }
                self.include(bounds);
                let fill = fill
                    .as_ref()
                    .map(|fill| self.brush(fill, "fill", local))
                    .unwrap_or_else(|| "fill=\"none\"".into());
                let stroke=stroke.as_ref().map(|stroke| {
                    let brush=self.brush(&stroke.brush,"stroke",local);
                    let cap=match stroke.cap {SceneLineCap::Butt=>"butt",SceneLineCap::Round=>"round",SceneLineCap::Square=>"square"};
                    let join=match stroke.join {SceneLineJoin::Miter=>"miter",SceneLineJoin::Round=>"round",SceneLineJoin::Bevel=>"bevel"};
                    let dash=if stroke.dash.is_empty(){String::new()}else{format!(" stroke-dasharray=\"{}\"",stroke.dash.iter().map(f32::to_string).collect::<Vec<_>>().join(" "))};
                    format!("{brush} stroke-width=\"{}\" stroke-linecap=\"{cap}\" stroke-linejoin=\"{join}\" stroke-miterlimit=\"{}\"{dash}",stroke.width,stroke.miter_limit)
                }).unwrap_or_default();
                let _ = write!(
                    self.body,
                    "<path d=\"{}\" fill-rule=\"{}\" {fill} {stroke}/>",
                    path(geometry),
                    rule(geometry)
                );
            }
            SceneNode::Image { image, rect, .. } => {
                self.include(geometry_bounds(
                    &SceneGeometry::Rect {
                        rect: *rect,
                        radii: [0.; 4],
                    },
                    parent,
                ));
                let bytes = photos
                    .get(image)
                    .filter(|photo| matches!(photo.decoded, Ok(Some(_))))
                    .and_then(|photo| match &photo.resource.state {
                        plugin_runtime::ImageState::Ready(bytes) => Some(bytes),
                        _ => None,
                    });
                if let Some(bytes) = bytes {
                    use base64::Engine as _;
                    let mime = match image::guess_format(bytes) {
                        Ok(image::ImageFormat::Png) => "image/png",
                        Ok(image::ImageFormat::Jpeg) => "image/jpeg",
                        Ok(image::ImageFormat::Gif) => "image/gif",
                        Ok(image::ImageFormat::WebP) => "image/webp",
                        _ => "image/svg+xml",
                    };
                    let encoded =
                        base64::engine::general_purpose::STANDARD.encode(bytes.as_slice());
                    let _ = write!(
                        self.body,
                        "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" href=\"data:{mime};base64,{encoded}\"/>",
                        rect.x, rect.y, rect.w, rect.h
                    );
                } else {
                    let _ = write!(
                        self.body,
                        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#808080\" fill-opacity=\"0.133\"/>",
                        rect.x, rect.y, rect.w, rect.h
                    );
                }
            }
        }
        Ok(())
    }
    /// Encode a solid or arbitrary bounded gradient using generated references only.
    fn brush(&mut self, brush: &SceneBrush, attribute: &str, bounds: protocol::Rect) -> String {
        match brush {
            SceneBrush::Solid { rgba } => format!(
                "{attribute}=\"#{:06x}\" {attribute}-opacity=\"{}\"",
                rgba >> 8,
                (rgba & 255) as f32 / 255.
            ),
            SceneBrush::LinearGradient { angle, stops } => {
                let id = self.next;
                self.next += 1;
                let radians = angle.rem_euclid(360.).to_radians();
                let (dx, dy) = (radians.sin(), -radians.cos());
                let extent = (bounds.w * dx.abs() + bounds.h * dy.abs()) * 0.5;
                let (cx, cy) = (bounds.x + bounds.w * 0.5, bounds.y + bounds.h * 0.5);
                let _ = write!(
                    self.defs,
                    "<linearGradient id=\"g{id}\" gradientUnits=\"userSpaceOnUse\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" color-interpolation=\"sRGB\">",
                    cx - dx * extent,
                    cy - dy * extent,
                    cx + dx * extent,
                    cy + dy * extent
                );
                for stop in stops {
                    let _ = write!(
                        self.defs,
                        "<stop offset=\"{}\" stop-color=\"#{:06x}\" stop-opacity=\"{}\"/>",
                        stop.offset,
                        stop.rgba >> 8,
                        (stop.rgba & 255) as f32 / 255.
                    );
                }
                self.defs.push_str("</linearGradient>");
                format!("{attribute}=\"url(#g{id})\"")
            }
        }
    }
    /// Expand the canvas-space allocation extent before the existing raster quotas are applied.
    fn include(&mut self, rect: protocol::Rect) {
        self.bounds = Some(if let Some(old) = self.bounds {
            let x = old.x.min(rect.x);
            let y = old.y.min(rect.y);
            protocol::Rect {
                x,
                y,
                w: (old.x + old.w).max(rect.x + rect.w) - x,
                h: (old.y + old.h).max(rect.y + rect.h) - y,
            }
        } else {
            rect
        });
    }
}
/// Serialize only validated numeric geometry, including four independently clamped corner radii.
fn path(geometry: &SceneGeometry) -> String {
    let mut out = String::new();
    match geometry {
        SceneGeometry::Rect { rect, radii } => {
            let [a, b, c, d] = radii.map(|radius| radius.min(rect.w.min(rect.h) * 0.5));
            let (x, y, r, bottom) = (rect.x, rect.y, rect.x + rect.w, rect.y + rect.h);
            // Circular arcs share the native PathBuilder radius policy, including zero-radius corners.
            let _ = write!(
                out,
                "M{} {y} L{} {y} A{b} {b} 0 0 1 {r} {} L{r} {} A{c} {c} 0 0 1 {} {bottom} L{} {bottom} A{d} {d} 0 0 1 {x} {} L{x} {} A{a} {a} 0 0 1 {} {y} Z",
                x + a,
                r - b,
                y + b,
                bottom - c,
                r - c,
                x + d,
                bottom - d,
                y + a,
                x + a
            );
        }
        SceneGeometry::Path { segments, .. } => {
            for segment in segments {
                match *segment {
                    ScenePathSegment::Move { x, y } => {
                        let _ = write!(out, "M{x} {y} ");
                    }
                    ScenePathSegment::Line { x, y } => {
                        let _ = write!(out, "L{x} {y} ");
                    }
                    ScenePathSegment::Quadratic { cx, cy, x, y } => {
                        let _ = write!(out, "Q{cx} {cy} {x} {y} ");
                    }
                    ScenePathSegment::Cubic {
                        x1,
                        y1,
                        x2,
                        y2,
                        x,
                        y,
                    } => {
                        let _ = write!(out, "C{x1} {y1} {x2} {y2} {x} {y} ");
                    }
                    ScenePathSegment::Close => out.push_str("Z "),
                }
            }
        }
    }
    out
}
/// Fill winding has the same spelling in the inert SVG adapter.
fn rule(geometry: &SceneGeometry) -> &'static str {
    match geometry {
        SceneGeometry::Path {
            rule: SceneFillRule::EvenOdd,
            ..
        } => "evenodd",
        _ => "nonzero",
    }
}
/// Control-point hull bounds conservatively enclose Bézier curves under an affine transform.
fn geometry_bounds(geometry: &SceneGeometry, transform: SceneTransform) -> protocol::Rect {
    let mut points = Vec::new();
    match geometry {
        SceneGeometry::Rect { rect, .. } => points.extend([
            (rect.x, rect.y),
            (rect.x + rect.w, rect.y),
            (rect.x, rect.y + rect.h),
            (rect.x + rect.w, rect.y + rect.h),
        ]),
        SceneGeometry::Path { segments, .. } => {
            for segment in segments {
                match *segment {
                    ScenePathSegment::Move { x, y } | ScenePathSegment::Line { x, y } => {
                        points.push((x, y))
                    }
                    ScenePathSegment::Quadratic { cx, cy, x, y } => {
                        points.extend([(cx, cy), (x, y)])
                    }
                    ScenePathSegment::Cubic {
                        x1,
                        y1,
                        x2,
                        y2,
                        x,
                        y,
                    } => points.extend([(x1, y1), (x2, y2), (x, y)]),
                    ScenePathSegment::Close => {}
                }
            }
        }
    }
    let mut min = (f32::INFINITY, f32::INFINITY);
    let mut max = (f32::NEG_INFINITY, f32::NEG_INFINITY);
    for (x, y) in points {
        let (x, y) = transform.point(x, y);
        min.0 = min.0.min(x);
        min.1 = min.1.min(y);
        max.0 = max.0.max(x);
        max.1 = max.1.max(y);
    }
    protocol::Rect {
        x: min.0,
        y: min.1,
        w: (max.0 - min.0).max(1.),
        h: (max.1 - min.1).max(1.),
    }
}
