//! Exercise Explorer filtering through actual input, pointer clicks and keyboard dispatch.

use super::*;

/// Move the pointer without changing focus, exercising the Explorer's hover-only shortcut admission.
fn move_pointer(visual: &mut VisualTestContext, position: Point<Pixels>) {
    visual.simulate_event(gpui_kit::MouseMoveEvent {
        position,
        pressed_button: None,
        modifiers: Modifiers::default(),
    });
    redraw(visual);
}

/// Read the live flattened paths so assertions observe the tree rendered to the user.
fn visible_paths(app: &Entity<EditorApp>, visual: &mut VisualTestContext) -> Vec<PathBuf> {
    visual.update(|_, cx| {
        let tree = app.read(cx).tree_state.read(cx);
        (0..)
            .map_while(|index| tree.entry(index))
            .map(|entry| PathBuf::from(entry.item().id.as_str()))
            .collect()
    })
}

#[gpui::test]
fn explorer_filter_shortcut_requires_panel_focus_or_hover(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let initial = directory.path().join("active.txt");
    std::fs::write(&initial, "editor text").unwrap();
    let (app, visual) = mount(cx, Workspace::open(directory.path()).unwrap(), initial);
    assert!(visual.debug_bounds("explorer-filter").is_none());

    move_pointer(visual, point(px(800.), px(400.)));
    visual.simulate_keystrokes("ctrl-f");
    redraw(visual);
    assert!(visual.debug_bounds("explorer-filter").is_none());
    visual.simulate_keystrokes("escape");

    let tree = visual.debug_bounds("explorer-root").unwrap();
    move_pointer(visual, tree.bottom_left() + point(px(20.), -px(30.)));
    visual.simulate_keystrokes("ctrl-f");
    redraw(visual);
    // The input is 24 px; its wrapper's rem-based padding follows the user's font size.
    let height = visual.debug_bounds("explorer-filter").unwrap().size.height;
    assert!(height > px(24.) && height <= px(32.));
    visual.update(|window, cx| {
        let app = app.read(cx);
        assert!(
            app.explorer_panel
                .focus_handle(cx)
                .contains_focused(window, cx)
        );
        assert!(!app.editor.read(cx).search_session().open);
    });
    visual.simulate_keystrokes("ctrl-f");
    redraw(visual);
    assert!(visual.debug_bounds("explorer-filter").is_none());

    move_pointer(visual, point(px(800.), px(400.)));
    // Closing returned focus to the tree, so the next chord works even outside its hover area.
    visual.simulate_keystrokes("ctrl-f");
    redraw(visual);
    assert!(visual.debug_bounds("explorer-filter").is_some());

    visual.simulate_keystrokes("escape");
    redraw(visual);
    assert!(visual.debug_bounds("explorer-filter").is_none());

    visual.update(|window, cx| app.read(cx).editor.focus_handle(cx).focus(window, cx));
    let title = visual.debug_bounds("explorer-title").unwrap().center();
    move_pointer(visual, title);
    visual.simulate_keystrokes("ctrl-f");
    redraw(visual);
    assert!(visual.debug_bounds("explorer-filter").is_some());
    visual.simulate_keystrokes("escape");
    visual.update(|window, cx| {
        app.update(cx, |app, cx| app.toggle_explorer(window, cx));
        app.read(cx).editor.focus_handle(cx).focus(window, cx);
    });
    redraw(visual);
    visual.simulate_keystrokes("ctrl-f");
    redraw(visual);
    assert!(!visual.update(|_, cx| app.read(cx).explorer_filter.visible));
}

