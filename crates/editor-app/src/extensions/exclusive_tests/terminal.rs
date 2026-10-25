//! An explicitly ordinary terminal exercises generic shared-region and exclusive-peer restoration.
//! Product-default exclusivity is covered by the native drag/default regression suite.

use super::*;
use gpui_kit::{MouseButton, point};

mod hidden_persistence;
mod persistence;
mod repeated_toggle;

/// Build an isolated editor and publish bottom peers, optionally relocating their visible region.
fn setup(
    cx: &mut TestAppContext,
    center_bottom: bool,
) -> (tempfile::TempDir, Entity<EditorApp>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        init(cx);
        cx.set_reduce_motion(true);
    });
    let directory = tempfile::tempdir().unwrap();
    let workspace_path = directory.path().join("workspace");
    std::fs::create_dir_all(&workspace_path).unwrap();
    let workspace = Workspace::open(&workspace_path).unwrap();
    // Session file hashing and identity checks use the same canonical root as the real workspace.
    let mut session =
        SessionState::load_in_profile(workspace.root(), &directory.path().join("profile"));
    session.explorer_visible = true;
    session.outline_visible = false;
    session.messages_visible = false;
    session.native_terminal_visible = false;
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let app =
            cx.new(|cx| EditorApp::new_with_session(workspace, None, Some(session), window, cx));
        *capture.borrow_mut() = Some(app.clone());
        Root::new(app, window, cx)
    });
    let app = slot.borrow_mut().take().unwrap();
    ordinary_terminal(&app, visual);
    visual.simulate_resize(size(px(1280.), px(900.)));
    let manifest: protocol::Manifest = serde_json::from_value(serde_json::json!({
        "id":"region-fixture", "name":"Regional panels", "version":"1.0.0", "protocol":7,
        "permissions":["ui.panels"], "storage_limit":1024,
        "api":{"base":"^1","required":{"ui.panels":">=1.2,<2"}},
        "panels":[
            {"id":"shared","title":"Shared bottom peer", "position":"bottom"},
            {"id":"exclusive","title":"Exclusive bottom peer", "position":"bottom",
             "exclusive":true,"default_visible":false}
        ]
    }))
    .unwrap();
    publish(&app, &manifest, visual);
    if center_bottom {
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                let editor = PanelId::from(app.editor_panel.entity_id());
                let peer = PanelId::from(app.plugin_panels["region-fixture/shared"].entity_id());
                // A user can place the existing Git-like panel below only the editor, retaining
                // a full-height Explorer. The hidden terminal still has its older default slot.
                app.dock_area.update(cx, |area, cx| {
                    let node = area
                        .layout(DockPlacement::Center)
                        .unwrap()
                        .find_panel_node(editor)
                        .unwrap();
                    area.move_panel(
                        peer,
                        gpui_base::dock::InsertTarget::Split {
                            node,
                            placement: gpui_base::Placement::Bottom,
                            size: Some(px(260.)),
                        },
                        window,
                        cx,
                    );
                });
            })
        });
    }
    paint(visual);
    (directory, app, visual)
}

/// Resolve current native identities in Editor, Explorer, Terminal, ordinary and exclusive order.
fn panels(app: &Entity<EditorApp>, visual: &mut VisualTestContext) -> [PanelId; 5] {
    visual.update(|_, cx| {
        let app = app.read(cx);
        [
            PanelId::from(app.editor_panel.entity_id()),
            PanelId::from(app.explorer_panel.entity_id()),
            PanelId::from(app.terminal.entity_id()),
            PanelId::from(app.plugin_panels["region-fixture/shared"].entity_id()),
            PanelId::from(app.plugin_panels["region-fixture/exclusive"].entity_id()),
        ]
    })
}

/// Keep shared-region fixtures independent of Terminal's new exclusive product default.
fn ordinary_terminal(app: &Entity<EditorApp>, visual: &mut VisualTestContext) {
    visual.update(|_, cx| {
        app.update(cx, |app, _| {
            app.dock_presentation
                .set_exclusive(PanelId::from(app.terminal.entity_id()), false);
        });
    });
}

