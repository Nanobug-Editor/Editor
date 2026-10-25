//! Window toolbar acceptance uses original controls, isolated profiles and real pointer gestures.

use crate::{EditorApp, SessionState, theme, typography};
use editor_core::Workspace;
use gpui_base::Root;
use gpui_kit::{
    AppContext as _, Entity, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    Point, TestAppContext, VisualTestContext, gpui, point, px, size,
};
use std::{path::Path, time::Duration};

#[cfg(windows)]
mod native;

/// New workspaces reserve these four host buttons before installed plugin windows.
const DEFAULT_ORDER: [&str; 4] = [
    "explorer-panel-toggle",
    "outline-toggle",
    "host-messages-toggle",
    "terminal-toggle",
];

/// Create the production shell under a disposable workspace and explicit private profile.
fn with_toolbar(
    cx: &mut TestAppContext,
    scenario: impl FnOnce(&mut VisualTestContext, Entity<EditorApp>, &Path, &Path),
) {
    let profile = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        theme::apply_theme(theme::builtin_theme(false), cx);
        crate::app::shortcuts::bootstrap::configure(cx, profile.path().join("shortcuts.json"))
            .unwrap();
        editor_ui::init(cx);
        cx.set_reduce_motion(true);
    });
    let workspace = Workspace::open(project.path()).unwrap();
    let root = workspace.root().to_path_buf();
    let session = SessionState::load_in_profile(&root, profile.path());
    let mut app = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let editor =
            cx.new(|cx| EditorApp::new_with_session(workspace, None, Some(session), window, cx));
        app = Some(editor.clone());
        Root::new(editor, window, cx)
    });
    visual.simulate_resize(size(px(1100.), px(800.)));
    // TestWindow starts inactive; native long-press admission intentionally rejects inactive windows.
    visual.update(|window, _| window.activate_window());
    draw(visual);
    scenario(visual, app.unwrap(), &root, profile.path());
}

/// Flush deferred native callbacks before examining the current layout.
fn draw(visual: &mut VisualTestContext) {
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
}

/// Read the actual horizontal positions rather than the implementation's ordering function.
fn order(visual: &mut VisualTestContext) -> Vec<&'static str> {
    let mut buttons = DEFAULT_ORDER.map(|id| {
        let bounds = visual
            .debug_bounds(id)
            .unwrap_or_else(|| panic!("missing {id}"));
        (id, bounds.left())
    });
    buttons.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    buttons.into_iter().map(|(id, _)| id).collect()
}

/// Start a left press and advance the executor beyond the public long-press threshold.
fn hold(visual: &mut VisualTestContext, id: &'static str) -> Point<Pixels> {
    let position = visual.debug_bounds(id).unwrap().center();
    visual.simulate_event(MouseDownEvent {
        position,
        button: MouseButton::Left,
        modifiers: Default::default(),
        click_count: 1,
        first_mouse: false,
    });
    visual.run_until_parked();
    visual.executor().advance_clock(Duration::from_millis(450));
    draw(visual);
    move_held(visual, position + point(px(8.), px(0.)));
    assert!(
        visual.update(|_, cx| cx.has_active_drag()),
        "held movement must start the native drag"
    );
    position
}

/// Preserve the held-button flag when moving across valid and invalid drop regions.
fn move_held(visual: &mut VisualTestContext, position: Point<Pixels>) {
    visual.simulate_event(MouseMoveEvent {
        position,
        pressed_button: Some(MouseButton::Left),
        modifiers: Default::default(),
    });
    draw(visual);
}

/// Complete the same press without invoking an application action directly.
fn release(visual: &mut VisualTestContext, position: Point<Pixels>) {
    visual.simulate_event(MouseUpEvent {
        position,
        button: MouseButton::Left,
        modifiers: Default::default(),
        click_count: 1,
    });
    draw(visual);
}

