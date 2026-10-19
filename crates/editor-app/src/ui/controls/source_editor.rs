//! Product-owned source frame over the original Base EditorState and native input element.
use super::{context_menu::NativeMenu, editor::synchronize_editor_appearance};
use crate::ui::theme::ActiveTheme as _;
use gpui_base::input::EditorState;
use gpui_base::{InputBase, RoleOverride, StyledExt as _};
use gpui_kit::{
    App, Entity, Focusable as _, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
    StyleRefinement, Styled, Window, relative,
};
use std::rc::Rc;
#[cfg(test)]
mod tests;
mod tools;

/// Only the frame and overlay presentation live here; text, IME, selections and Undo stay in Base.
#[derive(IntoElement)]
pub(crate) struct Editor {
    /// Existing document session input authority.
    state: Entity<EditorState>,
    /// Caller visual refinements applied after product defaults.
    style: StyleRefinement,
    /// Accessibility role override for projected source mounts.
    role: RoleOverride,
    /// Optional read-only intent; absent preserves the current presentation restriction.
    readonly: Option<bool>,
    /// Optional disabled intent; absent preserves the current presentation restriction.
    disabled: Option<bool>,
    /// Local action menu definition; projected source frames replace only its physical anchor.
    menu: Option<Rc<dyn Fn(NativeMenu, &mut Window, &mut App) -> NativeMenu>>,
}
impl Editor {
    /// Mount `state` without creating another text buffer, history stack or language provider.
    pub(crate) fn new(state: &Entity<EditorState>) -> Self {
        Self {
            state: state.clone(),
            style: StyleRefinement::default(),
            role: RoleOverride::default(),
            readonly: None,
            disabled: None,
            menu: None,
        }
    }
    /// Override the source accessibility role while retaining the original input authority.
    pub(crate) fn role(mut self, role: impl Into<RoleOverride>) -> Self {
        self.role = role.into();
        self
    }
    /// Apply the document owner's current edit restriction.
    pub(crate) fn readonly(mut self, readonly: bool) -> Self {
        self.readonly = Some(readonly);
        self
    }
    /// Apply the document owner's current enabled state.
    pub(crate) fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = Some(disabled);
        self
    }
    /// The source frame border follows the caller's explicit presentation choice.
    pub(crate) fn bordered(mut self, bordered: bool) -> Self {
        if bordered {
            self.style = self.style.border_1();
        }
        self
    }
    /// Supply actions for Base's deferred right-click selection and context-menu callback.
    pub(crate) fn context_menu(
        mut self,
        menu: impl Fn(NativeMenu, &mut Window, &mut App) -> NativeMenu + 'static,
    ) -> Self {
        self.menu = Some(Rc::new(menu));
        self
    }
}
impl Styled for Editor {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}
impl RenderOnce for Editor {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        synchronize_editor_appearance(&self.state, cx);
        self.state.update(cx, |state, cx| {
            if let Some(readonly) = self.readonly {
                state.set_readonly(readonly, cx);
            }
            if let Some(disabled) = self.disabled {
                state.set_disabled(disabled, cx);
            }
            if let Some(menu) = self.menu {
                state.on_context_menu(Rc::new(move |_, _, point, window, cx| {
                    menu(NativeMenu::new(), window, cx).show(point, window, cx)
                }));
            }
        });
        let state = self.state.clone();
        let tools = window.use_keyed_state(
            ("local-source-tools", self.state.entity_id()),
            cx,
            move |window, cx| tools::EditorTools::new(state, window, cx),
        );
        InputBase::new(("local-source", self.state.entity_id()))
            .role(self.role)
            .track_focus(&self.state.read(cx).focus_handle(cx))
            .relative()
            .size_full()
            .min_h_0()
            .font_family(cx.theme().mono_font_family.clone())
            .text_size(cx.theme().mono_font_size)
            .line_height(relative(1.5))
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .refine_style(&self.style)
            .child(gpui_base::input::Editor::new(&self.state))
            .child(tools)
    }
}
