//! Final source-free delivery combines real official packages, native actions and controlled catalog outages.
use super::*;
use crate::extensions::ExtensionPanel;
use crate::*;
use gpui_kit::{TestAppContext, VisualTestContext, gpui};

/// Fresh users install reviewed bytes; an existing local version keeps its identity and private data on upgrade.
#[gpui::test]
fn unbundled_official_market_install_and_existing_user_upgrade(cx: &mut TestAppContext) {
    let previous = rust_i18n::locale().to_string();
    let full: serde_json::Value =
        serde_json::from_slice(&crate::test_packages::official_catalog()).unwrap();
    let listing = full["plugins"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"] == "toml")
        .unwrap()
        .clone();
    let catalog = serde_json::json!({"schema": full["schema"], "plugins": [listing]});
    let bytes = std::fs::read(crate::test_packages::archive("toml")).unwrap();
    let old = crate::test_packages::package("toml-0.2.2");
    let statistics = crate::test_packages::official_statistics();
    for (locale, dark, existing) in [("en", true, None), ("zh-CN", false, Some(&old))] {
        rust_i18n::set_locale(locale);
        let server = Server::with_statistics(
            &serde_json::to_vec(&catalog).unwrap(),
            bytes.clone(),
            Some(statistics.clone()),
        );
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join(".runtime-plugin-test");
        std::fs::write(
            home.path().join("sample.toml"),
            "[package]\nname = '示例'\n",
        )
        .unwrap();
        let environment = Environment {
            os: std::env::consts::OS.into(),
            workspace: home.path().display().to_string(),
            ..Default::default()
        };
        if let Some(old) = existing {
            let mut manager = Manager::open(root.clone(), environment.clone()).unwrap();
            manager
                .install(old, old.manifest.permissions.clone())
                .unwrap();
            let private = manager.data_directory("toml");
            std::fs::create_dir_all(&private).unwrap();
            std::fs::write(private.join("retain.txt"), "既有插件私有数据").unwrap();
            manager.shutdown();
        }
        let worker = Arc::new(Worker::start_background(root.clone(), environment, true));
        wait(&worker, |state| state.ready);
        assert_eq!(
            Manager::read_registry(&root).unwrap().len(),
            usize::from(existing.is_some())
        );
        worker.refresh_market(
            root.join("marketplace/catalog.json"),
            format!("{}/catalog.json", server.url),
        );
        wait(&worker, |state| {
            state.market.fresh() && state.market_statistics.is_some()
        });
        let (_app, panel, form) = mount(cx, home.path(), &worker, dark, existing.is_some());
        inspect_history_and_upgrade(form, &panel, &worker, &server, &root, existing);
        outage_and_withdrawal(
            form,
            &panel,
            &worker,
            &server,
            &catalog,
            &bytes,
            &statistics,
            &root,
        );
        let (tx, rx) = futures::channel::oneshot::channel();
        worker.tx.send(Work::Shutdown(Some(tx))).unwrap();
        let _ = futures::executor::block_on(rx);
    }
    rust_i18n::set_locale(&previous);
}

/// Build real windows around the recovered actor; no synthetic installed entries or success publication are used.
fn mount<'a>(
    cx: &'a mut TestAppContext,
    home: &std::path::Path,
    worker: &Arc<Worker>,
    dark: bool,
    existing: bool,
) -> (
    Entity<EditorApp>,
    Entity<ExtensionPanel>,
    &'a mut VisualTestContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(dark), cx);
        // The second locale uses the same typography control as settings to exercise a larger native UI.
        if !dark {
            typography::step_by(cx, 4);
        }
        cx.set_reduce_motion(true);
    });
    let workspace = Workspace::open(home).unwrap();
    let slot = Rc::new(std::cell::RefCell::new(None));
    let capture = slot.clone();
    let (_, editor) = cx.add_window_view(move |window, cx| {
        let app = cx.new(|cx| EditorApp::new(workspace, None, window, cx));
        *capture.borrow_mut() = Some(app.clone());
        Root::new(app, window, cx)
    });
    let app = slot.borrow_mut().take().unwrap();
    let panel = editor.update(|window, cx| {
        let panel = app.read(cx).extensions.clone();
        panel.update(cx, |panel, cx| {
            panel.worker = worker.clone();
            panel.poll(cx);
            panel.manager_market = !existing;
        });
        app.update(cx, |app, cx| {
            app.open_file(home.join("sample.toml"), window, cx)
        });
        panel
    });
    if !existing {
        assert_eq!(
            editor.update(|_, cx| app.read(cx).editor.read(cx).language_name().to_string()),
            "text"
        );
        assert!(editor.debug_bounds("plugin-install-confirm").is_none());
    }
    editor.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(
        editor.debug_bounds("extensions-update-dot").is_some(),
        existing
    );
    let before = editor.update(|_, cx| cx.windows());
    editor.update(|window, cx| app.update(cx, |app, cx| app.toggle_extensions(window, cx)));
    let handle = editor
        .update(|_, cx| cx.windows())
        .into_iter()
        .find(|window| !before.contains(window))
        .unwrap();
    let form = VisualTestContext::from_window(handle, editor).into_mut();
    form.simulate_resize(size(px(1160.), px(840.)));
    draw(form);
    (app, panel, form)
}

