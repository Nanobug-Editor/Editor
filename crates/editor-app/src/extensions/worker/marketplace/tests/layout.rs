//! Real ZIPs and the native manager prove that reviewed browsing never redirects local lifecycle actions.
use super::*;
use crate::extensions::market_test_server::Server;
use crate::extensions::{ExtensionPanel, worker};
use crate::*;
use gpui_kit::{TestAppContext, VisualTestContext, gpui};
use std::cell::RefCell;
use std::io::{Cursor, Write};
#[path = "../../../../tests/plugin_manager_presentation.rs"]
mod presentation;

/// Derive a valid newer declarative package without relabeling the frozen archive or using guest SDK code.
fn fixture_package(old: &Package, version: &str, name: &str) -> (Package, Vec<u8>) {
    let mut files = old.files.clone();
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["version"] = version.into();
    manifest["name"] = name.into();
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    let descriptor = String::from_utf8(files["plugin.toml"].clone())
        .unwrap()
        .replace(&old.manifest.version, version)
        .replace(&old.manifest.name, name);
    files.insert("plugin.toml".into(), descriptor.into_bytes());
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in files {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&bytes).unwrap();
    }
    let bytes = zip.finish().unwrap().into_inner();
    (Package::from_bytes(&bytes).unwrap(), bytes)
}

