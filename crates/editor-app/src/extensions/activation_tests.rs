//! Real package metadata, native menus and visibility exercise cold view ownership end to end.
use super::*;
use gpui_kit::{TestAppContext, VisualTestContext, gpui};

/// Use the production canvas inside native overflow clipping; no demand is simulated by the harness.
struct ClippedProbe {
    panel: Entity<ExtensionPanel>,
    viewport_offset: f32,
    body_offset: f32,
}

impl Render for ClippedProbe {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let probe = self
            .panel
            .update(cx, |panel, cx| panel.view_demand_probe(cx));
        div().relative().size_full().child(
            div()
                .absolute()
                .left(px(self.viewport_offset))
                .top(px(0.))
                .w(px(80.))
                .h(px(80.))
                .overflow_hidden()
                .debug_selector(|| "cold-probe-viewport".into())
                .child(
                    div()
                        .absolute()
                        .left(px(self.body_offset))
                        .top(px(0.))
                        .w(px(80.))
                        .h(px(80.))
                        .debug_selector(|| "cold-probe-body".into())
                        .child(probe),
                ),
        )
    }
}

/// Native layout flags alone do not wake a body clipped by a container or outside the window.
/// This temporary surface shares the real worker, but never joins the editor's dock or invokes show.
fn assert_clipped_stays_cold(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    manager: &mut plugin_runtime::Manager,
) {
    let panel = visual.update(|_, cx| {
        let owner = app.read(cx).extensions.read(cx);
        let entry = owner
            .entries
            .iter()
            .find(|entry| entry.manifest.id == "native-cold-view")
            .unwrap();
        let descriptor = entry
            .manifest
            .panels
            .iter()
            .find(|panel| panel.id == "welcome")
            .unwrap()
            .clone();
        let workspace = owner.workspace.clone();
        let root = owner.root.clone();
        let worker = owner.worker.clone();
        let entries = owner.entries.clone();
        let views = owner.views.clone();
        cx.new(|cx| {
            ExtensionPanel::viewer_parts(
                app.downgrade(),
                workspace,
                root,
                worker,
                entries,
                views,
                "native-cold-view".into(),
                descriptor,
                true,
                cx,
            )
        })
    });
    let mut surface = None;
    let window = visual.update(|_, cx| {
        cx.open_window(Default::default(), |_, cx| {
            let view = cx.new(|_| ClippedProbe {
                panel: panel.clone(),
                viewport_offset: 0.,
                body_offset: 100.,
            });
            surface = Some(view.clone());
            view
        })
        .unwrap()
    });
    let mut clipped = VisualTestContext::from_window(window.into(), &visual.cx);
    clipped.simulate_resize(size(px(400.), px(300.)));
    for (viewport_offset, body_offset) in [(0., 100.), (2000., 0.)] {
        clipped.update(|_, cx| {
            surface.as_ref().unwrap().update(cx, |surface, cx| {
                surface.viewport_offset = viewport_offset;
                surface.body_offset = body_offset;
                cx.notify();
            })
        });
        clipped.update(|window, cx| window.draw(cx).clear(cx));
        clipped.run_until_parked();
        let body = clipped.debug_bounds("cold-probe-body").unwrap();
        assert!(
            body.size.width > px(0.) && body.size.height > px(0.),
            "the negative control must retain a real nonzero native layout"
        );
        composable_tests::pump(manager, app, visual);
        assert!(
            manager.live.is_empty(),
            "clipped or offscreen paint cannot run Main"
        );
    }
    clipped.update(|window, _| window.remove_window());
    drop(surface);
    drop(panel);
}

/// Retain the actual independent SDK component; only its visible contribution declarations vary.
fn hidden_package() -> Package {
    // Preserve the original reviewed SDK bytes while keeping them outside host distribution.
    let package = crate::test_packages::package("capability-example-0.19.0");
    let mut files =
        package_ui_test_support::identify(package, "native-cold-view", "Cold view").files;
    let mut manifest: serde_json::Value = serde_json::from_slice(&files["manifest.json"]).unwrap();
    for panel in manifest["panels"].as_array_mut().unwrap() {
        panel["default_visible"] = serde_json::json!(false);
    }
    // A static native menu is discoverable before Main and uses the same existing business command.
    let check = manifest["commands"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|command| command["id"] == "check-errors")
        .unwrap();
    check["menus"] = serde_json::json!([
        {"location":"editor","group":"cold","order":1},
        {"location":"explorer","group":"cold","order":1}
    ]);
    files.insert(
        "manifest.json".into(),
        serde_json::to_vec(&manifest).unwrap(),
    );
    language_tests::packages::repack(files).unwrap()
}

/// Observe the original native transport without replacing or synthesizing any guest notification.
/// Requeue every work item unchanged so the standard harness remains its sole consumer.
fn opened(visual: &mut VisualTestContext, app: &Entity<EditorApp>) -> Vec<u64> {
    visual.update(|_, cx| {
        let worker = app.read(cx).extensions.read(cx).worker.clone();
        let work = worker
            .recorded
            .lock()
            .unwrap()
            .try_iter()
            .collect::<Vec<_>>();
        let epochs = work
            .iter()
            .filter_map(|work| match work {
                Work::Event(plugin, epoch, Some(panel), PluginEvent::Command { id, .. })
                    if plugin == "native-cold-view"
                        && panel == "welcome"
                        && id == "panel.opened" =>
                {
                    Some(*epoch)
                }
                _ => None,
            })
            .collect();
        for work in work {
            worker.tx.send(work).unwrap();
        }
        epochs
    })
}

