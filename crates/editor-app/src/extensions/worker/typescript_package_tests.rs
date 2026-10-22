//! Installed TypeScript packages serve native configured/inferred documents independently of Vue.
use super::vue_test_support::{input, setup_for_workspace, shutdown, wait};
use super::*;
use crate::{EditorApp, Entity};
use gpui_kit::{TestAppContext, VisualTestContext, gpui};
use plugin_runtime::plugin_protocol::{direct, language::SourceSnapshot};

/// Seven real language operations consume EditorState text on the published native connection.
/// No fixture language server or implementation import can stand in for the installed tool archive.
#[gpui::test]
#[ignore = "set NANOBUG_TYPESCRIPT_PACKAGE and NANOBUG_VUE_PACKAGE to actual release candidates"]
fn typescript_package_serves_configured_and_inferred_sources_after_vue_disabled(
    cx: &mut TestAppContext,
) {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::create_dir(workspace.path().join("configured")).unwrap();
    std::fs::create_dir(workspace.path().join("loose")).unwrap();
    std::fs::write(
        workspace.path().join("configured/tsconfig.json"),
        r#"{"compilerOptions":{"strict":true,"skipLibCheck":true},"include":["*.ts"]}"#,
    )
    .unwrap();
    let disk = "const saved = true;\n";
    for name in ["configured/main.ts", "loose/main.js"] {
        std::fs::write(workspace.path().join(name), disk).unwrap();
    }
    let (fixture, visual) = setup_for_workspace(
        cx,
        &["NANOBUG_TYPESCRIPT_PACKAGE", "NANOBUG_VUE_PACKAGE"],
        workspace.path().to_owned(),
    );
    let app = fixture.app.clone();
    let actor = fixture.actor.clone();
    let vue = Package::read(&input("NANOBUG_VUE_PACKAGE"))
        .unwrap()
        .manifest
        .id;
    actor.tx.send(Work::Disable(vue.clone())).unwrap();
    wait(
        &app,
        &actor,
        visual,
        "Vue disabled without removing TypeScript",
        30,
        |_, _| {
            actor
                .state
                .lock()
                .unwrap()
                .entries
                .iter()
                .any(|entry| entry.manifest.id == vue && !entry.enabled)
        },
    );
    let source = "// unsaved 中文😀\nconst value=1;\nvalue.toUpperCase();\nconst answer=value+2;\n";
    let paths = ["configured/main.ts", "loose/main.js"]
        .map(|name| fixture.workspace.join(name).canonicalize().unwrap());
    for path in &paths {
        wait(
            &app,
            &actor,
            visual,
            "TypeScript native recognition",
            30,
            |app, _| {
                app.providers
                    .language_servers()
                    .get(&app.language_for_path(path))
                    .is_some_and(Option::is_some)
            },
        );
        visual.update(|window, cx| {
            window.activate_window();
            app.update(cx, |app, cx| app.open_file(path.clone(), window, cx));
            window.draw(cx).clear(cx);
        });
        visual.simulate_keystrokes("ctrl-a");
        visual.simulate_input(source);
        wait(
            &app,
            &actor,
            visual,
            "TypeScript package native providers",
            180,
            |app, cx| {
                for state in app.language_service_states.values() {
                    if let crate::app::language_servers::ServiceLoadState::Failed(error) = state {
                        panic!("TypeScript package preparation failed: {error}");
                    }
                }
                app.language_server_for_path(path)
                    .is_some_and(|server| server.is_active())
                    && app.editor.read(cx).lsp().completion_provider.is_some()
                    && app.editor.read(cx).lsp().hover_provider.is_some()
            },
        );
        let (server, document, version, captured) = visual.update(|_, cx| {
            let app = app.read(cx);
            let index = app.tabs.iter().position(|tab| tab.path() == *path).unwrap();
            let server = app.language_server_for_path(path).unwrap().clone();
            let uri = crate::language::navigation::file_uri(path).unwrap();
            let document = server.document(&uri).unwrap();
            let captured = app.editor.read(cx).text().to_string();
            assert_eq!(captured, source);
            (
                server,
                document,
                app.plugin_document_version(index).unwrap(),
                captured,
            )
        });
        let (send, receive) = std::sync::mpsc::channel();
        let root = fixture.workspace.clone();
        let expected = path.clone();
        std::thread::spawn(move || {
            // The background task receives one immutable native snapshot and its original lease.
            let result = (|| -> anyhow::Result<()> {
                let hover = server.hover(
                    document.uri.clone(),
                    captured.clone(),
                    lsp_types::Position::new(3, 15),
                )?;
                anyhow::ensure!(hover.is_some(), "hover must expose the inferred number");
                let completion = server.completions_at_version(
                    document.clone(),
                    captured.clone(),
                    lsp_types::Position::new(2, 6),
                    version.clone(),
                )?;
                let items = match completion {
                    lsp_types::CompletionResponse::Array(items) => items,
                    lsp_types::CompletionResponse::List(list) => list.items,
                };
                anyhow::ensure!(
                    items.iter().any(|item| item.label == "toFixed"),
                    "number member completion is missing"
                );
                let definitions = server.definitions(
                    document.uri.clone(),
                    captured.clone(),
                    lsp_types::Position::new(3, 15),
                )?;
                anyhow::ensure!(
                    definitions.len() == 1 && definitions[0].target_selection_range.start.line == 1,
                    "definition must refer to the unsaved declaration"
                );
                let target = url::Url::parse(definitions[0].target_uri.as_str())?
                    .to_file_path()
                    .map_err(|_| anyhow::anyhow!("non-file definition"))?;
                anyhow::ensure!(
                    target.canonicalize()? == expected,
                    "definition escaped original source"
                );
                let references = server.feature_for(
                    document.clone(),
                    SourceSnapshot {
                        document: version,
                        text: captured.clone(),
                    },
                    captured.rfind("value").unwrap() + 1,
                    direct::Feature::References,
                    &plugin_runtime::RequestCancellation::default(),
                    &root,
                )?;
                let direct::Result::References { locations } = references else {
                    anyhow::bail!("expected references");
                };
                anyhow::ensure!(
                    locations.len() == 3,
                    "all three native source references must remain visible"
                );
                let rename = server.rename_for(
                    document.clone(),
                    captured.clone(),
                    lsp_types::Position::new(3, 15),
                    "renamed".into(),
                )?;
                anyhow::ensure!(
                    rename.len() == 3 && rename.iter().all(|edit| edit.new_text == "renamed"),
                    "rename must preserve the complete edit set"
                );
                let format = server.format_for(document, captured, None, 2, true)?;
                anyhow::ensure!(
                    !format.is_empty(),
                    "formatting must propose the missing spaces"
                );
                Ok(())
            })();
            send.send(result.map_err(|error| format!("{error:#}")))
                .unwrap();
        });
        let result = std::cell::RefCell::new(None);
        wait(
            &app,
            &actor,
            visual,
            "six actual package language responses",
            30,
            |_, _| {
                if let Ok(value) = receive.try_recv() {
                    *result.borrow_mut() = Some(value);
                }
                result.borrow().is_some()
            },
        );
        result.into_inner().unwrap().unwrap();
        wait(
            &app,
            &actor,
            visual,
            "native semantic diagnostic from unsaved text",
            30,
            |app, cx| {
                app.editor.read(cx).diagnostics().is_some_and(|items| {
                    items
                        .iter()
                        .any(|item| item.message.contains("toUpperCase"))
                })
            },
        );
        assert_eq!(
            visual.update(|_, cx| app.read(cx).editor.read(cx).text().to_string()),
            source
        );
        assert_eq!(std::fs::read_to_string(path).unwrap(), disk);
    }
    let plans = actor
        .state
        .lock()
        .unwrap()
        .language_services
        .values()
        .filter_map(|entry| entry.as_ref().ok().cloned())
        .collect::<Vec<_>>();
    // Public facades and their internal engines may publish aliases of the same plan. Inspect
    // each document's actual selected connection instead of treating aliases as extra contexts.
    let roots = visual.update(|_, cx| {
        paths
            .iter()
            .map(|path| {
                app.read(cx)
                    .language_server_for_path(path)
                    .unwrap()
                    .navigation_context_service()
                    .unwrap()
                    .root
                    .clone()
            })
            .collect::<std::collections::BTreeSet<_>>()
    });
    assert_eq!(
        roots.len(),
        2,
        "configured and inferred sources must use distinct native roots"
    );
    assert!(
        plans
            .iter()
            .all(|plan| plan.owner == "typescript" && plan.is_active())
    );
    shutdown(&fixture, visual);
    assert!(
        plans
            .iter()
            .all(|plan| !plan.is_active() && plan.process_count() == 0),
        "normal shutdown must release every managed native process"
    );
}

