//! Ordinary footer visibility changes retain the user's split allocation across repeated cycles.

use super::*;
use gpui_base::dock::{DockArea, DockEvent, DockLayout, NodeId, PaneRef};

/// Locate the ordinary sibling slots through Base's public tree, retaining all sizes for diagnostics.
fn ordinary_split(
    area: &DockArea,
    placement: DockPlacement,
    ids: [PanelId; 5],
) -> (NodeId, [usize; 2], Vec<Option<Pixels>>) {
    let mut found = None;
    area.layout(placement).unwrap().root().walk(&mut |node| {
        let PaneRef::Split {
            axis,
            children,
            sizes,
        } = node.kind()
        else {
            return;
        };
        if axis != gpui_kit::Axis::Horizontal {
            return;
        }
        let positions = [ids[2], ids[3]].map(|id| {
            children.iter().position(|child| {
                matches!(child.kind(), PaneRef::Tabs { panels, .. } if panels.contains(&id))
            })
        });
        if let [Some(terminal), Some(shared)] = positions {
            found = Some((node.id(), [terminal, shared], sizes.to_vec()));
        }
    });
    found.expect("the ordinary terminal and plugin must remain sibling bottom slots")
}

/// Retain the Center sidebar used by existing saved layouts while placing either bottom variant.
fn center_sidebar(app: &Entity<EditorApp>, ids: [PanelId; 5], visual: &mut VisualTestContext) {
    visual.update(|window, cx| {
        app.read(cx).dock_area.clone().update(cx, |area, cx| {
            let node = area.layout(DockPlacement::Center).unwrap().root().id();
            area.move_panel(
                ids[1],
                gpui_base::dock::InsertTarget::Split {
                    node,
                    placement: gpui_base::Placement::Left,
                    size: Some(px(280.)),
                },
                window,
                cx,
            );
            assert!(
                area.layout(DockPlacement::Center)
                    .unwrap()
                    .find_panel_node(ids[1])
                    .is_some()
            );
        });
    });
    paint(visual);
}

/// A chosen 7/3 split must survive three real hide/reopen cycles without gradual shrinkage.
fn exercise_repeated_toggle(
    cx: &mut TestAppContext,
    center_bottom: bool,
    resize_while_hidden: bool,
) {
    let (_directory, app, visual) = setup(cx, center_bottom);
    let ids = panels(&app, visual);
    center_sidebar(&app, ids, visual);
    let before = [ids[0], ids[1], ids[3]].map(|id| native_panel_bounds(&app, id, visual));
    let placement = if center_bottom {
        DockPlacement::Center
    } else {
        DockPlacement::Bottom
    };
    click(visual, "terminal-toggle");
    visual.update(|window, cx| {
        app.read(cx).dock_area.clone().update(cx, |area, cx| {
            let (node, [terminal, shared], sizes) = ordinary_split(area, placement, ids);
            let mut widths = sizes
                .into_iter()
                .map(|value| value.unwrap_or(px(0.)))
                .collect::<Vec<_>>();
            widths[terminal] = px(700.);
            widths[shared] = px(300.);
            area.set_split_sizes(node, widths, window, cx);
        });
    });
    paint(visual);
    let original = [ids[2], ids[3]].map(|id| native_panel_bounds(&app, id, visual));
    assert!((original[0].size.width - before[2].size.width * 0.7).abs() < px(2.));
    assert!((original[1].size.width - before[2].size.width * 0.3).abs() < px(2.));
    let initial_sizes =
        visual.update(|_, cx| ordinary_split(app.read(cx).dock_area.read(cx), placement, ids).2);
    let mut observations = Vec::new();
    for cycle in 0..3 {
        click(visual, "terminal-toggle");
        assert!(visual.debug_bounds("native-terminal-output").is_none());
        if resize_while_hidden && cycle == 0 {
            visual.simulate_resize(size(px(1480.), px(900.)));
            paint(visual);
            visual.simulate_resize(size(px(1280.), px(900.)));
            paint(visual);
        }
        unchanged(before[2], native_panel_bounds(&app, ids[3], visual));
        let hidden_sizes = visual
            .update(|_, cx| ordinary_split(app.read(cx).dock_area.read(cx), placement, ids).2);
        click(visual, "terminal-toggle");
        assert_shared_bottom(&app, ids, before, visual);
        let painted = [ids[2], ids[3]].map(|id| native_panel_bounds(&app, id, visual));
        let restored_sizes = visual
            .update(|_, cx| ordinary_split(app.read(cx).dock_area.read(cx), placement, ids).2);
        observations.push((hidden_sizes, restored_sizes, painted));
    }
    // Observe all cycles before asserting the allocation, so a failure includes the full progression.
    for (cycle, (_, _, painted)) in observations.iter().enumerate() {
        for (expected, actual) in original.iter().zip(painted) {
            assert!(
                (expected.left() - actual.left()).abs() < px(2.)
                    && (expected.top() - actual.top()).abs() < px(2.)
                    && (expected.size.width - actual.size.width).abs() < px(2.)
                    && (expected.size.height - actual.size.height).abs() < px(2.),
                "ordinary visibility cycle {} changed {placement:?} allocation: original={original:?}, initial_sizes={initial_sizes:?}, cycles (hidden_sizes, restored_sizes, painted)={observations:?}",
                cycle + 1
            );
        }
    }
}

