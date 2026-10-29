//! Verify directory activation through the explorer's real pointer and settings controls.

use super::*;

/// Default activation must expand on the first press and ignore a double click's second press.
#[gpui::test]
fn directory_click_mode_defaults_to_single_click(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(directory.path().join("a-dir/b-sub")).unwrap();
    std::fs::write(directory.path().join("a-dir/b-sub/child.txt"), "child").unwrap();
    std::fs::write(directory.path().join("z.txt"), "initial").unwrap();
    let workspace = Workspace::open(directory.path()).unwrap();
    let initial = workspace.root().join("z.txt");
    let parent = workspace.root().join("a-dir");
    let (app, visual) = mount(cx, workspace, initial.clone());
    for dark in [false, true] {
        visual.update(|_, cx| apply_theme(builtin_theme(dark), cx));
        redraw(visual);
        let label = visual.debug_bounds("explorer-label-1").unwrap().center();
        visual.simulate_click(label, Modifiers::default());
        redraw(visual);
        visual.update(|window, cx| {
            let app = app.read(cx);
            let state = app.tree_state.read(cx);
            assert!(state.entry(1).unwrap().is_expanded());
            assert_eq!(
                Path::new(state.selected_item().unwrap().id.as_str()),
                parent
            );
            assert!(!app.editor.focus_handle(cx).is_focused(window));
            assert_eq!(app.active_path.as_ref(), Some(&initial));
            assert_eq!(app.tabs.len(), 1);
        });
        double_click(visual, label);
        visual.update(|_, cx| {
            assert!(
                app.read(cx)
                    .tree_state
                    .read(cx)
                    .entry(1)
                    .unwrap()
                    .is_expanded()
            );
        });
        let icon = visual.debug_bounds("explorer-icon-1").unwrap().center();
        visual.simulate_click(icon, Modifiers::default());
        redraw(visual);
        visual.update(|_, cx| {
            assert!(
                !app.read(cx)
                    .tree_state
                    .read(cx)
                    .entry(1)
                    .unwrap()
                    .is_expanded()
            );
        });
        // Direction keys still reach Base's tree after the directory's pointer activation.
        visual.simulate_keystrokes("right");
        redraw(visual);
        visual.update(|_, cx| {
            assert!(
                app.read(cx)
                    .tree_state
                    .read(cx)
                    .entry(1)
                    .unwrap()
                    .is_expanded()
            );
        });
        visual.simulate_keystrokes("left");
        redraw(visual);
        visual.update(|_, cx| {
            assert!(
                !app.read(cx)
                    .tree_state
                    .read(cx)
                    .entry(1)
                    .unwrap()
                    .is_expanded()
            );
        });
    }
}

/// Both modes apply to empty folders and the project root, while arrows always toggle once.
#[gpui::test]
fn directory_click_mode_preserves_empty_folders_root_and_arrows(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("empty")).unwrap();
    let workspace = Workspace::open(directory.path()).unwrap();
    let (app, visual) = mount(cx, workspace, None);
    for double in [false, true] {
        visual.update(|_, cx| {
            app.update(cx, |app, _| {
                app.session_state.explorer_expand_on_double_click = double;
            });
        });
        redraw(visual);
        let label = visual.debug_bounds("explorer-label-1").unwrap().center();
        visual.simulate_click(label, Modifiers::default());
        redraw(visual);
        visual.update(|_, cx| {
            assert_eq!(
                app.read(cx)
                    .tree_state
                    .read(cx)
                    .entry(1)
                    .unwrap()
                    .is_expanded(),
                !double
            );
        });
        double_click(visual, label);
        visual.update(|_, cx| {
            assert!(
                app.read(cx)
                    .tree_state
                    .read(cx)
                    .entry(1)
                    .unwrap()
                    .is_expanded()
            );
        });
        let arrow = visual
            .debug_bounds("explorer-disclosure-1")
            .unwrap()
            .center();
        visual.simulate_click(arrow, Modifiers::default());
        redraw(visual);
        visual.update(|_, cx| {
            assert!(
                !app.read(cx)
                    .tree_state
                    .read(cx)
                    .entry(1)
                    .unwrap()
                    .is_expanded()
            );
        });
        let root = visual.debug_bounds("explorer-label-0").unwrap().center();
        visual.simulate_click(root, Modifiers::default());
        redraw(visual);
        visual.update(|_, cx| {
            assert_eq!(
                app.read(cx)
                    .tree_state
                    .read(cx)
                    .entry(0)
                    .unwrap()
                    .is_expanded(),
                double
            );
        });
        double_click(visual, root);
        visual.update(|_, cx| {
            assert!(
                !app.read(cx)
                    .tree_state
                    .read(cx)
                    .entry(0)
                    .unwrap()
                    .is_expanded()
            );
        });
        let arrow = visual
            .debug_bounds("explorer-disclosure-0")
            .unwrap()
            .center();
        visual.simulate_click(arrow, Modifiers::default());
        redraw(visual);
        visual.update(|_, cx| {
            assert!(
                app.read(cx)
                    .tree_state
                    .read(cx)
                    .entry(0)
                    .unwrap()
                    .is_expanded()
            );
        });
    }
}
