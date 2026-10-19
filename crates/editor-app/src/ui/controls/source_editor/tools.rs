//! Local find/replace and code-action presentation; every effect returns to the original Base editor.
use crate::ui::{
    controls::{Button, Icon, Input},
    theme::ActiveTheme as _,
};
use gpui_base::input::{EditorState, InputEvent, InputOverlayKind, InputState};
use gpui_kit::{
    AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement, Render,
    StatefulInteractiveElement as _, Styled, Subscription, Window, div,
    prelude::FluentBuilder as _, px,
};

/// Persistent UI inputs are search drafts, never a second editable document or Undo owner.
pub(super) struct EditorTools {
    /// The source session receiving search navigation, replacements and code actions.
    editor: Entity<EditorState>,
    /// Base input for the current find query, including IME support.
    query: Entity<InputState>,
    /// Base input for the replacement draft.
    replacement: Entity<InputState>,
    /// Opening search again focuses/selects the query without overwriting user input every frame.
    activation: Option<u64>,
    /// Current code-action keyboard selection, reset when the menu content changes.
    selected: usize,
    /// Last action-menu revision used for selection synchronization.
    actions_revision: u64,
    /// Keyboard navigation reveals the selected row without moving the source document viewport.
    actions_scroll: gpui_kit::ScrollHandle,
    /// Observations keep this presentation in sync with the sole document owner.
    _subscriptions: Vec<Subscription>,
}
impl EditorTools {
    /// Allocate Base search inputs and observe the source; source text is never copied here.
    pub(super) fn new(
        editor: Entity<EditorState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let query = cx.new(|cx| InputState::new(window, cx));
        let replacement = cx.new(|cx| InputState::new(window, cx));
        let subscriptions = vec![
            cx.observe(&editor, |_, _, cx| cx.notify()),
            cx.subscribe(&query, |this: &mut Self, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    let query = input.read(cx).value().to_string();
                    let insensitive = this.editor.read(cx).search_session().case_insensitive;
                    this.editor.update(cx, |state, cx| {
                        state.set_search_query(query, insensitive, cx)
                    });
                }
            }),
        ];
        Self {
            editor,
            query,
            replacement,
            activation: None,
            selected: 0,
            actions_revision: 0,
            actions_scroll: gpui_kit::ScrollHandle::new(),
            _subscriptions: subscriptions,
        }
    }
}

mod actions;
mod search;

impl Render for EditorTools {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let search = self.render_search(window, cx);
        let actions = self.render_actions(window, cx);
        // Overlay geometry shares the source frame's origin; normal flow would place this
        // view after the full-height native editor and leave search inputs outside the window.
        div()
            .absolute()
            .inset_0()
            .children(search)
            .children(actions)
    }
}
