//! Project shutdown must retire independent surfaces before later shell frames can create new owners.
use super::*;

/// The real shutdown entry drains two declared work windows; subsequent native shell paints must
/// keep them retired even while their static enabled metadata and saved visibility remain present.
#[gpui::test]
fn project_shutdown_render_does_not_recreate_plugin_work_windows(cx: &mut TestAppContext) {
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, _| {
        visual.update(|_, cx| {
            let manifest: protocol::Manifest = serde_json::from_value(json!({
                "id":"shutdown-window.fixture","name":"Shutdown windows","version":"1.0.0","protocol":7,
                "permissions":["ui.panels"],"storage_limit":1024,
                "api":{"base":"^1","required":{"ui.native":">=1.1,<2","ui.windows":"^1"}},
                "panels":[
                    {"id":"details","title":"Shutdown details","position":"window","default_visible":true},
                    {"id":"conflicts","title":"Shutdown conflicts","position":"window","default_visible":true}
                ]
            }))
            .unwrap();
            // Publish ordinary installed metadata through the existing recorded actor boundary.
            // The production shell creates the panels and roots; no Git/WASM or native OS loop runs.
            let owner = app.read(cx).extensions.clone();
            owner.update(cx, |owner, cx| {
                let mut state = owner.worker.state.lock().unwrap();
                state.entries = vec![Installed {
                    grants: manifest.permissions.clone(),
                    manifest,
                    digest: "shutdown-window".into(),
                    enabled: true,
                    project_enabled: Default::default(),
                    replaced_by: None,
                    global_enabled: None,
                    retired_ui_contract: false,
                    error: None,
                }];
                drop(state);
                owner.poll(cx);
            });
        });
        visual.run_until_parked();
        visual.update(|window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        });
        visual.run_until_parked();

        let original = visual.update(|window, cx| {
            let handles = app
                .read(cx)
                .plugin_windows
                .values()
                .copied()
                .map(gpui_kit::AnyWindowHandle::from)
                .collect::<Vec<_>>();
            assert_eq!(
                handles.len(),
                2,
                "both declared independent roots must open"
            );
            let others = cx
                .windows()
                .into_iter()
                .filter(|handle| *handle != window.window_handle())
                .collect::<Vec<_>>();
            assert_eq!(
                others.len(),
                2,
                "the positive control needs two actual windows"
            );
            assert!(handles.iter().all(|handle| others.contains(handle)));
            handles
        });

        // Trigger the same production cleanup as the native child, rather than setting its
        // shutting_down flag or hiding panels. Keep its real cleanup future alive during paint.
        let cleanup = visual.update(|_, cx| {
            app.update(cx, |app, cx| {
                let cleanup = app.prepare_project_shutdown(cx).unwrap();
                assert!(
                    app.plugin_windows.is_empty(),
                    "shutdown must first drain its windows"
                );
                cleanup
            })
        });
        for _ in 0..2 {
            visual.run_until_parked();
            visual.update(|window, cx| {
                window.blur(cx);
                window.refresh();
                window.draw(cx).clear(cx);
            });
        }
        visual.run_until_parked();

        let (old_retired, remaining, recreated) = visual.update(|window, cx| {
            let old_retired = original
                .iter()
                .all(|handle| handle.update(cx, |_, _, _| ()).is_err());
            let remaining = cx
                .windows()
                .into_iter()
                .filter(|handle| *handle != window.window_handle())
                .collect::<Vec<_>>();
            let recreated = app
                .read(cx)
                .plugin_windows
                .iter()
                .map(|(key, handle)| (key.clone(), gpui_kit::AnyWindowHandle::from(*handle)))
                .collect::<Vec<_>>();
            (old_retired, remaining, recreated)
        });
        assert!(
            old_retired,
            "the original roots must finish their ordinary close path"
        );
        assert!(
            remaining.is_empty(),
            "shutdown shell paint created new native owners: original={original:?}, remaining={remaining:?}, panels={recreated:?}"
        );
        drop(cleanup);
    });
}
