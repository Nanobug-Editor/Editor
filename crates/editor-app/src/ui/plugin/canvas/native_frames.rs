//! Visible native frame demand does not dirty the Canvas or substitute a business timer.
use super::*;
use std::{sync::Arc, time::Instant};

/// At most one scheduled callback and one runtime-issued request belong to this native Canvas.
#[derive(Default)]
pub(super) struct NativeFrames {
    request: Option<Arc<plugin_runtime::CanvasFrameResource>>,
    scheduled: bool,
    /// Hidden/replaced callbacks lose their scheduling claim even if GPUI never invokes them.
    generation: u64,
    visible: bool,
    last: Option<Instant>,
}
impl CanvasView {
    /// Bounded native acceptance diagnostics; absent from production builds and never a frame source.
    #[cfg(test)]
    fn trace_native_frame(&self, phase: &str, window: Option<&Window>) {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static LINES: AtomicUsize = AtomicUsize::new(0);
        if std::env::var_os("NANOBUG_CANVAS_FRAME_TRACE").is_some()
            && LINES.fetch_add(1, Ordering::Relaxed) < 160
        {
            eprintln!(
                "[canvas-native-frame] {phase}: request={:?} pending={} scheduled={} generation={} native_visible={} enabled={} window_visible={:?} viewport={:?}",
                self.native_frames
                    .request
                    .as_ref()
                    .map(|request| request.request().request),
                self.native_frames
                    .request
                    .as_ref()
                    .is_some_and(|request| request.is_pending()),
                self.native_frames.scheduled,
                self.native_frames.generation,
                self.native_frames.visible,
                self.enabled,
                window.map(|window| window.is_visible()),
                self.target_content_viewport()
            );
        }
    }
    /// Hiding pauses the original lease. It resumes at a real visible native opportunity.
    pub(in crate::ui::plugin) fn pause_frame_requests(&mut self) {
        #[cfg(test)]
        self.trace_native_frame("pause", None);
        self.native_frames.visible = false;
        self.native_frames.scheduled = false;
        self.native_frames.generation = self.native_frames.generation.wrapping_add(1);
        if let Some(request) = &self.native_frames.request {
            request.set_visible(false);
        }
    }
    /// Borrow the current pending lease; repeated observations cannot queue duplicate callbacks.
    pub(in crate::ui::plugin) fn request_native_frame(
        &mut self,
        request: Arc<plugin_runtime::CanvasFrameResource>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        #[cfg(test)]
        self.trace_native_frame("request", Some(window));
        if !request.is_pending() {
            return;
        }
        if self
            .drawing
            .scene
            .as_ref()
            .is_none_or(|scene| scene.identity != request.request().identity)
        {
            return;
        }
        if self
            .native_frames
            .request
            .as_ref()
            .is_none_or(|old| !Arc::ptr_eq(old, &request))
        {
            self.native_frames.scheduled = false;
            self.native_frames.generation = self.native_frames.generation.wrapping_add(1);
            if let Some(old) = self.native_frames.request.replace(request.clone()) {
                old.set_visible(false);
            }
        }
        let viewport = self.target_content_viewport();
        self.native_frames.visible =
            self.enabled && window.is_visible() && viewport.w > 0. && viewport.h > 0.;
        request.set_visible(self.native_frames.visible);
        if !self.native_frames.visible {
            // GPUI may discard callbacks when a native window is hidden. Never wait for an
            // invisible callback to release its claim before a later ShowWindow can resume.
            self.pause_frame_requests();
            return;
        }
        if self.native_frames.scheduled {
            return;
        }
        self.native_frames.scheduled = true;
        #[cfg(test)]
        self.trace_native_frame("schedule", Some(window));
        let generation = self.native_frames.generation;
        let view = cx.weak_entity();
        // Locked GPUI schedules this opportunity without dirtying the view or window.
        // request_animation_frame would notify this Canvas and redraw unchanged artwork.
        window.on_next_frame(move |window, cx| {
            let _ = view.update(cx, |view, _| {
                #[cfg(test)]
                view.trace_native_frame("callback", Some(window));
                // A late pre-hide callback cannot clear or complete a replacement callback's lease.
                if view.native_frames.generation != generation {
                    return;
                }
                view.native_frames.scheduled = false;
                if !view.native_frames.visible || !view.enabled || !window.is_visible() {
                    request.set_visible(false);
                    return;
                }
                if view
                    .native_frames
                    .request
                    .as_ref()
                    .is_none_or(|current| !Arc::ptr_eq(current, &request))
                    || view
                        .drawing
                        .scene
                        .as_ref()
                        .is_none_or(|scene| scene.identity != request.request().identity)
                {
                    return;
                }
                let viewport = view.target_content_viewport();
                if viewport.w <= 0. || viewport.h <= 0. {
                    request.set_visible(false);
                    return;
                }
                let now = Instant::now();
                let interval = view
                    .native_frames
                    .last
                    .map_or(0., |last| now.duration_since(last).as_secs_f32() * 1000.);
                if request.present(viewport, interval).is_ok() {
                    #[cfg(test)]
                    view.trace_native_frame("completed", Some(window));
                    view.native_frames.last = Some(now);
                    view.native_frames.request = None;
                }
            });
        });
    }
}
