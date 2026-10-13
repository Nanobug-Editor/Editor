//! Local popover appearance; Base owns open state, positioning, dismissal and focus restoration.
use super::Button;
use gpui_base::PopoverState;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    AnyElement, App, Focusable, InteractiveElement, IntoElement, ParentElement, RenderOnce, Role,
    StatefulInteractiveElement, Styled, Window, div, px,
};

#[derive(IntoElement)]
struct Popover {
    id: &'static str,
    trigger: Button,
    content: AnyElement,
}

/// A non-modal panel below a leading input button, sharing Base's pointer and keyboard activation.
/// Content is rebuilt by its owner; closing restores focus and releases the deferred-popup lease.
pub(crate) fn popover(id: &'static str, trigger: Button, content: AnyElement) -> impl IntoElement {
    Popover {
        id,
        trigger,
        content,
    }
}

impl RenderOnce for Popover {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id, cx, |_, cx| PopoverState::new(false, cx));
        let open = state.read(cx).is_open();
        let parent = window.current_view();
        let focus = state.read(cx).focus_handle(cx);
        let trigger_focus = window.use_keyed_state((self.id, 1_u32), cx, |_, cx| cx.focus_handle());
        let trigger_focus = trigger_focus.read(cx).clone();
        let toggle = state.clone();
        let trigger = self
            .trigger
            .expanded(open)
            .track_focus(&trigger_focus)
            .on_click(move |_, window, cx| {
                // Capture the painted state: an outside-pointer dismissal must not reopen this click.
                toggle.update(cx, |state, cx| {
                    if open {
                        state.dismiss(window, cx);
                    } else {
                        // Pointer activation must capture the same trigger focus restored for keyboard reopening.
                        trigger_focus.focus(window, cx);
                        state.show(window, cx);
                    }
                });
                cx.notify(parent);
            });
        let popup = gpui_base::Popup::new(self.id, trigger);
        if !open {
            return popup.into_any_element();
        }
        let dismiss = state.clone();
        let palette = cx.theme();
        let card = div()
            .id(self.id)
            .debug_selector(move || self.id.into())
            .role(Role::Dialog)
            .tab_group()
            .track_focus(&focus)
            .key_context("Popover")
            // Include Input's half-rem padding and one-pixel border when aligning its outer edge.
            .ml(-(window.rem_size() * 0.5) - px(1.))
            .mt(px(28.))
            .w(px(232.))
            .max_h((window.viewport_size().height - px(64.)).min(px(360.)))
            .overflow_y_scroll()
            .p_3()
            .rounded(palette.radius)
            .border_1()
            .border_color(palette.border)
            .bg(palette.popover)
            .text_color(palette.popover_foreground)
            .shadow_md()
            .on_action(move |_: &gpui_base::actions::Cancel, window, cx| {
                state.update(cx, |state, cx| state.dismiss(window, cx));
                cx.stop_propagation();
                cx.notify(parent);
            })
            .on_mouse_down_out(move |_, window, cx| {
                dismiss.update(cx, |state, cx| state.dismiss(window, cx));
                cx.notify(parent);
            })
            .child(self.content);
        popup.content(card).into_any_element()
    }
}
