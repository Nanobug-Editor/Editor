//! Product-owned source frame over the original Base EditorState and native input element.
use super::{context_menu::NativeMenu, editor::synchronize_editor_appearance};
use crate::theme::ActiveTheme as _;
use crate::theme::component_styles;
use gpui_base::input::EditorState;
use gpui_base::{InputBase, RoleOverride, StyledExt as _};
use gpui_kit::{
    AccessibleAction, AnyElement, App, ElementId, Entity, Focusable as _, InteractiveElement as _,
    IntoElement, ParentElement, RenderOnce, Role, SharedString, StatefulInteractiveElement as _,
    StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, relative,
};
use plugin_schema::ThemeComponent;
use std::rc::Rc;
mod access;
mod actions;
mod snapshot;
#[cfg(test)]
mod tests;
mod tools;
pub use access::ReadOnlyLease;
pub use actions::{EditorActionElements, EditorActionProjection};
pub use snapshot::Snapshot;

/// Only the frame and overlay presentation live here; text, IME, selections and Undo stay in Base.
#[derive(IntoElement)]
pub struct Editor {
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
    /// Stable native identity for plugin literals and work-window mounts.
    id: Option<SharedString>,
    /// Readonly accessibility name; native bytes remain in the original state.
    label: Option<String>,
    /// Measured display-only composition; it supplies the single physical input/accessibility node.
    body: Option<AnyElement>,
    /// Source tools are optional for literal and work-window presentation.
    tools: bool,
    /// Optional edge controls and their projection use the same original native source.
    actions: Option<(EditorActionElements, Option<EditorActionProjection>)>,
    /// Admit host document shortcuts only for an authorized borrowed-document mount.
    document_commands: bool,
}
impl Editor {
    /// Mount `state` without creating another text buffer, history stack or language provider.
    pub fn new(state: &Entity<EditorState>) -> Self {
        Self {
            state: state.clone(),
            style: StyleRefinement::default(),
            role: RoleOverride::default(),
            readonly: None,
            disabled: None,
            menu: None,
            id: None,
            label: None,
            body: None,
            tools: true,
            actions: None,
            document_commands: false,
        }
    }
    /// Override the source accessibility role while retaining the original input authority.
    pub fn role(mut self, role: impl Into<RoleOverride>) -> Self {
        self.role = role.into();
        self
    }
    /// Set a stable surface identity and optional accessible name for an existing source.
    pub fn identity(mut self, id: impl Into<SharedString>, label: Option<String>) -> Self {
        self.id = Some(id.into());
        self.label = label;
        self
    }
    /// Compose a measured source body without registering a second physical input node.
    pub fn presentation(mut self, body: Option<AnyElement>) -> Self {
        self.body = body;
        self
    }
    /// Enable native search/code-action presentation; ordinary host sources enable it by default.
    pub fn tools(mut self, enabled: bool) -> Self {
        self.tools = enabled;
        self
    }
    /// Give a host-owned document mount its command context; literal snapshots leave this disabled.
    pub fn document_commands(mut self, enabled: bool) -> Self {
        self.document_commands = enabled;
        self
    }
    /// Attach approved line-edge controls; geometry, clipping and accessibility remain native.
    pub fn edge_controls(
        mut self,
        actions: EditorActionElements,
        projection: Option<EditorActionProjection>,
    ) -> Self {
        self.actions = Some((actions, projection));
        self
    }
    /// Apply the document owner's current edit restriction.
    pub fn readonly(mut self, readonly: bool) -> Self {
        self.readonly = Some(readonly);
        self
    }
    /// Apply the document owner's current enabled state.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = Some(disabled);
        self
    }
    /// The source frame border follows the caller's explicit presentation choice.
    pub fn bordered(mut self, bordered: bool) -> Self {
        if bordered {
            self.style = self.style.border_1();
        }
        self
    }
    /// Supply actions for Base's deferred right-click selection and context-menu callback.
    pub fn context_menu(
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
        let id: ElementId = self
            .id
            .clone()
            .map(Into::into)
            .unwrap_or_else(|| ("local-source", self.state.entity_id()).into());
        let focus = self.state.read(cx).focus_handle(cx);
        let accessible_id = self.id.clone().unwrap_or_else(|| "local-source".into());
        let readonly = self.state.read(cx).presentation().is_readonly();
        let appearance = component_styles(cx, ThemeComponent::Editor).base;
        let content = if let Some(body) = self.body {
            // The measured pane already owns accessibility and native input; this is placement only.
            div()
                .id(id)
                .flex()
                .size_full()
                .min_h_0()
                .min_w_0()
                .debug_selector({
                    let name = accessible_id.clone();
                    move || name.to_string()
                })
                .when(self.document_commands, |frame| {
                    frame.key_context("NativeEditorSource")
                })
                .bg(appearance.background.unwrap_or(cx.theme().background))
                .text_color(appearance.foreground.unwrap_or(cx.theme().foreground))
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(
                    appearance
                        .font_size_px
                        .map(gpui_kit::px)
                        .unwrap_or(cx.theme().mono_font_size),
                )
                .refine_style(&self.style)
                .child(body)
                .into_any_element()
        } else {
            let tools = self.tools.then(|| {
                let state = self.state.clone();
                window.use_keyed_state(
                    ("local-source-tools", self.state.entity_id()),
                    cx,
                    move |window, cx| tools::EditorTools::new(state, window, cx),
                )
            });
            let named = self.label.is_some();
            let value = (named
                && (window.is_a11y_active() || cfg!(any(test, feature = "test-support"))))
            .then(|| self.state.read(cx).value().to_string());
            let target = self.state.clone();
            let selector = self.id;
            InputBase::new(id.clone())
                .role(if readonly && matches!(self.role, RoleOverride::Implicit) {
                    Role::Document.into()
                } else {
                    self.role
                })
                .when(self.document_commands, |frame| {
                    frame.key_context("NativeEditorSource")
                })
                .debug_selector(move || {
                    selector
                        .as_ref()
                        .map_or_else(|| "local-source".to_owned(), ToString::to_string)
                })
                .when(named, |frame| frame.accessibility_id(accessible_id))
                .when_some(self.label, |frame, label| frame.accessibility_label(label))
                .when_some(value, |frame, value| frame.aria_value(value))
                .on_a11y_action(AccessibleAction::Focus, move |_, window, cx| {
                    target.update(cx, |state, cx| state.focus(window, cx));
                })
                .track_focus(&focus)
                .focused(focus.is_focused(window))
                .relative()
                .size_full()
                .min_h_0()
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(
                    appearance
                        .font_size_px
                        .map(gpui_kit::px)
                        .unwrap_or(cx.theme().mono_font_size),
                )
                .line_height(relative(1.5))
                .bg(appearance.background.unwrap_or(cx.theme().background))
                .text_color(appearance.foreground.unwrap_or(cx.theme().foreground))
                .refine_style(&self.style)
                .child(super::row_backgrounds::native_editor(&self.state, cx))
                .children(tools)
                .into_any_element()
        };
        if let Some((actions, projection)) = self.actions {
            actions::with_projected_editor_actions(content, &self.state, actions, projection)
        } else {
            content
        }
    }
}
