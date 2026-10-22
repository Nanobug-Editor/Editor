//! Frozen public WASM requests exercise the actual host save queue and durable history acknowledgement.

use super::*;
use crate::editor::history::tests::with_history_editor;
use plugin_runtime::plugin_protocol::api::{self, EditorOperation, EditorValue, RequestUpdate};

/// The package owns the typed request; the normal worker publication routes it to the real document.
#[gpui::test]
#[ignore = "opt-in frozen capability-example Package/WASM; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn history_public_wasm_save_records_the_exact_successfully_written_bytes(cx: &mut TestAppContext) {
    with_history_editor(cx, false, |visual, app, path| {
        let package =
            plugin_runtime::Package::read(&crate::test_packages::archive("capability-example"))
                .unwrap();
        let runtime = tempfile::tempdir().unwrap();
        let mut manager = plugin_runtime::Manager::open(
            runtime.path().to_path_buf(),
            plugin_runtime::plugin_protocol::Environment {
                workspace: path.parent().unwrap().display().to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        crate::extensions::demand_plugin_fixture(
            &mut manager,
            &package.manifest.id,
            plugin_runtime::DemandReason::Command("scope-probe".into()),
        )
        .unwrap();
        visual.simulate_input("plugin ");
        visual.run_until_parked();
        let document = visual.update(|_, cx| app.read(cx).plugin_document_version(0).unwrap());
        manager
            .invoke_command(
                &package.manifest.id,
                "scope-probe",
                serde_json::to_value(api::Operation::Editor {
                    operation: EditorOperation::SaveDocument { document },
                    timeout_ms: 30_000,
                })
                .unwrap(),
            )
            .unwrap();
        let save = manager
            .live
            .get_mut(&package.manifest.id)
            .unwrap()
            .take_editor_requests()
            .pop()
            .unwrap();
        visual.update(|_, cx| {
            let extension = app.read(cx).extensions.clone();
            extension.update(cx, |owner, cx| {
                let mut state = owner.worker.state.lock().unwrap();
                state.entries = manager.published_entries();
                state
                    .editor_requests
                    .push((package.manifest.id.clone(), save.clone()));
                drop(state);
                owner.poll(cx);
            });
        });
        visual.run_until_parked();
        assert!(
            matches!(
                save.status(),
                RequestUpdate::Completed {
                    result: Ok(EditorValue::Saved { .. })
                }
            ),
            "{:?}",
            save.status()
        );
        let root = visual.update(|_, cx| app.read(cx).history.paths.as_ref().unwrap().root.clone());
        let history =
            editor_history::History::open(root, editor_history::Policy::default()).unwrap();
        assert!(
            history
                .list(&editor_history::Query::default())
                .unwrap()
                .into_iter()
                .any(|row| {
                    row.reason == editor_history::Reason::Saved
                        && history.read(row.id).unwrap() == b"plugin original on disk"
                })
        );
        assert_eq!(std::fs::read(path).unwrap(), b"plugin original on disk");
        assert!(!visual.update(|_, cx| app.read(cx).tabs[0].is_dirty()));
    });
}
