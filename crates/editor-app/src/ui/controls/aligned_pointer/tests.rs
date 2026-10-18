//! Native pointer routing regressions: complete-root ordering, pane capture and safe retirement.
use super::*;
use gpui_base::input::{Editor, EditorState};
use gpui_kit::{
    AppContext as _, Context, Entity, HitboxBehavior, InteractiveElement as _, ParentElement as _,
    Render, Styled as _, TestAppContext, VisualTestContext, canvas, div, gpui, size,
};

const SOURCE: &str = "first\nsecond\nthird\nfourth\nfifth\n";

/// Actual source entities survive hiding; the router never owns their mutable document state.
struct Fixture {
    router: AlignedPointerRouter,
    regions: [PointerRegion; 2],
    editors: [Entity<EditorState>; 2],
    first_visible: bool,
    wrap_directly: bool,
    capture_moves: Rc<Cell<usize>>,
    capture_downs: Rc<Cell<usize>>,
    clicks: Rc<Cell<usize>>,
    cancel_on_down: Rc<Cell<bool>>,
    hide_on_cancel: Rc<Cell<bool>>,
    hidden_after_release: Rc<Cell<bool>>,
}

/// Publishes the complete input's hitbox before native paint. This fixture tests coordinate routing
/// only; its fixed projection does not draw blank rows or claim to verify aligned editor layout.
fn pane(editor: &Entity<EditorState>, region: &PointerRegion) -> AnyElement {
    let region = region.clone();
    div()
        .relative()
        .w(px(250.))
        .h(px(200.))
        .child(
            canvas(
                move |bounds, window, _| {
                    let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                    region.publish(hitbox, bounds, |position| {
                        point(position.x, position.y - px(30.))
                    });
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
        )
        .child(Editor::new(editor))
        .into_any_element()
}

impl Render for Fixture {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let moves = self.capture_moves.clone();
        let downs = self.capture_downs.clone();
        let clicks = self.clicks.clone();
        let cancel_on_down = self.cancel_on_down.clone();
        let cancel_region = self.regions[0].clone();
        let cancel_router = self.router.clone();
        let hide_on_cancel = self.hide_on_cancel.clone();
        let hidden_after_release = self.hidden_after_release.clone();
        let owner = cx.entity().downgrade();
        let first = if self.first_visible {
            pane(&self.editors[0], &self.regions[0])
        } else {
            div().w(px(250.)).h(px(200.)).into_any_element()
        };
        // This capture observer is inside the full root, preceding both pane elements. The wrapper
        // must shield it from restoration, unlike a pane-local interceptor.
        let content = div()
            .size_full()
            .flex()
            .flex_col()
            .text_size(px(14.))
            .capture_any_mouse_down(move |_, window, cx| {
                downs.set(downs.get() + 1);
                if cancel_on_down.replace(false) {
                    cancel_region.request_cancel();
                    assert!(
                        !cancel_router.flush(window, cx),
                        "nested native dispatch must defer release until listeners are restored"
                    );
                    if hide_on_cancel.replace(false) {
                        let owner = owner.clone();
                        let completion = hidden_after_release.clone();
                        cancel_router.after_pending_release(window, cx, move |_, cx| {
                            owner
                                .update(cx, |state, cx| {
                                    state.first_visible = false;
                                    cx.notify();
                                })
                                .unwrap();
                            completion.set(true);
                        });
                        assert!(!hidden_after_release.get(), "busy close cannot hide early");
                    }
                }
            })
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, _, window, _| {
                        window.on_mouse_event(move |_: &MouseMoveEvent, phase, _, _| {
                            if phase.capture() {
                                moves.set(moves.get() + 1);
                            }
                        });
                    },
                )
                .h(px(0.))
                .w_full(),
            )
            .child(
                div()
                    .flex()
                    .h(px(200.))
                    .child(first)
                    .child(pane(&self.editors[1], &self.regions[1])),
            )
            .child(
                div()
                    .h(px(40.))
                    .w_full()
                    .on_mouse_down(MouseButton::Left, move |_, _, _| {
                        clicks.set(clicks.get() + 1)
                    })
                    .child("Independent control"),
            )
            .into_any_element();
        if self.wrap_directly {
            self.router.wrap(content)
        } else {
            content
        }
    }
}

/// Initializes real platform input and editor state, leaving all mouse and keyboard handling native.
fn fixture(cx: &mut TestAppContext) -> (Entity<Fixture>, &mut VisualTestContext) {
    cx.update(|cx| gpui_kit::init(cx));
    let (view, ui) = cx.add_window_view(build_fixture);
    ui.simulate_resize(size(px(500.), px(300.)));
    draw(ui);
    (view, ui)
}

