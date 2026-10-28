//! Shared focus/hover reveal for native panel title buttons.

use gpui_base::{h_flex, motion};
use gpui_kit::{
    Context, Div, InteractiveElement as _, Stateful, Styled as _, Window,
    prelude::FluentBuilder as _, px,
};
use std::time::Duration;

/// Reserve `width` logical pixels for `id` and build its buttons while they are visible.
/// `active` means pointer or focus is inside the panel (including an owned open menu).
/// Base owns the reversible 140 ms transition and the system's reduced-motion preference;
/// `build` receives the row only above zero opacity, removing hidden input/accessibility targets.
/// `window` and the owning `Context<T>` retain transition state and supply the button callbacks.
pub fn title_actions<T: 'static>(
    id: &'static str,
    width: f32,
    active: bool,
    window: &mut Window,
    cx: &mut Context<T>,
    build: impl FnOnce(Stateful<Div>, &mut Context<T>) -> Stateful<Div>,
) -> Stateful<Div> {
    let opacity = motion::transition(
        format!("{id}-opacity"),
        if active { 1. } else { 0. },
        motion::Transition::new(Duration::from_millis(140)),
        window,
        cx,
    );
    h_flex()
        .id(id)
        .debug_selector(move || id.into())
        .items_center()
        .flex_shrink_0()
        .w(px(width))
        .h(px(24.))
        .opacity(opacity)
        .when(opacity > 0., |row| build(row, cx))
}