/// Activate a currently painted footer control and discard stale bounds from the preceding frame.
fn click(visual: &mut VisualTestContext, selector: &'static str) {
    let button = visual.debug_bounds(selector).unwrap();
    visual.simulate_click(button.center(), Default::default());
    paint(visual);
}

/// Compare complete painted allocations with the renderer's two-pixel rounding tolerance.
#[track_caller]
fn unchanged(before: Bounds<Pixels>, after: Bounds<Pixels>) {
    assert!(
        (after.left() - before.left()).abs() < px(2.)
            && (after.top() - before.top()).abs() < px(2.)
            && (after.size.width - before.size.width).abs() < px(2.)
            && (after.size.height - before.size.height).abs() < px(2.),
        "an unrelated allocation changed: {before:?} -> {after:?}"
    );
}

/// Give the ordinary Center pair an asymmetric saved preference through Base's public split edit.
fn set_unequal_bottom(app: &Entity<EditorApp>, ids: [PanelId; 5], visual: &mut VisualTestContext) {
    visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.dock_area.update(cx, |area, cx| {
                let mut split = None;
                area.layout(DockPlacement::Center)
                    .unwrap()
                    .root()
                    .walk(&mut |node| {
                        let gpui_base::dock::PaneRef::Split { axis, children, .. } = node.kind()
                        else {
                            return;
                        };
                        if axis != gpui_kit::Axis::Horizontal || children.len() != 2 {
                            return;
                        }
                        let widths = children
                            .iter()
                            .map(|child| match child.kind() {
                                gpui_base::dock::PaneRef::Tabs { panels, .. }
                                    if panels.contains(&ids[2]) =>
                                {
                                    Some(px(700.))
                                }
                                gpui_base::dock::PaneRef::Tabs { panels, .. }
                                    if panels.contains(&ids[3]) =>
                                {
                                    Some(px(300.))
                                }
                                _ => None,
                            })
                            .collect::<Option<Vec<_>>>();
                        if let Some(widths) = widths {
                            split = Some((node.id(), widths));
                        }
                    });
                let (node, widths) = split.expect("ordinary pair must occupy one bottom split");
                area.set_split_sizes(node, widths, window, cx);
            });
        });
    });
    paint(visual);
    let terminal = native_panel_bounds(app, ids[2], visual);
    let shared = native_panel_bounds(app, ids[3], visual);
    assert!((terminal.size.width - px(700.)).abs() < px(2.));
    assert!((shared.size.width - px(300.)).abs() < px(2.));
}

/// Both ordinary leaves must fill one former bottom allocation without consuming another row.
fn assert_shared_bottom(
    app: &Entity<EditorApp>,
    ids: [PanelId; 5],
    before: [Bounds<Pixels>; 3],
    visual: &mut VisualTestContext,
) {
    let [editor, explorer, terminal, peer, _] = ids;
    let terminal_after = native_panel_bounds(app, terminal, visual);
    let peer_after = native_panel_bounds(app, peer, visual);
    assert!(
        (terminal_after.top() - peer_after.top()).abs() < px(2.),
        "terminal must join the existing bottom row: terminal={terminal_after:?}, peer={peer_after:?}"
    );
    assert!(
        terminal_after.left() >= peer_after.right() - px(2.)
            || peer_after.left() >= terminal_after.right() - px(2.),
        "ordinary bottom peers must appear side by side"
    );
    assert!((terminal_after.top() - before[2].top()).abs() < px(2.));
    assert!((terminal_after.bottom() - before[2].bottom()).abs() < px(2.));
    assert!((peer_after.bottom() - before[2].bottom()).abs() < px(2.));
    assert!((terminal_after.left().min(peer_after.left()) - before[2].left()).abs() < px(2.));
    assert!(
        (terminal_after.right().max(peer_after.right()) - before[2].right()).abs() < px(2.),
        "ordinary bottom peers must retain the original region's right edge: before={:?}, terminal={terminal_after:?}, peer={peer_after:?}",
        before[2]
    );
    unchanged(before[0], native_panel_bounds(app, editor, visual));
    unchanged(before[1], native_panel_bounds(app, explorer, visual));
    assert!(visual.debug_bounds("native-terminal-output").is_some());
    assert!(visual.debug_bounds("plugin-ui-content-shared").is_some());
    visual.update(|_, cx| assert!(app.read(cx).terminal_visible(cx)));
}

