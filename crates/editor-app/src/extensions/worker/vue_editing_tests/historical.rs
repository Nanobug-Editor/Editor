//! Fixed historical declaration forms cross actual Package language services and native transactions.
//! The editor owns all mutable source and history; temporary fixtures retain only original disk bytes.
use super::*;
use gpui_kit::EntityInputHandler as _;

mod completion;
mod fixtures;
mod transactions;
use fixtures::{Projects, SAMPLES};

/// Each declared class/JSX/TSX combination must apply and undo actual mapped edits, not just emit a proposal.
#[gpui::test]
#[ignore = "set actual NANOBUG_TYPESCRIPT_PACKAGE/NANOBUG_VUE_PACKAGE and NANOBUG_VUE_FIXTURES; eight historical native transactions"]
fn historical_vue_forms_apply_native_rename_and_resolved_auto_import_with_undo(
    cx: &mut TestAppContext,
) {
    let projects = Projects::prepare();
    let (fixture, visual) = setup(cx, &["NANOBUG_TYPESCRIPT_PACKAGE", "NANOBUG_VUE_PACKAGE"]);
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "historical Vue catalog",
        30,
        |app, _| {
            app.providers
                .language_servers()
                .get("vue")
                .is_some_and(Option::is_some)
        },
    );
    for sample in SAMPLES {
        eprintln!(
            "historical native phase: {} / {} begin",
            sample.version, sample.kind
        );
        let directory = projects.directory(sample);
        let child = directory.join("Child.vue").canonicalize().unwrap();
        let parent = directory.join("App.vue").canonicalize().unwrap();
        let child_disk = std::fs::read_to_string(&child).unwrap();
        let parent_disk = std::fs::read_to_string(&parent).unwrap();
        let child_original = format!("<!-- unsaved 中文😀e\u{301} -->\r\n{child_disk}");
        open(&fixture, &child, visual);
        replace(&fixture, &child_original, visual);
        ready(&fixture, &child, visual);
        assert_eq!(source(&fixture, &child, visual), child_original);
        assert!(
            visual.update(|_, cx| !fixture
                .app
                .read(cx)
                .tabs
                .iter()
                .any(|tab| tab.path() == parent)),
            "each historical row must discover its own unopened parent"
        );
        transactions::rename(
            &fixture,
            &child,
            &parent,
            &child_original,
            &parent_disk,
            visual,
        );
        completion::accept(
            &fixture,
            &child,
            &parent,
            &child_original,
            &parent_disk,
            visual,
        );
        assert_eq!(std::fs::read_to_string(&child).unwrap(), child_disk);
        assert_eq!(std::fs::read_to_string(&parent).unwrap(), parent_disk);
        eprintln!(
            "historical native phase: {} / {} PASS preview, cancel, apply, group Undo, selected resolve+import and single Undo",
            sample.version, sample.kind
        );
    }
    let plans = fixture
        .actor
        .state
        .lock()
        .unwrap()
        .language_services
        .values()
        .filter_map(|plan| plan.as_ref().ok().cloned())
        .collect::<Vec<_>>();
    assert!(
        !plans.is_empty(),
        "historical semantics must use actual managed services"
    );
    shutdown(&fixture, visual);
    assert!(
        plans
            .iter()
            .all(|plan| !plan.is_active() && plan.process_count() == 0),
        "historical native contexts must retire after checked shutdown"
    );
    projects.assert_unchanged();
}

/// Open the normal native session and restore source focus before a user language action.
fn open(fixture: &Fixture, path: &Path, visual: &mut VisualTestContext) {
    visual.update(|window, cx| {
        window.activate_window();
        fixture
            .app
            .update(cx, |app, cx| app.open_file(path.to_owned(), window, cx));
        fixture
            .app
            .read(cx)
            .editor
            .clone()
            .update(cx, |editor, cx| editor.focus(window, cx));
        window.draw(cx).clear(cx);
    });
}

