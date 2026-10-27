//! Two-axis retained content is projected before visual zoom; no pixels or domain data are invented.
use super::*;

/// Nonnegative logical content position, independent of visual zoom and OS DPI.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneOffset {
    /// Absolute horizontal content position.
    pub x: f32,
    /// Absolute vertical content position.
    pub y: f32,
}

/// Finite logical content extents; a virtual provider publishes only its retained rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneExtent {
    /// Complete logical width, at most 100 million pixels.
    pub width: f32,
    /// Complete logical height under the same bound.
    pub height: f32,
}

/// Canvas 1.10 native two-axis scrolling over absolute scene content coordinates.
/// The host first subtracts its shown offset, then applies VisualViewport and finally OS DPI.
/// Requested positions may exceed the painted buffer; display waits at the last covered position
/// until the guest supplies replacement content. A delayed acknowledgement cannot rewind motion.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneScroll2D {
    /// Total virtual content; this does not allocate the invisible dataset in the host.
    pub content: SceneExtent,
    /// Guest-acknowledged absolute content position.
    pub offset: SceneOffset,
    /// Complete content-space rectangle actually painted, including overscan.
    pub buffer_rect: crate::Rect,
    /// Nonzero content incarnation, changed on replacement/order/meaning changes including ABA.
    pub epoch: u64,
}

impl SceneScroll2D {
    /// Validate finite ranges and a completely painted buffer containing the acknowledged origin.
    pub fn validate(&self) -> Result<(), String> {
        let b = self.buffer_rect;
        if self.epoch == 0
            || [self.content.width, self.content.height]
                .into_iter()
                .any(|v| !v.is_finite() || v <= 0. || v > 100_000_000.)
            || !self.valid_offset(self.offset)
            || [b.x, b.y, b.w, b.h, b.x + b.w, b.y + b.h]
                .into_iter()
                .any(|v| !v.is_finite())
            || b.x < 0.
            || b.y < 0.
            || b.w <= 0.
            || b.h <= 0.
            || b.x + b.w > self.content.width
            || b.y + b.h > self.content.height
            || self.offset.x < b.x
            || self.offset.y < b.y
            || self.offset.x > b.x + b.w
            || self.offset.y > b.y + b.h
        {
            return Err("Invalid retained two-axis scene scroll".into());
        }
        Ok(())
    }

    /// Native scroll notifications are absolute finite positions in this same content extent.
    pub fn valid_offset(&self, offset: SceneOffset) -> bool {
        offset.x.is_finite()
            && offset.y.is_finite()
            && offset.x >= 0.
            && offset.y >= 0.
            && offset.x <= self.content.width
            && offset.y <= self.content.height
    }

    /// Project absolute content geometry using the actual displayed offset, then optional zoom.
    /// `area` and the result use native logical pixels; callers apply the real Canvas clip and DPI.
    pub fn project(
        &self,
        mut rect: crate::Rect,
        shown: SceneOffset,
        viewport: Option<&super::super::VisualViewport>,
        area: super::super::ContentSize,
    ) -> crate::Rect {
        rect.x -= shown.x;
        rect.y -= shown.y;
        if let Some(viewport) = viewport
            && let Some(content) = viewport.content
        {
            viewport
                .transform
                .unwrap_or_default()
                .project(rect, content, area)
        } else {
            rect
        }
    }
}
