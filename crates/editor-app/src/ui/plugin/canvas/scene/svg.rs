//! Serialize validated complex layers to inert SVG for the existing bounded background rasterizer.
use super::bounds::geometry_bounds;
use super::*;
use std::fmt::Write as _;
mod budget;
use budget::SvgText;

/// Produce one finite SVG and its canvas-space bounds; no user XML or external resource is emitted.
pub(super) fn encode(
    scene: &CanvasScene,
    photos: &BTreeMap<String, std::sync::Arc<super::super::super::images::Photo>>,
    typography: &Typography,
    pixel_budget: u64,
    draft: Option<&paragraph::NativeDraft>,
) -> Result<
    (
        String,
        protocol::Rect,
        BTreeMap<String, std::sync::Arc<Vec<u8>>>,
    ),
    String,
> {
    let (defs, body) = SvgText::pair();
    let mut writer = Writer {
        defs,
        body,
        images: BTreeMap::new(),
        pngs: BTreeMap::new(),
        image_work: 16 * 1024 * 1024,
        next: 0,
        bounds: None,
        typography: typography.clone(),
        pixel_budget,
        draft,
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
        writer.pngs,
    ))
}
/// Own generated numeric IDs and the union of transformed primitive bounds for one compilation.
struct Writer<'a> {
    typography: Typography,
    pixel_budget: u64,
    defs: SvgText,
    body: SvgText,
    /// A decoded source is encoded once; placements only reference generated numeric IDs.
    images: BTreeMap<String, usize>,
    /// Only generated URIs resolve during parsing; every placement shares its encoded Arc.
    pngs: BTreeMap<String, std::sync::Arc<Vec<u8>>>,
    /// Aggregate decode and conservative 2x-density sampling pixels, charged before resvg.
    image_work: u64,
    next: usize,
    bounds: Option<protocol::Rect>,
    /// Only this admitted paragraph's native text/decoration may differ from committed source.
    draft: Option<&'a paragraph::NativeDraft>,
}
impl Writer<'_> {
    /// Encode one validated node, preserving true group opacity and local clip coordinates.
    fn node(
        &mut self,
        node: &SceneNode,
        parent: SceneTransform,
        photos: &BTreeMap<String, std::sync::Arc<super::super::super::images::Photo>>,
    ) -> Result<(), String> {
        self.body.check()?;
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
                let local = geometry_bounds(geometry, SceneTransform::default(), 1.);
                let mut bounds = geometry_bounds(geometry, parent, 1.);
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
                let bounds = geometry_bounds(
                    &SceneGeometry::Rect {
                        rect: *rect,
                        radii: [0.; 4],
                    },
                    parent,
                    1.,
                );
                self.include(bounds);
                let bitmap = photos
                    .get(image)
                    .and_then(|photo| photo.decoded.as_ref().ok()?.as_ref());
                if let Some(bitmap) = bitmap {
                    // resvg decodes PNGs per placement even when the encoded Arc is shared.
                    // Charge decoding plus projected sampling before parsing or raster allocation.
                    let work = u64::from(bitmap.width) * u64::from(bitmap.height)
                        + (f64::from(bounds.w) * f64::from(bounds.h) * 4.).ceil() as u64;
                    self.image_work = self
                        .image_work
                        .checked_sub(work)
                        .ok_or("Canvas images exceed their raster work budget")?;
                    let id = if let Some(id) = self.images.get(image) {
                        *id
                    } else {
                        // Reuse only admitted pixels: original SVG filters must never execute again
                        // under a different transform. PNG stays outside XML: usvg would otherwise
                        // allocate a new base64 buffer for every <use> placement.
                        let encoded = budget::png(bitmap, self.defs.remaining())?;
                        self.defs.charge(encoded.len())?;
                        let id = self.next;
                        self.next += 1;
                        let uri = format!("nanobug-canvas-image:{id}");
                        self.pngs.insert(uri.clone(), std::sync::Arc::new(encoded));
                        let _ = write!(
                            self.defs,
                            "<image id=\"i{id}\" width=\"1\" height=\"1\" preserveAspectRatio=\"none\" href=\"{uri}\"/>"
                        );
                        self.defs.check()?;
                        self.images.insert(image.clone(), id);
                        id
                    };
                    let _ = write!(
                        self.body,
                        "<use href=\"#i{id}\" transform=\"matrix({} 0 0 {} {} {})\"/>",
                        rect.w, rect.h, rect.x, rect.y
                    );
                } else {
                    let _ = write!(
                        self.body,
                        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#808080\" fill-opacity=\"0.133\"/>",
                        rect.x, rect.y, rect.w, rect.h
                    );
                }
            }
            SceneNode::Paragraph {
                id: paragraph,
                layout_key,
                text,
                rect,
                style,
            } => {
                let draft = self.draft.filter(|draft| {
                    draft.paragraph == *paragraph && draft.layout_key == *layout_key
                });
                let mut style = style.clone();
                if draft.is_some_and(|draft| draft.presentation.transient) {
                    style.runs.clear();
                }
                let font_style = style.font.clone().over(self.typography.font.clone());
                let mut face = font(font_style.family.unwrap_or_else(|| "Segoe UI".into()));
                if font_style.bold.unwrap_or(false) {
                    face = face.bold();
                }
                let context = editor_ui::text::paragraph::CanvasTextLayout {
                    font: face,
                    font_size: px(font_style.size_px.unwrap_or(14.)),
                    color: rgb(self.typography.foreground).into(),
                    bounds: Bounds::new(point(px(0.), px(0.)), size(px(rect.w), px(rect.h))),
                    style: style.clone(),
                    projection: None,
                };
                let [a, b, c, d, _, _] = parent.0;
                let density = (2. * (a.abs() + c.abs()).max(b.abs() + d.abs())).clamp(0.25, 4.);
                let pixels = if let Some(draft) = draft {
                    editor_ui::text::paragraph::rasterize_native_draft(
                        &draft.presentation,
                        &context,
                        density,
                        self.pixel_budget,
                        &mut self.image_work,
                    )?
                } else {
                    editor_ui::text::paragraph::rasterize_native_paragraph(
                        text.clone().into(),
                        &context,
                        density,
                        self.pixel_budget,
                        &mut self.image_work,
                    )?
                };
                let png = budget::png_rgba(
                    &pixels.rgba,
                    pixels.width,
                    pixels.height,
                    self.defs.remaining(),
                )?;
                self.defs.charge(png.len())?;
                let id = self.next;
                self.next += 1;
                let uri = format!("native-paragraph-{id}.png");
                self.pngs.insert(uri.clone(), std::sync::Arc::new(png));
                self.include(geometry_bounds(
                    &SceneGeometry::Rect {
                        rect: *rect,
                        radii: [0.; 4],
                    },
                    parent,
                    1.,
                ));
                let _ = write!(
                    self.body,
                    "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" href=\"{}\"/>",
                    rect.x, rect.y, rect.w, rect.h, uri
                );
            }
        }
        self.body.check()
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
