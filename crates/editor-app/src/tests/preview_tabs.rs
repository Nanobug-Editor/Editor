//! Explorer preview behavior through real pointer, native input and session restoration boundaries.

use crate::*;
use gpui_kit::{ClipboardItem, EntityInputHandler as _, TestAppContext, VisualTestContext, gpui};
use std::cell::RefCell;

/// Each fixture owns disk files and persisted session paths independently of user projects.
struct Fixture {
    _directory: tempfile::TempDir,
    workspace: Workspace,
    app: Entity<EditorApp>,
}

/// Mount production tree/editor surfaces, with one fixed file when requested.
fn mount(cx: &mut TestAppContext, fixed: bool) -> (Fixture, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        cx.set_reduce_motion(true);
    });
    let directory = tempfile::tempdir().unwrap();
    for (name, content) in [("a.txt", "first"), ("b.txt", "second"), ("c.txt", "third")] {
        std::fs::write(directory.path().join(name), content).unwrap();
    }
    let mut image_bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(2, 2)
        .write_to(&mut image_bytes, image::ImageFormat::Png)
        .unwrap();
    std::fs::write(
        directory.path().join("picture.png"),
        image_bytes.into_inner(),
    )
    .unwrap();
    let workspace = Workspace::open(directory.path()).unwrap();
    let initial = fixed.then(|| workspace.root().join("a.txt"));
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let opened = workspace.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let app = cx.new(|cx| EditorApp::new(opened, initial, window, cx));
        *capture.borrow_mut() = Some(app.clone());
        Root::new(app, window, cx)
    });
    let app = slot.borrow_mut().take().unwrap();
    visual.simulate_resize(size(px(1000.), px(700.)));
    draw(visual);
    if !fixed {
        // Normal startup opens a fallback file; close it through production lifecycle for empty fixtures.
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                if let Some(path) = app.active_path.clone() {
                    app.close_tab(path, window, cx);
                }
            });
        });
        draw(visual);
    }
    (
        Fixture {
            _directory: directory,
            workspace,
            app,
        },
        visual,
    )
}

/// Draw after subscriptions drain so assertions inspect the visible active document.
fn draw(ui: &mut VisualTestContext) {
    ui.run_until_parked();
    ui.update(|window, cx| window.draw(cx).clear(cx));
}

/// Send real mouse counts; normal GPUI click simulation always emits count one.
fn click(ui: &mut VisualTestContext, selector: &'static str, count: usize) {
    let position = ui
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("missing {selector}"))
        .center();
    ui.simulate_event(MouseDownEvent {
        position,
        button: MouseButton::Left,
        modifiers: Default::default(),
        click_count: count,
        first_mouse: false,
    });
    ui.simulate_event(MouseUpEvent {
        position,
        button: MouseButton::Left,
        modifiers: Default::default(),
        click_count: count,
    });
    draw(ui);
}

/// Intrinsic document widths and measured scrolling survive long names, themes and scaling.
#[gpui::test]
fn preview_tabs_fit_titles_cap_width_and_reveal_selection(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx, true);
    let names = [
        "中文名称.txt".to_string(),
        format!("{}.txt", "W".repeat(100)),
    ];
    for name in &names {
        let path = fixture.workspace.root().join(name);
        std::fs::write(&path, "content").unwrap();
        ui.update(|window, cx| {
            fixture
                .app
                .update(cx, |app, cx| app.open_file(path, window, cx));
        });
        draw(ui);
    }
    for (dark, scale) in [(false, 1.), (true, 1.5)] {
        ui.update(|window, cx| {
            apply_theme(builtin_theme(dark), cx);
            window.refresh();
        });
        ui.simulate_scale_factor_change(scale);
        ui.simulate_resize(size(px(1100.), px(700.)));
        draw(ui);
        let width = ui.debug_bounds("editor-tabs-container").unwrap().size.width;
        ui.simulate_resize(size(px(1100.) + px(500.) - width, px(700.)));
        draw(ui);
        let short = ui.debug_bounds("editor-tab-0").unwrap();
        let medium = ui.debug_bounds("editor-tab-1").unwrap();
        let long = ui.debug_bounds("editor-tab-2").unwrap();
        assert!(short.size.width < medium.size.width);
        assert!(medium.size.width < px(400.));
        assert!((long.size.width - px(400.)).abs() < px(0.1));
        let title = ui.debug_bounds("editor-tab-title-2").unwrap();
        let close = ui.debug_bounds("editor-tab-close-2").unwrap();
        assert!(title.size.width < px(400.));
        assert!(title.right() <= close.left());
        assert!(close.right() <= long.right());
        for (index, selector) in [
            (0, "editor-tab-0"),
            (2, "editor-tab-2"),
            (1, "editor-tab-1"),
        ] {
            ui.update(|window, cx| {
                fixture
                    .app
                    .update(cx, |app, cx| app.activate_tab(index, window, cx));
            });
            draw(ui);
            let tab = ui.debug_bounds(selector).unwrap();
            let strip = ui.debug_bounds("editor-tabs-container").unwrap();
            assert!(tab.left() >= strip.left() - px(0.1));
            assert!(tab.right() <= strip.right() + px(0.1));
        }
    }
}