/// The first three controls match the product requirement and a short click retains its action.
#[gpui::test]
fn window_toolbar_defaults_and_short_click(cx: &mut TestAppContext) {
    with_toolbar(cx, |visual, _, _, _| {
        assert_eq!(order(visual), DEFAULT_ORDER);
        assert!(visual.debug_bounds("host-messages-panel").is_none());
        let bounds = visual.debug_bounds("host-messages-toggle").unwrap();
        visual.simulate_click(bounds.center(), Default::default());
        draw(visual);
        assert!(visual.debug_bounds("host-messages-panel").is_some());
        assert_eq!(order(visual), DEFAULT_ORDER);
    });
}

/// A real held-pointer drop persists and restores without accidentally activating its button.
#[gpui::test]
fn window_toolbar_drag_persists_and_reopens(cx: &mut TestAppContext) {
    with_toolbar(cx, |visual, app, workspace, profile| {
        let first = visual.debug_bounds("explorer-panel-toggle").unwrap();
        let target = point(first.left() + px(1.), first.center().y);
        hold(visual, "host-messages-toggle");
        move_held(visual, target);
        assert!(visual.debug_bounds("reorder-insertion").is_some());
        release(visual, target);
        let expected = [
            "host-messages-toggle",
            "explorer-panel-toggle",
            "outline-toggle",
            "terminal-toggle",
        ];
        assert_eq!(order(visual), expected);
        assert!(visual.debug_bounds("host-messages-panel").is_none());
        let saved = SessionState::load_in_profile(workspace, profile);
        assert_eq!(saved.window_button_order, expected);
        assert_eq!(
            visual.update(|_, cx| app.read(cx).session_state.window_button_order.clone()),
            expected
        );
        // A second production constructor must restore from disk, not share the first view's state.
        let workspace = Workspace::open(workspace).unwrap();
        let handle = visual.update(|_, cx| {
            cx.open_window(Default::default(), |window, cx| {
                let restored = cx.new(|cx| {
                    EditorApp::new_with_session(workspace, None, Some(saved), window, cx)
                });
                cx.new(|cx| Root::new(restored, window, cx))
            })
            .unwrap()
        });
        let mut reopened = VisualTestContext::from_window(handle.into(), &visual.cx);
        reopened.simulate_resize(size(px(1100.), px(800.)));
        draw(&mut reopened);
        assert_eq!(order(&mut reopened), expected);
        assert!(reopened.debug_bounds("host-messages-panel").is_none());
    });
}

/// Illegal drops and Escape retain both order and visibility in each palette and fractional scale.
#[gpui::test]
fn window_toolbar_invalid_drop_and_escape_preserve_state(cx: &mut TestAppContext) {
    with_toolbar(cx, |visual, _, workspace, profile| {
        for (dark, scale) in [(false, 1.), (true, 1.5)] {
            visual.update(|window, cx| {
                theme::apply_theme(theme::builtin_theme(dark), cx);
                window.refresh();
            });
            visual.simulate_scale_factor_change(scale);
            draw(visual);
            hold(visual, "host-messages-toggle");
            let invalid = point(px(700.), px(400.));
            move_held(visual, invalid);
            assert!(visual.debug_bounds("reorder-forbidden").is_some());
            release(visual, invalid);
            assert_eq!(order(visual), DEFAULT_ORDER);
            assert!(visual.debug_bounds("host-messages-panel").is_none());
            hold(visual, "host-messages-toggle");
            let first = visual.debug_bounds("explorer-panel-toggle").unwrap();
            let target = point(first.left() + px(1.), first.center().y);
            move_held(visual, target);
            visual.simulate_keystrokes("escape");
            release(visual, target);
            assert_eq!(order(visual), DEFAULT_ORDER);
            assert!(visual.debug_bounds("host-messages-panel").is_none());
            assert!(
                SessionState::load_in_profile(workspace, profile)
                    .window_button_order
                    .is_empty()
            );
        }
    });
}

