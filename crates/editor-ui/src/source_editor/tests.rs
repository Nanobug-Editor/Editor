//! Real Base input events verify the migrated local source frame and its retained document owner.
use super::*;
use gpui_kit::{
    AppContext as _, Context, Render, TestAppContext, VisualTestContext, div, gpui, px, size,
};
use std::{cell::RefCell, rc::Rc};

mod resizing;

/// A literal source exercises optional rails through exactly the default host Editor interface.
struct SnapshotHost {
    /// Sole readonly source; configuration changes must retain this native entity.
    snapshot: Snapshot,
    /// Observable result of a real native edge-button activation.
    clicks: Rc<std::cell::Cell<usize>>,
    /// Enable the optional rails independently of whether they currently contain controls.
    rails: bool,
    /// Withdraw controls while retaining the declared rail width.
    show_button: bool,
}
impl Render for SnapshotHost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let action = crate::Button::new("snapshot-action")
            .label("Review")
            .debug_selector(|| "snapshot-action".into())
            .on_click(cx.listener(|this, _, _, _| this.clicks.set(this.clicks.get() + 1)))
            .into_any_element();
        let mut editor = Editor::new(&self.snapshot.editor)
            .identity("snapshot-source", Some("Readonly source".into()))
            .tools(false);
        if self.rails {
            let controls = if self.show_button {
                vec![(0, action)]
            } else {
                Vec::new()
            };
            editor = editor.edge_controls(EditorActionElements::new(controls, Vec::new()), None);
        }
        div().size_full().child(editor)
    }
}

/// Literal input remains immutable while native copying, selection and configured rails work.
#[gpui::test]
fn configurable_editor_snapshot_copies_and_routes_native_edges(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::typography::init(cx);
        crate::theme::apply_theme(crate::theme::builtin_theme(false), cx);
    });
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let clicks = Rc::new(std::cell::Cell::new(0));
    let counter = clicks.clone();
    let (_, visual) = cx.add_window_view(|window, cx| {
        let snapshot = Snapshot::new("source 中文😀\n".into(), Vec::new(), window, cx);
        snapshot
            .editor
            .update(cx, |state, cx| state.focus(window, cx));
        let editor = snapshot.editor.clone();
        let host = cx.new(|_| SnapshotHost {
            snapshot,
            clicks: counter,
            rails: true,
            show_button: true,
        });
        capture.replace(Some((editor, host.clone())));
        gpui_base::Root::new(host, window, cx)
    });
    visual.simulate_resize(size(px(800.), px(500.)));
    visual.update(|window, cx| window.draw(cx).clear(cx));
    let (editor, host) = slot.borrow_mut().take().unwrap();
    visual.simulate_input("blocked");
    visual.simulate_keystrokes("ctrl-a ctrl-c");
    visual.run_until_parked();
    visual.update(|_, cx| {
        assert_eq!(editor.read(cx).text().to_string(), "source 中文😀\n");
        assert_eq!(
            cx.read_from_clipboard().and_then(|item| item.text()),
            Some("source 中文😀\n".into())
        );
    });
    let button = visual
        .debug_bounds("snapshot-action")
        .expect("configured native edge is painted");
    visual.simulate_click(button.center(), Default::default());
    visual.run_until_parked();
    assert_eq!(clicks.get(), 1);
    let original_width = visual.debug_bounds("snapshot-source").unwrap().size.width;
    visual.update(|_, cx| {
        host.update(cx, |host, cx| {
            host.show_button = false;
            cx.notify();
        })
    });
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(
        visual.debug_bounds("snapshot-source").unwrap().size.width,
        original_width
    );
    visual.update(|_, cx| {
        host.update(cx, |host, cx| {
            host.rails = false;
            cx.notify();
        })
    });
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
    assert!(visual.debug_bounds("snapshot-source").unwrap().size.width > original_width);
}

/// Readonly configuration borrows input policy without replacing the document or its Undo history.
#[gpui::test]
fn configurable_editor_readonly_mount_restores_owner_after_last_release(cx: &mut TestAppContext) {
    let (editor, visual) = source(cx);
    let original = visual.update(|_, cx| editor.read(cx).text().to_string());
    let first = visual.update(|_, cx| ReadOnlyLease::new(&editor, cx));
    let second = visual.update(|_, cx| ReadOnlyLease::new(&editor, cx));
    visual.simulate_input("blocked");
    visual.run_until_parked();
    visual.update(|_, cx| assert_eq!(editor.read(cx).text().to_string(), original));
    drop(first);
    visual.run_until_parked();
    visual.update(|_, cx| assert!(!editor.read(cx).is_editable()));
    drop(second);
    visual.run_until_parked();
    visual.simulate_input("allowed");
    visual.run_until_parked();
    visual.update(|_, cx| assert_ne!(editor.read(cx).text().to_string(), original));
    visual.simulate_keystrokes("ctrl-z");
    visual.run_until_parked();
    visual.update(|_, cx| assert_eq!(editor.read(cx).text().to_string(), original));
    visual.update(|_, cx| editor.update(cx, |state, cx| state.set_readonly(true, cx)));
    let readonly_owner = visual.update(|_, cx| ReadOnlyLease::new(&editor, cx));
    drop(readonly_owner);
    visual.run_until_parked();
    visual.update(|_, cx| assert!(!editor.read(cx).is_editable()));
}

