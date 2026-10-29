//! Independent real SDK providers enter Manager and the native editor publication seam.
use super::*;
use gpui_kit::{TestAppContext, VisualTestContext, gpui};
use plugin_runtime::{
    Manager,
    plugin_protocol::{Environment, api, ui},
};
use serde_json::{Value as Json, json};

mod scroll;

/// Isolated real packages require exactly their declared grants, without a business-specific host API.
fn providers(workspace: &Path, ids: &[&str]) -> (tempfile::TempDir, Manager) {
    let root = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(
        root.path().into(),
        Environment {
            workspace: workspace.display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    for id in ids {
        // Provider discovery is never inferred from target output or shipped package directories.
        let package = crate::test_packages::package(&format!("{id}-0.1.1"));
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
    }
    (root, manager)
}

/// Task completion advances the actual guest, while native effects use production ExtensionPanel polling.
fn drain(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    manager: &mut Manager,
) -> Vec<std::sync::Arc<plugin_runtime::DecorationResource>> {
    let mut owned = Vec::new();
    for _ in 0..32 {
        // Typed tree activation and task completion follow the manager's ordinary actor tick.
        manager.poll();
        let mut requests = Vec::new();
        for (id, instance) in &mut manager.live {
            requests.extend(
                instance
                    .take_editor_requests()
                    .into_iter()
                    .map(|request| (id.clone(), request)),
            );
        }
        if requests.is_empty() {
            break;
        }
        owned.extend(
            requests
                .iter()
                .filter_map(|(_, request)| request.decoration().cloned()),
        );
        visual.update(|_, cx| {
            app.read(cx).extensions.clone().update(cx, |owner, cx| {
                let mut state = owner.worker.state.lock().unwrap();
                state.publish_manager(manager);
                state.editor_requests.extend(requests.iter().cloned());
                drop(state);
                owner.poll(cx);
            });
        });
        visual.run_until_parked();
        for (_, request) in &requests {
            assert!(
                matches!(
                    request.status(),
                    api::RequestUpdate::Completed { result: Ok(_) }
                        | api::RequestUpdate::Cancelled { .. }
                ),
                "native consumer failed: {:?}",
                request.status()
            );
        }
        manager.poll();
    }
    visual.update(|window, cx| window.draw(cx).clear(cx));
    owned
}

/// Deliver the actual native ingress; consumers rescan immutable snapshots instead of marker history.
pub(super) fn events(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    manager: &mut Manager,
) {
    let ingress = visual.update(|_, cx| {
        app.read(cx)
            .extensions
            .read(cx)
            .worker
            .state
            .lock()
            .unwrap()
            .document_stream
            .take_batch(128)
            .unwrap_or_default()
    });
    for event in ingress {
        manager.document_event(event);
    }
    for _ in 0..16 {
        manager.poll();
    }
    drain(visual, app, manager);
}

/// Native input drives two separate domains: Todo navigation and coverage semantic reconstruction.
#[gpui::test]
#[ignore = "opt-in frozen Package/WASM fixtures; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn community_todos_and_coverage_follow_real_cut_replace_undo_and_close(cx: &mut TestAppContext) {
    use plugin_runtime::plugin_protocol::tree;
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        visual.simulate_keystrokes("ctrl-a");
        visual.simulate_input("COVERED 中文😀\r\nTODO 行\r\nplain");
        visual.run_until_parked();
        let (_root, mut manager) = providers(path.parent().unwrap(), &["todo-tree", "coverage"]);
        events(visual, &app, &mut manager);
        // The tree is actually used here; its declared refresh command consumes the cold owner.
        command(visual, &app, &mut manager, "todo-tree", "refresh-todos");
        let collection = command(visual, &app, &mut manager, "coverage", "scan-coverage")
            .pop()
            .unwrap();
        let ranges = positions(visual, &app, &mut manager, "coverage", "read-coverage");
        assert_eq!(ranges[0].range, Some(api::TextRange { start: 0, end: 18 }));
        let resource = manager.live["todo-tree"].tree_resources().pop().unwrap();
        // Lazy nodes come from current unsaved text; the backing file still has no Todo content.
        let load = |manager: &mut Manager, parent: Option<String>| {
            let request = resource.request(parent, false).unwrap().unwrap();
            manager
                .event(
                    "todo-tree",
                    Some("presentation".into()),
                    api::Notification::Ui(ui::UiEvent {
                        revision: 1,
                        node: "todo-tree".into(),
                        action: ui::Action::Tree(tree::Event::Request(request)),
                    }),
                )
                .unwrap();
        };
        load(&mut manager, None);
        let document_node = resource.snapshot().branches[&None].children[0].clone();
        load(&mut manager, Some(document_node.clone()));
        let snapshot = resource.snapshot();
        let todo = snapshot.branches[&Some(document_node.clone())].children[0].clone();
        assert_eq!(snapshot.nodes[&todo].label, "2: TODO 行");
        manager
            .event(
                "todo-tree",
                Some("presentation".into()),
                api::Notification::Ui(ui::UiEvent {
                    revision: 1,
                    node: "todo-tree".into(),
                    action: ui::Action::Tree(tree::Event::Invoke {
                        tree: resource.handle().clone(),
                        epoch: snapshot.epoch,
                        node: todo.clone(),
                        action: tree::NodeAction::Activation,
                    }),
                }),
            )
            .unwrap();
        drain(visual, &app, &mut manager);
        visual.update(|_, cx| assert_eq!(app.read(cx).editor.read(cx).selected_range().start, 20));
        // Prefix input shifts current geometry without clearing the independent coverage source.
        visual.simulate_keystrokes("ctrl-home");
        visual.simulate_input("头😀\r\n");
        visual.run_until_parked();
        assert_eq!(
            positions(visual, &app, &mut manager, "coverage", "read-coverage")[0].range,
            Some(api::TextRange { start: 9, end: 27 })
        );
        events(visual, &app, &mut manager);
        assert!(
            manager
                .event(
                    "todo-tree",
                    Some("presentation".into()),
                    api::Notification::Ui(ui::UiEvent {
                        revision: 1,
                        node: "todo-tree".into(),
                        action: ui::Action::Tree(tree::Event::Invoke {
                            tree: resource.handle().clone(),
                            epoch: snapshot.epoch,
                            node: todo,
                            action: tree::NodeAction::Activation
                        })
                    })
                )
                .is_err(),
            "a cached node command cannot navigate after refreshed document events"
        );
        visual.simulate_keystrokes("ctrl-a ctrl-x");
        visual.run_until_parked();
        assert_eq!(
            positions(visual, &app, &mut manager, "coverage", "read-coverage")[0].range,
            None
        );
        events(visual, &app, &mut manager);
        assert!(positions(visual, &app, &mut manager, "coverage", "read-coverage").is_empty());
        visual.simulate_keystrokes("ctrl-z");
        visual.run_until_parked();
        events(visual, &app, &mut manager);
        // Undo does not resurrect host ranges; the independent coverage consumer derives them again.
        assert_eq!(
            positions(visual, &app, &mut manager, "coverage", "read-coverage")[0].range,
            Some(api::TextRange { start: 9, end: 27 })
        );
        // Whole-range replacement invalidates semantics and is independently undone/recomputed.
        visual.simulate_keystrokes("ctrl-a");
        visual.simulate_input("x");
        visual.run_until_parked();
        assert_eq!(
            positions(visual, &app, &mut manager, "coverage", "read-coverage")[0].range,
            None
        );
        events(visual, &app, &mut manager);
        visual.simulate_keystrokes("ctrl-z");
        visual.run_until_parked();
        events(visual, &app, &mut manager);
        assert_eq!(
            positions(visual, &app, &mut manager, "coverage", "read-coverage")[0].range,
            Some(api::TextRange { start: 9, end: 27 })
        );
        manager.disable("coverage").unwrap();
        assert!(
            !collection.is_live(),
            "retirement seals every retained native marker authority"
        );
        visual.run_until_parked();
        // Tree ownership survives another provider's retirement, then its own close/disable seals it.
        assert!(resource.is_live());
        assert_eq!(std::fs::read_to_string(path).unwrap(), "original on disk");
        visual.update(|window, cx| {
            app.update(cx, |owner, cx| {
                owner.close_tab(path.to_path_buf(), window, cx)
            })
        });
        visual.run_until_parked();
        events(visual, &app, &mut manager);
        command(visual, &app, &mut manager, "todo-tree", "refresh-todos");
        assert!(resource.snapshot().nodes.is_empty());
        manager.disable("todo-tree").unwrap();
        assert!(!resource.is_live());
    });
}
/// Demand a declared business command and return its actual native decoration resources.
pub(super) fn command(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    manager: &mut Manager,
    id: &str,
    command: &str,
) -> Vec<std::sync::Arc<plugin_runtime::DecorationResource>> {
    super::package_ui_test_support::demand(
        manager,
        id,
        plugin_runtime::DemandReason::Command(command.into()),
    )
    .unwrap();
    manager.invoke_command(id, command, Json::Null).unwrap();
    drain(visual, app, manager)
}
fn text(manager: &Manager, id: &str) -> String {
    let scene = manager.live[id].views.values().next().unwrap();
    let ui::Kind::Text { text } = &scene.root.kind else {
        panic!("text receipt expected")
    };
    text.clone()
}
/// Read versioned decoration positions through the guest's public command receipt.
pub(super) fn positions(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    manager: &mut Manager,
    id: &str,
    read: &str,
) -> Vec<plugin_runtime::plugin_protocol::decorations::Position> {
    command(visual, app, manager, id, read);
    let receipt = text(manager, id);
    let api::EditorValue::Decorations { positions, .. } = serde_json::from_str(&receipt)
        .unwrap_or_else(|error| panic!("positions receipt failed: {receipt}; {error}"))
    else {
        panic!("positions expected")
    };
    positions
}

