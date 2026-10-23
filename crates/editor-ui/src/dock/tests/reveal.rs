//! Reopening a closed native region must respect a side occupied while it was collapsed.

use super::*;
use gpui_base::dock::DockLayout;
use gpui_kit::{Axis, VisualTestContext, gpui};

/// Named native leaves expose identity and resource retirement without plugin-specific behavior.
struct RevealPanel {
    /// The editor name supplies the existing generic reveal anchor lookup.
    name: &'static str,
    /// Stable focus identity survives moves between native and center-local regions.
    focus: FocusHandle,
    /// Revealing a retained leaf must never retire its content resources.
    removals: usize,
}

impl EventEmitter<PanelEvent> for RevealPanel {}

impl Focusable for RevealPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for RevealPanel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

impl Panel for RevealPanel {
    fn panel_name(&self) -> &'static str {
        self.name
    }

    fn on_removed(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.removals += 1;
    }
}

/// Build two raw-visible leaves in a closed Bottom dock and optionally occupy Center's bottom.
/// Returned views keep native resources available for assertions after the public reveal action.
fn setup(
    cx: &mut TestAppContext,
    occupied: bool,
) -> (
    Entity<DockArea>,
    [Entity<RevealPanel>; 4],
    &mut VisualTestContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::typography::init(cx);
        crate::theme::apply_theme(crate::theme::builtin_theme(false), cx);
    });
    let slot = Rc::new(RefCell::new(None));
    let capture = slot.clone();
    let (_, visual) = cx.add_window_view(move |window, cx| {
        let panels = ["Editor", "Reveal", "Retain", "CenterBottom"].map(|name| {
            cx.new(|cx| RevealPanel {
                name,
                focus: cx.focus_handle(),
                removals: 0,
            })
        });
        let area = LocalDock::new(28.).create_area("closed-region-reveal", None, window, cx);
        area.update(cx, |area, cx| {
            let editor = DockLayout::tabs().panel(panels[0].clone());
            let center = if occupied {
                DockLayout::v_split()
                    .child(editor, None)
                    .child(DockLayout::tabs().panel(panels[3].clone()), Some(px(260.)))
            } else {
                editor
            };
            area.set_center(center, window, cx);
            area.set_dock(
                DockPlacement::Bottom,
                DockLayout::h_split()
                    .child(DockLayout::tabs().panel(panels[1].clone()), Some(px(400.)))
                    .child(DockLayout::tabs().panel(panels[2].clone()), Some(px(600.))),
                window,
                cx,
            );
            area.set_dock_collapsible(DockPlacement::Bottom, true, window, cx);
            if area.is_dock_open(DockPlacement::Bottom) {
                area.toggle_dock(DockPlacement::Bottom, window, cx);
            }
        });
        *capture.borrow_mut() = Some((area.clone(), panels));
        gpui_base::Root::new(area, window, cx)
    });
    let (area, panels) = slot.borrow_mut().take().unwrap();
    visual.simulate_resize(size(px(1000.), px(700.)));
    visual.run_until_parked();
    visual.update(|window, cx| window.draw(cx).clear(cx));
    (area, panels, visual)
}

/// A collapsed combination cannot reserve its side through its remaining raw-visible peers.
#[gpui::test]
fn reveal_from_closed_bottom_joins_existing_center_band(cx: &mut TestAppContext) {
    let (area, panels, visual) = setup(cx, true);
    let ids = panels
        .each_ref()
        .map(|panel| PanelId::from(panel.entity_id()));
    visual.update(|window, cx| {
        area.update(cx, |area, cx| {
            assert!(!area.is_dock_open(DockPlacement::Bottom));
            assert!(!panel_visible(area, ids[1], cx));
            reveal_panel(area, ids[1], window, cx);
            assert!(
                !area.is_dock_open(DockPlacement::Bottom),
                "revealing one leaf must not reopen a second occupied bottom region"
            );
            let center = area.layout(DockPlacement::Center).unwrap();
            let revealed_node = center.find_panel_node(ids[1]).unwrap();
            let peer_node = center.find_panel_node(ids[3]).unwrap();
            let mut shared_row = false;
            center.root().walk(&mut |node| {
                if let PaneRef::Split { axis, children, .. } = node.kind() {
                    shared_row |= axis == Axis::Horizontal
                        && children.iter().any(|child| child.id() == revealed_node)
                        && children.iter().any(|child| child.id() == peer_node);
                }
            });
            assert!(
                shared_row,
                "ordinary bottom leaves must share one horizontal row"
            );
            let bottom = area.layout(DockPlacement::Bottom).unwrap();
            assert!(bottom.find_panel_node(ids[1]).is_none());
            assert!(bottom.find_panel_node(ids[2]).is_some());
            assert!(panel_visible(area, ids[1], cx));
            assert!(!panel_visible(area, ids[2], cx));
            for panel in &panels {
                let id = PanelId::from(panel.entity_id());
                assert_eq!(
                    area.panel(id).unwrap().view().entity_id(),
                    panel.entity_id()
                );
                assert!(
                    panel.read(cx).visible(cx),
                    "raw visibility must remain enabled"
                );
                assert_eq!(
                    panel.read(cx).removals,
                    0,
                    "native resources must survive reveal"
                );
            }
        });
    });
}

/// Without a competing same-side band, reopening retains the original native combination.
#[gpui::test]
fn reveal_from_closed_bottom_reopens_unoccupied_side(cx: &mut TestAppContext) {
    let (area, panels, visual) = setup(cx, false);
    let ids = panels
        .each_ref()
        .map(|panel| PanelId::from(panel.entity_id()));
    visual.update(|window, cx| {
        area.update(cx, |area, cx| {
            assert!(!area.is_dock_open(DockPlacement::Bottom));
            reveal_panel(area, ids[1], window, cx);
            assert!(area.is_dock_open(DockPlacement::Bottom));
            let bottom = area.layout(DockPlacement::Bottom).unwrap();
            assert_eq!(bottom.panels().collect::<Vec<_>>(), vec![ids[1], ids[2]]);
            for panel in &panels[1..3] {
                let id = PanelId::from(panel.entity_id());
                assert!(panel_visible(area, id, cx));
                assert_eq!(
                    area.panel(id).unwrap().view().entity_id(),
                    panel.entity_id()
                );
                assert!(panel.read(cx).visible(cx));
                assert_eq!(panel.read(cx).removals, 0);
            }
        });
    });
}
