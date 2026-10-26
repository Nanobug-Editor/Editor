//! Native two-axis movement retains only viewport positions and bounded acknowledgement receipts.
use gpui_base::ScrollbarHandle;
use gpui_kit::{Bounds, Pixels, Point, Size, point, px, size};
use plugin_runtime::plugin_protocol::ui::{SceneOffset, SceneScroll2D};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};

/// Native motion is presentation-only; committed scene data stays in the public declaration.
#[derive(Clone, Default)]
pub(super) struct CanvasScroll2D(Rc<RefCell<State>>);

#[derive(Default)]
struct State {
    bounds: Bounds<Pixels>,
    declaration: Option<SceneScroll2D>,
    requested: SceneOffset,
    shown: SceneOffset,
    scale: f32,
    /// Visible content begins at this inverse viewport translation relative to the raw scroll.
    viewport_origin: SceneOffset,
    pending: bool,
    issued: VecDeque<SceneOffset>,
}

impl CanvasScroll2D {
    /// A replacement incarnation cannot inherit kinetic motion or acknowledgements.
    pub(super) fn reset(&mut self) {
        // Existing kinetic callbacks keep only the old Rc and cannot retarget a replacement owner.
        *self = Self::default();
    }

    /// Reconcile ordered receipts while preserving newer native requests on either axis.
    /// An explicit jump during pending native motion uses a fresh content epoch.
    pub(super) fn update(
        &self,
        bounds: Bounds<Pixels>,
        declaration: Option<SceneScroll2D>,
        projection: plugin_runtime::plugin_protocol::Rect,
    ) {
        let mut state = self.0.borrow_mut();
        state.bounds = bounds;
        state.scale = projection.w.max(0.000_001);
        state.viewport_origin = SceneOffset {
            x: -projection.x / state.scale,
            y: -projection.y / state.scale,
        };
        let same = match (&state.declaration, &declaration) {
            (Some(old), Some(next)) => old.epoch == next.epoch && old.content == next.content,
            _ => false,
        };
        if !same {
            state.requested = declaration
                .as_ref()
                .map_or(SceneOffset::default(), |value| value.offset);
            state.shown = state.requested;
            state.issued.clear();
            state.pending = false;
        } else if let Some(next) = &declaration {
            let changed = state
                .declaration
                .as_ref()
                .is_some_and(|old| old.offset != next.offset);
            if changed {
                if let Some(index) = state
                    .issued
                    .iter()
                    .position(|offset| near(*offset, next.offset))
                {
                    state.issued.drain(..=index);
                } else if state.issued.is_empty() && !state.pending {
                    state.requested = next.offset;
                    state.shown = next.offset;
                }
            }
        }
        state.declaration = declaration;
        state.clamp_requested();
        state.advance_shown();
    }

    /// The fallback is used before the first native layout, so initial nonzero offsets do not jump.
    pub(super) fn shown_or(&self, fallback: SceneOffset) -> SceneOffset {
        let state = self.0.borrow();
        if state.declaration.is_some() {
            state.shown
        } else {
            fallback
        }
    }
    pub(super) fn shown(&self) -> SceneOffset {
        self.0.borrow().shown
    }

    /// One native-frame request replaces intermediate scroll positions, with a bounded receipt set.
    pub(super) fn take_offset(&self) -> Option<SceneOffset> {
        let mut state = self.0.borrow_mut();
        if !std::mem::take(&mut state.pending) {
            return None;
        }
        let offset = state.requested;
        state.issued.push_back(offset);
        if state.issued.len() > 256 {
            state.issued.pop_front();
        }
        Some(offset)
    }
}

impl State {
    /// Native offsets are expressed in viewport pixels; domain positions stay unscaled.
    fn clamp_requested(&mut self) {
        let Some(scroll) = &self.declaration else {
            return;
        };
        let width = self.bounds.size.width / px(1.) / self.scale;
        let height = self.bounds.size.height / px(1.) / self.scale;
        self.requested.x = self
            .requested
            .x
            .clamp(0., (scroll.content.width - width).max(0.));
        self.requested.y = self
            .requested
            .y
            .clamp(0., (scroll.content.height - height).max(0.));
    }
    /// Wait at the last fully painted edge. Late buffers can reveal less, never rewind position.
    fn advance_shown(&mut self) {
        let Some(scroll) = &self.declaration else {
            return;
        };
        let b = scroll.buffer_rect;
        let minimum_x = b.x - self.viewport_origin.x;
        let minimum_y = b.y - self.viewport_origin.y;
        let maximum_x =
            (b.x + b.w - self.bounds.size.width / px(1.) / self.scale - self.viewport_origin.x)
                .max(minimum_x);
        let maximum_y =
            (b.y + b.h - self.bounds.size.height / px(1.) / self.scale - self.viewport_origin.y)
                .max(minimum_y);
        let candidate = SceneOffset {
            x: self.requested.x.clamp(minimum_x, maximum_x),
            y: self.requested.y.clamp(minimum_y, maximum_y),
        };
        let advance = |shown: f32, requested: f32, candidate: f32| {
            if requested > shown {
                candidate.max(shown).min(requested)
            } else if requested < shown {
                candidate.min(shown).max(requested)
            } else {
                shown
            }
        };
        self.shown.x = advance(self.shown.x, self.requested.x, candidate.x);
        self.shown.y = advance(self.shown.y, self.requested.y, candidate.y);
    }
}

