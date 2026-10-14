//! Bounded native content presentation shared by every guest; no CSS or executable style input.

use serde::{Deserialize, Serialize};

/// Physical pixel edges. Values are finite and nonnegative, checked before native allocation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Edges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Edges {
    /// Whether omitted geometry has the same effect; keeps default scene payloads compact.
    pub fn is_zero(&self) -> bool {
        *self == Self::default()
    }
    /// Symmetric horizontal/vertical insets, in native pixels.
    pub fn symmetric(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    pub(super) fn validate(self, maximum: f32) -> Result<(), String> {
        if [self.top, self.right, self.bottom, self.left]
            .into_iter()
            .all(|v| v.is_finite() && (0. ..=maximum).contains(&v))
        {
            Ok(())
        } else {
            Err("Content edges must be finite and within their pixel budget".into())
        }
    }
}

/// Horizontal content alignment: row distribution, column children and rich-text lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Alignment {
    Start,
    Center,
    End,
}

/// One heading level's native appearance; metrics scale with the resolved body font.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeadingStyle {
    /// Font size multiplier relative to body size, within 0.25..=4.
    pub scale: f32,
    /// Native font weight, within 100..=900.
    pub weight: u16,
    /// Unitless line-height multiplier, within 0.5..=4.
    pub line_height: f32,
    /// Bottom padding measured in this heading's ems.
    pub padding_bottom: f32,
    /// Pixel border thickness, within 0..=16; uses the resolved border color.
    pub border_bottom: f32,
    /// Use the resolved secondary text color.
    pub muted: bool,
}

/// Literal code presentation, also applied to native HTML preformatted blocks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeStyle {
    /// Font size multiplier relative to content size, within 0.25..=4.
    pub scale: f32,
    /// Unitless line-height multiplier, within 0.5..=4.
    pub line_height: f32,
    /// Uniform pixel inset, within 0..=128.
    pub padding: f32,
    /// Pixel corner radius, within 0..=128.
    pub radius: f32,
    /// Uniform pixel border, within 0..=16.
    pub border: f32,
}

/// Role-scoped native defaults, requiring `ui.styles`. User font/color tokens take priority.
/// Omitted fields retain ordinary host appearance. Styles never change control behavior or grant
/// URL/file access. Supplied headings contain exactly six levels; all numeric values are bounded.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ContentStyle {
    /// Content defaults; explicit user role typography overrides each supplied field.
    pub font: crate::FontStyle,
    /// Optional unitless line-height multiplier, within 0.5..=4.
    pub line_height: Option<f32>,
    /// Space between rich-text paragraphs, in body-font ems.
    pub paragraph_gap: Option<f32>,
    /// Empty retains host defaults; otherwise exactly six heading levels.
    pub headings: Vec<HeadingStyle>,
    /// Optional readonly code-block appearance; never starts a language tool.
    pub code: Option<CodeStyle>,
    /// Overrides uniform node padding; colors still use the existing theme role.
    pub padding: Option<Edges>,
    /// Per-edge pixel borders; colors follow the role's theme override.
    pub border: Option<Edges>,
    /// Optional corner radius in pixels, within 0..=128.
    pub radius: Option<f32>,
    /// Cell insets for native rich-text tables.
    pub table_cell_padding: Option<Edges>,
}

impl ContentStyle {
    pub(super) fn validate(&self) -> Result<(), String> {
        let bounded = |v: f32, min: f32, max: f32| v.is_finite() && (min..=max).contains(&v);
        if self
            .font
            .family
            .as_ref()
            .is_some_and(|s| s.is_empty() || s.len() > 128 || s.chars().any(char::is_control))
            || self.font.size_px.is_some_and(|v| !bounded(v, 8., 128.))
            || self.line_height.is_some_and(|v| !bounded(v, 0.5, 4.))
            || self.paragraph_gap.is_some_and(|v| !bounded(v, 0., 8.))
            || self.radius.is_some_and(|v| !bounded(v, 0., 128.))
        {
            return Err("Invalid content typography or radius".into());
        }
        if !self.headings.is_empty() && self.headings.len() != 6 {
            return Err("Content heading styles require exactly six levels".into());
        }
        for h in &self.headings {
            if !bounded(h.scale, 0.25, 4.)
                || !(100..=900).contains(&h.weight)
                || !bounded(h.line_height, 0.5, 4.)
                || !bounded(h.padding_bottom, 0., 8.)
                || !bounded(h.border_bottom, 0., 16.)
            {
                return Err("Invalid content heading metrics".into());
            }
        }
        if let Some(c) = &self.code {
            if !bounded(c.scale, 0.25, 4.)
                || !bounded(c.line_height, 0.5, 4.)
                || !bounded(c.padding, 0., 128.)
                || !bounded(c.radius, 0., 128.)
                || !bounded(c.border, 0., 16.)
            {
                return Err("Invalid content code metrics".into());
            }
        }
        for e in [self.padding, self.table_cell_padding]
            .into_iter()
            .flatten()
        {
            e.validate(128.)?;
        }
        if let Some(e) = self.border {
            e.validate(16.)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{Document, Node};

    /// Styles negotiate explicitly and reject unbounded geometry before native allocation.
    #[test]
    fn content_styles_are_bounded_and_require_the_public_capability() {
        let mut doc = Document::new(Node::text("sample", "content"));
        doc.content_styles.insert(
            "prose".into(),
            ContentStyle {
                font: crate::FontStyle {
                    size_px: Some(16.),
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        assert!(doc.required_capabilities().contains_key("ui.styles"));
        assert!(doc.validate().is_ok());
        doc.content_styles.get_mut("prose").unwrap().line_height = Some(f32::NAN);
        assert!(doc.validate().is_err());
        doc.content_styles.clear();
        doc.root.layout.margin.top = f32::INFINITY;
        assert!(doc.validate().is_err());
        doc.root.layout.margin = Edges::default();
        doc.root.layout.align = Some(Alignment::Center);
        assert!(doc.validate().is_ok());
        assert!(doc.required_capabilities().contains_key("ui.styles"));
    }
}
