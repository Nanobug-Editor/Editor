//! Native JS imports and Vue template navigation remain independent from the TypeScript plugin.
use super::super::vue_test_support::{input, setup_for_workspace, shutdown, wait};
use super::*;

/// Copy an explicitly supplied installed Vue library into an isolated, fully contained project.
fn copy_library(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_library(&entry.path(), &target);
        } else {
            assert!(
                entry.file_type().unwrap().is_file(),
                "fixture library must be self-contained"
            );
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// Real package admission, native source synchronization and original-source results share one loop.
#[gpui::test]
#[ignore = "set NANOBUG_JAVASCRIPT_PACKAGE, NANOBUG_VUE_PACKAGE and NANOBUG_VUE2_MODULE to built packages and an installed Vue 2.6.14 library"]
fn javascript_and_vue_own_native_navigation_without_typescript(cx: &mut TestAppContext) {
    let workspace = tempfile::tempdir().unwrap();
    copy_library(
        &input("NANOBUG_VUE2_MODULE").canonicalize().unwrap(),
        &workspace.path().join("node_modules/vue"),
    );
    std::fs::create_dir(workspace.path().join("src")).unwrap();
    for (name, source) in [
        ("package.json", r#"{"dependencies":{"vue":"2.6.14"}}"#),
        (
            "jsconfig.json",
            r#"{"compilerOptions":{"allowJs":true,"checkJs":true,"baseUrl":".","paths":{"@/*":["src/*"]},"moduleResolution":"Node","target":"ES2020"},"include":["src/**/*"]}"#,
        ),
        ("src/model.js", "export const message = 'saved';\n"),
        (
            "src/main.js",
            "import { message } from '@/model';\nmessage;\n",
        ),
        (
            "src/App.vue",
            "<script>\nimport Vue from 'vue';\nimport { message } from '@/model';\nexport default Vue.extend({ data() { return { message }; } });\n</script>\n<template><p>{{ message }}</p></template>\n",
        ),
    ] {
        std::fs::write(workspace.path().join(name), source).unwrap();
    }
    let (fixture, visual) = setup_for_workspace(
        cx,
        &["NANOBUG_JAVASCRIPT_PACKAGE", "NANOBUG_VUE_PACKAGE"],
        workspace.path().to_owned(),
    );
    let app = fixture.app.clone();
    let actor = fixture.actor.clone();
    for (relative, owner) in [("src/main.js", "javascript"), ("src/App.vue", "vue")] {
        let path = fixture.workspace.join(relative).canonicalize().unwrap();
        visual.update(|window, cx| {
            window.activate_window();
            app.update(cx, |app, cx| app.open_file(path.clone(), window, cx));
            window.draw(cx).clear(cx);
        });
        wait(
            &app,
            &actor,
            visual,
            "independent native language role",
            180,
            |app, cx| {
                for state in app.language_service_states.values() {
                    if let crate::app::language_servers::ServiceLoadState::Failed(error) = state {
                        panic!("independent language preparation failed: {error}");
                    }
                }
                app.language_server_for_path(&path)
                    .is_some_and(|server| server.is_active())
                    && app.editor.read(cx).lsp().definition_provider.is_some()
                    && app.editor.read(cx).lsp().hover_provider.is_some()
            },
        );
        visual.update(|_, cx| {
            let app = app.read(cx);
            assert!(
                crate::language::code_highlighting::selected_in(
                    &app.providers,
                    &app.language_for_path(&path)
                )
                .is_some()
            );
        });
        assert!(
            actor
                .state
                .lock()
                .unwrap()
                .language_services
                .values()
                .any(|plan| plan
                    .as_ref()
                    .is_ok_and(|plan| plan.owner == owner && plan.is_active()))
        );
        let observed = Rc::new(RefCell::new(None));
        let capture = observed.clone();
        visual.update(|window, cx| {
            let (source, definition_provider, hover_provider) = {
                let editor = app.read(cx).editor.read(cx);
                (
                    editor.text().clone(),
                    editor.lsp().definition_provider.clone().unwrap(),
                    editor.lsp().hover_provider.clone().unwrap(),
                )
            };
            let offset = source.to_string().rfind("message").unwrap() + 2;
            let definitions = definition_provider.definitions(&source, offset, window, cx);
            let hover = hover_provider.hover(&source, offset, window, cx);
            app.update(cx, |_, cx| {
                cx.spawn(async move |_, _| {
                    *capture.borrow_mut() = Some((definitions.await, hover.await));
                })
                .detach()
            });
        });
        wait(
            &app,
            &actor,
            visual,
            "source definitions and hover",
            45,
            |_, _| observed.borrow().is_some(),
        );
        let (definitions, hover) = observed.borrow_mut().take().unwrap();
        let definitions = definitions.expect("definition request must succeed");
        assert!(!definitions.is_empty(), "{relative} has no definitions");
        for definition in &definitions {
            let target = url::Url::parse(definition.target_uri.as_str())
                .unwrap()
                .to_file_path()
                .unwrap();
            assert!(
                target
                    .canonicalize()
                    .unwrap()
                    .starts_with(&fixture.workspace)
            );
            assert!(!target.to_string_lossy().contains("__VLS"));
        }
        if owner == "javascript" {
            assert!(
                definitions
                    .iter()
                    .any(|definition| definition.target_uri.as_str().ends_with("/src/model.js"))
            );
        }
        assert!(
            hover.expect("hover request must succeed").is_some(),
            "{relative} has no hover"
        );
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                let offset = app
                    .editor
                    .read(cx)
                    .text()
                    .to_string()
                    .rfind("message")
                    .unwrap()
                    + 2;
                app.editor.update(cx, |editor, cx| {
                    editor.set_selected_range(offset..offset, cx);
                    editor.focus(window, cx);
                });
            });
        });
        visual.simulate_keystrokes("f12");
        wait(
            &app,
            &actor,
            visual,
            "F12 reveals the definition",
            30,
            |app, _| {
                app.active_tab_index()
                    .and_then(|index| app.tabs[index].text.as_ref())
                    .is_some_and(|text| text.definition_highlight_generation > 0)
            },
        );
        visual.update(|_, cx| {
            let app = app.read(cx);
            assert!(
                crate::language::code_highlighting::selected_in(
                    &app.providers,
                    &app.language_for_path(&path)
                )
                .is_some(),
                "F12 must retain source language resources"
            );
        });
        assert!(
            actor
                .state
                .lock()
                .unwrap()
                .entries
                .iter()
                .all(|entry| entry.manifest.id != "typescript")
        );
    }
    shutdown(&fixture, visual);
    // Optional external evidence retains the same installed inputs for ordinary Windows UI checks.
    // Refuse an existing destination so acceptance cannot overwrite another profile or project.
    if let Some(destination) = std::env::var_os("NANOBUG_VUE_JS_EVIDENCE") {
        let destination = PathBuf::from(destination);
        assert!(
            !destination.exists(),
            "native evidence destination must be new"
        );
        copy_library(&fixture.workspace, &destination.join("workspace"));
        copy_library(&fixture.store, &destination.join("profile/runtime"));
    }
}
