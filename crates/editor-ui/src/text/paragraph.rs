//! Native paragraph layout retains original graphemes while exposing immutable visual geometry.
use gpui_kit::{
    App, Bounds, ContentMask, Font, FontWeight, Hsla, Pixels, SharedString, TextRun, Window,
    WrappedLine, point, px, rgba,
};
use plugin_runtime::plugin_protocol::ui::{CanvasTextLine, CanvasTextRange, ParagraphStyle};
use std::hash::{Hash, Hasher};
mod raster;
mod visual;
pub use raster::{
    NativeParagraphPixels, install_native_raster_fonts, rasterize_native_draft,
    rasterize_native_paragraph,
};

/// Immutable resolved typography and layout, shared by native paint, input and bounded raster adapters.
#[derive(Clone, PartialEq)]
pub struct CanvasTextLayout {
    /// Actual native face and logical pixel size, resolved before crossing a worker boundary.
    pub font: Font,
    pub font_size: Pixels,
    /// Straight-alpha native color and fixed logical layout box; clipping never changes wrapping.
    pub color: Hsla,
    pub bounds: Bounds<Pixels>,
    /// Wrapping, alignment, line step and original-byte style ranges.
    pub style: ParagraphStyle,
    /// Optional base-layout to window-logical affine map; wrapping always uses the untransformed box.
    pub projection: Option<plugin_runtime::plugin_protocol::ui::SceneTransform>,
}
impl CanvasTextLayout {
    /// Map an immutable local glyph box to its conservative native window rectangle.
    pub fn window_rect(&self, rect: Bounds<Pixels>) -> Bounds<Pixels> {
        let Some(transform) = self.projection else {
            return rect;
        };
        let mut left = f32::INFINITY;
        let mut top = f32::INFINITY;
        let mut right = f32::NEG_INFINITY;
        let mut bottom = f32::NEG_INFINITY;
        for corner in [
            rect.origin,
            rect.top_right(),
            rect.bottom_left(),
            rect.bottom_right(),
        ] {
            let (x, y) = transform.point(corner.x / px(1.), corner.y / px(1.));
            left = left.min(x);
            top = top.min(y);
            right = right.max(x);
            bottom = bottom.max(y);
        }
        Bounds::new(
            point(px(left), px(top)),
            gpui_kit::size(px(right - left), px(bottom - top)),
        )
    }
    /// Invert the shown native projection for platform point-to-original-text queries.
    pub fn local_point(&self, point: gpui_kit::Point<Pixels>) -> Option<gpui_kit::Point<Pixels>> {
        match self.projection {
            Some(transform) => transform.inverse().map(|inverse| {
                let (x, y) = inverse.point(point.x / px(1.), point.y / px(1.));
                gpui_kit::point(px(x), px(y))
            }),
            None => Some(point),
        }
    }
}

