//! Single-line input appearance around gpui-base's shared editing state.

use crate::theme::ActiveTheme as _;
pub(crate) mod body;
use gpui_base::StyledExt as _;
use gpui_base::input::InputState;
use gpui_kit::{
    AnyElement, App, Entity, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, px, rems,
};

/// Local single-line frame over one caller-owned Base InputState, including its IME and Undo.
#[derive(IntoElement)]
pub struct Input {
    state: Entity<InputState>,
    style: StyleRefinement,
    appearance: bool,
    bordered: bool,
    focus_bordered: bool,
    /// Uniform scale of control-owned defaults; explicit Styled refinements retain precedence.
    scale: f32,
    /// The native input publishes its semantic name independently of the placeholder text.
    accessibility_label: Option<SharedString>,
    /// Optional leading control shares this input's border; editing and IME remain owned by Base.
    prefix: Option<AnyElement>,
}

impl Input {
    /// Mount the original state without creating another text buffer, focus handle or Undo history.
    pub fn new(state: &Entity<InputState>) -> Self {
        Self {
            state: state.clone(),
            style: StyleRefinement::default(),
            appearance: true,
            bordered: true,
            focus_bordered: true,
            scale: 1.,
            accessibility_label: None,
            prefix: None,
        }
    }

    /// Choose whether to apply the local input background and default visual frame.
    pub fn appearance(mut self, appearance: bool) -> Self {
        self.appearance = appearance;
        self
    }
    /// Scale the frame's default geometry and text by finite positive `scale`, excluding OS DPI.
    /// The editing state, focus identity, IME and Undo are retained; explicit styles still win.
    pub fn scale(mut self, scale: f32) -> Self {
        debug_assert!(
            scale.is_finite() && scale > 0.,
            "input scale must be finite and positive"
        );
        self.scale = scale;
        self
    }
    /// Expose an explicit native accessibility label without changing shared editing behavior.
    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }

    /// Insert a local control before the editable area without adding another input state or frame.
    pub fn prefix(mut self, prefix: impl IntoElement) -> Self {
        self.prefix = Some(prefix.into_any_element());
        self
    }

    /// Choose whether to paint the local resting border.
    pub fn bordered(mut self, bordered: bool) -> Self {
        self.bordered = bordered;
        self
    }

    /// Choose whether focus paints the local accent border; Base still owns focus.
    pub fn focus_bordered(mut self, focus_bordered: bool) -> Self {
        self.focus_bordered = focus_bordered;
        self
    }
}

impl Styled for Input {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Input {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = cx.theme();
        div()
            .w_full()
            // Base's single-line engine fills its frame. A minimum height alone leaves its
            // percentage-height hit region at zero, so painted fields cannot receive pointer/IME input.
            .h(px(28. * self.scale))
            .min_h(px(28. * self.scale))
            .flex()
            .items_center()
            .text_color(palette.foreground)
            // Rem-based text follows the single global typography size, including form zoom.
            .text_sm()
            .when(self.scale != 1., |this| {
                this.text_size(rems(0.875 * self.scale))
            })
            .when(self.appearance, |this| {
                this.px(rems(0.5 * self.scale))
                    .rounded(palette.radius * self.scale)
                    .bg(palette.background)
                    .when(self.bordered, |this| {
                        this.border(px(self.scale)).border_color(palette.input)
                    })
            })
            .when(self.focus_bordered && self.appearance, |this| {
                this.in_focus(|style| style.border_color(palette.ring))
            })
            .when_some(self.prefix, |this, prefix| {
                this.child(div().flex_shrink_0().child(prefix))
            })
            .child(
                gpui_base::input::InputBase::new(("local-input", self.state.entity_id()))
                    // The original editing element publishes its role and single focus/Tab stop.
                    .role(gpui_base::RoleOverride::Presentational)
                    .flex_1()
                    .min_w(px(0.))
                    .h_full()
                    .child(body::NativeInputBody::new(
                        self.state.clone(),
                        self.accessibility_label,
                    )),
            )
            .refine_style(&self.style)
    }
}
