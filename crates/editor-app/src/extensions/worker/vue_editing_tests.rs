//! Actual Vue source proposals enter the native preview transaction and the single document Undo.
use super::vue_test_support::{Fixture, setup, shutdown, wait};
use super::*;
use crate::*;
use gpui_kit::{TestAppContext, VisualTestContext, gpui};

#[cfg(windows)]
mod external;
#[cfg(windows)]
mod historical;

/// The same dirty TypeScript dependency feeds three real Vue engines while its primary stays TS.
/// A type change is observed in source-mapped diagnostics without writing any participating file.
#[gpui::test]
#[ignore = "set NANOBUG_TYPESCRIPT_PACKAGE, NANOBUG_VUE_PACKAGE and NANOBUG_VUE_FIXTURES to real candidates"]
fn mixed_vue_projects_share_one_unsaved_typescript_dependency(cx: &mut TestAppContext) {
    let workspace = super::vue_test_support::input("NANOBUG_VUE_FIXTURES")
        .canonicalize()
        .unwrap();
    let temporary = tempfile::Builder::new()
        .prefix("s2-shared-")
        .tempdir_in(workspace.join("target"))
        .unwrap();
    let saved = "export const sharedValue = 'saved';\r\n";
    let shared = temporary.path().join("shared.ts");
    std::fs::write(&shared, saved).unwrap();
    let shared = shared.canonicalize().unwrap();
    let import = format!(
        "../../../{}",
        shared
            .strip_prefix(&workspace)
            .unwrap()
            .with_extension("")
            .to_string_lossy()
            .replace('\\', "/")
    );
    let paths = ["vue26", "vue27", "vue3"].map(|project| {
        workspace
            .join(format!("tests/projects/{project}/App.vue"))
            .canonicalize()
            .unwrap()
    });
    let disks = paths
        .iter()
        .map(|path| std::fs::read_to_string(path).unwrap())
        .collect::<Vec<_>>();
    let (fixture, visual) = setup(cx, &["NANOBUG_TYPESCRIPT_PACKAGE", "NANOBUG_VUE_PACKAGE"]);
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "mixed language catalog",
        30,
        |app, _| {
            app.providers
                .language_servers()
                .get("vue")
                .is_some_and(Option::is_some)
        },
    );
    visual.update(|window, cx| {
        fixture
            .app
            .update(cx, |app, cx| app.open_file(shared.clone(), window, cx))
    });
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "shared TS primary provider",
        120,
        |app, _| app.language_server_for_path(&shared).is_some(),
    );
    let primary = visual.update(|_, cx| {
        fixture
            .app
            .read(cx)
            .language_server_for_path(&shared)
            .unwrap()
            .clone()
    });
    for (index, path) in paths.iter().enumerate() {
        visual.update(|window, cx| {
            fixture
                .app
                .update(cx, |app, cx| app.open_file(path.clone(), window, cx))
        });
        let script = if index == 2 {
            format!(
                "<script setup lang=\"ts\">\r\nimport {{ sharedValue }} from '{import}';\r\n</script>"
            )
        } else {
            format!(
                "<script lang=\"ts\">\r\nimport Vue from 'vue';\r\nimport {{ sharedValue }} from '{import}';\r\nexport default Vue.extend({{data(){{return {{sharedValue}}}}}});\r\n</script>"
            )
        };
        visual.simulate_keystrokes("ctrl-a");
        visual.simulate_input(&format!("<!-- 中文😀e\u{301} -->\r\n{script}\r\n<template><div>{{{{ sharedValue.toUpperCase() }}}}</div></template>"));
        wait(
            &fixture.app,
            &fixture.actor,
            visual,
            "Vue shared consumer ready",
            180,
            |app, cx| {
                app.tabs
                    .iter()
                    .find(|tab| tab.path() == path)
                    .and_then(|tab| tab.text.as_ref())
                    .is_some_and(|text| text.editor.read(cx).lsp().hover_provider.is_some())
            },
        );
    }
    visual.update(|window, cx| {
        fixture
            .app
            .update(cx, |app, cx| app.open_file(shared.clone(), window, cx))
    });
    visual.simulate_keystrokes("ctrl-a");
    visual.simulate_input("// 未保存 中文😀e\u{301}\r\nexport const sharedValue = 42;\r\n");
    for path in &paths {
        visual.update(|window, cx| {
            fixture
                .app
                .update(cx, |app, cx| app.open_file(path.clone(), window, cx))
        });
        super::vue_test_support::assert_native_source_diagnostic(
            &fixture.app,
            &fixture.actor,
            path,
            "toUpperCase",
            &["toUpperCase"],
            visual,
        );
    }
    assert!(
        visual.update(|_, cx| fixture
            .app
            .read(cx)
            .language_server_for_path(&shared)
            .is_some_and(|server| Arc::ptr_eq(server, &primary))),
        "auxiliary synchronization must preserve the selected primary TS service"
    );
    assert_eq!(std::fs::read_to_string(&shared).unwrap(), saved);
    for (path, disk) in paths.iter().zip(&disks) {
        assert_eq!(std::fs::read_to_string(path).unwrap(), *disk);
    }
    shutdown(&fixture, visual);
}

/// In each admitted Vue version, renaming a prop discovers a closed parent and recomputes its sessions.
/// previews every source edit. Cancel changes neither source; confirmation stays unsaved and Undo
/// restores both documents. Package code supplies all semantic edits through the production actor.
#[gpui::test]
#[ignore = "set NANOBUG_TYPESCRIPT_PACKAGE, NANOBUG_VUE_PACKAGE and NANOBUG_VUE_FIXTURES to real candidates"]
fn vue_prop_rename_discovers_parent_previews_cancels_and_group_undo(cx: &mut TestAppContext) {
    let (fixture, visual) = setup(cx, &["NANOBUG_TYPESCRIPT_PACKAGE", "NANOBUG_VUE_PACKAGE"]);
    for project in ["vue26", "vue27", "vue3"] {
        let child = fixture
            .workspace
            .join(format!("tests/projects/{project}/Child.vue"))
            .canonicalize()
            .unwrap();
        let parent = fixture
            .workspace
            .join(format!("tests/projects/{project}/App.vue"))
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
                            .is_some_and(|text| {
                                text.editor.read(cx).text().to_string() == **expected
                            })
                    })
            },
        );
    }
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
