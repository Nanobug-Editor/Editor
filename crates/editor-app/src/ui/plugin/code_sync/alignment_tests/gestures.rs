//! A held native rail press cannot migrate to a replacement or withdrawn source proposal.
use super::*;

/// Replace only the public edge content; the comparison's original editor and source stay intact.
fn action_document(version: &DocumentVersion, action: ui::Node) -> Document {
    let mut document = default_document(version);
    let Kind::Row { children } = &mut document.root.kind else {
        unreachable!("the fixed comparison fixture is a row");
    };
    let Kind::EditorWithActions { left, .. } = &mut children[1].kind else {
        unreachable!("the fixed middle pane borrows the original native source");
    };
    left[0].content = Box::new(action);
    document
}

#[gpui::test]
#[ignore = "native source-action press ownership; run explicitly with --ignored --test-threads=1"]
fn native_code_alignment_held_action_cannot_activate_a_replacement_or_withdrawn_scene(
    cx: &mut TestAppContext,
) {
    let (mounted, ui) = mount(cx);
    let old = rail(ui, false);
    mounted.events.borrow_mut().clear();
    ui.simulate_mouse_down(old.center(), MouseButton::Left, Default::default());
    let focus = ui.update(|window, cx| window.focused(cx));
    ui.update(|window, cx| {
        mounted.view.update(cx, |view, cx| {
            // Keep the source and its alignment group unchanged: an unrelated remeasurement
            // must not be the mechanism that happens to revoke a pressed source action.
            view.update_document(
                mounted.document.clone().revision(2),
                Environment::default(),
                window,
                cx,
            );
            view.set_scene_current(true, cx);
        });
    });
    settle(ui);
    let next = rail(ui, false);
    assert_eq!(old, next, "replacing a proposal retains its painted rail");
    ui.simulate_mouse_up(next.center(), MouseButton::Left, Default::default());
    settle(ui);
    assert!(
        mounted.events.borrow().is_empty(),
        "releasing an old press cannot activate the replacement proposal"
    );
    assert_eq!(ui.update(|window, cx| window.focused(cx)), focus);
    ui.simulate_click(next.center(), Default::default());
    settle(ui);
    assert_eq!(mounted.events.borrow().len(), 1);
    assert_eq!(mounted.events.borrow()[0].revision, 2);
    mounted.events.borrow_mut().clear();

    ui.simulate_mouse_down(next.center(), MouseButton::Left, Default::default());
    ui.update(|_, cx| {
        mounted.view.update(cx, |view, cx| {
            // Withdrawal and reappearance may reuse the same public source and UI revision.
            // It still ends the native press, including when no hidden frame was painted.
            view.set_scene_current(false, cx);
            view.set_scene_current(true, cx);
        });
    });
    settle(ui);
    let resumed = rail(ui, false);
    ui.simulate_mouse_up(resumed.center(), MouseButton::Left, Default::default());
    settle(ui);
    assert!(
        mounted.events.borrow().is_empty(),
        "withdrawal cannot lend its pending MouseDown to the resumed surface"
    );
    ui.simulate_click(resumed.center(), Default::default());
    settle(ui);
    assert_eq!(
        mounted.events.borrow().len(),
        1,
        "fresh input remains usable"
    );
    ui.update(|_, cx| {
        assert_eq!(mounted.source.read(cx).value().as_ref(), content(1));
    });
}

/// Legacy code groups have no aligned-pane incarnation that might incidentally clear a press.
/// Replacing the complete source version must therefore revoke the button's own gesture too.
#[gpui::test]
#[ignore = "native legacy source-action press ownership; run explicitly with --ignored --test-threads=1"]
fn native_code_alignment_legacy_source_press_cannot_activate_a_new_source_version(
    cx: &mut TestAppContext,
) {
    let (mounted, ui) = mount_with_document(
        cx,
        |version| default_document(version).code_alignment(false),
        content(1),
        false,
        false,
        size(px(1060.), px(260.)),
    );
    let old = rail(ui, false);
    mounted.events.borrow_mut().clear();
    ui.simulate_mouse_down(old.center(), MouseButton::Left, Default::default());
    mounted.current_revision.set(8);
    ui.update(|window, cx| {
        let mut version = mounted.version.clone();
        version.revision = 8;
        mounted.view.update(cx, |view, cx| {
            view.set_code_sync_source(Some((version.clone(), mounted.source.clone())), cx);
            view.update_document(
                default_document(&version).code_alignment(false).revision(2),
                Environment::default(),
                window,
                cx,
            );
        });
    });
    settle(ui);
    let next = rail(ui, false);
    assert_eq!(old, next);
    ui.simulate_mouse_up(next.center(), MouseButton::Left, Default::default());
    settle(ui);
    assert!(
        mounted.events.borrow().is_empty(),
        "a source replacement cannot inherit a legacy button's pending press"
    );
    ui.simulate_click(next.center(), Default::default());
    settle(ui);
    assert_eq!(mounted.events.borrow().len(), 1);
    assert_eq!(mounted.events.borrow()[0].revision, 2);
}

