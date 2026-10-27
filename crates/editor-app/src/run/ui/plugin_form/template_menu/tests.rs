//! Native window regressions for template menus, using built-in contributors without WASM fixtures.
use super::*;
use gpui_kit::{TestAppContext, VisualTestContext, gpui};

/// Click a painted control and finish its resulting native-window frame.
fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let bounds = cx.debug_bounds(selector).expect(selector);
    cx.simulate_click(bounds.center(), Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

/// Open the production editor and its owned configuration dialog on an isolated workspace.
fn fixture<'a>(
    cx: &'a mut TestAppContext,
    root: &Path,
) -> (Entity<EditorApp>, &'a mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        cx.set_reduce_motion(true);
    });
    let workspace = Workspace::open(root).unwrap();
    let captured = std::rc::Rc::new(std::cell::RefCell::new(None));
    let capture = captured.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let app = cx.new(|cx| EditorApp::new(workspace, None, window, cx));
        capture.replace(Some(app.clone()));
        Root::new(app, window, cx)
    });
    let app = captured.borrow_mut().take().unwrap();
    visual.update(|window, cx| {
        app.update(cx, |app, cx| app.open_run_config_dialog(window, cx, None))
    });
    visual.run_until_parked();
    let handle = visual.update(|_, cx| app.read(cx).run_dialog_window.unwrap());
    *visual = VisualTestContext::from_window(handle.into(), &visual.cx);
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
    (app, visual)
}