/// Opening metadata and clipped paint stay cold; hiding cancels an original wait. Actual visible
/// paint and explicit show report Opened exactly once for each independently built guest's owner.
#[gpui::test]
#[ignore = "opt-in frozen Package/WASM fixtures; see crates/plugin-runtime/tests/fixtures/packages/README.md"]
fn native_cold_menu_and_view_wait_keep_exact_owner_without_eager_birth(cx: &mut TestAppContext) {
    crate::tests::with_shortcut_editor(cx, false, vec![], |visual, app, path| {
        let runtime = tempfile::tempdir().unwrap();
        let mut manager = plugin_runtime::Manager::open(
            runtime.path().into(),
            protocol::Environment {
                workspace: path.parent().unwrap().display().to_string(),
                ..Default::default()
            },
        )
        .unwrap();
        let package = hidden_package();
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        let mut renderer = images::VectorRenderer::default();
        composable_tests::publish(&mut manager, &mut renderer, &app, visual);
        assert!(manager.live.is_empty());
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                // The ordinary file menu has no comparison-session prerequisite. The readonly
                // comparison-pane popup is deliberately retired when no comparison is mounted.
                app.open_explorer_menu(
                    Some(path.to_path_buf()),
                    point(px(150.), px(150.)),
                    window,
                    cx,
                );
            })
        });
        visual.run_until_parked();
        visual.update(|window, cx| window.draw(cx).clear(cx));
        assert!(
            visual
                .debug_bounds("plugin-menu-native-cold-view/check-errors")
                .is_some(),
            "cold static menu must render"
        );
        visual.simulate_keystrokes("escape");
        composable_tests::pump(&mut manager, &app, visual);
        assert!(
            manager.live.is_empty(),
            "browsing declared commands cannot run Main"
        );
        assert_clipped_stays_cold(visual, &app, &mut manager);
        let panel =
            visual.update(|_, cx| app.read(cx).plugin_panels["native-cold-view/welcome"].clone());
        visual.update(|window, cx| {
            panel.update(cx, |panel, cx| {
                panel.show(window, cx);
                panel.hide();
                cx.notify();
            })
        });
        visual.run_until_parked();
        composable_tests::pump(&mut manager, &app, visual);
        assert!(
            manager.live.is_empty(),
            "hide must cancel the original wait before preparation"
        );
        // Restoring visibility is metadata only; a real native body paint is the positive control.
        visual.update(|_, cx| panel.update(cx, |panel, _| panel.visible.set(true)));
        composable_tests::publish(&mut manager, &mut renderer, &app, visual);
        composable_tests::pump(&mut manager, &app, visual);
        let first_owner = manager.instance_id("native-cold-view").unwrap().to_owned();
        composable_tests::publish(&mut manager, &mut renderer, &app, visual);
        assert!(
            visual.debug_bounds("plugin-ui-welcome-text").is_some(),
            "actual guest body must render"
        );
        let first_epoch = visual.update(|_, cx| {
            app.read(cx)
                .extensions
                .read(cx)
                .worker
                .state
                .lock()
                .unwrap()
                .owner_epoch("native-cold-view", &first_owner)
                .unwrap()
        });
        assert_eq!(opened(visual, &app), [first_epoch]);
        composable_tests::pump(&mut manager, &app, visual);
        visual.update(|window, cx| {
            panel.update(cx, |panel, cx| {
                panel.hide();
                panel.show(window, cx);
                cx.notify();
            })
        });
        composable_tests::pump(&mut manager, &app, visual);
        composable_tests::publish(&mut manager, &mut renderer, &app, visual);
        assert!(
            opened(visual, &app).is_empty(),
            "same exact owner cannot receive duplicate Opened"
        );
        assert_eq!(
            manager.instance_id("native-cold-view"),
            Some(first_owner.as_str())
        );
        manager.disable("native-cold-view").unwrap();
        composable_tests::publish(&mut manager, &mut renderer, &app, visual);
        manager.enable("native-cold-view").unwrap();
        composable_tests::publish(&mut manager, &mut renderer, &app, visual);
        let replacement =
            visual.update(|_, cx| app.read(cx).plugin_panels["native-cold-view/welcome"].clone());
        visual.update(|window, cx| {
            replacement.update(cx, |panel, cx| {
                panel.show(window, cx);
                cx.notify();
            })
        });
        composable_tests::pump(&mut manager, &app, visual);
        let next_owner = manager.instance_id("native-cold-view").unwrap().to_owned();
        assert_ne!(
            next_owner, first_owner,
            "disable/enable must produce a new exact owner"
        );
        composable_tests::publish(&mut manager, &mut renderer, &app, visual);
        let next_epoch = visual.update(|_, cx| {
            app.read(cx)
                .extensions
                .read(cx)
                .worker
                .state
                .lock()
                .unwrap()
                .owner_epoch("native-cold-view", &next_owner)
                .unwrap()
        });
        assert_eq!(opened(visual, &app), [next_epoch]);
        assert!(next_epoch > first_epoch);
        composable_tests::pump(&mut manager, &app, visual);
    });
}
