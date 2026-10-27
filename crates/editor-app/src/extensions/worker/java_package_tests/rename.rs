//! The installed Java plugin must route cross-file rename through ordinary native preview and recovery.
use super::super::vue_test_support::Fixture;
use super::*;
use gpui_kit::{EntityInputHandler as _, VisualTestContext};
use std::time::{Duration, Instant};

/// A rename previews both dirty buffers, changes them together, and restores both with group Undo.
#[gpui::test]
#[ignore = "Build Java with --plugin-package and set NANOBUG_JAVA_PACKAGE to the resulting ZIP"]
fn java_package_rename_previews_two_sources_and_supports_native_group_undo(
    cx: &mut TestAppContext,
) {
    let workspace = tempfile::tempdir().unwrap();
    let main_disk = "public class Main { String value = Helper.greet(); }\n";
    let helper_disk = "public class Helper { static String greet() { return \"hello\"; } }\n";
    std::fs::write(workspace.path().join("Main.java"), main_disk).unwrap();
    std::fs::write(workspace.path().join("Helper.java"), helper_disk).unwrap();
    let (fixture, visual) =
        setup_for_workspace(cx, &["NANOBUG_JAVA_PACKAGE"], workspace.path().into());
    let main = fixture.workspace.join("Main.java");
    let helper = fixture.workspace.join("Helper.java");
    let main_source = format!("{main_disk}// unsaved 主入口\n");
    let helper_source = format!("{helper_disk}// unsaved 帮助类\n");
    for (path, source) in [(&helper, &helper_source), (&main, &main_source)] {
        visual.update(|window, cx| {
            fixture
                .app
                .update(cx, |app, cx| app.open_file(path.clone(), window, cx));
            window.draw(cx).clear(cx);
        });
        visual.simulate_keystrokes("ctrl-a");
        visual.simulate_input(source);
    }
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Java rename provider",
        180,
        |app, cx| {
            app.editor.read(cx).lsp().definition_provider.is_some()
                && app
                    .language_service_states
                    .values()
                    .any(|state| *state == crate::app::language_servers::ServiceLoadState::Ready)
        },
    );
    visual.update(|window, cx| {
        fixture
            .app
            .read(cx)
            .editor
            .clone()
            .update(cx, |editor, cx| {
                let offset = main_source.find("greet").unwrap() + 1;
                editor.set_selected_range(offset..offset, cx);
                editor.focus(window, cx);
            });
    });
    visual.simulate_keystrokes("f2");
    wait_control(&fixture, visual, "editor-rename-prompt", 60);
    visual.simulate_input("welcome");
    visual.simulate_keystrokes("enter");
    wait_control(&fixture, visual, "workspace-plan-confirm", 30);
    assert_eq!(
        source(&fixture, visual, &main),
        main_source,
        "Preview cannot edit the initiating buffer"
    );
    assert_eq!(
        source(&fixture, visual, &helper),
        helper_source,
        "Preview cannot edit another dirty buffer"
    );
    click(visual, "workspace-plan-confirm");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Java rename transaction",
        30,
        |app, cx| {
            app.tabs
                .iter()
                .filter_map(|tab| tab.text.as_ref())
                .filter(|tab| tab.editor.read(cx).text().to_string().contains("welcome"))
                .count()
                == 2
        },
    );
    assert_eq!(
        source(&fixture, visual, &main),
        main_source.replace("greet", "welcome")
    );
    assert_eq!(
        source(&fixture, visual, &helper),
        helper_source.replace("greet", "welcome")
    );
    assert_eq!(std::fs::read_to_string(&main).unwrap(), main_disk);
    assert_eq!(std::fs::read_to_string(&helper).unwrap(), helper_disk);
    wait_control(&fixture, visual, "workspace-batch-undo", 15);
    click(visual, "workspace-batch-undo");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Java rename group Undo",
        30,
        |app, cx| {
            app.tabs
                .iter()
                .filter_map(|tab| tab.text.as_ref())
                .all(|tab| !tab.editor.read(cx).text().to_string().contains("welcome"))
        },
    );
    assert_eq!(source(&fixture, visual, &main), main_source);
    assert_eq!(source(&fixture, visual, &helper), helper_source);
    shutdown(&fixture, visual);
}

