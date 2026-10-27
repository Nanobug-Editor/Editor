//! A native combined acceptance flow builds exact Cargo artifacts and manages two real pauses.
use super::*;
use gpui_kit::VisualContext as _;
#[path = "../../../../plugin-runtime/tests/support/debugger_packages.rs"]
mod packages;

/// Native controls must route to one selected configuration and preserve unrelated target lifetimes.
#[gpui::test]
#[ignore = "build the unified rust package and verify the pinned CodeLLDB dependency first; no terminal package"]
fn cargo_artifacts_debug_in_parallel_and_retire_through_native_management(cx: &mut TestAppContext) {
    let root = tempfile::tempdir().unwrap();
    rust_project(root.path(), "parallel-debug");
    let source = root.path().join("src/main.rs");
    std::fs::write(&source, PROBE).unwrap();
    let (mut manager, app, cx) = editor_for(cx, root.path());
    let debugger = packages::rust_package("rust");
    manager
        .install(&debugger, debugger.manifest.permissions.clone())
        .unwrap();
    let mut driver = Driver::default();
    driver.frame(&mut manager, &app, cx);
    // Current configurations use their native provider forms. The historical discovery window's
    // opaque bindings do not describe these validated form values; Cargo discovery has separate
    // public-manager regressions and must not supply the retired configuration grammar here.
    let a = configuration(
        &mut manager,
        &app,
        cx,
        "run-template-rust-debug",
        "Cargo debug A",
        None,
    );
    let b = configuration(
        &mut manager,
        &app,
        cx,
        "run-template-rust-debug",
        "Cargo debug B",
        None,
    );
    // Prepare the independent Shell through its owned dialog before entering the two inspections.
    let c = configuration(
        &mut manager,
        &app,
        cx,
        "run-template-$nanobug.shell-PowerShell",
        "Unrelated ordinary run",
        Some("Start-Sleep -Seconds 60"),
    );
    cx.update(|_, cx| {
        let set = app.read(cx).run_controls.configuration_set();
        let data = &set.plugin_configurations[&c];
        assert!(
            matches!(data.validation, editor_core::ConfigurationValidation::Valid),
            "ordinary Shell setup must be valid: {data:?}"
        );
    });
    cx.update(|_, cx| {
        app.update(cx, |state, _| {
            let key = state.workspace_key();
            for id in [&a, &b] {
                let mut config = state.run_controls.configuration(id).unwrap().clone();
                config
                    .breakpoints
                    .insert(source.to_str().unwrap(), 12)
                    .unwrap();
                state.run_controls.upsert(config, &key).unwrap();
            }
            state.run_controls.select(&a, &key);
        })
    });
    click(cx, "run-debug");
    driver.wait(&mut manager, &app, cx, |state| {
        matches!(
            state.run_controls.debug_state(),
            editor_core::DebugSessionState::Paused { line: 12, .. }
        )
    });
    assert!(
        driver.launches.is_empty(),
        "Cargo preparation must launch only the debugger-owned final target"
    );
    assert!(root.path().join("target/debug/parallel-debug.exe").exists());
    let first_session =
        cx.update(|_, cx| app.read(cx).run_controls.debug_provider_session().unwrap());
    // Each current debugger session owns its adapter bridge and target PTY. Cargo metadata/build
    // handles have already exited; neither may be mistaken for an additional live debug session.
    assert_eq!(manager.live["rust"].process_count(), 2);
    // Starting B then selecting A while B connects must not steal the inspected location on B's stop.
    cx.update(|_, cx| {
        app.update(cx, |state, cx| {
            let key = state.workspace_key();
            state.run_controls.select(&b, &key);
            cx.notify();
        })
    });
    click(cx, "run-debug");
    driver.wait(&mut manager, &app, cx, |state| {
        state.run_controls.debug_session_of(&b).is_some()
    });
    cx.update(|_, cx| {
        app.update(cx, |state, cx| {
            // Inspection now follows the active native Run Tab. Use its ordinary locate path;
            // changing only the historical panel selection is overwritten by tab synchronization.
            state.terminal.update(cx, |panel, cx| {
                assert!(panel.locate_task(&a, cx));
            });
            assert!(state.run_controls.select_debug_session(&a));
            cx.notify();
        })
    });
    let inspected = cx.update(|_, cx| app.read(cx).active_path.clone());
    driver.wait(&mut manager, &app, cx, |state| {
        state
            .run_controls
            .debug_session_of(&b)
            .is_some_and(|session| {
                matches!(
                    session.state(),
                    editor_core::DebugSessionState::Paused { line: 12, .. }
                )
            })
    });
    assert_eq!(manager.live["rust"].process_count(), 4);
    assert_eq!(
        cx.update(|_, cx| app
            .read(cx)
            .run_controls
            .debug_session()
            .unwrap()
            .0
            .to_owned()),
        a
    );
    assert_eq!(
        cx.update(|_, cx| app.read(cx).active_path.clone()),
        inspected
    );
    assert!(cx.update(|_, cx| app.read(cx).run_controls.debug_panel_rows().another_paused));
    let second_session = cx.update(|_, cx| {
        app.read(cx)
            .run_controls
            .debug_session_of(&b)
            .unwrap()
            .provider_session()
            .unwrap()
            .to_owned()
    });
    assert_ne!(first_session, second_session);
    // Into/Over/Out travel the actual native panel keyboard path to real CodeLLDB/PDB pauses.
    // A source line can contain more than one instruction. Each keyboard step must produce
    // a fresh real pause; keep stepping until the call enters calculate rather than guessing.
    for _ in 0..6 {
        let epoch = cx.update(|_, cx| app.read(cx).run_controls.debug_pause_epoch().unwrap());
        click(cx, "debug-panel-state");
        cx.simulate_keystrokes("f11");
        assert!(
            cx.update(|_, cx| app.read(cx).run_controls.debug_action_pending()),
            "the native F11 key must reach the selected debug target; bounds={:?}; status={}",
            cx.debug_bounds("debug-panel-state"),
            cx.update(|_, cx| app.read(cx).status.clone())
        );
        driver.wait(&mut manager, &app, cx, |state| {
            state
                .run_controls
                .debug_pause_epoch()
                .is_some_and(|new| new > epoch)
                && !state.run_controls.debug_action_pending()
                && !state.run_controls.debug_frames().is_empty()
        });
        if cx.update(|_, cx| {
            app.read(cx)
                .run_controls
                .debug_frames()
                .iter()
                .any(|frame| frame.name.contains("calculate"))
        }) {
            break;
        }
    }
    driver.wait(&mut manager, &app, cx, |state| {
        state
            .run_controls
            .debug_frames()
            .iter()
            .any(|frame| frame.name.contains("calculate"))
            && state.run_controls.selected_debug_frame().is_some_and(|id| {
                state
                    .run_controls
                    .debug_variables(id)
                    .iter()
                    .any(|value| value.name == "value" && value.value == "5")
            })
    });
    assert!(
        manager
            .debug_observations()
            .iter()
            .any(|state| state.session == second_session && state.line == Some(12))
    );
    let frames = cx.update(|_, cx| app.read(cx).run_controls.debug_frames().to_vec());
    assert!(frames.len() > 20);
    let selector = Box::leak(format!("debug-panel-frame-{}", frames[0].id).into_boxed_str());
    let before = cx.debug_bounds(&*selector).unwrap().origin.y;
    let pane = cx.debug_bounds("debug-panel-frames").unwrap();
    // The native wheel is sent at the pointer position; hover is observed before scrolling.
    cx.simulate_event(gpui::MouseMoveEvent {
        position: pane.center(),
        pressed_button: None,
        modifiers: Default::default(),
    });
    cx.run_until_parked();
    cx.simulate_event(gpui::ScrollEvent {
        // Wheel and trackpad fixtures retain pointer-device semantics.
        source: gpui_kit::ScrollSource::Pointer,
        position: pane.center(),
        delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-350.))),
        modifiers: Default::default(),
        touch_phase: gpui::TouchPhase::Moved,
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(
        cx.debug_bounds(&*selector).unwrap().origin.y < before,
        "the actual wheel scrolls the stack pane"
    );
    click(cx, "debug-panel-state");
    cx.simulate_keystrokes("down");
    let frame = cx.update(|_, cx| app.read(cx).run_controls.selected_debug_frame().unwrap());
    assert_ne!(frame, frames[0].id);
    cx.simulate_keystrokes("up");
    assert_eq!(
        cx.update(|_, cx| app.read(cx).run_controls.selected_debug_frame().unwrap()),
        frames[0].id
    );
    for key in ["f10", "shift-f11"] {
        let epoch = cx.update(|_, cx| app.read(cx).run_controls.debug_pause_epoch().unwrap());
        cx.simulate_keystrokes(key);
        driver.wait(&mut manager, &app, cx, |state| {
            state
                .run_controls
                .debug_pause_epoch()
                .is_some_and(|new| new > epoch)
                && !state.run_controls.debug_action_pending()
        });
    }
    cx.simulate_keystrokes("f5");
    driver.wait(&mut manager, &app, cx, |state| {
        matches!(
            state.run_controls.debug_state(),
            editor_core::DebugSessionState::Running
        )
    });
    assert!(
        manager
            .debug_observations()
            .iter()
            .any(|state| state.session == second_session
                && state.state == plugin_runtime::DebugState::Paused)
    );
    cx.simulate_keystrokes("f6");
    driver.wait(&mut manager, &app, cx, |state| {
        matches!(
            state.run_controls.debug_state(),
            editor_core::DebugSessionState::Paused { .. }
        )
    });
    // Scaling/theme/locale use the same settings contracts, and retain the two live target owners.
    let previous_locale = LocaleRestore(rust_i18n::locale().to_string());
    for (dark, locale, font) in [(true, "en", 24.), (false, "zh-CN", 14.)] {
        rust_i18n::set_locale(locale);
        cx.update(|window, cx| {
            apply_theme(builtin_theme(dark), cx);
            typography::set_font_size(cx, font);
            crate::ui::theme::sync_font_sizes(cx);
            window.refresh();
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("debug-panel-step-into").is_some());
        assert_eq!(manager.live["rust"].process_count(), 4);
    }
    drop(previous_locale);
    // An unrelated ordinary terminal run must survive the debug provider's confirmed retirement.
    cx.update(|_, cx| {
        app.update(cx, |state, _| {
            let key = state.workspace_key();
            state.run_controls.select(&c, &key);
        })
    });
    click(cx, "run-start");
    driver.wait(&mut manager, &app, cx, |state| {
        state.run_controls.running_for(&c).is_some()
    });
    let unrelated = driver.launches.last().unwrap().0;
    // The market must explain these genuine provider-owned sessions before replacing their owner.
    market_replacement_consent(&manager, &app, cx, root.path());
    driver.frame(&mut manager, &app, cx);
    assert_eq!(manager.live["rust"].process_count(), 4);
    let open = |app: &Entity<EditorApp>, cx: &mut gpui_kit::VisualTestContext| {
        cx.update(|window, cx| {
            let impact = app
                .read(cx)
                .run_controls
                .plugin_session_impact("rust", None)
                .summary()
                .unwrap();
            assert!(
                impact.contains("Cargo debug A") && impact.contains("Cargo debug B"),
                "{impact}"
            );
            app.read(cx).extensions.clone().update(cx, |owner, cx| {
                owner.open_remove_dialog("rust".into(), false, Some(impact), window, cx)
            });
        })
    };
    open(&app, cx);
    cx.run_until_parked();
    assert!(cx.debug_bounds("plugin-remove-consent").is_some());
    click(cx, "plugin-remove-cancel");
    driver.frame(&mut manager, &app, cx);
    assert_eq!(manager.live["rust"].process_count(), 4);
    open(&app, cx);
    cx.run_until_parked();
    click(cx, "plugin-remove-preserve");
    driver.frame(&mut manager, &app, cx);
    assert!(!manager.live.contains_key("rust"));
    // Source revocation queues native retirement; only its observed exit receipt is a terminal state.
    driver.wait(&mut manager, &app, cx, |state| {
        [&a, &b].into_iter().all(|id| {
            state
                .run_controls
                .debug_session_of(id)
                .is_some_and(|session| {
                    matches!(
                        session.state(),
                        editor_core::DebugSessionState::Failed { .. }
                    )
                })
        })
    });
    assert!(
        manager
            .debug_observations()
            .iter()
            .filter(|state| [&first_session, &second_session].contains(&&state.session))
            .all(|state| state.state == plugin_runtime::DebugState::Failed)
    );
    assert_eq!(
        manager.execution(unrelated).unwrap().snapshot().state,
        plugin_runtime::ExecutionState::Running
    );
    assert!(cx.update(|_, cx| {
        matches!(
            app.read(cx)
                .run_controls
                .debug_session_of(&a)
                .unwrap()
                .state(),
            editor_core::DebugSessionState::Failed { .. }
        )
    }));
    assert_eq!(
        driver.launches.len(),
        1,
        "retirement never replays a debug or ordinary target"
    );
    manager.shutdown();
}