/// Single clicks share a slot while fixed files retain their identity, order and tree focus.
#[gpui::test]
fn preview_tabs_reuse_one_slot_and_activate_existing_files(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx, true);
    click(ui, "explorer-row-2", 1);
    assert!(ui.debug_bounds("editor-preview-badge").is_some());
    click(ui, "explorer-row-1", 1);
    ui.update(|window, cx| {
        let state = fixture.app.read(cx);
        assert_eq!(state.tabs.len(), 2);
        assert_eq!(state.editor.read(cx).value().to_string(), "first");
        assert!(window.focused(cx).is_some());
        assert!(!state.editor.focus_handle(cx).is_focused(window));
    });
    ui.simulate_keystrokes("down");
    draw(ui);
    ui.update(|_, cx| {
        assert!(
            fixture
                .app
                .read(cx)
                .tree_state
                .read(cx)
                .selected_item()
                .unwrap()
                .id
                .ends_with("b.txt")
        );
    });
    click(ui, "explorer-row-3", 1);
    ui.update(|window, cx| {
        let state = fixture.app.read(cx);
        assert_eq!(state.tabs.len(), 2);
        assert_eq!(state.tabs[0].title(), "a.txt");
        assert_eq!(state.tabs[1].title(), "c.txt");
        assert_eq!(state.editor.read(cx).value().to_string(), "third");
        assert!(window.focused(cx).is_some());
        assert!(!state.editor.focus_handle(cx).is_focused(window));
    });
}

/// Explorer and tab double clicks promote in place without duplicating the document.
#[gpui::test]
fn preview_tabs_double_click_keeps_file_or_tab(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx, false);
    click(ui, "explorer-row-1", 1);
    click(ui, "explorer-row-1", 2);
    assert!(ui.debug_bounds("editor-preview-badge").is_none());
    ui.update(|window, cx| {
        let state = fixture.app.read(cx);
        assert!(state.editor.focus_handle(cx).is_focused(window));
    });
    click(ui, "explorer-row-2", 1);
    click(ui, "editor-tab-1", 2);
    assert!(ui.debug_bounds("editor-preview-badge").is_none());
    click(ui, "explorer-row-3", 1);
    ui.update(|_, cx| {
        let state = fixture.app.read(cx);
        assert_eq!(state.tabs.len(), 3);
        assert!(!state.tabs[0].preview);
        assert!(!state.tabs[1].preview);
        assert!(state.tabs[2].preview);
    });
}

/// Typing Chinese text through native input promotes permanently, including after Undo and save.
#[gpui::test]
fn preview_tabs_edit_undo_and_save_never_restore_preview(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx, false);
    click(ui, "explorer-row-1", 1);
    ui.update(|window, cx| {
        fixture
            .app
            .read(cx)
            .editor
            .focus_handle(cx)
            .focus(window, cx);
    });
    ui.simulate_input("中文");
    draw(ui);
    assert!(ui.debug_bounds("editor-preview-badge").is_none());
    ui.simulate_keystrokes("ctrl-z");
    draw(ui);
    ui.update(|_, cx| {
        assert_eq!(
            fixture.app.read(cx).editor.read(cx).value().to_string(),
            "first"
        );
        fixture.app.update(cx, |app, cx| app.save_current(cx));
    });
    click(ui, "explorer-row-2", 1);
    ui.update(|_, cx| {
        let state = fixture.app.read(cx);
        assert_eq!(state.tabs.len(), 2);
        assert!(!state.tabs[0].preview);
        assert!(!state.tabs[0].is_dirty());
    });
    assert_eq!(
        std::fs::read_to_string(fixture.workspace.root().join("a.txt")).unwrap(),
        "first"
    );
}

/// Preedit remains in its original tab when browsing switches files before the committed Change.
#[gpui::test]
fn preview_tabs_ime_composition_keeps_its_original_document(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx, false);
    click(ui, "explorer-row-1", 1);
    let composing_editor = ui.update(|window, cx| {
        let editor = fixture.app.read(cx).editor.clone();
        editor.update(cx, |editor, cx| {
            editor.focus(window, cx);
            // Model the native marked-text phase before a platform IME commits its result.
            editor.replace_and_mark_text_in_range(None, "拼", Some(0..1), window, cx);
        });
        editor
    });
    draw(ui);
    // Base withholds Change during preedit; preview replacement must also inspect live content.
    click(ui, "explorer-row-2", 1);
    ui.update(|window, cx| {
        composing_editor.update(cx, |editor, cx| {
            editor.replace_text_in_range(None, "中文", window, cx);
        });
    });
    draw(ui);
    ui.update(|_, cx| {
        let state = fixture.app.read(cx);
        assert_eq!(state.tabs.len(), 2);
        assert!(!state.tabs[0].preview);
        assert_eq!(state.editor.read(cx).text().to_string(), "second");
        assert_eq!(
            state.tabs[0]
                .text
                .as_ref()
                .unwrap()
                .editor
                .read(cx)
                .text()
                .to_string(),
            "中文first"
        );
    });
}