/// This unknown WASM owns ordinary public service references, independently of the TypeScript guest.
/// Five asynchronous calls prove context continuity and release against the same native EditorState.
#[gpui::test]
#[ignore = "set NANOBUG_TYPESCRIPT_PACKAGE and NANOBUG_PROJECT_CONSUMER_PACKAGE to independent release packages"]
fn unknown_project_consumer_uses_actual_typescript_package_contracts(cx: &mut TestAppContext) {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::write(
        workspace.path().join("tsconfig.json"),
        r#"{"compilerOptions":{"strict":true},"include":["*.ts"]}"#,
    )
    .unwrap();
    let disk = "const saved = 0;";
    std::fs::write(workspace.path().join("main.ts"), disk).unwrap();
    let (fixture, visual) = setup_for_workspace(
        cx,
        &[
            "NANOBUG_TYPESCRIPT_PACKAGE",
            "NANOBUG_PROJECT_CONSUMER_PACKAGE",
        ],
        workspace.path().to_owned(),
    );
    let app = fixture.app.clone();
    let actor = fixture.actor.clone();
    let path = fixture.workspace.join("main.ts").canonicalize().unwrap();
    let source = "// native 中文😀\nconst value = 42;\nvalue;\n";
    wait(
        &app,
        &actor,
        visual,
        "unknown consumer package registry",
        30,
        |app, _| {
            app.providers
                .language_servers()
                .get(&app.language_for_path(&path))
                .is_some_and(Option::is_some)
        },
    );
    visual.update(|window, cx| {
        window.activate_window();
        app.update(cx, |app, cx| app.open_file(path.clone(), window, cx));
        window.draw(cx).clear(cx);
    });
    visual.simulate_keystrokes("ctrl-a");
    visual.simulate_input(source);
    wait(
        &app,
        &actor,
        visual,
        "native document available to public consumer",
        180,
        |app, cx| {
            app.language_server_for_path(&path)
                .is_some_and(|server| server.is_active())
                && app.editor.read(cx).lsp().hover_provider.is_some()
        },
    );
    // Compare the public receipt with the actual native incarnation and revision captured before
    // invocation. A path-only assertion would also accept a stale reopening or a guessed watermark.
    let native_version = visual.update(|_, cx| {
        let app = app.read(cx);
        let index = app.tabs.iter().position(|tab| tab.path() == path).unwrap();
        app.plugin_document_version(index).unwrap()
    });
    invoke_unknown_project_consumer(&actor);
    let result = std::cell::RefCell::new(None);
    wait(
        &app,
        &actor,
        visual,
        "unknown SDK resolve describe analyze release sequence",
        180,
        |_, _| {
            let state = actor.state.lock().unwrap();
            let Some(view) = state.views.get("unknown-project-consumer/receipt") else {
                return false;
            };
            let plugin_runtime::plugin_protocol::ui::Kind::Text { text } = &view.root.kind else {
                panic!("expected typed consumer receipt");
            };
            let value: serde_json::Value = serde_json::from_str(text).unwrap();
            assert_ne!(
                value["state"], "failed",
                "unknown public consumer failed: {value}"
            );
            if value["state"] == "complete" {
                *result.borrow_mut() = Some(value);
                true
            } else {
                false
            }
        },
    );
    let receipt = result.into_inner().unwrap();
    let observed = receipt["observations"].as_array().unwrap();
    assert_eq!(observed.len(), 4);
    assert_eq!(
        observed[0], observed[1],
        "describe must retain resolve's exact opaque context"
    );
    assert_eq!(observed[0]["engineVersion"], "5.9.3");
    assert_eq!(observed[0]["kind"], "configured");
    assert_eq!(observed[0]["features"].as_array().unwrap().len(), 7);
    assert!(
        observed[2]["result"]["displayString"]
            .as_str()
            .unwrap()
            .contains("value: 42"),
        "analysis must use unsaved native text rather than saved disk input: {receipt}"
    );
    let versions: Vec<plugin_runtime::plugin_protocol::api::DocumentVersion> =
        serde_json::from_value(observed[2]["versions"].clone()).unwrap();
    assert_eq!(
        versions,
        vec![native_version],
        "public analysis must seal the original native document identity and revision"
    );
    assert!(observed[3].is_null());
    assert_eq!(receipt["afterRelease"]["code"], "stale_revision");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), disk);
    assert_eq!(
        visual.update(|_, cx| app.read(cx).editor.read(cx).text().to_string()),
        source
    );
    let plans = actor
        .state
        .lock()
        .unwrap()
        .language_services
        .values()
        .filter_map(|entry| entry.as_ref().ok().cloned())
        .collect::<Vec<_>>();
    shutdown(&fixture, visual);
    assert!(
        plans
            .iter()
            .all(|plan| !plan.is_active() && plan.process_count() == 0)
    );
}

