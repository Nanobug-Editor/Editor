//! Real native form input survives a scene handover without replaying business activations.
use super::*;
use gpui_base::Root;
use gpui_base::test_support::{ElementSnapshot, snapshots};
use gpui_kit::gpui;
use gpui_kit::{Role, TestAppContext};
use std::cell::RefCell;
use std::collections::VecDeque;

/// This is the transport boundary: the native adapter observes actual delivery receipts, while
/// the owning actor integration uses the same channel after public Manager::event returns.
type Mailbox = Rc<RefCell<VecDeque<(UiEvent, futures::channel::oneshot::Sender<FormReceipt>)>>>;

/// The ordinary host contract uses one field identity and explicit plugin-owned draft revisions.
fn scene(multiline: bool, revision: u64) -> Document {
    let input = ui::Input {
        placeholder: "草稿 / Draft".into(),
        ..Default::default()
    };
    let field = if multiline {
        ui::Node::textarea("field", input)
    } else {
        ui::Node::input("field", input)
    };
    Document::new(ui::Node::column("form", vec![field.height(88.)]).padding(12.)).revision(revision)
}

/// Locate Base's actual editable frame; no synthetic semantic node can satisfy the typing step.
fn field(window: &Window) -> ElementSnapshot {
    snapshots(window)
        .into_iter()
        .find(|node| node.role() == Some(Role::TextInput) && node.visible())
        .expect("the native form field must be painted")
}

/// A user can type while a newer scene is waiting for native projection. That retained draft must
/// be delivered through the new current scene once, rather than remaining visible only in Base.
#[gpui::test]
fn native_form_draft_crosses_scene_handover_without_losing_its_change(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        editor_ui::typography::init(cx);
        crate::ui::theme::apply_theme(crate::ui::theme::builtin_theme(false), cx);
    });
    for multiline in [false, true] {
        let events = Rc::new(RefCell::new(Vec::new()));
        let output = events.clone();
        let slot = Rc::new(RefCell::new(None));
        let capture = slot.clone();
        let (_, visual) = cx.add_window_view(move |window, cx| {
            let view = cx.new(|cx| {
                PluginView::new(
                    "ordinary-form".into(),
                    scene(multiline, 4),
                    Environment::default(),
                    move |event, _| output.borrow_mut().push(event),
                    window,
                    cx,
                )
            });
            *capture.borrow_mut() = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = slot.borrow_mut().take().unwrap();
        let position = visual.update(|window, cx| {
            window.draw(cx).clear(cx);
            field(window).bounds().center()
        });
        visual.simulate_click(position, Default::default());
        visual.run_until_parked();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
            // The stable single-line adapter does not publish focus/value snapshot metadata.
            // Verify its original Base state instead; pointer placement still comes from the
            // painted semantic frame and no direct state.focus() participates in this test.
            let focused = if multiline {
                assert_eq!(field(window).focused(), Some(true));
                view.read(cx).textareas["field"]
                    .state
                    .read(cx)
                    .presentation()
                    .focus_handle()
                    .is_focused(window)
            } else {
                view.read(cx).inputs["field"]
                    .state
                    .read(cx)
                    .presentation()
                    .focus_handle()
                    .is_focused(window)
            };
            assert!(
                focused,
                "pointer input must focus the original Base state; multiline={multiline}"
            );
            view.update(cx, |view, cx| view.set_scene_current(false, cx));
        });
        visual
            .update(|window, cx| editor_ui::testing::input_text(window, "尚未投递的中文草稿", cx));
        visual.run_until_parked();
        assert!(
            events.borrow().is_empty(),
            "an obsolete scene cannot forward a Change"
        );
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
            let value = if multiline {
                assert_eq!(field(window).value(), Some("尚未投递的中文草稿"));
                view.read(cx).textareas["field"]
                    .state
                    .read(cx)
                    .value()
                    .to_string()
            } else {
                view.read(cx).inputs["field"]
                    .state
                    .read(cx)
                    .value()
                    .to_string()
            };
            assert_eq!(value, "尚未投递的中文草稿");
            view.update(cx, |view, cx| {
                view.update_document(scene(multiline, 5), Environment::default(), window, cx);
                view.set_scene_current(true, cx);
            });
        });
        visual.run_until_parked();
        let changes = events
            .borrow()
            .iter()
            .filter(|event| {
                event.node == "field"
                    && event.revision == 5
                    && event.action == Action::Change("尚未投递的中文草稿".into())
            })
            .count();
        assert_eq!(
            changes, 1,
            "the current native field must publish its retained draft exactly once; multiline={multiline}"
        );
    }
}

