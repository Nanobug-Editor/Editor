//! Restored native geometry must not abandon the first history page after background activation.
use super::*;

/// Report content actually published to the native canvas, without consulting guest-private state.
fn paints_title(manager: &Manager, title: &str) -> bool {
    let Some(node) = manager.live["git"].views["history"].active_node("log") else {
        return false;
    };
    let ui::Kind::Canvas(canvas) = &node.kind else {
        panic!("history log must remain a canvas");
    };
    canvas.paint.iter().any(|paint| {
        matches!(paint,
            plugin_runtime::plugin_protocol::Paint::Text { text, .. }
                | plugin_runtime::plugin_protocol::Paint::TextBox { text, .. }
                if text == title
        )
    })
}

/// Ordinary visibility restoration starts observation without the explicit Refresh command.
fn visibility(manager: &mut Manager, visible: bool) {
    manager
        .event(
            "git",
            Some("history".into()),
            api::Notification::PanelVisibility {
                panel: "history".into(),
                visible,
            },
        )
        .unwrap();
}

/// Mounting and later restoring dock geometry use current admitted canvas metadata.
fn resize(manager: &mut Manager, width: f32) {
    let revision = manager.live["git"].views["history"].revision;
    manager
        .event(
            "git",
            Some("history".into()),
            api::Notification::Ui(ui::UiEvent {
                revision,
                node: "log".into(),
                action: ui::Action::Canvas(ui::CanvasEvent::Resize {
                    width,
                    height: 240.,
                    grid: None,
                }),
            }),
        )
        .unwrap();
}

