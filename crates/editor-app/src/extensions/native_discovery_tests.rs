//! Real packages, native staged target selection and save, exact artifacts and explicit repair.
#![cfg(windows)]
use super::composable_tests::{publish_frame, publish_with_launches, pump_recording_all};
use super::*;
use gpui_kit::{TestAppContext, gpui};
use plugin_runtime::Manager;
use std::time::{Duration, Instant};

mod debugging;

/// Retain both production asynchronous dispatchers while native frames are painted.
#[derive(Default)]
struct Driver {
    renderer: images::VectorRenderer,
    launches: Vec<(u64, String, u64)>,
    debug: BTreeMap<u64, (String, plugin_runtime::DebugRequest)>,
    targets: super::worker::targets::TargetCalls,
    configurations: super::worker::configurations::ConfigurationCalls,
}
impl Driver {
    fn frame(
        &mut self,
        manager: &mut Manager,
        app: &Entity<EditorApp>,
        cx: &mut gpui_kit::VisualTestContext,
    ) {
        manager.poll();
        let (statuses, stops) = pump_recording_all(
            manager,
            app,
            cx,
            &mut self.launches,
            &mut self.debug,
            &mut self.targets,
            &mut self.configurations,
        );
        publish_frame(
            manager,
            &mut self.renderer,
            app,
            cx,
            &self.launches,
            statuses,
            stops,
        );
    }
    #[track_caller]
    fn wait(
        &mut self,
        manager: &mut Manager,
        app: &Entity<EditorApp>,
        cx: &mut gpui_kit::VisualTestContext,
        predicate: impl Fn(&EditorApp) -> bool,
    ) {
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            self.frame(manager, app, cx);
            if cx.update(|_, cx| predicate(app.read(cx))) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "native flow timed out: {}; {:?}; selected={:?}; frames={:?}",
                cx.update(|_, cx| app.read(cx).status.clone()),
                manager.debug_observations(),
                cx.update(|_, cx| app
                    .read(cx)
                    .run_controls
                    .debug_session()
                    .map(|(id, _)| id.to_owned())),
                cx.update(|_, cx| app.read(cx).run_controls.debug_frames().to_vec())
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
/// Install actual distribution ZIPs and keep all host-local state under the isolated fixture.
fn editor_for<'a>(
    cx: &'a mut TestAppContext,
    root: &Path,
) -> (
    Manager,
    Entity<EditorApp>,
    &'a mut gpui_kit::VisualTestContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        cx.set_reduce_motion(true);
    });
    let mut manager = Manager::open(
        root.join(".runtime-plugin-test"),
        protocol::Environment {
            workspace: root.display().to_string(),
            os: "windows".into(),
            ..Default::default()
        },
    )
    .unwrap();
    // Discovery/build rules remain plugins; the terminal is provided by the native application.
    for name in ["rust", "run-target-example"] {
        let package = Package::read(&crate::test_packages::archive(name)).unwrap();
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
    }
    super::test_configurations::install(&mut manager);
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let workspace = Workspace::open(root).unwrap();
    let (_, cx) = cx.add_window_view(move |window, cx| {
        let app = cx.new(|cx| EditorApp::new(workspace, None, window, cx));
        *capture.borrow_mut() = Some(app.clone());
        app.update(cx, |state, cx| {
            let key = state.workspace_key();
            state.run_controls = crate::run::RunControls::load_plugin_configurations(
                &key,
                Some(state.workspace.root().join("private-runs")),
            );
            cx.notify();
        });
        Root::new(app, window, cx)
    });
    let app = slot.borrow_mut().take().unwrap();
    cx.simulate_resize(size(px(1400.), px(900.)));
    let mut renderer = images::VectorRenderer::default();
    publish_with_launches(&mut manager, &mut renderer, &app, cx, &[]);
    (manager, app, cx)
}
/// Click the painted native hit region, never a synthetic action dispatched by selector.
fn click(cx: &mut gpui_kit::VisualTestContext, selector: &'static str) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("missing native control {selector}"));
    // Physical clicks move the pointer first; hover hit testing is part of native event routing.
    cx.simulate_event(gpui::MouseMoveEvent {
        position: bounds.center(),
        pressed_button: None,
        modifiers: Default::default(),
    });
    cx.run_until_parked();
    cx.simulate_click(bounds.center(), Default::default());
    cx.run_until_parked();
}
/// The final program itself records argv/env so a compiled artifact is distinguishable from Cargo.
fn rust_project(root: &Path, name: &str) {
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        format!("[package]\nname=\"{name}\"\nversion=\"0.1.0\"\nedition=\"2024\"\n"),
    )
    .unwrap();
    std::fs::write(root.join("src/main.rs"),"//! Native discovery acceptance program.\nfn main() {let value=format!(\"{:?} / {}\",std::env::args().skip(1).collect::<Vec<_>>(),std::env::var(\"RUN_ENV\").unwrap_or_default());std::fs::write(\"program-ran.txt\",value).unwrap();}\n").unwrap();
}

