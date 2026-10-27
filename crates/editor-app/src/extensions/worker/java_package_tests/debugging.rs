//! Actual configuration, source shortcut, inspection buttons and console control a real Java target.
use super::{rename::wait_control, *};
use crate::extensions::native_configuration_tests::{click, click_at};
use gpui_kit::{VisualContext as _, VisualTestContext};

/// Native thread/frame selection and lazy expansion retain one JVM, including console stdin/argv.
#[gpui::test]
#[ignore = "Build Java with --plugin-package and set NANOBUG_JAVA_PACKAGE to the resulting ZIP"]
fn java_package_native_debug_threads_variables_and_controls(cx: &mut TestAppContext) {
    let workspace = tempfile::Builder::new()
        .prefix("Java 调试 UI ")
        .tempdir()
        .unwrap();
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../plugin-runtime/tests/fixtures/java-debug/Main.java"
    ));
    std::fs::write(workspace.path().join("Main.java"), source).unwrap();
    let breakpoint = source
        .lines()
        .position(|line| line.contains("// BREAKPOINT"))
        .unwrap() as u32;
    let (fixture, visual) =
        setup_for_workspace(cx, &["NANOBUG_JAVA_PACKAGE"], workspace.path().into());
    let parent = visual.window_handle();
    visual.update(|window, cx| {
        fixture
            .app
            .update(cx, |app, cx| app.open_run_config_dialog(window, cx, None))
    });
    visual.run_until_parked();
    let form = visual
        .cx
        .update(|cx| fixture.app.read(cx).run_dialog_window.unwrap());
    *visual = VisualTestContext::from_window(form.into(), &visual.cx);
    visual.update(|window, cx| {
        window.activate_window();
        window.draw(cx).clear(cx);
    });
    wait_control(&fixture, visual, "run-config-add", 30);
    click(visual, "run-config-add");
    wait_control(&fixture, visual, "run-template-java-application", 90);
    click(visual, "run-template-java-application");
    wait_control(&fixture, visual, "plugin-ui-java-main-class", 30);
    let main_point = visual
        .debug_bounds("plugin-ui-java-main-class")
        .unwrap()
        .center();
    click_at(visual, main_point);
    visual.simulate_keystrokes("ctrl-a");
    visual.simulate_input("Main");
    click(visual, "plugin-button-arg-add");
    wait_control(&fixture, visual, "plugin-ui-arg-0", 30);
    click(visual, "plugin-ui-arg-0");
    visual.simulate_input("two words & 中文 ; \"quotes\" \\tail\\");
    click(visual, "run-config-save");
    *visual = VisualTestContext::from_window(parent, &visual.cx);
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Java debug configuration saved",
        30,
        |app, _| app.run_form.is_none() && app.run_controls.selected().is_some(),
    );
    visual.update(|window, cx| {
        fixture.app.update(cx, |app, cx| {
            app.open_file(fixture.workspace.join("Main.java"), window, cx);
            app.editor.update(cx, |editor, cx| {
                editor.set_cursor_position(lsp_types::Position::new(breakpoint, 0), window, cx)
            });
        })
    });
    visual.run_until_parked();
    visual.simulate_keystrokes("f9");
    assert!(
        visual.update(|_, cx| fixture
            .app
            .read(cx)
            .run_controls
            .selected()
            .unwrap()
            .breakpoints
            .entries()
            .iter()
            .any(|entry| entry.line == breakpoint + 1)),
        "F9 must persist the actual native source line"
    );
    click(visual, "run-debug");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Real Java source pause and locals",
        180,
        |app, _| {
            app.run_controls
                .debug_panel_rows()
                .variables
                .iter()
                .any(|row| row.label.contains("value = 7"))
        },
    );
    let threads =
        visual.update(|_, cx| fixture.app.read(cx).run_controls.debug_panel_rows().threads);
    let main = threads.iter().find(|row| row.selected).unwrap().id;
    let worker = threads
        .iter()
        .find(|row| row.label.contains("worker中文"))
        .unwrap()
        .id;
    let worker_selector = Box::leak(format!("debug-panel-thread-{worker}").into_boxed_str());
    click_scrolled(visual, "debug-panel-frames", worker_selector);
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Worker stack and variables",
        20,
        |app, _| {
            app.run_controls
                .debug_frames()
                .iter()
                .any(|frame| frame.name.contains("hold"))
                && app
                    .run_controls
                    .debug_panel_rows()
                    .variables
                    .iter()
                    .any(|row| row.label.contains("workerValue = 99"))
        },
    );
    let main_selector = Box::leak(format!("debug-panel-thread-{main}").into_boxed_str());
    click_scrolled(visual, "debug-panel-frames", main_selector);
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Main-thread locals restored",
        20,
        |app, _| {
            app.run_controls
                .debug_panel_rows()
                .variables
                .iter()
                .any(|row| row.label.contains("node ="))
        },
    );
    let node = visual.update(|_, cx| {
        fixture
            .app
            .read(cx)
            .run_controls
            .debug_panel_rows()
            .variables
            .into_iter()
            .find(|row| row.label.contains("node ="))
            .unwrap()
            .selector
    });
    click_scrolled(
        visual,
        "debug-panel-variables",
        Box::leak(node.into_boxed_str()),
    );
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Native object expansion",
        20,
        |app, _| {
            app.run_controls
                .debug_panel_rows()
                .variables
                .iter()
                .any(|row| row.label.contains("number = 42"))
        },
    );
    let numbers = visual.update(|_, cx| {
        fixture
            .app
            .read(cx)
            .run_controls
            .debug_panel_rows()
            .variables
            .into_iter()
            .find(|row| row.label.contains("numbers ="))
            .unwrap()
            .selector
    });
    click_scrolled(
        visual,
        "debug-panel-variables",
        Box::leak(numbers.into_boxed_str()),
    );
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "First array page",
        20,
        |app, _| {
            app.run_controls
                .debug_panel_rows()
                .variables
                .iter()
                .any(|row| {
                    matches!(
                        row.action,
                        Some(crate::run::inspection::VariableAction::More { .. })
                    )
                })
        },
    );
    let more = visual.update(|_, cx| {
        fixture
            .app
            .read(cx)
            .run_controls
            .debug_panel_rows()
            .variables
            .into_iter()
            .find(|row| {
                matches!(
                    row.action,
                    Some(crate::run::inspection::VariableAction::More { .. })
                )
            })
            .unwrap()
            .selector
    });
    click_scrolled(
        visual,
        "debug-panel-variables",
        Box::leak(more.into_boxed_str()),
    );
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Second array page",
        20,
        |app, _| {
            app.run_controls
                .debug_panel_rows()
                .variables
                .iter()
                .any(|row| row.label.ends_with(" = 100"))
        },
    );
    for (selector, into) in [
        ("debug-panel-step-into", true),
        ("debug-panel-step-out", false),
        ("debug-panel-step-over", false),
    ] {
        let epoch = visual.update(|_, cx| {
            fixture
                .app
                .read(cx)
                .run_controls
                .debug_pause_epoch()
                .unwrap()
        });
        click(visual, selector);
        wait(
            &fixture.app,
            &fixture.actor,
            visual,
            "Real Java single step",
            20,
            |app, _| {
                app.run_controls
                    .debug_pause_epoch()
                    .is_some_and(|pause| pause > epoch)
                    && !app.run_controls.debug_action_pending()
                    && !app.run_controls.debug_frames().is_empty()
            },
        );
        if into {
            assert!(visual.update(|_, cx| {
                fixture
                    .app
                    .read(cx)
                    .run_controls
                    .debug_frames()
                    .iter()
                    .any(|frame| frame.name.contains("calculate"))
            }));
        }
    }
    click(visual, "debug-panel-resume");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Java debug resumed",
        20,
        |app, _| {
            matches!(
                app.run_controls.debug_state(),
                editor_core::DebugSessionState::Running
            )
        },
    );
    click(visual, "native-terminal-output");
    visual.simulate_input("调试 输入");
    visual.simulate_keystrokes("enter");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Debug JVM receives literal stdin/argv",
        20,
        |_, _| {
            std::fs::read_to_string(workspace.path().join("debug-input.txt"))
                .is_ok_and(|text| text == "调试 输入\ntwo words & 中文 ; \"quotes\" \\tail\\")
        },
    );
    click(visual, "debug-panel-pause");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Manual Java pause",
        20,
        |app, _| {
            matches!(
                app.run_controls.debug_state(),
                editor_core::DebugSessionState::Paused { .. }
            )
        },
    );
    assert_eq!(
        std::fs::read_to_string(workspace.path().join("starts.txt"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    click(visual, "debug-panel-stop");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Debug native exit",
        20,
        |app, _| {
            matches!(
                app.run_controls.debug_state(),
                editor_core::DebugSessionState::Exited
            )
        },
    );
    fixture.actor.tx.send(Work::Disable("java".into())).unwrap();
    shutdown(&fixture, visual);
}

/// Scroll the actual native pane until the intended row is inside its viewport, then click it.
fn click_scrolled(visual: &mut VisualTestContext, pane: &'static str, selector: &'static str) {
    for _ in 0..100 {
        let bounds = visual.debug_bounds(pane).unwrap();
        let target = visual.debug_bounds(selector);
        if target.is_some_and(|target| bounds.contains(&target.center())) {
            click(visual, selector);
            return;
        }
        let delta = if target.is_some_and(|target| target.center().y < bounds.top()) {
            120.
        } else {
            -120.
        };
        visual.simulate_event(gpui::MouseMoveEvent {
            position: bounds.center(),
            pressed_button: None,
            modifiers: Default::default(),
        });
        visual.run_until_parked();
        visual.simulate_event(gpui::ScrollEvent {
            // Wheel and trackpad fixtures retain pointer-device semantics.
            source: gpui_kit::ScrollSource::Pointer,
            position: bounds.center(),
            delta: gpui::ScrollDelta::Pixels(gpui_kit::point(
                gpui_kit::px(0.),
                gpui_kit::px(delta),
            )),
            modifiers: Default::default(),
            touch_phase: gpui::TouchPhase::Moved,
        });
        visual.run_until_parked();
        std::thread::sleep(Duration::from_millis(10));
        visual.update(|window, cx| window.draw(cx).clear(cx));
    }
    panic!(
        "Native row remained clipped: {selector}; pane={:?}; row={:?}",
        visual.debug_bounds(pane),
        visual.debug_bounds(selector)
    );
}
