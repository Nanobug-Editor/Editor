//! Adapt portable scene values to cached GPUI paths on the existing background image worker.
use super::*;
use gpui_kit::{Background, FillOptions, FillRule, Path, PathBuilder, PathStyle, rgba};
use protocol::ui::{
    CanvasScene, SceneBrush, SceneFillRule, SceneGeometry, SceneLineCap, SceneLineJoin, SceneNode,
    ScenePathSegment, SceneStroke, SceneTransform,
};
mod bounds;
mod native;
use native::{brush, build_path, guarded_image, rectangular_clip};
pub(super) mod paragraph;
mod retained;
mod svg;
pub(crate) use retained::SceneStatistics;

/// Drawing, generated vectors and interactive clips share the same independent corner limits.
pub(super) fn corner_radii(rect: protocol::Rect, radii: [f32; 4]) -> [f32; 4] {
    radii.map(|radius| radius.min(rect.w.min(rect.h) * 0.5))
}

/// Scene preparation resolves the same inherited native face/color as the live Canvas adapter.
#[derive(Clone, PartialEq)]
pub(in crate::ui::plugin) struct Typography {
    pub font: protocol::FontStyle,
    pub foreground: u32,
}

/// Worker result retains its exact source so late preparation cannot paint a replacement scene.
pub(in crate::ui::plugin) struct PreparedScene {
    /// Flat hit geometry is prepared beside drawing and cannot outlive this source snapshot.
    pub(super) targets: std::sync::Arc<super::targets::geometry::PreparedTargets>,
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
    /// Inherited fonts/colors participate in prepared native glyph pixel identity.
    typography: Typography,
    /// Actual work for this preparation; worker diagnostics accumulate it once on publication.
    statistics: SceneStatistics,
}
impl PreparedScene {
    /// Tessellate `source` outside native UI callbacks and retain its admission/preparation result.
    pub fn new(
        source: CanvasScene,
        owner: &Document,
        photos: BTreeMap<String, std::sync::Arc<super::super::images::Photo>>,
        limits: (u64, usize),
        typography: Typography,
        mut rasterize: impl FnMut(
            &str,
            protocol::Rect,
            &BTreeMap<String, std::sync::Arc<Vec<u8>>>,
        ) -> Option<super::super::images::VectorImage>,
    ) -> Self {
        Self::reuse(
            source,
            owner,
            photos,
            limits,
            typography,
            None,
            |source, bounds, images, _budget| rasterize(source, bounds, images),
        )
    }
    /// Reuse unchanged drawing items from `previous` only within the same source and scene identity.
    /// Geometry, ancestor transforms/clips, typography and referenced native pixels remain key inputs.
    pub fn reuse(
        source: CanvasScene,
        owner: &Document,
        photos: BTreeMap<String, std::sync::Arc<super::super::images::Photo>>,
        limits: (u64, usize),
        typography: Typography,
        previous: Option<&Self>,
        mut rasterize: impl FnMut(
            &str,
            protocol::Rect,
            &BTreeMap<String, std::sync::Arc<Vec<u8>>>,
            u64,
        ) -> Option<super::super::images::VectorImage>,
    ) -> Self {
        let previous =
            previous.filter(|old| old.source.identity == source.identity && old.belongs_to(owner));
        let mut statistics = SceneStatistics::default();
        let mut targets = std::sync::Arc::new(super::targets::geometry::PreparedTargets::default());
        let drawing = source.validate().and_then(|()| {
            if let Some(old) = previous.filter(|old| {
                old.targets.vertices <= limits.1 && retained::same_targets(&old.source, &source)
            }) {
                targets = old.targets.clone();
                statistics.target_reuses += 1;
            } else {
                targets = std::sync::Arc::new(super::targets::geometry::PreparedTargets::new(
                    &source, limits.1,
                )?);
                statistics.target_preparations += 1;
            }
            let mut drawing = SceneDrawing::compile(
                &source,
                &photos,
                (limits.0, limits.1.saturating_sub(targets.vertices)),
                &typography,
                previous.and_then(|old| old.drawing.as_ref().ok()),
                &mut statistics,
                &mut rasterize,
            )
            .or_else(|_| -> Result<SceneDrawing, String> {
                let (svg, bounds, images) =
                    svg::encode(&source, &photos, &typography, limits.0, None)?;
                let raster = rasterize(&svg, bounds, &images, limits.0)
                    .ok_or("Canvas raster preparation exceeded its budget or failed".to_owned())?;
                statistics.rasterizations += 1;
                Ok(SceneDrawing {
                    native_layouts: previous
                        .and_then(|old| old.drawing.as_ref().ok())
                        .map(|old| old.native_layouts.clone())
                        .unwrap_or_default(),
                    paths: Vec::new(),
                    raster: Some(raster),
                    bitmaps: BTreeMap::new(),
                    vertices: 0,
                    retained: BTreeMap::new(),
                })
            })?;
            drawing.vertices += targets.vertices;
            Ok(drawing)
        });
        if drawing.is_err() {
            targets = Default::default();
        }
        Self {
            targets,
            source,
            drawing,
            document_source: owner.source.clone(),
            file_source: owner.file.clone(),
            photos,
            limits,
            typography,
            statistics,
        }
    }
    /// Report work increments for this preparation, current resources and cumulative native paragraph
    /// layouts for this Canvas/source incarnation. Accumulate work once per new PreparedScene Arc;
    /// worker-wide cumulative work and old-snapshot `live_*` belong to SceneImages::statistics.
    pub(crate) fn statistics(&self) -> SceneStatistics {
        let mut result = self.statistics;
        if let Ok(drawing) = &self.drawing {
            result.cached_items = drawing.retained.len();
            result.resident_bytes = drawing.resident_bytes();
            result.vertices = drawing.vertex_count();
            result.paragraph_layouts = drawing
                .native_layouts
                .load(std::sync::atomic::Ordering::Relaxed);
        }
        result
    }
    /// Geometry caches may survive UI revisions, never resource completion or source replacement.
    pub fn current(
        &self,
        source: &CanvasScene,
        owner: &Document,
        photos: &BTreeMap<String, std::sync::Arc<super::super::images::Photo>>,
        limits: (u64, usize),
        typography: &Typography,
    ) -> bool {
        self.source == *source
            && self.typography == *typography
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
    /// Complex layers substitute one sealed native presentation on the bounded image worker.
    /// Public scene identity/targets/resources stay immutable and are checked again before display.
    pub(in crate::ui::plugin::canvas) fn prepare_draft(
        &self,
        draft: &paragraph::NativeDraft,
        budget: u64,
    ) -> Result<super::super::images::VectorImage, String> {
        let (svg, bounds, images) = svg::encode(
            &self.source,
            &self.photos,
            &self.typography,
            budget,
            Some(draft),
        )?;
        super::super::images::VectorRenderer::rasterize_generated(&svg, bounds, &images, budget)
            .ok_or_else(|| "Native text raster preparation failed or exhausted its budget".into())
    }
}

/// Prepared native paths contain no guest callback or mutable domain state.
pub(in crate::ui::plugin) struct SceneDrawing {
    /// Retained native paragraph work belongs to this Canvas/source incarnation, not a test observer.
    native_layouts: std::sync::Arc<std::sync::atomic::AtomicU64>,
    paths: Vec<std::sync::Arc<ScenePrimitive>>,
    raster: Option<super::super::images::VectorImage>,
    /// One guarded native texture per source image; repeated placements share its pixels.
    bitmaps: BTreeMap<String, std::sync::Arc<gpui_kit::RenderImage>>,
    vertices: usize,
    /// Only this publication's live leaf items are retained; old revisions own no global cache entries.
    retained: BTreeMap<String, std::sync::Arc<retained::Item>>,
}
/// One tessellated primitive and its native solid/gradient brush.
enum ScenePrimitive {
    /// An independently retained isolated root; unrelated siblings remain native drawing items.
    Raster {
        image: super::super::images::VectorImage,
        clip: Option<protocol::Rect>,
    },
    /// Native paragraphs share the same ordered clips and projected bounds as artwork.
    Paragraph {
        drawing: paragraph::ParagraphDrawing,
        clip: Option<protocol::Rect>,
    },
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
    /// Only complex layers need a private raster; ordinary paragraphs paint their native draft directly.
    pub(in crate::ui::plugin::canvas) fn uses_raster(&self) -> bool {
        self.raster.is_some()
            || self
                .paths
                .iter()
                .any(|item| matches!(item.as_ref(), ScenePrimitive::Raster { .. }))
    }
    /// Build one atomic drawing, charging aggregate vertices before publishing it to the UI.
    fn compile(
        source: &CanvasScene,
        photos: &BTreeMap<String, std::sync::Arc<super::super::images::Photo>>,
        limits: (u64, usize),
        typography: &Typography,
        previous: Option<&Self>,
        statistics: &mut SceneStatistics,
        rasterize: &mut retained::Rasterizer<'_>,
    ) -> Result<Self, String> {
        let mut drawing = Self {
            native_layouts: previous
                .map(|old| old.native_layouts.clone())
                .unwrap_or_default(),
            paths: Vec::new(),
            raster: None,
            bitmaps: BTreeMap::new(),
            vertices: 0,
            retained: BTreeMap::new(),
        };
        for node in &source.roots {
            retained::layers::prepare_root(
                &mut drawing,
                node,
                photos,
                limits,
                typography,
                previous,
                statistics,
                rasterize,
            )?;
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
                self.paths.push(std::sync::Arc::new(ScenePrimitive::Image {
                    rect: protocol::Rect {
                        x: rect.x * a + x,
                        y: rect.y * d + y,
                        w: rect.w * a,
                        h: rect.h * d,
                    },
                    image: self.bitmaps.get(image).cloned(),
                    status,
                    clip,
                }));
            }
            SceneNode::Paragraph {
                id,
                layout_key,
                text,
                rect,
                style,
            } => {
                self.paths
                    .push(std::sync::Arc::new(ScenePrimitive::Paragraph {
                        drawing: paragraph::ParagraphDrawing::new(
                            id, layout_key, text, *rect, style, transform,
                        )?
                        .observe_layouts(self.native_layouts.clone()),
                        clip,
                    }));
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
        self.paths.push(std::sync::Arc::new(ScenePrimitive::Path {
            path,
            brush,
            clip,
        }));
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
        if self.uses_raster() && view.paint_native_draft(bounds, window)? {
            return Ok(());
        }
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
            let item = item.as_ref();
            let clip = match item {
                ScenePrimitive::Path { clip, .. }
                | ScenePrimitive::Raster { clip, .. }
                | ScenePrimitive::Image { clip, .. }
                | ScenePrimitive::Paragraph { clip, .. } => clip,
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
            if let ScenePrimitive::Raster { image, .. } = item {
                return window
                    .paint_image(
                        bounds,
                        rect_bounds(view.project(image.rect, bounds), bounds.origin),
                        Default::default(),
                        image.image.clone(),
                        0,
                        false,
                    )
                    .map_err(|error| format!("Canvas layer submission failed: {error}"));
            }
            if let ScenePrimitive::Paragraph { drawing, .. } = item {
                return drawing.paint(view, bounds, window, cx);
            }
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
            .chain(self.paths.iter().filter_map(|item| match item.as_ref() {
                ScenePrimitive::Image { image, .. } => image.clone(),
                ScenePrimitive::Raster { image, .. } => Some(image.image.clone()),
                _ => None,
            }))
            .collect()
    }
}
