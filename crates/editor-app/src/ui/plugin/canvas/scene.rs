//! Adapt portable scene values to cached GPUI paths on the existing background image worker.
use super::*;
use gpui_kit::{Background, FillOptions, FillRule, Path, PathBuilder, PathStyle, rgba};
use protocol::ui::{
    CanvasScene, SceneBrush, SceneFillRule, SceneGeometry, SceneLineCap, SceneLineJoin, SceneNode,
    ScenePathSegment, SceneStroke, SceneTransform,
};
mod svg;

/// Worker result retains its exact source so late preparation cannot paint a replacement scene.
pub(in crate::ui::plugin) struct PreparedScene {
    /// Complete public source identity, including geometry and resource references.
    pub source: CanvasScene,
    /// Bounded tessellation or an explicit preparation failure; never partial successful artwork.
    pub drawing: Result<SceneDrawing, String>,
    /// Source authority participates even when a guest keeps identical drawing IDs across previews.
    pub document_source: Option<protocol::api::DocumentVersion>,
    /// Binary file authority is independent of the optional editable text source.
    pub file_source: Option<protocol::api::FileVersion>,
    /// Only referenced native images retain preparation dependencies, not unrelated sibling images.
    photos: BTreeMap<String, std::sync::Arc<super::super::images::Photo>>,
    /// Failed bounded preparations may retry when another scene releases resident capacity.
    limits: (u64, usize),
}
impl PreparedScene {
    /// Tessellate `source` outside native UI callbacks and retain its admission/preparation result.
    pub fn new(
        source: CanvasScene,
        owner: &Document,
        photos: BTreeMap<String, std::sync::Arc<super::super::images::Photo>>,
        limits: (u64, usize),
        mut rasterize: impl FnMut(
            &str,
            protocol::Rect,
            &BTreeMap<String, std::sync::Arc<Vec<u8>>>,
        ) -> Option<super::super::images::VectorImage>,
    ) -> Self {
        let drawing = source.validate().and_then(|()| {
            SceneDrawing::compile(&source, &photos, limits).or_else(|_| {
                let (svg, bounds, images) = svg::encode(&source, &photos)?;
                let raster = rasterize(&svg, bounds, &images)
                    .ok_or("Canvas raster preparation exceeded its budget or failed".to_owned())?;
                Ok(SceneDrawing {
                    paths: Vec::new(),
                    raster: Some(raster),
                    bitmaps: BTreeMap::new(),
                    vertices: 0,
                })
            })
        });
        Self {
            source,
            drawing,
            document_source: owner.source.clone(),
            file_source: owner.file.clone(),
            photos,
            limits,
        }
    }
    /// Geometry caches may survive UI revisions, never resource completion or source replacement.
    pub fn current(
        &self,
        source: &CanvasScene,
        owner: &Document,
        photos: &BTreeMap<String, std::sync::Arc<super::super::images::Photo>>,
        limits: (u64, usize),
    ) -> bool {
        self.source == *source
            && self.belongs_to(owner)
            && self.photos.len() == photos.len()
            && self.photos.iter().all(|(id, old)| {
                photos
                    .get(id)
                    .is_some_and(|new| std::sync::Arc::ptr_eq(old, new))
            })
            && match &self.drawing {
                Ok(drawing) => {
                    drawing.resident_bytes() <= limits.0 && drawing.vertex_count() <= limits.1
                }
                Err(_) => limits.0 <= self.limits.0 && limits.1 <= self.limits.1,
            }
    }
    /// Native projections reject resources from a previous text/file authority before painting.
    pub fn belongs_to(&self, owner: &Document) -> bool {
        self.document_source == owner.source && self.file_source == owner.file
    }
}

