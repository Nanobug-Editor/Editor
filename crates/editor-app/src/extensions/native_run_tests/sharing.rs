//! Current workspace-local configuration storage, literal process inputs and fail-closed reloads.
use super::*;
use std::time::{Duration, Instant};

/// The historical sharing scenario now exercises the accepted local-only contract in both workspaces.
/// Keep literal argv/cwd/environment and pinned active execution coverage without restoring the retired selector.
#[gpui::test]
#[ignore = "requires a native rustc instrument; no terminal package is used"]
fn native_shared_configuration_runs_in_two_isolated_projects(cx: &mut TestAppContext) {
    let roots = tempfile::tempdir().unwrap();
    let first = roots.path().join("first");
    let second = roots.path().join("second");
    for root in [&first, &second] {
        std::fs::create_dir_all(root.join("personal-bin")).unwrap();
    }
    let first = first.canonicalize().unwrap();
    let second = second.canonicalize().unwrap();
    let source = first.join("probe.rs");
    std::fs::write(&source, PROBE).unwrap();
    let executable = first.join("personal-bin/share-probe.exe");
    let build = std::process::Command::new("rustc")
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    std::fs::copy(&executable, second.join("personal-bin/share-probe.exe")).unwrap();
    for (root, private_value) in [(&first, "first-private"), (&second, "second-private")] {
        let (mut manager, app, window) = fixture(
            cx,
            root,
            "share-probe.exe",
            vec!["literal space".into(), "quote\"中文;&|".into()],
        );
        let id = window.update(|_, cx| {
            app.update(cx, |state, _| {
                let key = state.workspace_key();
                let mut config = state.run_controls.selected().unwrap().clone();
                config.directory = Some(root.display().to_string());
                config
                    .env
                    .insert("LOCAL_VALUE".into(), private_value.into());
                config.tool_paths = vec![root.join("personal-bin").display().to_string()];
                let id = config.id.clone();
                super::super::test_configurations::accept(&mut state.run_controls, config);
                state.run_controls.select(&id, &key);
                id
            })
        });
        let base = root.join("private-runs");
        let key = state_key(root);
        let stored = editor_core::load(&base, &key).unwrap();
        assert!(stored.plugin_configurations.contains_key(&id));
        assert!(
            !editor_core::project_path(root).exists(),
            "current configuration must not write a shared project file"
        );
        let foreign = state_key(if root == &first { &second } else { &first });
        assert!(
            editor_core::load(&base, &foreign)
                .unwrap()
                .configurations
                .is_empty(),
            "workspace identity must prevent implicit cross-project configuration reuse"
        );
        let mut renderer = images::VectorRenderer::default();
        let mut launches = Vec::new();
        click(window, "run-start");
        wait_for_file(
            &mut manager,
            &app,
            window,
            &mut renderer,
            &mut launches,
            &root.join("observed.txt"),
        );
        let original = std::fs::read_to_string(root.join("observed.txt")).unwrap();
        assert!(
            original.contains("literal space")
                && original.contains("quote\\\"中文;&|")
                && original.ends_with(&format!("|{private_value}")),
            "{original}"
        );
        assert_eq!(
            std::path::Path::new(original.rsplit('|').nth(1).unwrap())
                .canonicalize()
                .unwrap(),
            *root
        );

        // Editing future provider values cannot retarget the active native process or create another launch.
        let mut edited = stored.clone();
        let data = edited.plugin_configurations.get_mut(&id).unwrap();
        let mut values: serde_json::Value = serde_json::from_str(&data.values).unwrap();
        values["target"]["args"] = serde_json::json!(["hand edited"]);
        data.values = values.to_string();
        editor_core::save(&base, &key, &edited).unwrap();
        click(window, "run-start");
        pump_recording(&mut manager, &app, window, &mut launches);
        publish_with_launches(&mut manager, &mut renderer, &app, window, &launches);
        assert_eq!(
            launches.len(),
            1,
            "an active configuration keeps its original target"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("observed.txt")).unwrap(),
            original
        );
        let reopened =
            crate::run::RunControls::load_plugin_configurations(&key, Some(base.clone()));
        let values: serde_json::Value =
            serde_json::from_str(&reopened.configuration_set().plugin_configurations[&id].values)
                .unwrap();
        assert_eq!(values["target"]["args"], serde_json::json!(["hand edited"]));
        assert_eq!(values["env"]["LOCAL_VALUE"], private_value);
        // The accepted local-only contract still guarantees that edits affect the next real run.
        // Stop through the native action before replacing controls; the active target stays pinned above.
        click(window, "run-stop");
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut forced = false;
        loop {
            // Native lifecycle/status timers use the dispatcher clock, independently of child wall time.
            window.executor().advance_clock(Duration::from_millis(30));
            window.run_until_parked();
            // Forward the real stop/status answers just as the worker does; dropping a receipt
            // would strand the native controller in its pending state after the child has exited.
            let (statuses, stops) = pump_recording(&mut manager, &app, window, &mut launches);
            // The production actor polls the manager between dispatch and publication so
            // asynchronous native exit receipts can complete the accepted stop request.
            manager.poll();
            super::super::composable_tests::publish_frame(
                &mut manager,
                &mut renderer,
                &app,
                window,
                &launches,
                statuses,
                stops,
            );
            if window.update(|_, cx| app.read(cx).run_controls.running_for(&id).is_none()) {
                break;
            }
            if !forced && window.debug_bounds("run-terminate").is_some() {
                // The fixture deliberately sleeps without consuming stdin; use the visible
                // explicit force action if the first graceful request cannot finish it.
                click(window, "run-terminate");
                forced = true;
            }
            assert!(
                Instant::now() < deadline,
                "the original native run must stop: sessions={:?}, pending={:?}, control={:?}",
                manager
                    .executions()
                    .iter()
                    .map(|session| (session.id(), session.state()))
                    .collect::<Vec<_>>(),
                window.update(|_, cx| app.read(cx).run_controls.provider_preparation_request(&id)),
                window.update(|_, cx| app.read(cx).run_controls.sessions_for(&id))
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        window.update(|_, cx| {
            app.update(cx, |state, _| {
                state.run_controls = reopened;
                state.run_controls.select(&id, &key);
            })
        });
        std::fs::remove_file(root.join("observed.txt")).unwrap();
        click(window, "run-start");
        wait_for_file(
            &mut manager,
            &app,
            window,
            &mut renderer,
            &mut launches,
            &root.join("observed.txt"),
        );
        let next = std::fs::read_to_string(root.join("observed.txt")).unwrap();
        assert!(next.starts_with("[\"hand edited\"]|"), "{next}");
        assert!(next.ends_with(&format!("|{private_value}")), "{next}");
        assert_eq!(
            std::path::Path::new(next.rsplit('|').nth(1).unwrap())
                .canonicalize()
                .unwrap(),
            *root,
        );
        assert_eq!(
            launches.len(),
            2,
            "the reopened values must start exactly one new run"
        );
        manager.shutdown();
        publish_with_launches(&mut manager, &mut renderer, &app, window, &launches);

        // An unreadable local file must report failure and block execution, preserving the exact corrupt bytes.
        let path = editor_core::configuration_path(&base, &key);
        std::fs::write(&path, b"{ broken").unwrap();
        window.update(|window, cx| {
            app.update(cx, |state, cx| {
                state.run_controls =
                    crate::run::RunControls::load_plugin_configurations(&key, Some(base.clone()));
                state.start_selected_run(window, cx);
            })
        });
        pump_recording(&mut manager, &app, window, &mut launches);
        assert_eq!(
            launches.len(),
            2,
            "a malformed reload must not execute a cached command"
        );
        assert!(window.update(|_, cx| app.read(cx).run_controls.launch_plan(&id, &key).is_err()));
        assert_eq!(std::fs::read(path).unwrap(), b"{ broken");
    }
}

/// Click a real painted local button, respecting its normal enabled and focus behavior.
fn click(cx: &mut gpui_kit::VisualTestContext, selector: &'static str) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let center = cx.debug_bounds(selector).unwrap().center();
    cx.simulate_click(center, Default::default());
    cx.run_until_parked();
}
/// Poll the production publication seam until the actual program writes its observable values.
fn wait_for_file(
    manager: &mut plugin_runtime::Manager,
    app: &Entity<EditorApp>,
    cx: &mut gpui_kit::VisualTestContext,
    renderer: &mut images::VectorRenderer,
    launches: &mut Vec<(u64, String, u64)>,
    path: &std::path::Path,
) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !path.exists() {
        pump_recording(manager, app, cx, launches);
        manager.poll();
        publish_with_launches(manager, renderer, app, cx, launches);
        assert!(
            Instant::now() < deadline,
            "shared program did not run: {}; sessions={:?}; text={}",
            cx.update(|_, cx| app.read(cx).status.clone()),
            manager
                .executions()
                .iter()
                .map(|session| (session.id(), session.state(), session.snapshot().failure))
                .collect::<Vec<_>>(),
            painted_text(app, cx)
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
/// Workspace keys use the editor's canonical path; no other project's settings are borrowed.
fn state_key(root: &std::path::Path) -> String {
    root.canonicalize().unwrap().display().to_string()
}
/// This native instrument records plain data; it never interprets argv as a script.
const PROBE: &str = "//! Shared configuration acceptance instrument.\nfn main(){let value=format!(\"{:?}|{}|{}\",std::env::args().skip(1).collect::<Vec<_>>(),std::env::current_dir().unwrap().display(),std::env::var(\"LOCAL_VALUE\").unwrap_or_default());std::fs::write(\"observed.txt\",value).unwrap();std::thread::sleep(std::time::Duration::from_secs(60));}\n";