fn init(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        editor_ui::typography::init(cx);
        crate::ui::theme::apply_theme(crate::ui::theme::builtin_theme(false), cx);
    });
}

/// An ordinary native form sends only Change through the injected receipt transport; no widget
/// state is replaced by this mailbox, and typing still uses the real focused platform handler.
fn reliable_form<'a>(
    cx: &'a mut TestAppContext,
    multiline: bool,
    mailbox: Mailbox,
) -> (Entity<PluginView>, &'a mut gpui_kit::VisualTestContext) {
    reliable_scene(cx, scene(multiline, 4), mailbox, Rc::default())
}

/// The ordinary EventSink remains separate from the reliable Change transport, as in a real
/// host surface. Tests observe native Click/Submit without substituting a business callback.
fn reliable_scene<'a>(
    cx: &'a mut TestAppContext,
    document: Document,
    mailbox: Mailbox,
    events: Rc<RefCell<Vec<UiEvent>>>,
) -> (Entity<PluginView>, &'a mut gpui_kit::VisualTestContext) {
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let sink: FormChangeSink = Rc::new(move |event, _| {
            let (reply, receipt) = futures::channel::oneshot::channel();
            mailbox.borrow_mut().push_back((event, reply));
            receipt
        });
        let view = cx.new(|cx| {
            PluginView::new(
                "ordinary-form".into(),
                document,
                Environment::default(),
                move |event, _| {
                    assert!(
                        !matches!(event.action, Action::Change(_)),
                        "reliable Change cannot bypass its receipt transport"
                    );
                    events.borrow_mut().push(event);
                },
                window,
                cx,
            )
            .with_form_changes(Some(sink))
        });
        *capture.borrow_mut() = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = slot.borrow_mut().take().unwrap();
    let position = visual.update(|window, cx| {
        window.draw(cx).clear(cx);
        field(window).bounds().center()
    });
    visual.simulate_click(position, Default::default());
    visual.run_until_parked();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    (view, visual)
}

fn replace_scene(
    view: &Entity<PluginView>,
    visual: &mut gpui_kit::VisualTestContext,
    document: Document,
) {
    visual.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.update_document(document, Environment::default(), window, cx)
        });
        window.draw(cx).clear(cx);
    });
    visual.run_until_parked();
}

/// A queued stale receipt retries the latest text at a newer current scene; successful delivery
/// stops even if the guest deliberately leaves its old value in later rendering acknowledgements.
#[gpui::test]
fn native_form_stale_receipt_retries_latest_draft_and_success_stops_echo_loop(
    cx: &mut TestAppContext,
) {
    init(cx);
    for multiline in [false, true] {
        let mailbox = Mailbox::default();
        let (view, visual) = reliable_form(cx, multiline, mailbox.clone());
        visual.update(|window, cx| editor_ui::testing::input_text(window, "草稿", cx));
        visual.run_until_parked();
        let (first, reply) = mailbox.borrow_mut().pop_front().unwrap();
        assert_eq!(first.revision, 4);
        assert_eq!(first.action, Action::Change("草稿".into()));
        visual.update(|window, cx| editor_ui::testing::input_text(window, "继续输入", cx));
        visual.run_until_parked();
        assert!(
            mailbox.borrow().is_empty(),
            "one field can have only one in-flight Change"
        );
        reply.send(FormReceipt::Stale).unwrap();
        visual.run_until_parked();
        assert!(
            mailbox.borrow().is_empty(),
            "a rejected scene cannot retry itself"
        );
        replace_scene(&view, visual, scene(multiline, 5));
        let (latest, reply) = mailbox.borrow_mut().pop_front().unwrap();
        assert_eq!(latest.revision, 5);
        assert_eq!(latest.action, Action::Change("草稿继续输入".into()));
        reply.send(FormReceipt::Delivered).unwrap();
        visual.run_until_parked();
        for revision in [6, 7] {
            replace_scene(&view, visual, scene(multiline, revision));
        }
        assert!(
            mailbox.borrow().is_empty(),
            "delivered text needs no guest-value echo and cannot form a publication loop"
        );
    }
}

