//! Ambient native observations belong to a live incarnation, never to cold plugin metadata.
use super::*;
use gpui_kit::{TestAppContext, VisualTestContext, gpui};

const PLUGIN: &str = "cold-observation.fixture";
const PANEL: &str = "ambient";

/// Mount the production surface beside a native focus peer without consuming its declared view.
/// Hidden view intent keeps ordinary paint from requesting Main while ambient rendering still runs.
struct ObservationWindow {
    panel: Entity<ExtensionPanel>,
    peer: FocusHandle,
}

impl Render for ObservationWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus = self.panel.read(cx).focus.clone();
        div()
            .size_full()
            .child(
                div()
                    .h(px(120.))
                    .track_focus(&focus)
                    .child(self.panel.clone()),
            )
            .child(div().h(px(80.)).track_focus(&self.peer))
    }
}

/// Publish enabled static metadata through the existing recording transport; no runtime is born.
fn surface(app: &Entity<EditorApp>, cx: &mut App) -> Entity<ExtensionPanel> {
    let manifest: protocol::Manifest = serde_json::from_value(serde_json::json!({
        "id":PLUGIN,"name":"Cold observation","version":"1.0.0","protocol":7,
        "permissions":["ui.panels"],"storage_limit":1024,
        "api":{"base":"^1","required":{"ui.native":">=1.1,<2","ui.windows":"^1"}},
        "panels":[{"id":PANEL,"title":"Ambient","position":"bottom","default_visible":false}]
    }))
    .unwrap();
    let descriptor = manifest.panels[0].clone();
    let entries = vec![Installed {
        grants: manifest.permissions.clone(),
        manifest,
        digest: "cold-observation".into(),
        enabled: true,
        project_enabled: Default::default(),
        replaced_by: None,
        global_enabled: None,
        retired_ui_contract: false,
        error: None,
    }];
    let owner = app.read(cx).extensions.read(cx);
    let workspace = owner.workspace.clone();
    let root = owner.root.clone();
    let worker = owner.worker.clone();
    worker.state.lock().unwrap().entries = entries.clone();
    cx.new(|cx| {
        ExtensionPanel::viewer_parts(
            app.downgrade(),
            workspace,
            root,
            worker,
            entries,
            Default::default(),
            PLUGIN.into(),
            descriptor,
            false,
            cx,
        )
    })
}

/// Observe the original work items before actor dispatch, including any accidental activation.
#[derive(Debug, Default)]
struct Observations {
    themes: Vec<(u64, Option<String>, protocol::Environment)>,
    focus: Vec<(u64, Option<String>, bool)>,
    demands: usize,
}

fn take_observations(worker: &Worker) -> Observations {
    let mut observations = Observations::default();
    for work in worker.recorded.lock().unwrap().try_iter() {
        match work {
            Work::Event(plugin, epoch, panel, PluginEvent::Theme(environment))
                if plugin == PLUGIN =>
            {
                observations.themes.push((epoch, panel, environment));
            }
            Work::Event(plugin, epoch, panel, PluginEvent::Focus(focused)) if plugin == PLUGIN => {
                observations.focus.push((epoch, panel, focused));
            }
            Work::Demand { plugin, .. } if plugin == PLUGIN => observations.demands += 1,
            _ => {}
        }
    }
    observations
}

/// Actual GPUI focus effects must settle before inspecting the shared worker transport.
fn draw(visual: &mut VisualTestContext) {
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
    visual.run_until_parked();
}

