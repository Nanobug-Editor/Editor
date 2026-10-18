//! Problems' rendered grouping, navigation and scrolling consume real canonical diagnostic publications.
use super::*;
use gpui_kit::{KeyDownEvent, KeyUpEvent, Keystroke, VisualTestContext};

/// Settle and paint production entities after each native input or model publication.
fn draw(visual: &mut VisualTestContext) {
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
}

/// File-group headers and the status entry are checked through their native accessible names.
fn visible_label(visual: &mut VisualTestContext, label: &str) -> bool {
    visual.update(|window, _| {
        gpui_base::test_support::snapshots(window)
            .iter()
            .any(|node| node.visible() && node.label() == Some(label))
    })
}

/// Base activation needs both platform events, including release, for Enter and Space.
fn activate(visual: &mut VisualTestContext, key: &str) {
    let keystroke = Keystroke::parse(key).unwrap();
    visual.simulate_event(KeyDownEvent {
        keystroke: keystroke.clone(),
        is_held: false,
        prefer_character_input: false,
    });
    visual.simulate_event(KeyUpEvent { keystroke });
    draw(visual);
}

/// Observe the actual disclosure button instead of inferring expansion from retained state.
fn file_expanded(visual: &mut VisualTestContext, label: &str) -> bool {
    visual.update(|window, _| {
        gpui_base::test_support::snapshots(window)
            .iter()
            .find(|node| {
                node.visible()
                    && node.role() == Some(gpui_kit::Role::Button)
                    && node.label() == Some(label)
            })
            .expect("File group must expose a native disclosure button")
            .expanded()
            .expect("File group must expose its expansion state")
    })
}

/// Inactive files contribute counts, row activation opens their actual editor, and replacement clears old groups.
#[gpui::test]
fn native_problems_group_files_and_refresh_inactive_diagnostics(cx: &mut TestAppContext) {
    let (app, visual, directory) = test_editor(cx, "first file\nsecond line\n");
    visual.simulate_resize(size(px(1100.), px(800.)));
    let other_path = directory.path().join("z-other.txt");
    std::fs::write(&other_path, "other file\nwarning here\n").unwrap();
    let other = other_path.clone();
    visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.open_file(other, window, cx);
            let first = app.tabs[0].text.as_ref().unwrap().editor.entity_id();
            let second = app.tabs[1].text.as_ref().unwrap().editor.entity_id();
            app.apply_semantic_diagnostics(
                first,
                vec![
                    Diagnostic::new(
                        Position::new(0, 2)..Position::new(0, 3),
                        "First file error / 第一份文件",
                    )
                    .with_source("checker-a")
                    .with_severity(DiagnosticSeverity::Error),
                ],
                cx,
            );
            app.apply_semantic_diagnostics(
                second,
                vec![
                    Diagnostic::new(
                        Position::new(1, 1)..Position::new(1, 2),
                        "Other file warning / 第二份文件",
                    )
                    .with_source("checker-b")
                    .with_severity(DiagnosticSeverity::Warning),
                ],
                cx,
            );
            app.activate_tab(0, window, cx);
            app.show_problems(window, cx);
        })
    });
    draw(visual);
    assert!(visible_label(
        visual,
        &t!("problems.file", path = "sample.txt", count = 1)
    ));
    assert!(visible_label(
        visual,
        &t!("problems.file", path = "z-other.txt", count = 1)
    ));
    assert!(visible_label(
        visual,
        &t!("problems.summary", count = 2, errors = 1, warnings = 1)
    ));
    assert!(visual.debug_bounds("native-problems-file-icon-0").is_some());
    assert!(visual.debug_bounds("native-problems-file-icon-1").is_some());
    assert!(visual.debug_bounds("problem-reveal").is_none());
    assert!(visual.debug_bounds("native-problems-actions").is_none());
    let row = visual.debug_bounds("native-problem-1").unwrap();
    visual.simulate_click(row.center(), Default::default());
    draw(visual);
    visual.update(|window, cx| {
        let app = app.read(cx);
        assert_eq!(
            app.active_path.as_deref(),
            Some(other_path.canonicalize().unwrap().as_path())
        );
        assert_eq!(app.editor.read(cx).cursor_position(), Position::new(1, 1));
        assert!(app.editor.focus_handle(cx).is_focused(window));
    });
    assert!(visual.debug_bounds("native-problems-panel").is_some());
    // Closing after cross-file navigation must not restore the now-unmounted first editor's focus.
    let close = visual.debug_bounds("problem-close").unwrap();
    visual.simulate_click(close.center(), Default::default());
    draw(visual);
    visual.update(|window, cx| {
        assert!(app.read(cx).editor.focus_handle(cx).is_focused(window));
    });
    visual.update(|window, cx| app.update(cx, |app, cx| app.show_problems(window, cx)));
    draw(visual);
    visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            let first = app.tabs[0].text.as_ref().unwrap().editor.entity_id();
            app.apply_semantic_diagnostics(first, Vec::new(), cx);
            app.activate_tab(0, window, cx);
        })
    });
    draw(visual);
    assert!(!visible_label(
        visual,
        &t!("problems.file", path = "sample.txt", count = 1)
    ));
    assert!(
        visible_label(
            visual,
            &t!("problems.summary", count = 1, errors = 0, warnings = 1)
        ),
        "An empty active file must retain another file's warning count"
    );
}