/// Reveal, hide and reopen against either representation of the user's existing bottom region.
fn exercise_bottom(cx: &mut TestAppContext, center_bottom: bool) {
    let (_directory, app, visual) = setup(cx, center_bottom);
    let ids = panels(&app, visual);
    let before = [ids[0], ids[1], ids[3]].map(|id| native_panel_bounds(&app, id, visual));
    click(visual, "terminal-toggle");
    assert_shared_bottom(&app, ids, before, visual);
    let shared = [ids[2], ids[3]].map(|id| native_panel_bounds(&app, id, visual));
    click(visual, "terminal-toggle");
    assert!(visual.debug_bounds("native-terminal-output").is_none());
    unchanged(before[2], native_panel_bounds(&app, ids[3], visual));
    visual.update(|_, cx| {
        assert!(!app.read(cx).terminal_visible(cx));
        assert_eq!(PanelId::from(app.read(cx).terminal.entity_id()), ids[2]);
    });
    click(visual, "terminal-toggle");
    assert_shared_bottom(&app, ids, before, visual);
    for (id, bounds) in [ids[2], ids[3]].into_iter().zip(shared) {
        unchanged(bounds, native_panel_bounds(&app, id, visual));
    }
}

/// Opening the terminal beside a relocated bottom peer must not add another full-window row.
#[gpui::test]
fn terminal_joins_existing_center_bottom_region(cx: &mut TestAppContext) {
    exercise_bottom(cx, true);
}

/// A native Bottom dock follows the same shared geometry and hide/reopen contract.
#[gpui::test]
fn terminal_joins_existing_native_bottom_region(cx: &mut TestAppContext) {
    exercise_bottom(cx, false);
}

/// First terminal registration while an exclusive leaf is selected preserves the ordinary region.
#[gpui::test]
fn terminal_first_registration_reclaims_exclusive_native_bottom(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, false);
    let ids = panels(&app, visual);
    let before = [ids[0], ids[1], ids[3]].map(|id| native_panel_bounds(&app, id, visual));
    visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            // Retain the host terminal entity while modelling a never-registered dock leaf.
            let terminal = app.terminal.clone();
            app.dock_area.update(cx, |area, cx| {
                area.remove_panel(terminal, window, cx);
            });
        });
    });
    paint(visual);
    click(visual, "plugin-window-region-fixture/exclusive");
    assert!(visual.debug_bounds("plugin-ui-content-exclusive").is_some());
    assert!(visual.debug_bounds("plugin-ui-content-shared").is_none());
    visual.update(|_, cx| {
        let app = app.read(cx);
        assert!(app.dock_area.read(cx).panel(ids[2]).is_none());
        assert!(!app.terminal.read(cx).visible());
        assert!(app.dock_presentation.is_suppressed(ids[3]));
    });
    click(visual, "terminal-toggle");
    assert_shared_bottom(&app, ids, before, visual);
    assert!(visual.debug_bounds("plugin-ui-content-exclusive").is_none());
    visual.update(|_, cx| {
        let area = app.read(cx).dock_area.read(cx);
        let bottom = area.layout(DockPlacement::Bottom).unwrap();
        for id in [ids[2], ids[3]] {
            assert!(bottom.find_panel_node(id).is_some());
        }
    });
}

