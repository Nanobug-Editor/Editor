//! Consecutive marketplace transactions must each expose native progress and cancellation.
use super::*;
use crate::*;
use gpui_kit::{TestAppContext, VisualTestContext, gpui};
use std::io::Cursor;

/// Serve two independent valid packages in one reviewed catalog, with separately pausable downloads.
fn consecutive_packages() -> (Server, Server) {
    let bytes =
        include_bytes!("../../../../../../plugin-runtime/tests/fixtures/marketplace/notes.zip");
    let original = Package::from_bytes(bytes).unwrap();
    let mut files = original.files;
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["id"] = "test.second".into();
    manifest["name"] = "Second plugin".into();
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    files.insert(
        "plugin.toml".into(),
        String::from_utf8(files["plugin.toml"].clone())
            .unwrap()
            .replace("test.notes", "test.second")
            .replace("Notes", "Second plugin")
            .into_bytes(),
    );
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in files {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&bytes).unwrap();
    }
    let second_bytes = zip.finish().unwrap().into_inner();
    let second = Package::from_bytes(&second_bytes).unwrap();
    let mut catalog: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../../../../plugin-runtime/tests/fixtures/marketplace/catalog.json"
    ))
    .unwrap();
    let mut listing = catalog["plugins"][0].clone();
    listing["id"] = second.manifest.id.clone().into();
    listing["versions"][0]["manifest"] = serde_json::to_value(&second.manifest).unwrap();
    listing["versions"][0]["sha256"] = second.digest.into();
    let second_catalog = serde_json::json!({ "schema": catalog["schema"], "plugins": [listing] });
    let second_server =
        Server::from_bytes(&serde_json::to_vec(&second_catalog).unwrap(), second_bytes);
    listing["versions"][0]["url"] = format!("{}/notes.zip", second_server.url).into();
    catalog["plugins"].as_array_mut().unwrap().push(listing);
    let first_server = Server::from_bytes(&serde_json::to_vec(&catalog).unwrap(), bytes.to_vec());
    (first_server, second_server)
}

/// Render queued native publications before locating a control in `form`.
fn draw(form: &mut VisualTestContext) {
    form.run_until_parked();
    form.update(|window, cx| window.draw(cx).clear(cx));
}

/// Activate a visible control through real pointer input, then settle its resulting layout.
fn click(form: &mut VisualTestContext, selector: &'static str) {
    let bounds = form
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("missing {selector}"));
    form.simulate_click(bounds.center(), Default::default());
    draw(form);
}

/// A completed first install and a cancelled second install cannot suppress subsequent progress windows.
#[gpui::test]
fn marketplace_consecutive_installs_and_retry_show_progress(cx: &mut TestAppContext) {
    consecutive_installations(cx, false);
}

/// Leaving completion feedback open must not let its old cancellation token control the next attempt.
#[gpui::test]
fn marketplace_retained_completion_yields_to_next_installation(cx: &mut TestAppContext) {
    consecutive_installations(cx, true);
}

