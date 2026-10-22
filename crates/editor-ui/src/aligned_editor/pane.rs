//! Native slice composition, one authority paint and shared viewport actions.
use super::super::editor::synchronize_editor_appearance;
use super::*;
use crate::theme::ActiveTheme as _;
use gpui_base::{
    InputBase, ScrollbarAxis,
    input::{MoveDown, MovePageDown, MovePageUp, MoveUp},
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
    /// Native wrapping is unchanged by scrolling; only completed row geometry is retained here.
    geometry_cache: Rc<std::cell::RefCell<geometry_cache::GeometryCache>>,
    /// Folded source intervals may need a different invisible native pointer origin.
    authority_position: geometry_authority::Position,
    /// A root-owned weak mount retains upstream glyphs between private native remeasurements.
    glyphs: geometry_glyphs::Lease,
    /// Weak registry lease also reaches row paints inside the source owner's atomic factory.
    _row_geometry: super::super::row_backgrounds::RowGeometryProvider,
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
        let geometry_cache = Rc::new(std::cell::RefCell::new(
            geometry_cache::GeometryCache::default(),
        ));
        let band_cache = geometry_cache.clone();
        let band_group = group.downgrade();
        let row_geometry = super::super::row_backgrounds::RowGeometryProvider::new(
            &editor,
            move |editor, window, cx| {
                let group = band_group.upgrade()?;
                let state = group.read(cx);
                if state.retired || state.effective_status() != AlignmentStatus::Ready {
                    return None;
                }
                band_cache.borrow().bands(editor, state.epoch, window)
            },
            cx,
        );
        let subscriptions = vec![
            cx.observe(&group, |_, _, cx| cx.notify()),
            cx.observe(&editor, move |this, editor, cx| {
                // IME candidate changes must invalidate even during a geometry notification.
                let changed = observe_content.update(cx, |group, cx| group.refresh_content(cx));
                if changed || this.authority_fence.should_relay(editor.read(cx)) {
                    this.geometry_cache.borrow_mut().invalidate();
                    this.glyphs.invalidate(cx);
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
            geometry_cache,
            authority_position: Default::default(),
            glyphs: geometry_glyphs::Lease::new(cx),
            _row_geometry: row_geometry,
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
        let source_rows = group.rows.clone();
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
        let large = source_rows
            .as_ref()
            .is_some_and(|rows| rows.source_rows[self.index].len() > 64);
        let ready = self.geometry_cache.borrow().ready(epoch);
        // Only a hint here: the final native paint rechecks the map after width/font/wrap changes.
        let mapped = !self.geometry_cache.borrow().full_required()
            && source_rows.as_ref().is_some_and(|rows| {
                let native = source.editor.read(cx);
                // A new source gets one bounded native paint before choosing its measurement
                // strategy. Its unprepared map cannot yet prove wrapping or horizontal extent.
                native.line_height().is_none()
                    || geometry_display_map::supported(native, &rows.source_rows[self.index])
            });
        let batch_measure = large
            && !ready
            && !mapped
            && !self.geometry_cache.borrow().full_required()
            && !source.editor.read(cx).presentation().is_masked();
        let measure_hint = group.measurements[self.index]
            .as_ref()
            .map(|measured| measured.logical_rows.clone());
        let bounded = (ready || batch_measure || mapped)
            && !group.needs_location_geometry(self.index)
            // Complete layout is cheap for small sources and retains their native query envelope.
            && large;
        let inner_gutter = group.inner_gutters && self.index == 0;
        let source_right_inset = group.source_right_inset(self.index);
        let shows_vertical_scrollbar = group.shows_vertical_scrollbar(self.index);
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
        let authority_height = geometry_authority::height(
            pane_layout.as_ref(),
            pane_viewport,
            scroll,
            native_scroll,
            line,
        );
        let authority_top = self.authority_position.read(epoch, scroll, native_scroll);
        let authority_position = self.authority_position.clone();
        // Horizontal scrolling remains native. Each mount applies its own public scroll request
        // immediately before Base chooses visible lines; the last mount owns actual input state.
        let requested_x = self.scroll.take_horizontal();
        let root_measurement = batch_measure
            .then(|| self.glyphs.measure(requested_x, cx))
            .flatten();
        let local_measurement = batch_measure && root_measurement.is_none();
        let batch_output = root_measurement
            .or_else(|| batch_measure.then(|| Rc::new(std::cell::RefCell::new(None))));
        let measure_group = self.group.clone();
        let wheel_group = self.group.clone();
        let wheel_editor = editor.clone();
        let index = self.index;
        let region = self.region.clone();
        let finish_editor = editor.clone();
        let painted_layout = pane_layout.clone();
        let scrollbar = self.scroll.clone();
        let authority_fence = self.authority_fence.clone();
        let geometry_cache = self.geometry_cache.clone();
        let glyphs = self.glyphs.clone();
        let glyph_source = source.clone();
        let owner = cx.entity_id();
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
                                    || crate::scroll::routes_wheel(window, cx)
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
            let native = if let Some(factory) = &source.native_authority_factory {
                factory(&editor, window, cx)
            } else {
                super::super::row_backgrounds::native_editor(&editor, cx)
            };
            if inner_gutter && painted_gutter.width > px(0.) {
                body = body
                    .child(gutter::pixels(
                        &editor,
                        painted_gutter,
                        px(0.),
                        painted_gutter.height,
                        true,
                        || {
                            if let Some(factory) = &source.native_authority_factory {
                                factory(&editor, window, cx)
                            } else {
                                super::super::row_backgrounds::native_editor(&editor, cx)
                            }
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
            if !bounded || batch_measure {
                // Consume pending native scrolling before visual copies issue their own y.
                // Base's public scrollbar prepaint applies the deferred offset; subsequent
                // mounts preserve that actual x, including caller requests and diagonal wheels.
                body = body.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .w_full()
                        .h(pane_viewport.map_or(line * 2., |view| view.height + view.inset_y))
                        .opacity(0.)
                        .child(native_viewport::NativeViewport::primer(&editor)),
                );
            }
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
                    let clipped_top = top.max(px(0.));
                    let clipped_height = (top + height).min(view.height) - clipped_top;
                    let source_top = px(slice.source.start) + clipped_top - top;
                    // Native scrolling bounds layout to this visible interval, including deep
                    // source locations. Neither the source prefix nor an offscreen gutter copy
                    // is shaped again; wrapping and glyphs still come from the original editor.
                    let paint_height = clipped_height + view.native_inset_y + line * 2.;
                    visible = visible.child(
                        div()
                            .id(("aligned-source-slice", piece))
                            .absolute()
                            .top(clipped_top)
                            .left_0()
                            .w_full()
                            .h(clipped_height)
                            .overflow_hidden()
                            .child(
                                // Reorder pixels within each measured source interval; zero-byte
                                // alignment gaps remain outside both text and native line numbers.
                                gutter::pixels(
                                    &editor,
                                    painted_gutter,
                                    -view.native_inset_y,
                                    paint_height,
                                    false,
                                    || {
                                        native_viewport::NativeViewport::new(
                                            &editor,
                                            requested_x,
                                            -source_top,
                                            super::super::row_backgrounds::native_editor(
                                                &editor, cx,
                                            ),
                                        )
                                        .into_any_element()
                                    },
                                ),
                            ),
                    );
                }
                body = body.child(visible);
            }
            if local_measurement
                && let (Some(rows), Some(output)) = (source_rows.clone(), batch_output.clone())
            {
                body = body.child(geometry_measurement::Batches::new(
                    source.clone(),
                    rows,
                    self.index,
                    measure_hint,
                    px(full_height),
                    line,
                    pane_viewport.map_or(px(0.), |view| view.native_inset_y),
                    requested_x,
                    output,
                ));
            }
            let native = if let Some(factory) = &source.native_authority_factory {
                factory(&editor, window, cx)
            } else {
                super::super::row_backgrounds::native_editor(&editor, cx)
            };
            body = body.child(
                div()
                    .id("aligned-source-authority")
                    .absolute()
                    .top(if bounded {
                        authority_top
                    } else {
                        -native_scroll + authority_top
                    })
                    .left_0()
                    .w_full()
                    .h(if bounded {
                        authority_height
                    } else {
                        px(full_height)
                    })
                    .opacity(0.)
                    .child(native_viewport::NativeViewport::new(
                        &editor,
                        requested_x,
                        if bounded { -native_scroll } else { px(0.) },
                        native,
                    )),
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
                    if measure_group.update(cx, |group, cx| group.refresh_content(cx))
                        || measure_group.read(cx).epoch != epoch
                    {
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
                            if unavailable {
                                px(0.)
                            } else if bounded {
                                authority_top
                            } else {
                                -native_scroll + authority_top
                            },
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
                    let Some(rows) = source_rows.as_ref().map(|rows| &rows.source_rows[index])
                    else {
                        authority_fence.completed(painted_native, cx);
                        return;
                    };
                    let acquired = batch_output
                        .as_ref()
                        .and_then(|output| output.borrow_mut().take());
                    if batch_measure && acquired.is_none() {
                        // Provisional batch bounds missed changed wraps/folds. Retry the native
                        // complete authority once; no unobserved source row is admitted.
                        geometry_cache.borrow_mut().require_full();
                        authority_fence.completed(painted_native, cx);
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
                    }
                    let measured = geometry_cache.borrow_mut().read(
                        native,
                        rows,
                        viewport,
                        // Changing wrapping can shrink content below the requested deep offset.
                        // Base clamps its own y before paint; cancel that actual translation so
                        // the difference cannot masquerade as thousands of pixels of padding.
                        if bounded {
                            -native.scroll_offset().y - authority_top
                        } else {
                            native_scroll - authority_top
                        },
                        epoch,
                        bounded,
                        acquired,
                        window,
                    );
                    let Some(measured) = measured else {
                        authority_fence.completed(painted_native, cx);
                        if bounded || batch_measure {
                            // Revoke this partial map before retrying complete native geometry.
                            // This frame cannot publish guessed input coordinates during resize.
                            if batch_measure
                                || finish_editor.read(cx).line_height().is_none()
                                || geometry_display_map::supported(finish_editor.read(cx), rows)
                            {
                                geometry_cache.borrow_mut().require_full();
                            } else {
                                // A bounded map probe can reveal nowrap or a changed signature.
                                // Try native batches next; only an incomplete batch escalates to
                                // full layout. This also bounds cold admission of a new source.
                                geometry_cache.borrow_mut().invalidate();
                            }
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
                        }
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
                    let mapped = geometry_display_map::supported(native, rows);
                    // This skips only the extra observer relay of a state already drawn above.
                    // The native Entity remains mounted: any later private native-state change
                    // still directly invalidates its window/ancestors through GPUI as usual.
                    authority_fence.completed(painted_native, cx);
                    let actual_native_bounds = measured.native;
                    if mapped {
                        glyphs.release(cx);
                    } else if large {
                        glyphs.publish(
                            &measure_group,
                            &glyph_source,
                            owner,
                            viewport,
                            px(full_height),
                            window,
                            cx,
                        );
                    }
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
                        group.frames[index] = frame.clone();
                        group.measured(index, epoch, measured, cx)
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
                    let prepared_frame = frame;
                    region.prepare_native_input(move |physical, _, window, cx| {
                        // Captured drags keep the originating pane's native bounds even when the
                        // physical pointer leaves it; preparation must not move input off-pane.
                        let physical = point_clamped(physical, prepared_frame.viewport);
                        let Some(native) = prepared_frame.native_point(physical) else {
                            return;
                        };
                        if native.y >= safe.top() && native.y < safe.bottom() {
                            return;
                        }
                        // A folded viewport can place distant source intervals side by side.
                        // Move only the transparent original input mount so Base receives the
                        // pointer inside its actual hitbox. Repaint after physical capture has
                        // unwound; the router then reads the new map and dispatches normally.
                        authority_position.set(
                            epoch,
                            scroll,
                            native_scroll,
                            authority_top + physical.y - native.y,
                        );
                        cx.notify(owner);
                        window.draw(cx).clear(cx);
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
                        .right(source_right_inset)
                        .bottom(px(scroll::RAIL))
                        .w_auto()
                        .h_auto(),
                )
                .when(shows_vertical_scrollbar, |body| {
                    body.child(
                        div()
                            .id("aligned-vertical-scrollbar")
                            .debug_selector(move || format!("aligned-vertical-scrollbar-{index}"))
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
                })
                .child(
                    div()
                        .id("aligned-horizontal-scrollbar")
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .h(px(scroll::RAIL))
                        .right(source_right_inset)
                        .child(
                            super::super::scrollbar::vertical_viewport_scrollbar(&self.scroll, cx)
                                .axis(ScrollbarAxis::Horizontal),
                        ),
                )
                .children(folding::disclosures(&self.group, self.index, cx))
                .into_any_element()
        };
        let appearance =
            crate::theme::component_styles(cx, plugin_schema::ThemeComponent::Editor).base;
        let focus = editor.focus_handle(cx);
        let readonly = editor.read(cx).presentation().is_readonly();
        let a11y = (window.is_a11y_active() || cfg!(any(test, feature = "test-support")))
            .then(|| editor.read(cx).value().to_string());
        let target = editor.clone();
        let id = SharedString::from(format!("aligned-source-{:?}", editor.entity_id()));
        let input = InputBase::new(id.clone())
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
            // Moving the first gutter inward must not remove the outer reading inset.
            .when(inner_gutter, |frame| frame.pl(px(gutter::READING_INSET)))
            .font_family(cx.theme().mono_font_family.clone())
            .text_size(
                appearance
                    .font_size_px
                    .map(px)
                    .unwrap_or(cx.theme().mono_font_size),
            )
            .bg(appearance.background.unwrap_or(cx.theme().background))
            .text_color(appearance.foreground.unwrap_or(cx.theme().foreground))
            .child(body);
        let columns = self.group.clone();
        // Measure outside the padded native input. Using its already reduced width here would
        // shrink the budget again on every render; the outer column is the stable constraint.
        let pane = div()
            .relative()
            .size_full()
            .min_w_0()
            .min_h_0()
            .child(input)
            .child(
                canvas(
                    |bounds, _, _| bounds,
                    move |bounds, _, window, cx| {
                        let group = columns.read(cx);
                        if group.retired || group.epoch != epoch || !group.inner_gutters {
                            return;
                        }
                        let Some(viewport) = group.viewports[index] else {
                            return;
                        };
                        let Some(measured) = scroll::ColumnMetrics::read(
                            bounds.size.width,
                            viewport,
                            editor.read(cx),
                        ) else {
                            return;
                        };
                        if group.columns[index] != Some(measured) {
                            columns.update(cx, |group, _| group.columns[index] = Some(measured));
                            let changed = columns.entity_id();
                            window.on_next_frame(move |_, cx| cx.notify(changed));
                        }
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .into_any_element();
        if unavailable {
            pane
        } else {
            crate::scroll::SmoothScroll::new(
                ("aligned-wheel", self.group.entity_id()),
                &self.scroll,
                crate::scroll::ScrollAxis::Both,
                pane,
            )
            .projected(line)
            .into_any_element()
        }
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
