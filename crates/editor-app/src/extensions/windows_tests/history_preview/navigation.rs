//! Real changed-file selection and toolbar presses reveal measured differences without editing sources.
use super::*;
use gpui_kit::px;
mod boundaries;

/// Long common context separates three known changes and keeps every target away from scroll clamps.
fn source(version: usize) -> String {
    (0..240)
        .map(|row| match (version, row) {
            (1, 80) => "feature first 中😀\n".into(),
            (2, 80) => "main first 中😀\n".into(),
            (3, 80) => format!("merge first 中😀 {}\n", "wrapped context ".repeat(20)),
            (1 | 3, 140) => "feature second 中😀\n".into(),
            (2 | 3, 180) => "main third 中😀\n".into(),
            _ => format!("shared row {row:03}\n"),
        })
        .collect()
}

/// Prepare a real merge and ordinary commits; all writes stay in the isolated fixture repository.
fn repository(root: &Path) {
    for args in [
        vec!["init", "-b", "main"],
        vec!["config", "core.autocrlf", "false"],
        vec!["config", "user.name", "Navigation fixture"],
        vec!["config", "user.email", "navigation@example.invalid"],
    ] {
        assert!(git(root, &args));
    }
    let file = root.join("shared.txt");
    std::fs::write(&file, source(0)).unwrap();
    assert!(git(root, &["add", "shared.txt"]));
    assert!(git(root, &["commit", "-m", "base"]));
    assert!(git(root, &["switch", "-c", "feature"]));
    std::fs::write(&file, source(1)).unwrap();
    assert!(git(root, &["commit", "-am", "feature"]));
    assert!(git(root, &["switch", "main"]));
    std::fs::write(&file, source(2)).unwrap();
    assert!(git(root, &["commit", "-am", "main"]));
    assert!(!git(root, &["merge", "--no-ff", "--no-commit", "feature"]));
    std::fs::write(&file, source(3)).unwrap();
    assert!(git(root, &["add", "shared.txt"]));
    assert!(git(root, &["commit", "-m", "merged"]));
}

/// Locate the subject in the real published log; topo-order does not promise parent sibling order.
fn select_subject(manager: &mut Manager, subject: &str) {
    let ui::Kind::Canvas(canvas) = &manager.live["git"].views["history"]
        .active_node("log")
        .unwrap()
        .kind
    else {
        panic!("native history canvas")
    };
    let row = canvas
        .paint
        .iter()
        .find_map(|paint| match paint {
            plugin_runtime::plugin_protocol::Paint::Text { text, y, .. } if text == subject => {
                Some(((y - 5. + canvas.scroll.as_ref().unwrap().offset) / 28.).round() as usize)
            }
            plugin_runtime::plugin_protocol::Paint::TextBox { text, rect, .. }
                if text == subject =>
            {
                Some(((rect.y + canvas.scroll.as_ref().unwrap().offset) / 28.).floor() as usize)
            }
            _ => None,
        })
        .expect("fixture subject must be in the loaded history");
    select_row(manager, row);
}

/// A settled process queue is weaker than complete metadata; wait for this commit's admitted tree.
fn await_history(
    manager: &mut Manager,
    app: &Entity<EditorApp>,
    visual: &mut VisualTestContext,
    subject: &str,
) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        flush(manager, app, visual);
        let scene = &manager.live["git"].views["history"];
        let exact = matches!(scene.active_node("history-subject").map(|node| &node.kind), Some(ui::Kind::Text { text }) if text == subject);
        if exact
            && scene.active_node("files").is_some()
            && scene.active_node("history-loading").is_none()
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "commit metadata not admitted; scene={scene:?}"
        );
    }
}

