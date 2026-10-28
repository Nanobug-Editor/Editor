//! Map the public canvas text region onto editor-owned native typography without domain rules.
use super::*;
#[cfg(test)]
mod tests;

/// Native advances survive drawing-only publications, but a queued reply belongs to one scene.
/// A newer scene that still requests these samples has not acknowledged their delivery yet.
pub(super) struct Measurements {
    requests: Vec<protocol::ui::TextMeasure>,
    family: Option<String>,
    widths: Vec<protocol::ui::TextWidth>,
    reply_revision: Option<u64>,
}

impl CanvasView {
    /// Shape sample groups once per request/family, reusing their widths for a new scene's reply.
    /// The actor may reject a queued old reply after publishing; repeating only for new revisions
    /// keeps pending geometry live without shaping or sending again during ordinary scroll frames.
    pub(super) fn measure_text(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.drawing.measure_text.is_empty() {
            self.text_measurements = None;
            return;
        }
        if !self.enabled {
            return;
        }
        if let Some(cached) = &mut self.text_measurements
            && cached.requests == self.drawing.measure_text
            && cached.family == self.font.family
        {
            if cached.reply_revision != Some(self.revision) {
                cached.reply_revision = Some(self.revision);
                let widths = cached.widths.clone();
                self.emit(CanvasEvent::TextMeasured { widths }, cx);
            }
            return;
        }
        let widths: Vec<_> = self
            .drawing
            .measure_text
            .iter()
            .map(|request| {
                let mut face = font(
                    request
                        .font
                        .clone()
                        .or(self.font.family.clone())
                        .unwrap_or_else(|| "Segoe UI".into()),
                );
                if request.bold {
                    face.weight = FontWeight::BOLD;
                }
                let width = request
                    .texts
                    .iter()
                    .map(|text| {
                        editor_ui::text::canvas::measure_width(
                            text.clone().into(),
                            face.clone(),
                            px(request.size),
                            window,
                        ) / px(1.)
                    })
                    .fold(0_f32, f32::max)
                    .clamp(0., 1_000_000.);
                protocol::ui::TextWidth {
                    id: request.id.clone(),
                    width,
                }
            })
            .collect();
        self.text_measurements = Some(Measurements {
            requests: self.drawing.measure_text.clone(),
            family: self.font.family.clone(),
            widths: widths.clone(),
            reply_revision: Some(self.revision),
        });
        self.emit(CanvasEvent::TextMeasured { widths }, cx);
    }
}

/// Resolve `operation`'s font and projection in `view`, then paint inside canvas `bounds`.
/// `window` supplies native shaping; `cx` performs the same glyph painting as other host controls.
pub(super) fn paint_box(
    view: &CanvasView,
    index: usize,
    operation: &protocol::Paint,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let protocol::Paint::TextBox {
        rect,
        text,
        color,
        size,
        bold,
        font: family,
        align,
    } = operation
    else {
        unreachable!("only text-region operations enter the native text adapter")
    };
    let mut face = font(
        family
            .clone()
            .or(view.font.family.clone())
            .unwrap_or_else(|| "Segoe UI".into()),
    );
    if *bold {
        face.weight = FontWeight::BOLD;
    }
    let scale = view
        .viewport
        .as_ref()
        .and_then(|value| value.transform)
        .map_or(1., |value| value.scale);
    let align = match align {
        protocol::ui::Alignment::Start => gpui_kit::TextAlign::Left,
        protocol::ui::Alignment::Center => gpui_kit::TextAlign::Center,
        protocol::ui::Alignment::End => gpui_kit::TextAlign::Right,
    };
    editor_ui::text::canvas::CanvasText::new(
        text.clone().into(),
        face,
        px(*size * scale),
        rgb(*color).into(),
        rect_bounds(
            view.project(view.column_rect(index, *rect), bounds),
            bounds.origin,
        ),
        align,
        window,
    )
    .paint(window, cx);
}
