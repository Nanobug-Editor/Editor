//! Physical wheel events exercise native offsets, virtualization, interruption and finite frame demand.
use super::*;
use gpui_base::input::EditorState;
use gpui_kit::{
    AppContext as _, Context, Entity, IntoElement, ParentElement as _, Point, Render, ScrollDelta,
    ScrollEvent, Styled as _, TestAppContext, VisualTestContext, point, px, size,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

#[cfg(target_os = "windows")]
mod frame_probe;
mod inertia;
#[cfg(target_os = "windows")]
mod native_windows;

/// Select the actual native surface used by the production adapters.
#[derive(Clone, Copy)]
enum Kind {
    /// Ordinary nested native overflow containers.
    Nested,
    /// Original document input and its private scroll authority.
    Editor,
    /// Base virtual tree with ten thousand source rows.
    Tree,
    /// Horizontal overflow uses vertical or horizontal wheel deltas.
    Tabs,
    /// Shared vertical projection above two original native source entities.
    Comparison,
}

/// All scroll state belongs to native handles/entities, never a test-local scroll implementation.
struct Harness {
    /// Parent viewport, or the independent horizontal tab strip.
    outer: ScrollHandle,
    /// Inner native viewport for nested propagation and reveal tests.
    inner: ScrollHandle,
    /// Sole text, selection, IME and Undo owner in source cases.
    editor: Entity<EditorState>,
    /// Original virtual-list and selection owner in tree cases.
    tree: Entity<gpui_base::TreeState>,
    /// Shared native comparison geometry, present only in the comparison case.
    group: Option<crate::aligned_editor::AlignmentGroup>,
    /// Which native production adapter is mounted.
    kind: Kind,
    /// Removing the actual element checks stale-animation cancellation on remount.
    show: bool,
    /// Actual row renderer calls measure visible work without wall-clock assertions.
    painted_rows: Rc<Cell<usize>>,
}

/// Dock-like caching must preserve wheel listeners when an unrelated root paint reuses the view.
struct CachedHarness(Entity<Harness>);
impl Render for CachedHarness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.0
            .clone()
            .cached(StyleRefinement::default().size_full())
    }
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.show {
            return gpui_kit::div().into_any_element();
        }
        let content = match self.kind {
            Kind::Nested => gpui_kit::div()
                .id("outer")
                .w(px(300.))
                .h(px(200.))
                .overflow_y_scroll()
                .track_scroll(&self.outer)
                .child(
                    gpui_kit::div().h(px(100.)).w_full().child(
                        gpui_kit::div()
                            .id("inner")
                            .w_full()
                            .h(px(100.))
                            .overflow_y_scroll()
                            .track_scroll(&self.inner)
                            .child(gpui_kit::div().h(px(1000.)).w_full())
                            .smooth_scroll(&self.inner, ScrollAxis::Vertical),
                    ),
                )
                .child(gpui_kit::div().h(px(1000.)).w_full())
                .smooth_scroll(&self.outer, ScrollAxis::Vertical)
                .into_any_element(),
            Kind::Editor => gpui_kit::div()
                .w(px(500.))
                .h(px(220.))
                .child(crate::source_editor::Editor::new(&self.editor))
                .into_any_element(),
            Kind::Tree => {
                let count = self.painted_rows.clone();
                gpui_base::Tree::new(&self.tree)
                    .w(px(300.))
                    .h(px(200.))
                    .list_style(StyleRefinement::default().size_full())
                    .item(move |_, entry, _, _, _| {
                        count.set(count.get() + 1);
                        gpui_kit::div()
                            .h(px(24.))
                            .child(entry.item().label.clone())
                            .into_any_element()
                    })
                    .smooth_scroll(self.tree.read(cx).scroll_handle(), ScrollAxis::Vertical)
                    .into_any_element()
            }
            Kind::Tabs => gpui_kit::div()
                .id("tabs")
                .w(px(300.))
                .h(px(32.))
                .overflow_x_scroll()
                .track_scroll(&self.outer)
                .child(gpui_kit::div().w(px(2000.)).h_full())
                .smooth_scroll(&self.outer, ScrollAxis::Horizontal)
                .into_any_element(),
            Kind::Comparison => {
                let group = self.group.as_ref().unwrap();
                let count = group.regions().len();
                gpui_kit::div()
                    .flex()
                    .w(px(250. * count as f32))
                    .h(px(220.))
                    .children((0..count).map(|index| {
                        gpui_kit::div()
                            .w(px(250.))
                            .h_full()
                            .child(group.pane(index).unwrap())
                    }))
                    .into_any_element()
            }
        };
        gpui_kit::div()
            .size_full()
            .child(content)
            .into_any_element()
    }
}

