//! Real candidate ZIPs retain reviewed installation authority and project-only panel consent.
use super::*;
use gpui_kit::AppContext as _;
use std::collections::BTreeSet;

/// A due UI check must leave the confirmed catalog intact until the active download has committed.
#[gpui_kit::gpui::test]
fn marketplace_scheduled_refresh_does_not_withdraw_active_installation(
    cx: &mut gpui_kit::TestAppContext,
) {
    let server = Server::new();
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join("runtime");
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
    server.pause.store(true, Ordering::Release);
    assert!(worker.install_market(release, generation, Default::default(), false));
    wait(&worker, |_| server.downloads.load(Ordering::Acquire) == 1);
    let retry_due = cx.update(|cx| {
        gpui_kit::init(cx);
        crate::typography::init(cx);
        crate::apply_theme(crate::builtin_theme(false), cx);
        let owner =
            cx.new(|cx| crate::extensions::ExtensionPanel::new_application(home.path(), cx));
        owner.update(cx, |owner, _| {
            owner.worker = worker.clone();
            owner.market_view = crate::extensions::marketplace::ViewState::default();
            owner.refresh_online_market();
            owner.market_view.needs_check(Instant::now())
        })
    });
    server.pause.store(false, Ordering::Release);
    wait(&worker, |state| state.progress.is_none());
    let (status, current_generation, fresh) = {
        let state = worker.state.lock().unwrap();
        (
            state.status.as_ref().map(|status| status.message.clone()),
            state.market.generation(),
            state.market.fresh(),
        )
    };
    let installed = Manager::read_registry(&root).unwrap();
    let (tx, rx) = futures::channel::oneshot::channel();
    worker.tx.send(Work::Shutdown(Some(tx))).unwrap();
    futures::executor::block_on(rx).unwrap().unwrap();
    assert!(
        status.is_none(),
        "a UI refresh must not abort confirmed installation: {status:?}"
    );
    assert_eq!(current_generation, generation);
    assert!(fresh);
    assert!(retry_due, "completion must leave the deferred check due");
    assert!(installed.contains_key("test.notes"));
}

/// An explicit actor refresh still invalidates old consent; UI deferral cannot weaken that boundary.
#[test]
fn marketplace_explicit_refresh_still_withdraws_active_download_authority() {
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
    let cache = root.join("marketplace/catalog.json");
    let url = format!("{}/catalog.json", server.url);
    worker.refresh_market(cache.clone(), url.clone());
    wait(&worker, |state| state.market.fresh());
    let (release, generation) = {
        let state = worker.state.lock().unwrap();
        (
            state.market.catalog.as_ref().unwrap().plugins[0].versions[0].clone(),
            state.market.generation(),
        )
    };
    server.pause.store(true, Ordering::Release);
    assert!(worker.install_market(release, generation, Default::default(), false));
    wait(&worker, |_| server.downloads.load(Ordering::Acquire) == 1);
    worker.refresh_market(cache, url);
    server.pause.store(false, Ordering::Release);
    wait(&worker, |state| state.progress.is_none());
    let state = worker.state.lock().unwrap();
    assert!(state.status.as_ref().is_some_and(|status| {
        status
            .message
            .contains("Installation authority was withdrawn")
    }));
    assert!(state.entries.is_empty());
    drop(state);
    assert!(Manager::read_registry(&root).unwrap().is_empty());
    let (tx, rx) = futures::channel::oneshot::channel();
    worker.tx.send(Work::Shutdown(Some(tx))).unwrap();
    futures::executor::block_on(rx).unwrap().unwrap();
}

/// An independent package follows the market actor, cold demand and native visibility ingress.
#[test]
#[ignore = "set NANOBUG_MARKET_INSTALL_CANDIDATE to an independently built ui.visibility package ZIP"]
fn marketplace_candidate_project_install_keeps_authority_and_panel_consent() {
    install_candidate(None);
}

/// A live predecessor exercises the same reviewed authority during background update and cutover.
#[test]
#[ignore = "set NANOBUG_MARKET_INSTALL_CANDIDATE and NANOBUG_MARKET_INSTALL_PREVIOUS to independent package ZIPs"]
fn marketplace_candidate_project_update_keeps_authority_and_panel_consent() {
    let previous = std::env::var_os("NANOBUG_MARKET_INSTALL_PREVIOUS")
        .expect("set the independently built predecessor ZIP");
    install_candidate(Some(
        Package::read(std::path::Path::new(&previous)).unwrap(),
    ));
}

