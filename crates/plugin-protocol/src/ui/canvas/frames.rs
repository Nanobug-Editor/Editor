//! One-shot visible frame opportunities, independent of business timers and scene publications.
use super::*;
use crate::api::{ErrorCode, Failure};

/// Canvas 1.10 requests one future visible GPUI opportunity, without changing artwork or UI revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasFrameRequest {
    /// Declared panel owned by the requesting instance.
    pub panel: String,
    /// Existing structured Canvas node in this panel.
    pub canvas: String,
    /// Current nonzero scene incarnation; replacement cancels this pending request.
    pub identity: u64,
    /// Nonzero monotonically increasing token within this Canvas incarnation.
    pub request: u64,
}
impl CanvasFrameRequest {
    /// Validate wire identity only. The host separately checks capability, owner and live scene.
    pub fn validate(&self) -> Result<(), Failure> {
        let name = |name: &str| {
            !name.is_empty() && name.len() <= 128 && !name.chars().any(char::is_control)
        };
        if self.identity == 0 || self.request == 0 || !name(&self.panel) || !name(&self.canvas) {
            return Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Invalid Canvas frame request",
            ));
        }
        Ok(())
    }
}

/// An actual native opportunity; it promises neither physical vblank nor a fixed refresh interval.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasFrameOpportunity {
    /// Exact scene incarnation for which the opportunity was requested.
    pub identity: u64,
    /// Original request token, consumed once before guest notification.
    pub request: u64,
    /// Monotonic native sequence, independent of the scene drawing revision.
    pub sequence: u64,
    /// Visible logical content viewport after the same inverse scroll/zoom basis used by input.
    pub viewport: crate::Rect,
    /// Actual elapsed milliseconds since this Canvas's previous visible opportunity; first is zero.
    pub interval_ms: f32,
    /// True for a delivered opportunity; hidden surfaces retain requests instead of notifying false.
    pub visible: bool,
}
impl CanvasFrameOpportunity {
    /// Reject malformed native receipts before they consume a pending request.
    pub fn validate(&self) -> Result<(), Failure> {
        let rect = self.viewport;
        if self.identity == 0
            || self.request == 0
            || self.sequence == 0
            || !self.visible
            || !self.interval_ms.is_finite()
            || self.interval_ms < 0.
            || [rect.x, rect.y, rect.w, rect.h]
                .into_iter()
                .any(|v| !v.is_finite() || v.abs() > 100_000_000.)
            || rect.w <= 0.
            || rect.h <= 0.
        {
            return Err(Failure::new(
                ErrorCode::InvalidRequest,
                "Invalid native Canvas frame opportunity",
            ));
        }
        Ok(())
    }
}