/// A closed dependency must acquire its own native revision before joining the same preview and Undo.
#[gpui::test]
#[ignore = "Build Java with --plugin-package and set NANOBUG_JAVA_PACKAGE to the resulting ZIP"]
fn java_package_rename_admits_an_unopened_source_before_native_preview(cx: &mut TestAppContext) {
    let workspace = tempfile::tempdir().unwrap();
    let main_source = "public class Main { String value = Helper.greet(); }\n";
    let helper_source = "public class Helper { static String greet() { return \"hello\"; } }\n";
    std::fs::write(workspace.path().join("Main.java"), main_source).unwrap();
    let (fixture, visual) =
        setup_for_workspace(cx, &["NANOBUG_JAVA_PACKAGE"], workspace.path().into());
    let main = fixture.workspace.join("Main.java");
    let helper = fixture.workspace.join("Helper.java");
    // Startup may restore/open the first project file. Add this dependency afterwards so the
    // scenario proves it is genuinely absent from the native document set before rename.
    std::fs::write(&helper, helper_source).unwrap();
    visual.update(|window, cx| {
        fixture
            .app
            .update(cx, |app, cx| app.open_file(main.clone(), window, cx));
        window.draw(cx).clear(cx);
    });
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Java single-source provider",
        180,
        |app, cx| {
            app.editor.read(cx).lsp().definition_provider.is_some()
                && app
                    .language_service_states
                    .values()
                    .any(|state| *state == crate::app::language_servers::ServiceLoadState::Ready)
        },
    );
    visual.update(|window, cx| {
        let app = fixture.app.read(cx);
        assert!(
            app.tabs.iter().all(|tab| tab.path() != helper),
            "The dependency starts unopened"
        );
        app.editor.clone().update(cx, |editor, cx| {
            let offset = main_source.find("greet").unwrap() + 1;
            editor.set_selected_range(offset..offset, cx);
            editor.focus(window, cx);
        });
    });
    visual.simulate_keystrokes("f2");
    wait_control(&fixture, visual, "editor-rename-prompt", 60);
    visual.simulate_input("welcome");
    visual.simulate_keystrokes("enter");
    wait_control(&fixture, visual, "workspace-plan-confirm", 30);
    assert_eq!(source(&fixture, visual, &main), main_source);
    assert_eq!(source(&fixture, visual, &helper), helper_source);
    click(visual, "workspace-plan-confirm");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Java unopened-target rename",
        30,
        |app, cx| {
            app.tabs
                .iter()
                .filter_map(|tab| tab.text.as_ref())
                .filter(|tab| tab.editor.read(cx).text().to_string().contains("welcome"))
                .count()
                == 2
        },
    );
    assert_eq!(std::fs::read_to_string(&main).unwrap(), main_source);
    assert_eq!(std::fs::read_to_string(&helper).unwrap(), helper_source);
    wait_control(&fixture, visual, "workspace-batch-undo", 15);
    click(visual, "workspace-batch-undo");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Java unopened-target group Undo",
        30,
        |app, cx| {
            app.tabs
                .iter()
                .filter_map(|tab| tab.text.as_ref())
                .all(|tab| !tab.editor.read(cx).text().to_string().contains("welcome"))
        },
    );
    assert_eq!(source(&fixture, visual, &main), main_source);
    assert_eq!(source(&fixture, visual, &helper), helper_source);
    shutdown(&fixture, visual);
}