/// An ordinary guest SDK diagnostic call exposes an accepted task; production polling performs it.
pub(super) fn admit_marker(
    manager: &mut Manager,
    id: &str,
    operation: api::Operation,
) -> Result<plugin_runtime::EditorRequest, api::Failure> {
    super::package_ui_test_support::demand(
        manager,
        id,
        plugin_runtime::DemandReason::Command("probe".into()),
    )?;
    manager
        .invoke_command(id, "probe", serde_json::to_value(operation).unwrap())
        .unwrap();
    let value: Result<api::Value, api::Failure> = serde_json::from_str(&text(manager, id)).unwrap();
    let api::Value::Accepted(handle) = value? else {
        panic!("accepted native task expected")
    };
    Ok(manager
        .live
        .get_mut(id)
        .unwrap()
        .take_editor_requests()
        .into_iter()
        .find(|request| request.handle() == &handle)
        .unwrap())
}

/// Failed and cancelled tasks use exactly the same native request consumer as successful publication.
pub(super) fn consume_marker(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    manager: &mut Manager,
    id: &str,
    request: &plugin_runtime::EditorRequest,
) -> api::RequestUpdate {
    visual.update(|_, cx| {
        app.read(cx).extensions.clone().update(cx, |owner, cx| {
            let mut state = owner.worker.state.lock().unwrap();
            state.publish_manager(manager);
            state.editor_requests.push((id.into(), request.clone()));
            drop(state);
            owner.poll(cx);
        });
    });
    visual.run_until_parked();
    let result = request.status();
    manager.poll();
    result
}

