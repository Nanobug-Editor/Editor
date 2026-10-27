//! Keyed multiline state uses gpui-base's ordinary editing engine, separate from document editors.
use super::*;
use gpui_base::input::TextareaState;

pub(super) struct NativeTextarea {
    pub state: Entity<TextareaState>,
    value_revision: u64,
    last_value: String,
    /// Native visual-row policy survives receipts without replacing the editing state.
    rows: (usize, usize),
    _subscription: Subscription,
}

impl PluginView {
    /// Retain native state across acknowledgements and release controls removed from this document.
    pub(super) fn sync_textareas(
        &mut self,
        nodes: &[plugin_runtime::plugin_protocol::ui::Node],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.textareas.retain(|id, _| {
            nodes
                .iter()
                .any(|node| node.id == *id && matches!(node.kind, Kind::Textarea(_)))
        });
        for node in nodes {
            let Kind::Textarea(input) = &node.kind else {
                continue;
            };
            if !self.textareas.contains_key(&node.id) {
                let rows = textarea_rows(node);
                let state = cx.new(|cx| {
                    TextareaState::new(window, cx)
                        .default_value(input.value.clone())
                        .placeholder(input.placeholder.clone())
                        .auto_grow(rows.0, rows.1)
                });
                let id = node.id.clone();
                let subscription = cx.subscribe_in(
                    &state,
                    window,
                    move |this, state, event: &InputEvent, window, cx| {
                        if !matches!(event, InputEvent::Change) {
                            return;
                        }
                        let value = state.read(cx).value().to_string();
                        let Some(entry) = this.textareas.get_mut(&id) else {
                            return;
                        };
                        if entry.last_value == value {
                            return;
                        }
                        entry.last_value = value.clone();
                        let revision = entry.value_revision;
                        this.form_changed(&id, state.entity_id(), revision, value, window, cx);
                    },
                );
                self.textareas.insert(
                    node.id.clone(),
                    NativeTextarea {
                        state,
                        value_revision: input.value_revision,
                        last_value: input.value.clone(),
                        rows,
                        _subscription: subscription,
                    },
                );
            }
            let active = self.document.active_node(&node.id).is_some()
                && (self.scene_current.get() || !self.overlays.scopes.contains_key(&node.id));
            let entry = self.textareas.get_mut(&node.id).unwrap();
            let rows = textarea_rows(node);
            let changed_rows = entry.rows != rows;
            entry.rows = rows;
            let replace = entry.value_revision != input.value_revision;
            if replace {
                self.forms.cancel(&node.id);
                entry.value_revision = input.value_revision;
                entry.last_value = input.value.clone();
            }
            entry.state.update(cx, |state, cx| {
                if changed_rows {
                    state.set_auto_grow(rows.0, rows.1, cx);
                }
                state.set_disabled(!active, cx);
                state.set_placeholder(input.placeholder.clone(), window, cx);
                if replace {
                    state.set_value(input.value.clone(), window, cx);
                }
            });
        }
    }
}

/// Resolve a validated field's row range; fixed pixel viewports retain a single native row.
fn textarea_rows(node: &ui::Node) -> (usize, usize) {
    if node.layout.height.is_some() {
        (1, 1)
    } else {
        node.textarea_rows.map_or((3, 8), |rows| {
            (usize::from(rows.min), usize::from(rows.max))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_base::{Root, test_support::snapshots};
    use gpui_kit::{ElementInputHandler, InputHandler as _, Role, TestAppContext, gpui, px};

    /// Row policy changes preserve the original editor, focus and typed draft across scene receipts.
    #[gpui::test]
    fn native_textarea_row_policy_preserves_draft_and_state(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            editor_ui::typography::init(cx);
            crate::ui::theme::apply_theme(crate::ui::theme::builtin_theme(false), cx);
        });
        let mut retained = None;
        let (_, visual) = cx.add_window_view(|window, cx| {
            let document = Document::new(
                ui::Node::textarea("draft", ui::Input::default())
                    .textarea_rows(2, 4)
                    .width(240.),
            )
            .revision(1);
            let view = cx.new(|cx| {
                PluginView::new(
                    "form-provider".into(),
                    document,
                    Environment::default(),
                    |_, _| {},
                    window,
                    cx,
                )
            });
            retained = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = retained.unwrap();
        let (at, before) = visual.update(|window, cx| {
            window.draw(cx).clear(cx);
            let field = snapshots(window)
                .into_iter()
                .find(|field| field.role() == Some(Role::TextInput))
                .unwrap();
            (field.bounds().center(), field.bounds().size.height)
        });
        visual.simulate_click(at, Default::default());
        visual.run_until_parked();
        visual.update(|window, cx| {
            editor_ui::testing::input_text(window, "中文\nsecond\nthird\nfourth\nfifth", cx)
        });
        visual.run_until_parked();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
            let state = view.read(cx).textareas["draft"].state.clone();
            let id = state.entity_id();
            let field = snapshots(window)
                .into_iter()
                .find(|field| field.role() == Some(Role::TextInput))
                .unwrap();
            assert!(field.bounds().size.height > before + px(10.));
            // Exercise the same UTF-16 composition bridge as Windows IME before a scene echo.
            let mut handler = ElementInputHandler::new(field.bounds(), state.clone());
            handler.replace_and_mark_text_in_range(None, "拼😀", Some(1..3), window, cx);
            let composing = state.read(cx).value().to_string();
            assert!(handler.marked_text_range(window, cx).is_some());
            let document = Document::new(
                ui::Node::textarea("draft", ui::Input::default())
                    .textarea_rows(2, 3)
                    .width(240.),
            )
            .revision(2);
            view.update(cx, |view, cx| {
                view.update_document(document, Environment::default(), window, cx)
            });
            assert_eq!(view.read(cx).textareas["draft"].state.entity_id(), id);
            assert_eq!(state.read(cx).value().as_ref(), composing);
            assert!(handler.marked_text_range(window, cx).is_some());
            handler.replace_text_in_range(None, "拼音", window, cx);
            assert!(handler.marked_text_range(window, cx).is_none());
            let committed = state.read(cx).value().to_string();
            editor_ui::testing::press_key(window, "ctrl-z", cx);
            assert_ne!(state.read(cx).value().as_ref(), committed);
            editor_ui::testing::press_key(window, "ctrl-y", cx);
            assert_eq!(state.read(cx).value().as_ref(), committed);
            assert!(
                state
                    .read(cx)
                    .presentation()
                    .focus_handle()
                    .is_focused(window)
            );
        });
    }
}
