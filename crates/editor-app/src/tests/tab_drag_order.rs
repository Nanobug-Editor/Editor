//! Released file-tab drops preserve native document ownership and persist only changed visible order.

use crate::*;
use gpui_kit::{TestAppContext, VisualTestContext, gpui};

mod placement;

/// Temporary project and profile keep ordering and session assertions away from the user's data.
struct Fixture {
    /// Keep the files alive for the native watcher and document sessions.
    _directory: tempfile::TempDir,
    /// Session writes use an isolated real profile so no-op drops can be checked on disk.
    profile: tempfile::TempDir,
    /// Canonical paths shared by the native application and session loader.
    workspace: Workspace,
    /// The production document owner exercised by each released-drop scenario.
    app: Entity<EditorApp>,
}

/// Mount three real documents in `cx`, optionally leaving the last document as an explorer preview.
fn mount(cx: &mut TestAppContext, preview_last: bool) -> (Fixture, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        cx.set_reduce_motion(true);
    });
    let directory = tempfile::tempdir().unwrap();
    let profile = tempfile::tempdir().unwrap();
    for (name, content) in [("a.txt", "first"), ("b.txt", "second"), ("c.txt", "third")] {
        std::fs::write(directory.path().join(name), content).unwrap();
    }
    let workspace = Workspace::open(directory.path()).unwrap();
    let session = SessionState::load_in_profile(workspace.root(), profile.path());
    let opened = workspace.clone();
    let initial = workspace.root().join("a.txt");
    let mut application = None;
    let (_, ui) = cx.add_window_view(|window, cx| {
        let app = cx.new(|cx| {
            EditorApp::new_with_session(opened, Some(initial), Some(session), window, cx)
        });
        application = Some(app.clone());
        Root::new(app, window, cx)
    });
    let app = application.unwrap();
    ui.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.open_file(workspace.root().join("b.txt"), window, cx);
            let last = workspace.root().join("c.txt");
            if preview_last {
                app.open_preview_file(last, window, cx);
            } else {
                app.open_file(last, window, cx);
            }
        });
    });
    ui.simulate_resize(size(px(1000.), px(700.)));
    draw(ui);
    (
        Fixture {
            _directory: directory,
            profile,
            workspace,
            app,
        },
        ui,
    )
}

/// Drain subscriptions in `ui` before checking the same visible native document surface.
fn draw(ui: &mut VisualTestContext) {
    ui.run_until_parked();
    ui.update(|window, cx| window.draw(cx).clear(cx));
}

/// Resolve the fixture's current live identities in painted file order through `ui`.
fn ids(fixture: &Fixture, ui: &mut VisualTestContext) -> Vec<u64> {
    ui.update(|_, cx| {
        fixture
            .app
            .read(cx)
            .tabs
            .iter()
            .map(|tab| tab.file_id)
            .collect()
    })
}

/// Forward/backward placement and the empty tail drop use the same persisted document order.
#[gpui::test]
fn tab_drag_order_moves_both_directions_and_to_end(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx, false);
    let original = ids(&fixture, ui);
    let [a, b, c] = original.as_slice() else {
        panic!("three fixture documents must be open")
    };
    for (source, before, expected) in [
        (*c, Some(*a), vec![*c, *a, *b]),
        (*c, Some(*b), vec![*a, *c, *b]),
        (*a, None, vec![*c, *b, *a]),
    ] {
        ui.update(|_, cx| {
            fixture
                .app
                .update(cx, |app, cx| app.reorder_file_tab(source, before, cx));
        });
        assert_eq!(ids(&fixture, ui), expected);
        let persisted =
            SessionState::load_in_profile(fixture.workspace.root(), fixture.profile.path());
        ui.update(|_, cx| {
            let app = fixture.app.read(cx);
            assert_eq!(
                persisted.open_tabs,
                app.tabs
                    .iter()
                    .map(|tab| tab.path().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                app.active_path.as_deref(),
                Some(fixture.workspace.root().join("c.txt").as_path())
            );
        });
    }
}

/// No-op positions and IDs from a closed incarnation must neither reorder nor write the session.
#[gpui::test]
fn tab_drag_order_rejects_unchanged_and_stale_drops(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx, false);
    let stale = ids(&fixture, ui)[1];
    ui.update(|window, cx| {
        fixture.app.update(cx, |app, cx| {
            let path = fixture.workspace.root().join("b.txt");
            app.close_tab(path.clone(), window, cx);
            app.open_file(path, window, cx);
        });
    });
    draw(ui);
    let original = ids(&fixture, ui);
    assert!(!original.contains(&stale));
    let session_file =
        SessionState::file_in_profile(fixture.workspace.root(), fixture.profile.path());
    let saved = std::fs::read(&session_file).unwrap();
    ui.update(|_, cx| {
        fixture.app.update(cx, |app, cx| {
            // An unrelated pending preference makes an accidental no-op persistence observable.
            app.session_state.window_width += 1.;
            for (source, before) in [
                (original[0], Some(original[0])),
                (original[0], Some(original[1])),
                (original[2], None),
                (stale, Some(original[0])),
                (original[0], Some(stale)),
                (u64::MAX, None),
            ] {
                app.reorder_file_tab(source, before, cx);
            }
        });
    });
    assert_eq!(ids(&fixture, ui), original);
    assert_eq!(std::fs::read(session_file).unwrap(), saved);
}