/// Receipt tolerance is below one native logical pixel and never accumulates into domain offsets.
fn near(a: SceneOffset, b: SceneOffset) -> bool {
    (a.x - b.x).abs() < 0.5 && (a.y - b.y).abs() < 0.5
}

impl ScrollbarHandle for CanvasScroll2D {
    fn viewport_bounds(&self) -> Bounds<Pixels> {
        self.0.borrow().bounds
    }
    fn offset(&self) -> Point<Pixels> {
        let state = self.0.borrow();
        point(
            px(-state.requested.x * state.scale),
            px(-state.requested.y * state.scale),
        )
    }
    fn content_size(&self) -> Size<Pixels> {
        let state = self.0.borrow();
        state
            .declaration
            .as_ref()
            .map_or(state.bounds.size, |scroll| {
                size(
                    px(scroll.content.width * state.scale),
                    px(scroll.content.height * state.scale),
                )
            })
    }
    fn set_offset(&self, offset: Point<Pixels>) {
        let mut state = self.0.borrow_mut();
        if state.declaration.is_none() {
            return;
        }
        let next = SceneOffset {
            x: -offset.x / px(1.) / state.scale,
            y: -offset.y / px(1.) / state.scale,
        };
        if next != state.requested {
            state.requested = next;
            state.clamp_requested();
            state.pending = true;
            state.advance_shown();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_base::ScrollbarHandle;
    use gpui_kit::{Bounds, point, px, size};
    use plugin_runtime::plugin_protocol::{
        Rect,
        ui::{SceneExtent, SceneOffset, SceneScroll2D},
    };

    /// A delayed acknowledgement cannot rewind newer motion, including both axes and zoom.
    #[test]
    fn two_axis_native_retains_newer_position_and_waits_for_painted_content() {
        let scroll = CanvasScroll2D::default();
        let bounds = Bounds::new(point(px(0.), px(0.)), size(px(400.), px(300.)));
        let declaration = |x, y| {
            Some(SceneScroll2D {
                content: SceneExtent {
                    width: 4000.,
                    height: 5000.,
                },
                offset: SceneOffset { x, y },
                buffer_rect: Rect {
                    x: 0.,
                    y: 0.,
                    w: 600.,
                    h: 500.,
                },
                epoch: 7,
            })
        };
        scroll.update(
            bounds,
            declaration(0., 0.),
            Rect {
                x: 0.,
                y: 0.,
                w: 2.,
                h: 2.,
            },
        );
        scroll.set_offset(point(px(-240.), px(-400.)));
        assert_eq!(scroll.take_offset(), Some(SceneOffset { x: 120., y: 200. }));
        scroll.set_offset(point(px(-600.), px(-600.)));
        assert_eq!(scroll.take_offset(), Some(SceneOffset { x: 300., y: 300. }));
        scroll.update(
            bounds,
            declaration(120., 200.),
            Rect {
                x: 0.,
                y: 0.,
                w: 2.,
                h: 2.,
            },
        );
        assert_eq!(scroll.offset(), point(px(-600.), px(-600.)));
        assert_eq!(scroll.shown(), SceneOffset { x: 300., y: 300. });
        scroll.set_offset(point(px(-1600.), px(-1800.)));
        assert_eq!(scroll.take_offset(), Some(SceneOffset { x: 800., y: 900. }));
        assert_eq!(scroll.shown(), SceneOffset { x: 400., y: 350. });
        let mut replacement = declaration(800., 900.).unwrap();
        replacement.buffer_rect = Rect {
            x: 600.,
            y: 700.,
            w: 600.,
            h: 500.,
        };
        scroll.update(
            bounds,
            Some(replacement),
            Rect {
                x: 0.,
                y: 0.,
                w: 2.,
                h: 2.,
            },
        );
        assert_eq!(scroll.offset(), point(px(-1600.), px(-1800.)));
        assert_eq!(scroll.shown(), SceneOffset { x: 800., y: 900. });
    }

    /// A zoom anchor/pan changes the content interval visible at the painted buffer edge.
    #[test]
    fn two_axis_native_buffer_edge_uses_the_same_zoom_pan_basis() {
        let scroll = CanvasScroll2D::default();
        let bounds = Bounds::new(point(px(0.), px(0.)), size(px(400.), px(300.)));
        scroll.update(
            bounds,
            Some(SceneScroll2D {
                content: SceneExtent {
                    width: 4000.,
                    height: 5000.,
                },
                offset: SceneOffset::default(),
                buffer_rect: Rect {
                    x: 0.,
                    y: 0.,
                    w: 600.,
                    h: 500.,
                },
                epoch: 7,
            }),
            Rect {
                x: -10.,
                y: -20.,
                w: 2.,
                h: 2.,
            },
        );
        scroll.set_offset(point(px(-1600.), px(-1800.)));
        assert_eq!(scroll.shown(), SceneOffset { x: 395., y: 340. });
    }

    /// A retained old kinetic callback owns only its original native content incarnation.
    #[test]
    fn two_axis_native_retired_motion_cannot_move_replacement_content() {
        let mut scroll = CanvasScroll2D::default();
        let bounds = Bounds::new(point(px(0.), px(0.)), size(px(400.), px(300.)));
        let unit = Rect {
            x: 0.,
            y: 0.,
            w: 1.,
            h: 1.,
        };
        let declaration = |epoch| {
            Some(SceneScroll2D {
                content: SceneExtent {
                    width: 4000.,
                    height: 5000.,
                },
                offset: SceneOffset::default(),
                buffer_rect: Rect {
                    x: 0.,
                    y: 0.,
                    w: 600.,
                    h: 500.,
                },
                epoch,
            })
        };
        scroll.update(bounds, declaration(7), unit);
        let retired = scroll.clone();
        scroll.reset();
        scroll.update(bounds, declaration(8), unit);
        retired.set_offset(point(px(-100.), px(-100.)));
        assert_eq!(scroll.offset(), point(px(0.), px(0.)));
        assert_eq!(scroll.take_offset(), None);
    }

    /// A new scene identity retires old native motion even when a plugin reuses its scroll epoch.
    #[gpui_kit::gpui::test]
    fn two_axis_scene_replacement_retires_motion_with_unchanged_scroll_epoch(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        use gpui_kit::AppContext as _;
        use plugin_runtime::plugin_protocol::ui::{Canvas, CanvasScene};
        cx.update(|cx| {
            gpui_kit::init(cx);
            editor_ui::typography::init(cx);
            editor_ui::theme::apply_theme(editor_ui::theme::builtin_theme(false), cx);
            editor_ui::init(cx);
        });
        let drawing = |identity| Canvas {
            scene: Some(CanvasScene {
                identity,
                revision: 1,
                roots: Vec::new(),
                targets: Vec::new(),
                semantics: Vec::new(),
            }),
            scroll_2d: Some(SceneScroll2D {
                content: SceneExtent {
                    width: 4000.,
                    height: 5000.,
                },
                offset: SceneOffset::default(),
                buffer_rect: Rect {
                    x: 0.,
                    y: 0.,
                    w: 600.,
                    h: 500.,
                },
                epoch: 7,
            }),
            ..Default::default()
        };
        let (view, _) = cx.add_window_view(|window, cx| {
            super::super::CanvasView::new(drawing(41), |_, _, _| {}, window, cx)
        });
        view.update(cx, |view, _| {
            let bounds = Bounds::new(point(px(0.), px(0.)), size(px(400.), px(300.)));
            let unit = Rect {
                x: 0.,
                y: 0.,
                w: 1.,
                h: 1.,
            };
            view.scroll_2d
                .update(bounds, view.drawing.scroll_2d.clone(), unit);
            let old_motion = view.scroll_2d.clone();
            view.update_drawing(drawing(42));
            view.scroll_2d
                .update(bounds, view.drawing.scroll_2d.clone(), unit);
            old_motion.set_offset(point(px(-100.), px(-100.)));
            assert_eq!(view.scroll_2d.offset(), point(px(0.), px(0.)));
            assert_eq!(view.scroll_2d.take_offset(), None);
        });
    }
}