/// Source-action canvases revoke drags on replacement; ordinary document canvases keep their
/// existing cross-repaint capture behavior, covered separately by the shared UI regression.
#[gpui::test]
#[ignore = "native source-canvas press ownership; run explicitly with --ignored --test-threads=1"]
fn native_code_alignment_source_canvas_cannot_release_into_a_replacement_scene(
    cx: &mut TestAppContext,
) {
    let (mounted, ui) = mount_with_document(
        cx,
        |version| {
            action_document(
                version,
                ui::Node::new("rail-canvas", Kind::Canvas(ui::Canvas::default()))
                    .width(64.)
                    .height(28.),
            )
        },
        content(1),
        false,
        false,
        size(px(1060.), px(260.)),
    );
    let hit = ui.debug_bounds("plugin-ui-rail-canvas").unwrap().center();
    mounted.events.borrow_mut().clear();
    ui.simulate_mouse_down(hit, MouseButton::Left, Default::default());
    settle(ui);
    assert!(mounted.events.borrow().iter().any(|event| matches!(
        event.action,
        Action::Canvas(ui::CanvasEvent::Pointer {
            phase: ui::PointerPhase::Down,
            ..
        })
    )));
    mounted.events.borrow_mut().clear();
    ui.update(|window, cx| {
        mounted.view.update(cx, |view, cx| {
            view.update_document(
                mounted.document.clone().revision(2),
                Environment::default(),
                window,
                cx,
            );
        });
    });
    settle(ui);
    ui.simulate_mouse_up(hit, MouseButton::Left, Default::default());
    settle(ui);
    assert!(
        !mounted.events.borrow().iter().any(|event| matches!(
            event.action,
            Action::Canvas(ui::CanvasEvent::Pointer {
                phase: ui::PointerPhase::Up,
                ..
            })
        )),
        "an old source-canvas capture cannot release into the replacement"
    );
    mounted.events.borrow_mut().clear();
    ui.simulate_click(hit, Default::default());
    settle(ui);
    assert!(
        mounted
            .events
            .borrow()
            .iter()
            .any(|event| event.revision == 2
                && matches!(
                    event.action,
                    Action::Canvas(ui::CanvasEvent::Pointer {
                        phase: ui::PointerPhase::Up,
                        ..
                    })
                ))
    );
}

#[gpui::test]
#[ignore = "native source-collection press ownership; run explicitly with --ignored --test-threads=1"]
fn native_code_alignment_source_collection_cannot_select_with_a_replaced_press(
    cx: &mut TestAppContext,
) {
    let (mounted, ui) = mount_with_document(
        cx,
        |version| {
            action_document(
                version,
                ui::Node::new(
                    "rail-tabs",
                    Kind::SideTabs(ui::SideTabs {
                        id: "rail-tabs".into(),
                        position: ui::SideTabsPosition::Left,
                        width: 64.,
                        min_width: 48.,
                        max_width: 76.,
                        selected: Some("first".into()),
                        rename: None,
                        items: ["first", "second"]
                            .into_iter()
                            .map(|id| ui::SideTab {
                                id: id.into(),
                                label: id.into(),
                                status: None,
                                disabled: false,
                                closable: false,
                            })
                            .collect(),
                    }),
                )
                .width(64.)
                .height(64.),
            )
        },
        content(1),
        false,
        false,
        size(px(1060.), px(260.)),
    );
    let hit = ui.debug_bounds("side-tab-second").unwrap().center();
    mounted.events.borrow_mut().clear();
    ui.simulate_mouse_down(hit, MouseButton::Left, Default::default());
    ui.update(|window, cx| {
        mounted.view.update(cx, |view, cx| {
            view.update_document(
                mounted.document.clone().revision(2),
                Environment::default(),
                window,
                cx,
            );
        });
    });
    settle(ui);
    ui.simulate_mouse_up(hit, MouseButton::Left, Default::default());
    settle(ui);
    assert!(
        mounted.events.borrow().is_empty(),
        "an old collection press cannot select in the new scene"
    );
    ui.simulate_click(hit, Default::default());
    settle(ui);
    assert_eq!(mounted.events.borrow().len(), 1);
    assert_eq!(mounted.events.borrow()[0].node, "rail-tabs");
    assert_eq!(mounted.events.borrow()[0].revision, 2);
}
