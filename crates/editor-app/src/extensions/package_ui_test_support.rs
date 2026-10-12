//! Shared actual-package transport and native input for file layouts and toolbar acceptance.
use super::*;
use gpui_kit::VisualTestContext;
use plugin_runtime::Manager;
use std::io::{Cursor, Write};

/// Consume one explicitly requested public role; installation and browsing remain cold.
/// Every fixture caller chooses its actual use instead of preparing every installed package.
pub(crate) fn demand(
    manager: &mut Manager,
    plugin: &str,
    reason: plugin_runtime::DemandReason,
) -> Result<plugin_runtime::ActivationReceipt, protocol::api::Failure> {
    let availability = manager.availability(plugin)?;
    let request = manager.begin_demand(&availability, reason, 60_000)?;
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        manager.poll();
        match request.status() {
            protocol::api::RequestUpdate::Completed { result } => {
                let receipt = result?;
                manager.admit_receipt(&availability, &receipt)?;
                return Ok(receipt);
            }
            protocol::api::RequestUpdate::Cancelled { .. } => {
                return Err(protocol::api::Failure::new(
                    protocol::api::ErrorCode::Cancelled,
                    "Fixture demand cancelled",
                ));
            }
            _ => {}
        }
        assert!(
            Instant::now() < deadline,
            "public fixture demand did not complete"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Publish only real, prepared role leases. A first role birth updates the catalog, not availability.
pub(super) fn publish_catalog(state: &mut worker::Published, manager: &mut Manager) {
    let language = manager.language_services();
    let direct = manager.direct_language_providers();
    let structure = manager.structure_providers();
    let changed = catalog_changed(&state.language_services, &language)
        || catalog_changed(&state.direct_language_providers, &direct)
        || catalog_changed(&state.structure_providers, &structure);
    state.language_services = language;
    state.direct_language_providers = direct;
    state.structure_providers = structure;
    if changed {
        state.configuration_revision += 1;
    }
}

/// Arc identity is the production lease identity; equal failure messages keep their stable snapshot.
fn catalog_changed<T>(
    old: &BTreeMap<String, Result<Arc<T>, String>>,
    new: &BTreeMap<String, Result<Arc<T>, String>>,
) -> bool {
    old.len() != new.len()
        || new.iter().any(|(key, value)| match (old.get(key), value) {
            (Some(Ok(old)), Ok(new)) => !Arc::ptr_eq(old, new),
            (Some(Err(old)), Err(new)) => old != new,
            _ => true,
        })
}

/// Repack one independent component under two identities through the ordinary package validator.
pub(super) fn package(id: &str, name: &str) -> Package {
    let package = Package::read(&crate::test_packages::archive("layout-example"))
        .expect("frozen layout package must be present and match its ledger");
    identify(package, id, name)
}

/// Alternate identities retain the actual shipped component and pass the public archive validator.
pub(super) fn identify(package: Package, id: &str, name: &str) -> Package {
    let mut files = package.files;
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    manifest["id"] = id.into();
    manifest["name"] = name.into();
    if let Some(path) = manifest["contributions"].as_str() {
        // Hybrid packages declare the same owner in both manifests; preserve artwork and other
        // contributions while changing identity instead of weakening the public admission checks.
        let source = std::str::from_utf8(&files[path]).unwrap();
        let mut contributions: toml::Value = toml::from_str(source).unwrap();
        contributions["plugin"]["id"] = toml::Value::String(id.into());
        contributions["plugin"]["name"] = toml::Value::String(name.into());
        files.insert(
            path.into(),
            toml::to_string(&contributions).unwrap().into_bytes(),
        );
    }
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in files {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(&bytes).unwrap();
    }
    Package::from_bytes(&zip.finish().unwrap().into_inner()).unwrap()
}

/// Execute the existing native transport, then publish Manager-owned immutable views at its real boundary.
pub(super) fn pump(app: &Entity<EditorApp>, manager: &mut Manager, cx: &mut App) {
    let owner = app.read(cx).extensions.clone();
    let worker = owner.read(cx).worker.clone();
    let trusted = worker.trusted.load(std::sync::atomic::Ordering::Acquire);
    let mut demands = worker::demands::Demands::default();
    let work = owner
        .read(cx)
        .worker
        .recorded
        .lock()
        .unwrap()
        .try_iter()
        .collect::<Vec<_>>();
    for work in work {
        let work = match work {
            work @ (Work::Demand { .. } | Work::Invoke { .. }) => {
                demands.dispatch(work, manager, &worker.state, trusted);
                continue;
            }
            work => work,
        };
        let work = match work {
            Work::ImportPreference {
                plugin,
                owner,
                workspace,
                key,
                data,
                ..
            } => {
                let succeeded = manager
                    .import_preference(&plugin, key, data.clone())
                    .is_ok();
                owner_receipt(
                    &owner,
                    &workspace,
                    data,
                    succeeded,
                    &app.read(cx).extensions.read(cx).worker,
                );
                continue;
            }
            work => work,
        };
        if let Work::Event(id, epoch, panel, event) = work {
            // A direct lifecycle call can retire the guest between native frames; its queued callbacks
            // lose authority just as production callbacks do when publication advances the instance epoch.
            let mut state = worker.state.lock().unwrap();
            state.reconcile_instances(manager);
            let current = manager
                .instance_id(&id)
                .and_then(|owner| state.owner_epoch(&id, owner));
            drop(state);
            if current != Some(epoch) {
                continue;
            }
            let result = manager.event(&id, panel, event);
            assert!(
                result.is_ok()
                    || result
                        .as_ref()
                        .err()
                        .and_then(|e| e.downcast_ref::<protocol::api::Failure>())
                        .is_some_and(|e| e.code == protocol::api::ErrorCode::StaleRevision),
                "{result:?}"
            );
        }
    }
    let deadline = Instant::now() + Duration::from_secs(60);
    while !demands.is_empty() {
        demands.poll(manager, &worker.state, trusted);
        manager.poll();
        demands.poll(manager, &worker.state, trusted);
        assert!(
            Instant::now() < deadline,
            "native fixture demand did not complete"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let views = manager
        .live
        .iter()
        .flat_map(|(id, live)| {
            live.views
                .iter()
                .map(move |(panel, view)| (format!("{id}/{panel}"), view.clone()))
        })
        .collect();
    let until = Instant::now() + Duration::from_secs(5);
    let resources = loop {
        let resources = manager.image_resources();
        if resources
            .values()
            .all(|image| !matches!(image.state, plugin_runtime::ImageState::Loading))
        {
            break resources;
        }
        assert!(Instant::now() < until, "file image did not finish");
        std::thread::sleep(Duration::from_millis(5));
    };
    let pixels = images::VectorRenderer::default().prepare_resources(&views, &resources);
    owner.update(cx, |owner, cx| {
        let mut state = owner.worker.state.lock().unwrap();
        // Publish recovered ownership/readiness through the same Manager projection as the real actor.
        state.publish_manager(manager);
        publish_catalog(&mut state, manager);
        // Preserve the same approved resource join as the production worker's tree publication.
        state.trees = manager
            .live
            .values()
            .flat_map(|instance| instance.tree_resources())
            .collect();
        state.views = views;
        state.images = pixels;
        drop(state);
        owner.poll(cx);
    });
    let panels = app
        .read(cx)
        .plugin_panels
        .values()
        .cloned()
        .collect::<Vec<_>>();
    for panel in panels {
        panel.update(cx, |panel, cx| panel.poll(cx));
    }
}
/// Native harnesses acknowledge actual Manager writes; they never synthesize stored intent.
pub(super) fn owner_receipt(
    owner: &str,
    workspace: &str,
    data: serde_json::Value,
    succeeded: bool,
    worker: &Worker,
) {
    worker
        .state
        .lock()
        .unwrap()
        .preference_imports
        .push(worker::PreferenceImport {
            owner: owner.into(),
            workspace: workspace.into(),
            data,
            succeeded,
        });
}

/// Flush paint, demand, owner birth and source publication turns through the ordinary native route.
pub(super) fn draw(app: &Entity<EditorApp>, manager: &mut Manager, cx: &mut VisualTestContext) {
    for _ in 0..4 {
        cx.run_until_parked();
        cx.update(|window, cx| {
            pump(app, manager, cx);
            window.draw(cx).clear(cx);
        });
    }
}

/// Select from the actual file Tab popup, capturing the same native context used in production.
pub(super) fn choose(
    index: usize,
    app: &Entity<EditorApp>,
    manager: &mut Manager,
    cx: &mut VisualTestContext,
) {
    let active = cx.update(|_, cx| app.read(cx).active_tab_index().unwrap());
    let tab = cx
        .debug_bounds(["editor-tab-0", "editor-tab-1", "editor-tab-2"][active])
        .unwrap()
        .center();
    cx.simulate_mouse_down(tab, MouseButton::Right, Modifiers::default());
    cx.simulate_mouse_up(tab, MouseButton::Right, Modifiers::default());
    // Drain tab activation's deferred focus before exercising the popup's keyboard owner.
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(cx.update(|_, cx| app.read(cx).file_view_menu.is_none()));
    cx.simulate_mouse_down(tab, MouseButton::Right, Modifiers::default());
    cx.simulate_mouse_up(tab, MouseButton::Right, Modifiers::default());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let steps = cx.update(|_, cx| {
        let items = &app.read(cx).file_view_menu.as_ref().unwrap().read(cx).items;
        assert!(!items[index].disabled);
        items
            .iter()
            .take(index)
            .filter(|item| !item.disabled)
            .count()
    });
    // Navigation skips disabled candidates just as the ordinary native popup does.
    cx.simulate_keystrokes(&format!("home {}enter", "down ".repeat(steps)));
    draw(app, manager, cx);
}