/// A top-level type and its physical filename share one preview and native group recovery.
#[gpui::test]
#[ignore = "Build Java with --plugin-package and set NANOBUG_JAVA_PACKAGE to the resulting ZIP"]
fn java_package_type_rename_reconciles_the_filename_and_native_undo(cx: &mut TestAppContext) {
    let workspace = tempfile::tempdir().unwrap();
    let original = "public class Main { Main self; }\n";
    std::fs::write(workspace.path().join("Main.java"), original).unwrap();
    let (fixture, visual) =
        setup_for_workspace(cx, &["NANOBUG_JAVA_PACKAGE"], workspace.path().into());
    let old = fixture.workspace.join("Main.java");
    let renamed = fixture.workspace.join("Renamed.java");
    visual.update(|window, cx| {
        fixture
            .app
            .update(cx, |app, cx| app.open_file(old.clone(), window, cx));
        window.draw(cx).clear(cx);
    });
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Java type rename provider",
        180,
        |app, cx| {
            app.editor.read(cx).lsp().definition_provider.is_some()
                && app
                    .language_service_states
                    .values()
                    .any(|state| *state == crate::app::language_servers::ServiceLoadState::Ready)
        },
    );
    visual.update(|window, cx| {
        fixture
            .app
            .read(cx)
            .editor
            .clone()
            .update(cx, |editor, cx| {
                let offset = original.find("Main").unwrap() + 1;
                editor.set_selected_range(offset..offset, cx);
                editor.focus(window, cx);
            });
    });
    visual.simulate_keystrokes("f2");
    wait_control(&fixture, visual, "editor-rename-prompt", 60);
    visual.simulate_input("Renamed");
    visual.simulate_keystrokes("enter");
    wait_control(&fixture, visual, "workspace-plan-confirm", 30);
    assert_eq!(source(&fixture, visual, &old), original);
    assert!(old.exists() && !renamed.exists());
    click(visual, "workspace-plan-confirm");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Java filename rename",
        30,
        |app, _| {
            renamed.exists() && !old.exists() && app.tabs.iter().any(|tab| tab.path() == renamed)
        },
    );
    assert_eq!(
        source(&fixture, visual, &renamed),
        "public class Renamed { Renamed self; }\n"
    );
    // Renaming changes disk identity; native text edits remain unsaved until the user saves.
    assert_eq!(std::fs::read_to_string(&renamed).unwrap(), original);
    wait_control(&fixture, visual, "workspace-batch-undo", 15);
    click(visual, "workspace-batch-undo");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Java filename rename Undo",
        30,
        |app, _| old.exists() && !renamed.exists() && app.tabs.iter().any(|tab| tab.path() == old),
    );
    assert_eq!(source(&fixture, visual, &old), original);
    shutdown(&fixture, visual);
}