/// Native Bottom peers keep the same unequal widths whenever the terminal footer restores them.
#[gpui::test]
fn terminal_repeated_toggle_retains_native_bottom_widths(cx: &mut TestAppContext) {
    exercise_repeated_toggle(cx, false, false);
}

/// A bottom region relocated under Editor follows the same ordinary visibility contract.
#[gpui::test]
fn terminal_repeated_toggle_retains_center_bottom_widths(cx: &mut TestAppContext) {
    exercise_repeated_toggle(cx, true, false);
}

/// Resizing the viewport while Terminal is hidden preserves the ordinary split when it returns.
#[gpui::test]
fn terminal_repeated_toggle_retains_widths_after_hidden_resize(cx: &mut TestAppContext) {
    exercise_repeated_toggle(cx, true, true);
}

/// A hidden sole Center sidebar retains its pixel width when restored into a larger viewport.
#[gpui::test]
fn center_sidebar_reveal_retains_width_after_hidden_viewport_resize(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, false);
    let ids = panels(&app, visual);
    center_sidebar(&app, ids, visual);
    let before = native_panel_bounds(&app, ids[1], visual);
    assert!((before.size.width - px(280.)).abs() < px(2.));

    click(visual, "explorer-panel-toggle");
    visual.update(|_, cx| {
        assert!(
            !app.read(cx)
                .dock_area
                .read(cx)
                .panel(ids[1])
                .unwrap()
                .visible(cx)
        );
    });
    visual.simulate_resize(size(px(1480.), px(900.)));
    paint(visual);
    click(visual, "explorer-panel-toggle");

    let after = native_panel_bounds(&app, ids[1], visual);
    assert!(
        (after.size.width - before.size.width).abs() < px(2.),
        "restoring the sole Center sidebar after a hidden viewport resize must retain its pixel width: {before:?} -> {after:?}"
    );
}

/// Restore the real legacy shape with nonzero hidden preferences at two Editor ancestor levels.
fn legacy_hidden_peer_layout(app: &Entity<EditorApp>, visual: &mut VisualTestContext) {
    visual.update(|window, cx| {
        app.update(cx, |app, cx| {
            let leaf = |panel| DockLayout::tabs().panel_view(panel, cx);
            let presentation = &app.dock_presentation;
            let sidebar = leaf(presentation.panel_handle(app.explorer_panel.clone()));
            let editor = leaf(presentation.panel_handle(app.editor_panel.clone()));
            let messages = leaf(presentation.panel_handle(app.messages.clone()));
            let problems = leaf(presentation.panel_handle(app.problems.panel.clone()));
            let terminal = leaf(presentation.panel_handle(app.terminal.clone()));
            let shared =
                leaf(presentation.panel_handle(app.plugin_panels["region-fixture/shared"].clone()));
            let exclusive = leaf(
                presentation.panel_handle(app.plugin_panels["region-fixture/exclusive"].clone()),
            );
            assert!(!dock::BasePanel::visible(app.messages.read(cx), cx));
            assert!(!dock::BasePanel::visible(app.problems.panel.read(cx), cx));
            let layout = DockLayout::h_split()
                .child(sidebar, Some(px(437.)))
                .child(
                    DockLayout::v_split()
                        .child(
                            DockLayout::h_split()
                                .child(editor, Some(px(1440.)))
                                .child(messages, Some(px(100.))),
                            None,
                        )
                        .child(
                            DockLayout::h_split()
                                .child(shared, Some(px(720.)))
                                .child(terminal, Some(px(720.)))
                                .child(exclusive, Some(px(0.))),
                            Some(px(260.)),
                        ),
                    Some(px(1440.)),
                )
                .child(problems, Some(px(100.)));
            app.dock_area.update(cx, |area, cx| {
                for placement in [
                    DockPlacement::Left,
                    DockPlacement::Right,
                    DockPlacement::Bottom,
                ] {
                    area.remove_dock(placement, window, cx);
                }
                area.set_center(layout, window, cx);
            });
            cx.notify();
        });
    });
}

