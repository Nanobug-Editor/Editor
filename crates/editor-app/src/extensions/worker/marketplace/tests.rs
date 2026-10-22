//! The production actor consumes a registry-produced fixture served over real loopback HTTP.
use super::*;
use crate::extensions::market_test_server::Server;
use std::io::Write;
use std::sync::atomic::Ordering;
mod delivery;
mod layout;
mod official;
mod updates;
mod wasm_updates;

/// A recorded UI actor stays inert for ambient startup checks but keeps real controlled HTTP coverage.
#[test]
fn recorded_ui_transport_isolates_ambient_market_checks() {
    let server = Server::new();
    let home = tempfile::tempdir().unwrap();
    let worker = Worker::start(home.path().join("runtime"), Environment::default(), true);
    let cache = home.path().join("catalog.json");
    worker.refresh_market(
        cache.clone(),
        plugin_runtime::marketplace::CATALOG_URL.into(),
    );
    assert_eq!(worker.state.lock().unwrap().market.generation(), 0);
    worker.refresh_market(cache, format!("{}/catalog.json", server.url));
    wait(&worker, |state| state.market.fresh());
    assert!(server.requests.load(Ordering::Acquire) > 0);
    assert!(!worker.state.lock().unwrap().ready);
}

/// The catalog may arrive before manager recovery; a genuine unstarted actor cannot authorize installs.
#[test]
fn marketplace_waits_for_initial_installed_records() {
    let server = Server::new();
    let home = tempfile::tempdir().unwrap();
    let worker = Worker::start(home.path().join("runtime"), Environment::default(), true);
    worker.refresh_market(
        home.path().join("cache.json"),
        format!("{}/catalog.json", server.url),
    );
    wait(&worker, |s| s.market.fresh());
    let (release, generation) = {
        let state = worker.state.lock().unwrap();
        assert!(!state.ready);
        (
            state.market.catalog.as_ref().unwrap().plugins[0].versions[0].clone(),
            state.market.generation(),
        )
    };
    assert!(!worker.install_market(release, generation, Default::default(), true));
    assert_eq!(server.downloads.load(Ordering::Acquire), 0);
    assert!(worker.recorded.lock().unwrap().try_recv().is_err());
}

/// The controlled guest is built by the host SDK entry, then exported by the registry builder.
#[test]
#[ignore = "build capability-example with --plugin-package and export target/marketplace-wasm via marketplace tests.export_wasm_fixture"]
fn marketplace_installs_actual_wasm_through_background_preparation() {
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/marketplace-wasm");
    let server = Server::from_bytes(
        &std::fs::read(fixture.join("catalog.json")).expect("export current WASM catalog"),
        std::fs::read(fixture.join("notes.zip")).expect("export current WASM ZIP"),
    );
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join("runtime");
    let worker = Worker::start_background(
        root.clone(),
        Environment {
            workspace: home.path().to_string_lossy().into_owned(),
            ..Environment::default()
        },
        true,
    );
    wait(&worker, |s| s.ready);
    worker.refresh_market(
        root.join("marketplace/catalog.json"),
        format!("{}/catalog.json", server.url),
    );
    wait(&worker, |s| s.market.fresh());
    let (candidate, generation) = {
        let s = worker.state.lock().unwrap();
        (
            s.market.catalog.as_ref().unwrap().plugins[0].versions[0].clone(),
            s.market.generation(),
        )
    };
    let id = candidate.manifest.id.clone();
    assert!(candidate.manifest.component.is_some());
    assert!(worker.install_market(candidate.clone(), generation, Default::default(), true));
    wait(&worker, |s| s.progress.is_none());
    let state = worker.state.lock().unwrap();
    assert!(
        state.status.is_none(),
        "{:?}",
        state.status.as_ref().map(|s| &s.message)
    );
    assert!(
        state.instance_epochs.contains_key(&id),
        "actual guest must become live"
    );
    assert!(Manager::read_registry(&root).unwrap().contains_key(&id));
    drop(state);
    worker.tx.send(Work::Shutdown(None)).unwrap();
}