/// A related native revision changing during preview invalidates the entire semantic proposal.
#[gpui::test]
#[ignore = "Build Java with --plugin-package and set NANOBUG_JAVA_PACKAGE to the resulting ZIP"]
fn java_package_rename_discards_preview_after_related_native_input(cx: &mut TestAppContext) {
    let workspace = tempfile::tempdir().unwrap();
    let main_source = "public class Main { String value = Helper.greet(); }\n";
    let helper_source = "public class Helper { static String greet() { return \"hello\"; } }\n";
    std::fs::write(workspace.path().join("Main.java"), main_source).unwrap();
    std::fs::write(workspace.path().join("Helper.java"), helper_source).unwrap();
    let (fixture, visual) =
        setup_for_workspace(cx, &["NANOBUG_JAVA_PACKAGE"], workspace.path().into());
    let main = fixture.workspace.join("Main.java");
    let helper = fixture.workspace.join("Helper.java");
    for path in [&helper, &main] {
        visual.update(|window, cx| {
            fixture
                .app
                .update(cx, |app, cx| app.open_file(path.clone(), window, cx));
            window.draw(cx).clear(cx);
        });
    }
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Java guarded rename provider",
        180,
        |app, cx| {
            app.editor.read(cx).lsp().definition_provider.is_some()
                && app
                    .language_service_states
                    .values()
                    .any(|state| *state == crate::app::language_servers::ServiceLoadState::Ready)
        },
    );
    visual.update(|window, cx| {
        fixture
            .app
            .read(cx)
            .editor
            .clone()
            .update(cx, |editor, cx| {
                let offset = main_source.find("greet").unwrap() + 1;
                editor.set_selected_range(offset..offset, cx);
                editor.focus(window, cx);
            });
    });
    visual.simulate_keystrokes("f2");
    wait_control(&fixture, visual, "editor-rename-prompt", 60);
    visual.simulate_input("welcome");
    visual.simulate_keystrokes("enter");
    wait_control(&fixture, visual, "workspace-plan-confirm", 30);
    // Exercise the same public native input handler as typing, retaining its unique session/history.
    visual.update(|window, cx| {
        let editor = fixture
            .app
            .read(cx)
            .tabs
            .iter()
            .find(|tab| tab.path() == helper)
            .unwrap()
            .text
            .as_ref()
            .unwrap()
            .editor
            .clone();
        editor.update(cx, |editor, cx| {
            editor.set_selected_range(helper_source.len()..helper_source.len(), cx);
            editor.replace_text_in_range(None, "// concurrent 原生输入\n", window, cx);
        });
    });
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        wait(
            &fixture.app,
            &fixture.actor,
            visual,
            "stale Java preview",
            1,
            |_, _| true,
        );
        if visual.debug_bounds("workspace-plan-confirm").is_none() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Stale refactor preview remained executable"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(source(&fixture, visual, &main), main_source);
    assert_eq!(
        source(&fixture, visual, &helper),
        format!("{helper_source}// concurrent 原生输入\n")
    );
    assert_eq!(std::fs::read_to_string(&main).unwrap(), main_source);
    assert_eq!(std::fs::read_to_string(&helper).unwrap(), helper_source);
    shutdown(&fixture, visual);
}

/// Paint through the real actor until a named native control becomes available, with a bounded wait.
pub(super) fn wait_control(
    fixture: &Fixture,
    visual: &mut VisualTestContext,
    selector: &'static str,
    seconds: u64,
) {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    loop {
        wait(
            &fixture.app,
            &fixture.actor,
            visual,
            "native frame",
            1,
            |_, _| true,
        );
        if visual.debug_bounds(selector).is_some() {
            return;
        }
        if Instant::now() >= deadline {
            let state = fixture.actor.state.lock().unwrap();
            let failures = state
                .entries
                .iter()
                .map(|entry| (&entry.manifest.id, &entry.error))
                .collect::<Vec<_>>();
            let logs = state
                .logs
                .records("java")
                .into_iter()
                .filter(|record| record.level > plugin_runtime::LogLevel::Info)
                .map(|record| (record.source, record.message))
                .collect::<Vec<_>>();
            panic!(
                "{selector}: {}; menu={:?}; providers={failures:?}; logs={logs:?}",
                visual.update(|_, cx| fixture.app.read(cx).status.clone()),
                visual.debug_bounds("native-popup-menu")
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Click the rendered Base control rather than invoking a private transaction implementation.
pub(super) fn click(visual: &mut VisualTestContext, selector: &'static str) {
    let bounds = visual.debug_bounds(selector).unwrap();
    visual.simulate_click(bounds.center(), Default::default());
}

/// Observe the sole native document text without opening or reloading the target from disk.
fn source(fixture: &Fixture, visual: &mut VisualTestContext, path: &PathBuf) -> String {
    visual.update(|_, cx| {
        fixture
            .app
            .read(cx)
            .tabs
            .iter()
            .find(|tab| tab.path() == path)
            .unwrap()
            .text
            .as_ref()
            .unwrap()
            .editor
            .read(cx)
            .text()
            .to_string()
    })
}
