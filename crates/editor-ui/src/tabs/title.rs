//! Document captions align their visible letter bodies with adjacent icons, preserving a full clip line.

use gpui_kit::{
    App, Div, ElementId, InteractiveElement, Interactivity, IntoElement, ParentElement, Pixels,
    Refineable as _, RenderOnce, SharedString, StatefulInteractiveElement, StyleRefinement, Styled,
    TextStyle, Window, div, px,
};

/// A single ellipsized caption with font-aware optical placement inside the tab's normal layout.
#[derive(IntoElement)]
pub struct FileTabTitle {
    /// Stable caption identity; the owner retains document and tab identities separately.
    id: ElementId,
    /// Immutable display text, including any owner-supplied document-state markers.
    text: SharedString,
    /// Caller text refinements and geometry; ordinary Div interaction/debug hooks remain available.
    base: Div,
}

impl FileTabTitle {
    /// Build a caption identified by `id` with display `text`; styling refines the inherited UI font.
    pub fn new(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            base: div().min_w(px(0.)).truncate(),
        }
    }
}

impl Styled for FileTabTitle {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl InteractiveElement for FileTabTitle {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl StatefulInteractiveElement for FileTabTitle {}

impl RenderOnce for FileTabTitle {
    fn render(mut self, window: &mut Window, _: &mut App) -> impl IntoElement {
        let mut text_style = window.text_style();
        text_style.refine(&self.base.style().text);
        let offset = optical_offset(&self.text, &text_style, window);
        // Relative positioning moves the line and its clip together, preserving descenders and width.
        self.base
            .id(self.id)
            .relative()
            .top(-offset)
            .child(self.text)
    }
}

/// Return the displacement from the em-line center to the caption's visible letter-body center.
/// `text` determines letter case, `style` supplies the effective font, and `window` shapes fallback runs.
fn optical_offset(text: &SharedString, style: &TextStyle, window: &Window) -> Pixels {
    if text.is_empty() {
        return px(0.);
    }
    let font_size = style.font_size.to_pixels(window.rem_size());
    let system = window.text_system();
    let line = system.shape_line(text.clone(), font_size, &[style.to_run(text.len())], None);
    let font = line
        .runs
        .first()
        .map(|run| run.font_id)
        .unwrap_or_else(|| system.resolve_font(&style.font()));
    let cap_height = match system.cap_height(font, font_size) {
        height if height > px(0.) => height,
        _ => font_size,
    };
    let x_height = match system.x_height(font, font_size) {
        height if height > px(0.) => height,
        _ => cap_height,
    };
    // Lowercase filenames predominantly occupy x-height. Capitals, digits and uncased scripts use
    // the cap body; punctuation and dirty/conflict badges must not move an unchanged name's baseline.
    let (lowercase, letters, descender_depth) =
        text.chars().filter(|ch| ch.is_alphanumeric()).fold(
            (0_usize, 0_usize, px(0.)),
            |(lowercase, letters, descender_depth), ch| {
                // Typographic bounds use y-up coordinates. Cased letters can extend below the
                // cap/x-height body. Uncased fallback scripts already use their full cap body;
                // applying Latin-style descent compensation would lift CJK captions too far.
                let depth = if ch.is_lowercase() || ch.is_uppercase() {
                    system
                        .typographic_bounds(font, font_size, ch)
                        .map_or(px(0.), |bounds| (-bounds.origin.y).max(px(0.)))
                } else {
                    px(0.)
                };
                (
                    lowercase + usize::from(ch.is_lowercase()),
                    letters + 1,
                    descender_depth + depth,
                )
            },
        );
    let lowercase_fraction = lowercase as f32 / letters.max(1) as f32;
    let body_height = cap_height + (x_height - cap_height) * lowercase_fraction;
    (line.ascent - line.descent - body_height + descender_depth / letters.max(1) as f32) / 2.
}
