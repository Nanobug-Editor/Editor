//! A real Package enters the native editor's existing publication seam without replacing its process or document.
use super::*;
use gpui_kit::{TestAppContext, VisualTestContext, gpui};
use plugin_runtime::Manager;
mod git;

/// Publish the real manager through the same receipt path as the actor; never synthesize guest views.
fn publish(manager: &mut Manager, app: &Entity<EditorApp>, visual: &mut VisualTestContext) {
    visual.update(|window, cx| {
        app.read(cx).extensions.clone().update(cx, |owner, cx| {
            let mut state = owner.worker.state.lock().unwrap();
            state.publish_manager(manager);
            state.views = manager
                .live
                .iter()
                .flat_map(|(id, live)| {
                    live.views
                        .iter()
                        .map(move |(panel, view)| (format!("{id}/{panel}"), view.clone()))
                })
                .collect();
            drop(state);
            owner.poll(cx);
        });
        app.update(cx, |app, cx| app.sync_plugin_panels(window, cx));
        window.draw(cx).clear(cx);
    });
    visual.update(|window, cx| window.draw(cx).clear(cx));
}

/// Installation, rejected replacement and disable leave the original unsaved document alive.
#[gpui::test]
#[ignore = "real independently built WASM Package; set NANOBUG_TOOLING_FIXTURES per plugin developer tooling verification"]
fn developer_tooling_hot_install_preserves_process_and_unsaved_document(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let fixture = PathBuf::from(
        std::env::var_os("NANOBUG_TOOLING_FIXTURES").expect("explicit fixture directory required"),
    );
    let bytes = std::fs::read(fixture.join("lifecycle-editor-0.1.0.zip")).unwrap();
    let ledger: serde_json::Value = serde_json::from_str(include_str!(
        "../../../plugin-runtime/tests/fixtures/lifecycle-inputs.json"
    ))
    .unwrap();
    let expected = ledger["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|input| input["archive"] == "lifecycle-editor-0.1.0.zip")
        .unwrap();
    use sha2::Digest as _;
    assert_eq!(
        format!("{:x}", sha2::Sha256::digest(&bytes)),
        expected["sha256"].as_str().unwrap()
    );
    let package = Package::from_bytes(&bytes).unwrap();
    let process = std::process::id();
    crate::tests::with_shortcut_editor(cx, false, vec![], |visual, app, path| {
        let mut manager = Manager::open(
            directory.path().join("runtime"),
            protocol::Environment {
                workspace: path.parent().unwrap().display().to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        visual.simulate_keystrokes("ctrl-a");
        visual.simulate_input("未保存的编辑 · lifecycle");
        let original_editor = visual.update(|_, cx| app.read(cx).editor.clone());
        let text = visual.update(|_, cx| original_editor.read(cx).text().to_string());
        assert_eq!(text, "未保存的编辑 · lifecycle");
        assert_ne!(std::fs::read_to_string(path).unwrap(), text);

        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        assert!(
            !manager.live.contains_key(&package.manifest.id),
            "install is cold"
        );
        crate::extensions::demand_plugin_fixture(
            &mut manager,
            &package.manifest.id,
            plugin_runtime::DemandReason::Command("query".into()),
        )
        .unwrap();
        publish(&mut manager, &app, visual);
        assert!(
            visual.debug_bounds("plugin-ui-receipt").is_some(),
            "new guest panel renders in the existing editor"
        );
        let old_view = manager.live[&package.manifest.id].views["result"].clone();

        // Static validation accepts the current manifest, then candidate preparation rejects invalid WASM.
        let mut files = package.files.clone();
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&files["manifest.json"]).unwrap();
        manifest["version"] = "0.1.1".into();
        files.insert(
            "manifest.json".into(),
            serde_json::to_vec(&manifest).unwrap(),
        );
        files.insert(
            package.manifest.component.clone().unwrap(),
            b"\0asm\x0d\0\x01\0invalid component".to_vec(),
        );
        let candidate = Package::from_files(files).unwrap();
        assert!(
            manager
                .install(&candidate, candidate.manifest.permissions.clone())
                .is_err()
        );
        assert!(Arc::ptr_eq(
            &manager.live[&package.manifest.id].views["result"],
            &old_view
        ));
        publish(&mut manager, &app, visual);
        assert!(
            visual.debug_bounds("plugin-ui-receipt").is_some(),
            "failed candidate keeps old publication"
        );

        manager.disable(&package.manifest.id).unwrap();
        publish(&mut manager, &app, visual);
        assert!(visual.debug_bounds("plugin-ui-receipt").is_none());
        visual.update(|_, cx| {
            assert_eq!(std::process::id(), process);
            assert_eq!(app.read(cx).editor, original_editor);
            assert_eq!(original_editor.read(cx).text().to_string(), text);
        });
        assert_ne!(std::fs::read_to_string(path).unwrap(), text);
        manager.shutdown_checked().unwrap();
    });
}
