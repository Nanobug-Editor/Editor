//! Recorded native demand transport distinguishes asynchronous admission from withdrawn preparation.
use super::*;
use crate::*;
use gpui_kit::{TestAppContext, VisualTestContext, gpui};
use std::cell::RefCell;

/// An actor can reject a queued original purpose before reserving a language plan. Repeated native
/// frames must retain that failure, rather than filling the worker channel with the same purpose.
#[gpui::test]
fn asynchronous_language_admission_rejection_does_not_requeue_each_frame(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (app, worker, visual, mut manager) = setup(cx, directory.path());
    let work = one_demand(&worker);
    manager.set_workspace_trust(false).unwrap();
    let mut demands = demands::Demands::default();
    assert!(
        demands
            .dispatch(work, &mut manager, &worker.state, true)
            .is_none()
    );
    assert!(demands.is_empty(), "rejection has no admitted preparation");
    for _ in 0..12 {
        frame(&app, visual);
        assert!(
            take_demands(&worker).is_empty(),
            "admission rejection was requeued"
        );
    }
    assert!(
        visual.update(|_, cx| app
            .read(cx)
            .language_service_states
            .values()
            .any(|state| matches!(
                state,
                crate::app::language_servers::ServiceLoadState::Failed(_)
            )))
    );
    assert!(
        worker
            .state
            .lock()
            .unwrap()
            .language_document_bindings
            .is_empty()
    );
    manager.shutdown_checked().unwrap();
}

/// An accepted purpose can be cancelled before preparation publishes any error. The native source
/// still owns the same intent, so its terminal cancellation must not become an automatic retry loop.
#[gpui::test]
fn cancelled_admitted_language_preparation_does_not_requeue_without_published_error(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let (app, worker, visual, mut manager) = setup(cx, directory.path());
    let work = one_demand(&worker);
    let (provider, document, availability) = match &work {
        Work::Demand {
            reason:
                plugin_runtime::DemandReason::LanguageDocument {
                    provider, document, ..
                },
            availability,
            ..
        } => (provider.clone(), document.clone(), availability.clone()),
        _ => panic!("the native source must request its actual document context"),
    };
    let mut demands = demands::Demands::default();
    assert!(
        demands
            .dispatch(work, &mut manager, &worker.state, true)
            .is_none()
    );
    assert!(
        !demands.is_empty(),
        "Manager must accept the original purpose before cancellation"
    );
    assert!(manager.language_services().is_empty());

    // Cancel through the public Manager document lease boundary before any preparation tick. This
    // reaches RequestUpdate::Cancelled while the native transport guard and source remain unchanged.
    manager.release_language_document(&provider, &document);
    assert!(manager.language_services().is_empty());
    assert!(manager.language_document_bindings().is_empty());
    demands.poll(&mut manager, &worker.state, true);
    assert!(demands.is_empty(), "the cancelled wait must finish once");
    assert_eq!(
        manager.availability(availability.plugin()).unwrap(),
        availability,
        "cancellation must preserve the same discoverable original intent"
    );
    frame(&app, visual);
    assert!(
        take_demands(&worker).is_empty(),
        "cancelled admitted preparation was requeued without a published error"
    );
    assert!(
        visual.update(|_, cx| app
            .read(cx)
            .language_service_states
            .values()
            .any(|state| matches!(
                state,
                crate::app::language_servers::ServiceLoadState::Failed(_)
            )))
    );
    for _ in 0..12 {
        frame(&app, visual);
        assert!(
            take_demands(&worker).is_empty(),
            "cancelled admitted preparation was requeued without a published error"
        );
    }
    assert!(
        worker
            .state
            .lock()
            .unwrap()
            .language_document_bindings
            .is_empty(),
        "no prepared error publication was ever delivered"
    );
    manager.shutdown_checked().unwrap();
}

