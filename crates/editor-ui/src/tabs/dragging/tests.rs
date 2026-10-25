//! Native public-strip interactions and unequal-width geometry guard release-only document moves.

mod axis;

use super::*;
use gpui_kit::{
    AppContext as _, Bounds, Context, Entity, FocusHandle, MouseButton, Pixels, Point, Render,
    TestAppContext, VisualTestContext, gpui, point, size,
};
use std::{cell::RefCell, time::Duration};

/// An owner applies events to its model, while clicks and close controls remain independent.
struct Harness {
    /// Stable IDs and deliberately unequal native widths.
    tabs: Vec<(u64, f32)>,
    /// Public gesture state retained across ordinary frames.
    state: FileTabDragState,
    /// Native handle also used by programmatic reveal.
    scroll: ScrollHandle,
    /// Focus belongs to the body, not the tab strip, matching the editor.
    focus: FocusHandle,
    /// Width can constrain the strip without changing any tab's dimensions.
    width: f32,
    /// Vertical lists and horizontal bars retain the same public state and scroll handle.
    axis: Axis,
    /// Height limits vertical overflow without changing the fixed row height.
    height: f32,
    /// Optional derived presentation verifies direct child indices after the document rows.
    comparison: bool,
    /// Let title content determine dimensions for live dirty/rename regression coverage.
    natural_titles: bool,
    /// The first document's title can change without replacing its stable identity.
    source_title: String,
    /// Observed moves, emitted only after accepted releases.
    moves: Vec<FileTabMove>,
    /// Original Base callbacks count native activations.
    clicks: usize,
    /// Child close buttons retain their original activation.
    closes: usize,
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut strip = FileTabStrip::new("tabs", self.state.clone(), self.scroll.clone())
            .axis(self.axis)
            .on_reorder(cx.listener(|this, event: &FileTabMove, _, cx| {
                let source = this
                    .tabs
                    .iter()
                    .position(|(id, _)| *id == event.source)
                    .unwrap();
                let tab = this.tabs.remove(source);
                let target = event
                    .before
                    .and_then(|before| this.tabs.iter().position(|(id, _)| *id == before))
                    .unwrap_or(this.tabs.len());
                this.tabs.insert(target, tab);
                this.moves.push(event.clone());
                cx.notify();
            }));
        for (id, width) in &self.tabs {
            let id = *id;
            let title = if id == 1 {
                self.source_title.clone()
            } else {
                format!("file-{id}.rs")
            };
            let tab = Tab::new(("tab", id))
                .debug_selector(move || format!("tab-{id}"))
                .w(px(*width))
                .when(self.natural_titles, |tab| tab.w_auto().max_w(px(400.)))
                .h_full()
                .px_2()
                .gap_2()
                .child(div().flex_1().min_w(px(0.)).truncate().child(title.clone()))
                .child(
                    div()
                        .id(("close", id))
                        .debug_selector(move || format!("close-{id}"))
                        .w(px(18.))
                        .h(px(18.))
                        .child("×")
                        .on_click(cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.closes += 1;
                        })),
                )
                .on_click(cx.listener(|this, _, _, _| this.clicks += 1));
            strip = strip.tab(
                id,
                FileTabPreview {
                    label: title.into(),
                    icon: gpui_kit::assets::IconName::File.path().into(),
                    preview: id == 1,
                    external: id == 3,
                },
                tab,
            );
        }
        if self.comparison {
            strip = strip.trailing(crate::comparison_tab(
                "comparison".into(),
                "Working changes".into(),
                true,
                self.axis,
                false,
                self.tabs.len(),
                self.tabs.len() + 1,
                |_, _, _| {},
                |_, _| {},
                cx,
            ));
        }
        let cancel = self.state.clone();
        div()
            .id("tab-harness")
            .size_full()
            .p(px(20.))
            .capture_key_down(move |event, window, cx| {
                if event.keystroke.key == "escape" && cancel.active() {
                    cancel.cancel(window, cx);
                    cx.stop_propagation();
                }
            })
            .child(div().w(px(self.width)).h(px(self.height)).child(strip))
            .child(div().id("body").track_focus(&self.focus).h(px(100.)))
    }
}