/// Immutable native glyphs, original-byte boundaries and clip; no editable document or undo history.
pub struct CanvasParagraph {
    /// Native shaping and visual-order adapter share the exact paint/query geometry.
    visual: visual::VisualParagraph,
    /// Whole original extended clusters, including CRLF and joined emoji.
    boundaries: Vec<usize>,
    /// Actual native face/fallback IDs, size and DPI bind geometry to its font environment.
    font_environment: u64,
    /// The same rectangle and line step control paint, selection and caret clipping.
    bounds: Bounds<Pixels>,
    line_height: Pixels,
}
impl CanvasParagraph {
    /// Move a shaped paragraph in window-logical pixels without reshaping; byte-point mapping,
    /// glyphs, carets, selections and line rectangles all move by the same delta.
    pub fn set_origin(&mut self, origin: gpui_kit::Point<Pixels>) {
        let delta = origin - self.bounds.origin;
        if delta == gpui_kit::point(px(0.), px(0.)) {
            return;
        }
        self.bounds.origin = origin;
        self.visual.translate(delta);
    }
    /// Move and paint retained glyphs; subsequent geometry queries use this same new origin.
    pub fn paint_at(
        &mut self,
        origin: gpui_kit::Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), String> {
        self.set_origin(origin);
        self.paint(window, cx)
    }
    /// Shape original UTF-8 `text` using the native `face`, logical `font_size`, `color` and `style`.
    /// `bounds` defines alignment, wrapping and clipping; `window` supplies the actual native fonts.
    /// Native layout errors propagate before presentation; styles never change original source offsets.
    pub fn new(
        text: SharedString,
        face: Font,
        font_size: Pixels,
        color: Hsla,
        bounds: Bounds<Pixels>,
        style: &ParagraphStyle,
        window: &Window,
    ) -> Result<Self, String> {
        Self::shape(
            text,
            face,
            font_size,
            color,
            bounds,
            style,
            window.text_system(),
            window.scale_factor(),
        )
    }
    /// The worker adapter shares native glyph construction without borrowing a Window or UI state.
    fn shape(
        text: SharedString,
        face: Font,
        font_size: Pixels,
        color: Hsla,
        bounds: Bounds<Pixels>,
        style: &ParagraphStyle,
        system: &gpui_kit::WindowTextSystem,
        scale_factor: f32,
    ) -> Result<Self, String> {
        // DPI is known only at the native boundary. Reject before DirectWrite shapes or allocates
        // glyphs, independently of a tiny/fully clipped paragraph's visible rectangle.
        let physical_font = f32::from(font_size) * scale_factor;
        let physical_line = style.line_height.unwrap_or(f32::from(font_size) * 1.45) * scale_factor;
        if !physical_font.is_finite()
            || !(0.01..=2048.).contains(&physical_font)
            || !physical_line.is_finite()
            || !(0.01..=16384.).contains(&physical_line)
        {
            return Err("Native paragraph exceeds its physical font budget".into());
        }
        let boundaries: Vec<usize> =
            plugin_runtime::plugin_protocol::ui::grapheme_boundaries(&text).collect();
        let runs = native_runs(text.len(), &face, color, style);
        let mut key = std::collections::hash_map::DefaultHasher::new();
        for run in &runs {
            run.font.hash(&mut key);
        }
        f32::from(font_size).to_bits().hash(&mut key);
        scale_factor.to_bits().hash(&mut key);
        let lines: Vec<WrappedLine> = system
            .shape_text(
                text,
                font_size,
                &runs,
                style.wrap.then_some(bounds.size.width.max(px(0.))),
                None,
            )
            .map_err(|error| format!("Native paragraph layout failed: {error}"))?
            .into_iter()
            .collect();
        let ellipsis = system
            .shape_text(
                "…".into(),
                font_size,
                &[TextRun {
                    len: 3,
                    font: face,
                    color,
                    ..Default::default()
                }],
                None,
                None,
            )
            .map_err(|error| format!("Native ellipsis layout failed: {error}"))?;
        for line in lines.iter().chain(ellipsis.iter()) {
            for run in line.runs() {
                run.font_id.hash(&mut key);
            }
        }
        let line_height = style.line_height.map(px).unwrap_or(font_size * 1.45);
        let color_at = |byte: usize| {
            let mut end = 0;
            runs.iter()
                .find_map(|run| {
                    end += run.len;
                    (byte < end).then_some(run.color)
                })
                .unwrap_or(color)
        };
        let visual = visual::VisualParagraph::new(
            &lines,
            bounds,
            line_height,
            &boundaries,
            style,
            ellipsis.first(),
            color_at,
        );
        Ok(Self {
            visual,
            boundaries,
            font_environment: key.finish().max(1),
            bounds,
            line_height,
        })
    }
    /// Opaque binding to the exact native fonts and density used by this layout.
    pub fn font_environment(&self) -> u64 {
        self.font_environment
    }
    /// Nearest original whole-grapheme byte at a window-logical point, using native visual rows.
    pub fn byte_for_point(&self, point: gpui_kit::Point<Pixels>) -> usize {
        self.visual.hit(point).unwrap_or(0)
    }
    /// Native caret for original `byte`; `downstream` selects the following side of a shared edge.
    /// Invalid graphemes, hidden suffixes and fully clipped carets return None.
    pub fn caret_bounds(&self, byte: usize, downstream: bool) -> Option<Bounds<Pixels>> {
        if self.boundaries.binary_search(&byte).is_err() {
            return None;
        }
        self.visual.position(byte, downstream).and_then(|position| {
            self.clip(Bounds::new(
                position,
                gpui_kit::size(px(1.), self.line_height),
            ))
        })
    }
    /// Visible original source ranges and native row rectangles; excess output is an explicit error.
    /// Returned coordinates are window-logical; the Canvas adapter applies its inverse projection.
    pub fn visible_lines(&self) -> Result<Vec<CanvasTextLine>, String> {
        let mut lines = Vec::new();
        for (range, rect, ellipsis) in self.visual.lines() {
            if let Some(rect) = self.clip(rect) {
                if lines.len() >= 1024 {
                    return Err("Native paragraph exceeds its visible-row budget".into());
                }
                lines.push(CanvasTextLine {
                    range: CanvasTextRange {
                        start: range.start,
                        end: range.end,
                    },
                    rect: plugin_runtime::plugin_protocol::Rect {
                        x: rect.origin.x / px(1.),
                        y: rect.origin.y / px(1.),
                        w: rect.size.width / px(1.),
                        h: rect.size.height / px(1.),
                    },
                    ellipsis,
                });
            }
        }
        Ok(lines)
    }
    /// Native selection fragments for whole original graphemes, bounded to 256 visible rectangles.
    pub fn selection_rects(
        &self,
        range: std::ops::Range<usize>,
    ) -> Result<Vec<Bounds<Pixels>>, String> {
        if range.start > range.end
            || self.boundaries.binary_search(&range.start).is_err()
            || self.boundaries.binary_search(&range.end).is_err()
        {
            return Err("Native selection requires original grapheme boundaries".into());
        }
        if range.is_empty() {
            return Ok(Vec::new());
        }
        self.visual.selection(range)
    }
    /// Intersect derived geometry with the exact native paragraph clip.
    fn clip(&self, rect: Bounds<Pixels>) -> Option<Bounds<Pixels>> {
        let origin = point(
            rect.origin.x.max(self.bounds.origin.x),
            rect.origin.y.max(self.bounds.origin.y),
        );
        let right =
            (rect.origin.x + rect.size.width).min(self.bounds.origin.x + self.bounds.size.width);
        let bottom =
            (rect.origin.y + rect.size.height).min(self.bounds.origin.y + self.bounds.size.height);
        (right > origin.x && bottom > origin.y)
            .then(|| Bounds::new(origin, gpui_kit::size(right - origin.x, bottom - origin.y)))
    }
    /// Paint the prepared native glyphs in the owning window, returning raster/submission errors.
    pub fn paint(&self, window: &mut Window, _cx: &mut App) -> Result<(), String> {
        window.with_content_mask(
            Some(ContentMask {
                bounds: self.bounds,
            }),
            |window| self.visual.paint(window),
        )
    }
}
/// Materialize ordered native font/color runs; gaps and an unstyled string inherit the paragraph.
fn native_runs(len: usize, face: &Font, color: Hsla, style: &ParagraphStyle) -> Vec<TextRun> {
    let base = TextRun {
        len,
        font: face.clone(),
        color,
        ..Default::default()
    };
    let mut runs = Vec::new();
    let mut end = 0;
    for run in &style.runs {
        if run.range.start > end {
            runs.push(TextRun {
                len: run.range.start - end,
                ..base.clone()
            });
        }
        let mut font = face.clone();
        if let Some(family) = &run.font.family {
            font.family = family.clone().into();
        }
        if let Some(bold) = run.font.bold {
            font.weight = if bold {
                FontWeight::BOLD
            } else {
                FontWeight::NORMAL
            };
        }
        runs.push(TextRun {
            len: run.range.end - run.range.start,
            font,
            color: run.rgba.map(|value| rgba(value).into()).unwrap_or(color),
            ..Default::default()
        });
        end = run.range.end;
    }
    if len > end || runs.is_empty() {
        runs.push(TextRun {
            len: len - end,
            ..base
        });
    }
    runs
}