/// Builds the same content either as a direct test root or beneath a genuine Base Root.
fn build_fixture(window: &mut Window, cx: &mut Context<Fixture>) -> Fixture {
    let router = AlignedPointerRouter::default();
    let editors = std::array::from_fn(|_| {
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .line_number(false)
                .soft_wrap(false)
        });
        editor.update(cx, |state, cx| {
            state.set_value(SOURCE, window, cx);
            state.set_selected_range(0..0, cx);
        });
        editor
    });
    Fixture {
        regions: [router.register(), router.register()],
        router,
        editors,
        first_visible: true,
        wrap_directly: true,
        capture_moves: Rc::default(),
        capture_downs: Rc::default(),
        clicks: Rc::default(),
        cancel_on_down: Rc::default(),
        hide_on_cancel: Rc::default(),
        hidden_after_release: Rc::default(),
    }
}

/// Flushes layout so public source bounds match the visible frame used by simulated platform input.
fn draw(ui: &mut VisualTestContext) {
    ui.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
        window.draw(cx).clear(cx);
    });
}

/// Adds the fixture's fixed physical gap to an actual native caret location.
fn source_point(
    view: &Entity<Fixture>,
    ui: &mut VisualTestContext,
    side: usize,
    offset: usize,
) -> Point<Pixels> {
    ui.update(|_, cx| {
        let state = view.read(cx).editors[side].read(cx);
        let caret = state.range_to_bounds(&(offset..offset)).unwrap();
        point(
            caret.left() + px(1.),
            caret.top() + state.line_height().unwrap() / 2. + px(30.),
        )
    })
}

/// Even the original root's capture listeners see one projected event and no pointer restoration.
#[gpui::test]
#[ignore = "native window pointer regression; run explicitly with --ignored --test-threads=1"]
fn aligned_pointer_root_hides_restore_and_preserves_native_unicode_undo(cx: &mut TestAppContext) {
    let (view, ui) = fixture(cx);
    let physical = source_point(&view, ui, 0, 6);
    ui.simulate_mouse_move(physical, None, Default::default());
    ui.simulate_click(physical, Default::default());
    ui.update(|window, cx| {
        let state = view.read(cx);
        assert_eq!(state.editors[0].read(cx).selected_range(), 6..6);
        assert_eq!(
            state.capture_moves.get(),
            1,
            "restoration cannot reach an ancestor capture listener"
        );
        assert_eq!(state.capture_downs.get(), 1);
        assert_eq!(state.clicks.get(), 0);
        assert_eq!(window.mouse_position(), physical);
    });
    ui.simulate_input("中文🙂");
    ui.update(|_, cx| {
        assert_eq!(
            view.read(cx).editors[0].read(cx).value().as_ref(),
            "first\n中文🙂second\nthird\nfourth\nfifth\n"
        )
    });
    ui.simulate_keystrokes("ctrl-z");
    ui.update(|_, cx| assert_eq!(view.read(cx).editors[0].read(cx).value().as_ref(), SOURCE));
    ui.simulate_click(point(px(20.), px(220.)), Default::default());
    ui.update(|_, cx| assert_eq!(view.read(cx).clicks.get(), 1));
}

/// Crossing a second pane cannot transfer the source's drag or leave it selecting after outside up.
#[gpui::test]
#[ignore = "native window pointer regression; run explicitly with --ignored --test-threads=1"]
fn aligned_pointer_drag_stays_with_origin_and_releases_outside(cx: &mut TestAppContext) {
    let (view, ui) = fixture(cx);
    let first = source_point(&view, ui, 0, 6);
    let other = source_point(&view, ui, 1, 13);
    ui.simulate_mouse_down(first, MouseButton::Left, Default::default());
    ui.simulate_mouse_move(other, MouseButton::Left, Default::default());
    ui.simulate_mouse_up(
        point(px(650.), px(280.)),
        MouseButton::Left,
        Default::default(),
    );
    let selection = ui.update(|window, cx| {
        let state = view.read(cx);
        assert_ne!(state.editors[0].read(cx).selected_range(), 6..6);
        assert_eq!(state.editors[1].read(cx).selected_range(), 0..0);
        assert_eq!(state.clicks.get(), 0);
        assert_eq!(window.mouse_position(), point(px(650.), px(280.)));
        state.editors[0].read(cx).selected_range()
    });
    // A stray pressed move is a sensitive public observation of a stale native selecting flag.
    ui.simulate_mouse_move(first, MouseButton::Left, Default::default());
    ui.update(|_, cx| {
        assert_eq!(
            view.read(cx).editors[0].read(cx).selected_range(),
            selection
        )
    });
    ui.simulate_click(other, Default::default());
    ui.update(|_, cx| assert_eq!(view.read(cx).editors[1].read(cx).selected_range(), 13..13));
}