/// Bound actual actor publication waits; never infer completion from an initially idle progress field.
fn wait(worker: &Worker, predicate: impl Fn(&worker::Published) -> bool) {
    let end = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        if predicate(&worker.state.lock().unwrap()) {
            return;
        }
        assert!(
            std::time::Instant::now() < end,
            "native lifecycle publication timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// Draw after pointer/keyboard actions so every assertion addresses the current native controls.
fn draw(form: &mut VisualTestContext) {
    form.run_until_parked();
    form.update(|window, cx| window.draw(cx).clear(cx));
}

/// Locate a rendered native target; actions never call the manager's private listeners directly.
fn click(form: &mut VisualTestContext, selector: &'static str) {
    let bounds = form
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("missing {selector}"));
    form.simulate_click(bounds.center(), Default::default());
    draw(form);
}

/// Both modes retain one frame while every management action addresses the installed 1.0.0 instance.
#[gpui::test]
fn marketplace_browsing_version_preserves_local_lifecycle_and_fixed_layout(
    cx: &mut TestAppContext,
) {
    let previous = rust_i18n::locale().to_string();
    for (locale, dark) in [("zh-CN", false), ("en", true)] {
        rust_i18n::set_locale(locale);
        cx.update(|cx| {
            gpui_kit::init(cx);
            typography::init(cx);
            apply_theme(builtin_theme(dark), cx);
            cx.set_reduce_motion(true);
        });
        let original = Package::from_bytes(include_bytes!(
            "../../../../../../plugin-runtime/tests/fixtures/marketplace/notes.zip"
        ))
        .unwrap();
        let (old, _) = fixture_package(&original, "1.0.0", "Legacy Guide");
        let (newer, bytes) = fixture_package(&old, "1.1.0", "Journal");
        let mut catalog: serde_json::Value = serde_json::from_slice(include_bytes!(
            "../../../../../../plugin-runtime/tests/fixtures/marketplace/catalog.json"
        ))
        .unwrap();
        catalog["plugins"][0]["versions"][0]["manifest"] =
            serde_json::to_value(&old.manifest).unwrap();
        catalog["plugins"][0]["versions"][0]["sha256"] = old.digest.clone().into();
        let mut release = catalog["plugins"][0]["versions"][0].clone();
        release["version"] = "1.1.0".into();
        release["manifest"] = serde_json::to_value(&newer.manifest).unwrap();
        release["sha256"] = newer.digest.clone().into();
        release["changelog"] = "Reviewed newer version; installed copy stays 1.0.0".into();
        catalog["plugins"][0]["versions"]
            .as_array_mut()
            .unwrap()
            .insert(0, release);
        catalog["plugins"][0]["maintainers"] = serde_json::json!([
            "maintainer-one",
            "maintainer-two",
            "maintainer-three",
            "maintainer-four",
            "maintainer-five",
            "maintainer-six"
        ]);
        // An inert reviewed history is long enough to exercise the real list viewport, never extra downloads.
        for patch in 1..=12 {
            let mut older = catalog["plugins"][0]["versions"][1].clone();
            let version = format!("0.9.{patch}");
            older["version"] = version.clone().into();
            older["manifest"]["version"] = version.into();
            catalog["plugins"][0]["versions"]
                .as_array_mut()
                .unwrap()
                .push(older);
        }
        let server = Server::from_bytes(&serde_json::to_vec(&catalog).unwrap(), bytes);
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join(".runtime-plugin-test");
        let environment = Environment {
            workspace: home.path().display().to_string(),
            ..Default::default()
        };
        let mut manager = plugin_runtime::Manager::open(root.clone(), environment.clone()).unwrap();
        manager
            .install(&old, old.manifest.permissions.clone())
            .unwrap();
        manager.shutdown();
        drop(manager);
        let worker = Arc::new(Worker::start_background(root.clone(), environment, true));
        wait(&worker, |state| state.ready);
        worker.refresh_market(
            root.join("marketplace/catalog.json"),
            format!("{}/catalog.json", server.url),
        );
        wait(&worker, |state| state.market.fresh());
        let workspace = Workspace::open(home.path()).unwrap();
        let slot = Rc::new(RefCell::new(None));
        let capture = slot.clone();
        let (_, editor) = cx.add_window_view(move |window, cx| {
            let app = cx.new(|cx| EditorApp::new(workspace, None, window, cx));
            *capture.borrow_mut() = Some(app.clone());
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
            panel
        });
        let dialog = editor
            .update(|_, cx| cx.windows())
            .into_iter()
            .find(|window| !before.contains(window))
            .unwrap();
        let form = VisualTestContext::from_window(dialog, editor).into_mut();
        draw(form);
        // Installed discovery exposes the reviewed upgrade without changing its local details or downloading it.
        assert!(
            form.debug_bounds("market-install").is_some(),
            "installed details must offer the reviewed 1.1.0 upgrade"
        );
        assert!(form.debug_bounds("plugin-row-test.notes-update").is_some());
        let local_sidebar = form.debug_bounds("plugin-manager-sidebar").unwrap();
        let local_detail = form.debug_bounds("plugin-manager-detail").unwrap();
        // Searching the old local name must not discard the chosen ID when the reviewed version is renamed.
        let search = form.debug_bounds("plugin-manager-search").unwrap();
        form.simulate_click(search.center(), Default::default());
        form.simulate_input("Legacy");
        draw(form);
        click(form, "plugin-detail-tabs-3");
        click(form, "market-version-1.1.0");
        assert_eq!(
            form.debug_bounds("plugin-manager-sidebar").unwrap(),
            local_sidebar
        );
        assert_eq!(
            form.debug_bounds("plugin-manager-detail").unwrap(),
            local_detail
        );
        assert!(
            form.debug_bounds("plugin-row-test.notes").is_some(),
            "renamed history keeps its exact identity"
        );
        presentation::check_details(form);
        check_local_actions(form, &panel, &worker, &server, &root);
        worker.tx.send(Work::Shutdown(None)).unwrap();
    }
    rust_i18n::set_locale(&previous);
}

/// Observe real disable/project-enable/restart/uninstall while another release remains browsed.
fn check_local_actions(
    form: &mut VisualTestContext,
    panel: &Entity<ExtensionPanel>,
    worker: &Worker,
    server: &Server,
    root: &Path,
) {
    assert!(
        form.debug_bounds("plugin-manager-browse-version-1.1.0")
            .is_some()
    );
    assert!(
        form.debug_bounds("plugin-manager-installed-version-1.0.0")
            .is_some()
    );
    let global = form.debug_bounds("plugin-global-scope-region").unwrap();
    form.simulate_click(
        point(global.left() + global.size.width * 0.75, global.center().y),
        Default::default(),
    );
    wait(worker, |state| {
        state.progress.is_none()
            && state.entries.iter().any(|entry| {
                entry.manifest.id == "test.notes" && !entry.global_enabled.unwrap_or(entry.enabled)
            })
    });
    form.update(|_, cx| panel.update(cx, |panel, cx| panel.poll(cx)));
    draw(form);
    assert!(
        form.debug_bounds("plugin-restart-action").is_none(),
        "disabled instances have no restart action"
    );
    click(form, "plugin-project-scope-region");
    wait(worker, |state| {
        state.progress.is_none()
            && state
                .entries
                .iter()
                .any(|entry| entry.manifest.id == "test.notes" && entry.enabled)
    });
    form.update(|_, cx| panel.update(cx, |panel, cx| panel.poll(cx)));
    draw(form);
    let epoch = worker.state.lock().unwrap().instance_epochs["test.notes"];
    click(form, "plugin-restart-action");
    wait(worker, |state| {
        state.progress.is_none()
            && state
                .instance_epochs
                .get("test.notes")
                .is_some_and(|next| *next > epoch)
    });
    form.update(|_, cx| panel.update(cx, |panel, cx| panel.poll(cx)));
    draw(form);
    assert_eq!(
        plugin_runtime::Manager::read_registry(root).unwrap()["test.notes"]
            .manifest
            .version,
        "1.0.0"
    );
    assert_eq!(
        server.downloads.load(std::sync::atomic::Ordering::Acquire),
        0
    );
    click(form, "plugin-detail-tabs-5");
    assert!(form.debug_bounds("plugin-runtime-log").is_some());
    // The same fixed header remains reachable after native font, window-size and display-scale changes.
    form.update(|_, cx| {
        typography::set_font_size(cx, 20.);
        apply_theme(builtin_theme(cx.theme().is_dark()), cx);
    });
    form.simulate_resize(size(px(850.), px(600.)));
    form.simulate_scale_factor_change(1.5);
    draw(form);
    let detail = form.debug_bounds("plugin-manager-detail").unwrap();
    for selector in [
        "market-install",
        "plugin-uninstall-action-region",
        "plugin-restart-action",
        "plugin-global-scope-region",
        "plugin-project-scope-region",
    ] {
        let bounds = form.debug_bounds(selector).unwrap();
        assert!(
            bounds.left() >= detail.left() && bounds.right() <= detail.right(),
            "{selector} escaped the detail frame"
        );
    }
    click(form, "plugin-uninstall-action-region");
    assert!(form.debug_bounds("plugin-remove-consent").is_some());
    click(form, "plugin-remove-cancel");
    assert!(
        plugin_runtime::Manager::read_registry(root)
            .unwrap()
            .contains_key("test.notes")
    );
    click(form, "plugin-uninstall-action-region");
    click(form, "plugin-remove-preserve");
    wait(worker, |state| {
        state.progress.is_none()
            && state
                .entries
                .iter()
                .all(|entry| entry.manifest.id != "test.notes")
    });
    form.update(|_, cx| panel.update(cx, |panel, cx| panel.poll(cx)));
    draw(form);
    assert!(form.update(|_, cx| panel.read(cx).manager_market));
    assert!(
        form.debug_bounds("plugin-manager-browse-version-1.1.0")
            .is_some()
    );
    assert!(
        form.debug_bounds("plugin-manager-installed-version-1.0.0")
            .is_none()
    );
    assert!(form.debug_bounds("plugin-global-scope-region").is_none());
    assert!(form.debug_bounds("plugin-runtime-log").is_none());
    assert_eq!(
        server.downloads.load(std::sync::atomic::Ordering::Acquire),
        0
    );
    check_failed_market_install(form, panel, worker, root);
}

/// Failed first installation has no runtime-log owner; its real digest failure must remain visible.
fn check_failed_market_install(
    form: &mut VisualTestContext,
    panel: &Entity<ExtensionPanel>,
    worker: &Worker,
    root: &Path,
) {
    let catalog = serde_json::to_vec(
        worker
            .state
            .lock()
            .unwrap()
            .market
            .catalog
            .as_deref()
            .unwrap(),
    )
    .unwrap();
    let bad = Server::from_bytes(&catalog, b"invalid reviewed package bytes".to_vec());
    worker.refresh_market(
        root.join("marketplace/catalog.json"),
        format!("{}/catalog.json", bad.url),
    );
    wait(worker, |state| state.market.fresh());
    form.update(|_, cx| panel.update(cx, |panel, cx| panel.poll(cx)));
    draw(form);
    click(form, "market-install");
    assert_eq!(bad.downloads.load(std::sync::atomic::Ordering::Acquire), 0);
    click(form, "confirm-market-install");
    wait(worker, |state| {
        state.progress.is_none()
            && state
                .status
                .as_ref()
                .is_some_and(|status| status.plugin.as_deref() == Some("test.notes"))
    });
    form.update(|_, cx| panel.update(cx, |panel, cx| panel.poll(cx)));
    draw(form);
    if form
        .debug_bounds("plugin-install-progress-close-region")
        .is_some()
    {
        click(form, "plugin-install-progress-close-region");
    }
    assert!(
        form.debug_bounds("plugin-manager-operation-error")
            .is_some(),
        "failed first installation has a visible cause"
    );
    assert!(
        plugin_runtime::Manager::read_registry(root)
            .unwrap()
            .is_empty()
    );
    assert_eq!(bad.downloads.load(std::sync::atomic::Ordering::Acquire), 1);
}