/// Mount a real Base Root so routing sees the same native hitboxes as application windows.
fn open(cx: &mut TestAppContext, kind: Kind) -> (Entity<Harness>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::typography::init(cx);
        crate::theme::apply_theme(crate::theme::builtin_theme(false), cx);
        crate::init(cx);
    });
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, ui) = cx.add_window_view(|window, cx| {
        let editor = cx.new(|cx| {
            EditorState::new(window, cx).default_value(
                (0..500)
                    .map(|i| format!("第 {i} 行 alpha 中文\n"))
                    .collect::<String>(),
            )
        });
        let tree = cx.new(|cx| gpui_base::TreeState::new(cx));
        tree.update(cx, |tree, cx| {
            tree.set_items(
                (0..10_000)
                    .map(|i| gpui_base::TreeItem::new(format!("item-{i}"), format!("row {i}")))
                    .collect::<Vec<_>>(),
                cx,
            )
        });
        let group = if matches!(kind, Kind::Comparison) {
            let other = cx.new(|cx| {
                EditorState::new(window, cx).default_value(
                    (0..500)
                        .map(|i| format!("第 {i} 行 alpha 中文\n"))
                        .collect::<String>(),
                )
            });
            let mut group = crate::aligned_editor::AlignmentGroup::new(cx);
            group
                .configure(
                    [editor.clone(), other]
                        .into_iter()
                        .enumerate()
                        .map(|(i, editor)| crate::aligned_editor::AlignmentPane {
                            editor,
                            revision: 1,
                            label: format!("source {i}"),
                            input_factory: None,
                            native_authority_factory: None,
                        })
                        .collect(),
                    &crate::aligned_pointer::AlignedPointerRouter::default(),
                    cx,
                )
                .unwrap();
            group.set_inner_gutters(true, cx);
            Some(group)
        } else {
            None
        };
        let view = cx.new(|_| Harness {
            outer: ScrollHandle::new(),
            inner: ScrollHandle::new(),
            editor,
            tree,
            group,
            kind,
            show: true,
            painted_rows: Rc::new(Cell::new(0)),
        });
        capture.replace(Some(view.clone()));
        let cached = cx.new(|_| CachedHarness(view));
        gpui_base::Root::new(cached, window, cx)
    });
    ui.simulate_resize(size(px(600.), px(400.)));
    for _ in 0..6 {
        frame(ui, 0);
    }
    (slot.borrow_mut().take().unwrap(), ui)
}