/// Exclusive peers replace the same bottom region; choosing Terminal restores its ordinary combination.
#[gpui::test]
fn terminal_and_exclusive_bottom_peer_restore_the_same_region(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, true);
    let ids = panels(&app, visual);
    let before = [ids[0], ids[1], ids[3]].map(|id| native_panel_bounds(&app, id, visual));
    click(visual, "terminal-toggle");
    assert_shared_bottom(&app, ids, before, visual);
    set_unequal_bottom(&app, ids, visual);
    let shared = [ids[2], ids[3]].map(|id| native_panel_bounds(&app, id, visual));
    for restore_with_terminal in [true, false] {
        click(visual, "plugin-window-region-fixture/exclusive");
        assert!(visual.debug_bounds("native-terminal-output").is_none());
        assert!(visual.debug_bounds("plugin-ui-content-shared").is_none());
        assert!(visual.debug_bounds("plugin-ui-content-exclusive").is_some());
        unchanged(before[2], native_panel_bounds(&app, ids[4], visual));
        unchanged(before[0], native_panel_bounds(&app, ids[0], visual));
        unchanged(before[1], native_panel_bounds(&app, ids[1], visual));
        visual.update(|_, cx| {
            assert!(app.read(cx).terminal.read(cx).visible());
            assert!(!app.read(cx).terminal_visible(cx));
        });
        click(
            visual,
            if restore_with_terminal {
                "terminal-toggle"
            } else {
                "plugin-window-region-fixture/exclusive"
            },
        );
        assert_shared_bottom(&app, ids, before, visual);
        assert!(visual.debug_bounds("plugin-ui-content-exclusive").is_none());
        for (id, bounds) in [ids[2], ids[3]].into_iter().zip(shared) {
            unchanged(bounds, native_panel_bounds(&app, id, visual));
        }
    }
}

/// A suppressed shared combination is still occupied and must not split when one member is revealed.
#[gpui::test]
fn terminal_reveal_keeps_suppressed_center_peers_together(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, true);
    let ids = panels(&app, visual);
    let before = [ids[0], ids[1], ids[3]].map(|id| native_panel_bounds(&app, id, visual));
    click(visual, "terminal-toggle");
    assert_shared_bottom(&app, ids, before, visual);
    let shared = [ids[2], ids[3]].map(|id| native_panel_bounds(&app, id, visual));
    visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            let exclusive = app.plugin_panels["region-fixture/exclusive"].clone();
            // Model a retained native Bottom slot without going through reveal's relocation.
            // Both ordinary leaves keep their existing Center band and raw visible intent.
            let handle = app.dock_presentation.panel_handle(exclusive.clone());
            app.dock_area.update(cx, |area, cx| {
                area.set_dock(
                    DockPlacement::Bottom,
                    DockLayout::tabs().panel_view(handle, cx),
                    window,
                    cx,
                );
                area.set_dock_size(DockPlacement::Bottom, px(260.), window, cx);
            });
            exclusive.update(cx, |panel, cx| {
                panel.show(window, cx);
                cx.notify();
            });
            app.sync_plugin_panels(window, cx);
            cx.notify();
        });
    });
    paint(visual);
    assert!(visual.debug_bounds("plugin-ui-content-exclusive").is_some());
    assert!(visual.debug_bounds("native-terminal-output").is_none());
    assert!(visual.debug_bounds("plugin-ui-content-shared").is_none());
    visual.update(|_, cx| {
        let app = app.read(cx);
        let area = app.dock_area.read(cx);
        for id in [ids[2], ids[3]] {
            assert!(
                area.layout(DockPlacement::Center)
                    .unwrap()
                    .find_panel_node(id)
                    .is_some()
            );
            assert!(app.dock_presentation.is_suppressed(id));
        }
        assert!(app.terminal.read(cx).visible());
        assert!(
            app.plugin_panels["region-fixture/shared"]
                .read(cx)
                .visible
                .get()
        );
        assert!(
            area.layout(DockPlacement::Bottom)
                .unwrap()
                .find_panel_node(ids[4])
                .is_some()
        );
    });

    click(visual, "terminal-toggle");
    visual.update(|_, cx| {
        let area = app.read(cx).dock_area.read(cx);
        for id in [ids[2], ids[3]] {
            assert!(
                area.layout(DockPlacement::Center)
                    .unwrap()
                    .find_panel_node(id)
                    .is_some(),
                "revealing Terminal must retain both ordinary peers in their original Center band"
            );
            assert!(
                area.layout(DockPlacement::Bottom)
                    .is_none_or(|tree| tree.find_panel_node(id).is_none())
            );
        }
    });
    assert_shared_bottom(&app, ids, before, visual);
    assert!(visual.debug_bounds("plugin-ui-content-exclusive").is_none());
    for (id, bounds) in [ids[2], ids[3]].into_iter().zip(shared) {
        unchanged(bounds, native_panel_bounds(&app, id, visual));
    }
}