/// A required public contract is rejected during ordinary activation, before the unknown consumer
/// can execute. Missing TypeScript must not create a substitute engine or a guessed project context.
#[test]
#[ignore = "set NANOBUG_PROJECT_CONSUMER_PACKAGE to the independently built SDK consumer"]
fn unknown_project_consumer_without_typescript_has_a_structured_unavailable_error() {
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let package = Package::read(&input("NANOBUG_PROJECT_CONSUMER_PACKAGE")).unwrap();
    let mut manager = Manager::open(
        profile.path().join("runtime-plugins"),
        plugin_runtime::plugin_protocol::Environment {
            workspace: workspace.path().display().to_string(),
            ..Default::default()
        },
    )
    .unwrap();
    manager
        .install(&package, package.manifest.permissions.clone())
        .unwrap();
    let failure = crate::extensions::demand_plugin_fixture(
        &mut manager,
        "unknown-project-consumer",
        plugin_runtime::DemandReason::Command("run".into()),
    )
    .err()
    .expect("a required TypeScript service cannot activate without its provider");
    assert_eq!(failure.code, api::ErrorCode::CapabilityUnavailable);
    assert!(
        failure.message.contains("Service typescript.")
            && failure.message.contains("0 compatible providers"),
        "the public failure must identify the absent contract: {failure:?}"
    );
    assert!(manager.instance_id("unknown-project-consumer").is_none());
    assert!(manager.language_services().is_empty());
    manager.shutdown();
}