/// Retirement delivers native release before hiding; remounting the same entity never revives drag.
#[gpui::test]
#[ignore = "native window pointer regression; run explicitly with --ignored --test-threads=1"]
fn aligned_pointer_retirement_releases_native_state_before_remount(cx: &mut TestAppContext) {
    let (view, ui) = fixture(cx);
    let first = source_point(&view, ui, 0, 6);
    ui.simulate_mouse_down(first, MouseButton::Left, Default::default());
    let completion = Rc::new(Cell::new(false));
    ui.update(|window, cx| {
        let state = view.read(cx);
        let router = state.router.clone();
        let region = state.regions[0].clone();
        assert!(
            !region.clear(),
            "active native state must be released before clear"
        );
        let view = view.clone();
        let completion = completion.clone();
        router.retire(&region, window, cx, move |_, cx| {
            view.update(cx, |state, cx| {
                state.first_visible = false;
                cx.notify();
            });
            completion.set(true);
        });
    });
    ui.run_until_parked();
    assert!(completion.get());
    draw(ui);
    // The physical tail must not activate an unrelated control after its origin disappears.
    ui.simulate_mouse_up(
        point(px(20.), px(220.)),
        MouseButton::Left,
        Default::default(),
    );
    ui.update(|_, cx| {
        view.update(cx, |state, cx| {
            state.first_visible = true;
            cx.notify();
        })
    });
    draw(ui);
    let selection = ui.update(|_, cx| view.read(cx).editors[0].read(cx).selected_range());
    let later = source_point(&view, ui, 0, 20);
    ui.simulate_mouse_move(later, MouseButton::Left, Default::default());
    ui.update(|_, cx| {
        let state = view.read(cx);
        assert_eq!(state.editors[0].read(cx).selected_range(), selection);
        assert_eq!(state.clicks.get(), 0);
        assert!(state.regions[0].clear());
    });
}

/// An external owner can withdraw a pane without a Window. The old frame must receive native up
/// during the next prepaint, before its listeners disappear; the same EditorState remains reusable.
#[gpui::test]
#[ignore = "native window pointer cancellation regression; run explicitly with --ignored --test-threads=1"]
fn aligned_pointer_owner_withdrawal_cancels_before_replacing_the_frame(cx: &mut TestAppContext) {
    let (view, ui) = fixture(cx);
    let first = source_point(&view, ui, 0, 6);
    let later = source_point(&view, ui, 0, 20);
    ui.simulate_mouse_down(first, MouseButton::Left, Default::default());
    ui.simulate_mouse_move(later, MouseButton::Left, Default::default());
    ui.update(|_, cx| {
        view.update(cx, |state, cx| {
            assert_ne!(state.editors[0].read(cx).selected_range(), 6..6);
            state.regions[0].request_cancel();
            state.regions[0] = state.router.register();
            state.first_visible = false;
            cx.notify();
        });
    });
    draw(ui);
    ui.update(|_, cx| {
        view.update(cx, |state, cx| {
            state.first_visible = true;
            cx.notify();
        });
    });
    draw(ui);
    let selection = ui.update(|_, cx| view.read(cx).editors[0].read(cx).selected_range());
    let first = source_point(&view, ui, 0, 0);
    ui.simulate_mouse_move(first, None, Default::default());
    ui.update(|_, cx| {
        assert_eq!(
            view.read(cx).editors[0].read(cx).selected_range(),
            selection
        );
    });
    // Clear the physical tail first, so a stale native selecting flag cannot be hidden by the
    // router's cancelled-gesture ownership when the following stray pressed move reaches Base.
    ui.simulate_mouse_up(
        point(px(20.), px(220.)),
        MouseButton::Left,
        Default::default(),
    );
    ui.simulate_mouse_move(first, MouseButton::Left, Default::default());
    ui.update(|_, cx| {
        let state = view.read(cx);
        assert_eq!(state.editors[0].read(cx).selected_range(), selection);
        assert_eq!(state.editors[0].read(cx).value().as_ref(), SOURCE);
        assert_eq!(state.clicks.get(), 0);
    });
}

