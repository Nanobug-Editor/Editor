//! An independently installed unknown contribution changes native Vue analysis through public APIs.
//! Candidate archives are explicit inputs; the production actor owns preparation, imports and teardown.
use super::vue_test_support::{input, setup, shutdown, wait};
use super::*;
use crate::*;
use gpui_kit::{TestAppContext, gpui};
use std::cell::RefCell;

mod roles;

/// This is a contributor, not another consumer: its exported compiler hook produces TS 2345 from
/// source which the unextended Vue backend accepts. Disabling it must remove precisely that effect.
#[gpui::test]
#[ignore = "set NANOBUG_TYPESCRIPT_PACKAGE, NANOBUG_VUE_PACKAGE, NANOBUG_ECOSYSTEM_PACKAGE and NANOBUG_VUE_FIXTURES"]
fn unknown_ecosystem_package_contributes_and_retires_native_mapped_analysis(
    cx: &mut TestAppContext,
) {
    let (fixture, visual) = setup(
        cx,
        &[
            "NANOBUG_TYPESCRIPT_PACKAGE",
            "NANOBUG_VUE_PACKAGE",
            "NANOBUG_ECOSYSTEM_PACKAGE",
        ],
    );
    let app = fixture.app.clone();
    let actor = fixture.actor.clone();
    let path = fixture
        .workspace
        .join("tests/projects/vue3/App.vue")
        .canonicalize()
        .unwrap();
    let disk = std::fs::read_to_string(&path).unwrap();
    let source = "<script setup lang=\"ts\">\n// unsaved 中文😀\nconst eco = 42;\n</script>\n<template>{{eco}}</template>\n";
    let ecosystem = Package::read(&input("NANOBUG_ECOSYSTEM_PACKAGE"))
        .unwrap()
        .manifest
        .id;
    wait(&app, &actor, visual, "cold registry", 30, |app, cx| {
        let language = app.language_for_path(&path);
        let _ = cx;
        app.providers
            .language_servers()
            .get(&language)
            .is_some_and(Option::is_some)
    });
    visual.update(|window, cx| {
        window.activate_window();
        app.update(cx, |app, cx| app.open_file(path.clone(), window, cx));
        window.draw(cx).clear(cx);
    });
    visual.simulate_keystrokes("ctrl-a");
    visual.simulate_input(source);
    assert_eq!(
        visual.update(|_, cx| app.read(cx).editor.read(cx).text().to_string()),
        source
    );
    wait(
        &app,
        &actor,
        visual,
        "contributed native provider",
        180,
        |app, cx| {
            assert_preparation(app, &actor);
            app.language_server_for_path(&path)
                .is_some_and(|server| server.is_active())
                && app.editor.read(cx).lsp().hover_provider.is_some()
        },
    );
    eprintln!("S4: waiting for actual contributed diagnostic on native EditorState");
    wait(&app, &actor, visual, "contributed 2345", 30, |app, cx| {
        assert_preparation(app, &actor);
        has_ecosystem_error(app, cx)
    });
    let (old, old_document, old_plan) = visual.update(|_, cx| {
        let app = app.read(cx);
        let diagnostic = app
            .editor
            .read(cx)
            .diagnostics()
            .unwrap()
            .iter()
            .find(|item| {
                item.message.contains("Argument of type") && item.message.contains("string")
            })
            .unwrap()
            .clone();
        // Base stores UTF-8 byte ranges; the earlier emoji deliberately separates bytes from the
        // LSP UTF-16 coordinates. Verify the exact original source token after native conversion.
        assert_eq!(&source[diagnostic.range.clone()], "42");
        assert_eq!(diagnostic.range.start, source.find("42").unwrap());
        let position =
            crate::language::navigation::position_at_byte(source, diagnostic.range.start);
        assert_eq!((position.line, position.character), (2, 12));
        let server = app.language_server_for_path(&path).unwrap().clone();
        let uri = crate::language::navigation::file_uri(&path).unwrap();
        let document = server.document(&uri).unwrap();
        let state = actor.state.lock().unwrap();
        let plan = state
            .language_document_bindings
            .iter()
            .find(|((provider, _), _)| provider == "vue/vue")
            .map(|(_, key)| state.language_services[key].as_ref().unwrap().clone())
            .unwrap();
        assert_eq!(plan.owner, "typescript");
        assert_eq!(plan.process_count(), 1);
        (server, document, plan)
    });
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        disk,
        "analysis must not save the generated source"
    );
    actor.tx.send(Work::Disable(ecosystem.clone())).unwrap();
    wait(
        &app,
        &actor,
        visual,
        "old module worker retired",
        30,
        |_, _| !old_plan.is_active(),
    );
    assert!(!old.is_active());
    assert_eq!(old_plan.process_count(), 0);
    assert!(
        old.diagnostics_for(old_document, source).is_err(),
        "old analysis cannot publish after module revocation"
    );
    eprintln!("S4: recovering the ordinary Vue backend with an empty contribution set");
    wait(
        &app,
        &actor,
        visual,
        "unextended Vue recovery",
        120,
        |app, cx| {
            app.language_server_for_path(&path)
                .is_some_and(|server| server.is_active() && !Arc::ptr_eq(server, &old))
                && app.editor.read(cx).lsp().hover_provider.is_some()
                && !has_ecosystem_error(app, cx)
        },
    );
    // Await an actual semantic response from the replacement provider. An empty marker list alone
    // could merely mean the old publication was cleared before the new compiler performed analysis.
    let hover = visual.update(|window, cx| {
        let app = app.read(cx);
        let editor = app.editor.read(cx);
        let rope = editor.text().clone();
        editor.lsp().hover_provider.clone().unwrap().hover(
            &rope,
            source.rfind("eco").unwrap() + 1,
            window,
            cx,
        )
    });
    let result = Rc::new(RefCell::new(None));
    let delivered = result.clone();
    visual.update(|_, cx| {
        app.update(cx, |_, cx| {
            cx.spawn(async move |_, _| {
                *delivered.borrow_mut() = Some(hover.await);
            })
            .detach()
        })
    });
    wait(
        &app,
        &actor,
        visual,
        "recovered actual hover",
        30,
        |_, _| result.borrow().is_some(),
    );
    assert!(
        result
            .borrow()
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap()
            .is_some()
    );
    assert!(!visual.update(|_, cx| has_ecosystem_error(app.read(cx), cx)));
    assert_eq!(
        visual.update(|_, cx| app.read(cx).editor.read(cx).text().to_string()),
        source
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), disk);
    // Re-enable through the normal public lifecycle action: a new collection epoch must reconstruct
    // the composition and restore the same mapped type contribution without touching source text.
    actor.tx.send(Work::Enable(ecosystem)).unwrap();
    wait(
        &app,
        &actor,
        visual,
        "contributor re-enabled",
        180,
        |app, cx| has_ecosystem_error(app, cx),
    );
    shutdown(&fixture, visual);
}

/// Observe only diagnostics already admitted to Base's native document model.
fn has_ecosystem_error(app: &EditorApp, cx: &gpui::App) -> bool {
    app.editor.read(cx).diagnostics().is_some_and(|items| {
        items.iter().any(|item| {
            item.message.contains("Argument of type") && item.message.contains("string")
        })
    })
}

/// Fail at the preparation boundary, preserving the actionable failure instead of waiting on a
/// diagnostic that an inactive provider cannot produce. Retirement waits deliberately omit this.
fn assert_preparation(app: &EditorApp, actor: &Worker) {
    for (key, state) in &app.language_service_states {
        if let crate::app::language_servers::ServiceLoadState::Failed(error) = state {
            panic!("S4 native preparation {key}: {error}");
        }
    }
    let state = actor.state.lock().unwrap();
    for entry in &state.entries {
        assert!(
            entry.error.is_none(),
            "S4 package {} failed: {:?}; startup={:?}; bindings={:?}",
            entry.manifest.id,
            entry.error,
            state.startup,
            state.language_document_bindings
        );
    }
}
