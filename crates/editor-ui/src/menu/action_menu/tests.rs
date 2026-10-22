//! Native menu search and composition preserve exact action identities and input ownership.
use super::*;
use plugin_runtime::plugin_protocol::{tree::Command, ui::LocalizedText};

/// Composition Enter is inert; committed Chinese search and native End/Enter select the exact action.
#[gpui_kit::gpui::test]
fn action_menu_ime_enter_waits_for_committed_search(cx: &mut gpui_kit::TestAppContext) {
    use gpui_base::Root;
    use std::cell::RefCell;
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::typography::init(cx);
        crate::theme::apply_theme(crate::theme::builtin_theme(false), cx);
    });
    let events = Rc::new(RefCell::new(Vec::new()));
    let sink = events.clone();
    let mut retained = None;
    let (_, visual) = cx.add_window_view(|window, cx| {
        let action = ActionMenuItem::action(
            "fetch-origin",
            Command {
                command: "fetch".into(),
                label: LocalizedText::new("Fetch", "获取"),
                arguments: serde_json::Value::Null,
                disabled: false,
                separator_before: false,
            },
        );
        let menu = cx.new(|cx| {
            ActionMenu::new(
                vec![action],
                "zh-CN".into(),
                Arc::new(BTreeMap::new()),
                point(px(10.), px(10.)),
                Some("搜索".into()),
                move |event, _, _| sink.borrow_mut().push(event),
                window,
                cx,
            )
        });
        retained = Some(menu.clone());
        Root::new(menu, window, cx)
    });
    let menu = retained.unwrap();
    visual.update(|window, cx| {
        window.draw(cx).clear(cx);
        menu.update(cx, |menu, cx| {
            let input = menu.search.clone().unwrap();
            input.update(cx, |input, cx| {
                input.replace_and_mark_text_in_range(None, "获取", Some(0..2), window, cx)
            });
            menu.activate_selected(window, cx);
            assert!(events.borrow().is_empty());
            input.update(cx, |input, cx| {
                input.replace_text_in_range(None, "获取", window, cx)
            });
        });
    });
    visual.update(|window, cx| window.draw(cx).clear(cx));
    visual.simulate_keystrokes("end enter");
    visual.run_until_parked();
    assert!(
        matches!(events.borrow().as_slice(), [ActionMenuEvent::Select(id)] if id == "fetch-origin")
    );
}

/// Overflow search reaches nested actions without losing ancestry, disabled state or exact IDs.
#[test]
fn search_finds_overflow_submenu_descendants() {
    let action = ActionMenuItem::action(
        "fetch-origin",
        Command {
            command: "fetch".into(),
            label: LocalizedText::new("Fetch", "获取"),
            arguments: serde_json::Value::Null,
            disabled: false,
            separator_before: false,
        },
    );
    let menu = vec![ActionMenuItem {
        id: "git-entry".into(),
        label: LocalizedText::new("main", "main"),
        detail: None,
        icon: None,
        checked: false,
        disabled: true,
        separator_before: false,
        kind: ActionMenuKind::Submenu {
            items: vec![action],
            on_open: None,
        },
    }];
    let mut rows = Vec::new();
    flatten_rows(
        &menu,
        0,
        "en",
        "fetch",
        &BTreeMap::new(),
        false,
        &[],
        &mut rows,
    );
    assert_eq!(
        rows.iter()
            .map(|row| row.item.id.as_str())
            .collect::<Vec<_>>(),
        ["git-entry", "fetch-origin"]
    );
    assert!(rows[1].item.disabled);
    assert_eq!(rows[1].item.detail.as_ref().unwrap().en, "main");
    rows.clear();
    flatten_rows(
        &menu,
        0,
        "zh-CN",
        "获取",
        &BTreeMap::new(),
        false,
        &[],
        &mut rows,
    );
    assert_eq!(rows[1].item.id, "fetch-origin");
}