/// Create current plugin-owned values with visible form input and the normal Save receipt.
fn configuration(
    manager: &mut Manager,
    app: &Entity<EditorApp>,
    cx: &mut gpui_kit::VisualTestContext,
    template: &'static str,
    name: &str,
    script: Option<&str>,
) -> String {
    use super::super::native_configuration_tests::{Driver as FormDriver, click, edit, open_form};
    let mut driver = FormDriver::default();
    let parent = open_form(app, cx);
    click(cx, "run-config-add");
    driver.wait(manager, app, cx, |cx| cx.debug_bounds(template).is_some());
    click(cx, template);
    driver.wait(manager, app, cx, |cx| {
        cx.debug_bounds("plugin-ui-name").is_some()
    });
    edit(cx, "plugin-ui-name", name);
    driver.frame(manager, app, cx);
    driver.frame(manager, app, cx);
    if let Some(script) = script {
        // Wheel input must expose the actual multiline field before its native hit target is used.
        for _ in 0..30 {
            cx.update(|window, cx| window.draw(cx).clear(cx));
            let viewport = cx.debug_bounds("plugin-ui-form-scroll").unwrap();
            let field = cx.debug_bounds("plugin-ui-script").unwrap();
            if viewport.contains(&field.center()) {
                break;
            }
            cx.simulate_event(gpui::ScrollEvent {
                // Wheel and trackpad fixtures retain pointer-device semantics.
                source: gpui_kit::ScrollSource::Pointer,
                position: viewport.center(),
                delta: gpui::ScrollDelta::Lines(point(0., -3.)),
                ..Default::default()
            });
            cx.run_until_parked();
        }
        edit(cx, "plugin-ui-script", script);
        driver.frame(manager, app, cx);
    }
    let dialog = cx.window_handle();
    *cx = gpui_kit::VisualTestContext::from_window(parent, &cx.cx);
    // Paint and click the normal Save control once it accepts input; busy controls reject the click.
    driver.wait(manager, app, cx, |cx| {
        if cx.update(|_, cx| app.read(cx).run_form.is_none()) {
            return true;
        }
        let mut form = gpui_kit::VisualTestContext::from_window(dialog, &cx.cx);
        click(&mut form, "run-config-save");
        false
    });
    cx.update(|_, cx| {
        app.read(cx)
            .run_controls
            .configurations()
            .iter()
            .find(|config| config.name == name)
            .unwrap()
            .id
            .clone()
    })
}

