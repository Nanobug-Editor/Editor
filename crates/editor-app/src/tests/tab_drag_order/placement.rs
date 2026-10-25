//! Presentation changes reuse the same visible documents, native editor and lifted drag behavior.

use super::*;
use app::editor_preferences::TabPlacement;

/// Apply the production preference effect inside the fixture's real owning window.
fn place(fixture: &Fixture, ui: &mut VisualTestContext, placement: TabPlacement) {
    ui.update(|window, cx| {
        fixture
            .app
            .update(cx, |app, cx| app.apply_tab_placement(placement, window, cx));
    });
    draw(ui);
}

/// Every editor edge uses the expected axis without shrinking, replacing or editing its document.
#[gpui::test]
fn tab_placement_edges_and_tool_window_preserve_document(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx, true);
    let original = ids(&fixture, ui);
    let editor = ui.update(|_, cx| fixture.app.read(cx).editor.clone());
    for placement in [
        TabPlacement::Top,
        TabPlacement::Bottom,
        TabPlacement::Left,
        TabPlacement::Right,
        TabPlacement::ToolWindow,
        TabPlacement::Top,
    ] {
        place(&fixture, ui, placement);
        let strip = ui.debug_bounds("editor-tabs-container").unwrap();
        let body = ui.debug_bounds("editor-tab-body").unwrap();
        let first = ui.debug_bounds("editor-tab-0").unwrap();
        let second = ui.debug_bounds("editor-tab-1").unwrap();
        match placement {
            TabPlacement::Top => assert!(strip.bottom() <= body.top() + px(1.)),
            TabPlacement::Bottom => assert!(strip.top() >= body.bottom() - px(1.)),
            TabPlacement::Left => assert!(strip.right() <= body.left() + px(1.)),
            TabPlacement::Right => assert!(strip.left() >= body.right() - px(1.)),
            TabPlacement::ToolWindow => {
                assert!(ui.debug_bounds("open-files-panel").is_some());
                assert!(
                    strip.right() <= body.left() + px(1.)
                        || strip.bottom() <= body.top() + px(1.)
                        || strip.top() >= body.bottom() - px(1.)
                );
            }
        }
        if matches!(placement, TabPlacement::Top | TabPlacement::Bottom) {
            assert!(second.left() >= first.right() - px(1.));
            assert!((first.top() - second.top()).abs() < px(1.));
        } else {
            assert!(second.top() >= first.bottom() - px(1.));
            assert!((first.left() - second.left()).abs() < px(1.));
            assert_eq!(first.size.height, px(PANEL_HEADER_HEIGHT));
        }
        assert!(body.size.width > px(100.) && body.size.height > px(100.));
        assert_eq!(ids(&fixture, ui), original);
        ui.update(|_, cx| {
            let app = fixture.app.read(cx);
            assert_eq!(app.editor, editor);
            assert!(app.tabs[2].preview);
            assert_eq!(editor.read(cx).text().to_string(), "third");
        });
    }
}

/// Each vertical presentation commits a pointer reorder only on release, using the same host action.
#[gpui::test]
fn tab_placement_vertical_drag_and_switch_cancel(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx, true);
    for placement in [
        TabPlacement::Left,
        TabPlacement::Right,
        TabPlacement::ToolWindow,
    ] {
        place(&fixture, ui, placement);
        let original = ids(&fixture, ui);
        let start = ui.debug_bounds("editor-tab-title-0").unwrap().center();
        let last = ui.debug_bounds("editor-tab-2").unwrap();
        let end = point(start.x, last.bottom() + px(12.));
        start_drag(ui, start);
        ui.simulate_mouse_move(end, Some(MouseButton::Left), Default::default());
        draw(ui);
        assert_eq!(ids(&fixture, ui), original);
        assert!(ui.debug_bounds("file-tab-drag-slot").is_some());
        release(ui, end, 1);
        assert_eq!(
            ids(&fixture, ui),
            vec![original[1], original[2], original[0]]
        );
    }
    let original = ids(&fixture, ui);
    let start = ui.debug_bounds("editor-tab-title-0").unwrap().center();
    start_drag(ui, start);
    ui.update(|_, cx| assert!(fixture.app.read(cx).tabs_drag.active()));
    place(&fixture, ui, TabPlacement::Bottom);
    ui.update(|_, cx| assert!(!fixture.app.read(cx).tabs_drag.active()));
    release(ui, start, 1);
    assert_eq!(ids(&fixture, ui), original);
}

