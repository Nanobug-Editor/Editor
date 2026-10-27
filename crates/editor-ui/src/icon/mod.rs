//! Local SVG icon rendering using the shared icon catalog only for asset paths.

use std::sync::Arc;

use gpui_base::StyledExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::{
    App, Hsla, IntoElement, RenderOnce, SharedString, StyleRefinement, Styled, Svg, Transformation,
    Window, prelude::FluentBuilder as _, px, svg,
};

#[derive(Clone)]
enum Source {
    Path(SharedString),
    Data(Arc<[u8]>),
}

/// Local monochrome SVG presentation from a catalog path or owned byte snapshot.
#[derive(Clone, IntoElement)]
pub struct Icon {
    source: Source,
    style: StyleRefinement,
    size: f32,
    transformation: Option<Transformation>,
}

impl Default for Icon {
    fn default() -> Self {
        Self {
            source: Source::Path("".into()),
            style: StyleRefinement::default(),
            size: 16.,
            transformation: None,
        }
    }
}

impl From<IconName> for Icon {
    fn from(name: IconName) -> Self {
        Self::default().path(name.path())
    }
}

impl Icon {
    /// Resolve name through the shared static Kit icon catalog.
    pub fn new(name: IconName) -> Self {
        name.into()
    }

    /// Read SVG artwork from the application asset source at path during rendering.
    pub fn path(mut self, path: impl Into<SharedString>) -> Self {
        self.source = Source::Path(path.into());
        self
    }

    /// Retain a copied immutable SVG byte snapshot from data for rendering.
    pub fn data(mut self, data: &[u8]) -> Self {
        self.source = Source::Data(Arc::from(data));
        self
    }

    /// Use a fourteen-pixel icon square.
    pub fn small(mut self) -> Self {
        self.size = 14.;
        self
    }

    /// Use a twelve-pixel icon square.
    pub fn xsmall(mut self) -> Self {
        self.size = 12.;
        self
    }

    /// Scale the native default extent; explicit caller-provided Styled dimensions remain authoritative.
    pub fn scale(mut self, scale: f32) -> Self {
        debug_assert!(
            scale.is_finite() && scale > 0.,
            "icon scale must be finite and positive"
        );
        self.size *= scale;
        self
    }

    /// Apply transformation when building the SVG; source artwork remains immutable.
    pub fn transform(mut self, transformation: Transformation) -> Self {
        self.transformation = Some(transformation);
        self
    }

    /// Build the SVG from the local style and the surrounding text color.
    fn into_svg(self, fallback_color: Hsla) -> Svg {
        svg()
            .size(px(self.size))
            .flex_shrink_0()
            // GPUI skips SVG painting when its own text color is absent.
            .text_color(fallback_color)
            .map(|this| match self.source {
                Source::Path(path) => this.path(path),
                Source::Data(data) => this.data(&data),
            })
            .when_some(self.transformation, |this, transform| {
                this.with_transformation(transform)
            })
            .refine_style(&self.style)
    }
}

impl Styled for Icon {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Icon {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        self.into_svg(window.text_style().color)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{blue, red};

    #[test]
    fn svg_receives_surrounding_text_color_unless_overridden() {
        let mut inherited = Icon::default().path("icons/settings.svg").into_svg(blue());
        assert_eq!(inherited.style().text.color, Some(blue()));

        let mut explicit = Icon::default()
            .data(br#"<svg viewBox="0 0 1 1"/>"#)
            .text_color(red())
            .into_svg(blue());
        assert_eq!(explicit.style().text.color, Some(red()));
    }
}
