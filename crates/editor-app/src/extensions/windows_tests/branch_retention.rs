//! Branch filtering and real dock toggles preserve loaded provider pages and native review state.
use super::*;
use gpui_base::test_support::{ElementSnapshot, snapshots};
use gpui_kit::{ElementId, FocusHandle, MouseButton, Role, ScrollDelta, ScrollEvent, point, px};
use plugin_runtime::{TreeResource, TreeSnapshot, plugin_protocol::ui};
use std::sync::Arc;

/// Scope observations to the actual branch tree, permitting renewed source-action credentials.
fn branch_node(window: &Window, role: Role, label: Option<&str>) -> ElementSnapshot {
    snapshots(window)
        .into_iter()
        .find(|node| {
            node.role() == Some(role)
                && node.visible()
                && label.is_none_or(|label| node.label() == Some(label))
                && node.path().iter().any(|part| {
                    matches!(part, ElementId::Name(name)
                        if name.as_ref() == "plugin-tree-branches"
                            || name.starts_with("plugin-tree-branches-action-"))
                })
        })
        .unwrap_or_else(|| panic!("Missing native branches {role:?} {label:?}"))
}

/// Deliver a physical row press; double-click is the real native folder activation gesture.
fn press_row(visual: &mut VisualTestContext, label: &str, clicks: usize) {
    let position = visual.update(|window, _| {
        let row = branch_node(window, Role::TreeItem, Some(label));
        assert_ne!(
            row.disabled(),
            Some(true),
            "branch row must accept the original press"
        );
        row.bounds().center()
    });
    visual.simulate_event(gpui_kit::MouseDownEvent {
        position,
        button: MouseButton::Left,
        modifiers: Default::default(),
        click_count: clicks,
        first_mouse: false,
    });
    visual.simulate_event(gpui_kit::MouseUpEvent {
        position,
        button: MouseButton::Left,
        modifiers: Default::default(),
        click_count: clicks,
    });
    visual.run_until_parked();
}

/// Click the original window-strip control, including its normal focus and visibility receipts.
fn toggle_panel(visual: &mut VisualTestContext, panel: &str) {
    let selector: &'static str = Box::leak(format!("plugin-window-git/{panel}").into_boxed_str());
    let position = visual
        .debug_bounds(selector)
        .expect("real panel toggle")
        .center();
    visual.simulate_click(position, Default::default());
    visual.run_until_parked();
}

/// Follow the real scene's owned handle; a replacement resource is not allowed to hide a reload.
fn branches(manager: &Manager) -> Arc<TreeResource> {
    let ui::Kind::Tree { handle } = &manager.live["git"].views["history"]
        .active_node("branches")
        .expect("current branch tree remains admitted")
        .kind
    else {
        panic!("actual branches scene node is a tree")
    };
    manager.live["git"]
        .tree_resources()
        .into_iter()
        .find(|resource| resource.handle() == handle)
        .expect("instance-owned branch provider")
}

/// Selecting a filter preserves epoch; hiding may revoke old callbacks without reloading cached pages.
/// `strict_epoch` distinguishes that security boundary from destructive provider refresh.
fn assert_loaded(
    manager: &Manager,
    original: &TreeResource,
    before: &TreeSnapshot,
    trigger: &str,
    strict_epoch: bool,
) {
    let current = branches(manager);
    assert_eq!(
        current.handle(),
        original.handle(),
        "{trigger}: replaced tree ownership"
    );
    let after = current.snapshot();
    if strict_epoch {
        assert_eq!(
            after.epoch, before.epoch,
            "{trigger}: reloaded unchanged branch data"
        );
    }
    assert_eq!(
        after.nodes.keys().collect::<Vec<_>>(),
        before.nodes.keys().collect::<Vec<_>>(),
        "{trigger}: discarded cached provider nodes"
    );
    assert_eq!(
        after.expanded, before.expanded,
        "{trigger}: collapsed native prefixes"
    );
    for (parent, page) in before.branches.iter().filter(|(_, page)| page.loaded) {
        let now = after
            .branches
            .get(parent)
            .expect("loaded page remains present");
        assert!(
            now.loaded && !now.loading && now.error.is_none(),
            "{trigger}: page {parent:?} returned to loading/error",
        );
        assert_eq!(
            now.children, page.children,
            "{trigger}: changed unchanged page"
        );
        assert_eq!(now.next, page.next, "{trigger}: lost cached continuation");
    }
}