/// End accounts for intervening file headers when scrolling and Enter leaves the list available.
#[gpui::test]
fn native_problems_keyboard_scroll_and_clear(cx: &mut TestAppContext) {
    let source = (0..80)
        .map(|index| format!("line {index}\n"))
        .collect::<String>();
    let (app, visual, _directory) = test_editor(cx, &source);
    visual.simulate_resize(size(px(1100.), px(700.)));
    visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            let editor = app.tabs[0].text.as_ref().unwrap().editor.entity_id();
            app.apply_semantic_diagnostics(
                editor,
                (0..80)
                    .map(|line| {
                        Diagnostic::new(
                            Position::new(line, 0)..Position::new(line, 1),
                            format!("Warning {line} / 警告"),
                        )
                        .with_source("scroll-checker")
                        .with_severity(DiagnosticSeverity::Warning)
                    })
                    .collect(),
                cx,
            );
            app.show_problems(window, cx);
        })
    });
    draw(visual);
    visual.simulate_keystrokes("end");
    draw(visual);
    let viewport = visual.debug_bounds("native-problems-list").unwrap();
    let last = visual.debug_bounds("native-problem-79").unwrap();
    assert!(last.top() >= viewport.top() && last.bottom() <= viewport.bottom());
    activate(visual, "enter");
    assert_eq!(
        visual.update(|_, cx| app.read(cx).editor.read(cx).cursor_position()),
        Position::new(79, 0)
    );
    assert!(visual.debug_bounds("native-problems-list").is_some());
    visual.update(|_, cx| {
        app.update(cx, |app, cx| {
            let editor = app.tabs[0].text.as_ref().unwrap().editor.entity_id();
            app.apply_semantic_diagnostics(editor, Vec::new(), cx);
        })
    });
    draw(visual);
    assert!(visible_label(visual, &t!("problems.empty")));
    assert!(visual.debug_bounds("syntax-error-indicator").is_none());
    assert!(visual.debug_bounds("native-problems-actions").is_none());
}

/// Mouse and Base keyboard disclosure preserve counts, skip hidden rows and retain state on replacement.
#[gpui::test]
fn native_problems_disclosure_and_hidden_row_navigation(cx: &mut TestAppContext) {
    let (app, visual, directory) = test_editor(cx, "first file\n");
    visual.simulate_resize(size(px(1100.), px(800.)));
    let other_path = directory.path().join("z-other.json");
    std::fs::write(&other_path, "{}\n").unwrap();
    visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            app.open_file(other_path.clone(), window, cx);
            for index in 0..2 {
                let editor = app.tabs[index].text.as_ref().unwrap().editor.entity_id();
                app.apply_semantic_diagnostics(
                    editor,
                    vec![
                        Diagnostic::new(
                            Position::new(0, 0)..Position::new(0, 1),
                            format!("File {index} diagnostic"),
                        )
                        .with_severity(DiagnosticSeverity::Error),
                    ],
                    cx,
                );
            }
            app.activate_tab(0, window, cx);
            app.show_problems(window, cx);
        })
    });
    draw(visual);
    let first_label = t!("problems.file", path = "sample.txt", count = 1).to_string();
    let second_label = t!("problems.file", path = "z-other.json", count = 1).to_string();
    assert!(file_expanded(visual, &first_label));
    assert!(file_expanded(visual, &second_label));
    for index in 0..2 {
        let selector = Box::leak(format!("native-problems-file-icon-{index}").into_boxed_str());
        let icon = visual.debug_bounds(selector).unwrap();
        assert!(
            icon.size.width >= px(12.) && icon.size.height >= px(12.),
            "The shared rem-sized file artwork must have usable native bounds: {icon:?}"
        );
    }
    let header = visual.debug_bounds("native-problems-file-0").unwrap();
    visual.simulate_click(header.center(), Default::default());
    draw(visual);
    assert!(!file_expanded(visual, &first_label));
    assert!(visual.debug_bounds("native-problem-1").is_none());
    assert!(visible_label(
        visual,
        &t!("problems.summary", count = 2, errors = 2, warnings = 0)
    ));
    // Down from a collapsed header selects the next file; Enter cannot navigate its hidden marker.
    visual.simulate_keystrokes("down");
    activate(visual, "enter");
    assert_eq!(
        visual.update(|_, cx| app.read(cx).active_path.clone()),
        Some(other_path.canonicalize().unwrap())
    );
    visual.update(|_, cx| {
        app.update(cx, |app, cx| {
            let editor = app.tabs[0].text.as_ref().unwrap().editor.entity_id();
            app.apply_semantic_diagnostics(editor, Vec::new(), cx);
        })
    });
    draw(visual);
    assert!(!visible_label(visual, &first_label));
    visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            let editor = app.tabs[0].text.as_ref().unwrap().editor.entity_id();
            app.apply_semantic_diagnostics(
                editor,
                vec![
                    Diagnostic::new(
                        Position::new(0, 0)..Position::new(0, 1),
                        "Replacement in collapsed file",
                    )
                    .with_severity(DiagnosticSeverity::Error),
                ],
                cx,
            );
            app.show_problems(window, cx);
        })
    });
    draw(visual);
    assert!(!file_expanded(visual, &first_label));
    let header = visual.debug_bounds("native-problems-file-0").unwrap();
    visual.simulate_click(header.center(), Default::default());
    draw(visual);
    assert!(file_expanded(visual, &first_label));
    assert!(visual.debug_bounds("native-problem-1").is_some());
    activate(visual, "space");
    assert!(!file_expanded(visual, &first_label));
    visual.simulate_keystrokes("right");
    draw(visual);
    assert!(file_expanded(visual, &first_label));
    visual.simulate_keystrokes("left");
    draw(visual);
    activate(visual, "enter");
    assert!(file_expanded(visual, &first_label));
    check_all_collapsed_navigation(visual, &app, &first_label, &second_label);
    check_file_header_palettes_and_scale(visual, &app);
}

