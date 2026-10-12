//! One cross-feature native combination preserves original versions across delivered capabilities.
use super::*;
use gpui_kit::{TestAppContext, VisualTestContext, gpui};
use plugin_runtime::{
    Manager,
    plugin_protocol::{api, tree, ui, workspace},
};
use serde_json::json;

// Reuse the owned loopback fixture shared with SDK lifecycle verification; no external service
// or test-specific host operation is introduced for the native combination.
#[path = "../../../plugin-runtime/tests/support/lifecycle_http.rs"]
mod endpoint;

/// Only migration declarations vary over the delivered old SDK's real component and assets.
fn migration_package(version: u32, policy: &str) -> Package {
    // Migration keeps the old SDK component intact; only the declared migration policy varies.
    let original = crate::test_packages::package("capability-example-0.19.0");
    let mut files = original.files;
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["version"] = json!(format!("{version}.0.0"));
    manifest["data_format"] = json!({"version":version,"migration_hook":true});
    manifest["api"]["required"]["storage.migration"] = json!("^1");
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    files.insert("migration-policy.txt".into(), policy.as_bytes().to_vec());
    language_tests::packages::repack(files).unwrap()
}

/// Read actual guest text only as a visible command result; native document text stays in EditorState.
fn guest_text(manager: &Manager, plugin: &str) -> String {
    let ui::Kind::Text { text } = &manager.live[plugin]
        .views
        .values()
        .next()
        .unwrap()
        .root
        .kind
    else {
        panic!("actual guest text expected");
    };
    text.clone()
}

/// Advance one public business command and its actual native request state machine.
fn command(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    manager: &mut Manager,
    plugin: &str,
    command: &str,
    arguments: serde_json::Value,
) {
    package_ui_test_support::demand(
        manager,
        plugin,
        plugin_runtime::DemandReason::Command(command.into()),
    )
    .unwrap();
    manager.invoke_command(plugin, command, arguments).unwrap();
    community_language_tests::pump(visual, app, manager);
}

