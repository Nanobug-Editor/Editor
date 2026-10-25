//! Explicit RGBA and stroke values retain the same meaning in native and raster adapters.
use super::*;

/// Solid color or bounded linear gradient, interpolated in sRGB with straight input alpha.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SceneBrush {
    /// Eight-bit RGBA packed as 0xRRGGBBAA; alpha zero is transparent.
    Solid {
        /// Packed sRGB color, including alpha.
        rgba: u32,
    },
    /// Clockwise degrees from the top of the primitive's bounds, matching GPUI's angle convention.
    LinearGradient {
        /// Finite angle, normalized by the renderer.
        angle: f32,
        /// Two through eight monotonically ordered stops.
        stops: Vec<SceneColorStop>,
    },
}
/// A position/color pair; equal offsets allow an explicit hard transition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneColorStop {
    /// Inclusive normalized position in 0..1.
    pub offset: f32,
    /// Straight sRGB 0xRRGGBBAA color.
    pub rgba: u32,
}
/// An independently painted outline; omitted decoration fields use ordinary butt/miter defaults.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneStroke {
    /// Outline color or gradient.
    pub brush: SceneBrush,
    /// Positive width in local logical pixels.
    pub width: f32,
    /// Line endpoint shape.
    #[serde(default)]
    pub cap: SceneLineCap,
    /// Corner join shape.
    #[serde(default)]
    pub join: SceneLineJoin,
    /// Maximum miter extension relative to width, default four.
    #[serde(default = "miter_limit")]
    pub miter_limit: f32,
    /// Nonnegative alternating draw/gap distances; empty means continuous.
    #[serde(default)]
    pub dash: Vec<f32>,
}
/// Endpoint policies shared by native and raster adapters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneLineCap {
    /// Flat at endpoint.
    #[default]
    Butt,
    /// Semicircular extension.
    Round,
    /// Half-width square extension.
    Square,
}
/// Corner policies shared by native and raster adapters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneLineJoin {
    /// Pointed join under the miter limit.
    #[default]
    Miter,
    /// Circular join.
    Round,
    /// Straight bevel.
    Bevel,
}
/// Standard miter limit in width multiples.
fn miter_limit() -> f32 {
    4.
}
impl SceneBrush {
    /// Reject malformed gradients before allocating native drawing resources.
    pub(super) fn validate(&self) -> Result<(), String> {
        if let Self::LinearGradient { angle, stops } = self {
            if !angle.is_finite()
                || !(2..=8).contains(&stops.len())
                || stops
                    .iter()
                    .any(|s| !s.offset.is_finite() || !(0. ..=1.).contains(&s.offset))
                || stops.windows(2).any(|w| w[0].offset > w[1].offset)
            {
                return Err("Invalid canvas gradient".into());
            }
        }
        Ok(())
    }
}
impl SceneStroke {
    /// Reject invalid widths, all-zero patterns and excessive dash work before tessellation.
    pub(super) fn validate(&self) -> Result<(), String> {
        self.brush.validate()?;
        if !self.width.is_finite()
            || !(0. ..=1_000_000.).contains(&self.width)
            || self.width == 0.
            || !self.miter_limit.is_finite()
            || !(1. ..=128.).contains(&self.miter_limit)
            || self.dash.len() > 32
            || self
                .dash
                .iter()
                .any(|x| !x.is_finite() || *x < 0. || *x > 1_000_000.)
            || (!self.dash.is_empty() && self.dash.iter().all(|x| *x == 0.))
        {
            return Err("Invalid canvas stroke".into());
        }
        Ok(())
    }
}