/// History, Chinese search input, update clicks and actual grammar activation share the normal native controls.
fn inspect_history_and_upgrade(
    form: &mut VisualTestContext,
    panel: &Entity<ExtensionPanel>,
    worker: &Arc<Worker>,
    server: &Server,
    root: &std::path::Path,
    existing: Option<&Package>,
) {
    assert_eq!(server.downloads.load(Ordering::Acquire), 0);
    if existing.is_some() {
        click(form, "plugin-detail-tabs-3");
    }
    assert_eq!(
        form.update(|_, cx| panel.read(cx).manager_selected.clone()),
        Some("toml".into())
    );
    click(form, "plugin-detail-tabs-3");
    click(form, "market-version-0.2.3");
    let state = worker.state.lock().unwrap();
    let release = state.market.catalog.as_ref().unwrap().plugins[0].versions[0].clone();
    assert!(!release.changelog.is_empty());
    let stats = state.market_statistics.as_ref().unwrap();
    assert_eq!(stats.measured_at.as_deref(), Some("2026-10-11T07:53:39Z"));
    assert!(stats.total("toml").is_some());
    drop(state);
    // Enter and clear Chinese through the real text input; no filter values are substituted.
    let search = form.debug_bounds("plugin-manager-search").unwrap();
    form.simulate_click(search.center(), Default::default());
    form.simulate_input("中文");
    draw(form);
    assert_eq!(
        form.update(|_, cx| panel
            .read(cx)
            .manager_search
            .as_ref()
            .unwrap()
            .read(cx)
            .value()
            .to_string()),
        "中文"
    );
    form.simulate_keystrokes("ctrl-a backspace");
    draw(form);
    assert!(form.update(|_, cx| {
        panel
            .read(cx)
            .manager_search
            .as_ref()
            .unwrap()
            .read(cx)
            .value()
            .is_empty()
    }));
    scroll_details(form, -350.);
    if existing.is_some() {
        // The installed page can update directly after history browsing without redirecting local details.
        click(form, "plugin-manager-tabs-0");
    }
    click(form, "market-install");
    if existing.is_none() {
        assert_eq!(server.downloads.load(Ordering::Acquire), 0);
        click(form, "confirm-market-install");
    } else {
        assert!(
            form.debug_bounds("confirm-market-install").is_none(),
            "unchanged permissions are retained"
        );
    }
    // A native click can enqueue installation before the actor publishes progress. Observe the
    // selected version, rather than mistaking an older status or the initial idle state for done.
    wait(worker, |state| {
        state.progress.is_none()
            && state.entries.iter().any(|entry| {
                entry.manifest.id == release.manifest.id
                    && entry.manifest.version == release.manifest.version
            })
    });
    form.update(|_, cx| panel.update(cx, |panel, cx| panel.poll(cx)));
    draw(form);
    assert!(
        form.debug_bounds("market-install").is_none(),
        "a current installed version must not leave a disabled update button"
    );
    assert!(form.debug_bounds("plugin-row-toml-update").is_none());
    assert_eq!(
        form.update(|_, cx| panel.read(cx).manager_market),
        existing.is_none()
    );
    let records = Manager::read_registry(root).unwrap();
    assert_eq!(records["toml"].manifest.version, "0.2.3");
    assert_eq!(records["toml"].digest, release.sha256);
    assert_eq!(records["toml"].grants, release.manifest.permissions);
    assert_eq!(records["toml"].enabled, existing.is_some());
    assert_eq!(
        records["toml"].project_enabled_in(&root.parent().unwrap().to_string_lossy()),
        existing.is_none(),
    );
    assert!(!form.update(|_, cx| panel.read(cx).has_market_updates()));
    let path = root.parent().unwrap().join("sample.toml");
    let language = crate::language::providers::language_for_path(&path).unwrap();
    let selected = crate::language::code_highlighting::selected(&language).unwrap();
    assert!(
        !crate::language::code_highlighting::highlight(
            &selected,
            "[package]\nname='示例'",
            &std::sync::atomic::AtomicBool::new(false),
            Instant::now() + Duration::from_secs(30)
        )
        .is_empty()
    );
    if let Some(old) = existing {
        let private = Manager::persisted_data_directory(
            root,
            &old.manifest,
            &root.parent().unwrap().display().to_string(),
        )
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(private.join("retain.txt")).unwrap(),
            "既有插件私有数据"
        );
    }
}

