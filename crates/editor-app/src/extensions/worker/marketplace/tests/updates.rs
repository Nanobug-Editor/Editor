//! Update regressions enter through real HTTP ZIP bytes and the production installation actor.
use super::*;

/// Repacking identical file contents with different ZIP metadata produces different reviewed bytes.
pub(super) fn zip_bytes(files: &BTreeMap<String, Vec<u8>>, day: u8) -> Vec<u8> {
    let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .last_modified_time(zip::DateTime::from_date_and_time(2026, 10, day, 0, 0, 0).unwrap());
    for (name, bytes) in files {
        archive.start_file(name, options).unwrap();
        archive.write_all(bytes).unwrap();
    }
    archive.finish().unwrap().into_inner()
}

/// Upgrade gates and failures keep the current identity and private data until a real commit succeeds.
#[test]
fn market_upgrades_require_fresh_review_and_exact_selected_zip_bytes() {
    let old = Package::from_bytes(include_bytes!(
        "../../../../../../plugin-runtime/tests/fixtures/marketplace/notes.zip"
    ))
    .unwrap();
    let mut files = old.files.clone();
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["version"] = "2.0.0".into();
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    files.insert(
        "plugin.toml".into(),
        String::from_utf8(files["plugin.toml"].clone())
            .unwrap()
            .replace("1.0.0", "2.0.0")
            .into_bytes(),
    );
    let reviewed_bytes = zip_bytes(&files, 10);
    let candidate_package = Package::from_bytes(&reviewed_bytes).unwrap();
    let replacement_bytes = zip_bytes(&files, 11);
    assert_ne!(
        candidate_package.digest,
        Package::from_bytes(&replacement_bytes).unwrap().digest
    );

    for scenario in ["replacement", "cancel", "offline", "withdrawn", "success"] {
        let mut catalog: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../../../../../../plugin-runtime/tests/fixtures/marketplace/catalog.json"
        ))
        .unwrap();
        let release = &mut catalog["plugins"][0]["versions"][0];
        release["version"] = "2.0.0".into();
        release["manifest"] = serde_json::to_value(&candidate_package.manifest).unwrap();
        release["sha256"] = candidate_package.digest.clone().into();
        if scenario == "withdrawn" {
            release["withdrawn"] = true.into();
            release["withdrawal_reason"] = "Author withdrew this release".into();
        }
        let server = Server::from_bytes(
            &serde_json::to_vec(&catalog).unwrap(),
            if scenario == "replacement" {
                replacement_bytes.clone()
            } else {
                reviewed_bytes.clone()
            },
        );
        server.pause.store(scenario == "cancel", Ordering::Release);
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join("runtime");
        let environment = Environment {
            workspace: home.path().display().to_string(),
            ..Default::default()
        };
        let mut manager = Manager::open(root.clone(), environment.clone()).unwrap();
        manager
            .install(&old, old.manifest.permissions.clone())
            .unwrap();
        let private = manager.data_directory(&old.manifest.id);
        std::fs::create_dir_all(&private).unwrap();
        std::fs::write(private.join("sentinel.txt"), "keep my data").unwrap();
        manager.shutdown();
        drop(manager);
        let worker = Worker::start_background(root.clone(), environment, true);
        wait(&worker, |state| state.ready);
        let cache = root.join("marketplace/catalog.json");
        worker.refresh_market(cache.clone(), format!("{}/catalog.json", server.url));
        wait(&worker, |state| state.market.fresh());
        let (candidate, generation) = {
            let state = worker.state.lock().unwrap();
            (
                state.market.catalog.as_ref().unwrap().plugins[0].versions[0].clone(),
                state.market.generation(),
            )
        };
        if scenario == "offline" {
            server.offline.store(true, Ordering::Release);
            worker.refresh_market(cache, format!("{}/catalog.json", server.url));
            wait(&worker, |state| state.market.error.is_some());
        }
        let accepted = worker.install_market(candidate.clone(), generation, Default::default());
        assert_eq!(
            accepted,
            !matches!(scenario, "offline" | "withdrawn"),
            "{scenario}"
        );
        if scenario == "cancel" {
            wait(&worker, |_| server.downloads.load(Ordering::Acquire) > 0);
            worker.cancel_installation();
        }
        if accepted {
            wait(&worker, |state| state.progress.is_none());
        }
        let installed = Manager::read_registry(&root).unwrap();
        assert_eq!(
            installed["test.notes"].manifest.version,
            if scenario == "success" {
                "2.0.0"
            } else {
                "1.0.0"
            },
            "{scenario}"
        );
        assert_eq!(
            std::fs::read_to_string(private.join("sentinel.txt")).unwrap(),
            "keep my data",
            "{scenario}"
        );
        if scenario == "replacement" {
            assert!(
                worker
                    .state
                    .lock()
                    .unwrap()
                    .status
                    .as_ref()
                    .unwrap()
                    .message
                    .contains("SHA-256")
            );
        }
        if scenario == "success" {
            // Advisory statistics fail on this fixture endpoint; a fresh catalog still authorizes updates.
            wait(&worker, |state| state.market_statistics_error.is_some());
            assert!(worker.state.lock().unwrap().market.fresh());
            assert!(
                !worker.install_market(candidate, generation, Default::default()),
                "same-version reinstall must be rejected"
            );
        }
        let (tx, rx) = futures::channel::oneshot::channel();
        worker.tx.send(Work::Shutdown(Some(tx))).unwrap();
        let _ = futures::executor::block_on(rx);
    }
}