/// Prepared native paths contain no guest callback or mutable domain state.
pub(in crate::ui::plugin) struct SceneDrawing {
    paths: Vec<ScenePrimitive>,
    raster: Option<super::super::images::VectorImage>,
    /// One guarded native texture per source image; repeated placements share its pixels.
    bitmaps: BTreeMap<String, std::sync::Arc<gpui_kit::RenderImage>>,
    vertices: usize,
}
/// One tessellated primitive and its native solid/gradient brush.
enum ScenePrimitive {
    /// A tessellated local path in final canvas coordinates.
    Path {
        path: Path<Pixels>,
        brush: Background,
        clip: Option<protocol::Rect>,
    },
    /// Reuse authorized native pixels; missing pixels keep a local, localized placeholder.
    Image {
        rect: protocol::Rect,
        image: Option<std::sync::Arc<gpui_kit::RenderImage>>,
        status: &'static str,
        clip: Option<protocol::Rect>,
    },
}
impl SceneDrawing {
    /// Build one atomic drawing, charging aggregate vertices before publishing it to the UI.
    fn compile(
        source: &CanvasScene,
        photos: &BTreeMap<String, std::sync::Arc<super::super::images::Photo>>,
        limits: (u64, usize),
    ) -> Result<Self, String> {
        let mut drawing = Self {
            paths: Vec::new(),
            raster: None,
            bitmaps: BTreeMap::new(),
            vertices: 0,
        };
        for node in &source.roots {
            drawing.node(node, SceneTransform::default(), photos, limits, None)?;
        }
        Ok(drawing)
    }
    /// Flatten `node` while preserving group transforms and drawing order.
    fn node(
        &mut self,
        node: &SceneNode,
        transform: SceneTransform,
        photos: &BTreeMap<String, std::sync::Arc<super::super::images::Photo>>,
        limits: (u64, usize),
        clip: Option<protocol::Rect>,
    ) -> Result<(), String> {
        if clip.is_some_and(|clip| clip.w <= 0. || clip.h <= 0.) {
            return Ok(());
        }
        match node {
            SceneNode::Group {
                transform: child,
                clip: local_clip,
                opacity,
                children,
                ..
            } => {
                if *opacity != 1. {
                    return Err("Canvas isolated group requires raster preparation".into());
                }
                let transform = transform.compose(*child);
                let clip = if let Some(geometry) = local_clip {
                    Some(rectangular_clip(geometry, transform, clip)?)
                } else {
                    clip
                };
                for child_node in children {
                    self.node(child_node, transform, photos, limits, clip)?;
                }
            }
            SceneNode::Shape {
                geometry,
                fill,
                stroke,
                ..
            } => {
                // GPUI backgrounds use final axis-aligned path bounds. A local gradient under
                // rotation, skew or nonuniform scale needs the SVG adapter's gradient transform.
                let [a, b, c, d, _, _] = transform.0;
                if (b != 0. || c != 0. || a <= 0. || a != d)
                    && (fill
                        .as_ref()
                        .is_some_and(|brush| matches!(brush, SceneBrush::LinearGradient { .. }))
                        || stroke.as_ref().is_some_and(|stroke| {
                            matches!(stroke.brush, SceneBrush::LinearGradient { .. })
                        }))
                {
                    return Err("Transformed gradient requires raster preparation".into());
                }
                if let Some(fill) = fill {
                    self.append_path(geometry, None, transform, brush(fill)?, limits.1, clip)?;
                }
                if let Some(stroke) = stroke {
                    self.append_path(
                        geometry,
                        Some(stroke),
                        transform,
                        brush(&stroke.brush)?,
                        limits.1,
                        clip,
                    )?;
                }
            }
            SceneNode::Image { image, rect, .. } => {
                let [a, b, c, d, x, y] = transform.0;
                if b != 0. || c != 0. || a <= 0. || d <= 0. {
                    return Err("Transformed image requires raster preparation".into());
                }
                let photo = photos.get(image);
                let bitmap =
                    photo.and_then(|photo| photo.decoded.as_ref().ok().and_then(Option::as_ref));
                let status = super::super::bitmap::status_key(
                    photo.and_then(|photo| photo.decoded.as_ref().err()),
                );
                if let Some(bitmap) = bitmap {
                    if !self.bitmaps.contains_key(image) {
                        let padded =
                            guarded_image(bitmap, limits.0.saturating_sub(self.resident_bytes()))?;
                        self.bitmaps.insert(image.clone(), padded);
                    }
                }
                self.paths.push(ScenePrimitive::Image {
                    rect: protocol::Rect {
                        x: rect.x * a + x,
                        y: rect.y * d + y,
                        w: rect.w * a,
                        h: rect.h * d,
                    },
                    image: self.bitmaps.get(image).cloned(),
                    status,
                    clip,
                });
            }
        }
        Ok(())
    }
    /// Charge each completed small native path once, before retaining it in the published drawing.
    fn append_path(
        &mut self,
        geometry: &SceneGeometry,
        stroke: Option<&SceneStroke>,
        transform: SceneTransform,
        brush: Background,
        limit: usize,
        clip: Option<protocol::Rect>,
    ) -> Result<(), String> {
        let path = build_path(geometry, stroke, transform)?;
        self.vertices += path.vertices.len();
        if self.vertices > limit {
            return Err("Canvas tessellation exceeds its shared vertex budget".into());
        }
        self.paths.push(ScenePrimitive::Path { path, brush, clip });
        Ok(())
    }
    /// Charge only unique derived textures; original authorized decode pixels have their own pool.
    pub fn resident_bytes(&self) -> u64 {
        self.raster
            .iter()
            .map(|image| &image.image)
            .chain(self.bitmaps.values())
            .map(|image| {
                let size = image.size(0);
                size.width.0 as u64 * size.height.0 as u64 * 4
            })
            .sum()
    }
    /// Aggregate tessellation is bounded across every plugin canvas on the worker.
    pub fn vertex_count(&self) -> usize {
        self.vertices
    }
    /// Submit cached paths using the same outer viewport/scroll projection as legacy canvas artwork.
    pub fn paint(
        &self,
        view: &CanvasView,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        _cx: &mut App,
    ) -> Result<(), String> {
        if let Some(raster) = &self.raster {
            window
                .paint_image(
                    bounds,
                    rect_bounds(view.project(raster.rect, bounds), bounds.origin),
                    Default::default(),
                    raster.image.clone(),
                    0,
                    false,
                )
                .map_err(|error| format!("Canvas texture submission failed: {error}"))?;
        }
        for item in &self.paths {
            let clip = match item {
                ScenePrimitive::Path { clip, .. } | ScenePrimitive::Image { clip, .. } => clip,
            };
            window.with_content_mask(
                clip.map(|clip| gpui_kit::ContentMask {
                    bounds: rect_bounds(view.project(clip, bounds), bounds.origin),
                }),
                |window| Self::paint_primitive(item, view, bounds, window, _cx),
            )?;
        }
        Ok(())
    }
    /// Keep native masks independent of path bounds so clipping does not change gradient placement.
    fn paint_primitive(
        item: &ScenePrimitive,
        view: &CanvasView,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), String> {
        let ScenePrimitive::Path { path, brush, .. } = item else {
            if let ScenePrimitive::Image {
                rect,
                image,
                status,
                ..
            } = item
            {
                let area = rect_bounds(view.project(*rect, bounds), bounds.origin);
                if let Some(image) = image {
                    // Crop guard texels from the destination while retaining them for filtering.
                    let pixels = image.size(0);
                    let dx = area.size.width / (pixels.width.0 - 2) as f32;
                    let dy = area.size.height / (pixels.height.0 - 2) as f32;
                    let image_bounds = Bounds::new(
                        area.origin - point(dx, dy),
                        area.size + size(dx * 2., dy * 2.),
                    );
                    window
                        .paint_image(
                            area,
                            image_bounds,
                            Default::default(),
                            image.clone(),
                            0,
                            false,
                        )
                        .map_err(|error| format!("Canvas image submission failed: {error}"))?;
                } else {
                    window.paint_quad(fill(area, gpui_kit::rgba(0x80808022)));
                    window.with_content_mask(
                        Some(gpui_kit::ContentMask { bounds: area }),
                        |window| {
                            paint_text(
                                &rust_i18n::t!(*status),
                                12.,
                                font(
                                    view.font
                                        .family
                                        .clone()
                                        .unwrap_or_else(|| "Segoe UI".into()),
                                ),
                                rgb(view.foreground).into(),
                                area.origin + point(px(4.), px(4.)),
                                window,
                                cx,
                            );
                        },
                    );
                }
            }
            return Ok(());
        };
        let mut path = path.clone();
        for vertex in &mut path.vertices {
            let mapped = view.project(
                protocol::Rect {
                    x: vertex.xy_position.x / px(1.),
                    y: vertex.xy_position.y / px(1.),
                    w: 0.,
                    h: 0.,
                },
                bounds,
            );
            vertex.xy_position = bounds.origin + point(px(mapped.x), px(mapped.y));
        }
        path.bounds = rect_bounds(
            view.project(
                protocol::Rect {
                    x: path.bounds.origin.x / px(1.),
                    y: path.bounds.origin.y / px(1.),
                    w: path.bounds.size.width / px(1.),
                    h: path.bounds.size.height / px(1.),
                },
                bounds,
            ),
            bounds.origin,
        );
        window.paint_path(path, *brush);
        Ok(())
    }
    /// Native projections retain and evict the same fallback texture as the worker scene.
    pub fn images(&self) -> Vec<std::sync::Arc<gpui_kit::RenderImage>> {
        self.raster
            .iter()
            .map(|image| image.image.clone())
            .chain(self.paths.iter().filter_map(|item| match item {
                ScenePrimitive::Image { image, .. } => image.clone(),
                _ => None,
            }))
            .collect()
    }
}

