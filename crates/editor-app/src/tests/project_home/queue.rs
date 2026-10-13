//! Closing during a real delayed language request must drain the native input and history FIFO.

use super::*;
use crate::app::projects::CloseIntent;
use gpui_kit::{EntityInputHandler as _, Keystroke};

// Reuse the same inspected Package and controlled stdio fixture as the editing integration tests.
#[allow(dead_code)]
#[path = "../editing_fixture.rs"]
mod editing_fixture;

/// Save and Close must include tail input following consecutive native Undo/Redo commands.
#[gpui::test]
#[ignore = "uses the frozen javascript Package and its approved private Node runtime"]
fn project_home_close_drains_native_history_and_tail_input(cx: &mut TestAppContext) {
    close_queued_input(cx, false);
}

/// Undo may return the session to clean while a queued platform preedit still needs committing.
#[gpui::test]
#[ignore = "uses the frozen javascript Package and its approved private Node runtime"]
fn project_home_close_drains_native_history_and_ime_tail(cx: &mut TestAppContext) {
    close_queued_input(cx, true);
}

/// Both native tails share the production package and window setup; only their command batch differs.
fn close_queued_input(cx: &mut TestAppContext, composing: bool) {
    let project = tempfile::tempdir().unwrap();
    let path = project.path().join("queued.linked");
    let source = "<!-- 中文🙂 -->\n<item>内容🙂</item>\n";
    std::fs::write(&path, source).unwrap();
    let log = project.path().join("wire.jsonl");
    let package = editing_fixture::package(
        "close-queue-language",
        "fixture",
        true,
        true,
        &log,
        2000,
        "bounded",
    );
    let profile = tempfile::tempdir().unwrap();
    with_home(cx, profile.path(), |visual, host| {
        click(visual, "project-open-folder");
        let selected = project.path().to_owned();
        visual.simulate_path_prompt_response(move |_| Some(vec![selected]));
        visual.run_until_parked();
        click(visual, "project-trust-accept");
        let editor = visual.update(|_, cx| host.read(cx).editor.clone().unwrap());
        let root = visual.update(|_, cx| editor.read(cx).workspace.root().to_owned());
        let mut manager = plugin_runtime::Manager::open(
            crate::extensions::profile_runtime_root(profile.path()),
            plugin_runtime::plugin_protocol::Environment {
                workspace: root.display().to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        crate::extensions::lsp_tests::publish(&editor, &mut manager, visual);
        visual.update(|window, cx| {
            editor.update(cx, |app, cx| app.open_file(path.clone(), window, cx));
            window.draw(cx).clear(cx);
        });
        crate::extensions::lsp_tests::publish(&editor, &mut manager, visual);
        visual.update(|_, cx| {
            editor.read(cx).language_servers["fixture"]
                .prepare_until_ready()
                .unwrap()
        });
        // Visit non-name text in its own frame before entering an uncached semantic position.
        visual.update(|window, cx| {
            editor.read(cx).editor.clone().update(cx, |state, cx| {
                state.set_selected_range(0..0, cx);
                cx.notify();
            });
            window.draw(cx).clear(cx);
        });
        visual.run_until_parked();
        let offset = source.find("<item>").unwrap() + 2;
        visual.update(|window, cx| {
            editor.read(cx).editor.clone().update(cx, |state, cx| {
                state.set_selected_range(offset..offset, cx);
                state.focus(window, cx);
                cx.notify();
            });
            window.draw(cx).clear(cx);
        });
        visual.simulate_input("X");
        assert!(
            visual.update(|_, cx| editor
                .read(cx)
                .language_edits
                .bridge
                .as_ref()
                .unwrap()
                .read(cx)
                .has_pending()),
            "fixture must start with a genuinely queued native input"
        );
        // One native batch is deliberate: draining between commands would hide the close race.
        visual.update(|window, cx| {
            window.dispatch_keystroke(Keystroke::parse("ctrl-z").unwrap(), cx);
            if composing {
                let bridge = editor.read(cx).language_edits.bridge.clone().unwrap();
                bridge.update(cx, |bridge, cx| {
                    bridge.replace_and_mark_text_in_range(None, "中文🙂", Some(0..2), window, cx);
                });
            } else {
                window.dispatch_keystroke(Keystroke::parse("ctrl-y").unwrap(), cx);
                window.dispatch_keystroke(Keystroke::parse("Y").unwrap(), cx);
            }
            host.update(cx, |host, cx| {
                host.request_close(CloseIntent::Home, window, cx);
            });
        });
        visual.run_until_parked();
        let entered = source.replacen(
            "item",
            if composing {
                "i中文🙂tem"
            } else {
                "iXYtem"
            },
            1,
        );
        assert_eq!(
            visual.update(|_, cx| editor.read(cx).editor.read(cx).value().to_string()),
            entered
        );
        if composing {
            assert!(
                visual.update(|window, cx| editor
                    .read(cx)
                    .editor
                    .clone()
                    .update(cx, |state, cx| {
                        state.marked_text_range(window, cx).is_none()
                    })),
                "close consent must follow an actual preedit commit, not merely a late Change event"
            );
        }
        click(visual, "project-close-save");
        assert!(
            visual.debug_bounds("project-open-folder").is_some(),
            "the first save choice must close after the whole FIFO; error={:?}",
            visual.update(|_, cx| host.read(cx).error.clone())
        );
        assert_eq!(std::fs::read_to_string(path).unwrap(), entered);
        // The recorded UI adapter does not own this actual service manager; retire its native lease explicitly.
        manager.shutdown_checked().unwrap();
    });
}
