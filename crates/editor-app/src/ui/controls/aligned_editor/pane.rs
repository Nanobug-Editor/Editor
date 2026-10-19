//! Native slice composition, one authority paint and shared viewport actions.
use super::super::editor::synchronize_editor_appearance;
use super::*;
use crate::ui::theme::ActiveTheme as _;
use gpui_base::{
    InputBase, ScrollbarAxis,
    input::{Editor, MoveDown, MovePageDown, MovePageUp, MoveUp},
};
use gpui_kit::{
    AccessibleAction, Context, DispatchPhase, Focusable as _, HitboxBehavior,
    InteractiveElement as _, ParentElement as _, Point, Render, Role, ScrollWheelEvent,
    SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Window, canvas,
};
use gpui_kit::{div, point, prelude::FluentBuilder as _};

pub(super) struct PaneView {
    group: Entity<GroupState>,
    index: usize,
    region: PointerRegion,
    scroll: scroll::Handle,
    authority_fence: notifications::AuthorityFence,
    /// Exact native metrics for the next visual frame; input always describes the painted frame.
    gutter_metrics: Rc<Cell<gutter::Metrics>>,
    _subscriptions: Vec<Subscription>,
}

impl PaneView {
    pub fn new(
        group: Entity<GroupState>,
        index: usize,
        region: PointerRegion,
        cx: &mut Context<Self>,
    ) -> Self {
        let editor = group.read(cx).sources[index].editor.clone();
        let scroll = scroll::Handle::new(group.read(cx).scroll.clone());
        let observe_content = group.clone();
        let authority_fence = notifications::AuthorityFence::default();
        let subscriptions = vec![
            cx.observe(&group, |_, _, cx| cx.notify()),
            cx.observe(&editor, move |this, editor, cx| {
                // IME candidate changes must invalidate even during a geometry notification.
                let changed = observe_content.update(cx, |group, cx| group.refresh_content(cx));
                if changed || this.authority_fence.should_relay(editor.read(cx)) {
                    cx.notify();
                }
            }),
        ];
        Self {
            group,
            index,
            region,
            scroll,
            authority_fence,
            gutter_metrics: Rc::new(Cell::new(gutter::Metrics::default())),
            _subscriptions: subscriptions,
        }
    }

    fn page(&mut self, down: bool, window: &mut Window, cx: &mut Context<Self>) {
        let group = self.group.read(cx);
        if !matches!(group.effective_status(), AlignmentStatus::Ready) {
            cx.propagate();
            return;
        }
        let Some(frame) = group.frames[self.index].as_ref() else {
            cx.propagate();
            return;
        };
        let editor = group.sources[self.index].editor.clone();
        if !editor.focus_handle(cx).is_focused(window) {
            cx.propagate();
            return;
        }
        let measured = &frame.measured;
        let Some(caret) = measured.caret else {
            cx.propagate();
            return;
        };
        let source_y = measured.source_y(caret);
        let Some(display_y) = frame.pane.source_to_display(source_y) else {
            return;
        };
        let direction = if down { 1. } else { -1. };
        let visible_start = f32::from(
            frame.scroll + frame.viewport.top() - measured.viewport.top() - frame.inset_y,
        );
        let height = f32::from(frame.viewport.size.height);
        let anchor = display_y.clamp(visible_start, visible_start + height);
        let target = (anchor + height * direction).clamp(0., frame.pane.display_height);
        let Some(source_target) = frame.pane.display_to_source(target) else {
            return;
        };
        let in_gap = frame
            .pane
            .source_to_display(source_target)
            .is_some_and(|round_trip| (round_trip - target).abs() > 0.5);
        let row = |y: f32| {
            measured
                .visual_rows
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| (**a - y).abs().partial_cmp(&(**b - y).abs()).unwrap())
                .map_or(0, |(index, _)| index)
        };
        let steps = row(source_y).abs_diff(row(source_target));
        self.group
            .update(cx, |group, cx| group.scroll_by(px(height * direction), cx));
        // A page may consist entirely of zero-byte blank pixels. Scroll that page while leaving
        // the native caret on a real source row; a later page can reach the following real row.
        if in_gap {
            cx.stop_propagation();
            return;
        }
        for _ in 0..steps {
            if down {
                window.dispatch_action(Box::new(MoveDown), cx);
            } else {
                window.dispatch_action(Box::new(MoveUp), cx);
            }
        }
        cx.stop_propagation();
    }
}