/// Exercise the same complete install/cancel/retry journey with either completed-window lifetime.
fn consecutive_installations(cx: &mut TestAppContext, retain_completion: bool) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        cx.set_reduce_motion(true);
    });
    let (first, second) = consecutive_packages();
    first.pause.store(true, Ordering::Release);
    second.pause.store(true, Ordering::Release);
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join(".runtime-plugin-test");
    let worker = Arc::new(Worker::start_background(
        root.clone(),
        Environment {
            workspace: home.path().display().to_string(),
            ..Default::default()
        },
        true,
    ));
    wait(&worker, |state| state.ready);
    worker.refresh_market(
        root.join("marketplace/catalog.json"),
        format!("{}/catalog.json", first.url),
    );
    wait(&worker, |state| state.market.fresh());
    let workspace = Workspace::open(home.path()).unwrap();
    let slot = Rc::new(std::cell::RefCell::new(None));
    let captured = slot.clone();
    let (_, editor) = cx.add_window_view(move |window, cx| {
        let app = cx.new(|cx| EditorApp::new(workspace, None, window, cx));
        *captured.borrow_mut() = Some(app.clone());
        Root::new(app, window, cx)
    });
    let app = slot.borrow_mut().take().unwrap();
    let before = editor.update(|_, cx| cx.windows());
    let panel = editor.update(|window, cx| {
        let panel = app.read(cx).extensions.clone();
        panel.update(cx, |panel, cx| {
            panel.worker = worker.clone();
            panel.poll(cx);
        });
        app.update(cx, |app, cx| app.toggle_extensions(window, cx));
        panel.update(cx, |panel, cx| {
            panel.manager_market = true;
            cx.notify();
        });
        panel
    });
    let manager = editor
        .update(|_, cx| cx.windows())
        .into_iter()
        .find(|window| !before.contains(window))
        .unwrap();
    let form = VisualTestContext::from_window(manager, editor).into_mut();
    form.simulate_resize(size(px(1160.), px(840.)));
    draw(form);
    let mut retained = None;
    for (index, (id, selector, server)) in [
        ("test.notes", "plugin-row-test.notes", &first),
        ("test.second", "plugin-row-test.second", &second),
        ("test.second", "plugin-row-test.second", &second),
    ]
    .into_iter()
    .enumerate()
    {
        click(form, selector);
        let before = form.update(|_, cx| cx.windows());
        let downloads = server.downloads.load(Ordering::Acquire);
        click(form, "market-install");
        assert!(
            form.debug_bounds("confirm-market-install").is_some(),
            "installation attempt {} must show permission consent before downloading",
            index + 1
        );
        assert_eq!(
            server.downloads.load(Ordering::Acquire),
            downloads,
            "consent precedes every first installation attempt"
        );
        click(form, "confirm-market-install");
        wait(&worker, |_| {
            server.downloads.load(Ordering::Acquire) > downloads
        });
        form.update(|_, cx| panel.update(cx, |panel, cx| panel.poll(cx)));
        draw(form);
        if let Some(previous) = retained.take() {
            // Closing obsolete feedback must leave the newly accepted attempt cancellable and active.
            assert!(!form.update(|_, cx| cx.windows()).contains(&previous));
            assert!(worker.state.lock().unwrap().progress.is_some());
        }
        let progress = form
            .update(|_, cx| cx.windows())
            .into_iter()
            .find(|window| !before.contains(window));
        assert!(
            progress.is_some(),
            "installation attempt {} must open its own progress window",
            index + 1
        );
        let progress_window = progress.unwrap();
        let progress = VisualTestContext::from_window(progress_window, form).into_mut();
        draw(progress);
        assert!(progress.debug_bounds("plugin-install-progress").is_some());
        let button = progress
            .debug_bounds("plugin-install-progress-close-region")
            .expect("visible cancellation");
        assert!(button.bottom() <= progress.update(|window, _| window.viewport_size().height));
        if index == 1 {
            click(progress, "plugin-install-progress-close-region");
            server.pause.store(false, Ordering::Release);
            wait(&worker, |state| {
                state.progress.is_none() && state.status.is_some()
            });
            assert!(!Manager::read_registry(&root).unwrap().contains_key(id));
            assert!(
                Manager::read_registry(&root)
                    .unwrap()
                    .contains_key("test.notes")
            );
        } else {
            server.pause.store(false, Ordering::Release);
            wait(&worker, |state| {
                state.progress.is_none()
                    && state.entries.iter().any(|entry| entry.manifest.id == id)
            });
            assert!(worker.state.lock().unwrap().status.is_none());
        }
        form.update(|_, cx| panel.update(cx, |panel, cx| panel.poll(cx)));
        draw(progress);
        if retain_completion && index == 0 {
            retained = Some(progress_window);
            continue;
        }
        let close = progress
            .debug_bounds("plugin-install-progress-close-region")
            .unwrap();
        progress.simulate_click(close.center(), Default::default());
        draw(form);
    }
    assert_eq!(Manager::read_registry(&root).unwrap().len(), 2);
    let (tx, rx) = futures::channel::oneshot::channel();
    worker.tx.send(Work::Shutdown(Some(tx))).unwrap();
    futures::executor::block_on(rx).unwrap().unwrap();
}

/// A transport failure leaves the same visible terminal report as an installation failure, and permits retry.
#[test]
fn marketplace_failed_download_keeps_failure_in_progress() {
    let server = Server::new();
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join("runtime");
    let worker = Worker::start_background(
        root.clone(),
        Environment {
            workspace: home.path().display().to_string(),
            ..Default::default()
        },
        true,
    );
    wait(&worker, |state| state.ready);
    worker.refresh_market(
        root.join("marketplace/catalog.json"),
        format!("{}/catalog.json", server.url),
    );
    wait(&worker, |state| state.market.fresh());
    let (release, generation) = {
        let state = worker.state.lock().unwrap();
        (
            state.market.catalog.as_ref().unwrap().plugins[0].versions[0].clone(),
            state.market.generation(),
        )
    };
    server.fail_downloads.store(true, Ordering::Release);
    assert!(worker.install_market(release.clone(), generation, Default::default(), false));
    wait(&worker, |state| {
        state.progress.is_none() && state.status.is_some()
    });
    let (report, failure, idle) = {
        let state = worker.state.lock().unwrap();
        (
            state.installation.clone(),
            state.status.as_ref().unwrap().message.clone(),
            state.install_control.is_none(),
        )
    };
    let before_retry = Manager::read_registry(&root).unwrap();
    server.fail_downloads.store(false, Ordering::Release);
    let accepted = worker.install_market(release, generation, Default::default(), false);
    wait(&worker, |state| state.progress.is_none());
    let installed = Manager::read_registry(&root).unwrap();
    let (tx, rx) = futures::channel::oneshot::channel();
    worker.tx.send(Work::Shutdown(Some(tx))).unwrap();
    futures::executor::block_on(rx).unwrap().unwrap();
    assert!(
        failure.contains("503"),
        "controlled transport failure: {failure}"
    );
    assert!(idle && before_retry.is_empty());
    let report = report.expect(
        "the progress dialog must retain the download failure instead of displaying Finished",
    );
    assert!(report.message.contains(&failure));
    assert!(!report.cancellable && !report.installed);
    assert!(
        accepted && installed.contains_key("test.notes"),
        "failed downloads must permit a new consented attempt"
    );
}
