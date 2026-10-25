//! Native input retains the last actually painted frame independently of a newly accepted drawing.
use super::*;

/// Immutable hit/source/projection evidence for exactly one successful native display.
pub(in crate::ui::plugin::canvas) struct InputFrame {
    /// The native font/theme values actually used, independent of a newly accepted theme update.
    pub font: protocol::FontStyle,
    pub foreground: u32,
    /// Complete bounded geometry remains alive until another frame is actually painted.
    pub graphics: std::sync::Arc<scene::PreparedScene>,
    /// Source/projection evidence copied from that native paint's receipt.
    pub receipt: CanvasEvent,
    /// Original outer UI revision and source-action credential.
    pub revision: u64,
    pub gesture: Option<action_gestures::ActionGesture>,
    /// Actual native Canvas bounds and content-to-window logical basis at that paint.
    pub bounds: Bounds<Pixels>,
    pub basis: protocol::Rect,
}
impl InputFrame {
    /// Tab visits only targets intersecting this shown viewport, including its original scroll/zoom.
    pub(super) fn target_visible(&self, id: &str, epoch: u64) -> bool {
        let Some(rect) = self.graphics.targets.bounds(id, epoch) else {
            return false;
        };
        let x = self.basis.x + rect.x * self.basis.w;
        let y = self.basis.y + rect.y * self.basis.h;
        let width = rect.w * self.basis.w;
        let height = rect.h * self.basis.h;
        width > 0.
            && height > 0.
            && x + width > 0.
            && y + height > 0.
            && x < self.bounds.size.width / px(1.)
            && y < self.bounds.size.height / px(1.)
    }
}
impl CanvasView {
    /// Install only successful native paint evidence. A failed paint cancels existing native input first.
    pub(in crate::ui::plugin::canvas) fn record_target_frame(
        &mut self,
        receipt: &CanvasEvent,
        bounds: Bounds<Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let CanvasEvent::ScenePresented { error, .. } = receipt else {
            return;
        };
        if error.is_some() {
            self.reconcile_target_lifetimes(
                None,
                false,
                protocol::ui::CanvasCancelReason::SceneReplaced,
                window,
                cx,
            );
            return;
        }
        let Some(graphics) = self.scene_graphics.clone() else {
            return;
        };
        let revision = self
            .targets
            .frame
            .as_ref()
            .filter(|frame| frame.receipt == *receipt && frame.gesture == self.action_gesture)
            .map_or(self.revision, |frame| frame.revision);
        self.targets.frame = Some(InputFrame {
            font: self.font.clone(),
            foreground: self.foreground,
            graphics,
            receipt: receipt.clone(),
            revision,
            gesture: self.action_gesture,
            bounds,
            basis: self.project(
                protocol::Rect {
                    x: 0.,
                    y: 0.,
                    w: 1.,
                    h: 1.,
                },
                bounds,
            ),
        });
    }
}