/// Editor admission follows the process receipt, so observe actual source and title before input.
fn await_comparison(
    manager: &mut Manager,
    app: &Entity<EditorApp>,
    visual: &mut VisualTestContext,
    file: &str,
    version: usize,
) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        flush(manager, app, visual);
        if visual.update(|_, cx| {
            let app = app.read(cx);
            app.comparison_views.iter().any(|view| {
                Some(&view.id) == app.active_comparison.as_ref() && view.title.ends_with(file)
            }) && app.editor.read(cx).value().as_ref() == source(version)
        }) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "comparison not admitted; scene={:?}",
            manager.live["git"].views["history"]
        );
    }
}

/// Deliver normal frame callbacks until pending native measurements and viewport requests settle.
fn frames(visual: &mut VisualTestContext) {
    for _ in 0..64 {
        visual.run_until_parked();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
            window.simulate_next_frame(cx);
        });
    }
}

/// Compare a known changed line's actual glyph bounds with the content rail, excluding headers.
fn centered(
    app: &Entity<EditorApp>,
    visual: &mut VisualTestContext,
    version: usize,
    marker: &str,
    panes: usize,
) {
    frames(visual);
    let selector: &'static str =
        Box::leak(format!("aligned-vertical-scrollbar-{}", panes - 1).into_boxed_str());
    let viewport = visual.debug_bounds(selector).expect("shared content rail");
    let byte = source(version).find(marker).expect("known fixture change");
    let bounds = visual.update(|_, cx| {
        app.read(cx)
            .comparison_alignment()
            .unwrap()
            .project_range(1, byte..byte, cx)
            .unwrap_or_else(|| panic!("changed file must initially reveal {marker}"))
    });
    assert!(
        (bounds.center().y - viewport.center().y).abs() <= px(12.),
        "{marker} must be centered in the content viewport: glyph={bounds:?}, viewport={viewport:?}"
    );
}

/// Snapshot existing text and selections through their native owners, independent of navigation state.
fn sources(
    app: &Entity<EditorApp>,
    visual: &mut VisualTestContext,
) -> Vec<(String, std::ops::Range<usize>)> {
    visual.update(|_, cx| {
        app.read(cx)
            .tabs
            .iter()
            .filter_map(|tab| {
                tab.text.as_ref().map(|text| {
                    let editor = text.editor.read(cx);
                    (editor.value().to_string(), editor.selected_range())
                })
            })
            .collect()
    })
}

/// Render the existing retained unified mode and press the same toolbar; sources stay read-only owners.
fn unified_navigation(app: &Entity<EditorApp>, visual: &mut VisualTestContext) {
    visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            let id = app.active_comparison.clone().unwrap();
            app.comparison_views
                .iter_mut()
                .find(|view| view.id == id)
                .unwrap()
                .unified = true;
            app.activate_comparison_view(&id, false, window, cx);
        })
    });
    frames(visual);
    let source_before = sources(app, visual);
    let unified_before = visual.update(|_, cx| {
        app.read(cx)
            .inspect_unified_comparison(cx, |editor| {
                (editor.value().to_string(), editor.selected_range())
            })
            .unwrap()
    });
    for (arrow, marker) in [
        ("diff-next", "-shared row 180"),
        ("diff-previous", "-shared row 080"),
    ] {
        press(visual, arrow.into(), 1);
        frames(visual);
        visual.update(|_, cx| {
            app.read(cx)
                .inspect_unified_comparison(cx, |editor| {
                    let value = editor.value().to_string();
                    let byte = value.find(marker).expect("known unified hunk start");
                    let glyph = editor
                        .range_to_bounds(&(byte..byte))
                        .expect("unified hunk visible");
                    let viewport = editor.input_bounds();
                    assert!(
                        (glyph.center().y - viewport.center().y).abs() <= px(12.),
                        "unified {marker}: {glyph:?} in {viewport:?}"
                    );
                    assert_eq!(
                        (value, editor.selected_range()),
                        unified_before,
                        "unified navigation must not select or edit derived text"
                    );
                })
                .unwrap()
        });
        assert_eq!(
            sources(app, visual),
            source_before,
            "unified navigation must preserve source text and selection"
        );
    }
    visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            let id = app.active_comparison.clone().unwrap();
            app.comparison_views
                .iter_mut()
                .find(|view| view.id == id)
                .unwrap()
                .unified = false;
            app.activate_comparison_view(&id, false, window, cx);
        })
    });
    frames(visual);
}