/// Native clicks preserve an update dot until commit without repeating unchanged permission grants.
#[gpui_kit::gpui::test]
fn market_update_dot_and_permission_delta_follow_real_native_actions(
    cx: &mut gpui_kit::TestAppContext,
) {
    let old = Package::from_bytes(include_bytes!(
        "../../../../../../plugin-runtime/tests/fixtures/marketplace/notes.zip"
    ))
    .unwrap();
    let catalog =
        include_bytes!("../../../../../../plugin-runtime/tests/fixtures/marketplace/catalog.json");
    for (locale, dark) in [("en", true), ("zh-CN", false)] {
        native_update(cx, &old, catalog, false, None, locale, dark);
    }
}

/// New executable grants are tested with an actual SDK guest, since declarative packages cannot request them.
#[gpui_kit::gpui::test]
#[ignore = "build capability-example with --plugin-package and export target/marketplace-wasm via marketplace tests.export_wasm_fixture"]
fn market_wasm_update_confirms_only_new_permissions(cx: &mut gpui_kit::TestAppContext) {
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/marketplace-wasm");
    let old = Package::read(&fixture.join("notes.zip")).expect("export current SDK guest");
    let catalog = std::fs::read(fixture.join("catalog.json")).unwrap();
    native_update(cx, &old, &catalog, true, None, "zh-CN", false);
}

/// A package replacement changes another installation even when the candidate adds no permission.
#[gpui_kit::gpui::test]
#[ignore = "build capability-example and export target/marketplace-wasm via marketplace tests.export_wasm_fixture"]
fn market_wasm_replacement_requires_visible_consent_without_new_grants(
    cx: &mut gpui_kit::TestAppContext,
) {
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/marketplace-wasm");
    let source = Package::read(&fixture.join("notes.zip")).unwrap();
    let mut files = source.files.clone();
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["api"]["required"]["dependencies"] = "^1.2".into();
    manifest["api"]["required"]["plugin.services"] = "^1.1".into();
    manifest["plugin_services"] = serde_json::json!({"provides":{"example.service":{
        "version":"1.0.0", "enabled_setting":"enabled",
        "methods":{"ping":{"parameters":{"type":"null"},"result":{"type":"null"}}}
    }}});
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    let old = Package::from_bytes(&zip_bytes(&files, 10)).unwrap();
    manifest["id"] = "previous-provider".into();
    manifest["version"] = "1.0.0".into();
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    let replaced = Package::from_bytes(&zip_bytes(&files, 10)).unwrap();
    let catalog = std::fs::read(fixture.join("catalog.json")).unwrap();
    native_update(cx, &old, &catalog, false, Some(&replaced), "en", true);
}

