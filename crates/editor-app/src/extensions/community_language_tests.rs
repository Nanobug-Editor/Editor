//! Actual independent language/checker packages enter the public actor and native document seams.
use super::*;
use gpui_kit::{TestAppContext, VisualTestContext, gpui};
use plugin_runtime::{
    EditorRequest, Manager, Package,
    plugin_protocol::{api, diagnostics, workspace},
};

/// Fixed independently built archives retain their public SDK contracts without entering host payloads.
fn packages(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    workspace: &Path,
    ids: &[&str],
) -> Manager {
    let mut manager = Manager::open(
        runtime_root(workspace),
        protocol::Environment {
            workspace: workspace.display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    for id in ids {
        let package = crate::test_packages::package(&format!("{id}-0.1.0"));
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
    }
    publish_providers(visual, app, &mut manager);
    manager
}

/// Replacements and disablement use the same production contribution snapshot as initial installation.
pub(super) fn publish_providers(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    manager: &mut Manager,
) {
    visual.update(|_, cx| {
        app.read(cx).extensions.clone().update(cx, |panel, cx| {
            let mut state = panel.worker.state.lock().unwrap();
            state.publish_manager(manager);
            super::package_ui_test_support::publish_catalog(&mut state, manager);
            drop(state);
            panel.poll(cx);
        })
    });
    draw(visual);
}

/// Drawing follows real deferred Change effects, so all subsequent calls observe committed revisions.
fn draw(visual: &mut VisualTestContext) {
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
}

/// Production actor ingress is mirrored without constructing handles, authorities or document text.
fn ingress(visual: &mut VisualTestContext, app: &Entity<EditorApp>, manager: &mut Manager) {
    let worker = visual.update(|_, cx| app.read(cx).extensions.read(cx).worker.clone());
    let trusted = worker.trusted.load(std::sync::atomic::Ordering::Acquire);
    let mut demands = super::worker::demands::Demands::default();
    let (changes, files, work) = visual.update(|_, cx| {
        let worker = app.read(cx).extensions.read(cx).worker.clone();
        let mut state = worker.state.lock().unwrap();
        let changes = state.document_events.take_batch(64).unwrap();
        let files = state.file_events.take_batch(64).unwrap();
        let work = worker
            .recorded
            .lock()
            .unwrap()
            .try_iter()
            .collect::<Vec<_>>();
        (changes, files, work)
    });
    for change in changes {
        manager.document_changed(change);
    }
    for file in files {
        manager.workspace_file_event(file);
    }
    for work in work {
        let work = match work {
            work @ (Work::Demand { .. } | Work::Invoke { .. }) => {
                demands.dispatch(work, manager, &worker.state, trusted);
                continue;
            }
            work => work,
        };
        if let Work::LanguageFix {
            origin,
            document,
            fix,
        } = work
        {
            manager.submit_language_fix(&origin, document, fix).unwrap();
        }
    }
    // Only actual native use enters this wait; idle provider metadata must remain cold.
    let deadline = Instant::now() + Duration::from_secs(60);
    while !demands.is_empty() {
        demands.poll(manager, &worker.state, trusted);
        manager.poll();
        demands.poll(manager, &worker.state, trusted);
        assert!(
            Instant::now() < deadline,
            "community native demand did not complete"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    publish_providers(visual, app, manager);
}

/// Execute ordinary published requests through the application's serialized native dispatcher.
fn publish(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    manager: &mut Manager,
    requests: &[(String, EditorRequest)],
) {
    visual.update(|_, cx| {
        app.read(cx).extensions.clone().update(cx, |panel, cx| {
            let mut state = panel.worker.state.lock().unwrap();
            state.publish_manager(manager);
            state.editor_requests.extend(requests.iter().cloned());
            drop(state);
            panel.poll(cx);
        })
    });
    draw(visual);
}

/// Bounded guest state machines drain through actual completion events rather than handcrafted results.
pub(super) fn pump(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    manager: &mut Manager,
) -> Vec<EditorRequest> {
    let mut observed = Vec::new();
    for _ in 0..16 {
        ingress(visual, app, manager);
        manager.poll();
        let requests = manager.take_editor_requests();
        if requests.is_empty() {
            draw(visual);
            break;
        }
        observed.extend(requests.iter().map(|(_, request)| request.clone()));
        publish(visual, app, manager, &requests);
    }
    observed
}

/// This is the consumer's ordinary command interface, with a visible JSON protocol receipt.
fn begin(
    manager: &mut Manager,
    id: &str,
    operation: api::Operation,
) -> Result<api::Value, api::Failure> {
    // The probe is an ordinary declared command; it consumes its own cold package on first use.
    super::package_ui_test_support::demand(
        manager,
        id,
        plugin_runtime::DemandReason::Command("probe".into()),
    )?;
    manager
        .invoke_command(id, "probe", serde_json::to_value(operation).unwrap())
        .unwrap();
    let protocol::ui::Kind::Text { text } =
        &manager.live[id].views.values().next().unwrap().root.kind
    else {
        panic!("Native probe result expected")
    };
    serde_json::from_str(text).unwrap()
}

/// Preserve the exact request handle while other independent checkers may publish concurrently.
fn invoke(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    manager: &mut Manager,
    id: &str,
    operation: api::EditorOperation,
) -> Result<api::EditorValue, api::Failure> {
    let api::Value::Accepted(handle) = begin(
        manager,
        id,
        api::Operation::Editor {
            operation,
            timeout_ms: 120_000,
        },
    )?
    else {
        panic!("Accepted request expected")
    };
    let request = pump(visual, app, manager)
        .into_iter()
        .find(|request| request.handle() == &handle)
        .unwrap();
    let api::RequestUpdate::Completed { result } = request.status() else {
        panic!("Native request did not finish: {:?}", request.status())
    };
    result
}

/// Click actual Base hit regions, including focus transfer and deferred callbacks.
fn click(visual: &mut VisualTestContext, selector: &'static str) {
    let bounds = visual
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("Missing native control {selector}"));
    visual.simulate_click(bounds.center(), Default::default());
    draw(visual);
}

/// Row selection derives from the painted accessible name, never a duplicated diagnostic list.
fn select_problem(visual: &mut VisualTestContext, app: &Entity<EditorApp>, label: &str) {
    visual.update(|window, cx| app.update(cx, |app, cx| app.show_problems(window, cx)));
    draw(visual);
    let selector = visual
        .update(|window, _| {
            gpui_base::test_support::snapshots(window)
                .iter()
                .find_map(|node| {
                    if node.role() != Some(gpui_kit::Role::Button)
                        || !node.label().is_some_and(|name| name.contains(label))
                    {
                        return None;
                    }
                    node.path().iter().find_map(|part| match part {
                        gpui_kit::ElementId::Name(name)
                            if name.as_str().starts_with("native-problem-") =>
                        {
                            Some(name.to_string())
                        }
                        _ => None,
                    })
                })
        })
        .unwrap_or_else(|| panic!("No painted problem named {label}"));
    // GPUI's debug selector is static; this bounded test retains only the selected painted row ID.
    click(visual, Box::leak(selector.into_boxed_str()));
}

/// Borrow the single current native session, retaining no alternative Rope or undo state.
fn text(visual: &mut VisualTestContext, app: &Entity<EditorApp>) -> String {
    visual.update(|_, cx| app.read(cx).editor.read(cx).text().to_string())
}

/// A user click produces one normal preview before any text or disk mutation.
fn fix_preview(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    manager: &mut Manager,
    label: &str,
) -> EditorRequest {
    select_problem(visual, app, label);
    click(visual, "problem-fix-0");
    let request = pump(visual, app, manager)
        .into_iter()
        .find(|request| {
            matches!(
                request.operation(),
                api::EditorOperation::Workspace {
                    operation: workspace::Operation::ApplyPlan { .. }
                }
            )
        })
        .unwrap();
    assert!(
        visual.debug_bounds("workspace-plan-confirm").is_some(),
        "A diagnostic fix must await native consent"
    );
    request
}

/// Actual plan completion supplies its owned recovery handle for explicit group Undo.
fn receipt(request: &EditorRequest) -> workspace::Receipt {
    let api::RequestUpdate::Completed {
        result: Ok(api::EditorValue::Workspace(workspace::Value::Batch(receipt))),
    } = request.status()
    else {
        panic!("Actual batch receipt expected: {:?}", request.status())
    };
    receipt
}

/// An admitted plan completes with its real batch receipt even when consent or a precondition fails.
/// Failures before admission remain typed request errors; neither form may report completed effects.
fn rejected(request: &EditorRequest) -> api::Failure {
    match request.status() {
        api::RequestUpdate::Completed { result: Err(error) } => error,
        api::RequestUpdate::Completed {
            result: Ok(api::EditorValue::Workspace(workspace::Value::Batch(receipt))),
        } => {
            assert!(
                receipt.completed.is_empty(),
                "Rejected plan applied an operation"
            );
            assert!(
                receipt.affected.is_empty(),
                "Rejected plan changed a disk target"
            );
            assert!(
                !receipt.recoverable,
                "A rejected proposal has no recovery effects"
            );
            receipt
                .failed
                .expect("A rejected receipt must explain its failure")
        }
        status => panic!("Expected a terminal rejection: {status:?}"),
    }
}

mod declarative;
mod diagnostics_native;
mod language_native;
mod optional;