/// Reset, disablement, node/type removal and source replacement revoke the old delivery token.
/// A late actor rejection cannot transfer that text to a replacement which reuses the node name.
#[gpui::test]
fn native_form_replacement_and_explicit_override_revoke_pending_delivery(cx: &mut TestAppContext) {
    init(cx);
    for multiline in [false, true] {
        for replacement in ["reset", "disabled", "removed", "type", "source"] {
            let mailbox = Mailbox::default();
            let (view, visual) = reliable_form(cx, multiline, mailbox.clone());
            visual.update(|window, cx| editor_ui::testing::input_text(window, "旧草稿", cx));
            visual.run_until_parked();
            let (_, reply) = mailbox.borrow_mut().pop_front().unwrap();
            let mut next = scene(multiline, 5);
            match replacement {
                "removed" => next.root = ui::Node::text("replacement", "No field"),
                "type" => next = scene(!multiline, 5),
                "source" => {
                    next.source = Some(plugin_runtime::plugin_protocol::api::DocumentVersion {
                        id: "new-source".into(),
                        path: "new-source.txt".into(),
                        revision: 1,
                    })
                }
                _ => {
                    let Kind::Column { children } = &mut next.root.kind else {
                        unreachable!()
                    };
                    if replacement == "disabled" {
                        children[0].disabled = true;
                    } else {
                        let input = match &mut children[0].kind {
                            Kind::Input(input) | Kind::Textarea(input) => input,
                            _ => unreachable!(),
                        };
                        input.value = "显式覆盖".into();
                        input.value_revision = 1;
                    }
                }
            }
            replace_scene(&view, visual, next);
            let _ = reply.send(FormReceipt::Stale);
            visual.run_until_parked();
            replace_scene(&view, visual, scene(multiline, 6));
            assert!(
                mailbox.borrow().is_empty(),
                "{replacement} must revoke the prior native draft; multiline={multiline}"
            );
        }
    }
}

/// Only stale scene rejection is retryable; retirement and real errors remain terminal outcomes.
#[gpui::test]
fn native_form_terminal_receipts_do_not_replay_into_later_scenes(cx: &mut TestAppContext) {
    init(cx);
    for outcome in [FormReceipt::Retired, FormReceipt::Rejected] {
        let mailbox = Mailbox::default();
        let (view, visual) = reliable_form(cx, true, mailbox.clone());
        visual.update(|window, cx| editor_ui::testing::input_text(window, "不会重投", cx));
        visual.run_until_parked();
        let (_, reply) = mailbox.borrow_mut().pop_front().unwrap();
        reply.send(outcome).unwrap();
        visual.run_until_parked();
        replace_scene(&view, visual, scene(true, 5));
        assert!(mailbox.borrow().is_empty());
    }
}

/// A business activation is never saved for a later scene; only a new native gesture may run
/// after the latest draft has entered the actor's FIFO ahead of that activation.
#[derive(Clone, Copy, Debug)]
enum Activation {
    Button,
    Enter,
}

/// Public scene composition keeps the same field identity both in ordinary content and modals.
fn action_scene(multiline: bool, revision: u64, modal: bool) -> Document {
    let mut document = scene(multiline, revision);
    let Kind::Column { children } = &mut document.root.kind else {
        unreachable!()
    };
    children.push(ui::Node::button("apply", "Apply latest draft"));
    if modal {
        let content = std::mem::replace(&mut document.root, ui::Node::text("outside", "Form"));
        document.dialog = Some(ui::Dialog::new("draft-dialog", "Draft", content));
    }
    document
}