/// Serve `previous`'s successor through real loopback HTTP; all runtime data belongs to a temporary profile.
fn install_candidate(previous: Option<Package>) {
    let candidate = std::env::var_os("NANOBUG_MARKET_INSTALL_CANDIDATE")
        .expect("set the independently built candidate ZIP");
    let candidate_path = std::path::Path::new(&candidate);
    let bytes = std::fs::read(candidate_path).unwrap();
    let package = Package::from_bytes(&bytes).unwrap();
    let id = &package.manifest.id;
    let panel = &package.manifest.panels[0].id;
    let mut catalog: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../../../../plugin-runtime/tests/fixtures/marketplace/catalog.json"
    ))
    .unwrap();
    catalog["plugins"][0]["id"] = id.clone().into();
    let release = &mut catalog["plugins"][0]["versions"][0];
    release["version"] = package.manifest.version.clone().into();
    release["manifest"] = serde_json::to_value(&package.manifest).unwrap();
    release["sha256"] = package.digest.clone().into();
    release["host_version"] = package
        .manifest
        .contributions
        .as_ref()
        .map_or_else(
            || "*".to_string(),
            |path| {
                plugin_schema::PluginManifest::parse(
                    std::str::from_utf8(&package.files[path]).unwrap(),
                )
                .unwrap()
                .plugin
                .host_version
            },
        )
        .into();
    let mut platforms = package
        .manifest
        .services
        .values()
        .filter(|service| !service.platforms.is_empty())
        .map(|service| service.platforms.iter().cloned().collect::<BTreeSet<_>>());
    release["platforms"] =
        serde_json::to_value(platforms.next().map_or_else(BTreeSet::new, |first| {
            platforms.fold(first, |common, set| {
                common.intersection(&set).cloned().collect()
            })
        }))
        .unwrap();
    let server = Server::from_bytes(&serde_json::to_vec(&catalog).unwrap(), bytes);
    let home = tempfile::tempdir().unwrap();
    let workspace = home.path().join("workspace");
    std::fs::create_dir(&workspace).unwrap();
    assert!(
        std::process::Command::new("git")
            .args(["init", "--quiet"])
            .arg(&workspace)
            .status()
            .unwrap()
            .success()
    );
    let environment = Environment {
        os: std::env::consts::OS.into(),
        workspace: workspace.display().to_string(),
        ..Default::default()
    };
    let root = home.path().join("runtime");
    if let Some(previous) = &previous {
        assert_eq!(previous.manifest.id, *id);
        let mut manager = Manager::open(root.clone(), environment.clone()).unwrap();
        manager
            .install_with_control(
                previous,
                previous.manifest.permissions.clone(),
                &plugin_runtime::InstallControl::default().with_default_global_enabled(false),
            )
            .unwrap();
        manager.shutdown_checked().unwrap();
    }
    let worker = Worker::start_background(root.clone(), environment.clone(), true);
    wait(&worker, |state| state.ready);
    if previous.is_some() {
        demand_panel(&worker, id, panel);
    }
    worker.refresh_market(
        root.join("marketplace/catalog.json"),
        format!("{}/catalog.json", server.url),
    );
    wait(&worker, |state| state.market.fresh());
    let (reviewed, generation) = {
        let state = worker.state.lock().unwrap();
        (
            state.market.catalog.as_ref().unwrap().plugins[0].versions[0].clone(),
            state.market.generation(),
        )
    };
    assert!(worker.install_market(reviewed, generation, Default::default(), false));
    wait(&worker, |state| state.progress.is_none());
    {
        let state = worker.state.lock().unwrap();
        assert!(
            state.status.is_none(),
            "installation: {:?}",
            state.status.as_ref().map(|status| &status.message)
        );
    }
    let installed = Manager::read_registry(&root).unwrap();
    assert_eq!(installed[id].digest, package.digest);
    assert!(!installed[id].enabled);
    assert!(installed[id].project_enabled_in(&environment.workspace));
    demand_panel(&worker, id, panel);
    let epoch = worker
        .state
        .lock()
        .unwrap()
        .live_instance_epoch(id)
        .unwrap();
    for visible in [true, false, true] {
        worker
            .tx
            .send(Work::Event(
                id.clone(),
                epoch,
                Some(panel.clone()),
                api::Notification::PanelVisibility {
                    panel: panel.clone(),
                    visible,
                },
            ))
            .unwrap();
    }
    // Inspection is a FIFO receipt after the observations, so a quiet status cannot be an early sample.
    worker
        .tx
        .send(Work::Inspect(candidate_path.into()))
        .unwrap();
    wait(&worker, |state| state.pending.is_some());
    let state = worker.state.lock().unwrap();
    assert!(
        state.status.is_none(),
        "visibility: {:?}",
        state.status.as_ref().map(|status| &status.message)
    );
    assert!(state.views.contains_key(&format!("{id}/{panel}")));
    drop(state);
    let (tx, rx) = futures::channel::oneshot::channel();
    worker.tx.send(Work::Shutdown(Some(tx))).unwrap();
    futures::executor::block_on(rx).unwrap().unwrap();
}

/// Demand the declared `panel` through `worker`'s current static identity and wait for real activation.
fn demand_panel(worker: &Worker, id: &str, panel: &str) {
    let availability = worker.state.lock().unwrap().availabilities[id].clone();
    let mut wait = worker
        .demand(
            availability,
            plugin_runtime::DemandReason::View(panel.into()),
            None,
        )
        .unwrap();
    futures::executor::block_on(wait.wait()).unwrap();
    super::wait(worker, |state| state.live_instance_epoch(id).is_some());
}
