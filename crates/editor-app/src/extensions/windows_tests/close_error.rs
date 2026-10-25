//! Failed plugin surfaces must remain closable through the native window's ordinary close gate.
use super::*;

/// A failed plugin publication still owns an ordinary window. Its native close callback
/// must hide the panel, preserve the parent editor and prevent a later paint from reopening it.
#[gpui::test]
fn failed_work_window_native_close_hides_the_surface_and_parent_stays_usable(
    cx: &mut TestAppContext,
) {
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, _| {
        visual.update(|_, cx| {
            let manifest: protocol::Manifest = serde_json::from_value(json!({
                "id":"close-error.fixture","name":"Failed work window","version":"1.0.0","protocol":7,
                "permissions":["ui.panels"],"storage_limit":1024,
                "api":{"base":"^1","required":{"ui.native":">=1.1,<2","ui.windows":"^1"}},
                "panels":[{"id":"details","title":"Failed details","position":"window","default_visible":true}]
            })).unwrap();
            let owner = app.read(cx).extensions.clone();
            owner.update(cx, |owner, cx| {
                let mut state = owner.worker.state.lock().unwrap();
                state.entries = vec![Installed {
                    install_sequence: 0,
                    grants: manifest.permissions.clone(), manifest, digest: "failed-window".into(),
                    enabled: true, project_enabled: Default::default(), replaced_by: None,
                    global_enabled: None, retired_ui_contract: false, error: Some("timeout".into()),
                }];
                drop(state);
                owner.poll(cx);
            });
        });
        visual.run_until_parked();
        let handle = visual.update(|window, cx| {
            window.draw(cx).clear(cx);
            let windows = &app.read(cx).plugin_windows;
            assert_eq!(
                windows.len(),
                1,
                "error fallback must own an actual work window"
            );
            *windows.get("close-error.fixture/details").unwrap()
        });
        let mut details = VisualTestContext::from_window(handle.into(), &visual.cx);
        details.update(|window, cx| window.draw(cx).clear(cx));
        assert!(
            details.simulate_close(),
            "native caption close must accept a failed surface"
        );
        // TestPlatform reports close consent without performing the OS's accepted close itself.
        details.update(|window, _| window.remove_window());
        visual.run_until_parked();
        for _ in 0..2 {
            visual.update(|window, cx| {
                window.refresh();
                window.draw(cx).clear(cx);
                assert!(
                    app.read(cx).plugin_windows.is_empty(),
                    "closed window must not reopen"
                );
                assert!(
                    !app.read(cx).plugin_panels["close-error.fixture/details"]
                        .read(cx)
                        .visible
                        .get()
                );
            });
            visual.run_until_parked();
        }
        visual.update(|window, cx| {
            app.read(cx)
                .editor
                .read(cx)
                .focus_handle(cx)
                .focus(window, cx);
            window.draw(cx).clear(cx);
            assert!(
                app.read(cx)
                    .editor
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window)
            );
        });
    });
}