/// Native activations target the painted local button rather than a guest node or direct emit.
fn action_button(window: &Window) -> ElementSnapshot {
    snapshots(window)
        .into_iter()
        .find(|node| {
            node.role() == Some(Role::Button)
                && node.label() == Some("Apply latest draft")
                && node.visible()
        })
        .expect("the ordinary native submit button must be painted")
}

/// Dispatch through the actual local button or the focused Base input's Enter binding.
fn activate_form(visual: &mut gpui_kit::VisualTestContext, activation: Activation) {
    match activation {
        Activation::Button => {
            let position = visual.update(|window, cx| {
                window.draw(cx).clear(cx);
                action_button(window).bounds().center()
            });
            visual.simulate_click(position, Default::default());
        }
        Activation::Enter => visual.simulate_keystrokes("enter"),
    }
    visual.run_until_parked();
}

/// Real typing retains its Base focus/value while one older Change waits for an actor receipt.
/// The ordinary action must not reach a guest that has only received the preceding value.
fn assert_latest_draft_precedes_activation(
    cx: &mut TestAppContext,
    multiline: bool,
    activation: Activation,
) {
    let mailbox = Mailbox::default();
    let events = Rc::new(RefCell::new(Vec::new()));
    let document = action_scene(multiline, 4, false);
    let (view, visual) = reliable_scene(cx, document, mailbox.clone(), events.clone());
    visual.update(|window, cx| editor_ui::testing::input_text(window, "A", cx));
    visual.run_until_parked();
    let (first, reply) = mailbox.borrow_mut().pop_front().unwrap();
    assert_eq!(first.action, Action::Change("A".into()));

    // A is already enqueued: an outstanding receipt alone must not discard the next native
    // activation, because the production Worker observes the two messages in this FIFO order.
    activate_form(visual, activation);
    let accepted = events.borrow_mut().pop().unwrap();
    assert_eq!(
        accepted.action,
        match activation {
            Activation::Button => Action::Click,
            Activation::Enter => Action::Submit("A".into()),
        }
    );
    let position = visual.update(|window, cx| {
        window.draw(cx).clear(cx);
        field(window).bounds().center()
    });
    visual.simulate_click(position, Default::default());
    visual.run_until_parked();
    visual.update(|window, cx| {
        let value = if multiline {
            let state = view.read(cx).textareas["field"].state.read(cx);
            assert!(state.presentation().focus_handle().is_focused(window));
            state.value().to_string()
        } else {
            let state = view.read(cx).inputs["field"].state.read(cx);
            assert!(state.presentation().focus_handle().is_focused(window));
            state.value().to_string()
        };
        assert_eq!(value, "A");
    });
    visual.update(|window, cx| editor_ui::testing::input_text(window, "B", cx));
    visual.run_until_parked();
    assert!(
        mailbox.borrow().is_empty(),
        "only one Change may be in flight"
    );
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
        let native = view.read(cx);
        let value = if multiline {
            assert!(native.textareas["field"].state.read(cx).is_editable());
            native.textareas["field"].state.read(cx).value().to_string()
        } else {
            assert!(native.inputs["field"].state.read(cx).is_editable());
            native.inputs["field"].state.read(cx).value().to_string()
        };
        assert_eq!(value, "AB", "the user has edited the original native field");
    });
    activate_form(visual, activation);
    assert!(
        events.borrow().is_empty(),
        "{activation:?} must not overtake the latest unsent AB draft; multiline={multiline}"
    );

    reply.send(FormReceipt::Delivered).unwrap();
    visual.run_until_parked();
    let (latest, reply) = mailbox.borrow_mut().pop_front().unwrap();
    assert_eq!(latest.action, Action::Change("AB".into()));
    assert!(
        events.borrow().is_empty(),
        "the blocked activation cannot replay"
    );
    activate_form(visual, activation);
    assert_eq!(events.borrow().len(), 1);
    assert_eq!(
        events.borrow()[0].action,
        match activation {
            Activation::Button => Action::Click,
            Activation::Enter => Action::Submit("AB".into()),
        },
        "a new native activation follows the already enqueued latest Change"
    );
    reply.send(FormReceipt::Delivered).unwrap();
    visual.run_until_parked();
    assert!(mailbox.borrow().is_empty());
    assert_eq!(
        events.borrow().len(),
        1,
        "no deferred business activation exists"
    );
}

