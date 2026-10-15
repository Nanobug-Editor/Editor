//! Real package comparisons retain native source edits and hide immutable snapshot tabs.
use super::*;

/// Pointer activation exercises Base tab selection and the local preview-pinning callback.
fn click_count(visual: &mut VisualTestContext, selector: &str, count: usize) {
    // GPUI debug lookups require static names; these bounded test names live for the test process.
    let selector: &'static str = Box::leak(selector.to_owned().into_boxed_str());
    let position = visual
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("Missing {selector}"))
        .center();
    visual.simulate_event(MouseDownEvent {
        position,
        button: MouseButton::Left,
        modifiers: Default::default(),
        click_count: count,
        first_mouse: false,
    });
    visual.simulate_event(gpui_kit::MouseUpEvent {
        position,
        button: MouseButton::Left,
        modifiers: Default::default(),
        click_count: count,
    });
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
}

/// An actual WASM provider opens a comparison, edits its right document, switches and pins the view.
#[gpui::test]
#[ignore = "build Git-for-Nanobug candidate with editor.diff 1.1; set NANOBUG_GIT_CANDIDATE"]
fn git_comparison_tabs_keep_unsaved_source_edits_and_hide_snapshot_tabs(cx: &mut TestAppContext) {
    crate::tests::with_shortcut_editor(cx, true, Vec::new(), |visual, app, path| {
        let root = path.parent().unwrap();
        assert!(git(root, &["init", "-b", "main"]));
        assert!(git(root, &["config", "core.autocrlf", "false"]));
        assert!(git(root, &["config", "user.name", "fixture"]));
        assert!(git(
            root,
            &["config", "user.email", "fixture@example.invalid"]
        ));
        let file = root.join("diff.txt");
        let base = (0..60)
            .map(|index| format!("line {index}\n"))
            .collect::<String>();
        std::fs::write(&file, &base).unwrap();
        std::fs::write(root.join("second.txt"), "base\n").unwrap();
        assert!(git(root, &["add", "--", "diff.txt", "second.txt"]));
        assert!(git(root, &["commit", "-m", "base"]));
        let changed = base
            .replace("line 1\n", "first 中😀\n")
            .replace("line 55\n", "last change\n");
        std::fs::write(&file, &changed).unwrap();
        std::fs::write(root.join("second.txt"), "second change\n").unwrap();
        let package = Package::read(Path::new(
            &std::env::var_os("NANOBUG_GIT_CANDIDATE").expect("candidate required"),
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
            plugin_runtime::DemandReason::Command("refresh".into()),
        )
        .unwrap();
        flush(&mut manager, &app, visual);
        manager
            .invoke_command("git", "diff", json!({"path":"diff.txt"}))
            .unwrap();
        flush(&mut manager, &app, visual);
        assert!(visual.debug_bounds("document-diff-left").is_some());
        assert!(visual.debug_bounds("document-diff-right").is_some());
        let (source, index) = visual.update(|_, cx| {
            let app = app.read(cx);
            let index = app
                .tabs
                .iter()
                .position(|tab| tab.path() == file.canonicalize().unwrap())
                .unwrap();
            (app.tabs[index].text.as_ref().unwrap().editor.clone(), index)
        });
        let normal_count = visual.update(|_, cx| {
            app.read(cx)
                .tabs
                .iter()
                .filter(|tab| {
                    tab.virtual_document
                        .as_ref()
                        .is_none_or(|source| !source.hidden)
                })
                .count()
        });
        let comparison_selector = format!("comparison-tab-{normal_count}");
        click_count(visual, &comparison_selector, 2);
        paint_comparison(visual);
        let source_point = visual.update(|_, cx| {
            let bounds = app
                .read(cx)
                .comparison_alignment()
                .unwrap()
                .project_range(1, 0..0, cx)
                .unwrap();
            point(bounds.left() + px(0.4), bounds.center().y)
        });
        visual.simulate_click(source_point, Default::default());
        visual.update(|window, cx| {
            assert!(
                source.focus_handle(cx).is_focused(window),
                "the visible aligned source owns real input focus"
            );
            assert_eq!(source.read(cx).cursor(), 0);
        });
        visual.simulate_keystrokes("ctrl-end");
        visual.simulate_input("native draft 中文😀\n");
        flush(&mut manager, &app, visual);
        assert!(
            visual.debug_bounds("document-diff-left").is_some(),
            "typing must keep the comparison mounted"
        );
        let draft = visual.update(|_, cx| source.read(cx).value().to_string());
        assert!(draft.ends_with("native draft 中文😀\n"));
        assert_eq!(std::fs::read_to_string(&file).unwrap(), changed);
        click_count(visual, &format!("editor-tab-{index}"), 1);
        assert!(visual.debug_bounds("document-diff-left").is_none());
        assert_eq!(visual.update(|_, cx| app.read(cx).editor.clone()), source);
        click_count(visual, &comparison_selector, 1);
        assert!(visual.debug_bounds("document-diff-left").is_some());
        click_count(visual, "diff-unified", 1);
        assert!(visual.debug_bounds("unified-diff").is_some());
        assert!(visual.debug_bounds("document-diff-right").is_none());
        // A real click and successful Copy establish that typing targets the unified editor,
        // rather than a toolbar button that would also ignore input without testing readonly.
        click_count(visual, "unified-diff", 1);
        visual.simulate_keystrokes("ctrl-a ctrl-c");
        let unified = clipboard(visual);
        assert!(unified.contains("-line 1\n+first 中😀\n"));
        assert!(unified.contains("+native draft 中文😀\n"));
        visual.simulate_input("must stay readonly");
        visual.simulate_keystrokes("ctrl-a ctrl-c");
        assert_eq!(clipboard(visual), unified);
        assert_eq!(
            visual.update(|_, cx| source.read(cx).value().to_string()),
            draft
        );
        // The second hunk is far below the first viewport. Keyboard selection after navigation
        // proves the visible unified control receives the jump, not the unmounted right source.
        let source_selection = visual.update(|_, cx| source.read(cx).selected_range());
        visual.simulate_keystrokes("ctrl-home");
        click_count(visual, "diff-next", 1);
        visual.simulate_keystrokes("shift-end ctrl-c");
        assert_eq!(clipboard(visual), "-line 55");
        assert_eq!(
            visual.update(|_, cx| source.read(cx).selected_range()),
            source_selection,
            "unified navigation must not move the hidden right source caret"
        );
        click_count(visual, "diff-unified", 1);
        manager
            .invoke_command("git", "diff", json!({"path":"second.txt"}))
            .unwrap();
        flush(&mut manager, &app, visual);
        assert_eq!(
            visual.update(|_, cx| app.read(cx).comparison_views.len()),
            2,
            "a pinned comparison coexists with another preview"
        );
        assert_eq!(
            visual.update(|_, cx| source.read(cx).value().to_string()),
            draft
        );
        let hidden_indices = visual.update(|_, cx| {
            app.read(cx)
                .tabs
                .iter()
                .enumerate()
                .filter(|(_, tab)| {
                    tab.virtual_document
                        .as_ref()
                        .is_some_and(|source| source.hidden)
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>()
        });
        for index in hidden_indices {
            assert!(
                visual
                    .debug_bounds(Box::leak(format!("editor-tab-{index}").into_boxed_str()))
                    .is_none()
            );
        }
        manager.disable("git").unwrap();
        flush(&mut manager, &app, visual);
    });
}

/// Copy is an observable native input action, independent of private comparison presentation fields.
fn clipboard(visual: &VisualTestContext) -> String {
    visual
        .read_from_clipboard()
        .and_then(|item| item.text())
        .unwrap_or_default()
}

/// Install the real candidate and open one comparison through its public command contribution.
fn with_comparison(
    cx: &mut TestAppContext,
    base: &str,
    changed: &str,
    run: impl FnOnce(&mut Manager, &mut VisualTestContext, &Entity<EditorApp>, &Path),
) {
    crate::tests::with_shortcut_editor(cx, true, Vec::new(), |visual, app, path| {
        let root = path.parent().unwrap();
        assert!(git(root, &["init", "-b", "main"]));
        assert!(git(root, &["config", "core.autocrlf", "false"]));
        assert!(git(root, &["config", "user.name", "fixture"]));
        assert!(git(
            root,
            &["config", "user.email", "fixture@example.invalid"]
        ));
        let file = root.join("diff.txt");
        std::fs::write(&file, base).unwrap();
        assert!(git(root, &["add", "--", "diff.txt"]));
        assert!(git(root, &["commit", "-m", "base"]));
        std::fs::write(&file, changed).unwrap();
        let package = Package::read(Path::new(
            &std::env::var_os("NANOBUG_GIT_CANDIDATE").expect("candidate required"),
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
            plugin_runtime::DemandReason::Command("refresh".into()),
        )
        .unwrap();
        flush(&mut manager, &app, visual);
        manager
            .invoke_command("git", "diff", json!({"path":"diff.txt"}))
            .unwrap();
        flush(&mut manager, &app, visual);
        assert!(visual.debug_bounds("document-diff-left").is_some());
        run(&mut manager, visual, &app, &file);
        manager.disable("git").unwrap();
        flush(&mut manager, &app, visual);
    });
}

/// Repeated public opens reuse their retained snapshot and closing the real source reclaims it.
#[gpui::test]
#[ignore = "build Git-for-Nanobug candidate with editor.diff 1.1; set NANOBUG_GIT_CANDIDATE"]
fn git_comparison_refresh_reuses_hidden_source_and_source_close_retires_view(
    cx: &mut TestAppContext,
) {
    with_comparison(cx, "base\n", "changed\n", |manager, visual, app, file| {
        let mut previous_source = None;
        for iteration in 0..40 {
            manager
                .invoke_command("git", "diff", json!({"path":"diff.txt"}))
                .unwrap();
            flush(manager, app, visual);
            let source = visual.update(|_, cx| {
                let app = app.read(cx);
                assert_eq!(app.comparison_views.len(), 1, "iteration {iteration}");
                let hidden = app
                    .tabs
                    .iter()
                    .filter(|tab| {
                        tab.virtual_document
                            .as_ref()
                            .is_some_and(|source| source.hidden)
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    hidden.len(),
                    1,
                    "retired sources must close at iteration {iteration}"
                );
                hidden[0].text.as_ref().unwrap().editor.entity_id()
            });
            if let Some(previous_source) = previous_source {
                assert_eq!(
                    previous_source, source,
                    "the real guest reuses its live snapshot"
                );
            }
            previous_source = Some(source);
            assert!(visual.debug_bounds("document-diff-left").is_some());
        }
        paint_comparison(visual);
        let source_point = visual.update(|_, cx| {
            let bounds = app
                .read(cx)
                .comparison_alignment()
                .unwrap()
                .project_range(1, 0..0, cx)
                .unwrap();
            point(bounds.left() + px(0.4), bounds.center().y)
        });
        visual.simulate_click(source_point, Default::default());
        visual.simulate_keystrokes("ctrl-a ctrl-c");
        assert_eq!(
            clipboard(visual),
            "changed\n",
            "the refreshed pane still targets the original native document"
        );
        visual.update(|window, cx| {
            app.update(cx, |app, cx| {
                app.close_tab(file.canonicalize().unwrap(), window, cx);
            })
        });
        flush(manager, app, visual);
        visual.update(|_, cx| {
            let app = app.read(cx);
            assert!(app.comparison_views.is_empty());
            assert!(app.tabs.iter().all(|tab| {
                tab.virtual_document
                    .as_ref()
                    .is_none_or(|source| !source.hidden)
            }));
        });
        assert!(visual.debug_bounds("document-diff-left").is_none());
    });
}

/// Differently wrapped changed lines require source anchors, not logical-row multiplied by pixels.
#[gpui::test]
#[ignore = "build Git-for-Nanobug candidate with editor.diff 1.1; set NANOBUG_GIT_CANDIDATE"]
fn git_comparison_soft_wrapped_scroll_tracks_source_without_moving_selections(
    cx: &mut TestAppContext,
) {
    let tail = (0..100)
        .map(|line| format!("shared row {line:03} 中文\n"))
        .collect::<String>();
    let base = format!("{}\n{tail}", "left long 中文 ".repeat(160));
    let changed = format!("{}\n{tail}", "right longer 😀 ".repeat(320));
    with_comparison(cx, &base, &changed, |_, visual, app, file| {
        let (left, right) = visual.update(|window, cx| {
            let app = app.read(cx);
            let left = app
                .tabs
                .iter()
                .find(|tab| {
                    tab.virtual_document
                        .as_ref()
                        .is_some_and(|source| source.hidden)
                })
                .unwrap()
                .text
                .as_ref()
                .unwrap()
                .editor
                .clone();
            let right = app
                .tabs
                .iter()
                .find(|tab| tab.path() == file.canonicalize().unwrap())
                .unwrap()
                .text
                .as_ref()
                .unwrap()
                .editor
                .clone();
            for editor in [&left, &right] {
                editor.update(cx, |editor, cx| {
                    editor.set_soft_wrap(true, window, cx);
                    editor.set_selected_range(1..4, cx);
                });
            }
            (left, right)
        });
        paint_comparison(visual);
        let anchor_left = base.find("shared row 035").unwrap();
        let anchor_right = changed.find("shared row 035").unwrap();
        // Narrow production columns wrap each common source row to 42px. Five rows leave the
        // anchor below the first 160px wheel movement while keeping it inside the 320px viewport;
        // ten logical rows would already be below the visible pane before scrolling starts.
        let reveal_right = changed.find("shared row 030").unwrap();
        visual.update(|_, cx| {
            let group = app.read(cx).comparison_alignment().unwrap().clone();
            assert!(group.reveal_offset(1, reveal_right, cx));
        });
        paint_comparison(visual);
        visual.update(|_, cx| {
            let app = app.read(cx);
            let group = app.comparison_alignment().unwrap();
            let viewport = group.viewport(1, cx).unwrap();
            let anchor = group
                .project_range(1, anchor_right..anchor_right, cx)
                .unwrap_or_else(|| {
                    let native = right.read(cx).range_to_bounds(&(anchor_right..anchor_right));
                    panic!(
                        "shared anchor missing before wheel: status={:?}, left_viewport={:?}, right_viewport={viewport:?}, reveal030={:?}, native035={native:?}, projected035={:?}, native_input={:?}, line_height={:?}",
                        group.status(cx),
                        group.viewport(0, cx),
                        group.project_range(1, reveal_right..reveal_right, cx),
                        native.and_then(|native| group.project_bounds(1, native, cx)),
                        right.read(cx).input_bounds(),
                        right.read(cx).line_height(),
                    )
                });
            assert!(
                anchor.top() - viewport.top() > px(160.) && anchor.bottom() < viewport.bottom(),
                "the shared anchor must have visible space for both wheel checks: {anchor:?} in {viewport:?}"
            );
        });
        for (driver, delta) in [(1, -160.), (0, 120.)] {
            let (position, before) = visual.update(|_, cx| {
                let app = app.read(cx);
                let group = app.comparison_alignment().unwrap();
                (
                    group.viewport(driver, cx).unwrap().center(),
                    group
                        .project_range(1, anchor_right..anchor_right, cx)
                        .unwrap()
                        .top(),
                )
            });
            visual.simulate_event(gpui_kit::ScrollWheelEvent {
                position,
                delta: gpui_kit::ScrollDelta::Pixels(point(px(0.), px(delta))),
                ..Default::default()
            });
            paint_comparison(visual);
            visual.update(|_, cx| {
                let app = app.read(cx);
                let group = app.comparison_alignment().unwrap();
                let a = group.project_range(0, anchor_left..anchor_left, cx).unwrap();
                let b = group.project_range(1, anchor_right..anchor_right, cx).unwrap();
                let left_viewport = group.viewport(0, cx).unwrap();
                let right_viewport = group.viewport(1, cx).unwrap();
                assert!(left_viewport.intersects(&a) && right_viewport.intersects(&b));
                assert!((b.top() - before).abs() > px(20.), "real wheel must move painted source geometry");
                assert!((a.top() - b.top()).abs() < px(1.),
                    "matching text after differently wrapped rows must align in the actual panes: {a:?} vs {b:?}");
                assert_eq!(left.read(cx).selected_range(), 1..4);
                assert_eq!(right.read(cx).selected_range(), 1..4);
                assert_eq!(left.read(cx).value().to_string(), base);
                assert_eq!(right.read(cx).value().to_string(), changed);
            });
        }
    });
}

/// Native projection measurement and locator follow-ups run on real frame callbacks, not just draw.
fn paint_comparison(visual: &mut VisualTestContext) {
    for _ in 0..64 {
        visual.run_until_parked();
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
            // TestWindow has no platform frame loop. Deliver the same scheduled notifications
            // used by the production window, without forcing unconditional layout refreshes.
            window.simulate_next_frame(cx);
        });
    }
}