/// Explicit project SDK settings flow through the actual consumer, facade and native initialization.
/// Every invalid selection must remain visible as a typed failure, even though the package includes
/// a working bundled engine that would otherwise make a silent fallback appear successful.
#[gpui::test]
#[ignore = "set NANOBUG_TYPESCRIPT_PACKAGE and NANOBUG_PROJECT_CONSUMER_PACKAGE to independent release packages"]
fn unknown_project_consumer_rejects_missing_wrong_and_incompatible_explicit_sdks(
    cx: &mut TestAppContext,
) {
    for (case, identity) in [
        ("missing", None),
        ("wrong-package", Some(("other-engine", "5.9.3"))),
        ("incompatible", Some(("typescript", "0.0.1"))),
    ] {
        let workspace = tempfile::tempdir().unwrap();
        std::fs::write(workspace.path().join("tsconfig.json"), "{}").unwrap();
        let disk = "const value = 42;\nvalue;\n";
        std::fs::write(workspace.path().join("main.ts"), disk).unwrap();
        let sdk = workspace.path().join("explicit-sdk/lib");
        if let Some((name, version)) = identity {
            std::fs::create_dir_all(&sdk).unwrap();
            std::fs::write(
                sdk.parent().unwrap().join("package.json"),
                serde_json::json!({"name":name,"version":version}).to_string(),
            )
            .unwrap();
            // The selected stub is deliberately inert; validation must reject it before execution.
            std::fs::write(
                sdk.join("tsserver.js"),
                "throw new Error('unverified SDK ran');",
            )
            .unwrap();
        }
        let (fixture, visual) = setup_for_workspace(
            cx,
            &[
                "NANOBUG_TYPESCRIPT_PACKAGE",
                "NANOBUG_PROJECT_CONSUMER_PACKAGE",
            ],
            workspace.path().to_owned(),
        );
        let app = fixture.app.clone();
        let actor = fixture.actor.clone();
        wait(
            &app,
            &actor,
            visual,
            "SDK setting publication",
            30,
            |_, _| actor.state.lock().unwrap().ready,
        );
        actor
            .tx
            .send(Work::SetSetting {
                request: 1,
                plugin: "typescript".into(),
                scope: settings::Scope::Project,
                key: "typescript.sdk".into(),
                value: Some(serde_json::json!(sdk)),
            })
            .unwrap();
        wait(
            &app,
            &actor,
            visual,
            "explicit project SDK accepted",
            60,
            |_, _| {
                let state = actor.state.lock().unwrap();
                let Some((1, result)) = &state.configuration_result else {
                    return false;
                };
                assert!(result.is_ok(), "setting {case} failed: {result:?}");
                true
            },
        );
        let path = fixture.workspace.join("main.ts").canonicalize().unwrap();
        visual.update(|window, cx| {
            app.update(cx, |app, cx| app.open_file(path.clone(), window, cx));
            window.draw(cx).clear(cx);
        });
        invoke_unknown_project_consumer(&actor);
        let receipt = failed_project_consumer_receipt(&app, &actor, visual);
        assert_eq!(receipt["phase"], 0, "{case}: {receipt}");
        let failure: api::Failure = serde_json::from_value(receipt["error"].clone()).unwrap();
        assert_eq!(failure.code, api::ErrorCode::OperationFailed, "{case}");
        assert!(
            failure
                .message
                .contains("TypeScript SDK missing or incompatible"),
            "the selected SDK must fail before returning any context: {case}: {receipt}"
        );
        let plans = actor
            .state
            .lock()
            .unwrap()
            .language_services
            .values()
            .filter_map(|entry| entry.as_ref().ok().cloned())
            .collect::<Vec<_>>();
        // Context resolution can reject an invalid SDK before publishing any native lease.
        // If preparation has published a plan already, it must retain that exact explicit SDK;
        // neither scheduling order may introduce an alternative bundled engine.
        assert!(plans.len() <= 1, "{case} selected alternative engine plans");
        for plan in &plans {
            assert_eq!(plan.owner, "typescript");
            assert_eq!(
                plan.provider.initialization_options["typescript"],
                serde_json::json!(sdk),
                "the actual plan must preserve the explicit choice"
            );
        }
        assert_eq!(std::fs::read_to_string(&path).unwrap(), disk);
        assert_eq!(
            visual.update(|_, cx| app.read(cx).editor.read(cx).text().to_string()),
            disk
        );
        shutdown(&fixture, visual);
        assert!(
            plans
                .iter()
                .all(|plan| !plan.is_active() && plan.process_count() == 0)
        );
    }
}