/// Clicking an ordinary button cannot submit A while the visible native Input/Textarea shows AB.
#[gpui::test]
fn native_form_click_waits_for_unsent_latest_change_without_replaying_the_click(
    cx: &mut TestAppContext,
) {
    init(cx);
    for multiline in [false, true] {
        assert_latest_draft_precedes_activation(cx, multiline, Activation::Button);
    }
}

/// Enter uses the original single-line Base control and must obey the same Change/action ordering.
#[gpui::test]
fn native_form_submit_waits_for_unsent_latest_change_without_replaying_enter(
    cx: &mut TestAppContext,
) {
    init(cx);
    assert_latest_draft_precedes_activation(cx, false, Activation::Enter);
}

/// A latest value sent for an older scene can still receive Stale. Fresh Click/Submit must wait
/// for its current-scene retry, while native modal dismissal remains available throughout.
#[gpui::test]
fn native_form_handover_blocks_actions_until_current_change_is_queued_but_keeps_dismiss(
    cx: &mut TestAppContext,
) {
    init(cx);
    for activation in [Activation::Button, Activation::Enter] {
        let mailbox = Mailbox::default();
        let events = Rc::new(RefCell::new(Vec::new()));
        let (view, visual) = reliable_scene(
            cx,
            action_scene(false, 4, true),
            mailbox.clone(),
            events.clone(),
        );
        visual.update(|window, cx| editor_ui::testing::input_text(window, "新场景草稿", cx));
        visual.run_until_parked();
        let (first, reply) = mailbox.borrow_mut().pop_front().unwrap();
        assert_eq!(first.revision, 4);
        replace_scene(&view, visual, action_scene(false, 5, true));
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
            assert!(view.read(cx).inputs["field"].state.read(cx).is_editable());
            assert_eq!(
                view.read(cx).inputs["field"].state.read(cx).value(),
                "新场景草稿"
            );
        });
        activate_form(visual, activation);
        assert!(
            events.borrow().is_empty(),
            "an old-scene flight is not current admission"
        );
        visual.simulate_keystrokes("escape");
        visual.run_until_parked();
        let dismissed = events
            .borrow_mut()
            .pop()
            .expect("native Escape must remain usable");
        assert_eq!(dismissed.node, "draft-dialog");
        assert_eq!(dismissed.action, Action::Dismiss);

        reply.send(FormReceipt::Stale).unwrap();
        visual.run_until_parked();
        let (latest, reply) = mailbox.borrow_mut().pop_front().unwrap();
        assert_eq!(latest.revision, 5);
        assert_eq!(latest.action, Action::Change("新场景草稿".into()));
        assert!(
            events.borrow().is_empty(),
            "no blocked activation may replay on retry"
        );
        let position = visual.update(|window, cx| {
            window.draw(cx).clear(cx);
            field(window).bounds().center()
        });
        visual.simulate_click(position, Default::default());
        visual.run_until_parked();
        activate_form(visual, activation);
        assert_eq!(events.borrow().len(), 1);
        assert_eq!(
            events.borrow()[0].action,
            match activation {
                Activation::Button => Action::Click,
                Activation::Enter => Action::Submit("新场景草稿".into()),
            }
        );
        reply.send(FormReceipt::Delivered).unwrap();
        visual.run_until_parked();
        assert!(mailbox.borrow().is_empty());
        assert_eq!(events.borrow().len(), 1);
    }
}