/// A real Manager-prepared error may be withdrawn before the native frame ever observes it.
/// Both orderings must issue one fresh purpose after withdrawal and retain its pending wait.
#[gpui::test]
fn withdrawn_prepared_language_error_requeues_once_even_before_ui_observation(
    cx: &mut TestAppContext,
) {
    for observe_first in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let (app, worker, visual, mut manager) = setup(cx, directory.path());
        let work = one_demand(&worker);
        let original = match &work {
            Work::Demand {
                reason,
                availability,
                ..
            } => (reason.clone(), availability.clone()),
            _ => unreachable!(),
        };
        let mut demands = demands::Demands::default();
        assert!(
            demands
                .dispatch(work, &mut manager, &worker.state, true)
                .is_none()
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            manager.poll();
            let failures = manager.language_services();
            if failures.values().any(Result::is_err) {
                let bindings = manager.language_document_bindings();
                assert!(
                    !bindings.is_empty(),
                    "real prepared failure owns a document intent"
                );
                let mut state = worker.state.lock().unwrap();
                state.language_services = failures;
                state.language_document_bindings = bindings;
                state.configuration_revision += 1;
                break;
            }
            assert!(
                Instant::now() < deadline,
                "missing executable did not produce a public failure"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        if observe_first {
            demands.poll(&mut manager, &worker.state, true);
            for _ in 0..3 {
                frame(&app, visual);
            }
            assert!(
                take_demands(&worker).is_empty(),
                "a still-published failure stays cached"
            );
        }
        // This is the existing mock Worker publication seam. Runtime collection invalidation is
        // covered separately by the actual ecosystem package scenario; no Broker internals are used.
        {
            let mut state = worker.state.lock().unwrap();
            state.language_services.clear();
            state.language_document_bindings.clear();
            state.configuration_revision += 1;
        }
        // In the first ordering, deliver the actual admitted Manager result only after withdrawal.
        // The internal transport phase must survive even though no UI frame saw the old publication.
        demands.poll(&mut manager, &worker.state, true);
        for _ in 0..4 {
            frame(&app, visual);
        }
        let renewed = one_demand(&worker);
        match &renewed {
            Work::Demand {
                reason,
                availability,
                ..
            } => {
                assert_eq!(reason, &original.0);
                assert_eq!(availability, &original.1);
            }
            _ => unreachable!(),
        }
        for _ in 0..8 {
            frame(&app, visual);
            assert!(
                take_demands(&worker).is_empty(),
                "pending renewal accumulated duplicates"
            );
        }
        drop(renewed);
        manager.shutdown_checked().unwrap();
    }
}

/// A real resource-only unknown-language package installs cold. Its missing executable makes public
/// preparation fail without WASM, a native child, a network source or a thousand quota fixtures.
fn setup<'a>(
    cx: &'a mut TestAppContext,
    root: &Path,
) -> (
    Entity<EditorApp>,
    Arc<Worker>,
    &'a mut VisualTestContext,
    Manager,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
    });
    let project_root = root.join("workspace");
    std::fs::create_dir(&project_root).unwrap();
    let workspace = Workspace::open(&project_root).unwrap();
    let path = workspace.root().join("sample.rejection");
    std::fs::write(&path, "unchanged native input\n").unwrap();
    // The shell must read the same private package/asset store as the public Manager. Merely
    // publishing Installed metadata cannot make another profile's plugin.toml exist there.
    let profile = root.join("profile");
    let store = crate::extensions::profile_runtime_root(&profile);
    let manifest = json!({
        "id":"unknown-native-failure", "name":"Unknown native failure", "version":"1.0.0", "protocol":7,
        "api":{"base":"^1","required":{"language.lsp":"^1.5","process":"^1","dependencies":"^1.2"}},
        "contributions":"plugin.toml", "permissions":["process.service.analysis"], "storage_limit":1024,
        "services":{"analysis":{"program":root.join("never-created-analyzer.exe")}},
        "language_servers":[{"id":"analysis","language":"unfamiliar-failure-language","service":"analysis"}]
    });
    let package = Package::from_files(BTreeMap::from([
        ("manifest.json".into(), serde_json::to_vec(&manifest).unwrap()),
        ("plugin.toml".into(), b"[plugin]\nid='unknown-native-failure'\nname='Unknown native failure'\nversion='1.0.0'\nhost_version='^0.1'\n[[language_definitions]]\nid='unfamiliar-failure-language'\nname='Unfamiliar failure'\nextensions=['rejection']\n".to_vec()),
    ])).unwrap();
    let mut manager = Manager::open(
        store.clone(),
        Environment {
            workspace: workspace.root().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    assert!(
        manager.language_services().is_empty(),
        "installation stays cold"
    );
    let slot = Rc::new(RefCell::new(None));
    let delivered = slot.clone();
    let session = SessionState::load_in_profile(workspace.root(), &profile);
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let app =
            cx.new(|cx| EditorApp::new_with_session(workspace, None, Some(session), window, cx));
        *delivered.borrow_mut() = Some(app.clone());
        Root::new(app, window, cx)
    });
    let app = slot.borrow_mut().take().unwrap();
    let worker = visual.update(|_, cx| app.read(cx).extensions.read(cx).worker.clone());
    worker.state.lock().unwrap().publish_manager(&mut manager);
    frame(&app, visual);
    visual.update(|_, cx| {
        let app = app.read(cx);
        let panel = app.extensions.read(cx);
        assert!(
            app.session_state.workspace_trusted,
            "private fixture session must be trusted"
        );
        assert_eq!(
            panel.root.canonicalize().unwrap(),
            store.canonicalize().unwrap()
        );
        assert_eq!(
            app.providers.store_root().canonicalize().unwrap(),
            store.canonicalize().unwrap()
        );
        assert_eq!(app.language_for_path(&path), "unfamiliar-failure-language");
        assert_eq!(
            app.providers
                .language_servers()
                .get("unfamiliar-failure-language"),
            Some(&Some("unknown-native-failure/analysis".into())),
            "actual installed assets must publish the unknown selected language provider"
        );
        assert!(panel.language_provider_has_contexts("unknown-native-failure/analysis"));
        assert!(
            panel.availability("unknown-native-failure").is_some(),
            "the recorded actor must publish the Manager's opaque availability"
        );
    });
    visual.update(|window, cx| {
        app.update(cx, |app, cx| app.open_file(path, window, cx));
        window.draw(cx).clear(cx);
    });
    frame(&app, visual);
    (app, worker, visual, manager)
}

/// Drive ordinary published registry reconciliation and actual native render; no context record is edited.
fn frame(app: &Entity<EditorApp>, visual: &mut VisualTestContext) {
    visual.run_until_parked();
    visual.update(|window, cx| {
        app.read(cx)
            .extensions
            .clone()
            .update(cx, |panel, cx| panel.poll(cx));
        app.update(cx, |app, cx| app.sync_runtime_contributions(window, cx));
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

/// Inspect the existing recorded transport, retaining only real language demand messages.
fn take_demands(worker: &Worker) -> Vec<Work> {
    worker
        .recorded
        .lock()
        .unwrap()
        .try_iter()
        .filter(|work| matches!(work, Work::Demand { .. }))
        .collect()
}

/// Exactly one native source owns the original or renewed pending purpose.
fn one_demand(worker: &Worker) -> Work {
    let mut pending = take_demands(worker);
    assert_eq!(
        pending.len(),
        1,
        "one source must queue exactly one preparation"
    );
    pending.pop().unwrap()
}