/// Open a themed native root with body focus and return its retained public-control owner.
fn harness(cx: &mut TestAppContext, width: f32) -> (Entity<Harness>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::typography::init(cx);
        crate::theme::apply_theme(crate::theme::builtin_theme(false), cx);
    });
    let retained = Rc::new(RefCell::new(None));
    let stored = retained.clone();
    let (_, ui) = cx.add_window_view(move |window, cx| {
        let view = cx.new(|cx| {
            let focus = cx.focus_handle();
            focus.focus(window, cx);
            Harness {
                tabs: vec![(1, 120.), (2, 160.), (3, 100.)],
                state: FileTabDragState::default(),
                scroll: ScrollHandle::new(),
                focus,
                width,
                axis: Axis::Horizontal,
                height: crate::PANEL_HEADER_HEIGHT,
                comparison: false,
                natural_titles: false,
                source_title: "file-1.rs".into(),
                moves: Vec::new(),
                clicks: 0,
                closes: 0,
            }
        });
        stored.replace(Some(view.clone()));
        gpui_base::Root::new(view, window, cx)
    });
    ui.update(|window, _| window.activate_window());
    ui.simulate_resize(size(px(600.), px(250.)));
    frame(ui, 0);
    let view = retained.borrow_mut().take().unwrap();
    (view, ui)
}