impl Render for PaneView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.group.read(cx).retired {
            return div().size_full().into_any_element();
        }
        let relay_scroll = self.group.update(cx, |group, cx| {
            group.refresh_native_overlays(cx);
            group.rebuild(cx);
            // Standalone native scrollbar handles can update a Cell before notifying this pane.
            // Relay that change once to all peers rather than copying scroll into each editor.
            if group.observed_scroll != group.scroll.get() {
                group.observed_scroll = group.scroll.get();
                group.clamp_scroll();
                return true;
            }
            false
        });
        if relay_scroll {
            let changed = self.group.entity_id();
            window.on_next_frame(move |_, cx| cx.notify(changed));
        }
        let group = self.group.read(cx);
        let source = group.sources[self.index].clone();
        let epoch = group.epoch;
        let source_rows = group
            .rows
            .as_ref()
            .map(|rows| rows.source_rows[self.index].clone());
        let pane_layout = group
            .layout
            .as_ref()
            .map(|layout| layout.panes[self.index].clone());
        let pane_viewport = group.display_viewport.and_then(|display| {
            group.measurements[self.index]
                .as_ref()
                .map(|m| display.pane(m))
        });
        let native_scroll = group.native_scroll(self.index);
        let inner_gutter = group.inner_gutters && self.index == 0;
        let painted_gutter = if inner_gutter {
            self.gutter_metrics.get()
        } else {
            gutter::Metrics::default()
        };
        let gutter_metrics = self.gutter_metrics.clone();
        let scroll = group.scroll.get();
        let unavailable = matches!(
            group.effective_status(),
            AlignmentStatus::Unavailable(_)
                | AlignmentStatus::Disabled
                | AlignmentStatus::NativeOverlays
        );
        let editor = source.editor.clone();
        synchronize_editor_appearance(&editor, cx);
        let line = editor
            .read(cx)
            .line_height()
            .unwrap_or(window.line_height());
        let full_height = ((editor.read(cx).text().len() + 2) as f32 * f32::from(line))
            .clamp(f32::from(line), MAX_LAYOUT_HEIGHT);
        // Native horizontal scrolling remains authoritative. Vertical scrolling belongs to the
        // projection while its complete-source authority is mounted, so old source y is revoked.
        let requested_x = self.scroll.take_horizontal();
        if !unavailable && (requested_x.is_some() || editor.read(cx).scroll_offset().y != px(0.)) {
            editor.update(cx, |editor, cx| {
                editor.set_scroll_offset(
                    point(requested_x.unwrap_or(editor.scroll_offset().x), px(0.)),
                    cx,
                );
            });
        }
        let measure_group = self.group.clone();
        let wheel_group = self.group.clone();
        let wheel_editor = editor.clone();
        let index = self.index;
        let region = self.region.clone();
        let finish_editor = editor.clone();
        let painted_layout = pane_layout.clone();
        let scrollbar = self.scroll.clone();
        let authority_fence = self.authority_fence.clone();
        let mut body = div()
            .relative()
            .size_full()
            .min_h_0()
            .overflow_hidden()
            .line_height(gpui_kit::relative(1.5))
            .capture_action(
                cx.listener(|this, _: &MovePageDown, window, cx| this.page(true, window, cx)),
            )
            .capture_action(
                cx.listener(|this, _: &MovePageUp, window, cx| this.page(false, window, cx)),
            )
            .child(
                canvas(
                    |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
                    move |_, hitbox, window, _| {
                        window.on_mouse_event(
                            move |event: &ScrollWheelEvent, phase, window, cx| {
                                if phase != DispatchPhase::Capture
                                    || !hitbox.bounds.contains(&event.position)
                                    || !hitbox.should_handle_scroll(window)
                                {
                                    return;
                                }
                                let delta = event.delta.pixel_delta(line);
                                if unavailable || delta.y == px(0.) {
                                    return;
                                }
                                // A touchpad event can move both axes. Consuming its shared vertical
                                // part must not discard horizontal motion owned by the native source.
                                if delta.x != px(0.) {
                                    wheel_editor.update(cx, |editor, cx| {
                                        editor.set_scroll_offset(
                                            point(editor.scroll_offset().x + delta.x, px(0.)),
                                            cx,
                                        );
                                    });
                                }
                                wheel_group.update(cx, |group, cx| group.scroll_by(-delta.y, cx));
                                cx.stop_propagation();
                            },
                        );
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            );
        if unavailable {
            // The owner shows the localized fallback status; native independent editing survives
            // a refused model without losing source text, composition, gestures or history.
            let native = source.native_authority_factory.as_ref().map_or_else(
                || Editor::new(&editor).into_any_element(),
                |factory| factory(&editor, window, cx),
            );
            if inner_gutter && painted_gutter.width > px(0.) {
                body = body
                    .child(gutter::pixels(
                        &editor,
                        painted_gutter,
                        px(0.),
                        painted_gutter.height,
                        true,
                        || {
                            source.native_authority_factory.as_ref().map_or_else(
                                || Editor::new(&editor).into_any_element(),
                                |factory| factory(&editor, window, cx),
                            )
                        },
                    ))
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full()
                            .opacity(0.)
                            .child(native),
                    );
            } else {
                body = body.child(native);
            }
        } else {
            if let (Some(layout), Some(view)) = (pane_layout, pane_viewport) {
                let mut visible = div()
                    .absolute()
                    .top(view.inset_y)
                    .left_0()
                    .w_full()
                    .h(view.height)
                    .overflow_hidden();
                for (piece, slice) in layout.slices.iter().enumerate() {
                    let top = px(slice.display.start) - scroll;
                    let height = px(slice.source.end - slice.source.start);
                    if top + height <= px(0.) || top >= view.height {
                        continue;
                    }
                    visible = visible.child(
                        div()
                            .id(("aligned-source-slice", piece))
                            .absolute()
                            .top(top)
                            .left_0()
                            .w_full()
                            .h(height)
                            .overflow_hidden()
                            .child(
                                // Reorder pixels within each measured source interval; zero-byte
                                // alignment gaps remain outside both text and native line numbers.
                                gutter::pixels(
                                    &editor,
                                    painted_gutter,
                                    px(-slice.source.start) - view.native_inset_y,
                                    px(full_height),
                                    false,
                                    || Editor::new(&editor).into_any_element(),
                                ),
                            ),
                    );
                }
                body = body.child(visible);
            }
            let native = if let Some(factory) = &source.native_authority_factory {
                factory(&editor, window, cx)
            } else {
                Editor::new(&editor).into_any_element()
            };
            body = body.child(
                div()
                    .id("aligned-source-authority")
                    .absolute()
                    .top(-native_scroll)
                    .left_0()
                    .w_full()
                    .h(px(full_height))
                    .opacity(0.)
                    .child(native),
            );
        }
        body = body.child(
            canvas(
                move |bounds, window, _| {
                    let code = gutter::Projection::new(bounds, painted_gutter, unavailable)
                        .map_or(bounds, |gutter| gutter.code_bounds());
                    window.insert_hitbox(code, HitboxBehavior::Normal)
                },
                move |viewport, hitbox, window, cx| {
                    if measure_group.read(cx).epoch != epoch {
                        return;
                    }
                    measure_group.update(cx, |group, _| group.viewports[index] = Some(viewport));
                    let horizontal = gutter::Projection::new(viewport, painted_gutter, unavailable);
                    if measure_group.read(cx).gutters[index] != horizontal {
                        // A resize/changed font can move the seam beneath a captured mouse-down.
                        // Release that old native gesture before publishing a different partition.
                        region.request_cancel();
                    }
                    measure_group.update(cx, |group, _| group.gutters[index] = horizontal);
                    let native = finish_editor.read(cx);
                    if inner_gutter {
                        let metrics = gutter::Metrics::read(
                            native,
                            viewport,
                            if unavailable { px(0.) } else { -native_scroll },
                            gutter_metrics.get(),
                        );
                        if metrics != gutter_metrics.replace(metrics) {
                            let changed = measure_group.entity_id();
                            window.on_next_frame(move |_, cx| cx.notify(changed));
                        }
                    }
                    if unavailable {
                        // Horizontal copies also notify while vertical alignment is paused.
                        // Fence their completed authority paint to avoid an idle repaint loop.
                        authority_fence.completed(notifications::PaintedNative::read(native), cx);
                        let retry = measure_group.update(cx, |group, cx| {
                            group.retry_disjoint_viewport(index, viewport, cx)
                        });
                        if retry {
                            let changed = measure_group.entity_id();
                            window.on_next_frame(move |_, cx| cx.notify(changed));
                        }
                        // This frame already painted the independent native control. Preserve
                        // its identity input delegate until the next complete geometry frame.
                        region.publish_drag(hitbox, viewport, move |point, anchor| {
                            horizontal.map_or(point, |gutter| gutter.native_point(point, anchor))
                        });
                        input::install(
                            &measure_group,
                            index,
                            finish_editor,
                            None,
                            false,
                            window,
                            cx,
                        );
                        return;
                    }
                    let painted_native = notifications::PaintedNative::read(native);
                    let Some(rows) = source_rows.as_ref() else {
                        authority_fence.completed(painted_native, cx);
                        return;
                    };
                    let Some(measured) = Measurement::read(native, rows, viewport, native_scroll)
                    else {
                        authority_fence.completed(painted_native, cx);
                        measure_group.update(cx, |group, cx| group.fail_geometry(index, epoch, cx));
                        let changed = measure_group.entity_id();
                        window.on_next_frame(move |_, cx| cx.notify(changed));
                        input::install(
                            &measure_group,
                            index,
                            finish_editor,
                            None,
                            true,
                            window,
                            cx,
                        );
                        return;
                    };
                    let metrics_changed = scrollbar.update(
                        &measured,
                        painted_layout
                            .as_ref()
                            .map_or(0., |pane| pane.display_height),
                        pane_viewport.map_or_else(
                            || {
                                Bounds::new(
                                    viewport.origin,
                                    gpui_kit::size(viewport.size.width, px(0.)),
                                )
                            },
                            |view| view.bounds(viewport),
                        ),
                        native.scroll_offset().x,
                    );
                    // This skips only the extra observer relay of a state already drawn above.
                    // The native Entity remains mounted: any later private native-state change
                    // still directly invalidates its window/ancestors through GPUI as usual.
                    authority_fence.completed(painted_native, cx);
                    let actual_native_bounds = measured.native;
                    let frame = painted_layout.zip(pane_viewport).map(|(pane, view)| {
                        geometry::Projection {
                            // A resize/padding change may be discovered only by this authority paint.
                            // Describe the slices actually drawn with the previous subtraction, then
                            // request a finite follow-up using the newly measured common origin.
                            inset_y: view.inset_y + measured.native_inset_y - view.native_inset_y,
                            measured: measured.clone(),
                            pane,
                            scroll,
                            viewport: view.bounds(viewport),
                        }
                    });
                    let redraw = measure_group.update(cx, |group, cx| {
                        if group.epoch != epoch {
                            return false;
                        }
                        let old_layout = group.layout.clone();
                        let old_scroll = group.scroll.get();
                        let old_status = group.status;
                        let old_viewport = group.display_viewport;
                        group.frames[index] = frame.clone();
                        group.measured(index, epoch, measured, cx);
                        old_layout != group.layout
                            || old_scroll != group.scroll.get()
                            || old_status != group.status
                            || old_viewport != group.display_viewport
                    });
                    // GPUI coalesces paint-time model notifications without requesting another
                    // frame. Only changed target geometry schedules one finite follow-up paint.
                    if redraw || metrics_changed {
                        let changed = measure_group.entity_id();
                        window.on_next_frame(move |_, cx| cx.notify(changed));
                    }
                    input::install(
                        &measure_group,
                        index,
                        finish_editor,
                        frame.clone(),
                        frame.is_none(),
                        window,
                        cx,
                    );
                    let Some(frame) = frame else {
                        return;
                    };
                    let mapped = frame.clone();
                    // Capture is constrained to the physical pane even though native source bounds
                    // are full-height and can begin above it after a distant projected scroll.
                    let safe = actual_native_bounds.intersect(&viewport);
                    region.publish_drag(hitbox, safe, move |point, anchor| {
                        let point =
                            horizontal.map_or(point, |gutter| gutter.native_point(point, anchor));
                        let native = mapped.native_point(point).unwrap_or(point);
                        // A gap can straddle the viewport top. Never send an ordinary click
                        // to the preceding, physically clipped source row above another control.
                        point_clamped(native, safe)
                    });
                },
            )
            // Absolute auto-insets retain the preceding block's static position. In native
            // fallback that block is the real Editor, so the input/measurement surface must be
            // explicitly anchored to this pane instead of shifting below it and causing retries.
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        );
        // Rails are siblings of the source input surface. They never enter its pointer region,
        // so Base's own thumb capture is not transformed into an editor selection gesture.
        let body = if unavailable {
            body.into_any_element()
        } else {
            div()
                .relative()
                .size_full()
                .min_h_0()
                .min_w_0()
                .child(
                    body.absolute()
                        .top_0()
                        .left_0()
                        .right(px(scroll::RAIL))
                        .bottom(px(scroll::RAIL))
                        .w_auto()
                        .h_auto(),
                )
                .child(
                    div()
                        .id("aligned-vertical-scrollbar")
                        .absolute()
                        .top(pane_viewport.map_or(px(0.), |view| view.inset_y))
                        .right_0()
                        .w(px(scroll::RAIL))
                        .h(pane_viewport.map_or(px(0.), |view| view.height))
                        .child(super::super::scrollbar::vertical_viewport_scrollbar(
                            &self.scroll,
                            cx,
                        )),
                )
                .child(
                    div()
                        .id("aligned-horizontal-scrollbar")
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .h(px(scroll::RAIL))
                        .right(px(scroll::RAIL))
                        .child(
                            super::super::scrollbar::vertical_viewport_scrollbar(&self.scroll, cx)
                                .axis(ScrollbarAxis::Horizontal),
                        ),
                )
                .into_any_element()
        };
        let appearance =
            crate::theme::component_styles(cx, plugin_schema::ThemeComponent::Editor).base;
        let focus = editor.focus_handle(cx);
        let readonly = editor.read(cx).presentation().is_readonly();
        let a11y =
            (window.is_a11y_active() || cfg!(test)).then(|| editor.read(cx).value().to_string());
        let target = editor.clone();
        let id = SharedString::from(format!("aligned-source-{:?}", editor.entity_id()));
        InputBase::new(id.clone())
            .debug_selector(move || id.to_string())
            .role(if readonly {
                Role::Document
            } else {
                Role::TextInput
            })
            .accessibility_label(source.label)
            .track_focus(&focus)
            .focused(focus.is_focused(window))
            .when_some(a11y, |frame, value| frame.aria_value(value))
            .on_a11y_action(AccessibleAction::Focus, move |_, window, cx| {
                target.update(cx, |state, cx| state.focus(window, cx))
            })
            .flex()
            .size_full()
            .min_h_0()
            .min_w_0()
            .font_family(cx.theme().mono_font_family.clone())
            .text_size(
                appearance
                    .font_size_px
                    .map(px)
                    .unwrap_or(cx.theme().mono_font_size),
            )
            .bg(appearance.background.unwrap_or(cx.theme().background))
            .text_color(appearance.foreground.unwrap_or(cx.theme().foreground))
            .child(body)
            .into_any_element()
    }
}

/// Physical and captured events stay within the same native pane, including oversized gaps.
fn point_clamped(mut point: Point<Pixels>, bounds: Bounds<Pixels>) -> Point<Pixels> {
    let inset = px(0.5);
    point.x = point
        .x
        .clamp(bounds.left(), (bounds.right() - inset).max(bounds.left()));
    point.y = point
        .y
        .clamp(bounds.top(), (bounds.bottom() - inset).max(bounds.top()));
    point
}