fn wait(worker: &Worker, predicate: impl Fn(&Published) -> bool) {
    wait_with_budget(worker, Duration::from_secs(60), predicate);
}

/// Rich delivered guests also prepare independent completion/structure instances before publication.
/// A caller can budget that measured CPU work without changing the ordinary actor-test deadline.
fn wait_with_budget(worker: &Worker, budget: Duration, predicate: impl Fn(&Published) -> bool) {
    let end = Instant::now() + budget;
    while !predicate(&worker.state.lock().unwrap()) {
        assert!(
            Instant::now() < end,
            "marketplace publication timed out: {:?}",
            {
                let state = worker.state.lock().unwrap();
                (
                    state.market.error.clone(),
                    state.status.as_ref().map(|s| s.message.clone()),
                    state.installation.clone(),
                )
            }
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Each failure traverses catalog admission, consent/download and the actor; old installations survive.
#[test]
fn marketplace_failed_downloads_and_restricted_workspaces_preserve_existing_installation() {
    for failure in ["digest", "manifest", "cancel", "restricted"] {
        let mut catalog: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../../../../../plugin-runtime/tests/fixtures/marketplace/catalog.json"
        ))
        .unwrap();
        match failure {
            "digest" => catalog["plugins"][0]["versions"][0]["sha256"] = "0".repeat(64).into(),
            "manifest" => {
                catalog["plugins"][0]["versions"][0]["manifest"]["name"] = "Forged display".into()
            }
            _ => {}
        }
        let bytes =
            include_bytes!("../../../../../plugin-runtime/tests/fixtures/marketplace/notes.zip");
        let server = Server::from_bytes(&serde_json::to_vec(&catalog).unwrap(), bytes.to_vec());
        server.pause.store(failure == "cancel", Ordering::Release);
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join("runtime");
        let environment = Environment {
            workspace: home.path().to_string_lossy().into_owned(),
            ..Environment::default()
        };
        // Seed an unrelated installed plugin through the same public manager, never a publication stub.
        let mut files = Package::from_bytes(bytes).unwrap().files;
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&files["manifest.json"]).unwrap();
        manifest["id"] = "existing.notes".into();
        files.insert(
            "manifest.json".into(),
            serde_json::to_vec(&manifest).unwrap(),
        );
        files.insert(
            "plugin.toml".into(),
            String::from_utf8(files["plugin.toml"].clone())
                .unwrap()
                .replace("test.notes", "existing.notes")
                .into_bytes(),
        );
        let existing = Package::from_files(files).unwrap();
        let mut manager = Manager::open(root.clone(), environment.clone()).unwrap();
        manager
            .install(&existing, existing.manifest.permissions.clone())
            .unwrap();
        manager.shutdown();
        drop(manager);
        let worker = Worker::start_background(root.clone(), environment, failure != "restricted");
        wait(&worker, |s| s.ready);
        worker.refresh_market(
            root.join("marketplace/catalog.json"),
            format!("{}/catalog.json", server.url),
        );
        wait(&worker, |s| s.market.fresh());
        let (candidate, generation) = {
            let s = worker.state.lock().unwrap();
            (
                s.market.catalog.as_ref().unwrap().plugins[0].versions[0].clone(),
                s.market.generation(),
            )
        };
        assert_eq!(
            worker.install_market(candidate, generation, Default::default(), true),
            failure != "restricted"
        );
        if failure == "cancel" {
            wait(&worker, |_| server.downloads.load(Ordering::Acquire) > 0);
            worker.cancel_installation();
        }
        if failure != "restricted" {
            wait(&worker, |s| s.progress.is_none() && s.status.is_some());
        }
        let installed = Manager::read_registry(&root).unwrap();
        assert_eq!(installed.len(), 1, "{failure}");
        assert_eq!(installed["existing.notes"].digest, existing.digest);
        if failure == "restricted" {
            assert_eq!(server.downloads.load(Ordering::Acquire), 0);
        }
        worker.tx.send(Work::Shutdown(None)).unwrap();
    }
}

