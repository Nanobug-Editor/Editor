//! Project native semantics through Base's editing state, preserving its one focus/Tab registration.
use gpui_base::input::{InputAccessibility, InputState};
use gpui_kit::{
    App, Entity, EntityId, IntoElement, Render, Role, SharedString, View, ViewElement, Window,
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

    /// Give Base the native `label` and role while its original state owns live value and focus.
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.state.update(cx, |state, cx| {
            // The public projection survives Base's observed-element wrapper and lets the
            // existing native root enforce disabled/read-only/masked accessibility behavior.
            state.set_accessibility(
                InputAccessibility::new(Some(Role::TextInput)).with_label(self.label),
            );
            Render::render(state, window, cx).into_any_element()
        })
    }
}