/// A failed refresh and a later withdrawn catalog cannot authorize downloads or disturb the installed version.
fn outage_and_withdrawal(
    form: &mut VisualTestContext,
    panel: &Entity<ExtensionPanel>,
    worker: &Arc<Worker>,
    server: &Server,
    catalog: &serde_json::Value,
    bytes: &[u8],
    statistics: &[u8],
    root: &std::path::Path,
) {
    server.offline.store(true, Ordering::Release);
    worker.refresh_market(
        root.join("marketplace/catalog.json"),
        format!("{}/catalog.json", server.url),
    );
    wait(worker, |state| state.market.error.is_some());
    form.update(|_, cx| panel.update(cx, |panel, cx| panel.poll(cx)));
    draw(form);
    let before = server.downloads.load(Ordering::Acquire);
    let (release, generation) = {
        let state = worker.state.lock().unwrap();
        assert!(!state.market.fresh());
        (
            state.market.catalog.as_ref().unwrap().plugins[0].versions[0].clone(),
            state.market.generation(),
        )
    };
    assert!(!worker.install_market(release, generation, Default::default(), true));
    assert_eq!(server.downloads.load(Ordering::Acquire), before);
    server.offline.store(false, Ordering::Release);
    worker.refresh_market(
        root.join("marketplace/catalog.json"),
        format!("{}/catalog.json", server.url),
    );
    wait(worker, |state| state.market.fresh());
    let mut withdrawn = catalog.clone();
    withdrawn["plugins"][0]["versions"][0]["withdrawn"] = true.into();
    withdrawn["plugins"][0]["versions"][0]["withdrawal_reason"] =
        "Acceptance withdrawal only".into();
    let revoked = Server::with_statistics(
        &serde_json::to_vec(&withdrawn).unwrap(),
        bytes.to_vec(),
        Some(statistics.to_vec()),
    );
    worker.refresh_market(
        root.join("marketplace/catalog.json"),
        format!("{}/catalog.json", revoked.url),
    );
    wait(worker, |state| {
        state.market.fresh()
            && state.market.catalog.as_ref().unwrap().plugins[0].versions[0].withdrawn
    });
    form.update(|_, cx| {
        panel.update(cx, |panel, cx| {
            panel.poll(cx);
            panel.manager_market = false;
        })
    });
    draw(form);
    click(form, "plugin-detail-tabs-3");
    assert!(
        form.debug_bounds("market-version-0.2.3").is_some(),
        "withdrawal remains visible in installed history"
    );
    click(form, "market-version-0.2.3");
    scroll_details(form, -350.);
    assert!(
        form.debug_bounds("market-install").is_none(),
        "withdrawn versions remain browsable without an install action"
    );
    assert_eq!(revoked.downloads.load(Ordering::Acquire), 0);
    assert_eq!(
        Manager::read_registry(root).unwrap()["toml"]
            .manifest
            .version,
        "0.2.3"
    );
}

/// Draw a real native frame after pending UI tasks, using selectors only to locate visible controls.
fn draw(form: &mut VisualTestContext) {
    form.run_until_parked();
    form.update(|window, cx| window.draw(cx).clear(cx));
}

/// Click the visible native target; missing controls fail instead of calling the listener directly.
fn click(form: &mut VisualTestContext, selector: &'static str) {
    let bounds = form
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("missing native control: {selector}"));
    form.simulate_click(bounds.center(), Default::default());
    draw(form);
}

/// Use the actual wheel input to reach version actions and the rendered changelog in the details viewport.
fn scroll_details(form: &mut VisualTestContext, delta: f32) {
    let viewport = form.debug_bounds("plugin-manager-detail-content").unwrap();
    form.simulate_event(gpui_kit::ScrollWheelEvent {
        position: viewport.center(),
        delta: gpui_kit::ScrollDelta::Pixels(point(px(0.), px(delta))),
        touch_phase: gpui_kit::TouchPhase::Moved,
        modifiers: Default::default(),
    });
    draw(form);
}