/// Cold render/focus stays inert. Each live owner gets one current theme even when its predecessor
/// had the same theme, while retained real focus callbacks cannot borrow a replacement's epoch.
#[gpui::test]
fn cold_native_observations_follow_live_incarnations_without_starting_a_guest(
    cx: &mut TestAppContext,
) {
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, _| {
        let panel = visual.update(|_, cx| surface(&app, cx));
        let worker = visual.update(|_, cx| panel.read(cx).worker.clone());
        let expected_environment =
            visual.update(|_, cx| environment(&panel.read(cx).workspace, cx));
        // Editor startup work predates this surface. From now on every observation is retained.
        let _ = take_observations(&worker);
        let mut peer = None;
        let handle = visual.update(|_, cx| {
            cx.open_window(Default::default(), |_, cx| {
                cx.new(|cx| {
                    let focus = cx.focus_handle();
                    peer = Some(focus.clone());
                    ObservationWindow {
                        panel: panel.clone(),
                        peer: focus,
                    }
                })
            })
            .unwrap()
        });
        let peer = peer.unwrap();
        let mut native = VisualTestContext::from_window(handle.into(), &visual.cx);
        native.simulate_resize(size(px(400.), px(240.)));
        // GPUI deliberately reports empty focus paths for inactive windows. Activate this native
        // test window before moving either handle, as an OS-activated surface would be.
        native.update(|window, _| window.activate_window());
        draw(&mut native);
        assert!(native.update(|window, _| window.is_window_active()));
        native.update(|window, cx| peer.focus(window, cx));
        draw(&mut native);
        native.update(|window, cx| panel.read(cx).focus_handle(cx).focus(window, cx));
        draw(&mut native);
        assert!(native.update(|window, cx| panel.read(cx).focus_handle(cx).is_focused(window)));
        native.update(|window, cx| peer.focus(window, cx));
        draw(&mut native);
        assert!(native.update(|window, _| peer.is_focused(window)));
        // The manager's non-active route has no local surface epoch and must also stay cold.
        native.update(|_, cx| {
            let owner = app.read(cx).extensions.read(cx);
            owner.send_to(
                PLUGIN,
                None,
                PluginEvent::Theme(expected_environment.clone()),
            );
            owner.send_to(PLUGIN, Some(PANEL.into()), PluginEvent::Focus(true));
            owner.send_to(PLUGIN, Some(PANEL.into()), PluginEvent::Focus(false));
        });
        let cold = take_observations(&worker);
        let cold_cached = native.update(|_, cx| panel.read(cx).last_theme.is_some());

        // Retain the actual old subscriptions across publication, as an already queued native
        // callback can outlive retirement. Their original epoch must remain their only authority.
        let _cold_callbacks = native
            .update(|_, cx| panel.update(cx, |panel, _| std::mem::take(&mut panel._focus_events)));
        native.update(|_, cx| {
            worker
                .state
                .lock()
                .unwrap()
                .instance_epochs
                .insert(PLUGIN.into(), 41);
            panel.update(cx, |panel, cx| panel.poll(cx));
        });
        draw(&mut native);
        draw(&mut native);
        native.update(|window, cx| panel.read(cx).focus_handle(cx).focus(window, cx));
        draw(&mut native);
        native.update(|window, cx| peer.focus(window, cx));
        draw(&mut native);
        let first = take_observations(&worker);

        let _first_callbacks = native
            .update(|_, cx| panel.update(cx, |panel, _| std::mem::take(&mut panel._focus_events)));
        let replacement_cached = native.update(|_, cx| {
            worker
                .state
                .lock()
                .unwrap()
                .instance_epochs
                .insert(PLUGIN.into(), 42);
            panel.update(cx, |panel, cx| {
                panel.poll(cx);
                // Observe retirement before update exits: GPUI can draw the replacement as soon
                // as the notification is flushed, legitimately caching its newly delivered theme.
                panel.last_theme.is_some()
            })
        });
        draw(&mut native);
        draw(&mut native);
        native.update(|window, cx| panel.read(cx).focus_handle(cx).focus(window, cx));
        draw(&mut native);
        native.update(|window, cx| peer.focus(window, cx));
        draw(&mut native);
        let replacement = take_observations(&worker);
        native.update(|window, _| window.remove_window());

        // Keep all phases in the failure so a single red run distinguishes cold delivery from a
        // lost first-live or replacement theme, without stopping before the later lifecycle edges.
        assert!(
            cold.themes.is_empty()
                && cold.focus.is_empty()
                && cold.demands == 0
                && !cold_cached
                && first.themes == vec![(41, None, expected_environment.clone())]
                && first.focus
                    == vec![
                        (41, Some(PANEL.into()), true),
                        (41, Some(PANEL.into()), false)
                    ]
                && first.demands == 0
                && !replacement_cached
                && replacement.themes == vec![(42, None, expected_environment)]
                && replacement.focus
                    == vec![
                        (42, Some(PANEL.into()), true),
                        (42, Some(PANEL.into()), false)
                    ]
                && replacement.demands == 0,
            "cold cache={cold_cached}, themes={}, focus={:?}, demands={}; first themes={}, focus={:?}, demands={}; replacement cache={replacement_cached}, themes={}, focus={:?}, demands={}",
            cold.themes.len(),
            cold.focus,
            cold.demands,
            first.themes.len(),
            first.focus,
            first.demands,
            replacement.themes.len(),
            replacement.focus,
            replacement.demands,
        );
    });
}