/// Esc/outside presses dismiss only the popup; keyboard selection skips unavailable templates.
#[gpui::test]
fn template_menu_preserves_tree_and_dialog_in_both_themes(cx: &mut TestAppContext) {
    let root = tempfile::tempdir().unwrap();
    let (app, cx) = fixture(cx, root.path());
    let form = cx.update(|_, cx| app.read(cx).run_form.clone().unwrap());
    click(cx, "run-config-folder");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    let folder = cx.update(|_, cx| form.read(cx).plugin.as_ref().unwrap().selected.clone());
    // The built-in template follows the same catalog projection as a plugin; policy remains with its provider.
    let (provider, mut allowed) = host_templates::templates(&root.path().display().to_string())
        .pop()
        .unwrap();
    allowed.group = "Templates".into();
    let mut blocked = allowed.clone();
    blocked.id = "blocked".into();
    blocked.unavailable = Some("Tool is unavailable".into());
    let catalog = vec![(provider.clone(), blocked), (provider, allowed)];
    cx.update(|_, cx| {
        form.update(cx, |form, cx| {
            form.plugin.as_mut().unwrap().catalog = catalog;
            cx.notify();
        })
    });
    for dark in [false, true] {
        cx.update(|_, cx| apply_theme(builtin_theme(dark), cx));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let page = cx.debug_bounds("run-config-page").unwrap();
        let tree = cx.debug_bounds("run-config-tree-list").unwrap();
        let button = cx.debug_bounds("run-config-add").unwrap();
        click(cx, "run-config-add");
        let menu = cx.debug_bounds("native-popup-menu").unwrap();
        // The native menu reserves an eight-pixel viewport gutter, even beside a seven-pixel trigger inset.
        assert_eq!(
            menu.origin,
            point(button.left().max(px(8.)), button.bottom())
        );
        assert_eq!(page, cx.debug_bounds("run-config-page").unwrap());
        assert_eq!(tree, cx.debug_bounds("run-config-tree-list").unwrap());
        cx.update(|_, cx| {
            let state = form.read(cx).plugin.as_ref().unwrap();
            assert_eq!(state.selected, folder);
            let popup = state.template_menu.as_ref().unwrap().popup.read(cx);
            assert!(popup.items[0].disabled, "group headings cannot be selected");
            assert!(popup.items[1].disabled);
        });
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(cx.debug_bounds("run-config-form").is_some());
        assert!(cx.debug_bounds("native-popup-menu").is_none());
        cx.update(|window, cx| {
            assert!(
                form.read(cx)
                    .plugin
                    .as_ref()
                    .unwrap()
                    .template_trigger_focus
                    .is_focused(window)
            )
        });
        // Dismissal returns focus to the add button, so keyboard activation can reopen the same menu.
        cx.simulate_keystrokes("enter");
        // Base button activation completes on release; simulate_keystrokes supplies key-down events only.
        cx.simulate_event(gpui_kit::KeyUpEvent {
            keystroke: gpui_kit::Keystroke::parse("enter").unwrap(),
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(cx.debug_bounds("native-popup-menu").is_some());
        cx.simulate_click(page.center(), Default::default());
        cx.run_until_parked();
        assert!(cx.debug_bounds("native-popup-menu").is_none());
        assert!(cx.update(|_, cx| {
            form.read(cx)
                .plugin
                .as_ref()
                .unwrap()
                .draft
                .configurations
                .is_empty()
        }));
    }
    click(cx, "run-config-add");
    cx.simulate_keystrokes("home enter");
    cx.run_until_parked();
    cx.update(|_, cx| {
        let state = form.read(cx).plugin.as_ref().unwrap();
        assert!(state.template_menu.is_none());
        assert_eq!(state.draft.configurations.len(), 1);
        assert_eq!(
            state.draft.tree.placements[state.selected.as_ref().unwrap()].parent,
            folder
        );
        assert!(
            app.read(cx).run_controls.configurations().is_empty(),
            "selection only stages a draft"
        );
    });
    assert!(cx.debug_bounds("plugin-ui-name").is_some());
}

/// Late discovery refreshes an open, scrollable menu and Enter still addresses its final visible item.
#[gpui::test]
fn template_menu_refreshes_catalog_and_scrolls_keyboard_selection(cx: &mut TestAppContext) {
    let root = tempfile::tempdir().unwrap();
    let (app, cx) = fixture(cx, root.path());
    let form = cx.update(|_, cx| app.read(cx).run_form.clone().unwrap());
    click(cx, "run-config-add");
    let original = cx.update(|_, cx| {
        form.read(cx)
            .plugin
            .as_ref()
            .unwrap()
            .template_menu
            .as_ref()
            .unwrap()
            .popup
            .clone()
    });
    cx.update(|_, cx| {
        form.update(cx, |form, cx| {
            let state = form.plugin.as_mut().unwrap();
            let (provider, template) = state.catalog.last().unwrap().clone();
            state.catalog = (0..60)
                .map(|index| {
                    let mut item = template.clone();
                    item.group = "Many templates".into();
                    item.id = format!("item-{index}");
                    item.label = format!("Template {index}");
                    (provider.clone(), item)
                })
                .collect();
            state.catalog.push((provider, template));
            cx.notify();
        })
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert_eq!(
        original,
        cx.update(|_, cx| form
            .read(cx)
            .plugin
            .as_ref()
            .unwrap()
            .template_menu
            .as_ref()
            .unwrap()
            .popup
            .clone())
    );
    assert!(cx.debug_bounds("native-popup-menu").unwrap().size.height <= px(360.));
    cx.simulate_keystrokes("end");
    cx.run_until_parked();
    // The shared menu scrolls the actual painted final row into the bounded viewport.
    let last = cx.update(|_, cx| original.read(cx).items.last().unwrap().id.clone());
    let selector: &'static str = format!("native-menu-{last}").leak();
    let menu = cx.debug_bounds("native-popup-menu").unwrap();
    assert!(menu.contains(&cx.debug_bounds(selector).unwrap().center()));
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(cx.debug_bounds("native-popup-menu").is_none());
    assert!(cx.update(|_, cx| {
        form.read(cx)
            .plugin
            .as_ref()
            .unwrap()
            .draft
            .configurations
            .len()
            == 1
    }));
}
