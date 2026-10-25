//! A declared independent surface must not replace the invoking window's native editing focus.
use super::*;
use gpui_kit::{Focusable as _, TestAppContext, VisualTestContext, gpui};

/// Static generic panel metadata exercises the real ExtensionPanel entry without a guest, Git
/// operation or test-only host API. Its recording worker keeps a show demand from starting WASM.
fn surface(app: &Entity<EditorApp>, position: &str, cx: &mut App) -> Entity<ExtensionPanel> {
    let manifest: protocol::Manifest = serde_json::from_value(serde_json::json!({
        "id":"window-focus.fixture","name":"Window focus","version":"1.0.0","protocol":7,
        "permissions":["ui.panels"],"storage_limit":1024,
        "api":{"base":"^1","required":{"ui.native":">=1.1,<2","ui.windows":"^1"}},
        "panels":[{"id":"surface","title":"Focus surface","position":position,"default_visible":false}]
    })).unwrap();
    let descriptor = manifest.panels[0].clone();
    let entries = vec![Installed {
        install_sequence: 0,
        grants: manifest.permissions.clone(),
        manifest,
        digest: "window-focus".into(),
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
    cx.new(|cx| {
        ExtensionPanel::viewer_parts(
            app.downgrade(),
            workspace,
            root,
            worker,
            entries,
            Default::default(),
            "window-focus.fixture".into(),
            descriptor,
            false,
            cx,
        )
    })
}

/// Start with no focused target, then focus the actual original editor by a native pointer event.
/// No call to FocusHandle::focus can conceal a broken parent-window input route in this fixture.
fn pointer_focus(visual: &mut VisualTestContext, app: &Entity<EditorApp>) -> FocusHandle {
    visual.update(|window, cx| {
        window.activate_window();
        window.blur(cx);
        window.draw(cx).clear(cx);
    });
    let bounds = visual.debug_bounds("editor-panel-content").unwrap();
    visual.simulate_click(bounds.center(), Default::default());
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
        let focus = app.read(cx).editor.focus_handle(cx);
        assert!(
            focus.is_focused(window),
            "real pointer must first focus the original editor"
        );
        focus
    })
}

/// SetPanelVisibility uses show on the invoking Window before the separate native root is created.
#[gpui::test]
fn work_window_show_preserves_parent_pointer_focus(cx: &mut TestAppContext) {
    crate::tests::with_shortcut_editor(cx, false, vec![], |visual, app, _| {
        let panel = visual.update(|_, cx| surface(&app, "window", cx));
        let original = pointer_focus(visual, &app);
        visual.update(|window, cx| {
            panel.update(cx, |panel, cx| panel.show(window, cx));
            assert!(
                panel.read(cx).visible.get(),
                "show must still reveal the surface"
            );
            assert!(
                original.is_focused(window),
                "showing an independent surface stole parent focus"
            );
        });
    });
}

/// Command admission directly calls focus after demanding its panel, sharing the same public seam.
#[gpui::test]
fn work_window_direct_focus_preserves_parent_pointer_focus(cx: &mut TestAppContext) {
    crate::tests::with_shortcut_editor(cx, false, vec![], |visual, app, _| {
        let panel = visual.update(|_, cx| surface(&app, "window", cx));
        let original = pointer_focus(visual, &app);
        visual.update(|window, cx| {
            panel.update(cx, |panel, cx| panel.focus(window, cx));
            assert!(
                original.is_focused(window),
                "independent command surface focused its handle in the parent window"
            );
        });
    });
}

/// Ordinary dock surfaces belong to the invoking Window and retain their existing show/focus behavior.
#[gpui::test]
fn dock_surface_show_still_takes_parent_focus(cx: &mut TestAppContext) {
    crate::tests::with_shortcut_editor(cx, false, vec![], |visual, app, _| {
        let panel = visual.update(|_, cx| surface(&app, "bottom", cx));
        let original = pointer_focus(visual, &app);
        visual.update(|window, cx| {
            panel.update(cx, |panel, cx| panel.show(window, cx));
            assert!(panel.read(cx).visible.get());
            assert!(
                panel.read(cx).focus_handle(cx).is_focused(window),
                "dock show must still focus its owning surface"
            );
            assert!(
                !original.is_focused(window),
                "positive control did not move native focus"
            );
        });
    });
}
