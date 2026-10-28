//! Exercise shared title-button reveal through arbitrary native plugin contributions.

use super::*;
use gpui_kit::{MouseMoveEvent, VisualTestContext};

/// Paint queued panel/focus changes before querying visible native button bounds.
fn redraw(visual: &mut VisualTestContext) {
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
}

/// Move without pressing a button, preserving the current keyboard focus.
fn hover(visual: &mut VisualTestContext, position: Point<Pixels>) {
    visual.simulate_event(MouseMoveEvent {
        position,
        pressed_button: None,
        modifiers: Default::default(),
    });
    redraw(visual);
}

/// Mount a contribution through normal publication and DockArea registration.
/// `menu` selects whether the manifest exposes a menu command; keep `directory` alive for the app.
fn mount<'a>(
    cx: &'a mut TestAppContext,
    directory: &Path,
    menu: bool,
) -> (
    Entity<EditorApp>,
    Entity<ExtensionPanel>,
    &'a mut VisualTestContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        init(cx);
        cx.set_reduce_motion(true);
    });
    let workspace = Workspace::open(directory).unwrap();
    let session = SessionState::load_in_profile(directory, &directory.join("profile"));
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let app =
            cx.new(|cx| EditorApp::new_with_session(workspace, None, Some(session), window, cx));
        *capture.borrow_mut() = Some(app.clone());
        Root::new(app, window, cx)
    });
    let app = slot.borrow_mut().take().unwrap();
    visual.simulate_resize(size(px(1200.), px(820.)));
    let mut manifest =
        crate::extensions::test_manifest(include_str!("../fixtures/panel-contract.json"));
    manifest.id = "title-actions-fixture".into();
    manifest.panels.truncate(1);
    manifest.panels[0].id = "review".into();
    manifest.panels[0].position = "right".into();
    manifest.panels[0].default_visible = true;
    for command in &mut manifest.commands {
        command.menu = menu;
    }
    let scene = protocol::ui::Document::new(
        protocol::ui::Node::new(
            "title-fixture-content",
            protocol::ui::Kind::Text {
                text: "Native plugin content".into(),
            },
        )
        .grow(),
    )
    .revision(1);
    let panel = visual.update(|window, cx| {
        app.read(cx).extensions.clone().update(cx, |owner, cx| {
            let mut state = owner.worker.state.lock().unwrap();
            state.entries = vec![Installed {
                install_sequence: 0,
                replaced_by: None,
                manifest: manifest.clone(),
                digest: "title-actions-fixture".into(),
                grants: manifest.permissions.clone(),
                enabled: true,
                project_enabled: Default::default(),
                global_enabled: None,
                retired_ui_contract: false,
                error: None,
            }];
            state
                .views
                .insert("title-actions-fixture/review".into(), Arc::new(scene));
            drop(state);
            owner.poll(cx);
        });
        app.update(cx, |app, cx| app.sync_plugin_panels(window, cx));
        let panel = app.read(cx).plugin_panels["title-actions-fixture/review"].clone();
        panel.update(cx, |panel, cx| panel.poll(cx));
        app.read(cx).editor.focus_handle(cx).focus(window, cx);
        panel
    });
    redraw(visual);
    (app, panel, visual)
}

/// The plugin's own focus and pointer scope reveal its buttons; neighboring panels stay hidden.
#[gpui::test]
fn plugin_title_actions_follow_pointer_and_focus(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (app, panel, visual) = mount(cx, directory.path(), true);
    let slot = visual.debug_bounds("plugin-title-actions").unwrap();
    assert_eq!(slot.size.width, px(48.));
    assert!(visual.debug_bounds("plugin-hide").is_none());
    let body = native_panel_bounds(&app, PanelId::from(panel.entity_id()), visual).center();
    let outside = visual
        .debug_bounds("editor-panel-content")
        .unwrap()
        .center();
    for dark in [false, true] {
        visual.update(|_, cx| apply_theme(builtin_theme(dark), cx));
        for inside in [body, slot.center()] {
            hover(visual, inside);
            assert!(visual.debug_bounds("plugin-hide").is_some());
            assert!(visual.debug_bounds("plugin-command-menu").is_some());
            assert!(visual.debug_bounds("explorer-collapse-all").is_none());
            assert_eq!(visual.debug_bounds("plugin-title-actions").unwrap(), slot);
            visual.update(|window, cx| {
                assert!(app.read(cx).editor.focus_handle(cx).is_focused(window));
                panel.read(cx).focus_handle(cx).focus(window, cx);
            });
            hover(visual, outside);
            assert!(visual.debug_bounds("plugin-hide").is_some());
            visual.update(|window, cx| app.read(cx).editor.focus_handle(cx).focus(window, cx));
            redraw(visual);
            assert!(visual.debug_bounds("plugin-hide").is_none());
        }
    }
    // Hover-revealed controls still execute their ordinary saved visibility action.
    hover(visual, body);
    let hide = visual.debug_bounds("plugin-hide").unwrap().center();
    visual.simulate_click(hide, Default::default());
    redraw(visual);
    visual.update(|_, cx| {
        assert!(!panel.read(cx).visible.get());
        assert_eq!(
            app.read(cx).session_state.plugin_panel_visibility["title-actions-fixture/review"],
            false
        );
    });
}

/// A panel without manifest menu commands reserves only its minimize button's slot.
#[gpui::test]
fn plugin_title_actions_without_menu_keep_a_single_slot(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (app, panel, visual) = mount(cx, directory.path(), false);
    assert_eq!(
        visual
            .debug_bounds("plugin-title-actions")
            .unwrap()
            .size
            .width,
        px(24.)
    );
    let body = native_panel_bounds(&app, PanelId::from(panel.entity_id()), visual).center();
    hover(visual, body);
    assert!(visual.debug_bounds("plugin-hide").is_some());
    assert!(visual.debug_bounds("plugin-command-menu").is_none());
}

/// An owned popup retains its native anchor while the pointer leaves the dock panel.
#[gpui::test]
fn plugin_title_actions_keep_the_open_menu_anchor(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (app, panel, visual) = mount(cx, directory.path(), true);
    let body = native_panel_bounds(&app, PanelId::from(panel.entity_id()), visual).center();
    hover(visual, body);
    let menu = visual.debug_bounds("plugin-command-menu").unwrap().center();
    visual.simulate_click(menu, Default::default());
    redraw(visual);
    visual.update(|_, cx| assert!(panel.read(cx).commands_open));
    let outside = visual
        .debug_bounds("editor-panel-content")
        .unwrap()
        .center();
    hover(visual, outside);
    assert!(visual.debug_bounds("plugin-command-menu").is_some());
    assert!(visual.debug_bounds("plugin-hide").is_some());
}