/// A source-only host exercises the same local frame used by the editor application.
struct SourceHost {
    /// Sole source editing and Undo authority mounted by this fixture.
    editor: Entity<EditorState>,
}
impl Render for SourceHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(Editor::new(&self.editor))
    }
}
/// Construct the product palette and source frame with one original document state.
fn source(cx: &mut TestAppContext) -> (Entity<EditorState>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::typography::init(cx);
        crate::theme::apply_theme(crate::theme::builtin_theme(false), cx);
    });
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, visual) = cx.add_window_view(|window, cx| {
        let editor = cx.new(|cx| {
            EditorState::new(window, cx)
                .default_value("alpha 中文 alpha")
                .searchable(true)
                .replaceable(true)
        });
        editor.update(cx, |editor, cx| editor.focus(window, cx));
        capture.replace(Some(editor.clone()));
        let host = cx.new(|_| SourceHost { editor });
        gpui_base::Root::new(host, window, cx)
    });
    visual.simulate_resize(size(px(800.), px(500.)));
    visual.update(|window, cx| window.draw(cx).clear(cx));
    (slot.borrow_mut().take().unwrap(), visual)
}

/// Search drafts accept real Unicode input and Escape restores the original source focus.
#[gpui::test]
fn local_source_find_uses_base_query_and_restores_source_focus(cx: &mut TestAppContext) {
    let (editor, visual) = source(cx);
    visual.simulate_keystrokes("ctrl-f");
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
    let search = visual.debug_bounds("local-editor-search").unwrap();
    visual.update(|window, _| {
        assert!(search.top() >= px(0.) && search.bottom() <= window.viewport_size().height)
    });
    visual.simulate_input("中文");
    visual.run_until_parked();
    visual.update(|_, cx| {
        assert_eq!(editor.read(cx).search_session().query, "中文");
        assert_eq!(editor.read(cx).text().to_string(), "alpha 中文 alpha");
    });
    visual.update(|window, _| {
        let fields = gpui_base::test_support::snapshots(window);
        assert!(
            fields
                .iter()
                .any(|field| field.role() == Some(gpui_kit::Role::TextInput)
                    && field.value() == Some("中文")
                    && field.focused() == Some(true))
        );
        assert!(
            fields
                .iter()
                .any(|field| field.role() == Some(gpui_kit::Role::Button)
                    && field.label() == Some(rust_i18n::t!("source.close").as_ref()))
        );
    });
    visual.simulate_keystrokes("escape");
    visual.run_until_parked();
    visual.update(|window, cx| {
        assert!(!editor.read(cx).search_session().open);
        assert!(editor.read(cx).focus_handle(cx).is_focused(window));
    });
}

/// Replacements enter through Base's existing edit path and are reversible with the source Undo.
#[gpui::test]
fn local_source_replace_keeps_the_original_undo_owner(cx: &mut TestAppContext) {
    let (editor, visual) = source(cx);
    visual.simulate_keystrokes("ctrl-h");
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
    visual.simulate_input("中文");
    visual.run_until_parked();
    let replacement = visual.debug_bounds("local-find-replacement").unwrap();
    visual.simulate_click(replacement.center(), Default::default());
    visual.simulate_input("replacement");
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
    visual.update(|_, cx| {
        let state = editor.read(cx);
        assert!(state.is_replaceable());
        assert_eq!(
            state.search_session().query,
            "中文",
            "replacement input must have its own focus"
        );
        assert_eq!(state.search_session().matcher.len(), 1);
    });
    let replace = visual.debug_bounds("local-replace-current").unwrap();
    visual.simulate_click(replace.center(), Default::default());
    visual.run_until_parked();
    visual.update(|_, cx| {
        assert_eq!(
            editor.read(cx).text().to_string(),
            "alpha replacement alpha"
        )
    });
    visual.simulate_keystrokes("escape");
    visual.run_until_parked();
    visual.simulate_keystrokes("ctrl-z");
    visual.run_until_parked();
    visual.update(|_, cx| assert_eq!(editor.read(cx).text().to_string(), "alpha 中文 alpha"));
}

/// Compile both resources so new local controls never ship untranslated resource keys.
#[test]
fn local_source_controls_have_both_locales() {
    assert_eq!(rust_i18n::t!("source.find", locale = "en"), "Find");
    assert_eq!(rust_i18n::t!("source.find", locale = "zh-CN"), "查找");
}

/// Code-action selection and dismissal remain reachable from the source keyboard dispatch scope.
#[gpui::test]
fn local_source_code_actions_keep_base_keyboard_dispatch(cx: &mut TestAppContext) {
    let (editor, visual) = source(cx);
    visual.update(|_, cx| {
        editor.update(cx, |editor, cx| {
            editor.present_code_actions(
                vec![
                    gpui_base::input::CodeActionItem {
                        provider_id: "test".into(),
                        action: lsp_types::CodeAction {
                            title: "First action".into(),
                            ..Default::default()
                        },
                    },
                    gpui_base::input::CodeActionItem {
                        provider_id: "test".into(),
                        action: lsp_types::CodeAction {
                            title: "Second action".into(),
                            ..Default::default()
                        },
                    },
                ],
                cx,
            )
        })
    });
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
    visual.simulate_keystrokes("down");
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
    visual.update(|window, _| {
        assert!(
            gpui_base::test_support::snapshots(window)
                .iter()
                .any(|row| row.role() == Some(gpui_kit::Role::MenuItem)
                    && row.label() == Some("Second action")
                    && row.selected() == Some(true))
        )
    });
    visual.simulate_keystrokes("escape");
    visual.run_until_parked();
    visual.update(|_, cx| {
        assert!(!editor.read(cx).code_action_menu_state().open);
        assert_eq!(editor.read(cx).text().to_string(), "alpha 中文 alpha");
    });
}