/// A hidden exclusive alternative must not retain an ordinary terminal in an otherwise dormant band.
#[gpui::test]
fn terminal_reveal_ignores_suppressed_exclusive_peer_in_old_band(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, false);
    let ids = panels(&app, visual);
    let before = [ids[0], ids[1], ids[3]].map(|id| native_panel_bounds(&app, id, visual));
    visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            // A saved Center band retains a hidden Terminal and an exclusive alternative;
            // the selected ordinary plugin still occupies the separate native Bottom dock.
            // Startup may have pruned the never-shown terminal with its old empty dock.
            app.ensure_terminal_dock(window, cx);
            let exclusive = app
                .dock_presentation
                .panel_handle(app.plugin_panels["region-fixture/exclusive"].clone());
            app.dock_area.update(cx, |area, cx| {
                if area.panel(ids[4]).is_none() {
                    area.add_panel_view(exclusive, DockPlacement::Bottom, None, window, cx);
                }
                let editor = area
                    .layout(DockPlacement::Center)
                    .unwrap()
                    .find_panel_node(ids[0])
                    .unwrap();
                area.move_panel(
                    ids[2],
                    gpui_base::dock::InsertTarget::Split {
                        node: editor,
                        placement: gpui_base::Placement::Bottom,
                        size: Some(px(260.)),
                    },
                    window,
                    cx,
                );
                let terminal = area
                    .layout(DockPlacement::Center)
                    .unwrap()
                    .find_panel_node(ids[2])
                    .unwrap();
                area.move_panel(
                    ids[4],
                    gpui_base::dock::InsertTarget::Split {
                        node: terminal,
                        placement: gpui_base::Placement::Right,
                        size: None,
                    },
                    window,
                    cx,
                );
            });
            for key in ["region-fixture/exclusive", "region-fixture/shared"] {
                app.plugin_panels[key].update(cx, |panel, cx| {
                    panel.show(window, cx);
                    cx.notify();
                });
            }
            app.sync_plugin_panels(window, cx);
            cx.notify();
        });
    });
    paint(visual);
    assert!(visual.debug_bounds("plugin-ui-content-shared").is_some());
    assert!(visual.debug_bounds("plugin-ui-content-exclusive").is_none());
    assert!(visual.debug_bounds("native-terminal-output").is_none());
    visual.update(|_, cx| {
        let app = app.read(cx);
        let area = app.dock_area.read(cx);
        assert!(!app.terminal.read(cx).visible());
        assert!(
            app.plugin_panels["region-fixture/exclusive"]
                .read(cx)
                .visible
                .get()
        );
        assert!(app.dock_presentation.is_suppressed(ids[4]));
        for id in [ids[2], ids[4]] {
            assert!(
                area.layout(DockPlacement::Center)
                    .unwrap()
                    .find_panel_node(id)
                    .is_some()
            );
        }
        assert!(
            area.layout(DockPlacement::Bottom)
                .unwrap()
                .find_panel_node(ids[3])
                .is_some()
        );
    });

    click(visual, "terminal-toggle");
    visual.update(|_, cx| {
        let area = app.read(cx).dock_area.read(cx);
        for id in [ids[2], ids[3]] {
            assert!(
                area.layout(DockPlacement::Bottom).unwrap().find_panel_node(id).is_some(),
                "an inactive exclusive alternative must not prevent Terminal joining the visible ordinary region"
            );
        }
        assert!(area.layout(DockPlacement::Center).unwrap().find_panel_node(ids[2]).is_none());
    });
    assert_shared_bottom(&app, ids, before, visual);
    assert!(visual.debug_bounds("plugin-ui-content-exclusive").is_none());
}