/// Native two-stop backgrounds retain explicit alpha; more complex gradients use raster preparation.
fn brush(value: &SceneBrush) -> Result<Background, String> {
    match value {
        SceneBrush::Solid { rgba: color } => Ok(rgba(*color).into()),
        SceneBrush::LinearGradient { angle, stops }
            if stops.len() == 2 && stops[0].offset < stops[1].offset =>
        {
            Ok(gpui_kit::linear_gradient(
                angle.rem_euclid(360.),
                gpui_kit::linear_color_stop(rgba(stops[0].rgba), stops[0].offset),
                gpui_kit::linear_color_stop(rgba(stops[1].rgba), stops[1].offset),
            ))
        }
        _ => Err("Canvas gradient requires raster preparation".into()),
    }
}

/// Tessellate in local coordinates first, then transform vertices so nonuniform strokes remain correct.
fn build_path(
    geometry: &SceneGeometry,
    stroke: Option<&SceneStroke>,
    transform: SceneTransform,
) -> Result<Path<Pixels>, String> {
    // Keep uninterruptible native tessellation small; larger paths use the bounded raster worker.
    if geometry.segment_count() > 32 {
        return Err("Complex path requires raster preparation".into());
    }
    let mut builder = if let Some(stroke) = stroke {
        if stroke.cap != SceneLineCap::Butt
            || stroke.join != SceneLineJoin::Miter
            || stroke.miter_limit != 4.
        {
            return Err("Canvas stroke requires raster preparation".into());
        }
        let mut builder = PathBuilder::stroke(px(stroke.width));
        if !stroke.dash.is_empty() {
            builder = builder.dash_array(&stroke.dash.iter().copied().map(px).collect::<Vec<_>>());
        }
        builder
    } else {
        let rule = match geometry {
            SceneGeometry::Path {
                rule: SceneFillRule::EvenOdd,
                ..
            } => FillRule::EvenOdd,
            _ => FillRule::NonZero,
        };
        PathBuilder::fill().with_style(PathStyle::Fill(FillOptions::default().with_fill_rule(rule)))
    };
    match geometry {
        SceneGeometry::Rect { rect, radii } => {
            let [a, b, c, d] = radii.map(|radius| radius.min(rect.w.min(rect.h) * 0.5));
            let (x, y, r, bottom) = (rect.x, rect.y, rect.x + rect.w, rect.y + rect.h);
            builder.move_to(point(px(x + a), px(y)));
            builder.line_to(point(px(r - b), px(y)));
            if b > 0. {
                builder.arc_to(
                    point(px(b), px(b)),
                    px(0.),
                    false,
                    true,
                    point(px(r), px(y + b)),
                );
            }
            builder.line_to(point(px(r), px(bottom - c)));
            if c > 0. {
                builder.arc_to(
                    point(px(c), px(c)),
                    px(0.),
                    false,
                    true,
                    point(px(r - c), px(bottom)),
                );
            }
            builder.line_to(point(px(x + d), px(bottom)));
            if d > 0. {
                builder.arc_to(
                    point(px(d), px(d)),
                    px(0.),
                    false,
                    true,
                    point(px(x), px(bottom - d)),
                );
            }
            builder.line_to(point(px(x), px(y + a)));
            if a > 0. {
                builder.arc_to(
                    point(px(a), px(a)),
                    px(0.),
                    false,
                    true,
                    point(px(x + a), px(y)),
                );
            }
            builder.close();
        }
        SceneGeometry::Path { segments, .. } => {
            for segment in segments {
                match *segment {
                    ScenePathSegment::Move { x, y } => builder.move_to(point(px(x), px(y))),
                    ScenePathSegment::Line { x, y } => builder.line_to(point(px(x), px(y))),
                    ScenePathSegment::Quadratic { cx, cy, x, y } => {
                        builder.curve_to(point(px(x), px(y)), point(px(cx), px(cy)))
                    }
                    ScenePathSegment::Cubic {
                        x1,
                        y1,
                        x2,
                        y2,
                        x,
                        y,
                    } => builder.cubic_bezier_to(
                        point(px(x), px(y)),
                        point(px(x1), px(y1)),
                        point(px(x2), px(y2)),
                    ),
                    ScenePathSegment::Close => builder.close(),
                }
            }
        }
    }
    let mut path = builder
        .build()
        .map_err(|error| format!("Canvas path tessellation failed: {error}"))?;
    for vertex in &mut path.vertices {
        let (x, y) = transform.point(vertex.xy_position.x / px(1.), vertex.xy_position.y / px(1.));
        vertex.xy_position = point(px(x), px(y));
    }
    if let Some(first) = path.vertices.first() {
        let mut min = first.xy_position;
        let mut max = min;
        for vertex in &path.vertices {
            min.x = min.x.min(vertex.xy_position.x);
            min.y = min.y.min(vertex.xy_position.y);
            max.x = max.x.max(vertex.xy_position.x);
            max.y = max.y.max(vertex.xy_position.y);
        }
        path.bounds = Bounds::new(min, size(max.x - min.x, max.y - min.y));
    }
    Ok(path)
}