/// Hiding the dock removes only its presentation; returning to an editor edge restores every tab.
#[gpui::test]
fn tab_placement_tool_window_hide_retains_tabs(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx, false);
    let original = ids(&fixture, ui);
    place(&fixture, ui, TabPlacement::ToolWindow);
    ui.update(|window, cx| {
        fixture
            .app
            .update(cx, |app, cx| app.toggle_open_files(window, cx));
    });
    draw(ui);
    assert!(ui.debug_bounds("editor-tabs-container").is_none());
    assert_eq!(ids(&fixture, ui), original);
    place(&fixture, ui, TabPlacement::Left);
    assert!(ui.debug_bounds("editor-tabs-container").is_some());
    assert!(ui.debug_bounds("open-files-panel").is_none());
    assert_eq!(ids(&fixture, ui), original);
}

/// File-only recovery stays inside the flexible body and never mounts a retained text input handler.
#[gpui::test]
fn tab_placement_file_only_body_remains_usable_on_every_edge(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx, false);
    let path = fixture.workspace.root().join("picture.png");
    let bytes = b"\x89PNG\r\n\x1a\n\xff";
    std::fs::write(&path, bytes).unwrap();
    ui.update(|window, cx| {
        fixture
            .app
            .update(cx, |app, cx| app.open_file(path.clone(), window, cx));
    });
    for placement in [
        TabPlacement::Top,
        TabPlacement::Bottom,
        TabPlacement::Left,
        TabPlacement::Right,
        TabPlacement::ToolWindow,
    ] {
        place(&fixture, ui, placement);
        let body = ui.debug_bounds("editor-tab-body").unwrap();
        let unavailable = ui.debug_bounds("file-view-unavailable").unwrap();
        let retry = ui.debug_bounds("file-view-retry-button").unwrap();
        assert!(unavailable.size.width > px(100.) && unavailable.size.height > px(100.));
        assert!(body.contains(&retry.center()));
        assert!(ui.debug_bounds("editor-source-pane").is_none());
        ui.simulate_input("must not edit an image");
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}

/// An overflowed active file becomes visible after the new axis and viewport have been measured.
#[gpui::test]
fn tab_placement_switch_reveals_active_file_after_layout(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx, false);
    ui.update(|window, cx| {
        fixture.app.update(cx, |app, cx| {
            for index in 0..35 {
                let path = fixture
                    .workspace
                    .root()
                    .join(format!("document-{index:02}.txt"));
                std::fs::write(&path, "document").unwrap();
                app.open_file(path, window, cx);
            }
        });
    });
    for placement in [
        TabPlacement::Left,
        TabPlacement::Bottom,
        TabPlacement::ToolWindow,
        TabPlacement::Right,
    ] {
        place(&fixture, ui, placement);
        ui.update(|window, cx| {
            window.simulate_next_frame(cx);
        });
        draw(ui);
        let strip = ui.debug_bounds("editor-tabs-container").unwrap();
        let active = ui.debug_bounds("editor-tab-37").unwrap();
        assert!(
            strip.contains(&active.center()),
            "active tab is outside {placement:?}"
        );
        if !matches!(placement, TabPlacement::Top | TabPlacement::Bottom) {
            assert!(active.bottom() <= strip.bottom());
        }
    }
}

/// Switching or hiding navigation while history is open restores its real filter focus.
#[gpui::test]
fn tab_placement_history_body_keeps_its_visible_focus(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx, false);
    place(&fixture, ui, TabPlacement::ToolWindow);
    ui.update(|window, cx| {
        fixture
            .app
            .update(cx, |app, cx| app.open_workspace_history(window, cx));
    });
    draw(ui);
    for placement in [TabPlacement::Top, TabPlacement::Right] {
        place(&fixture, ui, TabPlacement::ToolWindow);
        ui.update(|window, cx| {
            fixture
                .app
                .read(cx)
                .open_files_panel
                .focus_handle(cx)
                .focus(window, cx);
        });
        place(&fixture, ui, placement);
        ui.update(|window, cx| {
            let app = fixture.app.read(cx);
            assert!(app.history_focus_handle(cx).unwrap().is_focused(window));
            assert!(!app.editor.focus_handle(cx).is_focused(window));
        });
    }
    place(&fixture, ui, TabPlacement::ToolWindow);
    ui.update(|window, cx| {
        fixture.app.update(cx, |app, cx| {
            app.open_files_panel.focus_handle(cx).focus(window, cx);
            app.toggle_open_files(window, cx);
            assert!(app.history_focus_handle(cx).unwrap().is_focused(window));
        });
    });
}