/// Exercise the production market button while two genuine provider-owned pauses are still active.
fn market_replacement_consent(
    manager: &Manager,
    app: &Entity<EditorApp>,
    cx: &mut gpui_kit::VisualTestContext,
    root: &Path,
) {
    use std::{
        io::{Cursor, Write},
        sync::atomic::Ordering,
    };
    let original = packages::debugger("combined-debug-provider");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&original.files["manifest.json"]).unwrap();
    manifest["version"] = "0.6.2".into();
    manifest["replaces"] = serde_json::json!([{"id":"rust", "versions":"^0.6", "contracts":["debug.session"], "enabled_setting":"debugger.enabled"}]);
    let mut files = original.files;
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    // This controlled candidate has the actual guest and package digest; cancellation must prevent transfer.
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in files {
        archive
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        archive.write_all(&bytes).unwrap();
    }
    let bytes = archive.finish().unwrap().into_inner();
    let package = Package::from_bytes(&bytes).unwrap();
    let mut catalog: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../../plugin-runtime/tests/fixtures/marketplace/catalog.json"
    ))
    .unwrap();
    let listing = &mut catalog["plugins"][0];
    listing["id"] = package.manifest.id.clone().into();
    listing["summary"] = "Controlled replacement consent candidate".into();
    let version = &mut listing["versions"][0];
    version["version"] = "0.6.2".into();
    version["manifest"] = serde_json::to_value(&package.manifest).unwrap();
    version["sha256"] = package.digest.clone().into();
    version["readme"] = "Controlled native consent regression".into();
    let server = super::super::market_test_server::Server::from_bytes(
        &serde_json::to_vec(&catalog).unwrap(),
        bytes,
    );
    let panel = cx.update(|_, cx| app.read(cx).extensions.clone());
    let worker = cx.update(|_, cx| panel.read(cx).worker.clone());
    worker.refresh_market(
        root.join("market-consent-cache.json"),
        format!("{}/catalog.json", server.url),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while !worker.state.lock().unwrap().market.fresh() {
        assert!(
            Instant::now() < deadline,
            "controlled catalog refresh timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let before = cx.update(|_, cx| cx.windows());
    cx.update(|window, cx| {
        let impact = app
            .read(cx)
            .run_controls
            .plugin_session_impact("rust", None)
            .summary()
            .unwrap();
        assert!(impact.contains("Cargo debug A") && impact.contains("Cargo debug B"));
        panel.update(cx, |panel, cx| {
            panel.manager_market = true;
            panel.poll(cx);
        });
        app.update(cx, |app, cx| app.toggle_extensions(window, cx));
    });
    let handle = cx
        .update(|_, cx| cx.windows())
        .into_iter()
        .find(|window| !before.contains(window))
        .unwrap();
    let form = gpui_kit::VisualTestContext::from_window(handle, cx).into_mut();
    form.run_until_parked();
    form.update(|window, cx| window.draw(cx).clear(cx));
    click(form, "market-install");
    assert!(form.debug_bounds("market-consent-replacements").is_some());
    assert!(
        form.debug_bounds("market-consent-session-impact").is_some(),
        "the market confirmation must show active session impact"
    );
    click(form, "market-consent-cancel");
    assert_eq!(server.downloads.load(Ordering::Acquire), 0);
    assert!(manager.installed["rust"].replaced_by.is_none());
    form.update(|window, _| window.remove_window());
}

/// Failed assertions must not leak the selected locale into another native UI test.
struct LocaleRestore(String);
impl Drop for LocaleRestore {
    fn drop(&mut self) {
        rust_i18n::set_locale(&self.0);
    }
}

/// Recursion supplies a real long stack for wheel verification, with stable unoptimized source lines.
const PROBE: &str = "//! Cargo/PDB parallel acceptance target.\n#[inline(never)]\nfn calculate(value:i32)->i32 {\n    let doubled=value*2;\n    doubled+1\n}\n#[inline(never)]\nfn descend(depth:u32)->i32 {\n    if depth>0 {\n        let value=descend(depth-1);std::hint::black_box(value)\n    } else {\n        calculate(5)\n    }\n}\nfn main() {\n    let result=descend(24);\n    println!(\"RUST_RESULT:{result}\");\n    for _ in 0..6000 {std::thread::sleep(std::time::Duration::from_millis(10));}\n}\n";
