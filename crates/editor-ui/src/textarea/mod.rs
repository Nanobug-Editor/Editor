//! Multi-line form appearance; gpui-base retains selection, IME, undo, scrolling and keyboard input.
use crate::theme::ActiveTheme as _;
use gpui_base::input::TextareaState;
use gpui_kit::{
    App, Entity, InteractiveElement as _, IntoElement, ParentElement, Pixels, RenderOnce, Role,
    StatefulInteractiveElement as _, Styled, Window, div, prelude::FluentBuilder as _, px, rems,
};

/// A compact ordinary-text field, distinct from both a single-line input and a document editor.
#[derive(IntoElement)]
pub struct Textarea {
    state: Entity<TextareaState>,
    /// Explicit form viewport; omitted controls retain their native auto-grow layout.
    height: Option<Pixels>,
    /// Uniform form projection, independent of OS DPI and caller-owned editing state.
    scale: f32,
}
impl Textarea {
    /// The caller owns editing state for the form lifetime; painting never resets its value.
    pub fn new(state: &Entity<TextareaState>) -> Self {
        Self {
            state: state.clone(),
            height: None,
            scale: 1.,
        }
    }

    /// Bound the complete editing viewport, including its border and padding, in logical pixels.
    /// Callers should use a bounded native row policy instead of auto-growing beyond this viewport.
    pub fn height(mut self, height: Pixels) -> Self {
        self.height = Some(height.max(px(0.)));
        self
    }

    /// Scale default padding, border and typography by a finite positive factor; height stays explicit.
    pub fn scale(mut self, scale: f32) -> Self {
        debug_assert!(
            scale.is_finite() && scale > 0.,
            "textarea scale must be finite and positive"
        );
        self.scale = scale;
        self
    }
}
impl RenderOnce for Textarea {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = cx.theme();
        let presentation = self.state.read(cx).presentation();
        let focus = presentation.focus_handle();
        // Accessibility mirrors the original state only when observed; it never owns editable text.
        // Host tests consume this as a dependency, so their snapshots use the explicit feature.
        let value = (window.is_a11y_active() || cfg!(any(test, feature = "test-support")))
            .then(|| self.state.read(cx).value().to_string());
        div()
            .w_full()
            .when(self.height.is_none(), |view| {
                view.min_h(px(52. * self.scale))
            })
            .when_some(self.height, |view, height| view.h(height).overflow_hidden())
            .px(rems(0.5 * self.scale))
            .py(rems(0.25 * self.scale))
            .rounded(palette.radius * self.scale)
            .border(px(self.scale))
            .border_color(palette.input)
            .bg(palette.background)
            .text_color(palette.foreground)
            // Multiline fields scale with the same global typography as their labels and inputs.
            .text_size(rems(0.875 * self.scale))
            .child(
                gpui_base::input::InputBase::new(("local-textarea", self.state.entity_id()))
                    .w_full()
                    .min_w_0()
                    .when(self.height.is_some(), |frame| frame.h_full())
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
                    // Intrinsic rows size ordinary fields; only an explicitly bounded viewport
                    // propagates percentage height to the original native input engine.
                    .child(super::scroll::smooth_textarea(
                        &self.state,
                        super::scrollbar::native_textarea(&self.state),
                    )),
            )
    }
}

#[cfg(test)]
mod tests;
