//! Real Java configuration controls drive the normal actor, build sequence and native execution.
use super::rename::wait_control;
use super::*;
use crate::extensions::native_configuration_tests::click;
use gpui_kit::{VisualContext as _, VisualTestContext};

/// Building a cold project produces class files; only Run executes main in the normal console.
#[gpui::test]
#[ignore = "Build Java with --plugin-package and set NANOBUG_JAVA_PACKAGE to the resulting ZIP"]
fn java_package_native_configuration_builds_before_running(cx: &mut TestAppContext) {
    let workspace = tempfile::Builder::new()
        .prefix("Java 运行 空格 ")
        .tempdir()
        .unwrap();
    std::fs::write(workspace.path().join("Main.java"), r#"public class Main {
        public static void main(String[] args) throws Exception {
            java.nio.file.Path marker = java.nio.file.Paths.get("run.txt");
            int count = java.nio.file.Files.exists(marker)
                ? Integer.parseInt(new String(java.nio.file.Files.readAllBytes(marker), "UTF-8").split(":")[0]) + 1 : 1;
            java.nio.file.Files.write(marker, (count + ":中文 run").getBytes("UTF-8"));
            System.out.println("中文 run");
            String line = new java.util.Scanner(System.in, "UTF-8").nextLine();
            java.nio.file.Files.write(java.nio.file.Paths.get("input.txt"), (line + "\n" + args[0]).getBytes("UTF-8"));
            while (true) { Thread.sleep(1000); }
        }
    }"#).unwrap();
    let (fixture, visual) =
        setup_for_workspace(cx, &["NANOBUG_JAVA_PACKAGE"], workspace.path().into());
    let parent = visual.window_handle();
    visual.update(|window, cx| {
        fixture.app.update(cx, |app, cx| {
            app.open_run_config_dialog(window, cx, None);
        })
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
    // The Java main-class field is fixed above the SDK's command-fields scroller.
    // Check its actual form page, then dispatch the same pointer movement and click events.
    let main_field = visual.debug_bounds("plugin-ui-java-main-class").unwrap();
    assert!(
        visual
            .debug_bounds("run-config-page")
            .unwrap()
            .contains(&main_field.center())
    );
    crate::extensions::native_configuration_tests::click_at(visual, main_field.center());
    visual.simulate_keystrokes("ctrl-a");
    visual.simulate_input("Main");
    wait_control(&fixture, visual, "plugin-button-arg-add", 30);
    click(visual, "plugin-button-arg-add");
    wait_control(&fixture, visual, "plugin-ui-arg-0", 30);
    click(visual, "plugin-ui-arg-0");
    visual.simulate_input("two words & 中文 ; \"quotes\"");
    click(visual, "run-config-save");
    *visual = VisualTestContext::from_window(parent, &visual.cx);
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Java application saved",
        30,
        |app, _| app.run_form.is_none() && app.run_controls.selected().is_some(),
    );
    assert!(
        !workspace.path().join("out/Main.class").exists(),
        "Save is pure"
    );
    wait_control(&fixture, visual, "run-build", 30);
    click(visual, "run-build");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Native Java build completed",
        180,
        |app, _| {
            workspace.path().join("out/Main.class").exists()
                && app
                    .run_controls
                    .selected()
                    .is_some_and(|config| !app.run_controls.is_preparing(&config.id))
        },
    );
    assert!(
        !workspace.path().join("run.txt").exists(),
        "Build cannot execute main"
    );
    click(visual, "run-start");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Native Java main completed",
        90,
        |_, _| {
            std::fs::read_to_string(workspace.path().join("run.txt"))
                .is_ok_and(|text| text == "1:中文 run")
        },
    );
    // A native output frame may follow the target's marker file; observe actual painted glyphs.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        wait(
            &fixture.app,
            &fixture.actor,
            visual,
            "Java console frame",
            10,
            |_, _| true,
        );
        let output = crate::terminal::tests::painted(&fixture.app, visual);
        if output.contains("中文run") {
            break;
        }
        assert!(Instant::now() < deadline, "Native Java console: {output}");
        std::thread::sleep(Duration::from_millis(10));
    }
    click(visual, "native-terminal-output");
    visual.simulate_input("中文 输入 & literal");
    visual.simulate_keystrokes("enter");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Java literal stdin and argv",
        15,
        |_, _| {
            std::fs::read_to_string(workspace.path().join("input.txt"))
                .is_ok_and(|text| text == "中文 输入 & literal\ntwo words & 中文 ; \"quotes\"")
        },
    );
    let configuration = visual.update(|_, cx| {
        fixture
            .app
            .read(cx)
            .run_controls
            .selected()
            .unwrap()
            .id
            .clone()
    });
    click(visual, "run-stop");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Java native Stop",
        15,
        |app, _| {
            app.run_controls.running_for(&configuration).is_none()
                && !app.run_controls.is_preparing(&configuration)
        },
    );
    click(visual, "run-start");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Java second launch",
        60,
        |_, _| {
            std::fs::read_to_string(workspace.path().join("run.txt"))
                .is_ok_and(|text| text == "2:中文 run")
        },
    );
    click(visual, "terminal-task-rerun");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Java Rerun launches once",
        60,
        |_, _| {
            std::fs::read_to_string(workspace.path().join("run.txt"))
                .is_ok_and(|text| text == "3:中文 run")
        },
    );
    fixture.actor.tx.send(Work::Disable("java".into())).unwrap();
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Java retirement revokes the running target",
        15,
        |app, _| app.run_controls.running_for(&configuration).is_none(),
    );
    shutdown(&fixture, visual);
}
