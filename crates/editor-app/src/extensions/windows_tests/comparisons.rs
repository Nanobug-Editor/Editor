//! Real package comparisons retain native source edits and hide immutable snapshot tabs.
use super::*;
mod adjacent;
mod source_tabs;

/// Both native actions remain present and expose their independent availability after tab activation.
fn assert_fold_action(visual: &mut VisualTestContext, any_hidden: bool) {
    visual.update(|window, _| {
        let snapshots = gpui_base::test_support::snapshots(window);
        for (selector, label, disabled) in [
            (
                "diff-collapse-unchanged",
                t!("editor.diff_collapse_unchanged"),
                any_hidden,
            ),
            (
                "diff-expand-unchanged",
                t!("editor.diff_expand_unchanged"),
                !any_hidden,
            ),
        ] {
            let button = snapshots
                .iter()
                .find(|node| {
                    node.role() == Some(gpui_kit::Role::Button)
                    && node.path().iter().any(|part| {
                        matches!(part, gpui_kit::ElementId::Name(name) if name == selector)
                    })
                })
                .unwrap_or_else(|| panic!("missing independent action {selector}"));
            assert_eq!(button.label(), Some(label.as_ref()));
            if let Some(actual) = button.disabled() {
                assert_eq!(actual, disabled);
            }
        }
    });
}

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
    // Preserve the native test body inside the same private Git configuration as its Manager.
    if !run_isolated_test(
        "extensions::windows_tests::comparisons::git_comparison_tabs_keep_unsaved_source_edits_and_hide_snapshot_tabs",
    ) {
        return;
    }
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
        // A user-opened file remains a visible tab when the plugin borrows its native session.
        visual
            .update(|window, cx| app.update(cx, |app, cx| app.open_file(file.clone(), window, cx)));
        manager
            .invoke_command("git", "diff", json!({"path":"diff.txt"}))
            .unwrap();
        flush(&mut manager, &app, visual);
        assert!(visual.debug_bounds("document-diff-left").is_some());
        assert!(visual.debug_bounds("document-diff-right").is_some());
        for dark in [false, true] {
            visual.update(|_, cx| {
                crate::ui::theme::apply_theme(crate::ui::theme::builtin_theme(dark), cx)
            });
            paint_comparison(visual);
            super::comparison_chrome::assert_comparison_chrome(visual, 2);
        }
        let (source, index) = visual.update(|_, cx| {
            let app = app.read(cx);
            let index = app
                .tabs
                .iter()
                .position(|tab| tab.path() == file.canonicalize().unwrap())
                .unwrap();
            (app.tabs[index].text.as_ref().unwrap().editor.clone(), index)
        });
        // The real independent package supplies both SVGs through editor.diff 1.4 admission.
        // Native controls fold presentation rows, not the source buffer or its line identities.
        assert!(visual.debug_bounds("diff-toggle-unchanged").is_none());
        let collapse = visual
            .debug_bounds("diff-collapse-unchanged")
            .expect("separate collapse action");
        let expand = visual
            .debug_bounds("diff-expand-unchanged")
            .expect("separate expand action");
        let down = visual.debug_bounds("diff-next").unwrap();
        assert!(collapse.left() >= down.right() && expand.left() >= collapse.right());
        assert_eq!(collapse.top(), down.top());
        assert_eq!(expand.top(), down.top());
        assert_fold_action(visual, false);
        let unchanged = changed.find("line 30\n").unwrap();
        let original_selection = visual.update(|_, cx| source.read(cx).selected_range());
        click_count(visual, "diff-collapse-unchanged", 1);
        paint_comparison(visual);
        assert_fold_action(visual, true);
        visual.update(|_, cx| {
            let group = app.read(cx).comparison_alignment().unwrap();
            assert!(group.project_range(1, unchanged..unchanged, cx).is_none());
            assert_eq!(source.read(cx).value().as_ref(), changed);
            assert_eq!(source.read(cx).selected_range(), original_selection);
        });
        // This comparison has one long common interval. Its physical disclosure must update
        // both toolbar actions, even though no toolbar or document edit caused the action.
        let disclosure = visual.update(|window, _| {
            gpui_base::test_support::snapshots(window)
                .into_iter()
                .find(|node| {
                    node.visible()
                        && node.role() == Some(gpui_kit::Role::Button)
                        && node.label().is_some_and(|label| {
                            label.ends_with(" unchanged lines") || label.ends_with("行无差异内容")
                        })
                })
                .expect("visible native unchanged-region disclosure")
                .bounds()
                .center()
        });
        visual.simulate_click(disclosure, Default::default());
        paint_comparison(visual);
        assert_fold_action(visual, false);
        click_count(visual, "diff-collapse-unchanged", 1);
        paint_comparison(visual);
        assert_fold_action(visual, true);
        click_count(visual, "diff-expand-unchanged", 1);
        paint_comparison(visual);
        assert_fold_action(visual, false);
        visual.update(|_, cx| {
            let group = app.read(cx).comparison_alignment().unwrap().clone();
            assert!(group.reveal_offset(1, unchanged, cx));
        });
        paint_comparison(visual);
        visual.update(|_, cx| {
            let group = app.read(cx).comparison_alignment().unwrap();
            assert!(group.project_range(1, unchanged..unchanged, cx).is_some());
            assert_eq!(source.read(cx).selected_range(), original_selection);
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
        // Pinning and activating an ordinary file cannot reset this view's manual fold choices.
        click_count(visual, "diff-collapse-unchanged", 1);
        paint_comparison(visual);
        click_count(visual, &format!("editor-tab-{index}"), 1);
        click_count(visual, &comparison_selector, 1);
        paint_comparison(visual);
        visual.update(|_, cx| {
            assert!(
                app.read(cx)
                    .comparison_alignment()
                    .unwrap()
                    .project_range(1, unchanged..unchanged, cx)
                    .is_none(),
                "retained comparison must keep its folded regions"
            );
        });
        assert_fold_action(visual, true);
        click_count(visual, "diff-expand-unchanged", 1);
        paint_comparison(visual);
        visual.update(|_, cx| {
            let group = app.read(cx).comparison_alignment().unwrap().clone();
            assert!(group.reveal_offset(1, 0, cx));
        });
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
        assert!(visual.debug_bounds("diff-unified").is_none());
        assert!(visual.debug_bounds("diff-sync").is_none());
        // Native arrows reveal differences while the authoritative source keeps text and selection.
        let source_selection = visual.update(|_, cx| source.read(cx).selected_range());
        click_count(visual, "diff-next", 1);
        paint_comparison(visual);
        let target = draft.find("last change").unwrap();
        let body = visual.debug_bounds("document-diff-right").unwrap();
        let projected = visual.update(|_, cx| {
            app.read(cx)
                .comparison_alignment()
                .unwrap()
                .project_range(1, target..target + 4, cx)
                .unwrap()
        });
        assert!(
            body.intersects(&projected),
            "down arrow reveals the second real difference"
        );
        click_count(visual, "diff-previous", 1);
        paint_comparison(visual);
        let target = draft.find("first 中").unwrap();
        let projected = visual.update(|_, cx| {
            app.read(cx)
                .comparison_alignment()
                .unwrap()
                .project_range(1, target..target + 4, cx)
                .unwrap()
        });
        assert!(
            body.intersects(&projected),
            "up arrow reveals the first real difference"
        );
        assert_eq!(
            visual.update(|_, cx| source.read(cx).selected_range()),
            source_selection,
            "difference navigation must not move the original source caret"
        );
        assert_eq!(
            visual.update(|_, cx| source.read(cx).value().to_string()),
            draft
        );
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
    // Preserve the native test body inside the same private Git configuration as its Manager.
    if !run_isolated_test(
        "extensions::windows_tests::comparisons::git_comparison_refresh_reuses_hidden_source_and_source_close_retires_view",
    ) {
        return;
    }
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
    // Preserve the native test body inside the same private Git configuration as its Manager.
    if !run_isolated_test(
        "extensions::windows_tests::comparisons::git_comparison_soft_wrapped_scroll_tracks_source_without_moving_selections",
    ) {
        return;
    }
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
        // Measure visible native glyphs: a zero-width caret exactly on the inward gutter's
        // body edge is not a rectangle intersection, despite the word itself being visible.
        let left_word = anchor_left..anchor_left + "shared".len();
        let right_word = anchor_right..anchor_right + "shared".len();
        // Establish the starting position from the actual viewport, not an assumed 42px wrap
        // height for five logical rows. Toolbar/column changes can make those rows only 21px.
        visual.update(|_, cx| {
            let group = app.read(cx).comparison_alignment().unwrap().clone();
            assert!(group.reveal_offset(1, anchor_right, cx));
        });
        paint_comparison(visual);
        let (position, lead_in) = visual.update(|_, cx| {
            let viewport = app
                .read(cx)
                .comparison_alignment()
                .unwrap()
                .viewport(1, cx)
                .unwrap();
            (
                viewport.center(),
                (viewport.size.height * 0.6).min(px(240.)),
            )
        });
        visual.simulate_event(gpui_kit::ScrollEvent {
            // Wheel and trackpad fixtures retain pointer-device semantics.
            source: gpui_kit::ScrollSource::Pointer,
            position,
            delta: gpui_kit::ScrollDelta::Pixels(point(px(0.), lead_in)),
            ..Default::default()
        });
        paint_comparison(visual);
        visual.update(|_, cx| {
            let app = app.read(cx);
            let group = app.comparison_alignment().unwrap();
            let viewport = group.viewport(1, cx).unwrap();
            let anchor = group
                .project_range(1, right_word.clone(), cx)
                .unwrap_or_else(|| {
                    let native = right.read(cx).range_to_bounds(&(anchor_right..anchor_right));
                    panic!(
                        "shared anchor missing before wheel: status={:?}, left_viewport={:?}, right_viewport={viewport:?}, native035={native:?}, projected035={:?}, native_input={:?}, line_height={:?}",
                        group.status(cx),
                        group.viewport(0, cx),
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
                        .project_range(1, right_word.clone(), cx)
                        .unwrap()
                        .top(),
                )
            });
            visual.simulate_event(gpui_kit::ScrollEvent {
                // Wheel and trackpad fixtures retain pointer-device semantics.
                source: gpui_kit::ScrollSource::Pointer,
                position,
                delta: gpui_kit::ScrollDelta::Pixels(point(px(0.), px(delta))),
                ..Default::default()
            });
            paint_comparison(visual);
            visual.update(|_, cx| {
                let app = app.read(cx);
                let group = app.comparison_alignment().unwrap();
                let a = group.project_range(0, left_word.clone(), cx).unwrap();
                let b = group.project_range(1, right_word.clone(), cx).unwrap();
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