/// Advance bounded timer time and draw; native springs separately use the real wall clock.
fn frame(ui: &mut VisualTestContext, millis: u64) {
    ui.executor().advance_clock(Duration::from_millis(millis));
    ui.run_until_parked();
    ui.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

/// Cross Base's native threshold without a long press and retain the initial unshifted pointer.
fn begin(ui: &mut VisualTestContext, id: u64) -> Point<Pixels> {
    let selector = match id {
        1 => "tab-1",
        2 => "tab-2",
        3 => "tab-3",
        _ => panic!("unknown fixture tab {id}"),
    };
    let start = ui.debug_bounds(selector).unwrap().center();
    ui.simulate_mouse_down(start, MouseButton::Left, Default::default());
    frame(ui, 0);
    ui.simulate_mouse_move(
        start + point(px(8.), px(0.)),
        MouseButton::Left,
        Default::default(),
    );
    frame(ui, 0);
    assert!(ui.update(|_, cx| cx.has_active_drag()));
    start
}

/// The floating card retains source width; neighbors move while the owner's order stays unchanged.
#[gpui::test]
fn floating_drag_only_commits_on_release(cx: &mut TestAppContext) {
    let (view, ui) = harness(cx, 500.);
    let original = ui.debug_bounds("tab-1").unwrap();
    let start = begin(ui, 1);
    let target = point(px(440.), start.y);
    ui.simulate_mouse_move(target, MouseButton::Left, Default::default());
    // GPUI 0.3.9's spring reads std::time::Instant on Windows, whereas advance_clock
    // advances only the executor's separate TestClock and timers. Establish the target,
    // then allow an observable displacement, not full settling, on the native clock.
    // Keeping the real bounds assertion also detects an accidentally remounted spring.
    frame(ui, 0);
    std::thread::sleep(Duration::from_millis(80));
    frame(ui, 0);
    let floating = ui.debug_bounds("file-tab-drag-preview").unwrap();
    let slot = ui.debug_bounds("file-tab-drag-slot").unwrap();
    assert_eq!(floating.size.width, original.size.width);
    assert_eq!(slot.size.width, original.size.width);
    assert!(floating.top() < original.top());
    assert!(ui.debug_bounds("tab-2").unwrap().left() < px(140.));
    ui.update(|_, cx| {
        assert_eq!(
            view.read(cx)
                .tabs
                .iter()
                .map(|(id, _)| *id)
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert!(view.read(cx).moves.is_empty());
    });
    ui.simulate_mouse_up(target, MouseButton::Left, Default::default());
    frame(ui, 0);
    ui.update(|_, cx| {
        assert_eq!(
            view.read(cx).moves,
            vec![FileTabMove {
                source: 1,
                before: None
            }]
        );
        assert_eq!(
            view.read(cx)
                .tabs
                .iter()
                .map(|(id, _)| *id)
                .collect::<Vec<_>>(),
            vec![2, 3, 1]
        );
        assert_eq!(view.read(cx).clicks, 0);
    });
    assert!(ui.debug_bounds("file-tab-drag-preview").is_none());
    assert!(ui.debug_bounds("file-tab-drag-slot").is_none());
    assert_eq!(ui.debug_bounds("tab-2").unwrap().left(), px(20.));
}

/// Escape is captured above body focus and an outside release cannot activate or reorder a tab.
#[gpui::test]
fn escape_and_outside_release_preserve_native_clicks(cx: &mut TestAppContext) {
    let (view, ui) = harness(cx, 500.);
    let start = begin(ui, 2);
    ui.simulate_keystrokes("escape");
    frame(ui, 0);
    assert!(!ui.update(|_, cx| cx.has_active_drag()));
    ui.simulate_mouse_up(start, MouseButton::Left, Default::default());
    begin(ui, 1);
    let outside = point(px(80.), px(90.));
    ui.simulate_mouse_move(outside, MouseButton::Left, Default::default());
    frame(ui, 0);
    assert!(ui.debug_bounds("file-tab-drag-slot").is_none());
    ui.simulate_mouse_up(outside, MouseButton::Left, Default::default());
    frame(ui, 0);
    let title = ui.debug_bounds("tab-2").unwrap().center();
    ui.simulate_click(title, Default::default());
    frame(ui, 0);
    let close = ui.debug_bounds("close-2").unwrap().center();
    ui.simulate_click(close, Default::default());
    ui.update(|_, cx| {
        assert!(view.read(cx).moves.is_empty());
        assert_eq!(view.read(cx).clicks, 1);
        assert_eq!(view.read(cx).closes, 1);
    });
}

/// Canonical membership replacement and window deactivation revoke the matching native preview.
#[gpui::test]
fn membership_change_and_blur_cancel_file_drag(cx: &mut TestAppContext) {
    let (view, ui) = harness(cx, 500.);
    begin(ui, 1);
    ui.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.tabs.remove(0);
            cx.notify();
        })
    });
    frame(ui, 0);
    assert!(!ui.update(|_, cx| cx.has_active_drag()));
    ui.simulate_mouse_up(
        point(px(40.), px(34.)),
        MouseButton::Left,
        Default::default(),
    );
    begin(ui, 2);
    ui.deactivate_window();
    frame(ui, 20);
    assert!(!ui.update(|_, cx| cx.has_active_drag()));
    ui.update(|_, cx| assert!(view.read(cx).moves.is_empty()));
}

/// A dirty/renamed title cannot change the occupied source width until its drag ends.
#[gpui::test]
fn live_source_title_keeps_frozen_drag_allocation(cx: &mut TestAppContext) {
    let (view, ui) = harness(cx, 500.);
    ui.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.natural_titles = true;
            view.source_title = "a.rs".into();
            cx.notify();
        })
    });
    frame(ui, 0);
    let original = ui.debug_bounds("tab-1").unwrap();
    let neighbor = ui.debug_bounds("file-tab-slot-2").unwrap();
    let start = begin(ui, 1);
    ui.update(|_, cx| {
        view.update(cx, |view, cx| {
            view.source_title = "a_much_longer_dirty_document_name.rs ●".into();
            cx.notify();
        })
    });
    frame(ui, 0);
    assert_eq!(
        ui.debug_bounds("tab-1").unwrap().size.width,
        original.size.width
    );
    assert_eq!(
        ui.debug_bounds("file-tab-slot-1").unwrap().size.width,
        original.size.width
    );
    assert_eq!(
        ui.debug_bounds("file-tab-slot-2").unwrap().left(),
        neighbor.left()
    );
    assert_eq!(
        ui.debug_bounds("file-tab-drag-slot").unwrap().size.width,
        original.size.width
    );
    ui.simulate_keystrokes("escape");
    ui.simulate_mouse_up(start, MouseButton::Left, Default::default());
    frame(ui, 0);
    assert!(ui.debug_bounds("tab-1").unwrap().size.width > original.size.width);
    ui.update(|_, cx| assert!(view.read(cx).moves.is_empty()));
}

