//! Execution toolbar visibility follows live work across configuration selections and themes.

use super::*;

/// Finished history does not keep Stop visible; selecting an idle target never stops another one.
#[gpui::test]
fn stop_visibility_follows_live_sessions_in_both_themes(cx: &mut TestAppContext) {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("project");
    std::fs::create_dir_all(&workspace).unwrap();
    let (app, cx) = open_editor(cx, &workspace);
    let (active, idle) = cx.update(|_, cx| {
        app.update(cx, |app, cx| {
            let key = app.workspace_key();
            app.run_controls = crate::run::RunControls::load_with_project(
                &key,
                Some(root.path().join("local")),
                Some(workspace.clone()),
            );
            let mut ids = Vec::new();
            for name in ["Active target", "Idle target"] {
                let id = app.run_controls.generate_id(&key);
                let mut draft = crate::run::RunConfigDraft::from_config(None, id.clone());
                draft.name = name.into();
                draft.program = "fixture.exe".into();
                app.run_controls
                    .upsert(draft.to_config().unwrap(), &key)
                    .unwrap();
                ids.push(id);
            }
            app.run_controls.select(&ids[0], &key);
            cx.notify();
            (ids.remove(0), ids.remove(0))
        })
    });

    for (index, dark) in [false, true].into_iter().enumerate() {
        cx.update(|_, cx| apply_theme(builtin_theme(dark), cx));
        cx.run_until_parked();
        assert!(cx.debug_bounds("run-stop").is_none());

        // A pending launch stays cancellable before its provider publishes a program identity.
        let request = cx.update(|_, cx| {
            app.update(cx, |app, cx| {
                let request = app.run_controls.begin(&active);
                cx.notify();
                request
            })
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("run-stop").is_some());

        let mut snapshot = crate::extensions::HostRunSnapshot {
            id: 41 + index as u64,
            config: active.clone(),
            request_id: request,
            plugin: "execution-fixture".into(),
            state: plugin_runtime::ExecutionState::Running,
            provider_session: Some("program".into()),
            failure: None,
        };
        cx.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.run_controls.reconcile(&[snapshot.clone()]);
                let key = app.workspace_key();
                app.run_controls.select(&idle, &key);
                cx.notify();
            });
        });
        cx.run_until_parked();
        let stop = cx.debug_bounds("run-stop").unwrap();
        // Base does not publish a disabled accessibility flag; verify the pointer behavior itself.
        cx.simulate_click(stop.center(), Default::default());
        assert!(cx.update(|_, cx| !app.read(cx).run_controls.is_stopping(&active)));

        // Graceful stopping retains the same slot and its explicit immediate-termination action.
        snapshot.state = plugin_runtime::ExecutionState::Stopping;
        cx.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.run_controls.reconcile(&[snapshot.clone()]);
                let key = app.workspace_key();
                app.run_controls.select(&active, &key);
                cx.notify();
            });
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("run-stop").is_some());
        assert!(cx.debug_bounds("run-terminate").is_some());

        snapshot.state = plugin_runtime::ExecutionState::Exited;
        cx.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.run_controls.reconcile(&[snapshot.clone()]);
                cx.notify();
            });
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("run-stop").is_none());
        assert!(cx.debug_bounds("run-terminate").is_none());
        assert!(cx.update(|_, cx| !app.read(cx).run_controls.sessions().is_empty()));
    }
}