/// Historical workspace JSON without this optional field retains layout and uses the new defaults.
#[test]
fn window_toolbar_legacy_session_defaults_without_rewriting_other_choices() {
    let workspace = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    let root = workspace.path().canonicalize().unwrap();
    let mut value = serde_json::to_value(SessionState::for_workspace(&root)).unwrap();
    value.as_object_mut().unwrap().remove("window_button_order");
    value["outline_visible"] = true.into();
    value["explorer_visible"] = false.into();
    std::fs::write(
        SessionState::file_in_profile(&root, profile.path()),
        serde_json::to_vec(&value).unwrap(),
    )
    .unwrap();
    let loaded = SessionState::load_in_profile(&root, profile.path());
    assert!(loaded.window_button_order.is_empty());
    assert!(loaded.outline_visible);
    assert!(!loaded.explorer_visible);
}

/// Narrow windows keep every destination available in the popup, including currently visible buttons.
#[gpui::test]
fn window_toolbar_overflow_reorders_hidden_terminal_without_activation(cx: &mut TestAppContext) {
    with_toolbar(cx, |visual, app, workspace, profile| {
        visual.simulate_resize(size(px(300.), px(800.)));
        draw(visual);
        let overflow = visual.debug_bounds("plugin-windows-overflow").unwrap();
        visual.simulate_click(overflow.center(), Default::default());
        draw(visual);
        for id in [
            "native-menu-explorer-panel-toggle",
            "native-menu-outline-toggle",
            "native-menu-host-messages-toggle",
            "native-menu-terminal-toggle",
        ] {
            assert!(
                visual.debug_bounds(id).is_some(),
                "all four windows must be reorderable"
            );
        }
        let explorer = visual
            .debug_bounds("native-menu-explorer-panel-toggle")
            .unwrap();
        let target = point(explorer.center().x, explorer.top() + px(1.));
        hold(visual, "native-menu-terminal-toggle");
        move_held(visual, target);
        assert!(visual.debug_bounds("reorder-insertion").is_some());
        release(visual, target);
        let expected = [
            "terminal-toggle",
            "explorer-panel-toggle",
            "outline-toggle",
            "host-messages-toggle",
        ];
        assert_eq!(
            SessionState::load_in_profile(workspace, profile).window_button_order,
            expected
        );
        assert!(!visual.update(|_, cx| app.read(cx).session_state.native_terminal_visible));
        // Keep this same popup open: its captured list must not override the first committed move.
        let messages = visual
            .debug_bounds("native-menu-host-messages-toggle")
            .unwrap();
        let target = point(messages.center().x, messages.bottom() - px(1.));
        hold(visual, "native-menu-outline-toggle");
        move_held(visual, target);
        release(visual, target);
        let expected = [
            "terminal-toggle",
            "explorer-panel-toggle",
            "host-messages-toggle",
            "outline-toggle",
        ];
        let saved = SessionState::load_in_profile(workspace, profile);
        assert_eq!(saved.window_button_order, expected);
        let selectors = [
            "native-menu-terminal-toggle",
            "native-menu-explorer-panel-toggle",
            "native-menu-host-messages-toggle",
            "native-menu-outline-toggle",
        ];
        let positions = selectors.map(|id| visual.debug_bounds(id).unwrap().top());
        assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(!visual.update(|_, cx| app.read(cx).session_state.outline_visible));
        visual.simulate_keystrokes("escape");
        visual.simulate_resize(size(px(1100.), px(800.)));
        draw(visual);
        assert_eq!(order(visual), expected);
        let workspace = Workspace::open(workspace).unwrap();
        let handle = visual.update(|_, cx| {
            cx.open_window(Default::default(), |window, cx| {
                let restored = cx.new(|cx| {
                    EditorApp::new_with_session(workspace, None, Some(saved), window, cx)
                });
                cx.new(|cx| Root::new(restored, window, cx))
            })
            .unwrap()
        });
        let mut reopened = VisualTestContext::from_window(handle.into(), &visual.cx);
        reopened.simulate_resize(size(px(1100.), px(800.)));
        draw(&mut reopened);
        assert_eq!(order(&mut reopened), expected);
    });
}
