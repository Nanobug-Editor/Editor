//! Editor entry-point regressions for the independent native grid, find/Vi and split session routing.

use super::*;

/// Read a particular native leaf's submitted drawing, independent from which pane has focus.
fn pane_text(app: &Entity<EditorApp>, id: u64, visual: &mut VisualTestContext) -> String {
    visual.update(|_, cx| {
        app.read(cx)
            .terminal
            .read(cx)
            .grids
            .get(&id)
            .map(|grid| {
                grid.read(cx)
                    .drawing
                    .paint
                    .iter()
                    .filter_map(|paint| match paint {
                        GridPaint::Text { text, .. } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    })
}

/// Use the configured Shell's normal input path for test markers, never a parser-only output injection.
fn output(visual: &mut VisualTestContext, marker: &str) {
    visual.simulate_input(&if cfg!(windows) {
        format!("Write-Output ('{marker}')")
    } else {
        format!("printf '{marker}\\n'")
    });
    visual.simulate_keystrokes("enter");
}

/// Search/Vi use native controls while raw Shell input resumes without leaking navigation characters.
#[gpui::test]
fn native_find_vi_and_split_panes_keep_session_ownership(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        typography::init(cx);
        apply_theme(builtin_theme(false), cx);
        cx.set_reduce_motion(true);
    });
    let directory = tempfile::tempdir().unwrap();
    let (app, visual) = super::restore::open(cx, Workspace::open(directory.path()).unwrap());
    let toggle = visual.debug_bounds("terminal-toggle").unwrap();
    visual.simulate_click(toggle.center(), Default::default());
    wait(visual, &app, |visual| {
        painted(&app, visual).contains(if cfg!(windows) { "PS" } else { "$" })
    });
    output(visual, "find-中文-needle");
    wait(visual, &app, |visual| {
        painted(&app, visual).contains("find-中文-needle")
    });
    let find = visual.debug_bounds("native-terminal-find").unwrap();
    visual.simulate_click(find.center(), Default::default());
    visual.run_until_parked();
    assert!(visual.debug_bounds("terminal-find-query").is_some());
    visual.simulate_input("中文-needle");
    visual.run_until_parked();
    visual.update(|_, cx| {
        let terminal = app.read(cx).terminal.read(cx);
        assert!(terminal.sessions[0].engine.search.current.is_some());
    });
    visual.simulate_keystrokes("enter shift-enter escape");
    visual.run_until_parked();
    let vi = visual.debug_bounds("native-terminal-vi").unwrap();
    visual.simulate_click(vi.center(), Default::default());
    visual.run_until_parked();
    visual.simulate_keystrokes("0 v l l l l ctrl-c");
    visual.update(|_, cx| {
        assert!(
            cx.read_from_clipboard()
                .and_then(|item| item.text())
                .is_some()
        );
    });
    visual.simulate_keystrokes("i");
    output(visual, "after-vi-input");
    wait(visual, &app, |visual| {
        painted(&app, visual).contains("after-vi-input")
    });
    let menu = visual.debug_bounds("native-terminal-menu").unwrap();
    visual.simulate_click(menu.center(), Default::default());
    visual.run_until_parked();
    let split = visual.debug_bounds("native-menu-split-columns").unwrap();
    visual.simulate_click(split.center(), Default::default());
    wait(visual, &app, |visual| {
        visual.debug_bounds("terminal-pane-2").is_some()
            && pane_text(&app, 2, visual).contains(if cfg!(windows) { "PS" } else { "$" })
    });
    output(visual, "second-pane-only");
    wait(visual, &app, |visual| {
        pane_text(&app, 2, visual).contains("second-pane-only")
    });
    let first = visual.debug_bounds("terminal-pane-1").unwrap();
    visual.simulate_click(first.center(), Default::default());
    output(visual, "first-pane-only");
    wait(visual, &app, |visual| {
        pane_text(&app, 1, visual).contains("first-pane-only")
    });
    assert!(!pane_text(&app, 1, visual).contains("second-pane-only"));
    assert!(!pane_text(&app, 2, visual).contains("first-pane-only"));
    for (width, height) in [(950., 700.), (1300., 900.), (1100., 800.)] {
        visual.simulate_resize(size(px(width), px(height)));
        visual.run_until_parked();
        visual.update(|window, cx| window.draw(cx).clear(cx));
    }
    wait(visual, &app, |visual| {
        pane_text(&app, 1, visual).contains("first-pane-only")
            && pane_text(&app, 2, visual).contains("second-pane-only")
    });
    visual.update(|_, cx| {
        let terminal = app.read(cx).terminal.clone();
        terminal.update(cx, |terminal, _| terminal.save().unwrap());
    });
    visual.update(|window, _| window.remove_window());
    drop(app);
    cx.run_until_parked();
    let (app, visual) = super::restore::open(cx, Workspace::open(directory.path()).unwrap());
    wait(visual, &app, |visual| {
        visual.debug_bounds("terminal-pane-1").is_some()
            && visual.debug_bounds("terminal-pane-2").is_some()
    });
    assert!(pane_text(&app, 1, visual).contains("first-pane-only"));
    assert!(pane_text(&app, 2, visual).contains("second-pane-only"));
    visual.update(|_, cx| {
        assert!(
            app.read(cx)
                .terminal
                .read(cx)
                .sessions
                .iter()
                .all(|session| !session.launched)
        )
    });
}