/// No package bytes before consent; failure blocks new installs even when the package URL still works.
#[test]
fn marketplace_refresh_consent_install_and_offline_gate_use_the_real_actor() {
    let server = Server::new();
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join("runtime");
    let worker = Worker::start_background(
        root.clone(),
        Environment {
            workspace: home.path().to_string_lossy().into_owned(),
            ..Environment::default()
        },
        true,
    );
    wait(&worker, |s| s.ready);
    let cache = root.join("marketplace/catalog.json");
    worker.refresh_market(cache.clone(), format!("{}/catalog.json", server.url));
    wait(&worker, |s| s.market.fresh());
    assert_eq!(server.downloads.load(Ordering::Acquire), 0);
    let (candidate, generation) = {
        let s = worker.state.lock().unwrap();
        (
            s.market.catalog.as_ref().unwrap().plugins[0].versions[0].clone(),
            s.market.generation(),
        )
    };
    server.offline.store(true, Ordering::Release);
    worker.refresh_market(cache.clone(), format!("{}/catalog.json", server.url));
    wait(&worker, |s| s.market.error.is_some());
    assert!(!worker.install_market(candidate.clone(), generation, Default::default(), true));
    assert_eq!(server.downloads.load(Ordering::Acquire), 0);
    server.offline.store(false, Ordering::Release);
    worker.refresh_market(cache, format!("{}/catalog.json", server.url));
    wait(&worker, |s| s.market.fresh());
    let generation = worker.state.lock().unwrap().market.generation();
    assert!(worker.install_market(candidate, generation, Default::default(), true));
    wait(&worker, |s| s.progress.is_none());
    let state = worker.state.lock().unwrap();
    assert!(
        state.status.is_none(),
        "{:?}",
        state.status.as_ref().map(|s| &s.message)
    );
    assert!(state.entries.iter().any(|e| e.manifest.id == "test.notes"));
    drop(state);
    assert!(
        Manager::read_registry(&root)
            .unwrap()
            .contains_key("test.notes")
    );
    assert_eq!(server.downloads.load(Ordering::Acquire), 1);
    let candidate = worker
        .state
        .lock()
        .unwrap()
        .market
        .catalog
        .as_ref()
        .unwrap()
        .plugins[0]
        .versions[0]
        .clone();
    assert!(
        !worker.install_market(candidate, generation, Default::default(), true),
        "market cannot reinstall an existing ID"
    );
    worker.tx.send(Work::Shutdown(None)).unwrap();
}

