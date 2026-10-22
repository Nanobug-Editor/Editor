//! Actual Vue source proposals enter the native preview transaction and the single document Undo.
use super::vue_test_support::{Fixture, setup, shutdown, wait};
use super::*;
use crate::*;
use gpui_kit::{TestAppContext, VisualTestContext, gpui};

/// Renaming an exported prop discovers a closed parent, recomputes against native sessions and
/// previews every source edit. Cancel changes neither source; confirmation stays unsaved and Undo
/// restores both documents. Package code supplies all semantic edits through the production actor.
#[gpui::test]
#[ignore = "set NANOBUG_TYPESCRIPT_PACKAGE, NANOBUG_VUE_PACKAGE and NANOBUG_VUE_FIXTURES to real candidates"]
fn vue_prop_rename_discovers_parent_previews_cancels_and_group_undo(cx: &mut TestAppContext) {
    let (fixture, visual) = setup(cx, &["NANOBUG_TYPESCRIPT_PACKAGE", "NANOBUG_VUE_PACKAGE"]);
    let child = fixture
        .workspace
        .join("tests/projects/vue3/Child.vue")
        .canonicalize()
        .unwrap();
    let parent = fixture
        .workspace
        .join("tests/projects/vue3/App.vue")
        .canonicalize()
        .unwrap();
    let child_disk = std::fs::read_to_string(&child).unwrap();
    let parent_disk = std::fs::read_to_string(&parent).unwrap();
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Vue package recognition",
        30,
        |app, _| {
            app.providers
                .language_servers()
                .get("vue")
                .is_some_and(Option::is_some)
        },
    );
    visual.update(|window, cx| {
        window.activate_window();
        fixture
            .app
            .update(cx, |app, cx| app.open_file(child.clone(), window, cx));
        window.draw(cx).clear(cx);
    });
    visual.simulate_input("<!-- unsaved 中文😀 -->\n");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Vue prop editor ready",
        180,
        |app, cx| {
            for state in app.language_service_states.values() {
                if let crate::app::language_servers::ServiceLoadState::Failed(error) = state {
                    panic!("Vue source preparation failed: {error}");
                }
            }
            app.editor.read(cx).lsp().definition_provider.is_some()
        },
    );
    let child_original = source(&fixture, &child, visual);
    assert!(child_original.contains("unsaved 中文😀"));
    assert!(visual.update(|_, cx| {
        !fixture
            .app
            .read(cx)
            .tabs
            .iter()
            .any(|tab| tab.path() == parent)
    }));
    for apply in [false, true] {
        let caret = child_original.rfind("title").unwrap() + 2;
        visual.update(|window, cx| {
            fixture
                .app
                .read(cx)
                .editor
                .clone()
                .update(cx, |editor, cx| {
                    editor.set_selected_range(caret..caret, cx);
                    editor.focus(window, cx);
                    cx.notify();
                });
            window.draw(cx).clear(cx);
        });
        visual.simulate_keystrokes("f2");
        control(&fixture, visual, "editor-rename-prompt");
        visual.simulate_input("caption");
        visual.simulate_keystrokes("enter");
        control(&fixture, visual, "workspace-plan-confirm");
        assert_eq!(source(&fixture, &child, visual), child_original);
        assert_eq!(source(&fixture, &parent, visual), parent_disk);
        assert_eq!(std::fs::read_to_string(&child).unwrap(), child_disk);
        assert_eq!(std::fs::read_to_string(&parent).unwrap(), parent_disk);
        click(
            visual,
            if apply {
                "workspace-plan-confirm"
            } else {
                "workspace-plan-cancel"
            },
        );
        control_visible(&fixture, visual, "workspace-plan-confirm", false);
        if !apply {
            assert_eq!(source(&fixture, &child, visual), child_original);
            assert_eq!(source(&fixture, &parent, visual), parent_disk);
        }
    }
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "complete Vue rename applied",
        15,
        |app, cx| {
            [(&child, "caption"), (&parent, ":caption=")]
                .iter()
                .all(|(path, expected)| {
                    app.tabs
                        .iter()
                        .find(|tab| tab.path() == **path)
                        .and_then(|tab| tab.text.as_ref())
                        .is_some_and(|text| {
                            text.editor.read(cx).text().to_string().contains(*expected)
                        })
                })
        },
    );
    assert_eq!(
        source(&fixture, &child, visual),
        child_original.replace("title", "caption")
    );
    assert_eq!(
        source(&fixture, &parent, visual),
        parent_disk.replace(":title=", ":caption=")
    );
    assert_eq!(std::fs::read_to_string(&child).unwrap(), child_disk);
    assert_eq!(std::fs::read_to_string(&parent).unwrap(), parent_disk);
    control(&fixture, visual, "workspace-batch-undo");
    click(visual, "workspace-batch-undo");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "group Undo restored both Vue sources",
        15,
        |app, cx| {
            [(&child, &child_original), (&parent, &parent_disk)]
                .iter()
                .all(|(path, expected)| {
                    app.tabs
                        .iter()
                        .find(|tab| tab.path() == **path)
                        .and_then(|tab| tab.text.as_ref())
                        .is_some_and(|text| text.editor.read(cx).text().to_string() == **expected)
                })
        },
    );
    shutdown(&fixture, visual);
}

/// Read only the public native session text; an unopened target is a test failure.
fn source(fixture: &Fixture, path: &Path, visual: &mut VisualTestContext) -> String {
    visual.update(|_, cx| {
        fixture
            .app
            .read(cx)
            .tabs
            .iter()
            .find(|tab| tab.path() == path)
            .expect("normal target session must be open")
            .text
            .as_ref()
            .unwrap()
            .editor
            .read(cx)
            .text()
            .to_string()
    })
}

/// Paint through production publication until the requested native Base control is visible.
fn control(fixture: &Fixture, visual: &mut VisualTestContext, selector: &'static str) {
    control_visible(fixture, visual, selector, true);
}

/// A preview's asynchronous dismissal is observable through the same visible control boundary.
fn control_visible(
    fixture: &Fixture,
    visual: &mut VisualTestContext,
    selector: &'static str,
    visible: bool,
) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        wait(
            &fixture.app,
            &fixture.actor,
            visual,
            "native frame",
            1,
            |_, _| true,
        );
        if visual.debug_bounds(selector).is_some() == visible {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Missing {selector}; status={}",
            visual.update(|_, cx| fixture.app.read(cx).status.clone())
        );
    }
}

/// Activate the same Base button as a user confirmation or Undo action.
fn click(visual: &mut VisualTestContext, selector: &'static str) {
    let bounds = visual.debug_bounds(selector).unwrap();
    visual.simulate_mouse_move(bounds.center(), None, gpui_kit::Modifiers::default());
    visual.simulate_click(bounds.center(), gpui_kit::Modifiers::default());
    visual.run_until_parked();
}
