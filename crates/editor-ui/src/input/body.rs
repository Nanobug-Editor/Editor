//! Attach native semantics to Base's actual editing root, preserving its one focus/Tab registration.
use gpui_base::{TestSupportExt as _, input::InputState};
use gpui_kit::{
    App, Div, Entity, EntityId, Focusable as _, InteractiveElement as _, IntoElement, Render, Role,
    SharedString, Stateful, StatefulInteractiveElement as _, View, ViewElement, Window, div,
};

/// A reactive presentation of the original state; no additional entity, buffer or Undo stack exists.
pub(crate) struct NativeInputBody {
    /// The caller-owned Base editing state also supplies this view's reactive identity.
    state: Entity<InputState>,
    /// Optional explicit native name; omission preserves an unnamed ordinary input.
    label: Option<SharedString>,
}

impl NativeInputBody {
    /// Mount `state` under its existing reactive identity with the optional native `label`.
    pub(crate) fn new(state: Entity<InputState>, label: Option<SharedString>) -> ViewElement<Self> {
        ViewElement::new(Self { state, label })
    }
}

impl View for NativeInputBody {
    /// Preserve Base's original reactive rendering identity for caret, selection and IME updates.
    fn entity_id(&self) -> Option<EntityId> {
        Some(self.state.entity_id())
    }

    /// Render Base unchanged, then decorate its locked public native root with accessibility metadata.
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let value = self.state.read(cx).value();
        let focus = self.state.read(cx).focus_handle(cx);
        let endpoint = self.state.downgrade();
        let mut native = self.state.update(cx, |state, cx| {
            Render::render(state, window, cx).into_any_element()
        });
        // Base 0.8 renders one Stateful<Div>. The public type-erased downcast keeps all original
        // key handlers, layout, painting, selection and IME on that root; dependency upgrades
        // must re-run the native input/Tab/accessibility acceptance before changing this adapter.
        let Some(root) = native.downcast_mut::<Stateful<Div>>() else {
            tracing::error!("locked Base input root cannot publish native accessibility metadata");
            return native;
        };
        let root = std::mem::replace(root, div().id("native-input-replacement"));
        let mut root = root.role(Role::TextInput).aria_value(value).on_a11y_action(
            gpui_kit::AccessibleAction::Focus,
            move |_, window, cx| {
                let _ = endpoint.update(cx, |state, cx| {
                    if !state.presentation().is_disabled() {
                        state.focus(window, cx);
                    }
                });
            },
        );
        if let Some(label) = self.label {
            root = root.aria_label(label);
        }
        // Observe the existing root's focus binding on the same element, without another stop.
        root.test_support().track_focus(&focus).into_any_element()
    }
}