/// Both palettes and locales use actual pointer consent, then observe the actor's real installation.
#[gpui_kit::gpui::test]
fn marketplace_native_consent_and_install_in_both_locales(cx: &mut gpui_kit::TestAppContext) {
    use crate::*;
    use gpui_kit::VisualTestContext;
    let previous = rust_i18n::locale().to_string();
    for (locale, dark) in [("zh-CN", false), ("en", true)] {
        rust_i18n::set_locale(locale);
        cx.update(|cx| {
            gpui_kit::init(cx);
            typography::init(cx);
            apply_theme(builtin_theme(dark), cx);
            cx.set_reduce_motion(true);
        });
        // Snapshot markup cannot trigger file/network access before consent, even if its author asks.
        let external = Server::new();
        let mut catalog: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../../../../../plugin-runtime/tests/fixtures/marketplace/catalog.json"
        ))
        .unwrap();
        let version = &mut catalog["plugins"][0]["versions"][0];
        version["readme"] = format!(
            "![remote]({}/image.png)\n![local](file:///C:/Windows/win.ini)\n\n{}",
            external.url,
            version["readme"].as_str().unwrap()
        )
        .into();
        version["icon"] = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><image href="{}/image.png" width="24" height="24"/><image href="file:///C:/Windows/win.ini" width="24" height="24"/></svg>"#, external.url).into();
        let mut incompatible = version.clone();
        incompatible["version"] = "2.0.0".into();
        incompatible["manifest"]["version"] = "2.0.0".into();
        incompatible["host_version"] = ">=2.0.0".into();
        incompatible["changelog"] = "Requires a newer editor".into();
        catalog["plugins"][0]["versions"]
            .as_array_mut()
            .unwrap()
            .push(incompatible);
        // A second inert listing makes filter changes observable without another package download.
        let mut other = catalog["plugins"][0].clone();
        other["id"] = "test.other".into();
        other["category"] = "utilities".into();
        other["tags"] = serde_json::json!(["other"]);
        for release in other["versions"].as_array_mut().unwrap() {
            release["manifest"]["id"] = "test.other".into();
            release["manifest"]["name"] = "Other".into();
        }
        catalog["plugins"].as_array_mut().unwrap().push(other);
        let server = Server::from_bytes(
            &serde_json::to_vec(&catalog).unwrap(),
            include_bytes!("../../../../../plugin-runtime/tests/fixtures/marketplace/notes.zip")
                .to_vec(),
        );
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join(".runtime-plugin-test");
        let worker = Arc::new(Worker::start_background(
            root.clone(),
            Environment {
                workspace: home.path().to_string_lossy().into_owned(),
                ..Environment::default()
            },
            true,
        ));
        wait(&worker, |s| s.ready);
        worker.refresh_market(
            root.join("marketplace/catalog.json"),
            format!("{}/catalog.json", server.url),
        );
        wait(&worker, |s| s.market.fresh());
        let workspace = Workspace::open(home.path()).unwrap();
        let slot = Rc::new(std::cell::RefCell::new(None));
        let captured = slot.clone();
        let (_, editor_cx) = cx.add_window_view(move |window, cx| {
            let app = cx.new(|cx| EditorApp::new(workspace, None, window, cx));
            *captured.borrow_mut() = Some(app.clone());
            Root::new(app, window, cx)
        });
        let app = slot.borrow_mut().take().unwrap();
        let before = editor_cx.update(|_, cx| cx.windows());
        let panel = editor_cx.update(|window, cx| {
            let panel = app.read(cx).extensions.clone();
            panel.update(cx, |panel, cx| {
                panel.worker = worker.clone();
                panel.manager_market = true;
                panel.poll(cx);
            });
            app.update(cx, |app, cx| app.toggle_extensions(window, cx));
            // Opening resets selection; switch through the same mode state used by the native tabs.
            panel.update(cx, |panel, cx| {
                panel.manager_market = true;
                cx.notify();
            });
            panel
        });
        let window = editor_cx
            .update(|_, cx| cx.windows())
            .into_iter()
            .find(|w| !before.contains(w))
            .unwrap();
        let form = VisualTestContext::from_window(window, editor_cx).into_mut();
        form.run_until_parked();
        form.update(|window, cx| window.draw(cx).clear(cx));
        assert_eq!(server.downloads.load(Ordering::Acquire), 0);
        // Browsing uses the same native manager frame as installed plugins, before any download.
        assert!(form.debug_bounds("plugin-manager-sidebar").is_some());
        assert!(form.debug_bounds("plugin-manager-tabs-0").is_some());
        assert!(form.debug_bounds("plugin-manager-tabs-1").is_some());
        assert!(form.debug_bounds("plugin-manager-header").is_some());
        assert!(form.debug_bounds("plugin-row-test.other").is_some());
        let search_frame = form.debug_bounds("plugin-manager-search").unwrap();
        let trigger = form.debug_bounds("market-filter-trigger").unwrap();
        assert!(search_frame.contains(&trigger.center()));
        form.simulate_click(trigger.center(), Default::default());
        form.run_until_parked();
        form.update(|window, cx| window.draw(cx).clear(cx));
        let filters = form.debug_bounds("market-filter-panel").unwrap();
        assert_eq!(
            filters.left(),
            search_frame.left(),
            "filter panel aligns with the search frame"
        );
        assert!(
            filters.top() >= search_frame.bottom(),
            "filter panel opens below search"
        );
        let category = form.debug_bounds("market-category-tools").unwrap();
        form.simulate_click(category.center(), Default::default());
        form.run_until_parked();
        form.update(|window, cx| window.draw(cx).clear(cx));
        assert!(form.debug_bounds("plugin-row-test.other").is_none());
        assert!(form.debug_bounds("market-filter-active").is_some());
        let clear = form.debug_bounds("market-clear-filters").unwrap();
        form.simulate_click(clear.center(), Default::default());
        form.run_until_parked();
        form.update(|window, cx| window.draw(cx).clear(cx));
        assert!(form.debug_bounds("plugin-row-test.other").is_some());
        assert!(form.debug_bounds("market-filter-active").is_none());
        let windows = form.cx.update(|cx| cx.windows().len());
        form.simulate_keystrokes("escape");
        form.run_until_parked();
        form.update(|window, cx| window.draw(cx).clear(cx));
        assert!(form.debug_bounds("market-filter-panel").is_none());
        assert_eq!(
            form.cx.update(|cx| cx.windows().len()),
            windows,
            "Escape closes only the filter panel"
        );
        // Base synthesizes a keyboard click on key-up; dispatching a lone key-down is not an activation.
        let keystroke = gpui_kit::Keystroke::parse("enter").unwrap();
        form.simulate_event(gpui_kit::KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
            prefer_character_input: false,
        });
        form.simulate_event(gpui_kit::KeyUpEvent { keystroke });
        form.run_until_parked();
        form.update(|window, cx| window.draw(cx).clear(cx));
        assert!(
            form.debug_bounds("market-filter-panel").is_some(),
            "keyboard activation restores the popover"
        );
        form.simulate_click(search_frame.center(), Default::default());
        form.run_until_parked();
        form.update(|window, cx| window.draw(cx).clear(cx));
        assert!(form.debug_bounds("market-filter-panel").is_none());
        // Traverse the actual search/tab/list focus order and select the second row with a complete key press.
        for _ in 0..3 {
            form.simulate_keystrokes("tab");
            let keystroke = gpui_kit::Keystroke::parse("enter").unwrap();
            form.simulate_event(gpui_kit::KeyDownEvent {
                keystroke: keystroke.clone(),
                is_held: false,
                prefer_character_input: false,
            });
            form.simulate_event(gpui_kit::KeyUpEvent { keystroke });
            form.run_until_parked();
            form.update(|window, cx| window.draw(cx).clear(cx));
            if form.update(|_, cx| panel.read(cx).manager_selected.as_deref() == Some("test.other"))
            {
                break;
            }
        }
        assert!(
            form.update(|_, cx| panel.read(cx).manager_selected.as_deref() == Some("test.other")),
            "market rows remain keyboard selectable"
        );
        let notes = form.debug_bounds("plugin-row-test.notes").unwrap();
        form.simulate_click(notes.center(), Default::default());
        form.run_until_parked();
        form.update(|window, cx| window.draw(cx).clear(cx));
        // Reviewed history is a detail page; selecting it keeps the fixed management actions available.
        let versions = form.debug_bounds("plugin-detail-tabs-3").unwrap();
        form.simulate_click(versions.center(), Default::default());
        form.run_until_parked();
        form.update(|window, cx| window.draw(cx).clear(cx));
        let newer = form
            .debug_bounds("market-version-2.0.0")
            .expect("historical version selector");
        form.simulate_click(newer.center(), Default::default());
        form.run_until_parked();
        form.update(|window, cx| window.draw(cx).clear(cx));
        assert!(
            form.debug_bounds("market-install").is_none(),
            "incompatible history hides the install action"
        );
        assert!(
            form.debug_bounds("confirm-market-install").is_none(),
            "incompatible history cannot install"
        );
        let older = form.debug_bounds("market-version-1.0.0").unwrap();
        form.simulate_click(older.center(), Default::default());
        form.run_until_parked();
        form.update(|window, cx| window.draw(cx).clear(cx));
        assert_eq!(
            external.requests.load(Ordering::Acquire),
            0,
            "browsing cannot resolve external images"
        );
        let information = form.debug_bounds("plugin-detail-tabs-4").unwrap();
        form.simulate_click(information.center(), Default::default());
        form.run_until_parked();
        form.update(|window, cx| window.draw(cx).clear(cx));
        let source = form.debug_bounds("market-source").unwrap();
        form.simulate_click(source.center(), Default::default());
        assert_eq!(
            form.opened_url().as_deref(),
            Some("https://github.com/author/notes")
        );
        let feedback = form.debug_bounds("market-feedback").unwrap();
        form.simulate_click(feedback.center(), Default::default());
        assert_eq!(
            form.opened_url().as_deref(),
            Some("https://github.com/author/notes/issues")
        );
        let search = form.debug_bounds("plugin-manager-search").unwrap();
        form.simulate_click(search.center(), Default::default());
        form.simulate_input("未匹配 插件");
        form.run_until_parked();
        form.update(|window, cx| window.draw(cx).clear(cx));
        assert!(form.debug_bounds("market-install").is_none());
        form.simulate_keystrokes("ctrl-a");
        form.simulate_input("Notes");
        form.run_until_parked();
        form.update(|window, cx| window.draw(cx).clear(cx));
        let viewport = form.debug_bounds("plugin-manager-detail-content").unwrap();
        form.simulate_event(gpui_kit::ScrollWheelEvent {
            position: viewport.center(),
            delta: gpui_kit::ScrollDelta::Pixels(point(px(0.), px(-600.))),
            touch_phase: gpui_kit::TouchPhase::Moved,
            modifiers: Default::default(),
        });
        form.run_until_parked();
        form.update(|window, cx| window.draw(cx).clear(cx));
        assert!(form.update(|_, cx| panel.read(cx).manager_detail_scroll.offset().y < px(0.)));
        form.simulate_event(gpui_kit::ScrollWheelEvent {
            position: viewport.center(),
            delta: gpui_kit::ScrollDelta::Pixels(point(px(0.), px(600.))),
            touch_phase: gpui_kit::TouchPhase::Moved,
            modifiers: Default::default(),
        });
        form.simulate_scale_factor_change(1.5);
        form.run_until_parked();
        form.update(|window, cx| window.draw(cx).clear(cx));
        let bounds = form
            .debug_bounds("market-install")
            .expect("native install button");
        form.simulate_click(bounds.center(), Default::default());
        form.run_until_parked();
        form.update(|window, cx| window.draw(cx).clear(cx));
        assert_eq!(
            server.downloads.load(Ordering::Acquire),
            0,
            "opening consent cannot download"
        );
        let bounds = form
            .debug_bounds("confirm-market-install")
            .expect("permission confirmation");
        let option = form.debug_bounds("plugin-install-global-option").unwrap();
        assert!(option.right() <= bounds.left());
        let global_enabled = locale == "en";
        if global_enabled {
            form.simulate_click(
                point(option.left() + px(7.), option.center().y),
                Default::default(),
            );
        }
        form.simulate_click(bounds.center(), Default::default());
        wait(&worker, |s| s.progress.is_none());
        form.update(|_, cx| panel.update(cx, |panel, cx| panel.poll(cx)));
        assert!(form.update(|_, cx| {
            panel
                .read(cx)
                .entries
                .iter()
                .any(|e| e.manifest.id == "test.notes")
        }));
        let installed = Manager::read_registry(&root).unwrap();
        assert_eq!(installed["test.notes"].enabled, global_enabled);
        assert_eq!(
            installed["test.notes"].project_enabled_in(&home.path().to_string_lossy()),
            !global_enabled,
        );
        worker.tx.send(Work::Shutdown(None)).unwrap();
        assert_eq!(external.requests.load(Ordering::Acquire), 0);
    }
    rust_i18n::set_locale(&previous);
}
