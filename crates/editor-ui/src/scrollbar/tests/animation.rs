//! Observe native thumb geometry through idle, reversal, hover and captured dragging.

use super::*;
use gpui_base::input::{EditorState, TextareaState};
use gpui_kit::{Entity, ScrollDelta, ScrollEvent, VisualTestContext};

/// Configure a short public idle hold, retaining the production width transition and colors.
fn init(cx: &mut TestAppContext, dark: bool) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::typography::init(cx);
        crate::theme::apply_theme(crate::theme::builtin_theme(dark), cx);
    });
}

/// Start motion after startup palette work; shortening idle makes real-time geometry checks bounded.
fn configure(ui: &mut VisualTestContext) {
    ui.update(|window, cx| {
        let theme = gpui_base::Theme::global(cx).scrollbar;
        let motion = theme.motion().with_idle(Duration::from_millis(35));
        gpui_base::Theme::global_mut(cx).scrollbar = theme.with_motion(motion);
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

/// Force one finite native frame after `wait`; no test reimplements the interpolation.
fn frame(ui: &mut VisualTestContext, wait: u64) {
    std::thread::sleep(Duration::from_millis(wait));
    ui.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

/// Read the scrollbar's native colored quad inside `viewport`; zero-area paint counts as hidden.
fn thumb(
    ui: &mut VisualTestContext,
    viewport: Bounds<Pixels>,
    horizontal: bool,
) -> Option<(f32, f32, Point<Pixels>)> {
    ui.update(|window, cx| {
        let scale = window.scale_factor();
        let styles = component_styles(cx, ThemeComponent::Scrollbar);
        let normal = styles
            .base
            .background
            .unwrap_or(cx.theme().primary)
            .opacity(0.55);
        let hover = styles
            .hover
            .background
            .unwrap_or(cx.theme().primary)
            .opacity(0.7);
        let active = styles
            .active
            .background
            .unwrap_or(cx.theme().primary)
            .opacity(0.8);
        window.painted_quads().into_iter().find_map(|quad| {
            let color = quad.background.as_solid()?;
            if color != normal && color != hover && color != active {
                return None;
            }
            let visible = quad.bounds.intersect(&quad.content_mask.bounds);
            let bounds = Bounds::new(
                point(px(visible.left().0 / scale), px(visible.top().0 / scale)),
                size(
                    px(visible.size.width.0 / scale),
                    px(visible.size.height.0 / scale),
                ),
            );
            let width = f32::from(if horizontal {
                bounds.size.height
            } else {
                bounds.size.width
            });
            let length = if horizontal {
                bounds.size.width
            } else {
                bounds.size.height
            };
            (width > 0. && width <= 8.1 && length >= px(20.) && viewport.contains(&bounds.center()))
                .then_some((width, color.a, bounds.center()))
        })
    })
}

/// Local vertical/horizontal rails contract without fading or moving content, then reverse smoothly.
#[gpui::test]
fn scrollbar_idle_contracts_width_to_zero_and_reverses_without_fading(cx: &mut TestAppContext) {
    for dark in [false, true] {
        init(cx, dark);
        for horizontal in [false, true] {
            let viewport = Bounds::new(point(px(0.), px(0.)), size(px(100.), px(200.)));
            let handle = Handle {
                offset: Rc::new(Cell::new(point(px(-100.), px(-100.)))),
                viewport,
            };
            let mounted = handle.clone();
            let (_, ui) = cx.add_window_view(move |_, _| Harness {
                handle: mounted,
                local: true,
                horizontal,
                padded: false,
            });
            configure(ui);
            let initial =
                thumb(ui, viewport, horizontal).expect("a new scroll viewport reveals its rail");
            assert_eq!(initial.0, 6.);
            let before = handle.offset();
            frame(ui, 50);
            frame(ui, 65);
            let midway =
                thumb(ui, viewport, horizontal).expect("contraction must have intermediate pixels");
            assert!(
                midway.0 > 0. && midway.0 < initial.0,
                "width must contract: {midway:?}"
            );
            assert_eq!(midway.1, initial.1, "hiding changes thickness, not opacity");
            frame(ui, 150);
            assert!(
                thumb(ui, viewport, horizontal).is_none(),
                "the width must reach zero"
            );
            assert_eq!(
                handle.offset(),
                before,
                "visibility must never move content"
            );
            // The track away from the thumb must also reveal a fully hidden rail.
            let rail = if horizontal {
                point(px(90.), px(195.))
            } else {
                point(px(95.), px(180.))
            };
            ui.simulate_mouse_move(rail, None, Default::default());
            frame(ui, 0);
            frame(ui, 210);
            assert!(
                thumb(ui, viewport, horizontal).is_some(),
                "hovering any track segment must reveal it"
            );
            frame(ui, 80);
            assert!(
                thumb(ui, viewport, horizontal).is_some(),
                "hover must outlast the short idle hold"
            );
            // Exiting the native window leaves its last pointer position on the rail.
            ui.simulate_event(gpui_kit::MouseExitEvent {
                position: rail,
                pressed_button: None,
                modifiers: Default::default(),
            });
            frame(ui, 0);
            frame(ui, 50);
            frame(ui, 210);
            assert!(thumb(ui, viewport, horizontal).is_none());
            ui.simulate_mouse_move(point(px(10.), px(10.)), None, Default::default());
            handle.set_offset(point(px(-200.), px(-200.)));
            // Extend the public hold for the reveal/reversal/drag checks.
            ui.update(|_, cx| {
                let theme = gpui_base::Theme::global(cx).scrollbar;
                let motion = theme.motion().with_idle(Duration::from_secs(2));
                gpui_base::Theme::global_mut(cx).scrollbar = theme.with_motion(motion);
            });
            frame(ui, 0);
            frame(ui, 65);
            let returning = thumb(ui, viewport, horizontal).unwrap();
            assert!(returning.0 > 0. && returning.0 < 6.);
            assert_eq!(returning.1, initial.1);
            frame(ui, 150);
            let shown = thumb(ui, viewport, horizontal).unwrap();
            assert_eq!(shown.0, 6.);
            ui.simulate_mouse_move(shown.2, None, Default::default());
            frame(ui, 210);
            assert_eq!(thumb(ui, viewport, horizontal).unwrap().0, 8.);
            ui.simulate_mouse_down(shown.2, MouseButton::Left, Default::default());
            let destination = shown.2
                + if horizontal {
                    point(px(20.), px(0.))
                } else {
                    point(px(0.), px(30.))
                };
            ui.simulate_mouse_move(destination, Some(MouseButton::Left), Default::default());
            frame(ui, 210);
            assert_eq!(thumb(ui, viewport, horizontal).unwrap().0, 8.);
            assert_ne!(
                handle.offset(),
                point(px(-200.), px(-200.)),
                "Base's real drag must move the original handle"
            );
            ui.simulate_mouse_up(destination, MouseButton::Left, Default::default());
        }
    }
}

/// Reduced-motion users get the same idle policy without any width-transition frame requests.
#[gpui::test]
fn scrollbar_reduced_motion_hides_without_animation_frames(cx: &mut TestAppContext) {
    init(cx, false);
    let viewport = Bounds::new(point(px(0.), px(0.)), size(px(100.), px(200.)));
    let handle = Handle {
        offset: Rc::new(Cell::new(point(px(0.), px(-100.)))),
        viewport,
    };
    let (_, ui) = cx.add_window_view(move |_, _| Harness {
        handle,
        local: true,
        horizontal: false,
        padded: false,
    });
    configure(ui);
    ui.update(|_, cx| cx.set_reduce_motion(true));
    frame(ui, 50);
    assert!(thumb(ui, viewport, false).is_none());
    ui.update(|window, cx| assert_eq!(window.simulate_next_frame(cx), 0));
}

/// Real native source/textarea inputs expose their own internal bars through the same public skin.
struct Inputs {
    /// EditorState remains the only source text, scroll and selection owner.
    editor: Entity<EditorState>,
    /// Ordinary form state is independent of the document editor.
    textarea: Entity<TextareaState>,
    /// An unwrapped Base sibling witnesses that scoped native painting restored the shared skin.
    witness: Handle,
}

impl Render for Inputs {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .relative()
            .w(px(750.))
            .h(px(180.))
            .flex()
            .child(
                div()
                    .relative()
                    .w(px(250.))
                    .h_full()
                    .child(native_editor(&self.editor)),
            )
            .child(
                div()
                    .relative()
                    .w(px(250.))
                    .h_full()
                    .child(native_textarea(&self.textarea)),
            )
            .child(
                div()
                    .relative()
                    .w(px(250.))
                    .h_full()
                    .child(gpui_base::Scrollbar::vertical(&self.witness)),
            )
    }
}

/// Native bars must shrink individually and leave the global theme and both text authorities intact.
#[gpui::test]
fn scrollbar_native_inputs_contract_independently_and_restore_global_theme(
    cx: &mut TestAppContext,
) {
    init(cx, false);
    let text = (0..100)
        .map(|i| format!("第{i}行 中文😀\n"))
        .collect::<String>();
    let original = text.clone();
    let (view, ui) = cx.add_window_view(move |window, cx| {
        let editor = cx.new(|cx| EditorState::new(window, cx).default_value(text.clone()));
        let textarea = cx.new(|cx| TextareaState::new(window, cx).rows(8).default_value(text));
        Inputs {
            editor,
            textarea,
            witness: Handle {
                offset: Rc::new(Cell::new(point(px(0.), px(-50.)))),
                viewport: Bounds::new(point(px(500.), px(0.)), size(px(250.), px(180.))),
            },
        }
    });
    configure(ui);
    let left = Bounds::new(point(px(0.), px(0.)), size(px(250.), px(180.)));
    let right = Bounds::new(point(px(250.), px(0.)), size(px(250.), px(180.)));
    assert_eq!(thumb(ui, left, false).unwrap().0, 6.);
    frame(ui, 50);
    frame(ui, 65);
    assert!(thumb(ui, left, false).unwrap().0 < 6.);
    frame(ui, 150);
    assert!(thumb(ui, left, false).is_none());
    assert!(thumb(ui, right, false).is_none());
    let witness = Bounds::new(point(px(500.), px(0.)), size(px(250.), px(180.)));
    assert_eq!(
        thumb(ui, witness, false).unwrap().0,
        6.,
        "a hidden child must not lend its palette to a native sibling"
    );
    // A real wheel reveals only its source and preserves both buffers and selection authority.
    ui.simulate_event(ScrollEvent {
        // Wheel and trackpad fixtures retain pointer-device semantics.
        source: gpui_kit::ScrollSource::Pointer,
        position: point(px(100.), px(70.)),
        delta: ScrollDelta::Pixels(point(px(0.), px(-60.))),
        ..Default::default()
    });
    frame(ui, 0);
    frame(ui, 20);
    assert!(thumb(ui, left, false).is_some());
    assert!(thumb(ui, right, false).is_none());
    ui.update(|_, cx| {
        let state = view.read(cx);
        assert_eq!(state.editor.read(cx).value(), original);
        assert_eq!(state.textarea.read(cx).value(), original);
        assert!(state.editor.read(cx).scroll_offset().y < px(0.));
        assert_eq!(state.textarea.read(cx).scroll_offset().y, px(0.));
    });
}