/// Issue the independently packaged consumer's declared command using its current host epoch.
fn invoke_unknown_project_consumer(actor: &Worker) {
    let state = actor.state.lock().unwrap();
    actor
        .tx
        .send(Work::Invoke {
            plugin: "unknown-project-consumer".into(),
            command: "run".into(),
            arguments: serde_json::json!({"path":"main.ts"}),
            context: None,
            expected_epoch: state
                .command_epoch("unknown-project-consumer", "run")
                .unwrap(),
            origin: state
                .availabilities
                .get("unknown-project-consumer")
                .cloned()
                .map(|availability| super::demands::CommandOrigin {
                    availability,
                    guard: None,
                }),
        })
        .unwrap();
}

/// Read the ordinary SDK guest's typed native panel, never a test-only manager error injection.
fn failed_project_consumer_receipt(
    app: &Entity<EditorApp>,
    actor: &Worker,
    visual: &mut VisualTestContext,
) -> serde_json::Value {
    let result = std::cell::RefCell::new(None);
    wait(
        app,
        actor,
        visual,
        "public consumer SDK refusal",
        180,
        |_, _| {
            let state = actor.state.lock().unwrap();
            let Some(view) = state.views.get("unknown-project-consumer/receipt") else {
                return false;
            };
            let plugin_runtime::plugin_protocol::ui::Kind::Text { text } = &view.root.kind else {
                panic!("expected typed consumer receipt");
            };
            let value: serde_json::Value = serde_json::from_str(text).unwrap();
            assert_ne!(
                value["state"], "complete",
                "SDK silently fell back: {value}"
            );
            if value["state"] == "failed" {
                *result.borrow_mut() = Some(value);
                true
            } else {
                false
            }
        },
    );
    result.into_inner().unwrap()
}