#[gpui::test]
#[ignore = "build rust and run-target-example through the current public SDK first"]
fn a_real_rust_project_is_discovered_built_and_run(cx: &mut TestAppContext) {
    let root = tempfile::tempdir().unwrap();
    rust_project(root.path(), "delivery-target");
    let (mut manager, app, cx) = editor_for(cx, root.path());
    let mut driver = Driver::default();
    // Discovery remains a public target service. The retired target form is superseded by
    // plugin-owned configuration windows, whose native editing is covered by rollout_tests.
    cx.update(|_, cx| app.update(cx, |state, cx| state.discover_run_targets(cx)));
    driver.wait(&mut manager, &app, cx, |state| {
        state.run_controls.discovered_targets().len() == 2
    });
    assert!(
        cx.update(|_, cx| app.read(cx).run_controls.configurations().is_empty()),
        "discovery stores no candidates"
    );
    assert!(!root.path().join("program-ran.txt").exists());
    let target = cx.update(|_, cx| {
        app.read(cx)
            .run_controls
            .discovered_targets()
            .iter()
            .find(|target| target.label.ends_with("Debug"))
            .unwrap()
            .clone()
    });
    let mut draft = cx.update(|_, cx| {
        assert!(
            app.read(cx).run_controls.configurations().is_empty(),
            "a plugin choice is only a draft until Save"
        );
        app.read(cx)
            .run_controls
            .target_template(&target.id, &app.read(cx).workspace_key())
            .unwrap()
    });
    let stored = draft.id.clone();
    draft.name = "用户保留的名称".into();
    draft.env.insert("RUN_ENV".into(), "本机环境".into());
    let editor_core::RunTarget::Provided { args, .. } = &mut draft.target else {
        panic!("expected discovered target")
    };
    *args = vec![
        "literal space".into(),
        "quote\"value".into(),
        "中文;&|".into(),
    ];
    cx.update(|_, cx| {
        app.update(cx, |state, cx| {
            // Save the current opaque envelope, then recompute its launch through the real SDK
            // provider at execution time. This preserves exact discovered-artifact coverage.
            super::test_configurations::accept(&mut state.run_controls, draft);
            let set = state.run_controls.configuration_set();
            state
                .run_controls
                .commit_configuration_set(set, &state.workspace_key())
                .unwrap();
            state.run_controls.select(&stored, &state.workspace_key());
            cx.notify();
        })
    });
    let config = cx.update(|_, cx| {
        app.read(cx)
            .run_controls
            .configuration(&stored)
            .unwrap()
            .clone()
    });
    assert_eq!(config.name, "用户保留的名称");
    assert_eq!(
        config.literal_arguments(),
        ["literal space", "quote\"value", "中文;&|"]
    );
    assert_eq!(config.env["RUN_ENV"], "本机环境");
    assert!(matches!(
        config.target,
        editor_core::RunTarget::Provided { .. }
    ));
    click(cx, "run-build");
    driver.wait(&mut manager, &app, cx, |state| {
        state.run_controls.preparation(&stored).is_some()
            && !state.run_controls.is_pending(&stored)
            && !state.run_controls.is_preparing(&stored)
    });
    assert!(driver.launches.is_empty(), "Build creates no final program");
    assert!(!root.path().join("program-ran.txt").exists());
    // Build history now belongs to the built-in terminal's task tab, not the retired overlay.
    assert!(cx.debug_bounds("terminal-task-rerun").is_some());
    let build_history = super::native_run_tests::painted_text(&app, cx);
    assert!(
        build_history.contains("Finished"),
        "Cargo output was not retained: {build_history}"
    );
    click(cx, "native-terminal-hide");
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("terminal-task-rerun").is_none());
    cx.update(|window, cx| {
        app.update(cx, |state, cx| {
            state.toggle_terminal(window, cx);
        })
    });
    driver.frame(&mut manager, &app, cx);
    assert!(cx.debug_bounds("terminal-task-rerun").is_some());
    assert_eq!(
        super::native_run_tests::painted_text(&app, cx),
        build_history
    );
    click(cx, "run-start");
    driver.wait(&mut manager, &app, cx, |state| {
        root.path().join("program-ran.txt").exists()
            && !state.run_controls.is_pending(&stored)
            && !state.run_controls.is_preparing(&stored)
    });
    assert_eq!(driver.launches.len(), 1, "the artifact is executed once");
    let request = manager
        .execution(driver.launches[0].0)
        .unwrap()
        .request()
        .clone();
    assert!(request.program.ends_with("delivery-target.exe") && !request.program.contains("cargo"));
    let result = std::fs::read_to_string(root.path().join("program-ran.txt")).unwrap();
    assert!(
        result.contains("literal space")
            && result.contains("中文;&|")
            && result.ends_with(" / 本机环境"),
        "{result}"
    );
    manager.shutdown();
}

