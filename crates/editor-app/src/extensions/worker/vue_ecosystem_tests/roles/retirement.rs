//! Pending mapped proposals remain tied to their originating contribution set and native view.
use super::*;

/// Keep a real F2 proposal unconfirmed while its separately installed contributor changes.
pub(super) fn pending_mapped_rename(
    fixture: &Fixture,
    path: &Path,
    symbol: &str,
    visual: &mut VisualTestContext,
) {
    caret(fixture, SOURCE.rfind(symbol).unwrap() + 1, visual);
    visual.simulate_keystrokes("f2");
    control(fixture, visual, "editor-rename-prompt", true);
    visual.simulate_input(&format!("retired_{symbol}"));
    visual.simulate_keystrokes("enter");
    control(fixture, visual, "workspace-plan-confirm", true);
    assert!(visual.update(|_, cx| mapping::sources_equal(
        fixture.app.read(cx),
        cx,
        path,
        SOURCE,
        STATE_SOURCE
    )));
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        "<template><p>saved</p></template>\r\n"
    );
    assert_eq!(
        std::fs::read_to_string(path.parent().unwrap().join("state.ts")).unwrap(),
        STATE_SOURCE
    );
}

/// The native modal must withdraw before any new Enter can consent to an obsolete mapping.
/// Fresh mapped rename later in the scenario proves that refusal did not disable the whole editor.
pub(super) fn assert_pending_mapping_withdrawn(
    fixture: &Fixture,
    path: &Path,
    visual: &mut VisualTestContext,
) {
    control(fixture, visual, "workspace-plan-confirm", false);
    assert!(visual.debug_bounds("workspace-plan-cancel").is_none());
    visual.simulate_keystrokes("enter");
    // Closing a preview may restore source focus. Undo any ordinary newline Enter produced;
    // neither that input nor its local Undo may expose a retired mapped rename as a side effect.
    let current = visual.update(|_, cx| fixture.app.read(cx).editor.read(cx).text().to_string());
    if current != SOURCE {
        assert!(
            !current.contains("retired_"),
            "obsolete mapped proposal applied: {current}"
        );
        visual.simulate_keystrokes("ctrl-z");
    }
    assert_eq!(
        visual.update(|_, cx| fixture.app.read(cx).editor.read(cx).text().to_string()),
        SOURCE
    );
    assert!(visual.update(|_, cx| mapping::sources_equal(
        fixture.app.read(cx),
        cx,
        path,
        SOURCE,
        STATE_SOURCE
    )));
    assert!(
        !visual
            .update(|_, cx| fixture.app.read(cx).editor.read(cx).text().to_string())
            .contains("retired_")
    );
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        "<template><p>saved</p></template>\r\n"
    );
    assert_eq!(
        std::fs::read_to_string(path.parent().unwrap().join("state.ts")).unwrap(),
        STATE_SOURCE
    );
}
