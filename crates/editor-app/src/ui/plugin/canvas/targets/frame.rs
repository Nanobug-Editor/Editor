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
    /// Actual retained content coverage; unpainted positions never acquire native hit authority.
    pub content_clip: Option<protocol::Rect>,
}
impl InputFrame {
    /// Tab visits only targets intersecting this shown viewport, including its original scroll/zoom.
    pub(super) fn target_visible(&self, id: &str, epoch: u64) -> bool {
        self.graphics
            .targets
            .bounds_in(id, epoch, self.content_viewport())
            .is_some()
    }
    /// Shared content-space viewport for keyboard, semantics, pointer and native data transfer.
    pub(in crate::ui::plugin::canvas) fn content_viewport(&self) -> protocol::Rect {
        clip_content(content_viewport(self.bounds, self.basis), self.content_clip)
    }
}
/// Intersect native visible content with the provider's explicit painted coverage.
pub(in crate::ui::plugin::canvas) fn clip_content(
    viewport: protocol::Rect,
    clip: Option<protocol::Rect>,
) -> protocol::Rect {
    let Some(clip) = clip else {
        return viewport;
    };
    let x = viewport.x.max(clip.x);
    let y = viewport.y.max(clip.y);
    protocol::Rect {
        x,
        y,
        w: ((viewport.x + viewport.w).min(clip.x + clip.w) - x).max(0.),
        h: ((viewport.y + viewport.h).min(clip.y + clip.h) - y).max(0.),
    }
}
/// Invert the viewport's positive axis-aligned native basis without changing scene affine geometry.
pub(in crate::ui::plugin::canvas) fn content_viewport(
    bounds: Bounds<Pixels>,
    basis: protocol::Rect,
) -> protocol::Rect {
    if basis.w <= 0. || basis.h <= 0. {
        return protocol::Rect {
            x: 0.,
            y: 0.,
            w: 0.,
            h: 0.,
        };
    }
    protocol::Rect {
        x: -basis.x / basis.w,
        y: -basis.y / basis.h,
        w: (bounds.size.width / px(1.)) / basis.w,
        h: (bounds.size.height / px(1.)) / basis.h,
    }
}
impl CanvasView {
    /// Accepted-but-unpainted or disabled scenes cannot publish native drop geometry.
    pub(in crate::ui::plugin) fn transfer_frame(
        &self,
    ) -> Option<super::super::transfer::TransferFrame> {
        let frame = self.targets.frame.as_ref()?;
        if !self.enabled || self.drawing.scene.as_ref() != Some(&frame.graphics.source) {
            return None;
        }
        Some(super::super::transfer::TransferFrame::from_frame(frame))
    }
    /// Focus only the declared live target in a successful painted frame after its receipt is emitted.
    pub(in crate::ui::plugin::canvas) fn apply_overlay_focus(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((identity, target)) = self.pending_overlay_focus.take() else {
            return;
        };
        let valid = self.enabled
            && window.is_window_active()
            && matches!(
                self.scene_presented,
                Some(CanvasEvent::ScenePresented { error: None, .. })
            )
            && self.targets.frame.as_ref().is_some_and(|frame| {
                frame.graphics.source.identity == identity
                    && frame.gesture == self.action_gesture
                    && self.drawing.scene.as_ref() == Some(&frame.graphics.source)
                    && frame.target_visible(&target.id, target.epoch)
                    && frame.graphics.source.targets.iter().any(|item| {
                        item.id == target.id && item.epoch == target.epoch && item.focusable
                    })
            });
        if valid && let Some(handle) = self.target_focus(&(target.id, target.epoch)) {
            handle.focus(window, cx);
            cx.notify();
        }
    }

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
            content_clip: self.retained_content_clip(),
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
