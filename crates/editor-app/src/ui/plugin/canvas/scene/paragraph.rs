//! Project validated paragraph values onto the local native text component.
use super::*;
mod query;
pub(in crate::ui::plugin::canvas) use query::query;

/// One bounded private presentation overrides only its exact paragraph; it never changes public source.
#[derive(Clone, PartialEq)]
pub(in crate::ui::plugin::canvas) struct NativeDraft {
    /// Exact paragraph drawing/layout association from the live target binding.
    pub paragraph: String,
    pub layout_key: String,
    /// Sealed native draft and original-byte decorations, without a domain undo history.
    pub presentation: editor_ui::text::input::CanvasTextPresentation,
}

/// One immutable paragraph placement; native shaping still uses the owning window's fonts.
pub(super) struct ParagraphDrawing {
    /// Stable association used only to select the focused native text endpoint.
    id: String,
    layout_key: String,
    /// Committed plugin text, never a second mutable editor document.
    text: String,
    /// Scene-projected rectangle before the optional visual viewport and OS DPI.
    rect: protocol::Rect,
    /// Optional native font overrides and wrapping behavior.
    style: protocol::ui::ParagraphStyle,
    /// Parent scale is applied to typography exactly once before viewport projection.
    scale: f32,
}

/// Resolve OS geometry from the actual shown frame while retaining the native session's private draft.
pub(in crate::ui::plugin::canvas) fn input_context(
    frame: &super::super::targets::frame::InputFrame,
    id: &str,
) -> Result<editor_ui::text::paragraph::CanvasTextLayout, String> {
    fn find(
        node: &SceneNode,
        parent: SceneTransform,
        id: &str,
    ) -> Option<(ParagraphDrawing, SceneTransform)> {
        match node {
            SceneNode::Paragraph {
                id: found,
                layout_key,
                text,
                rect,
                style,
            } if found == id => Some((
                ParagraphDrawing::new(
                    found,
                    layout_key,
                    text,
                    *rect,
                    style,
                    SceneTransform::default(),
                )
                .expect("identity paragraph placement"),
                parent,
            )),
            SceneNode::Group {
                transform,
                children,
                ..
            } => children
                .iter()
                .find_map(|node| find(node, parent.compose(*transform), id)),
            _ => None,
        }
    }
    frame
        .graphics
        .source
        .roots
        .iter()
        .find_map(|node| find(node, SceneTransform::default(), id))
        .ok_or_else(|| "Shown native text paragraph was replaced".to_owned())
        .map(|(drawing, transform)| {
            if let Ok(axis) = ParagraphDrawing::new(
                &drawing.id,
                &drawing.layout_key,
                &drawing.text,
                drawing.rect,
                &drawing.style,
                transform,
            ) {
                return axis.context(&frame.font, frame.foreground, frame.basis, frame.bounds);
            }
            let mut context = drawing.context(
                &frame.font,
                frame.foreground,
                protocol::Rect {
                    x: 0.,
                    y: 0.,
                    w: 1.,
                    h: 1.,
                },
                Bounds::default(),
            );
            let [a, b, c, d, x, y] = transform.0;
            context.projection = Some(SceneTransform([
                a * frame.basis.w,
                b * frame.basis.h,
                c * frame.basis.w,
                d * frame.basis.h,
                frame.bounds.origin.x / px(1.) + frame.basis.x + x * frame.basis.w,
                frame.bounds.origin.y / px(1.) + frame.basis.y + y * frame.basis.h,
            ]));
            context
        })
}
impl ParagraphDrawing {
    /// Admit a native axis-aligned placement from `text`, local `rect`, `style` and ancestor transform.
    /// Complex placements request the bounded glyph-raster adapter instead of ignoring a transform.
    pub(super) fn new(
        id: &str,
        layout_key: &str,
        text: &str,
        rect: protocol::Rect,
        style: &protocol::ui::ParagraphStyle,
        transform: SceneTransform,
    ) -> Result<Self, String> {
        let [a, b, c, d, x, y] = transform.0;
        if a <= 0. || a != d || b != 0. || c != 0. {
            return Err("Transformed paragraphs require native glyph raster preparation".into());
        }
        Ok(Self {
            id: id.into(),
            layout_key: layout_key.into(),
            text: text.into(),
            rect: protocol::Rect {
                x: rect.x * a + x,
                y: rect.y * a + y,
                w: rect.w * a,
                h: rect.h * a,
            },
            style: style.clone(),
            scale: a,
        })
    }
    /// Resolve the current canvas font, project bounds and submit native glyphs to the owning window.
    pub(super) fn paint(
        &self,
        view: &CanvasView,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<(), String> {
        if let Some(input) = view.paragraph_input(&self.id, &self.layout_key, window, cx) {
            let context = self.context(
                &view.font,
                view.foreground,
                view.project(
                    protocol::Rect {
                        x: 0.,
                        y: 0.,
                        w: 1.,
                        h: 1.,
                    },
                    bounds,
                ),
                bounds,
            );
            return input.update(cx, |input, cx| {
                input.set_layout(context, window);
                input.paint(window, cx)
            });
        }
        self.layout(view, bounds, window)?.paint(window, cx)
    }
    /// Share the exact native layout construction between drawing and point queries.
    fn layout(
        &self,
        view: &CanvasView,
        bounds: Bounds<Pixels>,
        window: &Window,
    ) -> Result<editor_ui::text::paragraph::CanvasParagraph, String> {
        let context = self.context(
            &view.font,
            view.foreground,
            view.project(
                protocol::Rect {
                    x: 0.,
                    y: 0.,
                    w: 1.,
                    h: 1.,
                },
                bounds,
            ),
            bounds,
        );
        editor_ui::text::paragraph::CanvasParagraph::new(
            self.text.clone().into(),
            context.font,
            context.font_size,
            context.color,
            context.bounds,
            &context.style,
            window,
        )
    }
    /// Materialize one immutable native typography/projection used identically by paint and OS input.
    fn context(
        &self,
        base: &protocol::FontStyle,
        foreground: u32,
        basis: protocol::Rect,
        bounds: Bounds<Pixels>,
    ) -> editor_ui::text::paragraph::CanvasTextLayout {
        let font_style = self.style.font.clone().over(base.clone());
        let mut face = font(font_style.family.unwrap_or_else(|| "Segoe UI".into()));
        if font_style.bold.unwrap_or(false) {
            face = face.bold();
        }
        let scale = self.scale * basis.w;
        let mut style = self.style.clone();
        style.line_height = style.line_height.map(|height| height * scale);
        editor_ui::text::paragraph::CanvasTextLayout {
            font: face,
            font_size: px(font_style.size_px.unwrap_or(14.) * scale),
            color: rgb(foreground).into(),
            bounds: rect_bounds(
                protocol::Rect {
                    x: basis.x + self.rect.x * basis.w,
                    y: basis.y + self.rect.y * basis.h,
                    w: self.rect.w * basis.w,
                    h: self.rect.h * basis.h,
                },
                bounds.origin,
            ),
            style,
            projection: None,
        }
    }
}