/// User-visible continuity uses the selected leaf's position, not private TreeState internals.
struct Retained {
    /// Nonzero scroll is observed by the same source leaf's offset inside its native viewport.
    leaf_top: gpui_kit::Pixels,
    /// The original real tree focus handle must still receive keyboard input after reopening.
    focus: FocusHandle,
}

/// Read the selected row before any refocusing click, so that click cannot conceal lost selection.
fn assert_native(visual: &mut VisualTestContext, retained: &Retained, trigger: &str) {
    visual.update(|window, _| {
        assert!(
            !snapshots(window).iter().any(|node| {
                node.visible()
                    && node.role() == Some(Role::TreeItem)
                    && node.path().iter().any(|part| {
                        matches!(part, ElementId::Name(name)
                            if name.as_ref() == "plugin-tree-branches"
                                || name.starts_with("plugin-tree-branches-action-"))
                    })
                    && node.path().iter().any(|part| {
                        matches!(part, ElementId::Name(name)
                            if name.starts_with("plugin-tree-row-@loading/")
                                || name.starts_with("plugin-tree-row-@lazy/"))
                    })
            }),
            "{trigger}: retained branch pages cannot become loading/lazy placeholders",
        );
        let tree = branch_node(window, Role::Tree, None);
        let row = branch_node(window, Role::TreeItem, Some("topic-01"));
        assert_eq!(
            row.selected(),
            Some(true),
            "{trigger}: lost selected branch"
        );
        assert_ne!(
            tree.disabled(),
            Some(true),
            "{trigger}: disabled native tree"
        );
        assert_ne!(
            row.disabled(),
            Some(true),
            "{trigger}: disabled selected branch"
        );
        let top = row.bounds().top() - tree.bounds().top();
        assert!(
            (top - retained.leaf_top).abs() < px(1.),
            "{trigger}: changed branch scroll: {top:?} != {:?}",
            retained.leaf_top,
        );
    });
}

