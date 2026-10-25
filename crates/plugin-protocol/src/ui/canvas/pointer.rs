//! Portable pointer payloads preserve native availability and continuous content-space coordinates.
use super::*;

/// Native cursor policy; no guest image or platform cursor object crosses the boundary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasCursor {
    /// Ordinary system arrow.
    #[default]
    Arrow,
    /// Native insertion caret.
    Text,
    /// Pointing hand.
    Pointer,
    /// Crosshair.
    Crosshair,
    /// Open hand.
    Grab,
    /// Closed hand during a drag.
    Grabbing,
    /// Horizontal resize.
    ResizeHorizontal,
    /// Vertical resize.
    ResizeVertical,
    /// Unavailable action.
    NotAllowed,
}
/// Actual native pointer category; unsupported pressure/button masks remain None.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasPointerDevice {
    /// The native mouse has stable ID 1 within the Canvas owner.
    Mouse,
    /// A platform touch contact, only when actually supplied by the backend.
    Touch,
    /// A platform pen contact, only when actually supplied by the backend.
    Pen,
}
/// Separate target transitions and capture cancellation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasPointerPhase {
    /// Native press.
    Down,
    /// Native movement.
    Move,
    /// Native release, including release outside the captured target.
    Up,
    /// Enter this actual hit shape.
    Enter,
    /// Leave this actual hit shape.
    Leave,
    /// Native ownership was cancelled without a release.
    Cancel,
}
/// Revocation reasons have no business or document authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasCancelReason {
    /// Native focus moved elsewhere or the window deactivated.
    FocusLost,
    /// The target was withdrawn.
    TargetRemoved,
    /// The stable ID now names a different target generation.
    EpochReplaced,
    /// The owning scene was replaced.
    SceneReplaced,
    /// A modal native surface took over input.
    Modal,
    /// The instance or panel retired.
    Retired,
    /// The native pointer no longer has the button/contact that began capture.
    PointerLost,
}
/// One native pointer observation; all points are Canvas content logical pixels before OS DPI.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasPointerInput {
    /// Stable nonzero pointer identity within this native owner.
    pub pointer: u64,
    /// Actual native device kind.
    pub device: CanvasPointerDevice,
    /// Native transition.
    pub phase: CanvasPointerPhase,
    /// Continuous content-space horizontal position, including outside during capture.
    pub x: f32,
    /// Continuous content-space vertical position.
    pub y: f32,
    /// Changed button: left=0, middle=1, right=2, back=3, forward=4; None on movement.
    pub button: Option<u8>,
    /// Complete native pressed-button mask only when provided; never inferred from one pressed button.
    pub buttons: Option<Vec<u8>>,
    /// Actual click count only on press/release, at most 1024.
    pub clicks: Option<u32>,
    /// Normalized native pressure, or None when unavailable.
    pub pressure: Option<f32>,
    /// All native modifiers for this observation.
    pub modifiers: CanvasModifiers,
    /// Present exactly on Cancel; cancellation never commits text or a business action.
    pub cancel_reason: Option<CanvasCancelReason>,
}
/// Wheel units preserve native precision; line steps are dimensionless and never pixel-rounded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasWheelUnit {
    /// Logical content pixels, inverse-projected by the current viewport scale.
    Pixels,
    /// Native fractional line steps.
    Lines,
}
/// Native wheel input addressed to the actual hit/captured target.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasWheelInput {
    /// Content-space point.
    pub x: f32,
    /// Content-space point.
    pub y: f32,
    /// Original continuous delta, in declared units.
    pub delta_x: f32,
    /// Original continuous delta, in declared units.
    pub delta_y: f32,
    /// Native unit availability.
    pub unit: CanvasWheelUnit,
    /// All native modifiers.
    pub modifiers: CanvasModifiers,
}
impl CanvasPointerInput {
    /// Check bounded native availability before forwarding a pointer to an untrusted guest.
    pub(super) fn valid(&self) -> bool {
        self.pointer != 0
            && finite(self.x)
            && finite(self.y)
            && self.button.is_none_or(|button| button <= 4)
            && self.buttons.as_ref().is_none_or(|buttons| {
                buttons.len() <= 5 && buttons.iter().all(|button| *button <= 4)
            })
            && self
                .clicks
                .is_none_or(|clicks| (1..=1024).contains(&clicks))
            && self
                .pressure
                .is_none_or(|pressure| pressure.is_finite() && (0. ..=1.).contains(&pressure))
            && (self.phase == CanvasPointerPhase::Cancel) == self.cancel_reason.is_some()
            && (!matches!(
                self.phase,
                CanvasPointerPhase::Down | CanvasPointerPhase::Up
            ) || self.button.is_some())
    }
}
impl CanvasWheelInput {
    /// Keep fractional motion and reject nonfinite or excessive deltas.
    pub(super) fn valid(&self) -> bool {
        [self.x, self.y, self.delta_x, self.delta_y]
            .into_iter()
            .all(finite)
    }
}
/// Native input uses the same public coordinate extent as drawing and text geometry.
fn finite(value: f32) -> bool {
    value.is_finite() && value.abs() <= 1_000_000.
}
