//! Multi-line form appearance; gpui-base retains selection, IME, undo, scrolling and keyboard input.
use gpui_base::input::{Textarea as BaseTextarea, TextareaState};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{
    App, Entity, InteractiveElement as _, IntoElement, ParentElement, RenderOnce, Role,
    StatefulInteractiveElement as _, Styled, Window, div, prelude::FluentBuilder as _, px,
};

/// A compact ordinary-text field, distinct from both a single-line input and a document editor.
#[derive(IntoElement)]
pub(crate) struct Textarea {
    state: Entity<TextareaState>,
}
impl Textarea {
    /// The caller owns editing state for the form lifetime; painting never resets its value.
    pub(crate) fn new(state: &Entity<TextareaState>) -> Self {
        Self {
            state: state.clone(),
        }
    }
}
impl RenderOnce for Textarea {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = cx.theme();
        let presentation = self.state.read(cx).presentation();
        let focus = presentation.focus_handle();
        // Accessibility mirrors the original state only when observed; it never owns editable text.
        let value = (window.is_a11y_active() || cfg!(test))
            .then(|| self.state.read(cx).value().to_string());
        div()
            .w_full()
            .min_h(px(52.))
            .px_2()
            .py_1()
            .rounded(palette.radius)
            .border_1()
            .border_color(palette.input)
            .bg(palette.background)
            .text_color(palette.foreground)
            // Multiline fields scale with the same global typography as their labels and inputs.
            .text_sm()
            .child(
                gpui_base::input::InputBase::new(("local-textarea", self.state.entity_id()))
                    .w_full()
                    .min_w_0()
                    .track_focus(focus)
                    .focused(focus.is_focused(window))
                    .disabled(presentation.is_disabled())
                    .role(if presentation.is_readonly() {
                        Role::Document
                    } else {
                        Role::TextInput
                    })
                    .accessibility_label(presentation.placeholder().clone())
                    .when_some(value, |frame, value| frame.aria_value(value))
                    // Base retains auto-grow's intrinsic row height. A percentage-height frame
                    // here would lose hit geometry inside an otherwise auto-sized form.
                    .child(BaseTextarea::new(&self.state)),
            )
    }
}

#[cfg(test)]
mod tests;