/// Consecutive frames expose alternating layouts without helper reads drawing extra hidden frames.
fn stable_legacy_frames(
    app: &Entity<EditorApp>,
    ids: [PanelId; 5],
    terminal_visible: bool,
    events: &std::cell::Cell<usize>,
    visual: &mut VisualTestContext,
) -> [Option<Bounds<Pixels>>; 4] {
    let mut frames = Vec::new();
    for _ in 0..24 {
        visual.update(|window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        });
        let (root, nodes) = visual.update(|_, cx| {
            let tree = app
                .read(cx)
                .dock_area
                .read(cx)
                .layout(DockPlacement::Center)
                .unwrap();
            (
                tree.root().id(),
                [ids[0], ids[1], ids[2], ids[3]].map(|id| tree.find_panel_node(id).unwrap()),
            )
        });
        let bounds = nodes.map(|node| {
            let selector = format!("local-dock-content-{}", node.as_u64());
            visual.debug_bounds(Box::leak(selector.into_boxed_str()))
        });
        assert!(bounds[0].is_some() && bounds[1].is_some() && bounds[3].is_some());
        assert_eq!(bounds[2].is_some(), terminal_visible);
        frames.push((root, bounds, events.get()));
    }
    // Allow startup reconciliation, then require sixteen successive frames to remain at rest.
    let settled = &frames[8..];
    assert!(
        settled.windows(2).all(|pair| pair[0] == pair[1]),
        "hidden legacy preferences must not continuously rebuild or resize the dock: frames (root, bounds, layout_events)={frames:?}"
    );
    settled.last().unwrap().1
}

/// Hidden legacy peers cannot cause idle layout churn or change the ordinary terminal allocation.
#[gpui::test]
fn legacy_hidden_peers_keep_idle_frames_and_terminal_toggle_stable(cx: &mut TestAppContext) {
    let (_directory, app, visual) = setup(cx, false);
    click(visual, "terminal-toggle");
    visual.simulate_resize(size(px(1877.), px(900.)));
    let ids = panels(&app, visual);
    let events = Rc::new(std::cell::Cell::new(0));
    let recorded = events.clone();
    let _subscription = visual.update(|_, cx| {
        app.update(cx, |app, cx| {
            cx.subscribe(&app.dock_area, move |_, _, event, _| {
                if matches!(event, DockEvent::LayoutChanged) {
                    recorded.set(recorded.get() + 1);
                    // A reentrant layout loop can prevent the next explicit frame from returning.
                    // Bound the public event stream so that the regression fails instead of hanging.
                    assert!(
                        recorded.get() <= 64,
                        "hidden legacy peers emitted more than 64 layout changes without a user layout edit"
                    );
                }
            })
        })
    });
    legacy_hidden_peer_layout(&app, visual);
    let before = stable_legacy_frames(&app, ids, true, &events, visual);
    assert!((before[1].unwrap().size.width - px(437.)).abs() < px(2.));
    click(visual, "terminal-toggle");
    stable_legacy_frames(&app, ids, false, &events, visual);
    click(visual, "terminal-toggle");
    let after = stable_legacy_frames(&app, ids, true, &events, visual);
    for (before, after) in before.into_iter().zip(after) {
        unchanged(before.unwrap(), after.unwrap());
    }
}
