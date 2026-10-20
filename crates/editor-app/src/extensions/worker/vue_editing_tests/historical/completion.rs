//! Actual native completion selection must resolve its mapped import and main edit as one Undo unit.
use super::*;

/// A distinctive helper name leaves one real module candidate; accepting it runs the production resolver.
pub(super) fn accept(
    fixture: &Fixture,
    child: &Path,
    parent: &Path,
    original: &str,
    parent_original: &str,
    visual: &mut VisualTestContext,
) {
    const PREFIX: &str = "formatHistoricalTit";
    const NAME: &str = "formatHistoricalTitle";
    open(fixture, child, visual);
    ready(fixture, child, visual);
    let token = original.find("const completionSample = null;").unwrap()
        + "const completionSample = ".len();
    caret(fixture, token..token + "null".len(), visual);
    assert_input_target(fixture, child, token..token + "null".len(), visual);
    visual.simulate_input(PREFIX);
    eprintln!(
        "historical native completion input entered; linked queue pending={}",
        visual.update(|_, cx| fixture
            .app
            .read(cx)
            .language_edits
            .bridge
            .as_ref()
            .is_some_and(|bridge| bridge.read(cx).has_pending()))
    );
    let typed = original.replacen(
        "const completionSample = null;",
        &format!("const completionSample = {PREFIX};"),
        1,
    );
    // The production linked-input bridge may enqueue platform input while its semantic query
    // finishes. Observe the same native sessions under the existing 15s transaction guard;
    // simulate_input returning is not proof that the original editor has replayed the queue.
    await_sources(
        fixture,
        "historical queued completion input",
        child,
        parent,
        &typed,
        parent_original,
        visual,
    );
    assert_eq!(source(fixture, child, visual), typed);
    eprintln!("historical native completion input observed in the original source");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "historical module completion candidate",
        30,
        |app, cx| {
            let state = app.editor.read(cx);
            let menu = state.completion_menu_state();
            menu.open && menu.items.first().is_some_and(|item| item.label == NAME)
        },
    );
    control(fixture, visual, "editor-completion-row-0");
    // Enter selects the painted Base row. The harness never calls resolve or applies its edits.
    visual.simulate_keystrokes("enter");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "historical selected completion with required import",
        15,
        |app, cx| {
            let text = app.editor.read(cx).text().to_string();
            text.contains(&format!("const completionSample = {NAME};"))
                && text.lines().any(|line| {
                    line.contains("import") && line.contains(NAME) && line.contains("helpers")
                })
        },
    );
    let inserted = source(fixture, child, visual);
    assert!(inserted.starts_with("<!-- unsaved 中文😀e\u{301} -->\r\n<script lang="));
    assert!(
        inserted.ends_with("<template><p>{{ title }}</p></template>\r\n"),
        "auto-import must stay within the original script"
    );
    assert!(
        !inserted.contains("__VLS_") && !inserted.contains("__NANOBUG"),
        "virtual helper edits cannot leak into source"
    );
    assert_eq!(
        source(fixture, parent, visual),
        parent_original,
        "completion does not rewrite the other component"
    );
    visual.simulate_keystrokes("ctrl-z");
    await_sources(
        fixture,
        "historical resolved completion single Undo",
        child,
        parent,
        &typed,
        parent_original,
        visual,
    );
    assert_eq!(
        source(fixture, child, visual),
        typed,
        "one local Undo must remove both import and completed main edit, preserving typed prefix"
    );
    // Restore this row through the same native input handler before another form uses the project graph.
    replace(fixture, original, visual);
    assert_eq!(source(fixture, child, visual), original);
}

/// A prior preview/Undo must hand native input back to this tab's current editor, not a popover.
fn assert_input_target(
    fixture: &Fixture,
    path: &Path,
    selection: std::ops::Range<usize>,
    visual: &mut VisualTestContext,
) {
    visual.update(|window, cx| {
        let app = fixture.app.read(cx);
        let tab = app.tabs.iter().find(|tab| tab.path() == path).unwrap();
        assert_eq!(app.active_path.as_deref(), Some(path));
        assert!(
            tab.owns_editor(&app.editor),
            "input target is a different native entity"
        );
        assert!(
            app.editor.focus_handle(cx).is_focused(window),
            "source has not regained native focus"
        );
        assert_eq!(app.editor.read(cx).selected_range(), selection);
        eprintln!(
            "historical completion target: path={}, editor={:?}, focus=true, selection={selection:?}",
            path.display(),
            app.editor.entity_id()
        );
    });
}