/// A file tree click initializes the first difference; both arrows center later changes in either layout.
#[gpui::test]
#[ignore = "real independent Git ZIP and native comparison; set NANOBUG_GIT_CANDIDATE"]
fn git_changed_file_and_difference_arrows_center_native_viewports(cx: &mut TestAppContext) {
    if !run_isolated_test(
        "extensions::windows_tests::history_preview::navigation::git_changed_file_and_difference_arrows_center_native_viewports",
    ) {
        return;
    }
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        let root = path.parent().unwrap();
        repository(root);
        let package = Package::read(Path::new(
            &std::env::var_os("NANOBUG_GIT_CANDIDATE").expect("candidate ZIP"),
        ))
        .unwrap();
        let profile = tempfile::tempdir().unwrap();
        let env = visual.update(|_, cx| environment(root, cx));
        let mut manager = Manager::open(profile.path().into(), env).unwrap();
        manager
            .install(&package, package.manifest.permissions.clone())
            .unwrap();
        package_ui_test_support::demand(
            &mut manager,
            "git",
            plugin_runtime::DemandReason::View("history".into()),
        )
        .unwrap();
        flush(&mut manager, &app, visual);
        for (subject, version, panes, first, next) in [
            ("merged", 3, 3, "merge first 中😀", "feature second 中😀"),
            ("main", 2, 2, "main first 中😀", "main third 中😀"),
        ] {
            select_subject(&mut manager, subject);
            await_history(&mut manager, &app, visual, subject);
            press(
                visual,
                format!("plugin-tree-row-{}", changed_file(&manager)),
                1,
            );
            await_comparison(&mut manager, &app, visual, "shared.txt", version);
            assert!(
                visual.update(
                    |_, cx| app.read(cx).editor.read(cx).value().as_ref() == source(version)
                ),
                "selected comparison must contain the exact requested commit's source"
            );
            centered(&app, visual, version, first, panes);
            let before = sources(&app, visual);
            press(visual, "diff-previous".into(), 1);
            assert!(
                visual.debug_bounds("centered-notice").is_some(),
                "first difference must warn instead of wrapping"
            );
            centered(&app, visual, version, first, panes);
            press(visual, "diff-next".into(), 1);
            centered(&app, visual, version, next, panes);
            press(visual, "diff-previous".into(), 1);
            centered(&app, visual, version, first, panes);
            assert_eq!(
                sources(&app, visual),
                before,
                "navigation must not edit or select source text"
            );
            if panes == 2 {
                unified_navigation(&app, visual);
            }
        }
        // A root commit adds the whole file and has no common region. Both controls remain
        // visible, and native activation of either disabled action must leave source state intact.
        select_subject(&mut manager, "base");
        await_history(&mut manager, &app, visual, "base");
        press(
            visual,
            format!("plugin-tree-row-{}", changed_file(&manager)),
            1,
        );
        await_comparison(&mut manager, &app, visual, "shared.txt", 0);
        let before = sources(&app, visual);
        for (id, label) in [
            (
                "diff-collapse-unchanged",
                t!("editor.diff_collapse_unchanged"),
            ),
            ("diff-expand-unchanged", t!("editor.diff_expand_unchanged")),
        ] {
            visual.update(|window, _| {
                let nodes = snapshots(window);
                let button = nodes
                    .iter()
                    .find(|node| {
                        node.role() == Some(gpui_kit::Role::Button)
                            && node.label() == Some(label.as_ref())
                    })
                    .expect("disabled comparison action remains visible");
                if let Some(disabled) = button.disabled() {
                    assert!(disabled);
                }
            });
            press(visual, id.into(), 1);
            frames(visual);
            assert_eq!(sources(&app, visual), before);
        }
    });
}