/// One native replacement makes the original source dirty without writing a second text buffer.
fn replace(fixture: &Fixture, value: &str, visual: &mut VisualTestContext) {
    visual.update(|window, cx| {
        fixture
            .app
            .read(cx)
            .editor
            .clone()
            .update(cx, |editor, cx| {
                let length = editor.text().len();
                editor.set_selected_range(0..length, cx);
                editor.replace_text_in_range(None, value, window, cx);
                editor.focus(window, cx);
                cx.notify();
            });
    });
}

/// Readiness comes from the selected real adapter, not a warm manual call to tsserver.
fn ready(fixture: &Fixture, path: &Path, visual: &mut VisualTestContext) {
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "historical native primary",
        180,
        |app, cx| {
            for state in app.language_service_states.values() {
                if let crate::app::language_servers::ServiceLoadState::Failed(error) = state {
                    panic!("historical source preparation failed: {error}");
                }
            }
            app.language_server_for_path(path)
                .is_some_and(|server| server.is_active())
                && app.editor.read(cx).lsp().definition_provider.is_some()
                && app.editor.read(cx).lsp().completion_provider.is_some()
        },
    );
}

/// Select original UTF-8 source coordinates through EditorState before the actual user command.
fn caret(fixture: &Fixture, range: std::ops::Range<usize>, visual: &mut VisualTestContext) {
    visual.update(|window, cx| {
        fixture
            .app
            .read(cx)
            .editor
            .clone()
            .update(cx, |editor, cx| {
                editor.set_selected_range(range, cx);
                editor.focus(window, cx);
                cx.notify();
            });
        window.draw(cx).clear(cx);
    });
}

/// Both necessary targets must reach one complete native postimage; partial application cannot pass.
fn await_sources(
    fixture: &Fixture,
    phase: &'static str,
    child: &Path,
    parent: &Path,
    child_text: &str,
    parent_text: &str,
    visual: &mut VisualTestContext,
) {
    use sha2::{Digest as _, Sha256};
    // Only log fingerprints survive between polls; these are observations, never editable state.
    let previous = std::cell::Cell::new(None::<[u8; 32]>);
    let remaining = std::cell::Cell::new(4usize);
    eprintln!(
        "historical source phase {phase}: child={}, parent={}",
        child.display(),
        parent.display()
    );
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        phase,
        15,
        |app, cx| {
            let mut matched = true;
            let mut details = String::new();
            for (path, expected) in [(child, child_text), (parent, parent_text)] {
                let observed = app
                    .tabs
                    .iter()
                    .enumerate()
                    .find(|(_, tab)| tab.path() == path)
                    .and_then(|(index, tab)| {
                        let text = tab.text.as_ref()?;
                        Some((
                            text.editor.read(cx).text().to_string(),
                            text.editor.entity_id(),
                            app.plugin_document_version(index),
                        ))
                    });
                matched &= observed
                    .as_ref()
                    .is_some_and(|(text, _, _)| text == expected);
                if remaining.get() > 0 {
                    let (actual, authority) = observed.as_ref().map_or_else(
                        || ("<closed>".to_owned(), "<no native entity>".to_owned()),
                        |(text, editor, version)| {
                            (
                                text.chars().take(4096).collect(),
                                format!("editor={editor:?}, version={version:?}"),
                            )
                        },
                    );
                    details.push_str(&format!(
                        "path={}, {authority}, expected={:?}, actual={actual:?}\n",
                        path.display(),
                        expected.chars().take(4096).collect::<String>()
                    ));
                }
            }
            if !matched && remaining.get() > 0 {
                let fingerprint: [u8; 32] = Sha256::digest(details.as_bytes()).into();
                if previous.replace(Some(fingerprint)) != Some(fingerprint) {
                    remaining.set(remaining.get() - 1);
                    eprintln!("historical source mismatch {phase}: {details}");
                }
            }
            matched
        },
    );
    eprintln!("historical source phase {phase}: exact sources PASS");
}
