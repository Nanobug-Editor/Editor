//! Real package publication verifies an independent work window shares the existing native document.
use super::git_fixture_environment::run_isolated_test;
use super::*;
use gpui_kit::{TestAppContext, VisualTestContext, gpui};
use plugin_runtime::Manager;
use serde_json::json;
mod branch_retention;
mod close_error;
mod comparisons;
mod history_preview;
mod merge;
mod shutdown;

/// Every Git write is confined to the test's temporary workspace.
fn git(root: &Path, args: &[&str]) -> bool {
    super::git_fixture_environment::reject_repository_overrides()
        .expect("fixture environment must not redirect repository-local Git state");
    super::git_fixture_environment::require_private_configuration()
        .expect("window fixture requires its isolated test child");
    std::process::Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
        .status
        .success()
}

/// Drive the existing worker publication and native request path until the owned Git query ends.
fn flush(manager: &mut Manager, app: &Entity<EditorApp>, visual: &mut VisualTestContext) {
    let mut images = images::VectorRenderer::default();
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut settled_frames = 0;
    let mut previous_scenes = BTreeMap::new();
    for _ in 0..3000 {
        // Match the production actor: receive queued native input before polling timers and
        // request completions, which may legitimately publish a newer scene revision.
        composable_tests::pump(manager, app, visual);
        manager.poll();
        composable_tests::publish(manager, &mut images, app, visual);
        // TestWindow has no wall-clock UI pulse. Advance Base/native tree reconciliation so
        // a completed lazy page is actually painted before a physical row press is asserted.
        visual.executor().advance_clock(Duration::from_millis(160));
        visual.run_until_parked();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
            window.simulate_next_frame(cx);
            // TestWindow has no platform frame loop. Draw every real work-window scene and
            // deliver its frame callbacks so native measurement can settle before a click.
            let windows = app
                .read(cx)
                .plugin_windows
                .values()
                .copied()
                .map(gpui_kit::AnyWindowHandle::from)
                .collect::<Vec<_>>();
            for handle in windows {
                handle
                    .update(cx, |_, window, cx| {
                        window.draw(cx).clear(cx);
                        window.simulate_next_frame(cx);
                    })
                    .unwrap();
            }
        });
        // Match the production actor's two document ingress streams. UI publication alone does
        // not deliver native changes back to the guest's revision-bound merge proposals.
        let (changes, events) = visual.update(|_, cx| {
            let worker = &app.read(cx).extensions.read(cx).worker;
            let mut state = worker.state.lock().unwrap();
            (
                state.document_events.take_batch(64),
                state.document_stream.take_batch(64),
            )
        });
        // Timeout evidence identifies a stream that never settles without dumping document text.
        let ingress = format!("changes={changes:?}, events={events:?}");
        let mut native_changes = false;
        match changes {
            Ok(changes) => {
                native_changes |= !changes.is_empty();
                for change in changes {
                    manager.document_changed(change);
                }
            }
            Err(error) => manager.document_events_failed(error),
        }
        match events {
            Ok(events) => {
                native_changes |= !events.is_empty();
                for event in events {
                    manager.document_event(event);
                }
            }
            Err(error) => manager.document_stream_failed(error),
        }
        let scenes = manager
            .live
            .iter()
            .flat_map(|(id, instance)| {
                instance
                    .views
                    .iter()
                    .map(move |(panel, scene)| ((id.clone(), panel.clone()), scene.revision))
            })
            .collect::<BTreeMap<_, _>>();
        // A withdrawn instance is also quiescent: disable/retirement intentionally removes all
        // controls. An active instance must expose its real serial-queue readiness instead.
        let guest_ready = manager.live.get("git").is_none_or(|instance| {
            instance
                .views
                .get("history")
                .is_some_and(|view| view.active_node("log").is_some())
        });
        settled_frames = if !native_changes
            && guest_ready
            && scenes == previous_scenes
            && manager
                .live
                .get("git")
                .is_none_or(|instance| instance.process_count() == 0)
        {
            settled_frames + 1
        } else {
            0
        };
        previous_scenes = scenes;
        // The guest's public history canvas stays disabled while its serial work or native editor
        // acknowledgements are pending. Require two further stable bridge turns, including native
        // ingress after drawing. Eight arbitrary full UI frames can span its next automatic
        // refresh even though the user already has an idle, current and actionable scene.
        if settled_frames >= 2 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Git native bridge did not settle: {ingress}; scenes={previous_scenes:?}; ready={guest_ready}; processes={:?}",
            manager
                .live
                .get("git")
                .map(|instance| instance.process_count()),
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Separate native chrome, one authoritative editor/Undo and hide-without-abort are observable behavior.
#[gpui::test]
#[ignore = "build Git-for-Nanobug candidate; set NANOBUG_GIT_CANDIDATE to its ZIP"]
fn plugin_work_window_edits_the_original_session_and_hiding_keeps_the_draft(
    cx: &mut TestAppContext,
) {
    // Preserve the native test body inside the same private Git configuration as its Manager.
    if !run_isolated_test(
        "extensions::windows_tests::plugin_work_window_edits_the_original_session_and_hiding_keeps_the_draft",
    ) {
        return;
    }
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        let root = path.parent().unwrap();
        let conflict = root.join("conflict.txt");
        assert!(git(root, &["init", "-b", "main"]));
        assert!(git(root, &["config", "core.autocrlf", "false"]));
        assert!(git(root, &["config", "user.name", "fixture"]));
        assert!(git(
            root,
            &["config", "user.email", "fixture@example.invalid"]
        ));
        std::fs::write(&conflict, "base\n").unwrap();
        // A real repository attribute exercises variable-width marker handling through the native editor.
        std::fs::write(
            root.join(".gitattributes"),
            "conflict.txt conflict-marker-size=9\n",
        )
        .unwrap();
        assert!(git(root, &["add", "--", "conflict.txt", ".gitattributes"]));
        assert!(git(root, &["commit", "-m", "base"]));
        assert!(git(root, &["switch", "-c", "feature"]));
        std::fs::write(&conflict, "local\n").unwrap();
        assert!(git(root, &["commit", "-am", "local"]));
        assert!(git(root, &["switch", "main"]));
        std::fs::write(&conflict, "incoming\n").unwrap();
        assert!(git(root, &["commit", "-am", "incoming"]));
        assert!(git(root, &["switch", "feature"]));
        assert!(!git(root, &["merge", "main"]));
        let profile = tempfile::tempdir().unwrap();
        let package = Package::read(Path::new(
            &std::env::var_os("NANOBUG_GIT_CANDIDATE").expect("candidate required"),
        ))
        .unwrap();
        let env = visual.update(|_, cx| environment(root, cx));
        let mut manager = Manager::open(profile.path().into(), env).unwrap();
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        package_ui_test_support::demand(
            &mut manager,
            "git",
            plugin_runtime::DemandReason::Command("refresh".into()),
        )
        .unwrap();
        flush(&mut manager, &app, visual);
        manager
            .invoke_command("git", "resolve-file", json!({"path":"conflict.txt"}))
            .unwrap();
        flush(&mut manager, &app, visual);
        let handle = visual
            .update(|_, cx| app.read(cx).plugin_windows.get("git/conflicts").copied())
            .expect("independent native window");
        let original = visual.update(|_, cx| {
            app.read(cx)
                .tabs
                .iter()
                .find(|tab| tab.path() == conflict.canonicalize().unwrap())
                .unwrap()
                .text
                .as_ref()
                .unwrap()
                .editor
                .clone()
        });
        let mut separate = VisualTestContext::from_window(handle.into(), &visual.cx);
        separate.update(|window, cx| window.draw(cx).clear(cx));
        {
            assert!(
                manager.live["git"].views["conflicts"]
                    .active_node("merge-block-0-left")
                    .is_some(),
                "conflict block must be parsed from the actual native result"
            );
            let button = separate
                .debug_bounds("plugin-ui-merge-block-0-left")
                .unwrap_or_else(|| {
                    let geometry = separate.update(|_, cx| {
                        let source = original.read(cx);
                        (
                            source.input_bounds(),
                            source.visible_row_range(),
                            source.range_to_bounds(&(0..0)),
                            source.range_to_bounds(&(13..13)),
                        )
                    });
                    panic!(
                        "visible block acceptance: source={geometry:?}, frame={:?}",
                        separate.debug_bounds("work-window-document"),
                    );
                });
            separate.simulate_click(button.center(), Default::default());
            flush(&mut manager, &app, visual);
            assert_eq!(
                separate.update(|_, cx| original.read(cx).value().to_string()),
                "local\n"
            );
            separate.update(|window, cx| {
                original.update(cx, |editor, cx| editor.focus(window, cx));
                window.draw(cx).clear(cx);
            });
            separate.simulate_keystrokes("ctrl-z");
            separate.run_until_parked();
            assert!(
                separate
                    .update(|_, cx| original.read(cx).value().to_string())
                    .contains("<<<<<<<")
            );
        }
        separate.update(|window, cx| {
            window.activate_window();
            original.update(cx, |editor, cx| editor.focus(window, cx));
            window.draw(cx).clear(cx)
        });
        separate.run_until_parked();
        separate.simulate_keystrokes("ctrl-a");
        separate.simulate_input("合并草稿😀\n");
        separate.run_until_parked();
        assert_eq!(
            separate.update(|_, cx| original.read(cx).value().to_string()),
            "合并草稿😀\n"
        );
        // A work-window shortcut saves its focused source even while another main-window tab is active.
        let unrelated_before = std::fs::read(path).unwrap();
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.open_file(path.canonicalize().unwrap(), window, cx);
            });
        });
        separate.update(|window, cx| {
            original.update(cx, |editor, cx| editor.focus(window, cx));
            window.draw(cx).clear(cx);
        });
        separate.simulate_keystrokes("ctrl-s");
        separate.run_until_parked();
        assert_eq!(std::fs::read_to_string(&conflict).unwrap(), "合并草稿😀\n");
        assert_eq!(std::fs::read(path).unwrap(), unrelated_before);
        // A second shortcut cannot silently approve a warning displayed in another window.
        separate.simulate_keystrokes("ctrl-a");
        separate.simulate_input("合并草稿😀\n第二次修改\n");
        separate.run_until_parked();
        std::fs::write(&conflict, "external edit\n").unwrap();
        separate.simulate_keystrokes("ctrl-s");
        separate.simulate_keystrokes("ctrl-s");
        separate.run_until_parked();
        assert_eq!(
            std::fs::read_to_string(&conflict).unwrap(),
            "external edit\n"
        );
        let confirm = separate
            .debug_bounds("window-confirm-overwrite")
            .expect("explicit work-window save confirmation");
        separate.simulate_click(confirm.center(), Default::default());
        separate.run_until_parked();
        assert_eq!(
            std::fs::read_to_string(&conflict).unwrap(),
            "合并草稿😀\n第二次修改\n"
        );
        visual.update(|_, cx| {
            let panel = app.read(cx).plugin_panels["git/conflicts"].clone();
            panel.read(cx).visible.set(false);
            app.update(cx, |app, cx| app.sync_plugin_windows(cx));
        });
        visual.run_until_parked();
        assert!(visual.update(|_, cx| app.read(cx).plugin_windows.is_empty()));
        // A later local observation refreshes proposals without reopening a window the user hid.
        manager.invoke_command("git", "refresh", json!({})).unwrap();
        flush(&mut manager, &app, visual);
        assert!(visual.update(|_, cx| app.read(cx).plugin_windows.is_empty()));
        assert_eq!(
            visual.update(|_, cx| original.read(cx).value().to_string()),
            "合并草稿😀\n第二次修改\n"
        );
        assert!(
            root.join(".git/MERGE_HEAD").exists(),
            "hiding must not abort the external merge"
        );
        manager.disable("git").unwrap();
    });
}