#[gpui::test]
fn explorer_filter_button_matches_hidden_files_and_hides_after_file_selection(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(directory.path().join("src/deep")).unwrap();
    std::fs::write(directory.path().join("src/deep/Main.RS"), "selected file").unwrap();
    std::fs::write(directory.path().join("README.md"), "readme").unwrap();
    let workspace = Workspace::open(directory.path()).unwrap();
    let root = workspace.root().to_path_buf();
    let target = root.join("src/deep/Main.RS");
    let (app, visual) = mount(cx, workspace, None);
    focus_tree(&app, visual);
    let before = visual.update(|_, cx| expansion_state(app.read(cx), cx));
    let button = visual
        .debug_bounds("explorer-toggle-filter")
        .unwrap()
        .center();
    visual.simulate_click(button, Modifiers::default());
    redraw(visual);
    assert!(visual.debug_bounds("explorer-filter").is_some());
    visual.simulate_click(button, Modifiers::default());
    redraw(visual);
    assert!(visual.debug_bounds("explorer-filter").is_none());
    visual.simulate_click(button, Modifiers::default());
    redraw(visual);
    visual.simulate_input("mAiN.rS");
    redraw(visual);
    assert_eq!(
        visible_paths(&app, visual),
        [
            root.clone(),
            root.join("src"),
            root.join("src/deep"),
            target.clone()
        ]
    );
    visual.update(|_, cx| {
        app.update(cx, |app, cx| app.capture_explorer_state(cx));
        assert!(
            !app.read(cx)
                .session_state
                .expanded_directories
                .iter()
                .any(|path| Path::new(path) == root.join("src"))
        );
    });
    // Closing the control restores the exact expansion state before filtering.
    visual.simulate_keystrokes("ctrl-f");
    redraw(visual);
    assert_eq!(
        visual.update(|_, cx| expansion_state(app.read(cx), cx)),
        before
    );
    focus_tree(&app, visual);
    let button = visual
        .debug_bounds("explorer-toggle-filter")
        .unwrap()
        .center();
    visual.simulate_click(button, Modifiers::default());
    redraw(visual);
    visual.simulate_input("MAIN");
    redraw(visual);
    let folder = visual.debug_bounds("explorer-row-1").unwrap().center();
    visual.simulate_click(folder, Modifiers::default());
    redraw(visual);
    assert!(visual.debug_bounds("explorer-filter").is_some());
    // Default activation resets descendants when collapsing; reopening each ancestor keeps filtering.
    visual.simulate_click(folder, Modifiers::default());
    redraw(visual);
    assert!(visual.debug_bounds("explorer-filter").is_some());
    let nested = visual.debug_bounds("explorer-row-2").unwrap().center();
    visual.simulate_click(nested, Modifiers::default());
    redraw(visual);
    assert!(visual.debug_bounds("explorer-filter").is_some());
    let file = visual.debug_bounds("explorer-row-3").unwrap().center();
    visual.simulate_click(file, Modifiers::default());
    redraw(visual);
    assert!(visual.debug_bounds("explorer-filter").is_none());
    visual.update(|_, cx| {
        let app = app.read(cx);
        assert_eq!(app.active_path.as_ref(), Some(&target));
        assert_eq!(app.editor.read(cx).value().to_string(), "selected file");
        assert_eq!(
            app.tree_state.read(cx).selected_item().unwrap().id.as_str(),
            target.to_string_lossy()
        );
    });
}

#[gpui::test]
fn explorer_filter_refresh_clear_and_keyboard_file_open(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("文件É.txt"), "unicode").unwrap();
    std::fs::write(directory.path().join("other.txt"), "other").unwrap();
    let workspace = Workspace::open(directory.path()).unwrap();
    let root = workspace.root().to_path_buf();
    let (app, visual) = mount(cx, workspace, None);
    focus_tree(&app, visual);
    visual.simulate_keystrokes("ctrl-f");
    redraw(visual);
    visual.simulate_input("文件é");
    redraw(visual);
    assert_eq!(
        visible_paths(&app, visual),
        [root.clone(), root.join("文件É.txt")]
    );
    std::fs::write(root.join("文件é2.txt"), "new match").unwrap();
    visual.update(|_, cx| app.update(cx, |app, cx| app.refresh_files(cx)));
    redraw(visual);
    assert_eq!(visible_paths(&app, visual).len(), 3);
    visual.simulate_keystrokes("ctrl-a backspace");
    redraw(visual);
    assert_eq!(visible_paths(&app, visual).len(), 4);
    visual.simulate_input("other");
    redraw(visual);
    visual.simulate_keystrokes("down enter");
    redraw(visual);
    assert!(visual.debug_bounds("explorer-filter").is_none());
    visual.update(|_, cx| {
        assert_eq!(
            app.read(cx).active_path.as_ref(),
            Some(&root.join("other.txt"))
        )
    });
}