/// The real independent ZIP is exercised through Manager, worker publication and native App controls.
#[gpui::test]
#[ignore = "real independent Git candidate and native GPUI; set NANOBUG_GIT_CANDIDATE"]
fn git_branch_filter_and_dock_toggles_retain_loaded_native_tree_state(cx: &mut TestAppContext) {
    if !run_isolated_test(
        "extensions::windows_tests::branch_retention::git_branch_filter_and_dock_toggles_retain_loaded_native_tree_state",
    ) {
        return;
    }
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        let root = path.parent().unwrap();
        assert!(git(root, &["init", "-b", "main"]));
        for (key, value) in [
            ("core.autocrlf", "false"),
            ("user.name", "Native retention"),
            ("user.email", "retention@example.invalid"),
        ] {
            assert!(git(root, &["config", key, value]));
        }
        assert!(git(root, &["add", "shortcuts.txt"]));
        assert!(git(root, &["commit", "-m", "Branch retention base"]));
        // A real overflow is essential: a tree with three rows could conceal a reset-to-top bug.
        for index in 0..24 {
            assert!(git(root, &["branch", &format!("feature/topic-{index:02}")]));
        }
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
        for panel in ["history", "staging"] {
            let shown = visual.update(|_, cx| {
                app.read(cx).plugin_panels[&format!("git/{panel}")]
                    .read(cx)
                    .visible
                    .get()
            });
            if !shown {
                toggle_panel(visual, panel);
                flush(&mut manager, &app, visual);
            }
        }
        let local = visual.update(|window, _| {
            snapshots(window)
                .into_iter()
                .find(|node| {
                    node.role() == Some(Role::TreeItem)
                        && node.visible()
                        && matches!(node.label(), Some("Local" | "本地"))
                })
                .expect("localized native local branch root")
                .label()
                .unwrap()
                .to_owned()
        });
        // Initial location now reveals the checked-out branch's ancestors before user input.
        if !visual.update(|window, _| {
            branch_node(window, Role::TreeItem, Some(&local)).expanded() == Some(true)
        }) {
            press_row(visual, &local, 2);
            flush(&mut manager, &app, visual);
        }
        press_row(visual, "feature", 2);
        flush(&mut manager, &app, visual);
        let resource = branches(&manager);
        let before = resource.snapshot();
        assert!(
            before.expanded.len() >= 2,
            "both native disclosures were delivered"
        );
        let original_focus = visual
            .update(|window, cx| window.focused(cx).expect("expanded native tree owns focus"));
        press_row(visual, "topic-01", 2);
        flush(&mut manager, &app, visual);
        assert_loaded(&manager, &resource, &before, "branch selection", true);
        visual.update(|window, cx| {
            assert_eq!(
                window.focused(cx).as_ref(),
                Some(&original_focus),
                "filter completion replaced the original tree's native focus"
            );
        });
        let unscrolled = visual.update(|window, _| {
            branch_node(window, Role::TreeItem, Some("topic-01"))
                .bounds()
                .top()
                - branch_node(window, Role::Tree, None).bounds().top()
        });
        let position =
            visual.update(|window, _| branch_node(window, Role::Tree, None).bounds().center());
        visual.simulate_event(ScrollEvent {
            // Wheel and trackpad fixtures retain pointer-device semantics.
            source: gpui_kit::ScrollSource::Pointer,
            position,
            delta: ScrollDelta::Pixels(point(px(0.), px(-26.))),
            ..Default::default()
        });
        flush(&mut manager, &app, visual);
        let retained = visual.update(|window, cx| {
            let top = branch_node(window, Role::TreeItem, Some("topic-01"))
                .bounds()
                .top()
                - branch_node(window, Role::Tree, None).bounds().top();
            assert!(
                unscrolled - top > px(5.),
                "one native wheel must create nonzero scroll"
            );
            assert_eq!(window.focused(cx).as_ref(), Some(&original_focus));
            Retained {
                leaf_top: top,
                focus: original_focus,
            }
        });
        assert_native(visual, &retained, "after native wheel");
        for panel in ["staging", "history"] {
            for visible in [false, true] {
                toggle_panel(visual, panel);
                assert_loaded(
                    &manager,
                    &resource,
                    &before,
                    "immediate native dock toggle",
                    false,
                );
                flush(&mut manager, &app, visual);
                let trigger = format!("{panel} visible={visible}");
                assert_loaded(&manager, &resource, &before, &trigger, false);
                visual.update(|_, cx| {
                    assert_eq!(
                        app.read(cx).plugin_panels[&format!("git/{panel}")]
                            .read(cx)
                            .visible
                            .get(),
                        visible,
                        "real dock toggle {trigger}"
                    );
                });
                if panel != "history" || visible {
                    assert_native(visual, &retained, &trigger);
                }
            }
        }
        // Explicit dock shows intentionally focus their panel. Check the preserved row/scroll
        // first, then one ordinary click must return to the same original tree focus authority.
        assert_native(visual, &retained, "before refocusing retained row");
        press_row(visual, "topic-01", 1);
        flush(&mut manager, &app, visual);
        visual.update(|window, cx| {
            assert_eq!(
                window.focused(cx).as_ref(),
                Some(&retained.focus),
                "reopened tree replaced its original native focus identity"
            );
        });
        assert_native(visual, &retained, "after refocusing retained row");
        // Real filtering after a visibility cycle must retain state under the renewed credential.
        let reopened = resource.snapshot();
        for label in ["topic-02", "topic-01"] {
            press_row(visual, label, 2);
            flush(&mut manager, &app, visual);
            assert_loaded(
                &manager,
                &resource,
                &reopened,
                "filter after reopening",
                true,
            );
        }
        assert_native(visual, &retained, "after filtering reopened tree");
        assert_loaded(
            &manager,
            &resource,
            &before,
            "completed visibility cycle",
            false,
        );
        manager.disable("git").unwrap();
        flush(&mut manager, &app, visual);
        assert_eq!(manager.resource_count(), 0);
    });
}

