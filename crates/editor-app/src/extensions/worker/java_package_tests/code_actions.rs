//! Java fixes use the published native action menu and ordinary revision-bound workspace preview.
use super::super::vue_test_support::Fixture;
use super::rename::{click, wait_control};
use super::*;
use gpui_kit::{EntityInputHandler as _, VisualTestContext};

/// A real Java type correction is selected from native rows and cannot apply before confirmation.
#[gpui::test]
#[ignore = "Build Java with --plugin-package and set NANOBUG_JAVA_PACKAGE to the resulting ZIP"]
fn java_package_quick_fix_uses_native_action_preview(cx: &mut TestAppContext) {
    let workspace = tempfile::tempdir().unwrap();
    let source = "public class Main { void test() { int number = \"wrong\"; } }\n";
    std::fs::write(workspace.path().join("Main.java"), source).unwrap();
    let (fixture, visual) =
        setup_for_workspace(cx, &["NANOBUG_JAVA_PACKAGE"], workspace.path().into());
    let path = fixture.workspace.join("Main.java");
    visual.update(|window, cx| {
        fixture
            .app
            .update(cx, |app, cx| app.open_file(path.clone(), window, cx));
        window.draw(cx).clear(cx);
    });
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Java diagnostic for native fix",
        180,
        |app, cx| {
            app.editor.read(cx).diagnostics().is_some_and(|items| {
                items
                    .iter()
                    .any(|item| item.message.contains("Type mismatch"))
            })
        },
    );
    select(&fixture, visual, source.find("\"wrong\"").unwrap() + 2);
    visual.simulate_keystrokes("ctrl-.");
    wait_control(&fixture, visual, "language-feature-row-0", 60);
    choose(visual, |label| {
        label.contains("Change type") && label.contains("String")
    });
    wait_control(&fixture, visual, "workspace-plan-confirm", 30);
    assert_eq!(text(&fixture, visual), source);
    click(visual, "workspace-plan-confirm");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Java type correction applied",
        30,
        |app, cx| {
            app.editor
                .read(cx)
                .text()
                .to_string()
                .contains("String number")
        },
    );
    assert_eq!(
        text(&fixture, visual),
        source.replace("int number", "String number")
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    let imports = "import java.util.ArrayList;\nimport java.util.List;\n\npublic class Main { List<String> values; }\n";
    visual.update(|window, cx| {
        fixture
            .app
            .update(cx, |app, cx| app.open_file(path.clone(), window, cx));
    });
    visual.simulate_keystrokes("ctrl-a");
    visual.simulate_input(imports);
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Java import source diagnostic",
        30,
        |app, cx| {
            app.editor.read(cx).diagnostics().is_some_and(|items| {
                items.iter().any(|item| {
                    item.message.contains("ArrayList") && item.message.contains("never used")
                })
            })
        },
    );
    select(&fixture, visual, imports.find("Main").unwrap() + 1);
    visual.simulate_keystrokes("ctrl-.");
    wait_control(&fixture, visual, "language-feature-row-0", 60);
    choose(visual, |label| label.contains("Organize imports"));
    wait_control(&fixture, visual, "workspace-plan-confirm", 30);
    assert_eq!(text(&fixture, visual), imports);
    click(visual, "workspace-plan-confirm");
    wait(
        &fixture.app,
        &fixture.actor,
        visual,
        "Java imports organized",
        30,
        |app, cx| !app.editor.read(cx).text().to_string().contains("ArrayList"),
    );
    let organized = text(&fixture, visual);
    assert!(organized.contains("import java.util.List;"));
    assert!(organized.contains("List<String> values"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    shutdown(&fixture, visual);
}

/// Native input keeps focus and the sole document value; the action reads its actual byte caret.
fn select(fixture: &Fixture, visual: &mut VisualTestContext, offset: usize) {
    visual.update(|window, cx| {
        fixture
            .app
            .read(cx)
            .editor
            .clone()
            .update(cx, |editor, cx| {
                editor.set_selected_range(offset..offset, cx);
                editor.focus(window, cx);
            });
    });
}

/// Inspect Base's painted accessible rows, preserving provider ordering instead of assuming row zero.
fn choose(visual: &mut VisualTestContext, accepts: impl Fn(&str) -> bool) {
    let rows = visual.update(|window, _| {
        gpui_base::test_support::snapshots(window)
            .iter()
            .filter_map(|item| {
                let id = item.path().iter().find_map(|part| match part {
                    gpui_kit::ElementId::Name(name)
                        if name.starts_with("language-feature-row-") =>
                    {
                        Some(name.to_string())
                    }
                    _ => None,
                })?;
                Some((id, item.label()?.to_owned()))
            })
            .collect::<Vec<_>>()
    });
    let (selector, _) = rows
        .iter()
        .find(|(_, label)| accepts(label))
        .unwrap_or_else(|| panic!("Expected Java action was not painted: {rows:?}"));
    // Native keyboard navigation scrolls to rows beyond the initial viewport before choosing.
    let index = selector
        .rsplit('-')
        .next()
        .unwrap()
        .parse::<usize>()
        .unwrap();
    for _ in 0..index {
        visual.simulate_keystrokes("down");
        visual.run_until_parked();
        visual.update(|window, cx| window.draw(cx).clear(cx));
    }
    // Base buttons activate on release. Keep the same complete native key sequence as a user.
    let keystroke = gpui_kit::Keystroke::parse("enter").unwrap();
    visual.simulate_event(gpui_kit::KeyDownEvent {
        keystroke: keystroke.clone(),
        is_held: false,
        prefer_character_input: false,
    });
    visual.simulate_event(gpui_kit::KeyUpEvent { keystroke });
    visual.run_until_parked();
}

/// Observe the active native editor after the real UI delivery path has run.
fn text(fixture: &Fixture, visual: &mut VisualTestContext) -> String {
    visual.update(|_, cx| fixture.app.read(cx).editor.read(cx).text().to_string())
}