/// Advance the test executor's clock and draw one finite real native frame.
fn frame(ui: &mut VisualTestContext, millis: u64) {
    ui.executor().advance_clock(Duration::from_millis(millis));
    ui.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

/// Use a Windows-style discrete line wheel or a precise device's original pixel delta.
fn wheel(ui: &mut VisualTestContext, delta: ScrollDelta) {
    ui.simulate_event(ScrollEvent {
        // Wheel and trackpad fixtures retain pointer-device semantics.
        source: gpui_kit::ScrollSource::Pointer,
        position: point(px(50.), px(50.)),
        delta,
        modifiers: Default::default(),
        touch_phase: gpui_kit::TouchPhase::Moved,
    });
    frame(ui, 0);
}

/// Read the authoritative native offset, including the virtual list's public Base adapter.
fn offset(host: &Entity<Harness>, ui: &mut VisualTestContext) -> Point<gpui_kit::Pixels> {
    ui.update(|_, cx| native_offset(host.read(cx), cx))
}

/// Shared native authority read supports both deterministic and real Windows paint probes.
fn native_offset(host: &Harness, cx: &gpui_kit::App) -> Point<gpui_kit::Pixels> {
    use gpui_base::ScrollbarHandle as _;
    match host.kind {
        Kind::Nested => host.inner.offset(),
        Kind::Editor => host.editor.read(cx).scroll_offset(),
        Kind::Tree => host.tree.read(cx).scroll_handle().offset(),
        Kind::Tabs => host.outer.offset(),
        Kind::Comparison => point(px(0.), -host.group.as_ref().unwrap().scroll_offset(cx)),
    }
}

#[gpui::test]
fn smooth_scroll_native_surfaces_have_intermediate_frames_and_settle(cx: &mut TestAppContext) {
    for kind in [
        Kind::Nested,
        Kind::Editor,
        Kind::Tree,
        Kind::Tabs,
        Kind::Comparison,
    ] {
        let (host, ui) = open(cx, kind);
        let position = if matches!(kind, Kind::Tabs) {
            point(px(50.), px(16.))
        } else {
            point(px(50.), px(50.))
        };
        ui.simulate_event(ScrollEvent {
            // Wheel and trackpad fixtures retain pointer-device semantics.
            source: gpui_kit::ScrollSource::Pointer,
            position,
            delta: ScrollDelta::Lines(point(0., -3.)),
            modifiers: Default::default(),
            touch_phase: gpui_kit::TouchPhase::Moved,
        });
        frame(ui, 0);
        assert_eq!(
            offset(&host, ui),
            point(px(0.), px(0.)),
            "the initial frame must not jump to the target"
        );
        frame(ui, 40);
        let early = offset(&host, ui);
        assert!(
            early.x < px(0.) || early.y < px(0.),
            "native offset must move between frames"
        );
        frame(ui, 1000);
        let settled = offset(&host, ui);
        assert!(settled.x < early.x || settled.y < early.y);
        frame(ui, 170);
        assert_eq!(offset(&host, ui), settled);
        ui.update(|_, cx| {
            let host = host.read(cx);
            assert_eq!(
                host.outer.offset().y,
                px(0.),
                "nested inner scroll must not move its parent"
            );
            if matches!(kind, Kind::Editor) {
                assert!(host.editor.read(cx).value().starts_with("第 0 行"));
            }
            if matches!(kind, Kind::Tree) {
                assert!(
                    host.painted_rows.get() < 200,
                    "10,000 rows must remain virtualized"
                );
            }
        });
    }
}

#[gpui::test]
fn smooth_scroll_accumulates_reverses_and_yields_to_precise_input(cx: &mut TestAppContext) {
    let (host, ui) = open(cx, Kind::Nested);
    wheel(ui, ScrollDelta::Lines(point(0., -3.)));
    frame(ui, 40);
    let first = offset(&host, ui).y;
    wheel(ui, ScrollDelta::Lines(point(0., -3.)));
    frame(ui, 1000);
    assert!(offset(&host, ui).y < first * 2.);
    wheel(ui, ScrollDelta::Lines(point(0., -3.)));
    frame(ui, 35);
    let before = offset(&host, ui).y;
    wheel(ui, ScrollDelta::Lines(point(0., 2.)));
    frame(ui, 30);
    assert!(
        offset(&host, ui).y > before,
        "reverse input must discard old-direction backlog"
    );
    let before = offset(&host, ui).y;
    wheel(ui, ScrollDelta::Pixels(point(px(0.), px(-7.))));
    assert_eq!(offset(&host, ui).y, before - px(7.));
    frame(ui, 200);
    assert_eq!(
        offset(&host, ui).y,
        before - px(7.),
        "precise input cancels old easing"
    );
}

#[gpui::test]
fn smooth_scroll_boundaries_reveal_and_removal_keep_native_authority(cx: &mut TestAppContext) {
    let (host, ui) = open(cx, Kind::Nested);
    ui.update(|_, cx| {
        host.read(cx).inner.set_offset(point(px(0.), px(-900.)));
    });
    frame(ui, 0);
    wheel(ui, ScrollDelta::Lines(point(0., -3.)));
    frame(ui, 160);
    ui.update(|_, cx| {
        assert!(
            host.read(cx).outer.offset().y < px(0.),
            "an exhausted inner view yields to its parent"
        );
    });
    ui.update(|_, cx| {
        host.read(cx).inner.set_offset(point(px(0.), px(-100.)));
    });
    frame(ui, 0);
    wheel(ui, ScrollDelta::Lines(point(0., -3.)));
    frame(ui, 30);
    ui.update(|_, cx| {
        host.read(cx).inner.set_offset(point(px(0.), px(-300.)));
    });
    frame(ui, 0);
    frame(ui, 200);
    assert_eq!(
        offset(&host, ui).y,
        px(-300.),
        "programmatic reveal wins over pending easing"
    );
    wheel(ui, ScrollDelta::Lines(point(0., -3.)));
    ui.update(|_, cx| {
        host.update(cx, |host, cx| {
            host.show = false;
            cx.notify();
        })
    });
    frame(ui, 0);
    let hidden = offset(&host, ui);
    frame(ui, 200);
    assert_eq!(
        offset(&host, ui),
        hidden,
        "removed surfaces cannot be moved by a stale frame"
    );
    ui.update(|_, cx| {
        host.update(cx, |host, cx| {
            host.show = true;
            cx.notify();
        })
    });
    frame(ui, 0);
    frame(ui, 180);
    assert_eq!(
        offset(&host, ui),
        hidden,
        "remounting a cached surface must not revive old wheel motion"
    );
    ui.update(|window, cx| {
        window.simulate_next_frame(cx);
    });
    frame(ui, 0);
    ui.update(|window, cx| {
        assert_eq!(
            window.simulate_next_frame(cx),
            0,
            "idle motion must not request frames"
        )
    });
}

#[gpui::test]
fn smooth_scroll_reduced_motion_uses_native_wheel_immediately(cx: &mut TestAppContext) {
    let (host, ui) = open(cx, Kind::Nested);
    ui.update(|_, cx| cx.set_reduce_motion(true));
    wheel(ui, ScrollDelta::Lines(point(0., -3.)));
    assert!(offset(&host, ui).y < px(0.));
    let actual = offset(&host, ui);
    frame(ui, 200);
    assert_eq!(offset(&host, ui), actual);
}

/// A high-rate input burst updates one endpoint; layout work remains bounded by drawn frames.
#[gpui::test]
fn smooth_scroll_burst_preserves_virtualization_and_stops_requesting_frames(
    cx: &mut TestAppContext,
) {
    let (host, ui) = open(cx, Kind::Tree);
    let before = ui.update(|_, cx| host.read(cx).painted_rows.get());
    ui.update(|window, cx| {
        for _ in 0..120 {
            window.dispatch_event(
                gpui_kit::PlatformInput::Scroll(ScrollEvent {
                    // Wheel and trackpad fixtures retain pointer-device semantics.
                    source: gpui_kit::ScrollSource::Pointer,
                    position: point(px(50.), px(50.)),
                    delta: ScrollDelta::Lines(point(0., -3.)),
                    modifiers: Default::default(),
                    touch_phase: gpui_kit::TouchPhase::Moved,
                }),
                cx,
            );
        }
        assert_eq!(
            host.read(cx).painted_rows.get(),
            before,
            "wheel callbacks must not lay out tree rows"
        );
    });
    frame(ui, 0);
    frame(ui, 35);
    let early = offset(&host, ui).y;
    frame(ui, 1000);
    assert!(offset(&host, ui).y < early && early < px(0.));
    ui.update(|window, cx| {
        window.simulate_next_frame(cx);
    });
    frame(ui, 0);
    ui.update(|window, cx| {
        assert!(
            host.read(cx).painted_rows.get() - before < 100,
            "burst work must remain bounded by visible rows"
        );
        assert_eq!(
            window.simulate_next_frame(cx),
            0,
            "settled wheel motion must have no frame demand"
        );
    });
}

/// A real pointer press cancels pending wheel motion before native dragging or selection starts.
#[gpui::test]
fn smooth_scroll_pointer_input_cancels_pending_motion(cx: &mut TestAppContext) {
    let (host, ui) = open(cx, Kind::Nested);
    wheel(ui, ScrollDelta::Lines(point(0., -3.)));
    frame(ui, 30);
    let before = offset(&host, ui);
    ui.simulate_event(gpui_kit::MouseDownEvent {
        position: point(px(550.), px(350.)),
        button: gpui_kit::MouseButton::Left,
        modifiers: Default::default(),
        click_count: 1,
        first_mouse: false,
    });
    frame(ui, 180);
    assert_eq!(offset(&host, ui), before);
}

/// Native location requests are deferred until paint and must win over an older wheel animation.
#[gpui::test]
fn smooth_scroll_native_deferred_location_wins_over_pending_wheel(cx: &mut TestAppContext) {
    let (host, ui) = open(cx, Kind::Editor);
    wheel(ui, ScrollDelta::Lines(point(0., -3.)));
    frame(ui, 30);
    assert!(
        offset(&host, ui).y < px(0.),
        "the native wheel must already be in flight"
    );
    ui.update(|_, cx| {
        host.read(cx).editor.clone().update(cx, |editor, cx| {
            editor.set_scroll_offset(point(px(0.), px(-300.)), cx)
        });
    });
    frame(ui, 25);
    assert_eq!(offset(&host, ui).y, px(-300.));
    frame(ui, 180);
    assert_eq!(
        offset(&host, ui).y,
        px(-300.),
        "a deferred native reveal cancels the old wheel endpoint"
    );
}

/// Logical wheel travel is independent of physical DPI and the dark/light paint palette.
#[gpui::test]
fn smooth_scroll_theme_and_dpi_keep_native_logical_distance(cx: &mut TestAppContext) {
    let (host, ui) = open(cx, Kind::Tree);
    let mut first = None;
    for (dark, scale) in [(false, 1.), (true, 1.5)] {
        ui.update(|_, cx| {
            crate::theme::apply_theme(crate::theme::builtin_theme(dark), cx);
            gpui_base::ScrollbarHandle::set_offset(
                host.read(cx).tree.read(cx).scroll_handle(),
                point(px(0.), px(0.)),
            );
        });
        ui.simulate_scale_factor_change(scale);
        frame(ui, 0);
        wheel(ui, ScrollDelta::Lines(point(0., -3.)));
        frame(ui, 40);
        assert!(offset(&host, ui).y < px(0.));
        frame(ui, 1000);
        let actual = offset(&host, ui);
        if let Some(first) = first {
            assert_eq!(actual, first);
        } else {
            first = Some(actual);
        }
    }
}
