//! Exercise routing and cancellation through real native project windows in one GPUI application.
use super::*;
use gpui_kit::VisualContext as _;

/// Opening B from A must preserve A's editor; aliases activate an existing owner without another session.
#[gpui::test]
fn project_windows_open_new_project_and_reuse_the_same_directory(cx: &mut TestAppContext) {
    let profile = tempfile::tempdir().unwrap();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    std::fs::write(a.path().join("a.txt"), "A original").unwrap();
    std::fs::write(b.path().join("b.txt"), "B original").unwrap();
    with_home(cx, profile.path(), |visual, host| {
        visual.update(|window, cx| {
            host.update(cx, |host, cx| {
                host.open_path(a.path().join("a.txt"), window, cx)
            })
        });
        visual.run_until_parked();
        click(visual, "project-trust-restricted");
        let original = visual.update(|_, cx| host.read(cx).editor.clone().unwrap());
        visual.update(|window, cx| {
            original.read(cx).editor.clone().update(cx, |editor, cx| {
                editor.set_value("A unsaved", window, cx);
                cx.emit(gpui_base::input::InputEvent::Change);
            });
        });
        visual.run_until_parked();
        click(visual, "project-open-another");
        visual.simulate_path_prompt_response(|_| None);
        visual.run_until_parked();
        assert_eq!(visual.update(|_, cx| cx.windows().len()), 1);
        click(visual, "project-open-another");
        let selected = b.path().to_owned();
        visual.simulate_path_prompt_response(move |_| Some(vec![selected]));
        visual.run_until_parked();
        let windows = visual.update(|_, cx| cx.windows());
        assert_eq!(
            windows.len(),
            2,
            "a different project must create a second native window"
        );
        assert_eq!(
            visual.update(|_, cx| host.read(cx).editor.clone()),
            Some(original.clone())
        );
        assert_eq!(
            visual.update(|_, cx| original.read(cx).editor.read(cx).value().to_string()),
            "A unsaved"
        );
        visual.update(|window, cx| {
            host.update(cx, |host, cx| {
                host.open_path(a.path().join("."), window, cx)
            })
        });
        visual.run_until_parked();
        assert_eq!(
            visual.update(|_, cx| cx.windows().len()),
            2,
            "the same real directory must be activated"
        );
        let second_window = windows
            .into_iter()
            .find(|handle| handle.window_id() != visual.window_handle().window_id())
            .unwrap();
        let mut second = VisualTestContext::from_window(second_window, &visual.cx);
        second.run_until_parked();
        click(&mut second, "project-trust-restricted");
        assert!(second.debug_bounds("editor-shell").is_some());
        second.update(|window, cx| {
            let directory =
                platform_windows::directory_identity::DirectoryIdentity::open(b.path()).unwrap();
            super::super::controller::owner(&directory, cx)
                .unwrap()
                .host
                .update(cx, |host, cx| {
                    host.editor.as_ref().unwrap().update(cx, |app, cx| {
                        app.open_file(b.path().join("b.txt"), window, cx)
                    });
                })
                .unwrap();
        });
        // Home requests share one native home and the application-owned recent list.
        visual.dispatch_action(crate::app::projects::ShowHome);
        visual.run_until_parked();
        assert_eq!(visual.update(|_, cx| cx.windows().len()), 3);
        second.dispatch_action(crate::app::projects::ShowHome);
        second.run_until_parked();
        assert_eq!(second.update(|_, cx| cx.windows().len()), 3);
        let home_handle = second.update(|_, cx| {
            cx.windows()
                .into_iter()
                .find(|entry| {
                    entry.window_id() != second_window.window_id()
                        && entry.window_id() != visual.window_handle().window_id()
                })
                .unwrap()
        });
        let mut home = VisualTestContext::from_window(home_handle, &visual.cx);
        home.run_until_parked();
        assert!(
            home.debug_bounds("project-recent-0").is_some()
                && home.debug_bounds("project-recent-1").is_some()
        );
        home.dispatch_action(crate::app::projects::CloseWindow);
        home.run_until_parked();
        assert_eq!(second.update(|_, cx| cx.windows().len()), 2);
        visual.dispatch_action(crate::app::projects::CloseWindow);
        visual.run_until_parked();
        click(visual, "project-close-discard");
        assert_eq!(second.update(|_, cx| cx.windows().len()), 1);
        second.update(|window, cx| {
            let directory =
                platform_windows::directory_identity::DirectoryIdentity::open(b.path()).unwrap();
            let host = super::super::controller::owner(&directory, cx)
                .unwrap()
                .host
                .upgrade()
                .unwrap();
            host.read(cx)
                .editor
                .as_ref()
                .unwrap()
                .read(cx)
                .editor
                .clone()
                .update(cx, |editor, cx| {
                    editor.set_value("B continues after A closes", window, cx);
                    cx.emit(gpui_base::input::InputEvent::Change);
                });
        });
        second.simulate_keystrokes("ctrl-s");
        second.run_until_parked();
        assert_eq!(
            std::fs::read_to_string(b.path().join("b.txt")).unwrap(),
            "B continues after A closes"
        );
        assert_eq!(
            std::fs::read_to_string(a.path().join("a.txt")).unwrap(),
            "A original"
        );
        second.dispatch_action(crate::app::projects::CloseWindow);
        second.run_until_parked();
        assert!(
            second.cx.update(|cx| cx.windows().is_empty()),
            "the last window waits for shared retirement before exiting"
        );
    });
}

