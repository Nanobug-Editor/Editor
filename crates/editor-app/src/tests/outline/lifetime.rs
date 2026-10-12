//! A real independent WASM failure revokes the current native tree without a subsequent user event.
//! Mounted under the model's tests so completion ownership can be inspected before any final paint.
use crate::extensions::lsp_tests::publish;
use crate::tests::wait_outline_node;
use crate::*;
use gpui_kit::{TestAppContext, gpui};
use std::{
    cell::{Cell, RefCell},
    io::{Cursor, Write},
    rc::Rc,
    time::{Duration, Instant},
};

/// Add recognition to the independently SDK-built hostile guest; no fake LSP or grammar is needed for structure.
fn hostile_package() -> plugin_runtime::Package {
    let original =
        plugin_runtime::Package::read(&crate::test_packages::archive("capability-example"))
            .unwrap();
    let mut files = original.files;
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["structure_providers"] =
        serde_json::json!([{"id":"definitions", "language":"unfamiliar-language"}]);
    manifest["api"]["required"]["language.structure"] = serde_json::json!("^1");
    manifest["api"]["optional"]
        .as_object_mut()
        .unwrap()
        .remove("language.structure");
    manifest["contributions"] = serde_json::json!("plugin.toml");
    files.insert("plugin.toml".into(), format!("[plugin]\nid = \"capability-example\"\nname = \"Pure hostile structure\"\nversion = \"{}\"\nhost_version = \">=0.1.0\"\n[[language_definitions]]\nid = \"unfamiliar-language\"\nname = \"Pure structure\"\nextensions = [\"pure\"]\n", original.manifest.version).into_bytes());
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in files {
        archive
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        archive.write_all(&bytes).unwrap();
    }
    plugin_runtime::Package::from_bytes(&archive.finish().unwrap().into_inner()).unwrap()
}

/// Completion itself must clear its revoked target and request a paint, rather than wait for another editor operation.
#[gpui::test]
#[ignore = "opt-in real WASM/native integration; uses fixed reviewed test packages and required pinned native tools"]
fn native_structure_trap_clears_tree_without_another_document_event(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        cx.set_reduce_motion(true);
    });
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("document.pure");
    std::fs::write(&path, "healthy source").unwrap();
    let package = hostile_package();
    let workspace = Workspace::open(directory.path()).unwrap();
    let mut manager = plugin_runtime::Manager::open(
        workspace.root().join(".runtime-plugin-test"),
        plugin_runtime::plugin_protocol::Environment {
            workspace: workspace.root().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let app = cx.new(|cx| EditorApp::new(workspace, None, window, cx));
        *capture.borrow_mut() = Some(app.clone());
        Root::new(app, window, cx)
    });
    let app = slot.borrow_mut().take().unwrap();
    publish(&app, &mut manager, visual);
    visual.update(|window, cx| app.update(cx, |app, cx| app.open_file(path.clone(), window, cx)));
    wait_outline_node(visual, &app, &mut manager, "outline-node-/0");
    // Retain the actual healthy lease so a later catalog replacement cannot stand in for its retirement.
    let provider = manager.structure_providers()["capability-example/definitions"]
        .as_ref()
        .unwrap()
        .clone();
    assert!(provider.is_active());
    let original = visual.update(|_, cx| {
        let app = app.read(cx);
        app.plugin_document_version(app.active_text_tab_index().unwrap())
            .unwrap()
    });
    visual.update(|window, cx| {
        app.read(cx).editor.clone().update(cx, |editor, cx| {
            editor.set_selected_range(0..editor.text().len(), cx);
            editor.focus(window, cx);
        })
    });
    visual.simulate_input("trap");
    // Prove native input changed this document before investigating a guest or completion failure.
    let changed = visual.update(|_, cx| {
        let app = app.read(cx);
        assert_eq!(app.editor.read(cx).text().to_string(), "trap");
        app.plugin_document_version(app.active_text_tab_index().unwrap())
            .unwrap()
    });
    assert_eq!(changed.id, original.id);
    assert_eq!(changed.path, original.path);
    assert!(changed.revision > original.revision);
    // Each new source version needs a receipt even for a warm provider. The test Worker records
    // messages instead of running an actor, so deliver this original demand before description.
    // Only this launch phase pumps/draws; the later trap must clear itself without another event.
    visual.update(|window, cx| window.draw(cx).clear(cx));
    publish(&app, &mut manager, visual);
    assert!(std::sync::Arc::ptr_eq(
        &provider,
        manager.structure_providers()["capability-example/definitions"]
            .as_ref()
            .unwrap()
    ));
    // The initial version transition already clears the old tree. Observe the window's real
    // application notification only after this original request's target and task are retired;
    // an extra paint must not repair a missing completion cleanup or notification.
    let completion_notified = Rc::new(Cell::new(false));
    let _completion_subscription = visual.update(|window, cx| {
        let notified = completion_notified.clone();
        let expected = changed.clone();
        let provider = provider.clone();
        window.observe(&app, cx, move |app, _, cx| {
            let app = app.read(cx);
            let current = app
                .active_text_tab_index()
                .and_then(|index| app.plugin_document_version(index).ok());
            if current.as_ref() == Some(&expected)
                && !provider.is_active()
                && app.outline.target.is_none()
                && app.outline.pending.is_none()
                && app.outline.tree.read(cx).entry(0).is_none()
            {
                notified.set(true);
            }
        })
    });
    visual.update(|window, cx| window.draw(cx).clear(cx));
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        visual.executor().advance_clock(Duration::from_millis(10));
        visual.run_until_parked();
        let (empty, target_clear, pending_clear, current) = visual.update(|_, cx| {
            let app = app.read(cx);
            (
                app.outline.tree.read(cx).entry(0).is_none(),
                app.outline.target.is_none(),
                app.outline.pending.is_none(),
                app.active_text_tab_index()
                    .and_then(|index| app.plugin_document_version(index).ok()),
            )
        });
        let active = provider.is_active();
        let notified = completion_notified.get();
        // Do not force another frame while the callback is running: its own notification must revoke this target.
        if empty
            && target_clear
            && pending_clear
            && !active
            && current.as_ref() == Some(&changed)
            && notified
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "trapped structure did not revoke and notify its native outline: empty={empty}; target_clear={target_clear}; pending_clear={pending_clear}; provider_active={active}; notified={notified}; expected={changed:?}; current={current:?}; actual_text={:?}",
            visual.update(|_, cx| app.read(cx).editor.read(cx).text().to_string())
        );
        std::thread::sleep(Duration::from_millis(15));
    }
    visual.update(|window, cx| window.draw(cx).clear(cx));
    assert!(visual.debug_bounds("outline-empty").is_some());
    assert!(visual.debug_bounds("outline-node-/0").is_none());
    assert!(
        visual.debug_bounds("outline-loading").is_none(),
        "a revoked callback must not leave the panel indefinitely loading"
    );
    visual.update(|_, cx| assert_eq!(app.read(cx).editor.read(cx).text().to_string(), "trap"));
}
