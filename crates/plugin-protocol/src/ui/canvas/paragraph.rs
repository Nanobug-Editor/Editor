//! Native paragraph typography is independent of plugin text ownership and undo history.
use serde::{Deserialize, Serialize};

/// Typography for a bounded multiline paragraph; omitted font fields inherit the canvas font.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParagraphStyle {
    /// Horizontal alignment within each native wrapped row.
    #[serde(default)]
    pub align: ParagraphAlign,
    /// Vertical alignment when all native rows fit; overflowing rows start at the top.
    #[serde(default)]
    pub vertical_align: ParagraphVerticalAlign,
    /// Clip or replace the visible final row's hidden suffix with a native ellipsis glyph.
    #[serde(default)]
    pub overflow: ParagraphOverflow,
    /// Ordered, nonoverlapping original grapheme ranges, at most 256. Gaps inherit paragraph style.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runs: Vec<ParagraphRun>,
    /// Optional logical line step; omitted uses 1.45 times the effective native font size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_height: Option<f32>,
    /// Optional native family, logical pixel size and weight, with the Canvas font limits.
    #[serde(default)]
    pub font: crate::FontStyle,
    /// Use native soft wrapping within the declared width; true by default.
    #[serde(default = "wrap")]
    pub wrap: bool,
}
/// Native row alignment; logical source offsets are independent of visual direction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParagraphAlign {
    /// Start at the left edge.
    #[default]
    Left,
    /// Center each shaped row.
    Center,
    /// End at the right edge.
    Right,
}
/// Placement of the complete native line block within its bounded rectangle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParagraphVerticalAlign {
    /// Start at the top edge.
    #[default]
    Top,
    /// Center a fully fitting block.
    Center,
    /// End a fully fitting block at the bottom edge.
    Bottom,
}
/// Visual overflow preserves the original string and all reported original-byte offsets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParagraphOverflow {
    /// Paint only native glyphs intersecting the paragraph clip.
    #[default]
    Clip,
    /// Reserve the native ellipsis advance on the final visible overflowing row.
    Ellipsis,
}
/// Native font/color override over one original UTF-8 grapheme range.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParagraphRun {
    /// Half-open original source range; empty ranges are invalid.
    pub range: super::CanvasTextRange,
    /// Family and bold overrides; run sizes inherit the paragraph's single logical font size.
    #[serde(default)]
    pub font: crate::FontStyle,
    /// Optional straight-alpha sRGB RGBA; omission inherits the Canvas foreground.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rgba: Option<u32>,
}
/// Omitted paragraph wrapping uses native word boundaries rather than overflowing a label.
fn wrap() -> bool {
    true
}
impl Default for ParagraphStyle {
    fn default() -> Self {
        Self {
            align: Default::default(),
            vertical_align: Default::default(),
            overflow: Default::default(),
            runs: Vec::new(),
            line_height: None,
            font: Default::default(),
            wrap: true,
        }
    }
}
impl ParagraphStyle {
    /// Reject invalid font descriptors before calling the native text system.
    pub(super) fn validate(&self, text: &str) -> Result<(), String> {
        if self
            .line_height
            .is_some_and(|height| !height.is_finite() || !(1. ..=1024.).contains(&height))
            || self.font.family.as_ref().is_some_and(|family| {
                family.is_empty() || family.len() > 256 || family.chars().any(char::is_control)
            })
            || self
                .font
                .size_px
                .is_some_and(|size| !size.is_finite() || !(1. ..=128.).contains(&size))
        {
            return Err("Invalid native paragraph font".into());
        }
        if self.runs.len() > 256 {
            return Err("Native paragraph exceeds its style-run budget".into());
        }
        let mut end = 0;
        for run in &self.runs {
            if run.range.start < end
                || run.range.start >= run.range.end
                || !super::is_grapheme_boundary(text, run.range.start)
                || !super::is_grapheme_boundary(text, run.range.end)
                || run.font.size_px.is_some()
                || run.font.family.as_ref().is_some_and(|family| {
                    family.is_empty() || family.len() > 256 || family.chars().any(char::is_control)
                })
            {
                return Err("Invalid native paragraph style range or font override".into());
            }
            end = run.range.end;
        }
        Ok(())
    }
}
