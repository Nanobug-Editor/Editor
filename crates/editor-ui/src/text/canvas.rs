//! Single-line canvas typography uses native glyph geometry for clipping and alignment.
use gpui_kit::{
    App, Bounds, ContentMask, Font, Hsla, Pixels, Point, ShapedLine, SharedString, TextAlign,
    TextRun, Window, point, px, size,
};

/// One shaped native line and its actual drawing position, bounded by its caller's rectangle.
pub struct CanvasText {
    /// Host-shaped text, including any final ellipsis; glyph widths never come from character counts.
    line: ShapedLine,
    /// Final native origin shared by drawing and geometry inspection.
    origin: Point<Pixels>,
    /// Intersects the enclosing canvas mask so text cannot overlap a neighboring column.
    clip: Bounds<Pixels>,
    /// Match the existing canvas line height independently of the available clip height.
    line_height: Pixels,
}

impl CanvasText {
    /// Shape `text` using `face`, `font_size` and `color` in `window`'s native text system.
    /// `bounds` supplies the clip/alignment region; `align` selects its left, center or right edge.
    /// The returned layout owns the ellipsized line and the exact origin later used by `paint`.
    pub fn new(
        text: SharedString,
        face: Font,
        font_size: Pixels,
        color: Hsla,
        bounds: Bounds<Pixels>,
        align: TextAlign,
        window: &Window,
    ) -> Self {
        let line = fit(
            text,
            face,
            font_size,
            color,
            bounds.size.width.max(px(0.)),
            window,
        );
        let remaining = (bounds.size.width - line.width).max(px(0.));
        let offset = match align {
            TextAlign::Left => px(0.),
            TextAlign::Center => remaining / 2.,
            TextAlign::Right => remaining,
        };
        Self {
            line,
            origin: bounds.origin + point(offset, px(0.)),
            clip: bounds,
            line_height: px((f32::from(font_size) * 1.45).ceil()),
        }
    }

    /// Return the actual visible string after native ellipsis, preserving UTF-8 boundaries.
    pub fn text(&self) -> &str {
        &self.line.text
    }

    /// Return the same advance rectangle used by painting, in the caller's pixel coordinates.
    pub fn bounds(&self) -> Bounds<Pixels> {
        Bounds::new(self.origin, size(self.line.width, self.line_height))
    }

    /// Paint the already shaped line into `window` under its clip, using `cx` for native glyphs.
    pub fn paint(&self, window: &mut Window, cx: &mut App) {
        window.with_content_mask(Some(ContentMask { bounds: self.clip }), |window| {
            let _ = self.line.paint(
                self.origin,
                self.line_height,
                TextAlign::Left,
                None,
                window,
                cx,
            );
        });
    }
}

/// Return `text`'s full native advance in logical pixels using `face` and `font_size` in `window`.
/// This shares the painter's native shaping, without clipping, ellipsis or character-width estimates.
pub fn measure_width(text: SharedString, face: Font, font_size: Pixels, window: &Window) -> Pixels {
    shape(text, face, font_size, gpui_kit::rgb(0).into(), window).width
}

/// Shape through GPUI's cached native system; text/font/size/color define the complete run.
fn shape(
    text: SharedString,
    face: Font,
    font_size: Pixels,
    color: Hsla,
    window: &Window,
) -> ShapedLine {
    let run = TextRun {
        len: text.len(),
        font: face,
        color,
        ..Default::default()
    };
    window
        .text_system()
        .shape_line(text, font_size, &[run], None)
}

/// Keep a single line within `width` using native glyph indices, not a guessed ASCII advance.
/// A bounded number of reshapes handles contextual glyph changes; the ellipsis itself is the fallback.
fn fit(
    text: SharedString,
    face: Font,
    font_size: Pixels,
    color: Hsla,
    width: Pixels,
    window: &Window,
) -> ShapedLine {
    let full = shape(text.clone(), face.clone(), font_size, color, window);
    if full.width <= width {
        return full;
    }
    let ellipsis = shape("…".into(), face.clone(), font_size, color, window);
    if ellipsis.width > width {
        return shape("".into(), face, font_size, color, window);
    }
    let budget = width - ellipsis.width;
    let mut end = full
        .index_for_x(budget)
        .unwrap_or(text.len())
        .min(text.len());
    for _ in 0..4 {
        // Native indices are UTF-8 boundaries. Keep this defensive clamp for fallback fonts.
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        let candidate = shape(
            format!("{}…", &text[..end]).into(),
            face.clone(),
            font_size,
            color,
            window,
        );
        if candidate.width <= width {
            return candidate;
        }
        if end == 0 {
            break;
        }
        let earlier = text[..end]
            .char_indices()
            .last()
            .map_or(0, |(index, _)| index);
        end = candidate
            .index_for_x(budget)
            .unwrap_or(earlier)
            .min(earlier);
    }
    ellipsis
}

#[cfg(test)]
mod tests;