/// Quit must collect both windows' decisions before destroying either editor or stopping resources.
#[gpui::test]
fn project_windows_quit_cancel_retains_every_editor_and_revalidates_new_input(
    cx: &mut TestAppContext,
) {
    let profile = tempfile::tempdir().unwrap();
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    std::fs::write(a.path().join("a.txt"), "A disk").unwrap();
    std::fs::write(b.path().join("b.txt"), "B disk").unwrap();
    with_home(cx, profile.path(), |visual, host| {
        visual.update(|window, cx| {
            host.update(cx, |host, cx| {
                host.open_path(a.path().join("a.txt"), window, cx)
            })
        });
        visual.run_until_parked();
        click(visual, "project-trust-restricted");
        let first = visual.update(|_, cx| host.read(cx).editor.clone().unwrap());
        visual.update(|window, cx| {
            host.update(cx, |host, cx| {
                host.open_path(b.path().join("b.txt"), window, cx)
            })
        });
        visual.run_until_parked();
        let second_handle = visual.update(|window, cx| {
            cx.windows()
                .into_iter()
                .find(|entry| entry.window_id() != window.window_handle().window_id())
                .unwrap()
        });
        let mut second = VisualTestContext::from_window(second_handle, &visual.cx);
        second.run_until_parked();
        click(&mut second, "project-trust-restricted");
        let second_host = visual.update(|_, cx| {
            super::super::controller::owner(
                &platform_windows::directory_identity::DirectoryIdentity::open(b.path()).unwrap(),
                cx,
            )
            .unwrap()
            .host
            .upgrade()
            .unwrap()
        });
        let next = second.update(|_, cx| second_host.read(cx).editor.clone().unwrap());
        let edit =
            |context: &mut VisualTestContext, app: &Entity<crate::EditorApp>, value: &str| {
                context.update(|window, cx| {
                    app.read(cx).editor.clone().update(cx, |editor, cx| {
                        editor.set_value(value, window, cx);
                        cx.emit(gpui_base::input::InputEvent::Change);
                    })
                });
                context.run_until_parked();
            };
        edit(visual, &first, "A pending");
        edit(&mut second, &next, "B pending");
        // Application quit takes over an already visible local close prompt, including its cancellation route.
        click(visual, "project-close-project");
        assert!(visual.debug_bounds("project-close-discard").is_some());
        second.dispatch_action(crate::extensions::QuitEditor);
        second.run_until_parked();
        visual.run_until_parked();
        click(visual, "project-decision-cancel");
        assert!(
            visual.update(|_, cx| !super::super::controller::is_quitting(cx)),
            "cancelling the adopted prompt must cancel the whole application round"
        );
        assert_eq!(visual.update(|_, cx| cx.windows().len()), 2);
        visual.dispatch_action(crate::extensions::QuitEditor);
        visual.run_until_parked();
        click(visual, "project-close-discard");
        assert_eq!(
            visual.cx.update(|cx| cx.windows().len()),
            2,
            "collecting B's consent cannot destroy A"
        );
        second.run_until_parked();
        click(&mut second, "project-decision-cancel");
        assert_eq!(
            visual.update(|_, cx| first.read(cx).editor.read(cx).value().to_string()),
            "A pending"
        );
        assert_eq!(
            second.update(|_, cx| next.read(cx).editor.read(cx).value().to_string()),
            "B pending"
        );
        visual.dispatch_action(crate::extensions::QuitEditor);
        visual.run_until_parked();
        click(visual, "project-close-discard");
        // Input in a previously approved window invalidates that consent before all cleanup begins.
        edit(visual, &first, "A newer input");
        click(&mut second, "project-close-discard");
        assert_eq!(visual.update(|_, cx| cx.windows().len()), 2);
        assert!(visual.debug_bounds("project-close-discard").is_some());
        click(visual, "project-decision-cancel");
        assert_eq!(
            std::fs::read_to_string(a.path().join("a.txt")).unwrap(),
            "A disk"
        );
        assert_eq!(
            std::fs::read_to_string(b.path().join("b.txt")).unwrap(),
            "B disk"
        );
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt as _;
            // An actual B session-file lease rejects replacement after A has acknowledged cleanup.
            let path = second.update(|_, cx| next.read(cx).session_state.file_path().unwrap());
            let lock = std::fs::OpenOptions::new()
                .read(true)
                .share_mode(1)
                .open(&path)
                .unwrap();
            visual.dispatch_action(crate::extensions::QuitEditor);
            visual.run_until_parked();
            click(visual, "project-close-discard");
            click(&mut second, "project-close-discard");
            assert_eq!(
                visual.update(|_, cx| cx.windows().len()),
                2,
                "a late cleanup failure retains every native window"
            );
            assert!(second.debug_bounds("project-error").is_some());
            drop(lock);
        }
        // A final successful retry saves A explicitly and discards B only after both decisions have been accepted.
        visual.dispatch_action(crate::extensions::QuitEditor);
        visual.run_until_parked();
        click(visual, "project-close-save");
        click(&mut second, "project-close-discard");
        assert!(visual.cx.update(|cx| cx.windows().is_empty()));
        assert_eq!(
            std::fs::read_to_string(a.path().join("a.txt")).unwrap(),
            "A newer input"
        );
        assert_eq!(
            std::fs::read_to_string(b.path().join("b.txt")).unwrap(),
            "B disk"
        );
    });
}