/// Visible gutter geometry follows the public native range layout, including UTF-8 point zero.
fn gutter_position(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    selector: &'static str,
    offset: usize,
) {
    visual.update(|window, cx| window.draw(cx).clear(cx));
    let gutter = visual
        .debug_bounds(selector)
        .expect("owned icon must be visibly projected");
    let (native, viewport) = visual.update(|_, cx| {
        let owner = app.read(cx);
        let editor = owner.editor.read(cx);
        (
            editor.range_to_bounds(&(offset..offset)).unwrap(),
            editor.input_bounds(),
        )
    });
    assert_eq!(gutter.center().y, native.center().y);
    assert!(viewport.contains(&gutter.center()));
}

/// A point at byte zero survives native prefix input and ordinary Undo without changing the backing file.
#[gpui::test]
#[ignore = "opt-in frozen Package/WASM fixtures; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn community_bookmark_point_zero_tracks_real_input_and_undo(cx: &mut TestAppContext) {
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        let (_root, mut manager) = providers(path.parent().unwrap(), &["bookmarks", "coverage"]);
        visual.simulate_keystrokes("ctrl-home");
        visual.run_until_parked();
        let bookmark = command(visual, &app, &mut manager, "bookmarks", "bookmark-line")
            .pop()
            .unwrap();
        // GPUI's diagnostic selector takes a static name; retain one bounded test identity.
        let selector = Box::leak(
            format!(
                "decoration-{}-{}-bookmark-1",
                bookmark.handle().instance,
                bookmark.handle().resource
            )
            .into_boxed_str(),
        );
        assert_eq!(
            positions(visual, &app, &mut manager, "bookmarks", "read-bookmarks")[0].range,
            Some(api::TextRange { start: 0, end: 0 })
        );
        gutter_position(visual, &app, selector, 0);
        visual.simulate_input("中😀\r\n");
        visual.run_until_parked();
        assert_eq!(
            positions(visual, &app, &mut manager, "bookmarks", "read-bookmarks")[0].range,
            Some(api::TextRange { start: 9, end: 9 })
        );
        gutter_position(visual, &app, selector, 9);
        // Base groups adjacent typing by time. Check each actual native Undo/Redo unit against
        // known UTF-8 prefixes without imposing a second history policy on the plugin.
        for (key, final_offset) in [("ctrl-z", 0), ("ctrl-y", 9)] {
            let mut reached = false;
            for _ in 0..4 {
                assert!(visual.update(|window, cx| {
                    app.read(cx)
                        .editor
                        .read(cx)
                        .focus_handle(cx)
                        .is_focused(window)
                }));
                visual.simulate_keystrokes(key);
                visual.run_until_parked();
                let native_text =
                    visual.update(|_, cx| app.read(cx).editor.read(cx).text().to_string());
                let offset = match native_text.strip_suffix("original on disk").unwrap() {
                    "" => 0,
                    "中" => 3,
                    "中😀" => 7,
                    "中😀\r" => 8,
                    "中😀\r\n" => 9,
                    prefix => panic!("unexpected native history prefix: {prefix:?}"),
                };
                assert_eq!(
                    positions(visual, &app, &mut manager, "bookmarks", "read-bookmarks")[0].range,
                    Some(api::TextRange {
                        start: offset,
                        end: offset
                    }),
                    "native source after {key}: {native_text:?}"
                );
                gutter_position(visual, &app, selector, offset);
                if offset == final_offset {
                    reached = true;
                    break;
                }
            }
            assert!(reached, "native {key} must reach its complete text state");
        }
        // Public positions provide the current version and exact owned handle for admission checks.
        command(visual, &app, &mut manager, "bookmarks", "read-bookmarks");
        let api::EditorValue::Decorations {
            handle, document, ..
        } = serde_json::from_str(&text(&manager, "bookmarks")).unwrap()
        else {
            panic!("current collection expected")
        };
        let operation = |operation| api::Operation::Editor {
            operation: api::EditorOperation::Decorations { operation },
            timeout_ms: 30_000,
        };
        let cross_owner = admit_marker(
            &mut manager,
            "coverage",
            operation(
                plugin_runtime::plugin_protocol::decorations::Operation::Read {
                    handle: handle.clone(),
                },
            ),
        )
        .err()
        .expect("a peer cannot borrow the collection");
        assert_eq!(cross_owner.code, api::ErrorCode::InvalidHandle);
        let invalid = || {
            vec![plugin_runtime::plugin_protocol::decorations::Marker {
                id: "invalid-boundary".into(),
                anchor: plugin_runtime::plugin_protocol::decorations::Anchor::Point {
                    offset: 1,
                    affinity: plugin_runtime::plugin_protocol::decorations::Affinity::Left,
                },
                label: ui::LocalizedText::new("Boundary probe", "边界检查"),
                icon: None,
                style: plugin_runtime::plugin_protocol::decorations::Style::Accent,
            }]
        };
        // More than sixteen failed creates cannot exhaust the persistent collection budget.
        for _ in 0..17 {
            let request = admit_marker(
                &mut manager,
                "bookmarks",
                operation(
                    plugin_runtime::plugin_protocol::decorations::Operation::Create {
                        document: document.clone(),
                        markers: invalid(),
                    },
                ),
            )
            .unwrap();
            let collection = request.decoration().unwrap().clone();
            assert!(matches!(
                consume_marker(visual, &app, &mut manager, "bookmarks", &request),
                api::RequestUpdate::Completed {
                    result: Err(api::Failure {
                        code: api::ErrorCode::InvalidRequest,
                        ..
                    })
                }
            ));
            assert!(
                !collection.is_live(),
                "failed native admission releases its own authority before retry"
            );
        }
        let mut stale = document.clone();
        stale.revision = stale.revision.saturating_sub(1);
        let request = admit_marker(
            &mut manager,
            "bookmarks",
            operation(
                plugin_runtime::plugin_protocol::decorations::Operation::Replace {
                    handle: handle.clone(),
                    document: stale,
                    markers: Vec::new(),
                },
            ),
        )
        .unwrap();
        assert!(matches!(
            consume_marker(visual, &app, &mut manager, "bookmarks", &request),
            api::RequestUpdate::Completed {
                result: Err(api::Failure {
                    code: api::ErrorCode::StaleRevision,
                    ..
                })
            }
        ));
        let request = admit_marker(
            &mut manager,
            "bookmarks",
            operation(
                plugin_runtime::plugin_protocol::decorations::Operation::Create {
                    document: document.clone(),
                    markers: Vec::new(),
                },
            ),
        )
        .unwrap();
        let cancelled = request.decoration().unwrap().clone();
        request.cancel_from_host(api::CancelMode::StopWaiting);
        assert!(matches!(
            consume_marker(visual, &app, &mut manager, "bookmarks", &request),
            api::RequestUpdate::Cancelled {
                effect: api::CancellationEffect::NotExecuted,
                ..
            }
        ));
        assert!(!cancelled.is_live());
        let mut admitted = Vec::new();
        for _ in 0..15 {
            let request = admit_marker(
                &mut manager,
                "bookmarks",
                operation(
                    plugin_runtime::plugin_protocol::decorations::Operation::Create {
                        document: document.clone(),
                        markers: Vec::new(),
                    },
                ),
            )
            .unwrap();
            admitted.push(request.decoration().unwrap().clone());
            assert!(matches!(
                consume_marker(visual, &app, &mut manager, "bookmarks", &request),
                api::RequestUpdate::Completed { result: Ok(_) }
            ));
        }
        assert!(matches!(
            admit_marker(
                &mut manager,
                "bookmarks",
                operation(
                    plugin_runtime::plugin_protocol::decorations::Operation::Create {
                        document,
                        markers: Vec::new()
                    }
                )
            ),
            Err(api::Failure {
                code: api::ErrorCode::LimitExceeded,
                ..
            })
        ));
        // Retained native layers observe retirement without relying on another successful task.
        manager.disable("bookmarks").unwrap();
        assert!(!bookmark.is_live());
        assert!(admitted.iter().all(|resource| !resource.is_live()));
        assert_eq!(std::fs::read_to_string(path).unwrap(), "original on disk");
    });
}