/// Rename/repair preserves edits; an independent plugin's defaults are staged and explicitly saved.
#[gpui::test]
#[ignore = "build rust and run-target-example through the current public SDK first"]
fn a_discovered_project_is_confirmed_built_run_and_offered_for_debugging(cx: &mut TestAppContext) {
    let root = tempfile::tempdir().unwrap();
    rust_project(root.path(), "original-target");
    std::fs::create_dir_all(root.path().join("tool")).unwrap();
    std::fs::write(root.path().join("tool/native-tool.toml"),"[tool]\nname=\"Independent native tool\"\nprogram=\"powershell.exe\"\narguments=\"-NoProfile\\n-Command\\nWrite-Output INDEPENDENT\"\n").unwrap();
    let (mut manager, app, cx) = editor_for(cx, root.path());
    let mut driver = Driver::default();
    cx.update(|_, cx| app.update(cx, |state, cx| state.discover_run_targets(cx)));
    driver.wait(&mut manager, &app, cx, |state| {
        state.run_controls.discovered_targets().len() == 3
    });
    let (target, independent) = cx.update(|_, cx| {
        let targets = app.read(cx).run_controls.discovered_targets();
        (
            targets
                .iter()
                .find(|target| target.label.ends_with("Debug"))
                .unwrap()
                .id
                .clone(),
            targets
                .iter()
                .find(|target| target.provider == "run-target-example")
                .unwrap()
                .clone(),
        )
    });
    assert_eq!(independent.target_type, "native-tool");
    let stored = cx.update(|_, cx| {
        app.update(cx, |state, _| {
            let key = state.workspace_key();
            let id = state.run_controls.confirm_target(&target, &key).unwrap();
            let mut config = state.run_controls.configuration(&id).unwrap().clone();
            config.name = "My retained edits".into();
            config.env.insert("RUN_ENV".into(), "RETAINED".into());
            if let editor_core::RunTarget::Provided { args, .. } = &mut config.target {
                args.push("user argv".into());
            }
            state.run_controls.upsert(config, &key).unwrap();
            id
        })
    });
    rust_project(root.path(), "renamed-target");
    cx.update(|_, cx| app.update(cx, |state, cx| state.discover_run_targets(cx)));
    driver.wait(&mut manager, &app, cx, |state| {
        state.run_controls.target_missing(&stored)
    });
    assert!(cx.update(|_, cx| app.read(cx).run_controls.launch_blocker(&stored).is_some()));
    assert!(driver.launches.is_empty());
    let renamed = cx.update(|_, cx| {
        app.read(cx)
            .run_controls
            .discovered_targets()
            .iter()
            .find(|target| target.label.ends_with("Debug"))
            .unwrap()
            .id
            .clone()
    });
    cx.update(|_, cx| {
        app.update(cx, |state, _| {
            let key = state.workspace_key();
            state
                .run_controls
                .repair_target_with(&stored, &renamed, &key)
                .unwrap();
        })
    });
    let repaired = cx.update(|_, cx| {
        app.read(cx)
            .run_controls
            .configuration(&stored)
            .unwrap()
            .clone()
    });
    assert_eq!(repaired.name, "My retained edits");
    assert_eq!(repaired.env["RUN_ENV"], "RETAINED");
    assert_eq!(repaired.literal_arguments(), ["user argv"]);
    assert!(cx.update(|_, cx| {
        !app.read(cx)
            .run_controls
            .configuration_set()
            .plugin_configurations
            .contains_key(&stored)
    }));
    click(cx, "run-start");
    driver.frame(&mut manager, &app, cx);
    assert!(
        driver.launches.is_empty() && !root.path().join("program-ran.txt").exists(),
        "legacy records cannot execute before current provider validation"
    );
    cx.update(|_, cx| {
        app.update(cx, |state, cx| {
            super::test_configurations::accept(&mut state.run_controls, repaired);
            let set = state.run_controls.configuration_set();
            state
                .run_controls
                .commit_configuration_set(set, &state.workspace_key())
                .unwrap();
            state.run_controls.select(&stored, &state.workspace_key());
            cx.notify();
        })
    });
    let count = cx.update(|_, cx| app.read(cx).run_controls.configurations().len());
    click(cx, "run-start");
    driver.wait(&mut manager, &app, cx, |state| {
        root.path().join("program-ran.txt").exists()
            && !state.run_controls.is_pending(&stored)
            && !state.run_controls.is_preparing(&stored)
    });
    assert_eq!(driver.launches.len(), 1);
    assert!(
        cx.update(|_, cx| app.read(cx).run_controls.debug_blocker(&stored).is_none()),
        "the unified Rust package also offers debugging for the repaired target"
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("program-ran.txt")).unwrap(),
        "[\"user argv\"] / RETAINED"
    );
    let draft = cx.update(|_, cx| {
        app.read(cx)
            .run_controls
            .target_template(&independent.id, &app.read(cx).workspace_key())
            .unwrap()
    });
    assert_eq!(
        cx.update(|_, cx| app.read(cx).run_controls.configurations().len()),
        count
    );
    cx.update(|_, cx| {
        app.update(cx, |state, cx| {
            let id = draft.id.clone();
            super::test_configurations::accept(&mut state.run_controls, draft);
            let set = state.run_controls.configuration_set();
            state
                .run_controls
                .commit_configuration_set(set, &state.workspace_key())
                .unwrap();
            state.run_controls.select(&id, &state.workspace_key());
            cx.notify();
        })
    });
    assert!(cx.update(|_,cx|matches!(app.read(cx).run_controls.selected().unwrap().target,editor_core::RunTarget::Program {ref program,..} if program=="powershell.exe")));
    assert_eq!(
        cx.update(|_, cx| app.read(cx).run_controls.configurations().len()),
        count + 1
    );
    manager.shutdown();
}