/// A request from an actual forwarded down callback sees GPUI's listener vector temporarily moved
/// out. Keep that request until deferred dispatch can deliver native up after down has completed.
#[gpui::test]
#[ignore = "native nested pointer cancellation regression; run explicitly with --ignored --test-threads=1"]
fn aligned_pointer_nested_cancel_waits_for_the_native_dispatch_to_unwind(cx: &mut TestAppContext) {
    let (view, ui) = fixture(cx);
    ui.update(|_, cx| {
        view.read(cx).cancel_on_down.set(true);
        view.read(cx).hide_on_cancel.set(true);
    });
    let first = source_point(&view, ui, 0, 6);
    ui.simulate_mouse_down(first, MouseButton::Left, Default::default());
    ui.run_until_parked();
    ui.update(|_, cx| {
        assert!(!view.read(cx).cancel_on_down.get());
        assert!(view.read(cx).hidden_after_release.get());
        assert!(!view.read(cx).first_visible);
    });
    draw(ui);
    ui.update(|_, cx| {
        view.update(cx, |state, cx| {
            state.first_visible = true;
            cx.notify();
        });
    });
    draw(ui);
    ui.simulate_mouse_up(
        point(px(650.), px(280.)),
        MouseButton::Left,
        Default::default(),
    );
    let selection = ui.update(|_, cx| view.read(cx).editors[0].read(cx).selected_range());
    assert_eq!(selection, 6..6);
    let later = source_point(&view, ui, 0, 20);
    ui.simulate_mouse_move(later, MouseButton::Left, Default::default());
    ui.update(|_, cx| {
        assert_eq!(
            view.read(cx).editors[0].read(cx).selected_range(),
            selection
        );
    });
}

/// An independently registered presentation with both an outer decorator and a root overlay.
#[derive(Default)]
struct OverlayCapture {
    overlay_moves: Rc<Cell<usize>>,
    overlay_downs: Rc<Cell<usize>>,
    decorator_downs: Rc<Cell<usize>>,
}

impl Render for OverlayCapture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let moves = self.overlay_moves.clone();
        let downs = self.overlay_downs.clone();
        // The transparent overlay adds ordinary capture listeners without occluding native input.
        div()
            .absolute()
            .size_full()
            .capture_any_mouse_down(move |_, _, _| downs.set(downs.get() + 1))
            .child(
                canvas(
                    |_, _, _| (),
                    move |_, _, window, _| {
                        window.on_mouse_event(move |_: &MouseMoveEvent, phase, _, _| {
                            if phase.capture() {
                                moves.set(moves.get() + 1);
                            }
                        });
                    },
                )
                .size_full(),
            )
    }
}

impl gpui_base::RootPlugin for OverlayCapture {
    fn decorate(
        &self,
        surface: AnyElement,
        _: &gpui_base::Root,
        _: &mut Window,
        _: &mut App,
    ) -> impl IntoElement {
        let downs = self.decorator_downs.clone();
        div()
            .size_full()
            .capture_any_mouse_down(move |_, _, _| downs.set(downs.get() + 1))
            .child(surface)
    }
}

/// A real Base Root retains component WindowState, earlier decorators and overlays, all inside the
/// final pointer presentation. Registering twice must not create two routers or translate twice.
#[gpui::test]
#[ignore = "native RootPlugin pointer regression; run explicitly with --ignored --test-threads=1"]
fn aligned_pointer_root_plugin_shields_component_overlays_and_keeps_base_root(
    cx: &mut TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        gpui_base::Root::register_plugin::<OverlayCapture>(cx, |_, _| OverlayCapture::default());
        init(cx);
        init(cx);
    });
    let (root, ui) = cx.add_window_view(|window, cx| {
        let content = cx.new(|cx| build_fixture(window, cx));
        gpui_base::Root::new(content, window, cx)
    });
    let (view, overlay) = ui.update(|window, cx| {
        let router =
            for_window(window, cx).expect("presentation belongs to the existing Base Root");
        let view = root.read(cx).view().clone().downcast::<Fixture>().unwrap();
        let overlay = root.read(cx).plugin::<OverlayCapture>().unwrap();
        view.update(cx, |state, cx| {
            state.regions = [router.register(), router.register()];
            state.router = router;
            state.wrap_directly = false;
            cx.notify();
        });
        assert!(window.root::<gpui_base::Root>().flatten().is_some());
        (view, overlay)
    });
    ui.simulate_resize(size(px(500.), px(300.)));
    draw(ui);
    let physical = source_point(&view, ui, 0, 6);
    ui.simulate_mouse_move(physical, None, Default::default());
    ui.simulate_click(physical, Default::default());
    ui.update(|window, cx| {
        let overlay = overlay.read(cx);
        assert_eq!(
            overlay.overlay_moves.get(),
            1,
            "Restore cannot reach another plugin's overlay"
        );
        assert_eq!(overlay.overlay_downs.get(), 1);
        assert_eq!(
            overlay.decorator_downs.get(),
            1,
            "physical and projected down must not both leak"
        );
        assert_eq!(view.read(cx).capture_moves.get(), 1);
        assert_eq!(view.read(cx).editors[0].read(cx).selected_range(), 6..6);
        assert_eq!(window.mouse_position(), physical);
        assert!(for_window(window, cx).is_some());
    });
}
