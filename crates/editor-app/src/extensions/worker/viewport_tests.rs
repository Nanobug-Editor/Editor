//! Exercise delayed source measurements through the real worker and delivered guest.
use super::*;

/// Poll a published outcome with a finite deadline; the production actor owns all guest calls.
fn wait_for(worker: &Worker, ready: impl Fn(&Published) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if ready(&worker.state.lock().unwrap()) {
            return;
        }
        assert!(Instant::now() < deadline, "viewport worker timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// A new preview can overtake the previous frame's source measurement in the actor queue.
/// Reject that obsolete input without a user-facing error, but retain malformed-input errors.
#[test]
#[ignore = "opt-in real WASM/native integration; uses fixed reviewed test packages and required pinned native tools"]
fn stale_source_viewport_is_discarded_without_plugin_error() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("plugins");
    let environment = Environment {
        workspace: directory.path().display().to_string(),
        ..Default::default()
    };
    let package = Package::read(&crate::test_packages::archive("markdown")).unwrap();
    let mut manager = Manager::open(root.clone(), environment.clone()).unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    drop(manager);
    let worker = Worker::start_background(root, environment, true);
    wait_for(&worker, |state| state.ready);
    let availability = worker.state.lock().unwrap().availabilities["markdown"].clone();
    let mut demand = worker
        .demand(
            availability,
            plugin_runtime::DemandReason::View("preview".into()),
            None,
        )
        .unwrap();
    futures::executor::block_on(demand.wait()).unwrap();
    wait_for(&worker, |state| {
        state.instance_epochs.contains_key("markdown")
    });
    let epoch = worker.state.lock().unwrap().instance_epochs["markdown"];
    let send = |event| {
        worker
            .tx
            .send(Work::Event(
                "markdown".into(),
                epoch,
                Some("preview".into()),
                event,
            ))
            .unwrap()
    };
    let document = api::DocumentVersion {
        id: "document".into(),
        path: "notes.md".into(),
        revision: 1,
    };
    // Mirror the native host's ordered file/text binding so the real footer tools are present.
    let publish = |document: &api::DocumentVersion, text: &str| {
        send(api::Notification::FilePreview {
            file: Some(api::FileContext {
                version: api::FileVersion {
                    id: "file".into(),
                    path: "notes.md".into(),
                    revision: 1,
                },
                file_type: "md".into(),
                text: Some(document.clone()),
            }),
        });
        send(api::Notification::Preview {
            document: Some(document.clone()),
            text: text.into(),
        });
    };
    publish(&document, "# First\n\nParagraph");
    wait_for(&worker, |state| {
        state
            .views
            .get("markdown/preview")
            .is_some_and(|scene| scene.source.as_ref() == Some(&document))
    });
    let revision = worker.state.lock().unwrap().views["markdown/preview"].revision;
    let tool = worker.state.lock().unwrap().views["markdown/preview"]
        .tools
        .iter()
        .find(|tool| tool.id == "display-sync")
        .unwrap()
        .clone();
    let stale_preview = {
        let state = worker.state.lock().unwrap();
        let scene = &state.views["markdown/preview"];
        let mut block = None;
        scene.root.visit(&mut |node| {
            if block.is_none() && matches!(node.kind, ui::Kind::RichText { .. }) {
                block = node.source_range.map(|range| (node.id.clone(), range));
            }
        });
        let (block, source_range) = block.expect("actual mapped preview paragraph");
        api::Notification::Ui(ui::UiEvent {
            node: scene.editor_viewport.clone().unwrap(),
            revision,
            action: ui::Action::Viewport(api::PreviewViewport {
                block,
                source_range,
                fraction: 0.25,
                origin: None,
                layout: false,
            }),
        })
    };
    let next = api::DocumentVersion {
        revision: 2,
        ..document.clone()
    };
    publish(&next, "# First\n\nParagraph edited");
    wait_for(&worker, |state| {
        state
            .views
            .get("markdown/preview")
            .is_some_and(|scene| scene.source.as_ref() == Some(&next))
    });
    send(api::Notification::Tool(ui::ToolEvent {
        revision,
        tool: tool.id,
        target: tool.target,
    }));
    for _ in 0..32 {
        let source = api::Notification::SourceViewport(api::SourceViewport {
            document: document.clone(),
            ui_revision: revision,
            offset: 0,
            line_fraction: 0.25,
            origin: None,
            layout: false,
        });
        send(source.clone());
        send(stale_preview.clone());
        // An already retired native owner must also discard its high-frequency callback quietly.
        worker
            .tx
            .send(Work::Event(
                "markdown".into(),
                epoch - 1,
                Some("preview".into()),
                source,
            ))
            .unwrap();
    }
    // A following valid source publication is the actor barrier: stale positions must be
    // discarded silently before it, rather than filling the user's informational log.
    let final_document = api::DocumentVersion {
        revision: 3,
        ..next.clone()
    };
    publish(&final_document, "# First\n\nLatest paragraph");
    wait_for(&worker, |state| {
        state
            .views
            .get("markdown/preview")
            .is_some_and(|scene| scene.source.as_ref() == Some(&final_document))
    });
    let stale_logs = worker
        .state
        .lock()
        .unwrap()
        .logs
        .records("markdown")
        .iter()
        .filter(|entry| matches!(entry.source.as_str(), "host.ui.stale" | "host.ui.retired"))
        .count();
    let (status, severity, current) = {
        let state = worker.state.lock().unwrap();
        (
            state.status.is_some(),
            state.logs.unread_severity("markdown"),
            state.views["markdown/preview"].source.clone(),
        )
    };
    assert!(
        !status && stale_logs == 0,
        "obsolete native input must be silent: error status={status}, stale scroll messages={stale_logs}"
    );
    assert_eq!(severity, None);
    assert_eq!(current.as_ref(), Some(&final_document));
    // Validation runs before revision checks: a zero origin is invalid even on an obsolete scene.
    send(api::Notification::SourceViewport(api::SourceViewport {
        document,
        ui_revision: revision,
        offset: 0,
        line_fraction: 0.25,
        origin: Some(0),
        layout: false,
    }));
    wait_for(&worker, |state| state.status.is_some());
    assert_eq!(
        worker
            .state
            .lock()
            .unwrap()
            .logs
            .unread_severity("markdown"),
        Some(plugin_runtime::logs::LogLevel::Error)
    );
    let (tx, rx) = futures::channel::oneshot::channel();
    worker.tx.send(Work::Shutdown(Some(tx))).unwrap();
    // A delivered receipt is not success unless the owned runtime also retired successfully.
    futures::executor::block_on(rx).unwrap().unwrap();
}