/// Direct child indices still reveal the requested document; edge motion exposes overflow tabs.
#[gpui::test]
fn reveal_indices_and_edge_scroll_keep_native_handle(cx: &mut TestAppContext) {
    let (view, ui) = harness(cx, 220.);
    ui.update(|_, cx| view.read(cx).scroll.scroll_to_item(2));
    frame(ui, 0);
    let viewport = ui.debug_bounds("file-tab-strip").unwrap();
    assert!(ui.debug_bounds("tab-3").unwrap().right() <= viewport.right());
    ui.update(|_, cx| view.read(cx).scroll.set_offset(point(px(0.), px(0.))));
    frame(ui, 0);
    let start = begin(ui, 1);
    let edge = point(viewport.right() - px(3.), start.y);
    ui.simulate_mouse_move(edge, MouseButton::Left, Default::default());
    frame(ui, 48);
    assert!(ui.update(|_, cx| view.read(cx).scroll.offset().x) < px(0.));
    ui.simulate_mouse_up(
        point(edge.x, px(90.)),
        MouseButton::Left,
        Default::default(),
    );
}

/// The same midpoint and shift invariants hold for unequal extents on either principal axis.
#[test]
fn unequal_width_insertion_geometry() {
    for axis in [Axis::Horizontal, Axis::Vertical] {
        let position = |leading, cross| match axis {
            Axis::Horizontal => point(px(leading), px(cross)),
            Axis::Vertical => point(px(cross), px(leading)),
        };
        let dimensions = |extent, cross| match axis {
            Axis::Horizontal => size(px(extent), px(cross)),
            Axis::Vertical => size(px(cross), px(extent)),
        };
        let mut state = state::State {
            axis,
            ids: vec![1, 2, 3],
            viewport: Bounds::new(position(20., 10.), dimensions(500., 28.)),
            ..state::State::default()
        };
        for (id, leading, extent) in [(1, 20., 120.), (2, 140., 160.), (3, 300., 100.)] {
            state.items.insert(
                id,
                Bounds::new(position(leading, 10.), dimensions(extent, 28.)),
            );
        }
        state.session = Some(state::Session {
            source: 1,
            pointer: position(370., 24.),
            token: 1,
            source_bounds: state.items[&1],
        });
        assert_eq!(
            state.completed(),
            Some(FileTabMove {
                source: 1,
                before: None
            })
        );
        assert_eq!(state.placeholder().unwrap().origin.along(axis), px(280.));
        assert_eq!(state.presentation(2), (false, px(-120.)));
        assert_eq!(state.presentation(3), (false, px(-120.)));
        state.session.as_mut().unwrap().pointer = position(80., 24.);
        assert_eq!(state.completed(), None);
        assert_eq!(state.placeholder().unwrap().origin.along(axis), px(20.));
        state.session.as_mut().unwrap().source = 3;
        state.session.as_mut().unwrap().source_bounds = state.items[&3];
        assert_eq!(
            state.completed(),
            Some(FileTabMove {
                source: 3,
                before: Some(2)
            })
        );
        // Exact midpoints insert after that tab, without an oscillating drop threshold.
        assert_eq!(state.presentation(2), (false, px(100.)));
        state.session.as_mut().unwrap().pointer = position(80., 90.);
        assert_eq!(state.completed(), None);
        assert_eq!(state.placeholder(), None);
    }
}