/// Preview status, active native identity and edited text/Undo survive moves of either document.
#[gpui::test]
fn tab_drag_order_preserves_preview_active_editor_and_undo(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx, true);
    let original = ids(&fixture, ui);
    ui.update(|window, cx| {
        fixture
            .app
            .update(cx, |app, cx| app.activate_tab(0, window, cx));
    });
    ui.simulate_input("中文");
    draw(ui);
    let (editor, text, revision, selection) = ui.update(|_, cx| {
        let app = fixture.app.read(cx);
        let text_tab = app.tabs[0].text.as_ref().unwrap();
        assert!(app.tabs[0].is_dirty());
        (
            app.editor.clone(),
            app.editor.read(cx).text().to_string(),
            text_tab.session.revision(),
            app.editor.read(cx).selected_range(),
        )
    });
    ui.update(|_, cx| {
        fixture.app.update(cx, |app, cx| {
            app.reorder_file_tab(original[0], None, cx);
            app.reorder_file_tab(original[2], Some(original[1]), cx);
        });
    });
    draw(ui);
    ui.update(|_, cx| {
        let app = fixture.app.read(cx);
        assert_eq!(app.tabs[0].file_id, original[2]);
        assert!(app.tabs[0].preview);
        assert_eq!(app.tabs[2].file_id, original[0]);
        assert!(app.tabs[2].is_dirty());
        assert_eq!(
            app.tabs[2].text.as_ref().unwrap().session.revision(),
            revision
        );
        assert_eq!(app.editor, editor);
        assert_eq!(app.editor.read(cx).text().to_string(), text);
        assert_eq!(app.editor.read(cx).selected_range(), selection);
        assert_eq!(
            app.active_path.as_deref(),
            Some(fixture.workspace.root().join("a.txt").as_path())
        );
    });
    let persisted = SessionState::load_in_profile(fixture.workspace.root(), fixture.profile.path());
    assert_eq!(persisted.open_tabs.len(), 2);
    assert!(persisted.open_tabs[0].ends_with("b.txt"));
    assert!(persisted.open_tabs[1].ends_with("a.txt"));
    ui.simulate_keystrokes("ctrl-z");
    draw(ui);
    ui.update(|_, cx| {
        let app = fixture.app.read(cx);
        assert_eq!(app.editor, editor);
        assert_eq!(app.editor.read(cx).text().to_string(), "first");
        assert!(app.tabs[0].preview);
    });
}

/// Press a tab title at `position` in `ui`; keep the click count for native double-click behavior.
fn press(ui: &mut VisualTestContext, position: Point<Pixels>, count: usize) {
    ui.simulate_event(MouseDownEvent {
        position,
        button: MouseButton::Left,
        modifiers: Default::default(),
        click_count: count,
        first_mouse: false,
    });
    draw(ui);
}

/// Release the pointer at `position` in `ui`, preserving the matching native click count.
fn release(ui: &mut VisualTestContext, position: Point<Pixels>, count: usize) {
    ui.simulate_event(MouseUpEvent {
        position,
        button: MouseButton::Left,
        modifiers: Default::default(),
        click_count: count,
    });
    draw(ui);
}

/// Cross the real drag threshold from `position` without crossing another tab's center.
fn start_drag(ui: &mut VisualTestContext, position: Point<Pixels>) {
    press(ui, position, 1);
    ui.simulate_mouse_move(
        position + point(px(8.), px(0.)),
        Some(MouseButton::Left),
        Default::default(),
    );
    draw(ui);
}