/// Delivered packages share one Manager and one real native editing session. This joins only the
/// cross-feature transitions; detailed failure matrices remain in their individual ticket tests.
#[gpui::test]
#[ignore = "opt-in frozen Package/WASM fixtures; loopback 18906 must be free; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn native_community_activation_combination_retains_source_undo_and_upgrade_recovery(
    cx: &mut TestAppContext,
) {
    crate::tests::with_shortcut_editor(cx, false, vec![], |visual, app, path| {
        let root = path.parent().unwrap();
        std::fs::write(root.join("source.txt"), "workspace").unwrap();
        workspace_transaction_tests::seed(visual, path);
        visual.simulate_keystrokes("ctrl-end");
        visual.simulate_input("\r\nteh\r\ninclude=generated.conf\r\nCOVERED 中文\r\nTODO 行");
        visual.run_until_parked();
        let original = visual.update(|_, cx| app.read(cx).editor.read(cx).text().to_string());
        let original_version = visual.update(|_, cx| {
            app.read(cx)
                .plugin_document_version(app.read(cx).active_text_tab_index().unwrap())
                .unwrap()
        });
        let profile = tempfile::tempdir().unwrap();
        let mut manager = Manager::open(
            profile.path().into(),
            protocol::Environment {
                workspace: root.display().to_string(),
                locale: "en".into(),
                ..Default::default()
            },
        )
        .unwrap();
        for (id, version) in [
            ("history-preview", "0.1.2"),
            ("workspace-scaffold", "0.1.1"),
            ("spell-check", "0.1.0"),
            ("config-check", "0.1.0"),
            ("todo-tree", "0.1.1"),
            ("coverage", "0.1.1"),
            ("cloud-translation", "0.1.2"),
        ] {
            // Every joined consumer comes from its exact prior acceptance bytes, not build output discovery.
            let package = crate::test_packages::package(&format!("{id}-{version}"));
            manager
                .install(&package, package.manifest.permissions.clone())
                .unwrap();
        }
        let old = migration_package(1, "ok");
        manager
            .install(&old, old.manifest.permissions.clone())
            .unwrap();
        assert!(
            manager.live.is_empty(),
            "all installed declarations initially stay cold"
        );
        // This scenario begins with the user's surfaces closed. Later only the actual Todo view
        // is shown, so default-visible layout declarations do not turn unused purposes into demand.
        visual.update(|_, cx| {
            app.update(cx, |app, _| {
                for entry in manager.installed.values() {
                    for panel in &entry.manifest.panels {
                        app.session_state
                            .plugin_panel_visibility
                            .insert(format!("{}/{}", entry.manifest.id, panel.id), false);
                    }
                }
            })
        });
        visual.update(|_, cx| {
            app.read(cx).extensions.clone().update(cx, |owner, _| {
                owner.root = profile.path().into();
            })
        });
        let api::EditorValue::DocumentSnapshot(snapshot) = community_document_tests::invoke_for(
            visual,
            &app,
            &mut manager,
            "history-preview",
            json!({"kind":"read_document","document":original_version}),
        )
        .unwrap() else {
            panic!("original native snapshot expected");
        };
        assert_eq!(snapshot.text, original);
        assert_eq!(snapshot.info.document, original_version);
        assert_eq!(std::fs::read_to_string(path).unwrap(), "original on disk");
        for checker in ["spell-check", "config-check"] {
            command(visual, &app, &mut manager, checker, "check", json!(null));
        }
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                assert_eq!(
                    app.tabs[app.active_text_tab_index().unwrap()]
                        .text
                        .as_ref()
                        .unwrap()
                        .diagnostics
                        .direct
                        .len(),
                    2
                );
                app.show_problems(window, cx);
            })
        });
        visual.run_until_parked();
        visual.update(|window, cx| window.draw(cx).clear(cx));
        assert!(visual.debug_bounds("native-problems-list").is_some());
        visual.simulate_keystrokes("escape");
        visual.run_until_parked();
        let preview = workspace_transaction_tests::scaffold_preview(visual, &app, &mut manager);
        let api::EditorOperation::Workspace {
            operation: workspace::Operation::ApplyPlan { plan },
        } = preview.operation()
        else {
            panic!("native mixed preview expected");
        };
        assert_eq!(plan.changes.len(), 8);
        assert!(plan.changes.iter().any(|change| matches!(change,
            workspace::Change::EditDocument { document, .. } if document == &original_version)));
        assert_eq!(
            visual.update(|_, cx| app.read(cx).editor.read(cx).text().to_string()),
            original
        );
        assert!(!root.join("community-generated").exists());
        workspace_transaction_tests::confirm(visual);
        let applied = workspace_transaction_tests::batch(&preview);
        assert_eq!(applied.state, workspace::BatchState::Complete);
        assert_eq!(applied.completed.len(), 8);
        assert!(root.join("community-generated/final.txt").exists());
        assert_eq!(
            workspace_transaction_tests::undo(
                visual,
                &app,
                &mut manager,
                "workspace-scaffold",
                applied.batch
            )
            .state,
            workspace::BatchState::Undone
        );
        assert_eq!(
            visual.update(|_, cx| app.read(cx).editor.read(cx).text().to_string()),
            original
        );
        assert!(!root.join("community-generated").exists());
        assert_eq!(
            std::fs::read_to_string(root.join("community-disk.txt")).unwrap(),
            "磁盘旧A🙂\r\n旧B\r\n"
        );
        community_presentation_tests::events(visual, &app, &mut manager);
        community_presentation_tests::command(
            visual,
            &app,
            &mut manager,
            "todo-tree",
            "refresh-todos",
        );
        let collection = community_presentation_tests::command(
            visual,
            &app,
            &mut manager,
            "coverage",
            "scan-coverage",
        )
        .pop()
        .unwrap();
        let positions = community_presentation_tests::positions(
            visual,
            &app,
            &mut manager,
            "coverage",
            "read-coverage",
        );
        assert_eq!(
            &original[positions[0].range.unwrap().start..positions[0].range.unwrap().end],
            "COVERED 中文"
        );
        let tree = manager.live["todo-tree"].tree_resources().pop().unwrap();
        let load = |manager: &mut Manager, parent| {
            if let Some(request) = tree.request(parent, false).unwrap() {
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
            }
        };
        load(&mut manager, None);
        let document_node = tree.snapshot().branches[&None].children[0].clone();
        load(&mut manager, Some(document_node));
        assert!(
            tree.snapshot()
                .nodes
                .values()
                .any(|node| node.label.contains("TODO 行"))
        );
        let mut renderer = images::VectorRenderer::default();
        composable_tests::publish(&mut manager, &mut renderer, &app, visual);
        visual.update(|window, cx| {
            let panel = app.read(cx).plugin_panels["todo-tree/presentation"].clone();
            panel.update(cx, |panel, cx| {
                panel.show(window, cx);
                cx.notify();
            });
        });
        for _ in 0..2 {
            composable_tests::pump(&mut manager, &app, visual);
            composable_tests::publish(&mut manager, &mut renderer, &app, visual);
        }
        assert!(
            visual.debug_bounds("plugin-ui-todo-tree").is_some(),
            "actual tree must project natively"
        );
        let server = endpoint::Server::new(18906);
        // The frozen restored source is carried across demand as a payload, never sampled on resume.
        command(
            visual,
            &app,
            &mut manager,
            "cloud-translation",
            "query",
            json!({"url":"http://127.0.0.1:18906/stall","text":snapshot.text}),
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while server.count() == 0 {
            assert!(
                Instant::now() < deadline,
                "real cloud stream did not reach the owned endpoint"
            );
            manager.poll();
            std::thread::sleep(Duration::from_millis(5));
        }
        command(
            visual,
            &app,
            &mut manager,
            "cloud-translation",
            "cancel",
            json!(null),
        );
        assert_eq!(guest_text(&manager, "cloud-translation"), "Cancelled");
        for id in [
            "history-preview",
            "workspace-scaffold",
            "spell-check",
            "config-check",
            "todo-tree",
            "coverage",
            "cloud-translation",
        ] {
            manager.disable(id).unwrap();
        }
        composable_tests::publish(&mut manager, &mut renderer, &app, visual);
        assert!(!tree.is_live() && !collection.is_live());
        assert_eq!(
            manager.resource_count(),
            0,
            "cold capability metadata owns no live resources"
        );
        command(
            visual,
            &app,
            &mut manager,
            "capability-example",
            "scope-write",
            json!({"text":"private"}),
        );
        let original_digest = manager.installed["capability-example"].digest.clone();
        let mut files = migration_package(2, "ok").files;
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&files["manifest.json"]).unwrap();
        manifest["permissions"]
            .as_array_mut()
            .unwrap()
            .push(json!("network.http"));
        manifest["api"]["required"]["network.http"] = json!("^1");
        files.insert(
            "manifest.json".into(),
            serde_json::to_vec(&manifest).unwrap(),
        );
        let additional = language_tests::packages::repack(files).unwrap();
        assert!(
            manager
                .install(&additional, old.manifest.permissions.clone())
                .is_err(),
            "new permission refusal must retain the old package"
        );
        assert_eq!(
            manager.installed["capability-example"].digest,
            original_digest
        );
        let control = plugin_runtime::InstallControl::default();
        let failure = migration_package(2, "activate-fail");
        let prepared = manager
            .prepare_installation(&failure, failure.manifest.permissions.clone(), &control)
            .unwrap();
        command(
            visual,
            &app,
            &mut manager,
            "capability-example",
            "scope-write",
            json!({"text":"latest"}),
        );
        assert!(manager.commit_installation(prepared, &control).is_err());
        command(
            visual,
            &app,
            &mut manager,
            "capability-example",
            "scope-read",
            json!(null),
        );
        assert!(guest_text(&manager, "capability-example").ends_with("|latest"));
        assert_eq!(
            manager.installed["capability-example"].digest,
            original_digest
        );
        assert_eq!(
            visual.update(|_, cx| app.read(cx).editor.read(cx).text().to_string()),
            original
        );
        assert_eq!(std::fs::read_to_string(path).unwrap(), "original on disk");
        manager.disable("capability-example").unwrap();
        manager.shutdown();
        assert_eq!(manager.resource_count(), 0);
    });
}