/// Base may emit Change for Delete at EOF or identical replacement; neither is a content edit.
#[gpui::test]
fn preview_tabs_unchanged_input_remains_replaceable(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx, false);
    click(ui, "explorer-row-1", 1);
    ui.update(|window, cx| {
        fixture
            .app
            .read(cx)
            .editor
            .focus_handle(cx)
            .focus(window, cx)
    });
    ui.simulate_keystrokes("ctrl-end delete");
    draw(ui);
    assert!(ui.debug_bounds("editor-preview-badge").is_some());
    ui.write_to_clipboard(ClipboardItem::new_string("first".into()));
    ui.simulate_keystrokes("ctrl-a");
    ui.simulate_keystrokes("ctrl-v");
    draw(ui);
    assert!(ui.debug_bounds("editor-preview-badge").is_some());
    click(ui, "explorer-row-2", 1);
    ui.update(|_, cx| {
        let state = fixture.app.read(cx);
        assert_eq!(state.tabs.len(), 1);
        assert_eq!(state.editor.read(cx).value().to_string(), "second");
    });
}

/// A saved session with no fixed files must show an empty editor instead of reopening a fallback.
#[gpui::test]
fn preview_tabs_restart_with_only_preview_restores_empty_editor(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx, false);
    click(ui, "explorer-row-1", 1);
    assert!(
        SessionState::load(fixture.workspace.root())
            .open_tabs
            .is_empty()
    );
    let workspace = fixture.workspace.clone();
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, ui) = cx.add_window_view(move |window, cx| {
        let app = cx.new(|cx| EditorApp::new(workspace, None, window, cx));
        *capture.borrow_mut() = Some(app.clone());
        Root::new(app, window, cx)
    });
    let restored = slot.borrow_mut().take().unwrap();
    draw(ui);
    ui.update(|_, cx| {
        assert!(restored.read(cx).tabs.is_empty());
        assert!(restored.read(cx).active_path.is_none());
    });
}

/// Binary previews preserve tree focus and never borrow the background text session.
#[gpui::test]
fn preview_tabs_file_only_view_replaces_text_without_taking_focus(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx, false);
    click(ui, "explorer-row-1", 1);
    click(ui, "explorer-row-4", 1);
    ui.update(|window, cx| {
        let state = fixture.app.read(cx);
        assert_eq!(state.tabs.len(), 1);
        assert!(state.active_text_tab_index().is_none());
        assert!(window.focused(cx).is_some());
        assert!(!state.editor.focus_handle(cx).is_focused(window));
    });
    click(ui, "explorer-row-4", 2);
    click(ui, "explorer-row-2", 1);
    ui.update(|_, cx| {
        let state = fixture.app.read(cx);
        assert_eq!(state.tabs.len(), 2);
        assert!(!state.tabs[0].preview);
        assert!(state.tabs[0].text.is_none());
    });
}

/// Explicit opening promotes a preview; failed preparation preserves the current preview and contents.
#[gpui::test]
fn preview_tabs_explicit_open_and_failed_file_are_lossless(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx, false);
    click(ui, "explorer-row-1", 1);
    ui.update(|window, cx| {
        fixture.app.update(cx, |app, cx| {
            app.open_preview_file(fixture.workspace.root().join("missing.txt"), window, cx);
            assert_eq!(app.tabs.len(), 1);
            assert!(app.tabs[0].preview);
            app.open_file(fixture.workspace.root().join("a.txt"), window, cx);
        });
    });
    draw(ui);
    assert!(ui.debug_bounds("editor-preview-badge").is_none());
    click(ui, "explorer-row-2", 1);
    ui.update(|_, cx| assert_eq!(fixture.app.read(cx).tabs.len(), 2));
}

/// Restart uses the actual persisted session and restores only promoted files.
#[gpui::test]
fn preview_tabs_restart_restores_only_fixed_documents(cx: &mut TestAppContext) {
    let (fixture, ui) = mount(cx, true);
    click(ui, "explorer-row-2", 1);
    let saved = SessionState::load(fixture.workspace.root());
    assert_eq!(saved.open_tabs.len(), 1);
    assert!(saved.active_file.is_none());
    let workspace = fixture.workspace.clone();
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, restored_ui) = cx.add_window_view(move |window, cx| {
        let app = cx.new(|cx| EditorApp::new(workspace, None, window, cx));
        *capture.borrow_mut() = Some(app.clone());
        Root::new(app, window, cx)
    });
    let restored = slot.borrow_mut().take().unwrap();
    draw(restored_ui);
    restored_ui.update(|_, cx| {
        let state = restored.read(cx);
        assert_eq!(state.tabs.len(), 1);
        assert_eq!(state.editor.read(cx).value().to_string(), "first");
        assert!(!state.tabs[0].preview);
    });
}