/// File artwork follows the shared rem size; large fonts must grow the header without overlapping its row.
fn check_file_header_palettes_and_scale(visual: &mut VisualTestContext, app: &Entity<EditorApp>) {
    for (dark, font_size) in [(false, 10.), (true, 24.), (false, 14.)] {
        visual.update(|_, cx| {
            app.update(cx, |app, cx| {
                app.dark_theme = dark;
                typography::set_font_size(cx, font_size);
                apply_theme(theme::builtin_theme(dark), cx);
                cx.notify();
            })
        });
        draw(visual);
        assert_eq!(visual.update(|window, _| window.rem_size()), px(font_size));
        let header = visual.debug_bounds("native-problems-file-0").unwrap();
        let icon = visual.debug_bounds("native-problems-file-icon-0").unwrap();
        let row = visual.debug_bounds("native-problem-0").unwrap();
        assert!(icon.top() >= header.top() && icon.bottom() <= header.bottom());
        assert!(row.top() >= header.bottom());
    }
}

/// Hidden diagnostics leave the file triggers accessible, without replacing the list by an empty message.
fn check_all_collapsed_navigation(
    visual: &mut VisualTestContext,
    app: &Entity<EditorApp>,
    first: &str,
    second: &str,
) {
    visual.simulate_keystrokes("left");
    draw(visual);
    let header = visual.debug_bounds("native-problems-file-1").unwrap();
    visual.simulate_click(header.center(), Default::default());
    draw(visual);
    assert!(!file_expanded(visual, first));
    assert!(!file_expanded(visual, second));
    assert!(visual.debug_bounds("native-problem-0").is_none());
    assert!(visual.debug_bounds("native-problems-empty").is_none());
    assert!(visible_label(
        visual,
        &t!("problems.summary", count = 2, errors = 2, warnings = 0)
    ));
    // Removing the focused, collapsed group must restore panel focus even though its row count stays zero.
    visual.update(|_, cx| {
        app.update(cx, |app, cx| {
            let editor = app.tabs[1].text.as_ref().unwrap().editor.entity_id();
            app.apply_semantic_diagnostics(editor, Vec::new(), cx);
        })
    });
    draw(visual);
    assert!(!visible_label(visual, second));
    visual.simulate_keystrokes("home");
    visual.simulate_keystrokes("right");
    draw(visual);
    assert!(file_expanded(visual, first));
}

/// Each severity entry disappears independently, including the entire counter when all files clear.
#[gpui::test]
fn native_problems_nonzero_status_counts(cx: &mut TestAppContext) {
    let (app, visual, _directory) = test_editor(cx, "line\n");
    for (severities, expected) in [
        (vec![DiagnosticSeverity::Error], [true, false, false]),
        (vec![DiagnosticSeverity::Warning], [false, true, false]),
        (vec![DiagnosticSeverity::Info], [false, false, true]),
        (vec![DiagnosticSeverity::Hint], [false, false, true]),
        (
            vec![DiagnosticSeverity::Error, DiagnosticSeverity::Warning],
            [true, true, false],
        ),
        (Vec::new(), [false, false, false]),
    ] {
        visual.update(|_, cx| {
            app.update(cx, |app, cx| {
                let editor = app.tabs[0].text.as_ref().unwrap().editor.entity_id();
                app.apply_semantic_diagnostics(
                    editor,
                    severities
                        .into_iter()
                        .map(|severity| {
                            Diagnostic::new(Position::new(0, 0)..Position::new(0, 1), "Marker")
                                .with_severity(severity)
                        })
                        .collect(),
                    cx,
                );
            })
        });
        draw(visual);
        for (selector, visible) in [
            "problem-count-errors",
            "problem-count-warnings",
            "problem-count-information",
        ]
        .into_iter()
        .zip(expected)
        {
            assert_eq!(
                visual.debug_bounds(selector).is_some(),
                visible,
                "{selector}"
            );
        }
        assert_eq!(
            visual.debug_bounds("syntax-error-indicator").is_some(),
            expected.into_iter().any(|visible| visible)
        );
    }
}