/// A real nested branch beyond the first lazy page seeds native selection and the complete first preview.
#[gpui::test]
#[ignore = "real independent Git candidate with ui.tree 1.3; set NANOBUG_GIT_CANDIDATE"]
fn git_initial_selection_reveals_paged_current_branch_and_colored_first_commit(
    cx: &mut TestAppContext,
) {
    if !run_isolated_test(
        "extensions::windows_tests::branch_retention::git_initial_selection_reveals_paged_current_branch_and_colored_first_commit",
    ) {
        return;
    }
    crate::tests::with_shortcut_editor(cx, false, Vec::new(), |visual, app, path| {
        let root = path.parent().unwrap();
        assert!(git(root, &["init", "-b", "main"]));
        for (key, value) in [
            ("core.autocrlf", "false"),
            ("user.name", "Initial author"),
            ("user.email", "initial@example.invalid"),
        ] {
            assert!(git(root, &["config", key, value]));
        }
        std::fs::write(root.join("deleted.txt"), "old\n").unwrap();
        std::fs::write(root.join("modified.txt"), "old\n").unwrap();
        assert!(git(root, &["add", "."]));
        assert!(git(root, &["commit", "-m", "Base"]));
        for index in 0..140 {
            assert!(git(root, &["branch", &format!("feature/topic-{index:03}")]));
        }
        assert!(git(root, &["switch", "feature/topic-139"]));
        std::fs::remove_file(root.join("deleted.txt")).unwrap();
        std::fs::write(root.join("modified.txt"), "changed\n").unwrap();
        std::fs::write(root.join("added.txt"), "new\n").unwrap();
        assert!(git(root, &["add", "-A"]));
        assert!(git(
            root,
            &["commit", "-m", "Initial preview", "-m", "完整提交说明 😀"]
        ));
        let original_focus = visual.update(|window, cx| window.focused(cx));
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
        let resource = branches(&manager);
        // Lazy tree replies do not replace the guest scene or start Git processes. Keep advancing
        // the real native pulse until the paged initial path has completed, not just Git's queue.
        for _ in 0..12 {
            if resource.snapshot().initial_selection.is_none() {
                break;
            }
            flush(&mut manager, &app, visual);
        }
        let initial = resource.snapshot();
        assert!(
            initial.initial_selection.is_none(),
            "native default path completed"
        );
        let prefix = initial
            .nodes
            .values()
            .find(|node| node.label == "feature" && node.branch)
            .unwrap();
        assert_eq!(
            initial.branches[&Some(prefix.id.clone())].children.len(),
            140
        );
        assert!(initial.expanded.contains(&prefix.id));
        visual.update(|window, cx| {
            assert_eq!(
                branch_node(window, Role::TreeItem, Some("topic-139")).selected(),
                Some(true)
            );
            assert_eq!(
                window.focused(cx),
                original_focus,
                "default positioning must not steal focus"
            );
            assert!(app.read(cx).comparison_views.is_empty());
            assert!(
                app.read(cx)
                    .plugin_windows
                    .get("git/commit-details")
                    .is_none()
            );
        });
        let history = &manager.live["git"].views["history"];
        assert!(
            matches!(&history.active_node("history-subject").unwrap().kind, ui::Kind::Text { text } if text == "Initial preview")
        );
        assert!(
            matches!(&history.active_node("history-message-0").unwrap().kind, ui::Kind::Text { text } if text.contains("完整提交说明 😀"))
        );
        let ui::Kind::Tree { handle } = &history.active_node("files").unwrap().kind else {
            panic!("file tree");
        };
        let files = manager.live["git"]
            .tree_resources()
            .into_iter()
            .find(|tree| tree.handle() == handle)
            .unwrap()
            .snapshot();
        for (name, tone) in [
            (
                "added.txt",
                plugin_runtime::plugin_protocol::tree::Tone::Positive,
            ),
            (
                "deleted.txt",
                plugin_runtime::plugin_protocol::tree::Tone::Negative,
            ),
            (
                "modified.txt",
                plugin_runtime::plugin_protocol::tree::Tone::Informative,
            ),
        ] {
            assert_eq!(
                files
                    .nodes
                    .values()
                    .find(|node| node.label == name)
                    .unwrap()
                    .tone,
                Some(tone)
            );
        }
        press_row(visual, "topic-138", 2);
        flush(&mut manager, &app, visual);
        let retained = resource.snapshot();
        for panel in ["staging", "history"] {
            for _ in 0..2 {
                toggle_panel(visual, panel);
                flush(&mut manager, &app, visual);
            }
        }
        visual.update(|window, _| {
            assert_eq!(
                branch_node(window, Role::TreeItem, Some("topic-138")).selected(),
                Some(true)
            )
        });
        assert_eq!(resource.snapshot().expanded, retained.expanded);
        assert_eq!(
            resource.snapshot().nodes.keys().collect::<Vec<_>>(),
            retained.nodes.keys().collect::<Vec<_>>()
        );
        manager.disable("git").unwrap();
        flush(&mut manager, &app, visual);
        assert_eq!(manager.resource_count(), 0);
    });
}