/// Duplicate boundary texels under the derived-image quota so atlas filtering cannot bleed in.
fn guarded_image(
    bitmap: &super::super::bitmap::Bitmap,
    remaining: u64,
) -> Result<std::sync::Arc<gpui_kit::RenderImage>, String> {
    let width = bitmap.width + 2;
    let height = bitmap.height + 2;
    if u64::from(width) * u64::from(height) * 4 > remaining {
        return Err("Canvas image exceeds its derived pixel budget".into());
    }
    let bytes = bitmap
        .image
        .as_bytes(0)
        .ok_or("Canvas image frame is unavailable")?;
    let mut output = image::RgbaImage::new(width, height);
    for (x, y, pixel) in output.enumerate_pixels_mut() {
        let (sx, sy) = (
            x.saturating_sub(1).min(bitmap.width - 1),
            y.saturating_sub(1).min(bitmap.height - 1),
        );
        let offset = ((sy * bitmap.width + sx) * 4) as usize;
        *pixel = image::Rgba(
            bytes[offset..offset + 4]
                .try_into()
                .map_err(|_| "Invalid canvas image frame")?,
        );
    }
    Ok(std::sync::Arc::new(gpui_kit::RenderImage::new([
        image::Frame::new(output),
    ])))
}

/// Axis-aligned rectangular groups use GPUI's native intersecting mask stack, including reflection.
fn rectangular_clip(
    geometry: &SceneGeometry,
    transform: SceneTransform,
    parent: Option<protocol::Rect>,
) -> Result<protocol::Rect, String> {
    let SceneGeometry::Rect { rect, radii } = geometry else {
        return Err("Path clipping requires raster preparation".into());
    };
    let [_, b, c, _, _, _] = transform.0;
    if b != 0. || c != 0. || radii.iter().any(|radius| *radius != 0.) {
        return Err("Transformed clipping requires raster preparation".into());
    }
    let (x0, y0) = transform.point(rect.x, rect.y);
    let (x1, y1) = transform.point(rect.x + rect.w, rect.y + rect.h);
    let mut clip = protocol::Rect {
        x: x0.min(x1),
        y: y0.min(y1),
        w: (x1 - x0).abs(),
        h: (y1 - y0).abs(),
    };
    if let Some(parent) = parent {
        let right = (clip.x + clip.w).min(parent.x + parent.w);
        let bottom = (clip.y + clip.h).min(parent.y + parent.h);
        clip.x = clip.x.max(parent.x);
        clip.y = clip.y.max(parent.y);
        clip.w = (right - clip.x).max(0.);
        clip.h = (bottom - clip.y).max(0.);
    }
    Ok(clip)
}
