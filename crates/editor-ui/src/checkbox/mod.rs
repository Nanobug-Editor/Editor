//! Checkbox appearance stays local while gpui-base handles toggle and focus.

use crate::theme::ActiveTheme as _;
use gpui_base::{Checkbox as BaseCheckbox, CheckboxState};
use gpui_kit::{
    App, ClickEvent, ElementId, IntoElement, ParentElement, RenderOnce, SharedString, Styled,
    Window, div, prelude::FluentBuilder as _, px,
};

/// Local controlled checkbox presentation over Base activation and accessibility semantics.
#[derive(IntoElement)]
pub struct Checkbox {
    base: BaseCheckbox,
    label: SharedString,
    checked: bool,
    /// A partially selected group draws a dash and activates to checked.
    indeterminate: bool,
    /// Marker-only controls keep an accessible name without repeating adjacent row text.
    marker_only: bool,
    disabled: bool,
}

impl Checkbox {
    /// Create an unchecked control with stable id; its owner supplies checked on subsequent renders.
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            base: BaseCheckbox::new(id),
            label: SharedString::default(),
            checked: false,
            indeterminate: false,
            marker_only: false,
            disabled: false,
        }
    }

    /// Set the visible and accessible label.
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    /// Project the caller-owned checked value into Base and the local check mark.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self.indeterminate = false;
        self
    }

    /// Draw a mixed state when `indeterminate` is true; the next activation requests checked.
    pub fn indeterminate(mut self, indeterminate: bool) -> Self {
        self.indeterminate = indeterminate;
        self
    }

    /// Hide the visible label while retaining it for assistive technology.
    pub fn marker_only(mut self) -> Self {
        self.marker_only = true;
        self
    }

    /// Project disabled state into both native behavior and local appearance.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Deliver the next checked value, original click event and App to handler; the owner commits it.
    pub fn on_change(mut self, handler: impl Fn(&bool, &ClickEvent, &mut App) + 'static) -> Self {
        self.base = self.base.on_change(move |state, event, _, cx| {
            let checked = state == CheckboxState::Checked;
            handler(&checked, event, cx);
        });
        self
    }
}

impl RenderOnce for Checkbox {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = cx.theme();
        let mark = div()
            .size(px(15.))
            .rounded(px(3.))
            .border_1()
            .border_color(if self.checked || self.indeterminate {
                palette.primary
            } else {
                palette.border
            })
            .bg(if self.checked || self.indeterminate {
                palette.primary
            } else {
                palette.background
            })
            .text_color(palette.primary_foreground)
            .text_size(px(12.))
            .flex()
            .items_center()
            .justify_center()
            .when(self.checked || self.indeterminate, |this| {
                this.child(if self.indeterminate { "−" } else { "✓" })
            });
        self.base
            .checked(self.checked)
            .indeterminate(self.indeterminate)
            .disabled(self.disabled)
            .accessibility_label(self.label.clone())
            .flex()
            .items_center()
            .gap_2()
            .text_color(palette.foreground)
            .text_size(px(13.))
            .when(self.disabled, |this| this.opacity(0.5))
            .child(mark)
            .when(!self.marker_only, |this| this.child(self.label))
    }
}