/// Native drag chrome previews both directions while the document owner changes order only on release.
#[gpui::test]
fn tab_drag_order_pointer_commits_only_on_release(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx, true);
    let original = ids(&fixture, ui);
    let active = ui.update(|_, cx| fixture.app.read(cx).editor.clone());
    let start = ui.debug_bounds("editor-tab-title-0").unwrap().center();
    let last = ui.debug_bounds("editor-tab-2").unwrap();
    let strip = ui.debug_bounds("editor-tabs-container").unwrap();
    let end = point(last.right() + px(24.), strip.center().y);
    assert!(end.x < strip.right());
    start_drag(ui, start);
    assert!(ui.update(|_, cx| fixture.app.read(cx).tabs_drag.active()));
    assert!(ui.debug_bounds("file-tab-drag-preview").is_some());
    assert!(ui.debug_bounds("file-tab-drag-slot").is_some());
    assert_eq!(ids(&fixture, ui), original);
    ui.simulate_mouse_move(end, Some(MouseButton::Left), Default::default());
    draw(ui);
    assert_eq!(ids(&fixture, ui), original);
    ui.update(|_, cx| {
        let app = fixture.app.read(cx);
        assert_eq!(app.editor, active);
        assert_eq!(
            app.active_path.as_deref(),
            Some(fixture.workspace.root().join("c.txt").as_path())
        );
        assert_eq!(app.editor.read(cx).text().to_string(), "third");
    });
    release(ui, end, 1);
    assert_eq!(
        ids(&fixture, ui),
        vec![original[1], original[2], original[0]]
    );
    assert!(!ui.update(|_, cx| fixture.app.read(cx).tabs_drag.active()));
    assert!(ui.debug_bounds("file-tab-drag-preview").is_none());
    assert!(ui.debug_bounds("file-tab-drag-slot").is_none());

    // Moving the preview itself backward must keep its replaceable identity and current text.
    let preview = ui.debug_bounds("editor-tab-title-1").unwrap().center();
    let first = ui.debug_bounds("editor-tab-0").unwrap();
    let before = point(first.center().x - px(2.), first.center().y);
    start_drag(ui, preview);
    ui.simulate_mouse_move(before, Some(MouseButton::Left), Default::default());
    draw(ui);
    assert_eq!(
        ids(&fixture, ui),
        vec![original[1], original[2], original[0]]
    );
    release(ui, before, 1);
    assert_eq!(
        ids(&fixture, ui),
        vec![original[2], original[1], original[0]]
    );
    ui.update(|_, cx| {
        let app = fixture.app.read(cx);
        assert_eq!(app.editor, active);
        assert!(app.tabs[0].preview);
        assert_eq!(app.editor.read(cx).text().to_string(), "third");
    });
}

/// Releasing below the strip or pressing Escape with editor focus cancels the host gesture.
#[gpui::test]
fn tab_drag_order_pointer_cancels_outside_and_on_escape(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx, false);
    let original = ids(&fixture, ui);
    let active = ui.update(|window, cx| {
        let editor = fixture.app.read(cx).editor.clone();
        editor.focus_handle(cx).focus(window, cx);
        editor
    });
    draw(ui);
    for escape in [false, true] {
        let start = ui.debug_bounds("editor-tab-title-0").unwrap().center();
        let strip = ui.debug_bounds("editor-tabs-container").unwrap();
        let last = ui.debug_bounds("editor-tab-2").unwrap();
        let destination = point(
            last.right() + px(24.),
            if escape {
                strip.center().y
            } else {
                strip.bottom() + px(30.)
            },
        );
        start_drag(ui, start);
        ui.simulate_mouse_move(destination, Some(MouseButton::Left), Default::default());
        draw(ui);
        assert!(ui.update(|_, cx| fixture.app.read(cx).tabs_drag.active()));
        assert_eq!(ids(&fixture, ui), original);
        if escape {
            ui.update(|window, cx| assert!(active.focus_handle(cx).is_focused(window)));
            ui.simulate_keystrokes("escape");
            draw(ui);
            assert!(!ui.update(|_, cx| fixture.app.read(cx).tabs_drag.active()));
        }
        release(ui, destination, 1);
        assert_eq!(ids(&fixture, ui), original);
        assert!(!ui.update(|_, cx| fixture.app.read(cx).tabs_drag.active()));
        ui.update(|_, cx| {
            let app = fixture.app.read(cx);
            assert_eq!(app.editor, active);
            assert_eq!(
                app.active_path.as_deref(),
                Some(fixture.workspace.root().join("c.txt").as_path())
            );
            assert_eq!(app.editor.read(cx).text().to_string(), "third");
        });
    }
}

/// Suppressing the completed drag's click must not consume the next close or preview double click.
#[gpui::test]
fn tab_drag_order_pointer_keeps_close_and_preview_double_click(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx, true);
    let original = ids(&fixture, ui);
    let preview = ui.debug_bounds("editor-tab-title-2").unwrap().center();
    let first = ui.debug_bounds("editor-tab-0").unwrap();
    let before = point(first.center().x - px(2.), first.center().y);
    start_drag(ui, preview);
    ui.simulate_mouse_move(before, Some(MouseButton::Left), Default::default());
    draw(ui);
    release(ui, before, 1);
    assert_eq!(
        ids(&fixture, ui),
        vec![original[2], original[0], original[1]]
    );
    assert!(ui.update(|_, cx| fixture.app.read(cx).tabs[0].preview));
    let close = ui.debug_bounds("editor-tab-close-2").unwrap().center();
    ui.simulate_click(close, Default::default());
    draw(ui);
    assert_eq!(ids(&fixture, ui), vec![original[2], original[0]]);
    let preview = ui.debug_bounds("editor-tab-title-0").unwrap().center();
    press(ui, preview, 2);
    release(ui, preview, 2);
    ui.update(|_, cx| {
        let app = fixture.app.read(cx);
        assert_eq!(app.tabs.len(), 2);
        assert!(!app.tabs[0].preview);
        assert_eq!(app.editor.read(cx).text().to_string(), "third");
    });
    assert!(!ui.update(|_, cx| fixture.app.read(cx).tabs_drag.active()));
}