/// Move the visible native terminal through its real title into a left peer or an empty right side.
fn drag_terminal(
    app: &Entity<EditorApp>,
    ids: [PanelId; 5],
    right: bool,
    visual: &mut VisualTestContext,
) {
    let terminal = ids[2];
    native_panel_bounds(app, terminal, visual);
    let node = visual.update(|_, cx| {
        let area = app.read(cx).dock_area.read(cx);
        [DockPlacement::Center, DockPlacement::Bottom]
            .into_iter()
            .find_map(|placement| area.layout(placement)?.find_panel_node(terminal))
            .unwrap()
    });
    let title = visual
        .debug_bounds(Box::leak(
            format!("local-dock-title-{}", node.as_u64()).into_boxed_str(),
        ))
        .unwrap();
    let target = native_panel_bounds(app, ids[if right { 0 } else { 1 }], visual);
    let destination = if right {
        point(target.right() - px(8.), target.center().y)
    } else {
        point(target.center().x, target.bottom() - px(8.))
    };
    visual.simulate_mouse_down(title.center(), MouseButton::Left, Default::default());
    visual.simulate_mouse_move(
        title.center() + point(px(12.), px(0.)),
        MouseButton::Left,
        Default::default(),
    );
    visual.simulate_mouse_move(destination, MouseButton::Left, Default::default());
    visual.update(|window, cx| window.draw(cx).clear(cx));
    assert!(visual.debug_bounds("local-dock-drop-indicator").is_some());
    visual.simulate_mouse_up(destination, MouseButton::Left, Default::default());
    paint(visual);
}

/// A moved terminal remains selected and toggles its actual side even after its former bottom closes.
#[gpui::test]
fn terminal_toggle_follows_actual_left_and_right_placement(cx: &mut TestAppContext) {
    for right in [false, true] {
        let (_directory, app, visual) = setup(cx, false);
        let ids = panels(&app, visual);
        click(visual, "terminal-toggle");
        drag_terminal(&app, ids, right, visual);
        let terminal = native_panel_bounds(&app, ids[2], visual);
        let editor = native_panel_bounds(&app, ids[0], visual);
        if right {
            assert!(terminal.left() >= editor.right() - px(2.));
        } else {
            let explorer = native_panel_bounds(&app, ids[1], visual);
            assert!((terminal.left() - explorer.left()).abs() < px(2.));
            assert!((terminal.size.width - explorer.size.width).abs() < px(2.));
            assert!(terminal.top() >= explorer.bottom() - px(2.));
        }
        // Close the last bottom peer through its real control so the old Dock's open flag
        // cannot mask a visibility query that still assumes the terminal's default position.
        click(visual, "plugin-window-region-fixture/shared");
        visual.update(|_, cx| {
            let app = app.read(cx);
            assert!(!app.dock_area.read(cx).is_dock_open(DockPlacement::Bottom));
            assert!(app.terminal_visible(cx));
        });
        let terminal = native_panel_bounds(&app, ids[2], visual);
        let editor = native_panel_bounds(&app, ids[0], visual);
        click(visual, "terminal-toggle");
        assert!(visual.debug_bounds("native-terminal-output").is_none());
        visual.update(|_, cx| assert!(!app.read(cx).terminal.read(cx).visible()));
        click(visual, "terminal-toggle");
        unchanged(terminal, native_panel_bounds(&app, ids[2], visual));
        unchanged(editor, native_panel_bounds(&app, ids[0], visual));
        visual.update(|_, cx| {
            let app = app.read(cx);
            assert!(app.terminal_visible(cx));
            assert!(!app.dock_area.read(cx).is_dock_open(DockPlacement::Bottom));
            assert_eq!(PanelId::from(app.terminal.entity_id()), ids[2]);
        });
    }
}