/// Each scenario owns real installed records and a window; no published success state is substituted.
fn native_update(
    cx: &mut gpui_kit::TestAppContext,
    old: &Package,
    catalog_bytes: &[u8],
    extra_permission: bool,
    replaced: Option<&Package>,
    locale: &str,
    dark: bool,
) {
    use crate::*;
    use gpui_kit::VisualTestContext;
    let previous = rust_i18n::locale().to_string();
    rust_i18n::set_locale(locale);
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(dark), cx);
        cx.set_reduce_motion(true);
    });
    let id = old.manifest.id.clone();
    let mut files = old.files.clone();
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["version"] = "2.0.0".into();
    if extra_permission {
        manifest["permissions"]
            .as_array_mut()
            .unwrap()
            .push("workspace.write".into());
    }
    if let Some(replaced) = replaced {
        manifest["replaces"] = serde_json::json!([{"id":replaced.manifest.id, "versions":"^1",
            "contracts":["example.service"],"enabled_setting":"enabled"}]);
    }
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    // Pure WASM guests need no declarative descriptor; keep one in sync only when the package has it.
    if let Some(descriptor) = files.get_mut("plugin.toml") {
        *descriptor = String::from_utf8(descriptor.clone())
            .unwrap()
            .replace(&old.manifest.version, "2.0.0")
            .into_bytes();
    }
    let bytes = zip_bytes(&files, 10);
    let package = Package::from_bytes(&bytes).unwrap();
    let mut catalog: serde_json::Value = serde_json::from_slice(catalog_bytes).unwrap();
    let version = &mut catalog["plugins"][0]["versions"][0];
    version["version"] = "2.0.0".into();
    version["manifest"] = serde_json::to_value(&package.manifest).unwrap();
    version["sha256"] = package.digest.clone().into();
    // A different row sorts first after clearing filters, exposing accidental loss of the routed identity.
    let mut other = catalog["plugins"][0].clone();
    other["id"] = "aaa.other".into();
    other["versions"][0]["manifest"]["id"] = "aaa.other".into();
    catalog["plugins"].as_array_mut().unwrap().push(other);
    let server = Server::from_bytes(&serde_json::to_vec(&catalog).unwrap(), bytes);
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join(".runtime-plugin-test");
    let environment = Environment {
        workspace: home.path().display().to_string(),
        ..Default::default()
    };
    let mut manager = Manager::open(root.clone(), environment.clone()).unwrap();
    manager
        .install(old, old.manifest.permissions.clone())
        .unwrap();
    if let Some(replaced) = replaced {
        manager
            .install(replaced, replaced.manifest.permissions.clone())
            .unwrap();
        let files =
            Manager::persisted_data_directory(&root, &replaced.manifest, &environment.workspace)
                .unwrap();
        std::fs::create_dir_all(&files).unwrap();
        std::fs::write(files.join("retain.bin"), b"original replacement owner").unwrap();
    }
    manager.shutdown();
    drop(manager);
    let worker = Arc::new(Worker::start_background(root.clone(), environment, true));
    wait(&worker, |state| state.ready);
    let cache = root.join("marketplace/catalog.json");
    worker.refresh_market(cache.clone(), format!("{}/catalog.json", server.url));
    wait(&worker, |state| state.market.fresh());
    let workspace = Workspace::open(home.path()).unwrap();
    let slot = Rc::new(std::cell::RefCell::new(None));
    let capture = slot.clone();
    let (_, editor) = cx.add_window_view(move |window, cx| {
        let app = cx.new(|cx| EditorApp::new(workspace, None, window, cx));
        *capture.borrow_mut() = Some(app.clone());
        Root::new(app, window, cx)
    });
    let app = slot.borrow_mut().take().unwrap();
    let panel = editor.update(|_, cx| {
        let panel = app.read(cx).extensions.clone();
        panel.update(cx, |panel, cx| {
            panel.worker = worker.clone();
            panel.poll(cx);
        });
        panel
    });
    editor.update(|window, cx| window.draw(cx).clear(cx));
    assert!(editor.debug_bounds("extensions-update-dot").is_some());
    server.offline.store(true, Ordering::Release);
    worker.refresh_market(cache.clone(), format!("{}/catalog.json", server.url));
    wait(&worker, |state| state.market.error.is_some());
    editor.update(|_, cx| panel.update(cx, |panel, cx| panel.poll(cx)));
    editor.update(|window, cx| window.draw(cx).clear(cx));
    assert!(
        editor.debug_bounds("extensions-update-dot").is_some(),
        "failed check retains last result"
    );
    server.offline.store(false, Ordering::Release);
    worker.refresh_market(cache, format!("{}/catalog.json", server.url));
    wait(&worker, |state| state.market.fresh());
    let before = editor.update(|_, cx| cx.windows());
    editor.update(|window, cx| {
        panel.update(cx, |panel, cx| {
            panel.manager_market = false;
            panel.poll(cx);
        });
        app.update(cx, |app, cx| app.toggle_extensions(window, cx));
    });
    let handle = editor
        .update(|_, cx| cx.windows())
        .into_iter()
        .find(|window| !before.contains(window))
        .unwrap();
    let form = VisualTestContext::from_window(handle, editor).into_mut();
    form.run_until_parked();
    form.update(|window, cx| window.draw(cx).clear(cx));
    assert!(
        form.update(|_, cx| panel.read(cx).has_market_updates()),
        "viewing management does not clear the dot"
    );
    form.update(|window, cx| {
        let input = panel.read(cx).manager_search.clone().unwrap();
        input.update(cx, |input, cx| {
            input.set_value(old.manifest.name.clone(), window, cx)
        });
    });
    form.run_until_parked();
    form.update(|window, cx| window.draw(cx).clear(cx));
    let history = form.debug_bounds("plugin-market-history").unwrap();
    form.simulate_click(history.center(), Default::default());
    form.run_until_parked();
    form.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(
        form.update(|_, cx| panel.read(cx).manager_selected.clone()),
        Some(id.clone())
    );
    let viewport = form.debug_bounds("online-market-details").unwrap();
    form.simulate_event(gpui_kit::ScrollWheelEvent {
        position: viewport.center(),
        delta: gpui_kit::ScrollDelta::Pixels(point(px(0.), px(-350.))),
        touch_phase: gpui_kit::TouchPhase::Moved,
        modifiers: Default::default(),
    });
    form.run_until_parked();
    form.update(|window, cx| window.draw(cx).clear(cx));
    let install = form.debug_bounds("market-install").unwrap();
    form.simulate_click(install.center(), Default::default());
    form.run_until_parked();
    form.update(|window, cx| window.draw(cx).clear(cx));
    if extra_permission || replaced.is_some() {
        assert_eq!(server.downloads.load(Ordering::Acquire), 0);
        if replaced.is_some() {
            assert!(form.debug_bounds("market-consent-replacements").is_some());
        }
        let cancel = form
            .debug_bounds("market-consent-cancel")
            .expect("new grant requires consent");
        form.simulate_click(cancel.center(), Default::default());
        form.run_until_parked();
        form.update(|window, cx| window.draw(cx).clear(cx));
        assert_eq!(
            Manager::read_registry(&root).unwrap()[&id].manifest.version,
            old.manifest.version
        );
        if let Some(replaced) = replaced {
            assert!(
                Manager::read_registry(&root).unwrap()[&replaced.manifest.id]
                    .replaced_by
                    .is_none()
            );
        }
        let install = form.debug_bounds("market-install").unwrap();
        form.simulate_click(install.center(), Default::default());
        form.run_until_parked();
        form.update(|window, cx| window.draw(cx).clear(cx));
        let confirm = form.debug_bounds("confirm-market-install").unwrap();
        form.simulate_click(confirm.center(), Default::default());
    } else {
        assert!(
            form.debug_bounds("confirm-market-install").is_none(),
            "unchanged grants must not prompt again"
        );
    }
    wait(&worker, |state| state.progress.is_none());
    form.update(|_, cx| panel.update(cx, |panel, cx| panel.poll(cx)));
    assert_eq!(
        Manager::read_registry(&root).unwrap()[&id].manifest.version,
        "2.0.0"
    );
    assert!(!form.update(|_, cx| panel.read(cx).has_market_updates()));
    if let Some(replaced) = replaced {
        let registry = Manager::read_registry(&root).unwrap();
        assert_eq!(
            registry[&replaced.manifest.id].replaced_by.as_deref(),
            Some(id.as_str())
        );
        let files = Manager::persisted_data_directory(
            &root,
            &replaced.manifest,
            &home.path().display().to_string(),
        )
        .unwrap();
        assert_eq!(
            std::fs::read(files.join("retain.bin")).unwrap(),
            b"original replacement owner"
        );
    }
    let (tx, rx) = futures::channel::oneshot::channel();
    worker.tx.send(Work::Shutdown(Some(tx))).unwrap();
    let _ = futures::executor::block_on(rx);
    rust_i18n::set_locale(&previous);
}
