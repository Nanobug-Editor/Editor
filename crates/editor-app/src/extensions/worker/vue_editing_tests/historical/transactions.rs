//! Native preview and its explicit whole-batch Undo observe every necessary historical source edit.
use super::*;

/// Initiate from the declaration form's own script use, then inspect both original-source native sessions.
pub(super) fn rename(
    fixture: &Fixture,
    child: &Path,
    parent: &Path,
    child_original: &str,
    parent_original: &str,
    visual: &mut VisualTestContext,
) {
    let expression = if child_original.contains("this.title") {
        "this.title"
    } else {
        "props.title"
    };
    let offset = child_original
        .find(expression)
        .expect("real historical script property use")
        + expression.find("title").unwrap()
        + 2;
    for apply in [false, true] {
        open(fixture, child, visual);
        ready(fixture, child, visual);
        caret(fixture, offset..offset, visual);
        visual.simulate_keystrokes("f2");
        control(fixture, visual, "editor-rename-prompt");
        visual.simulate_input("caption");
        visual.simulate_keystrokes("enter");
        control(fixture, visual, "workspace-plan-confirm");
        assert!(
            visual.debug_bounds("workspace-plan-details").is_some(),
            "historical rename requires its native full preview"
        );
        await_sources(
            fixture,
            if apply {
                "historical confirmed preview unchanged"
            } else {
                "historical cancelled preview unchanged"
            },
            child,
            parent,
            child_original,
            parent_original,
            visual,
        );
        assert_eq!(
            source(fixture, child, visual),
            child_original,
            "preview cannot mutate the historical script"
        );
        assert_eq!(
            source(fixture, parent, visual),
            parent_original,
            "preview cannot mutate the imported component use"
        );
        click(
            visual,
            if apply {
                "workspace-plan-confirm"
            } else {
                "workspace-plan-cancel"
            },
        );
        control_visible(fixture, visual, "workspace-plan-confirm", false);
        if !apply {
            await_sources(
                fixture,
                "historical preview cancellation unchanged",
                child,
                parent,
                child_original,
                parent_original,
                visual,
            );
        }
    }
    let child_expected = child_original.replace("title", "caption");
    let parent_expected = parent_original.replace("title=", "caption=");
    await_sources(
        fixture,
        "historical complete rename postimage",
        child,
        parent,
        &child_expected,
        &parent_expected,
        visual,
    );
    assert_eq!(
        source(fixture, child, visual),
        child_expected,
        "every class/render declaration and source use must rename together"
    );
    assert_eq!(
        source(fixture, parent, visual),
        parent_expected,
        "the imported component attribute is a necessary target"
    );
    super::super::super::vue_test_support::receipts::undo(
        fixture,
        visual,
        "historical receipt native frame",
    );
    await_sources(
        fixture,
        "historical group Undo exact originals",
        child,
        parent,
        child_original,
        parent_original,
        visual,
    );
    assert_eq!(source(fixture, child, visual), child_original);
    assert_eq!(source(fixture, parent, visual), parent_original);
}