/// Track the first published row while finishing only this round's real native receipts. Eight
/// quiet polls also detect abandoned work promptly, without giving the periodic timer a retry.
fn finish_first_round(manager: &mut Manager, title: &str, started: Instant) -> Option<Duration> {
    let mut first_row = None;
    let mut quiet = 0;
    loop {
        manager.poll();
        let requests = manager.take_editor_requests();
        let had_requests = !requests.is_empty();
        for (_, request) in requests {
            match request.operation().clone() {
                api::EditorOperation::ListDocuments => {
                    request.finish(Ok(api::EditorValue::Documents {
                        documents: vec![],
                        active: None,
                    }));
                }
                api::EditorOperation::SetPanelVisibility { panel, visible } => {
                    request.finish(Ok(api::EditorValue::PanelVisibility { panel, visible }));
                }
                operation => panic!("unexpected startup editor request: {operation:?}"),
            }
        }
        if first_row.is_none() && paints_title(manager, title) {
            first_row = Some(started.elapsed());
        }
        quiet = if !had_requests && manager.live["git"].process_count() == 0 {
            quiet + 1
        } else {
            0
        };
        if quiet >= 8 {
            return first_row;
        }
        assert!(
            started.elapsed() < Duration::from_secs(15),
            "initial query round must complete before the periodic retry timer"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// An installed package reopens under its background purpose. Hide the panel while its first
/// repository discovery runs, exactly as a host restoring tools before mounting their content.
fn reopen_hidden(package: &Package, root: &Path) -> (tempfile::TempDir, Manager) {
    let profile = tempfile::tempdir().unwrap();
    let runtime = profile.path().join("plugins");
    let environment = Environment {
        workspace: root.display().to_string(),
        os: "windows".into(),
        locale: "en".into(),
        ..Default::default()
    };
    let mut installed = Manager::open(runtime.clone(), environment.clone()).unwrap();
    installed
        .install(package, package.manifest.permissions.clone())
        .unwrap();
    installed.shutdown_checked().unwrap();
    drop(installed);

    let mut manager = Manager::open(runtime, environment).unwrap();
    let available = manager.availability("git").unwrap();
    let demand = manager
        .begin_demand(&available, plugin_runtime::DemandReason::Background, 60_000)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        manager.poll();
        if let api::RequestUpdate::Completed { result } = demand.status() {
            result.unwrap();
            break;
        }
        assert!(Instant::now() < deadline, "background activation timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
    visibility(&mut manager, false);
    idle(&mut manager);
    (profile, manager)
}

/// The first layout event follows visibility without waiting for Git. The real public scene
/// must gain its first row in this query round, not remain empty until the thirty-second timer.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; real restored startup and native geometry"]
fn git_package_reopened_history_first_page_survives_initial_canvas_resize() {
    if !run_isolated_test(
        "workbench::history::startup::git_package_reopened_history_first_page_survives_initial_canvas_resize",
    ) {
        return;
    }
    let repository = repository();
    let root = repository.path();
    let title = "first page after restart";
    super::super::commit_file(root, "tracked.txt", "first\n", title);
    let (_profile, mut manager) = reopen_hidden(&candidate(), root);
    assert!(
        manager.live["git"].views["history"]
            .active_node("branches")
            .is_some()
    );
    assert!(
        !paints_title(&manager, title),
        "hidden background initialization must not prefill this first-page fixture"
    );
    let observation = manager.runtime_observations();
    observation.start_capture();
    let started = Instant::now();
    visibility(&mut manager, true);
    resize(&mut manager, 900.);
    let first_row = finish_first_round(&mut manager, title, started);
    let elapsed = started.elapsed();
    let painted = paints_title(&manager, title);
    let native = observation
        .snapshot(Some("git"))
        .records
        .into_iter()
        .filter(|record| record.stage == plugin_runtime::observability::Stage::Native)
        .count();
    eprintln!(
        "restored first page: visible={painted}, first_row_ms={:?}, query_round_ms={}, completed_native_operations={native}, live_processes={}",
        first_row.map(|elapsed| elapsed.as_millis()),
        elapsed.as_millis(),
        manager.live["git"].process_count(),
    );
    manager.shutdown_checked().unwrap();
    assert_eq!(manager.resource_count(), 0);
    assert!(
        painted,
        "restored canvas geometry abandoned the first page; no manual refresh or timer should be needed"
    );
}

/// A host may restore visibility, focus and several layout sizes before Git replies. This
/// finite notification burst still owns just one eventual first page, including an empty repo.
#[test]
#[ignore = "build independent Git candidate; set NANOBUG_GIT_CANDIDATE; real startup notifications and unborn repository"]
fn git_package_reopened_history_notification_burst_and_empty_repository_settle() {
    if !run_isolated_test(
        "workbench::history::startup::git_package_reopened_history_notification_burst_and_empty_repository_settle",
    ) {
        return;
    }
    let package = candidate();
    for populated in [true, false] {
        let repository = repository();
        let root = repository.path();
        let title = "startup burst first page";
        if populated {
            super::super::commit_file(root, "tracked.txt", "first\n", title);
        }
        let (_profile, mut manager) = reopen_hidden(&package, root);
        let started = Instant::now();
        visibility(&mut manager, true);
        resize(&mut manager, 720.);
        manager
            .event("git", None, api::Notification::Focus(true))
            .unwrap();
        visibility(&mut manager, true);
        resize(&mut manager, 960.);
        let first_row = finish_first_round(&mut manager, title, started);
        assert_eq!(
            first_row.is_some(),
            populated,
            "restored input must finish the initial page without fabricating unborn commits"
        );
        // An empty result is terminal. More polling must not keep starting immediate retries.
        for _ in 0..12 {
            manager.poll();
            assert_eq!(manager.live["git"].process_count(), 0);
            assert!(manager.take_editor_requests().is_empty());
            std::thread::sleep(Duration::from_millis(10));
        }
        eprintln!(
            "restored notification burst: populated={populated}, first_row_ms={:?}, settled_ms={}",
            first_row.map(|elapsed| elapsed.as_millis()),
            started.elapsed().as_millis()
        );
        manager.shutdown_checked().unwrap();
        assert_eq!(manager.resource_count(), 0);
    }
}
